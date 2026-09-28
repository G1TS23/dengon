import base64
import hashlib
import json
import sqlite3

import pytest
from nacl.signing import SigningKey


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


# --- Aides de test : construire un batch signé valide (US-216) -------------
#
# Miroir minimal de contracts/tools/catalogue.py::canonical_json/event_id —
# voir app/canonical.py pour la même duplication volontaire côté app.


def _canonical_json(obj: object) -> bytes:
    return json.dumps(
        obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    ).encode("utf-8")


def _event_id(node_id: str, seq: int) -> str:
    return hashlib.sha256(node_id.encode("ascii") + seq.to_bytes(8, "big")).hexdigest()


def _register_node(client, node_id: str = "relay-3f2a9c", kind: str = "relay"):
    """Enregistre un nœud avec une clé Ed25519 fraîche, renvoie
    `(signing_key, token)`.
    """
    signing_key = SigningKey.generate()
    response = client.post(
        "/api/nodes",
        json={
            "node_id": node_id,
            "kind": kind,
            "pub_sign": signing_key.verify_key.encode().hex(),
        },
    )
    assert response.status_code == 201, response.text
    return signing_key, response.json()["token"]


def _build_signed_batch(
    node_id: str,
    signing_key: SigningKey,
    *,
    events: list[tuple[int, str, dict]] | None = None,
    ts_ms: int = 1_725_800_000_000,
    tamper_event_id: bool = False,
    spoof_event_node_id: str | None = None,
    spoof_event_node_kind: str | None = None,
) -> dict:
    """Construit un batch valide (schéma + `event_id` cohérents), signé avec
    `signing_key`. `tamper_event_id=True` force un `event_id` incohérent sur
    le premier événement — la signature reste valide (calculée APRÈS la
    falsification, comme le ferait un nœud buggé), pour tester le contrôle
    de cohérence indépendamment de la vérification de signature.
    `spoof_event_node_id`/`spoof_event_node_kind` remplacent le `node_id`/
    `node_kind` du PREMIER événement seulement — `event_id` reste calculé
    avec le `node_id` du BATCH (comme le ferait `_verify_event_ids` côté
    serveur), pour reproduire un événement qui prétend appartenir à un autre
    nœud (revue PR #91, point bloquant).
    """
    if events is None:
        events = [
            (1, "msg.queued", {"msg_log_id": "1122334455667788", "conv_hash": "0011223344556677"})
        ]

    node_kind = "relay" if node_id.startswith("relay-") else "client"
    built_events = []
    for seq, name, payload in events:
        eid = _event_id(node_id, seq)
        if tamper_event_id and seq == events[0][0]:
            eid = "0" * 64
        event_node_id = node_id
        event_node_kind = node_kind
        if seq == events[0][0]:
            if spoof_event_node_id is not None:
                event_node_id = spoof_event_node_id
            if spoof_event_node_kind is not None:
                event_node_kind = spoof_event_node_kind
        built_events.append(
            {
                "event_id": eid,
                "node_id": event_node_id,
                "node_kind": event_node_kind,
                "seq": seq,
                "ts_ms": ts_ms,
                "name": name,
                "payload": payload,
            }
        )

    batch_id = hashlib.sha256(_canonical_json(built_events)).hexdigest()
    unsigned = {
        "batch_id": batch_id,
        "node_id": node_id,
        "schema_version": 1,
        "events": built_events,
    }
    signature = signing_key.sign(_canonical_json(unsigned)).signature
    return {**unsigned, "sig": base64.b64encode(signature).decode("ascii")}


def _auth(token: str) -> dict:
    return {"Authorization": f"Bearer {token}"}


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
    monkeypatch.setenv("DENGON_DASHBOARD_JWT_SECRET", "peu-importe-mais-assez-long-pour-le-warning")
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "2MB")

    from fastapi.testclient import TestClient

    from app.main import app

    with pytest.raises(RuntimeError, match="2MB"):
        with TestClient(app):
            pass


