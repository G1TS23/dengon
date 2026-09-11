# Module : `contracts/events` — contrat des événements d'observabilité

**Rôle en une phrase :** figer, sous forme d'artefacts neutres en langage, la
tête d'un événement d'observabilité, le corps de `POST /ingest/batch`, et
20 exemples signés qui font référence pour tous les composants.
**Correspond à la conception :** [`docs/powl/08-observability-events.md`](../../powl/08-observability-events.md),
[`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md) §9.
**Dernière mise à jour :** 2026-09-11
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
- **Identifiants pseudonymes tronqués = 8 octets / 16 hex** (`msg_log_id`,
  `conv_hash`, `from_peer`/`peer`/`to_peer` — contradiction des docs tranchée,
  voir `03-ecarts-conception.md`).
- **Un `sig` par batch** (pas par événement).
- **`canonical_json` avec `allow_nan=False`** : la règle « pas de NaN/Infinity »
  de `CANONICAL.md` est appliquée par la référence, pas seulement écrite (retour
  de revue #60).
- **`pkt.seen.rssi` reste optionnel dans `CATALOGUE`**, malgré `powl/08` et
  `synthese/09` qui le listent sans `?` : le RSSI n'est pas toujours
  disponible côté transport (`TransportEvent::PeerConnected.rssi:
  Option<i16>`, US-105) — le rendre requis forcerait à inventer une valeur
  sur les chemins où le transport n'en a pas. `synthese/09` corrigé
  (`rssi?`) ; écart consigné (retour de revue #60).
- **`batch.schema.json.node_id` référence `envelope.schema.json#/$defs/node_id`**
  via `$ref` plutôt que de retyper le motif `^(relay|client)-[0-9a-f]{6,}$` :
  il n'était dupliqué qu'à cet endroit-là côté JSON Schema (`envelope.schema.json`
  l'a toujours eu en `$defs`) — une divergence future ne peut plus passer
  inaperçue. Le motif reste dupliqué une fois côté Python
  (`catalogue.NODE_ID_PATTERN`, pour `subject_node`) : `catalogue.py` ne peut
  pas faire de `$ref` vers un fichier JSON Schema (retour de revue #60).
- **`seq` validé avant `event_id()`** dans `validate.py` : un `seq` manquant,
  négatif ou non entier faisait planter `int.to_bytes()` (`OverflowError`/
  `AttributeError`) **après** que `_check_schema` l'avait déjà signalé —
  traceback brute au lieu du rapport `✗ N problème(s)` attendu. Reproduit et
  corrigé (retour de revue #60) : un `seq` invalide devient une entrée
  d'`errors`, plus un crash.
- **`Draft202012Validator` mis en cache par nom d'événement** dans
  `_check_payload_vs_catalogue` : il était reconstruit à chaque événement
  alors que le schéma est identique pour tous les events qui partagent un
  `name` — ~28 recompilations pour 28 events sur 20 fixtures (retour de
  revue #60, perf, non bloquant).
- **`node_id`/`payload` validés avant tout calcul** dans `_check_event` : un
  `node_id: null` (présent mais nul — `event.get("node_id", "")` ne couvre
  que la clé *absente*) faisait planter `event_id()`
  (`AttributeError`) ; un `payload` non-objet (ex. une liste) faisait
  planter `.items()`. Même famille de bug que le `seq` invalide ci-dessus,
  trouvée en relecture approfondie par @OswinFreyr. Corrigé de la même
  façon : anomalie → entrée d'`errors`, jamais une exception.
- **`seq` plafonné à `< 2**64`** en plus de `>= 0` : un `seq` en overflow
  (`2**64`) passait la garde initiale et faisait planter
  `seq.to_bytes(8, "big")` (`OverflowError`) — même catégorie de bug,
  trouvée dans la même relecture.
- **`required` natif du schéma remplace une boucle manuelle** dans
  `_payload_validator` : la boucle `for field in spec["required"]: ...`
  dupliquait ce que le mot-clé JSON Schema `required` fait déjà (retour de
  revue #60, nit).
- **Chaque fixture n'est lue/parsée qu'une fois** par `main()` (avant :
  `check_fixture`, la couverture du catalogue et le résumé final relisaient
  chacun les 20 fichiers) — retour de revue #60, nit perf.
- **`minLength`/`maxLength` ajoutés à côté de `pattern`** sur les champs de
  longueur fixe (`HEX16`/`HEX32`/`HEX64` du catalogue, `event_id`/`batch_id`/
  `sig` des schémas) : le moteur regex Python de `jsonschema` fait
  correspondre `$` juste avant un `\n` final, donc `"<16 hex>\n"` passait le
  seul `pattern`. `\Z` (extension Python) aurait fermé le trou mais
  introduirait une dépendance à Python dans des schémas censés rester
  neutres en langage — `minLength`/`maxLength` ferme le même trou sans ça.
  Les motifs **ouverts** (`node_id`, `name`) restent vulnérables : dette
  assumée, documentée dans `03-ecarts-conception.md`.

## Tests

- `tools/validate.py` **est** la suite de tests. `uv run python tools/validate.py`
  → `✓ 20 fixtures valides — 28 noms d'événements couverts.` (2026-09-11).
- Contrôles par fixture : schéma batch/enveloppe · redaction · **signature
  Ed25519** · **`batch_id` recalculé** · par événement : `node_id`/`event_id`
  cohérents (`seq` validé avant tout calcul), `msg_log_id` en 16 hex,
  `payload` vs catalogue **et vs le `payloads.schema.json` livré**. Globaux :
  fraîcheur du schéma généré, couverture du catalogue.
- Négatif vérifié en local : `batch_id` trafiqué → rejet ; champ requis retiré
  d'un payload → rejet par le catalogue **et** par `payloads.schema.json` ;
  `seq` mis à `-1` puis à `2**64` dans une fixture → rapport propre, plus de
  traceback (les deux crashes reproduits sans le fix, absents avec) ;
  `node_id: null` → rapport propre (`AttributeError` reproduite sans le fix) ;
  `payload` remplacé par une liste → rapport propre (`AttributeError`
  reproduite sans le fix) ; `msg_log_id` de 16 hex + `\n` final (17
  caractères) → rejeté par `minLength`/`maxLength` (passait le seul `pattern`
  avant le fix, confirmé en isolant le regex Python).
- CI : `.github/workflows/contracts.yml` (ruff + régénération stable + validate).

## Limites connues / TODO

- Le `payload` n'est **pas fermé** (`additionalProperties` autorisé) : un champ
  en trop passe. Voulu — l'ajout de champ est rétrocompatible (`schema_version`).
- Pas de fixture « invalide attendue » committée (batch mal signé, clé
  interdite) : le rejet est vérifié à la main, pas dans la CI. À committer si le
  besoin se confirme.
- `catalogue.py` doit rester synchronisé à la main avec `docs/powl/08` — pas de
  vérification croisée automatique.

## Pour l'oral

Avant d'écrire une ligne des trois morceaux qui manipulent des événements
(cœur, dashboard, firmware), on a figé **le format exact** d'un événement et
**20 exemples de référence, signés**. Résultat : les trois peuvent être codés en
même temps par des personnes différentes, et un test commun (« ces octets
entrent, cette décision sort ») garantit qu'ils se comprendront le jour de
l'intégration.
