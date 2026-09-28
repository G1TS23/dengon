import sys
from pathlib import Path

import pytest

# Permet `import app.*` même sans installation éditable du paquet.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))


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
