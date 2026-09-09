# Module : `contracts/events` — contrat des événements d'observabilité

**Rôle en une phrase :** figer, sous forme d'artefacts neutres en langage, la
tête d'un événement d'observabilité, le corps de `POST /ingest/batch`, et
20 exemples signés qui font référence pour tous les composants.
**Correspond à la conception :** [`docs/powl/08-observability-events.md`](../../powl/08-observability-events.md),
[`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md) §9.
**Dernière mise à jour :** 2026-09-09
**État :** fonctionnel — schémas + 20 fixtures + `validate.py` vert. **Contrat
à geler** au point d'équipe (US-107).

Cette fiche tient aussi lieu de **note d'onboarding de l'area `contract`**.

## À quoi ça sert

US-216 (validation d'ingestion), US-217 (projections dashboard) et US-208
(`dengon-core::observability`) doivent s'écrire **en parallèle sans se parler**.
Elles y arrivent parce qu'elles partagent trois choses figées ici :

1. la **forme** d'un événement et d'un batch (JSON Schema) ;
2. la **forme canonique signée** (`CANONICAL.md`) — l'octet-près que le Rust et
   le Python doivent tous deux produire ;
3. **20 fixtures** : des batches réels, signés, qu'on rejoue tels quels.

## Structure

```
contracts/
  README.md
  pyproject.toml  uv.lock          — outillage uv (jsonschema, pynacl, referencing)
  events/
    envelope.schema.json           — 1 événement (strict)
    batch.schema.json              — corps POST /ingest/batch (strict, $ref envelope)
    payloads.schema.json           — payload par nom d'événement — GÉNÉRÉ
    CANONICAL.md                   — forme canonique + signature Ed25519 (fait foi)
    test-signing-key.json          — clé Ed25519 de test (graine publique)
    fixtures/
      README.md                    — table : fichier → événements → ce que ça exerce
      01-pkt-seen.json … 20-client-summary.json
  tools/
    catalogue.py                   — CATALOGUE (source lisible) + canonical_json + event_id
    build_fixtures.py              — génère fixtures/ + payloads.schema.json
    validate.py                    — vérifie tout (schéma, payload, sig, redaction, couverture)
```

## Concepts / types importants

| Élément | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `CATALOGUE` | `tools/catalogue.py:34` | `nom → {required, props}` pour les 28 événements MVP ; source de `payloads.schema.json` |
| `canonical_json(x)` | `tools/catalogue.py` | `json.dumps(sort_keys, separators=(",",":"), ensure_ascii=False)` → bytes ; **le** contrat de signature |
| `event_id(node, seq)` | `tools/catalogue.py` | `hex(SHA-256(node ‖ seq_be64))` |
| `payloads_json_schema()` | `tools/catalogue.py` | dérive le JSON Schema `allOf` de `if name==X then …` |
| `batch(...)` | `tools/build_fixtures.py` | assemble un batch et le **signe** (Ed25519 sur `canonical_json` sans `sig`) |
| `check_fixture(...)` | `tools/validate.py` | schéma + redaction + signature + cohérence `event_id`/`node_id` + payload |

## Flux principal (exemple)

```
tools/build_fixtures.py
  charge test-signing-key.json (graine → SigningKey)
  pour chaque fixture : construit events[], calcule batch_id = SHA-256(canonical_json(events)),
    signe canonical_json(batch sans sig), écrit events/fixtures/NN-*.json
  génère events/payloads.schema.json depuis CATALOGUE
tools/validate.py  (ce que lance la CI)
  charge envelope+batch schema dans un referencing.Registry
  pour chaque fixture : valide vs batch.schema.json, marche l'arbre pour les clés interdites,
    vérifie la signature avec la clé publique de test, revalide event_id / node_id,
    valide chaque payload contre CATALOGUE
  vérifie que payloads.schema.json == payloads_json_schema()  (anti-dérive)
  vérifie que les 20 fixtures couvrent tout le CATALOGUE
```

## Dépendances

- **Internes :** aucune. C'est une *couture*, pas un composant runtime.
- **Externes :** `jsonschema` (validation), `pynacl` (Ed25519), `referencing`
  (résolution des `$ref`), `ruff` (dev). Gérées par `uv` (`contracts/uv.lock`).

## Décisions d'implémentation

- **`contracts/` top-level**, pas sous `docs/` ni sous un composant : neutre en
  langage, consommé par trois composants. Écart au layout `synthese/04` §5,
  consigné dans `03-ecarts-conception.md`.
- **Catalogue en Python, schéma dérivé** : une seule source à maintenir.
- **Fixtures committées, régénération vérifiée en CI** (`git diff --exit-code`) :
  un diff = une dérive visible.
- **`msg_log_id` = 8 octets / 16 hex** (contradiction des docs tranchée, voir
  `03-ecarts-conception.md`).
- **Un `sig` par batch** (pas par événement).

## Tests

- `tools/validate.py` **est** la suite de tests. `uv run python tools/validate.py`
  → `✓ 20 fixtures valides — 28 noms d'événements couverts.` (2026-09-09).
- CI : `.github/workflows/contracts.yml` (ruff + régénération stable + validate).

## Limites connues / TODO

- Le `payload` n'est **pas fermé** (`additionalProperties` autorisé) : un champ
  en trop passe. Voulu — l'ajout de champ est rétrocompatible (`schema_version`).
- Pas de fixture « invalide attendue » (batch mal signé, clé interdite) pour
  tester que `validate.py` **rejette** bien. À ajouter si le besoin se confirme.
- `catalogue.py` doit rester synchronisé à la main avec `docs/powl/08` — pas de
  vérification croisée automatique.

## Pour l'oral

Avant d'écrire une ligne des trois morceaux qui manipulent des événements
(cœur, dashboard, firmware), on a figé **le format exact** d'un événement et
**20 exemples de référence, signés**. Résultat : les trois peuvent être codés en
même temps par des personnes différentes, et un test commun (« ces octets
entrent, cette décision sort ») garantit qu'ils se comprendront le jour de
l'intégration.
