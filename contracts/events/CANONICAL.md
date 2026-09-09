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
| Entiers | sans `.0`, sans exposant |
| Pas de `NaN` / `Infinity` | interdits |
| Fin | pas de retour à la ligne final |

Référence Python (`contracts/tools/catalogue.py`) :

```python
json.dumps(x, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
```

Côté Rust : `serde_json` ne trie pas les clés par défaut. Utiliser une `Map`
triée (feature `preserve_order` + tri explicite) ou `serde_json_canonicalizer`,
et vérifier contre les fixtures de ce dossier (test de conformité `cross-vectors`).

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
| `msg_log_id` | `hex(SHA-256(msgID))[0:16]` — **16 hex (8 octets)**, voir la note ci-dessous |
| `conv_hash` | `hex(SHA-256(min(peerA,peerB) ‖ max(peerA,peerB)))[0:16]` — 16 hex |
| `from_peer` / `peer` / `to_peer` | `peerID` tronqué à **8 octets** → 16 hex |
| `recipient_tag` | 16 octets → 32 hex (déjà anonyme et tournant, `docs/synthese/06`) |

### Note — longueur de `msg_log_id`

Les documents de conception se contredisent : `docs/powl/08` §1.3 écrit
`SHA-256(msgID)[0..16]`, `docs/synthese/04` §7 `SHA-256(msgID)[:16]`,
`docs/synthese/09` §11.2 commente « hex 16 o ». Le **seul exemple concret**
(`docs/synthese/09` §9, `"4d5e6f7a8b9c0d1e"`) fait **16 caractères hex = 8
octets**. Ce contrat retient donc **8 octets / 16 hex** — le plus court est
préférable (moins corrélable, suffisant pour dédupliquer de l'observabilité).
Réconciliation à répercuter dans `docs/powl/` et `docs/synthese/` — voir
`docs/suivi/03-ecarts-conception.md`.