def test_startup_fails_fast_when_jwt_secret_is_missing(tmp_path, monkeypatch):
    # Même discipline que max_batch_bytes() ci-dessus, pour le secret JWT
    # (US-216) : pas de valeur par défaut, doit échouer au démarrage plutôt
    # qu'à la première requête.
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "startup2.db"))
    monkeypatch.delenv("DENGON_DASHBOARD_JWT_SECRET", raising=False)

    from fastapi.testclient import TestClient

    from app.main import app

    with pytest.raises(RuntimeError, match="DENGON_DASHBOARD_JWT_SECRET"):
        with TestClient(app):
            pass


def test_startup_fails_fast_when_jwt_secret_is_too_short(tmp_path, monkeypatch):
    # PyJWT lève InsecureKeyLengthWarning en dessous de 32 octets pour HS256
    # — vérifié au démarrage plutôt que laissé comme un avertissement
    # ignorable à chaque requête (revue PR #91, point mineur).
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "startup3.db"))
    monkeypatch.setenv("DENGON_DASHBOARD_JWT_SECRET", "trop-court")

    from fastapi.testclient import TestClient

    from app.main import app

    with pytest.raises(RuntimeError, match="DENGON_DASHBOARD_JWT_SECRET"):
        with TestClient(app):
            pass


# --- POST /api/nodes ---------------------------------------------------


def test_register_node_returns_a_usable_token(client):
    signing_key, token = _register_node(client)
    assert token
    batch = _build_signed_batch("relay-3f2a9c", signing_key)
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 202, response.text


@pytest.mark.parametrize(
    "body",
    [
        {"kind": "relay", "pub_sign": "aa" * 32},  # node_id manquant
        {"node_id": "relay-3f2a9c", "pub_sign": "aa" * 32},  # kind manquant
        {"node_id": "relay-3f2a9c", "kind": "modem", "pub_sign": "aa" * 32},  # kind invalide
        {"node_id": "relay-3f2a9c", "kind": "relay", "pub_sign": "pas-de-hex-ici!!"},
        {"node_id": "relay-3f2a9c", "kind": "relay", "pub_sign": "aa" * 10},  # mauvaise longueur
    ],
)
def test_register_node_rejects_malformed_bodies(client, body):
    response = client.post("/api/nodes", json=body)
    assert response.status_code == 400


def test_register_node_rejects_re_registration_of_an_existing_node_id(client):
    # L'upsert précédent (`ON CONFLICT(node_id) DO UPDATE`) remplaçait la clé
    # publique d'un nœud déjà enregistré et le re-whitelistait — sans auth
    # opérateur sur cette route, n'importe qui pouvait ainsi prendre le
    # contrôle d'un node_id existant, y compris un nœud qu'un opérateur
    # aurait retiré (revue PR #91, point important).
    _register_node(client, node_id="relay-3f2a9c")
    imposter_key = SigningKey.generate()
    response = client.post(
        "/api/nodes",
        json={
            "node_id": "relay-3f2a9c",
            "kind": "relay",
            "pub_sign": imposter_key.verify_key.encode().hex(),
        },
    )
    assert response.status_code == 409


# --- POST /ingest/batch — authentification et validation ---------------


def test_ingest_rejects_missing_authorization(client):
    batch = _build_signed_batch("relay-3f2a9c", SigningKey.generate())
    response = client.post("/ingest/batch", json=batch)
    assert response.status_code == 401


def test_ingest_rejects_malformed_authorization_header(client):
    batch = _build_signed_batch("relay-3f2a9c", SigningKey.generate())
    response = client.post(
        "/ingest/batch", json=batch, headers={"Authorization": "PasDuBearer abc"}
    )
    assert response.status_code == 401


def test_ingest_rejects_expired_or_forged_jwt(client):
    # Un JWT signé avec un AUTRE secret (donc jamais émis par ce serveur)
    # doit être rejeté — vérifie que la vérification de signature JWT
    # fonctionne, pas seulement le parsing.
    import jwt as pyjwt

    forged = pyjwt.encode({"node_id": "relay-3f2a9c"}, "un-autre-secret", algorithm="HS256")
    batch = _build_signed_batch("relay-3f2a9c", SigningKey.generate())
    response = client.post("/ingest/batch", json=batch, headers=_auth(forged))
    assert response.status_code == 401


