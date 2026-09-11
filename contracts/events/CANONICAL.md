# Forme canonique & signature d'un batch

Ce fichier **fait foi** : l'implémentation Rust (`dengon-core`, firmware) et
l'ingest Python (`dashboard/api`) doivent produire et vérifier **exactement**
ces octets.

## 1. JSON canonique

`canonical_json(x)` = les octets UTF-8 de `x` sérialisé avec :

| Règle | Détail |
| --- | --- |
| Clés d'objet triées | ordre lexicographique par point de code Unicode, **récursivement** |
| Aucun espace | séparateurs `,` et `:` — pas d'espace après |
| Pas d'échappement non-ASCII | les caractères ≥ U+0080 sortent en UTF-8 littéral, pas en `\uXXXX` |
| Nombres = entiers | tous les champs numériques du catalogue sont des **entiers** : sérialisés sans partie fractionnaire (`2`, pas `2.0`) ni exposant. Aucun flottant dans le contrat. |
| Pas de `NaN` / `Infinity` | **interdits** : la sérialisation lève une erreur plutôt que de les émettre |
| Fin | pas de retour à la ligne final |

Référence Python (`contracts/tools/catalogue.py`) :

```python
json.dumps(
    x, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False
).encode("utf-8")
```

`allow_nan=False` fait échouer `json.dumps` sur `NaN` / `Infinity` — le contrat
est ainsi appliqué, pas seulement énoncé.

Côté Rust : deux pièges à traiter pour produire les **mêmes octets**.

1. **Tri des clés.** `serde_json` ne trie pas par défaut : utiliser une `Map`
   triée (feature `preserve_order` + tri explicite) ou `serde_json_canonicalizer`.
2. **Entiers vs flottants.** Un champ numérique désérialisé en `f64` puis
   resérialisé sort en `2.0` et **casse la signature**. Désérialiser les champs
   numériques du catalogue en entier (`u64` / `i64`), jamais en `f64`.

Vérifier l'ensemble contre les fixtures de ce dossier (test de conformité
`cross-vectors`).

> Les fixtures n'utilisent que de l'ASCII : la règle non-ASCII est spécifiée
> pour l'avenir, pas exercée aujourd'hui.

## 2. Signature d'un batch

1. Construire l'objet batch **sans** le champ `sig` :
   `{ batch_id, node_id, schema_version, events }`.
2. `msg = canonical_json(batch_sans_sig)`.
3. `sig_bytes = Ed25519_sign(clé_privée_sign_du_nœud, msg)` — 64 octets.
4. `batch["sig"] = base64_standard(sig_bytes)` — 88 caractères, terminés par `==`.

Vérification : retirer `sig`, recalculer `canonical_json`, `Ed25519_verify` avec
la **clé publique de signature du nœud** (connue du dashboard via la liste
blanche, `POST /api/nodes`).

> **Un seul `sig` par batch**, sur l'ensemble. Il n'y a pas de signature
> par événement : l'intégrité fine vient du journal chaîné (`seq` + `prev_hash`,
> recalculé par `dengon-verify`), et `event_id` assure la déduplication.

## 3. Champs dérivés

| Champ | Calcul |
| --- | --- |
| `event_id` | `hex(SHA-256(node_id_ascii ‖ seq_uint64_big_endian))` — 64 hex |
| `batch_id` | `hex(SHA-256(canonical_json(events)))` — 64 hex |
| `msg_log_id` | `SHA-256(msgID)`, **8 premiers octets** → 16 hex — voir la note |
| `conv_hash` | `SHA-256(min(peerA,peerB) ‖ max(peerA,peerB))`, **8 premiers octets** → 16 hex — voir la note |
| `from_peer` / `peer` / `to_peer` | `peerID` tronqué aux **8 premiers octets** → 16 hex |
| `recipient_tag` | 16 octets → 32 hex (déjà anonyme et tournant, `docs/synthese/06`) |

### Note — largeur des identifiants pseudonymes tronqués

Les documents de conception donnent des notations incohérentes, et lisibles dans
les deux sens (octets ou caractères hex) : `msg_log_id` = `[0..16]` (`powl/08`
§1.3), `[:16]` (`synthese/04` §7), « 16 o » (`synthese/09` §11.2) ;
`conv_hash` = `[0..8]` (`synthese/09` §9) ; `from_peer` = « `peerID` tronqué à
8 o » (`synthese/09` §9).

Ce contrat tranche : **tous ces champs = 8 premiers octets du SHA-256 → 16
caractères hex** (`^[0-9a-f]{16}$`). Les deux exemples concrets du corpus
(`synthese/09` §9 : `msg_log_id` `"4d5e6f7a8b9c0d1e"`, `from_peer`
`"a1b2c3d4e5f60718"`) font 16 hex. Uniformiser évite des largeurs différentes
pour des objets de même nature. `recipient_tag` reste à 16 octets / 32 hex.

Réconciliation à répercuter dans `docs/powl/` et `docs/synthese/` — voir
`docs/suivi/03-ecarts-conception.md`.
