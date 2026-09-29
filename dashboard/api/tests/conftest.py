import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

# Permet `import app.*` même sans installation éditable du paquet.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

# dashboard/api/tests/conftest.py -> ... -> racine du dépôt (workspace cargo).
_REPO_ROOT = Path(__file__).resolve().parents[3]

FIXTURES_DIR = _REPO_ROOT / "contracts" / "events" / "fixtures"

# Une seule paire de clés Ed25519 pour les 4 node_id des fixtures golden
# (`contracts/events/test-signing-key.json`) — partagée par tous les tests
# qui ingèrent ces fixtures via le vrai pipeline HTTP.
_TEST_SIGNING_PUB_HEX = "c561fa9f643fe5c60113cce9db282fde2b9e5ca5fc6b6fc0d1679bb339c9f72f"


@pytest.fixture(scope="session")
def dengon_verify_bin() -> str:
    """Construit `dengon-verify` une fois par session (US-310) et
    positionne `DENGON_VERIFY_BIN` (`app/config.py`) dessus, pour que
    `tests/test_integrity.py` tourne contre le VRAI binaire — la règle de
    vérification vient de `dengon_core::ledger`, jamais réécrite côté test.

    `DENGON_VERIFY_BIN` déjà positionné (CI, VPS) et résoluble : réutilisé
    tel quel, pas de rebuild. `cargo` absent (environnement sans toolchain
    Rust) : les tests qui en dépendent sont sautés plutôt qu'en échec — un
    outil manquant n'est pas la même chose qu'une régression.
    """
    existing = os.environ.get("DENGON_VERIFY_BIN")
    if existing and shutil.which(existing):
        return existing
    try:
        subprocess.run(
            ["cargo", "build", "-p", "dengon-verify"],
            cwd=_REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        pytest.skip("cargo indisponible : impossible de construire dengon-verify pour les tests")
    except subprocess.CalledProcessError as exc:
        pytest.fail(f"cargo build -p dengon-verify a échoué :\n{exc.stderr}")
    binaire = _REPO_ROOT / "target" / "debug" / "dengon-verify"
    if not binaire.exists():
        pytest.fail(f"dengon-verify introuvable après build : {binaire}")
    os.environ["DENGON_VERIFY_BIN"] = str(binaire)
    return str(binaire)


@pytest.fixture
def client(tmp_path, monkeypatch):
    """Client de test sur une base SQLite jetable, migrée par le lifespan."""
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "test.db"))
    # Secret JWT de test — jamais utilisé hors tests, voir config.py::jwt_secret
    # (pas de valeur par défaut en production, volontairement).
    monkeypatch.setenv("DENGON_DASHBOARD_JWT_SECRET", "test-secret-ne-jamais-utiliser-en-prod")

    from fastapi.testclient import TestClient

    from app.main import app

    with TestClient(app) as test_client:  # __enter__ déclenche le lifespan → migrations
        yield test_client


def _load_fixture_batches() -> list[dict]:
    return [json.loads(path.read_text()) for path in sorted(FIXTURES_DIR.glob("*.json"))]


def _register_and_authorize(client, node_id: str, kind: str) -> dict[str, str]:
    response = client.post(
        "/api/nodes",
        json={"node_id": node_id, "kind": kind, "pub_sign": _TEST_SIGNING_PUB_HEX},
    )
    assert response.status_code == 201, response.text
    return {"Authorization": f"Bearer {response.json()['token']}"}


def _ingest_fixture_batches(client, batches: list[dict]) -> None:
    node_ids = {
        (b["node_id"], "relay" if b["node_id"].startswith("relay-") else "client") for b in batches
    }
    headers_by_node = {
        node_id: _register_and_authorize(client, node_id, kind) for node_id, kind in node_ids
    }
    for batch in batches:
        response = client.post(
            "/ingest/batch", json=batch, headers=headers_by_node[batch["node_id"]]
        )
        assert response.status_code == 202, response.text