def test_ingest_rejects_unregistered_node(client):
    # Jeton syntaxiquement valide (vrai secret du serveur) mais pour un
    # node_id jamais enregistré via /api/nodes — doit être rejeté comme
    # « nœud inconnu ou non whitelisté », pas planter.
    from app.auth import create_token

    token = create_token("relay-9a9a9a")
    batch = _build_signed_batch("relay-9a9a9a", SigningKey.generate())
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 401


def test_ingest_rejects_node_id_mismatch_between_jwt_and_batch(client):
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    # Le jeton est pour relay-3f2a9c, le batch prétend venir d'un autre nœud.
    batch = _build_signed_batch("relay-aaaaaa", signing_key)
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 401


def test_ingest_rejects_forged_signature(client):
    # Nœud enregistré avec une vraie clé, mais le batch est signé par une
    # AUTRE clé (jamais enregistrée) — la signature ne correspond pas à
    # `pub_sign` du nœud whitelisté.
    _, token = _register_node(client, node_id="relay-3f2a9c")
    imposter_key = SigningKey.generate()
    batch = _build_signed_batch("relay-3f2a9c", imposter_key)
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 401

    from app.db import connect

    conn = connect()
    count = conn.execute("SELECT COUNT(*) AS n FROM events").fetchone()["n"]
    conn.close()
    assert count == 0, "une signature invalide ne doit stocker aucun événement"


def test_ingest_rejects_batch_failing_schema_validation(client):
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch("relay-3f2a9c", signing_key)
    del batch["schema_version"]  # requis par batch.schema.json
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 400


def test_ingest_rejects_non_json_body(client):
    _, token = _register_node(client)
    response = client.post(
        "/ingest/batch",
        content=b"ceci n'est pas du json",
        headers={**_auth(token), "content-type": "application/json"},
    )
    assert response.status_code == 400


def test_ingest_rejects_a_huge_integer_literal(client):
    # Depuis Python 3.11 (limite `sys.int_max_str_digits` = 4300),
    # json.loads lève un ValueError générique — PAS un json.JSONDecodeError
    # — sur un littéral entier de plus de 4300 chiffres (même piège que le
    # round 7 de la revue #59, transposé dans app/ingest.py::_parse_json).
    _, token = _register_node(client)
    body = ('{"events": [' + "1" * 5000 + "]}").encode()
    response = client.post(
        "/ingest/batch",
        content=body,
        headers={**_auth(token), "content-type": "application/json"},
    )
    assert response.status_code == 400


def test_ingest_rejects_tampered_event_id(client):
    # Batch structurellement valide et correctement signé, mais dont
    # l'event_id d'un événement ne correspond pas à
    # hex(SHA-256(node_id ‖ seq)) — doit être détecté indépendamment de la
    # signature (qui, elle, est valide : signée après la falsification).
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch("relay-3f2a9c", signing_key, tamper_event_id=True)
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 400


def test_ingest_rejects_event_node_id_different_from_batch_node_id(client):
    # `_verify_event_ids` recalculait l'event_id avec le node_id du BATCH,
    # mais `_insert_events` stockait celui de l'ÉVÉNEMENT sans jamais
    # vérifier qu'ils correspondent — un nœud whitelisté pouvait donc signer
    # un batch valide tout en attribuant ses événements à un AUTRE node_id
    # enregistré, ce qui annule la garantie « signé par le nœud émetteur »
    # (revue PR #91, point bloquant).
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch(
        "relay-3f2a9c", signing_key, spoof_event_node_id="client-9c1d84"
    )
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 400

    from app.db import connect

    conn = connect()
    count = conn.execute("SELECT COUNT(*) AS n FROM events").fetchone()["n"]
    conn.close()
    assert count == 0, "un node_id d'événement usurpé ne doit stocker aucun événement"


