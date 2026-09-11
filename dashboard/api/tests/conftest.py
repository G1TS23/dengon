import sys
from pathlib import Path

import pytest

# Permet `import app.*` même sans installation éditable du paquet.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))


@pytest.fixture
def client(tmp_path, monkeypatch):
    """Client de test sur une base SQLite jetable, migrée par le lifespan."""
    monkeypatch.setenv("DENGON_DASHBOARD_DB", str(tmp_path / "test.db"))

    from fastapi.testclient import TestClient

    from app.main import app

    with TestClient(app) as test_client:  # __enter__ déclenche le lifespan → migrations
        yield test_client
