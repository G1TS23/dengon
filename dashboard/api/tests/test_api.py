import json


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


def test_migrations_applied_once(client):
    from app.db import connect, run_migrations

    conn = connect()
    # Le lifespan les a déjà appliquées : un second passage ne fait rien.
    assert run_migrations(conn) == []
    versions = [r["version"] for r in conn.execute("SELECT version FROM schema_migrations")]
    conn.close()
    assert versions == [1]