def test_ingest_rejects_event_node_kind_different_from_registered_kind(client):
    # Même défaut que ci-dessus côté `node_kind` : rien ne vérifiait qu'il
    # correspond au `kind` enregistré pour ce nœud dans `nodes`.
    signing_key, token = _register_node(client, node_id="relay-3f2a9c", kind="relay")
    batch = _build_signed_batch(
        "relay-3f2a9c", signing_key, spoof_event_node_kind="client"
    )
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 400


# --- POST /ingest/batch — cas nominal et idempotence --------------------


def test_ingest_accepts_a_valid_signed_batch_and_stores_it(client):
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch("relay-3f2a9c", signing_key)
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 202
    body = response.json()
    assert body["stored"] is True
    assert body["batch_id"] == batch["batch_id"]
    assert body["event_count"] == 1
    assert body["new_event_count"] == 1

    from app.db import connect

    conn = connect()
    row = conn.execute(
        "SELECT node_id, name, seq, integrity, payload FROM events WHERE event_id = ?",
        (batch["events"][0]["event_id"],),
    ).fetchone()
    conn.close()
    assert row is not None
    assert row["node_id"] == "relay-3f2a9c"
    assert row["name"] == "msg.queued"
    assert row["seq"] == 1
    assert row["integrity"] == "unverified"  # vérif de journal chaîné hors périmètre US-216
    assert json.loads(row["payload"]) == batch["events"][0]["payload"]


def test_ingest_is_idempotent_on_exact_replay(client):
    # Critère d'acceptation US-216 : rejouer deux fois le même batch ne crée
    # aucun doublon (déduplication sur event_id).
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch("relay-3f2a9c", signing_key)

    first = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert first.status_code == 202
    assert first.json()["new_event_count"] == 1

    second = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert second.status_code == 202
    assert second.json()["new_event_count"] == 0, "le rejeu ne doit créer aucune nouvelle ligne"

    from app.db import connect

    conn = connect()
    count = conn.execute(
        "SELECT COUNT(*) AS n FROM events WHERE event_id = ?", (batch["events"][0]["event_id"],)
    ).fetchone()["n"]
    conn.close()
    assert count == 1, "un seul exemplaire de l'événement en base, pas un par rejeu"


def test_ingest_accepts_multiple_events_in_one_batch(client):
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch(
        "relay-3f2a9c",
        signing_key,
        events=[
            (1, "msg.queued", {"msg_log_id": "1122334455667788", "conv_hash": "0011223344556677"}),
            (2, "peer.connected", {"peer": "a1b2c3d4e5f60718", "rssi": -58, "role": "peripheral"}),
        ],
    )
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 202
    assert response.json()["event_count"] == 2
    assert response.json()["new_event_count"] == 2


# --- Garde-fous de taille du corps (retours de revue #59, repris avec auth) --


def test_ingest_rejects_body_over_max_size(client, monkeypatch):
    # Garde-fou mémoire : un corps plus grand que la limite configurée est
    # rejeté sans être stocké (retour de revue #59, point 1). Auth vérifiée
    # AVANT la taille (le jeton est donc valide ici, pour exercer vraiment
    # le chemin de lecture bornée du corps, pas juste le rejet d'auth).
    _, token = _register_node(client)
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "16")
    response = client.post(
        "/ingest/batch",
        content=b'{"events": [1, 2, 3, 4, 5, 6, 7, 8, 9]}',  # > 16 octets
        headers={**_auth(token), "content-type": "application/json"},
    )
    assert response.status_code == 413

    from app.db import connect

    conn = connect()
    count = conn.execute("SELECT COUNT(*) AS n FROM events").fetchone()["n"]
    conn.close()
    assert count == 0, "un corps rejeté pour taille ne doit laisser aucune ligne"


