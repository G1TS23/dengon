# contracts/ — contrats inter-composants

Artefacts **neutres en langage** que plusieurs composants consomment sans se
parler. Un changement ici est un changement de contrat : il passe par une PR
revue et un point d'équipe.

| Contrat | Dossier | Consommé par |
| --- | --- | --- |
| Enveloppe d'événement + batch `/ingest/batch` + 20 fixtures golden | [`events/`](events/) | dashboard `api` (US-216/217), `dengon-core::observability` (US-208), firmware relais |

## `events/`

| Fichier | Rôle |
| --- | --- |
| `envelope.schema.json` | JSON Schema d'un événement (strict, `additionalProperties: false`) |
| `batch.schema.json` | JSON Schema du corps `POST /ingest/batch` (strict) |
| `payloads.schema.json` | contraintes de `payload` par nom d'événement — **généré** depuis `tools/catalogue.py` |
| `CANONICAL.md` | **fait foi** : forme canonique du JSON signé + procédure de signature Ed25519 |
| `test-signing-key.json` | clé Ed25519 **de test** (graine publique, déterministe) qui signe les fixtures |
| `fixtures/*.json` | 20 batches valides et signés couvrant les 28 noms d'événements du périmètre MVP |

### Consommer les fixtures

- **Dashboard (US-217)** : chaque fixture est un corps `POST /ingest/batch` prêt
  à l'emploi. Rejeu → mêmes projections attendues.
- **Core (US-208)** : itérer `fixture["events"]` ; chaque élément est une
  enveloppe à round-tripper en JSON canonique.
- **Conformité `cross-vectors`** : signer une fixture avec `test-signing-key.json`
  et comparer octet à octet au `sig` committé prouve que l'implémentation
  respecte `CANONICAL.md`.

### Régénérer (après modification du catalogue)

```bash
cd contracts
uv sync
uv run python tools/build_fixtures.py   # réécrit fixtures/ + payloads.schema.json
uv run python tools/validate.py         # doit être vert
git add events/ && git commit
```

### Vérifier (ce que fait la CI)

```bash
cd contracts && uv sync && uv run python tools/validate.py
```

`validate.py` contrôle : schéma batch + enveloppe, `payload` vs catalogue,
`event_id` / `node_id` cohérents, **signature Ed25519**, **redaction** (aucune
clé `msg_uuid` / `recipient` / texte ; tout `msg_log_id` est une empreinte 16
hex), fraîcheur de `payloads.schema.json`, couverture du catalogue.

## Statut

**Contrat gelé** au merge de la PR US-107. Toute évolution = incrément de
`schema_version` (`CANONICAL.md` §1) + PR + point d'équipe.
