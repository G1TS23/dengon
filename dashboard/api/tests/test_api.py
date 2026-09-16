import json
import sqlite3

import pytest


def test_healthz(client):
    response = client.get("/healthz")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


def test_ingest_accepts_arbitrary_object(client):
    response = client.post(
        "/ingest/batch",
        json={"peu_importe": "quoi", "events": [{"a": 1}, {"b": 2}, {"c": 3}]},
    )
    assert response.status_code == 202
    body = response.json()
    assert body["stored"] is True
    assert body["event_count"] == 3
    assert body["batch_id"]


def test_ingest_accepts_bare_array(client):
    response = client.post("/ingest/batch", json=[{"a": 1}, {"b": 2}])
    assert response.status_code == 202
    assert response.json()["event_count"] == 2


def test_ingest_stores_body_verbatim(client):
    payload = {"hello": "wörld", "nested": {"x": [1, 2]}, "no_events_key": True}
    response = client.post("/ingest/batch", json=payload)
    batch_id = response.json()["batch_id"]

    from app.db import connect

    conn = connect()
    row = conn.execute(
        "SELECT body, event_count FROM raw_batches WHERE batch_id = ?",
        (batch_id,),
    ).fetchone()
    conn.close()

    assert row is not None
    assert json.loads(row["body"]) == payload
    assert row["event_count"] is None  # pas de clé "events" → indéterminé, mais accepté


def test_ingest_rejects_non_json(client):
    response = client.post(
        "/ingest/batch",
        content=b"ceci n'est pas du json",
        headers={"content-type": "application/json"},
    )
    assert response.status_code == 400


def test_ingest_rejects_deeply_nested_json(client):
    # json.loads (accélérateur C) recourt à la pile Python au-delà d'une
    # certaine profondeur d'imbrication et lève RecursionError, pas
    # JSONDecodeError — vérifié en local : 10000 niveaux la déclenchent (2000
    # ne suffisent pas), pour un corps de ~20 Ko (retour de revue #59, round
    # 2). Sans le fix, cette requête remonte un 500 brut au lieu du 400
    # attendu pour un corps invalide.
    body = ("[" * 10_000 + "]" * 10_000).encode()
    response = client.post(
        "/ingest/batch",
        content=body,
        headers={"content-type": "application/json"},
    )
    assert response.status_code == 400


def test_ingest_rejects_non_utf8_json(client):
    # JSON valide mais encodé en UTF-16 : json.loads l'accepterait, mais le
    # stocker en texte le corromprait (retour de revue #59, point 1).
    body = json.dumps({"events": [{"x": 1}]}).encode("utf-16")
    response = client.post(
        "/ingest/batch",
        content=body,
        headers={"content-type": "application/json"},
    )
    assert response.status_code == 400


def test_ingest_rejects_body_over_max_size(client, monkeypatch):
    # Garde-fou mémoire : un corps plus grand que la limite configurée est
    # rejeté sans être stocké (retour de revue #59, point 1). httpx pose
    # toujours Content-Length pour du contenu `bytes` : ce test n'exerce que
    # le fast-path Content-Length, pas la boucle de comptage en flux — voir
    # test_ingest_rejects_chunked_body_over_max_size ci-dessous pour le cas
    # malveillant réel (retour de revue #59, round 2, point de Paul).
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "16")
    response = client.post(
        "/ingest/batch",
        content=b'{"events": [1, 2, 3, 4, 5, 6, 7, 8, 9]}',  # > 16 octets
        headers={"content-type": "application/json"},
    )
    assert response.status_code == 413

    from app.db import connect

    conn = connect()
    count = conn.execute("SELECT COUNT(*) AS n FROM raw_batches").fetchone()["n"]
    conn.close()
    assert count == 0, "un corps rejeté pour taille ne doit laisser aucune ligne"


def test_ingest_rejects_chunked_body_over_max_size(client, monkeypatch):
    # Cas malveillant réel : Content-Length absent (chunked) ou mensonger, la
    # seule protection est alors le comptage dans `async for morceau in
    # request.stream()`. Un contenu `bytes` chez httpx pose toujours
    # Content-Length ; passer un générateur force l'encodage chunked, donc
    # exerce vraiment cette boucle (retour de revue #59, round 2, point de
    # Paul — les deux tests de taille précédents ne passaient que par le
    # fast-path Content-Length).
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "16")

    def _morceaux():
        yield b'{"events": ['
        yield b"1, 2, 3, 4, 5, 6, 7, 8, 9"
        yield b"]}"

    response = client.post(
        "/ingest/batch",
        content=_morceaux(),
        headers={"content-type": "application/json"},
    )
    assert response.status_code == 413

    from app.db import connect

    conn = connect()
    count = conn.execute("SELECT COUNT(*) AS n FROM raw_batches").fetchone()["n"]
    conn.close()
    assert count == 0


