import json
import sqlite3

import pytest


def _apply_migrations_in_subprocess(db_path: str, start_barrier, result_queue) -> None:
    """Cible d'un process séparé (voir `test_migrations_are_safe_across_processes`).

    Fonction de haut niveau, pas une fermeture : `multiprocessing` avec la
    méthode de démarrage `spawn` (défaut sur macOS) exige une cible picklable,
    donc importable par son nom — une closure ne le serait pas. Le résultat
    passe par une `Queue` plutôt qu'une valeur de retour : avec `Process` (pas
    `Pool`), rien ne récupère la valeur de retour de la cible.
    """
    import os
    import sys
    from pathlib import Path

    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    os.environ["DENGON_DASHBOARD_DB"] = db_path

    from app.db import connect, run_migrations

    conn = None
    try:
        # La barrière doit aligner les N process AVANT `connect()`, pas
        # seulement avant `run_migrations()` : c'est `connect()` (le passage
        # en WAL, via `_set_wal_mode_with_retry`) qui est la race concurrente
        # réelle qu'on veut forcer à chaque exécution. Avant ce correctif, le
        # bug du WAL n'apparaissait que grâce au hasard du démarrage en
        # `spawn` (retour de revue #59, round 5, point 6 d'OswinFreyr).
        start_barrier.wait(timeout=10)
        conn = connect()
        run_migrations(conn)
        result_queue.put(None)
    except Exception as exc:  # noqa: BLE001 — remonté au process parent via la queue, pas une trace
        result_queue.put(repr(exc))
    finally:
        if conn is not None:
            conn.close()


def test_healthz(client):
    response = client.get("/healthz")
    assert response.status_code == 200
    assert response.json() == {"status": "ok"}


def test_startup_fails_fast_on_malformed_max_batch_bytes(tmp_path, monkeypatch):
    # config.max_batch_bytes() lève RuntimeError sur une valeur malformée,
    # mais rien ne garantissait que cette levée remontait bien AU DÉMARRAGE
    # (via l'appel dans lifespan()) plutôt que d'être silencieusement
    # avalée quelque part entre lifespan() et TestClient — seul le
    # comportement de config.py était couvert, pas l'intégration avec le
    # démarrage de l'app (retour de revue #59, round 4, point d'OswinFreyr).
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "startup.db"))
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "2MB")

    from fastapi.testclient import TestClient

    from app.main import app

    with pytest.raises(RuntimeError, match="2MB"):
        with TestClient(app):
            pass


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


def test_ingest_rejects_a_huge_integer_literal(client):
    # Depuis Python 3.11 (limite `sys.int_max_str_digits` = 4300),
    # json.loads lève un ValueError générique — PAS un json.JSONDecodeError
    # — sur un littéral entier de plus de 4300 chiffres. Un `except
    # (UnicodeDecodeError, json.JSONDecodeError, RecursionError)` laissait
    # ce cas remonter en 500 (retour de revue #59, round 7, point
    # d'OswinFreyr). Corps volontairement petit (~5 Ko), bien en dessous de
    # la limite de taille — ce n'est pas un garde-fou mémoire qui doit
    # intervenir ici, mais la gestion d'erreur JSON.
    body = ('{"events": [' + "1" * 5000 + "]}").encode()
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