def test_ingest_skips_drain_when_client_expects_100_continue(client, monkeypatch):
    # Avec `Expect: 100-continue`, lire `request.stream()` ici déclencherait
    # l'envoi de `100 Continue` par uvicorn, invitant le client à téléverser
    # un corps qu'on s'apprête à rejeter — bande passante perdue (retour de
    # revue #59, round 8, point 1 d'OswinFreyr). Vérifié en espionnant
    # `_drain_bounded` : appelé sans l'en-tête, jamais avec.
    import app.main as main_module

    _, token = _register_node(client)
    calls: list[bool] = []
    original = main_module._drain_bounded

    async def spy(stream):
        calls.append(True)
        return await original(stream)

    monkeypatch.setattr(main_module, "_drain_bounded", spy)
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "16")

    response = client.post(
        "/ingest/batch",
        content=b'{"events": [1, 2, 3, 4, 5, 6, 7, 8, 9]}',
        headers={**_auth(token), "content-type": "application/json", "expect": "100-continue"},
    )
    assert response.status_code == 413
    assert calls == [], "le drain ne doit pas être appelé quand le client attend 100 Continue"

    response = client.post(
        "/ingest/batch",
        content=b'{"events": [1, 2, 3, 4, 5, 6, 7, 8, 9]}',
        headers={**_auth(token), "content-type": "application/json"},
    )
    assert response.status_code == 413
    assert calls == [True], "le drain doit être appelé normalement sans l'en-tête Expect"


def test_ingest_rejects_chunked_body_over_max_size(client, monkeypatch):
    # Cas malveillant réel : Content-Length absent (chunked) ou mensonger, la
    # seule protection est alors le comptage dans `async for morceau in
    # request.stream()`. Un contenu `bytes` chez httpx pose toujours
    # Content-Length ; passer un générateur force l'encodage chunked, donc
    # exerce vraiment cette boucle (retour de revue #59, round 2, point de
    # Paul — les deux tests de taille précédents ne passaient que par le
    # fast-path Content-Length).
    _, token = _register_node(client)
    monkeypatch.setenv("DENGON_DASHBOARD_MAX_BATCH_BYTES", "16")

    def _morceaux():
        yield b'{"events": ['
        yield b"1, 2, 3, 4, 5, 6, 7, 8, 9"
        yield b"]}"

    response = client.post(
        "/ingest/batch",
        content=_morceaux(),
        headers={**_auth(token), "content-type": "application/json"},
    )
    assert response.status_code == 413

    from app.db import connect

    conn = connect()
    count = conn.execute("SELECT COUNT(*) AS n FROM events").fetchone()["n"]
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
    _, token = _register_node(client)
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
        "headers": [
            (b"content-type", b"application/json"),
            (b"authorization", f"Bearer {token}".encode()),
        ],
        "client": ("test", 0),
        "server": ("test", 80),
    }

    anyio.run(app, scope, receive, send)

    status = next(m["status"] for m in messages if m["type"] == "http.response.start")
    assert status == 413
    assert produced < n_chunks, (
        "le drain a consommé tout le flux produit au lieu de s'arrêter à la borne"
    )


# --- Connexion partagée, concurrence, erreurs de stockage ----------------


def test_a_single_connection_is_reused_for_all_writes(tmp_path, monkeypatch):
    # La connexion ouverte au démarrage doit servir à toutes les écritures :
    # pas de connect() par requête (retour de revue #59, point 3).
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "reuse.db"))
    monkeypatch.setenv("DENGON_DASHBOARD_JWT_SECRET", "test-secret-assez-long-pour-le-warning")
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
        signing_key, token = _register_node(c, node_id="relay-3f2a9c")
        for i in range(5):
            payload = {"msg_log_id": "1122334455667788", "conv_hash": "0011223344556677"}
            batch = _build_signed_batch(
                "relay-3f2a9c", signing_key, events=[(i, "msg.queued", payload)]
            )
            response = c.post("/ingest/batch", json=batch, headers=_auth(token))
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
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch("relay-3f2a9c", signing_key)
    client.app.state.db.close()
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 503
    assert response.headers.get("Retry-After") == "1"