def test_ingest_accepts_body_within_max_size(client, monkeypatch):
    # Le garde-fou ne doit pas rejeter un batch qui tient dans la limite.
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "4096")
    response = client.post("/ingest/batch", json={"events": []})
    assert response.status_code == 202


def test_a_single_connection_is_reused_for_all_writes(tmp_path, monkeypatch):
    # La connexion ouverte au démarrage doit servir à toutes les écritures :
    # pas de connect() par requête (retour de revue #59, point 3).
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "reuse.db"))
    from app.db import connect as vraie_connect

    ouvertures = []

    def _connect_comptee():
        conn = vraie_connect()
        ouvertures.append(conn)
        return conn

    monkeypatch.setattr("app.main.connect", _connect_comptee)

    from fastapi.testclient import TestClient

    from app.main import app

    with TestClient(app) as c:
        for _ in range(5):
            response = c.post("/ingest/batch", json={"events": []})
            assert response.status_code == 202

    assert len(ouvertures) == 1, "une seule connexion doit être ouverte pour les 5 écritures"


def test_ingest_returns_503_when_connection_closed_under_a_write(client):
    # Course entre l'arrêt du lifespan (ferme app.state.db_conn dans son
    # `finally`) et une écriture encore en cours sur le threadpool : seul
    # sqlite3.OperationalError était rattrapé, pas sqlite3.ProgrammingError
    # ("Cannot operate on a closed database"), qui remontait un 500 brut au
    # lieu du 503 attendu (retour de revue #59, round 2). Reproduit ici sans
    # vraie course : fermer la connexion partagée avant le POST suffit à
    # produire la même ProgrammingError.
    client.app.state.db_conn.close()
    response = client.post("/ingest/batch", json={"events": []})
    assert response.status_code == 503
    assert response.headers.get("Retry-After") == "1"


def test_concurrent_writes_are_not_lost(client):
    # La connexion partagée + le verrou doivent tenir sous des écritures
    # concurrentes issues du threadpool (retour de revue #59, points 2 et 3).
    import concurrent.futures

    def _post(i: int):
        return client.post("/ingest/batch", json={"events": [], "i": i})

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        reponses = list(pool.map(_post, range(20)))

    assert all(r.status_code == 202 for r in reponses)
    batch_ids = {r.json()["batch_id"] for r in reponses}
    assert len(batch_ids) == 20, "pas de collision d'id (uuid4 unique, pas une preuve à elle seule)"

    # La vraie preuve qu'aucune écriture n'a été perdue : 20 lignes en base,
    # pas seulement 20 batch_id distincts renvoyés par l'API (retour de revue
    # #59, round 2, point de Paul — uuid4 est unique par construction, ça ne
    # dit rien sur le nombre de lignes réellement insérées sous concurrence).
    from app.db import connect

    conn = connect()
    stored = conn.execute("SELECT COUNT(*) AS n FROM raw_batches").fetchone()["n"]
    conn.close()
    assert stored == 20, "20 requêtes acceptées doivent laisser 20 lignes, pas moins"


def test_ingest_returns_503_when_storage_is_locked(client, monkeypatch):
    # Le chemin d'erreur « base verrouillée » : un flush concurrent perd la
    # course et le nœud doit retenter (retour de revue #59, point 3).
    def _boom(*_args, **_kwargs):
        raise sqlite3.OperationalError("database is locked")

    monkeypatch.setattr("app.main._store_raw_batch", _boom)
    response = client.post("/ingest/batch", json={"events": []})
    assert response.status_code == 503
    assert response.headers.get("Retry-After") == "1"


def test_migrations_applied_once(client):
    from app.db import connect, run_migrations

    conn = connect()
    # Le lifespan les a déjà appliquées : un second passage ne fait rien.
    assert run_migrations(conn) == []
    versions = [r["version"] for r in conn.execute("SELECT version FROM schema_migrations")]
    conn.close()
    assert versions == [1]


def test_migrations_idempotent_after_partial_apply(tmp_path, monkeypatch):
    # Simule un arrêt entre le DDL et l'INSERT dans schema_migrations :
    # la reprise ne doit pas planter (retour de revue #59, point 4).
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "partial.db"))
    from app.db import connect, run_migrations
    from app.migrations import MIGRATIONS

    conn = connect()
    for statement in MIGRATIONS[0][2]:  # applique le DDL sans enregistrer la version
        conn.execute(statement)

    assert run_migrations(conn) == [1]  # se termine proprement grâce à IF NOT EXISTS
    assert [r["version"] for r in conn.execute("SELECT version FROM schema_migrations")] == [1]
    conn.close()


@pytest.mark.parametrize("payload", [{"events": []}, [], {"a": 1}])
def test_ingest_event_count_shapes(client, payload):
    response = client.post("/ingest/batch", json=payload)
    assert response.status_code == 202