def test_drain_stops_at_the_cap_instead_of_consuming_everything(client, monkeypatch):
    # httpx (TestClient) livre toujours le corps en un seul message ASGI
    # (`more_body=False` dès le départ), y compris via un générateur chunked
    # — voir le commentaire de `_read_limited_body` sur `_stream_consumed`.
    # Aucun test ci-dessus n'exerçait donc réellement `_drain_bounded` ni la
    # continuation dans la même boucle `async for` (retour de revue #59,
    # round 6, point 3 d'OswinFreyr, resté sans suite depuis le round 5).
    # Ce test pilote l'ASGI directement, avec un `receive` qui renvoie le
    # corps en petits morceaux (`more_body=True` entre chacun), bien plus
    # loin que `limit + _DRAIN_CAP_BYTES`, et vérifie que la lecture
    # s'arrête effectivement à la borne plutôt que de tout consommer.
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "16")

    import anyio

    from app.main import _DRAIN_CAP_BYTES, app

    chunk = b"x" * 1024
    limit = 16
    # Assez de morceaux pour dépasser largement limit + _DRAIN_CAP_BYTES si
    # le drain n'était pas borné.
    n_chunks = (limit + _DRAIN_CAP_BYTES) // len(chunk) + 10
    produced = 0

    async def receive():
        nonlocal produced
        if produced >= n_chunks:
            # Ne devrait jamais être atteint si le drain est bien borné.
            return {"type": "http.request", "body": b"", "more_body": False}
        produced += 1
        return {"type": "http.request", "body": chunk, "more_body": produced < n_chunks}

    messages: list[dict] = []

    async def send(message):
        messages.append(message)

    scope = {
        "type": "http",
        "asgi": {"version": "3.0"},
        "http_version": "1.1",
        "method": "POST",
        "path": "/ingest/batch",
        "raw_path": b"/ingest/batch",
        "query_string": b"",
        "headers": [(b"content-type", b"application/json")],
        "client": ("test", 0),
        "server": ("test", 80),
    }

    anyio.run(app, scope, receive, send)

    status = next(m["status"] for m in messages if m["type"] == "http.response.start")
    assert status == 413
    assert produced < n_chunks, (
        "le drain a consommé tout le flux produit au lieu de s'arrêter à la borne"
    )


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
    # Course entre l'arrêt du lifespan (ferme app.state.db dans son
    # `finally`) et une écriture encore en cours sur le threadpool : seul
    # sqlite3.OperationalError était rattrapé, pas sqlite3.ProgrammingError
    # ("Cannot operate on a closed database"), qui remontait un 500 brut au
    # lieu du 503 attendu (retour de revue #59, round 2). Reproduit ici sans
    # vraie course : fermer la connexion partagée avant le POST suffit à
    # produire la même ProgrammingError.
    client.app.state.db.close()
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


def test_migrations_are_safe_across_processes(tmp_path, monkeypatch):
    # La docstring de db.py affirme que run_migrations() est sûr sous
    # `uvicorn --workers N` (plusieurs PROCESS, pas juste plusieurs threads
    # dans le même process) : jusqu'ici, seul `test_concurrent_writes_are_not_
    # lost` couvrait la concurrence, en multi-thread. Ce test lance de vrais
    # process OS séparés pour vérifier l'affirmation elle-même (retour de
    # revue #59, round 4, point d'OswinFreyr).
    import multiprocessing

    db_path = str(tmp_path / "multiproc.db")
    ctx = multiprocessing.get_context("spawn")
    n_workers = 5
    start_barrier = ctx.Barrier(n_workers)
    result_queue = ctx.Queue()

    worker_args = (db_path, start_barrier, result_queue)
    processes = [
        ctx.Process(target=_apply_migrations_in_subprocess, args=worker_args)
        for _ in range(n_workers)
    ]
    for p in processes:
        p.start()
    for p in processes:
        p.join(timeout=30)

    if not all(not p.is_alive() for p in processes):
        for p in processes:
            if p.is_alive():
                p.terminate()
        pytest.fail("un process n'a pas terminé à temps")

    # `.get(timeout=...)` plutôt que `.get_nowait()` : un worker qui a mis du
    # temps à pousser son résultat (le `put()` suit `join()` dans le temps,
    # pas garanti instantané) ferait échouer `get_nowait()` sur un
    # `queue.Empty` opaque plutôt que sur l'assertion lisible ci-dessous
    # (retour de revue #59, round 5, point 6 d'OswinFreyr).
    results = [result_queue.get(timeout=5) for _ in range(n_workers)]
    assert results == [None] * n_workers, (
        f"un des {n_workers} process a échoué au lieu d'attendre sous busy_timeout : {results}"
    )

    monkeypatch.setenv("DENGON_DASHBOARD_DB", db_path)
    from app.db import connect

    conn = connect()
    versions = [r["version"] for r in conn.execute("SELECT version FROM schema_migrations")]
    conn.close()
    assert versions == [1], "la migration ne doit être enregistrée qu'une seule fois, pas dupliquée"


@pytest.mark.parametrize("payload", [{"events": []}, [], {"a": 1}])
def test_ingest_event_count_shapes(client, payload):
    response = client.post("/ingest/batch", json=payload)
    assert response.status_code == 202