def test_concurrent_writes_are_not_lost(client):
    # La connexion partagée + le verrou doivent tenir sous des écritures
    # concurrentes issues du threadpool (retour de revue #59, points 2 et 3).
    import concurrent.futures

    signing_key, token = _register_node(client, node_id="relay-3f2a9c")

    def _post(i: int):
        payload = {"msg_log_id": "1122334455667788", "conv_hash": "0011223344556677"}
        batch = _build_signed_batch(
            "relay-3f2a9c",
            signing_key,
            events=[(i, "msg.queued", payload)],
        )
        return client.post("/ingest/batch", json=batch, headers=_auth(token))

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        reponses = list(pool.map(_post, range(20)))

    assert all(r.status_code == 202 for r in reponses)

    # La vraie preuve qu'aucune écriture n'a été perdue : 20 lignes en base,
    # pas seulement 20 réponses 202 (retour de revue #59, round 2, point de
    # Paul — la réponse HTTP seule ne dit rien sur ce qui a vraiment été
    # committé sous concurrence).
    from app.db import connect

    conn = connect()
    stored = conn.execute(
        "SELECT COUNT(*) AS n FROM events WHERE node_id = ?", ("relay-3f2a9c",)
    ).fetchone()["n"]
    conn.close()
    assert stored == 20, "20 requêtes acceptées doivent laisser 20 lignes, pas moins"


def test_ingest_returns_503_when_storage_is_locked(client, monkeypatch):
    # Le chemin d'erreur « base verrouillée » : un flush concurrent perd la
    # course et le nœud doit retenter (retour de revue #59, point 3).
    def _boom(*_args, **_kwargs):
        raise sqlite3.OperationalError("database is locked")

    monkeypatch.setattr("app.ingest._insert_events", _boom)
    signing_key, token = _register_node(client, node_id="relay-3f2a9c")
    batch = _build_signed_batch("relay-3f2a9c", signing_key)
    response = client.post("/ingest/batch", json=batch, headers=_auth(token))
    assert response.status_code == 503
    assert response.headers.get("Retry-After") == "1"


# --- Migrations -----------------------------------------------------------


def test_migrations_applied_once(client):
    from app.db import connect, run_migrations

    conn = connect()
    # Le lifespan les a déjà appliquées : un second passage ne fait rien.
    assert run_migrations(conn) == []
    versions = [r["version"] for r in conn.execute("SELECT version FROM schema_migrations")]
    conn.close()
    assert versions == [1, 2, 3]


def test_migrations_idempotent_after_partial_apply(tmp_path, monkeypatch):
    # Simule un arrêt entre le DDL et l'INSERT dans schema_migrations :
    # la reprise ne doit pas planter (retour de revue #59, point 4).
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "partial.db"))
    from app.db import connect, run_migrations
    from app.migrations import MIGRATIONS

    conn = connect()
    # DDL de la migration 1 appliqué sans enregistrer sa version.
    for statement in MIGRATIONS[0][2]:
        conn.execute(statement)

    assert run_migrations(conn) == [1, 2, 3]  # se termine proprement grâce à IF NOT EXISTS
    versions = [r["version"] for r in conn.execute("SELECT version FROM schema_migrations")]
    assert versions == [1, 2, 3]
    conn.close()


def test_migration_error_survives_a_transaction_sqlite_already_closed(tmp_path, monkeypatch):
    # SQLite annule lui-même la transaction sur certaines erreurs
    # (SQLITE_FULL/IOERR/NOMEM, difficiles à déclencher de façon portable en
    # test) : simulé ici avec une migration dont une instruction termine
    # explicitement la transaction (COMMIT) avant qu'une instruction
    # invalide ne lève. Sans la garde `in_transaction`, le `ROLLBACK`
    # explicite du bloc `except` lèverait à son tour ("cannot rollback - no
    # transaction is active"), masquant l'erreur d'origine — retour de revue
    # #59, round 8, point 2 d'OswinFreyr.
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "rollback.db"))
    from app import db as db_module

    monkeypatch.setattr(
        db_module,
        "MIGRATIONS",
        [(1, "cassee", ["COMMIT", "CECI N'EST PAS DU SQL"])],
    )

    conn = db_module.connect()
    with pytest.raises(sqlite3.OperationalError) as exc_info:
        db_module.run_migrations(conn)
    assert "cannot rollback" not in str(exc_info.value)
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
    assert versions == [1, 2, 3], "les migrations ne doivent être enregistrées qu'une seule fois"
