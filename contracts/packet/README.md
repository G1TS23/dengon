# `contracts/packet/` — vecteurs de conformité, neutres en langage

Trois fichiers de vecteurs, **consommés par plusieurs implémentations
indépendantes**. C'est la matière du job CI `cross-vectors` (US-222,
`docs/synthese/10-benchmarks-mvp-tests.md` §4.7) : le même octet, lu par
plusieurs codes écrits séparément, doit donner la même décision.

| Fichier | Contenu | Fait foi |
| --- | --- | --- |
| `vectors_v0.json` | format de trame L3 : paquets en `hex` → champs attendus (`accept`) ou règle violée (`reject`) | [`docs/powl/03-network-protocol.md`](../../docs/powl/03-network-protocol.md), résumé dans [`docs/synthese/05-protocole-et-trame.md`](../../docs/synthese/05-protocole-et-trame.md) |
| `crypto_v0.json` | Noise `XX` / `X`, `recipient_tag`, `PAD_BUCKETS` (US-204) | [`docs/synthese/06-securite-et-crypto.md`](../../docs/synthese/06-securite-et-crypto.md) |
| `identity_v0.json` | keypair, `peer_id`, empreinte, QR `dengon:v1:…`, code de vérification (US-205) | idem |

## Qui lit quoi

| Vecteur | Lecteur | Où |
| --- | --- | --- |
| `vectors_v0.json` | `dengon-core` **avec `std`** (hôte : Android, PC, dashboard) | `crates/dengon-core/tests/protocol_vectors.rs` |
| `vectors_v0.json` | `dengon-core` **sans `std`** — la configuration que le firmware ESP32 embarquera | `crates/dengon-conformance/tests/packet_vectors_nostd.rs` |
| `vectors_v0.json` | **Python**, découpage des octets écrit indépendamment du Rust | `contracts/tools/validate_packets.py` |
| `crypto_v0.json`, `identity_v0.json` | `dengon-core` (`std`) | `crates/dengon-core/tests/{crypto,identity}_vectors.rs` |

La patte firmware est aujourd'hui un **proxy** : `firmware/dengon-relay/` ne
contient que le BLE, le pont `dengon_core_ffi` est l'US-307. Compiler le même
décodeur sans `std` est ce qui s'en approche le plus tant qu'elle n'est pas
livrée — écart consigné dans
[`docs/suivi/03-ecarts-conception.md`](../../docs/suivi/03-ecarts-conception.md).

## Format de `vectors_v0.json`

Deux clés de tête décrivent le format **en données**, pour que chaque lecteur
puisse s'y adosser plutôt que de recopier la spec :

- `header_layout_be` — la disposition des champs, en notation `nom[début:fin]` ;
- `flag_bits` — la valeur de chaque drapeau, `RESERVED_MASK` compris.

Puis `accept[]` = `{name, note, hex, expect{…}}` et `reject[]` =
`{name, hex, reject}`.

## Régénérer `crypto_v0.json` / `identity_v0.json`

Ces deux-là sont **calculés** par le code Rust (graines fixes, sortie
déterministe) ; `vectors_v0.json` est écrit à la main.

```bash
cargo test -p dengon-core --test crypto_vectors   -- --ignored generer_vecteurs
cargo test -p dengon-core --test identity_vectors -- --ignored generer_vecteurs
```

## Vérifier

```bash
cargo test -p dengon-core --test protocol_vectors --test crypto_vectors --test identity_vectors
cargo test -p dengon-conformance
cd contracts && uv run python tools/validate_packets.py
```
