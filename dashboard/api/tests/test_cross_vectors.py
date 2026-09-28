"""Les 20 fixtures golden de `contracts/events/` rejouées dans l'API — patte
dashboard du job CI `cross-vectors` (US-222).

Les mêmes fichiers sont relus ailleurs par deux implémentations écrites
séparément :

* `crates/dengon-core/tests/event_fixtures.rs` — Rust : recalcul d'`event_id`,
  de `batch_id`, JSON canonique octet à octet, vérification de la signature ;
* `contracts/tools/validate.py` — Python, côté contrat.

Ce fichier-ci vérifie la seule chose que les deux autres ne peuvent pas : que
le **pipeline d'ingestion réel** (schéma → JWT → signature Ed25519 →
`event_id` → déduplication → projections) accepte exactement ce que les
autres composants produisent. Un `POST /ingest/batch` qui refuserait une
fixture voudrait dire que le dashboard et le core ne lisent pas le contrat de
la même façon — c'est précisément ce que l'US-222 cherche à rendre
impossible sans que la CI rougisse.

Les fixtures sont signées par `contracts/events/test-signing-key.json` : on
enregistre donc chaque nœud avec CETTE clé publique, plutôt qu'une clé
fraîche comme dans `test_api.py`.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from nacl.signing import SigningKey

_CONTRACTS_EVENTS = Path(__file__).resolve().parents[3] / "contracts" / "events"
_FIXTURES = sorted((_CONTRACTS_EVENTS / "fixtures").glob("*.json"))


def _cle_de_test() -> SigningKey:
    seed = json.loads((_CONTRACTS_EVENTS / "test-signing-key.json").read_text())["seed_hex"]
    return SigningKey(bytes.fromhex(seed))


def _charger(path: Path) -> dict:
    return json.loads(path.read_text())


def _enregistrer_noeuds(client, batches: list[dict]) -> dict[str, str]:
    """Enregistre chaque `node_id` rencontré avec la clé publique de test.

    Renvoie `{node_id: token}`. Le `kind` vient du premier événement du
    batch : il n'y a pas de `node_kind` au niveau du batch.
    """
    tokens: dict[str, str] = {}
    pub_sign = _cle_de_test().verify_key.encode().hex()
    for body in batches:
        node_id = body["node_id"]
        if node_id in tokens:
            continue
        reponse = client.post(
            "/api/nodes",
            json={"node_id": node_id, "kind": body["events"][0]["node_kind"], "pub_sign": pub_sign},
        )
        assert reponse.status_code == 201, reponse.text
        tokens[node_id] = reponse.json()["token"]
    return tokens


def test_il_y_a_bien_vingt_fixtures():
    # Garde-fou : un glob qui ne trouve rien rendrait tous les tests
    # paramétrés de ce fichier verts sans rien exécuter.
    assert len(_FIXTURES) >= 20, f"20 fixtures golden attendues, {len(_FIXTURES)} trouvées"


@pytest.mark.parametrize("fixture", _FIXTURES, ids=lambda p: p.stem)
def test_chaque_fixture_golden_est_acceptee_telle_quelle(client, fixture):
    body = _charger(fixture)
    tokens = _enregistrer_noeuds(client, [body])

    reponse = client.post(
        "/ingest/batch",
        content=json.dumps(body).encode(),
        headers={
            "Authorization": f"Bearer {tokens[body['node_id']]}",
            "Content-Type": "application/json",
        },
    )
    assert reponse.status_code == 202, reponse.text
    corps = reponse.json()
    assert corps["batch_id"] == body["batch_id"], "batch_id recalculé par l'API != celui du contrat"
    assert corps["event_count"] == len(body["events"])
    assert corps["new_event_count"] == len(body["events"]), "tous les événements sont neufs"


def test_les_vingt_fixtures_passent_puis_sont_idempotentes(client):
    """Le corpus complet en une seule base, puis rejoué à l'identique.

    Deux propriétés d'un coup : les 20 batches cohabitent (pas de collision
    d'`event_id` entre fixtures), et un rejeu n'insère rien — l'idempotence
    demandée par la DoD « Dashboard API ».
    """
    batches = [_charger(p) for p in _FIXTURES]
    tokens = _enregistrer_noeuds(client, batches)

    total_evenements = 0
    for path, body in zip(_FIXTURES, batches, strict=True):
        reponse = client.post(
            "/ingest/batch",
            content=json.dumps(body).encode(),
            headers={"Authorization": f"Bearer {tokens[body['node_id']]}"},
        )
        assert reponse.status_code == 202, f"{path.name} : {reponse.text}"
        assert reponse.json()["new_event_count"] == len(body["events"]), path.name
        total_evenements += len(body["events"])

    for path, body in zip(_FIXTURES, batches, strict=True):
        reponse = client.post(
            "/ingest/batch",
            content=json.dumps(body).encode(),
            headers={"Authorization": f"Bearer {tokens[body['node_id']]}"},
        )
        assert reponse.status_code == 202, f"{path.name} (rejeu) : {reponse.text}"
        assert reponse.json()["new_event_count"] == 0, f"{path.name} : rejeu non idempotent"

    assert total_evenements >= 20


def test_une_fixture_alteree_est_refusee(client):
    """Sans ce test, rien ne prouve que les précédents détectent quoi que ce soit.

    Un seul champ change, la signature du contrat n'est pas retouchée : le
    pipeline doit refuser. Si ce test passait au vert avec un 202, c'est que
    la vérification de signature ne sert à rien.
    """
    body = _charger(_FIXTURES[0])
    tokens = _enregistrer_noeuds(client, [body])
    body["events"][0]["seq"] = 999_999

    reponse = client.post(
        "/ingest/batch",
        content=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {tokens[body['node_id']]}"},
    )
    assert reponse.status_code >= 400, "une fixture altérée a été acceptée"
