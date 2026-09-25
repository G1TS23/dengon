# `dengon-core/tests/`

## `vectors_v0.json` — vecteurs de conformité du format de trame L3

Chaque entrée décrit un paquet **en octets** (`hex`) et soit les champs attendus
après décodage (`accept[].expect`), soit la règle qu'il viole (`reject[].reject`).

- **Fait foi** : [`docs/powl/03-network-protocol.md`](../../../docs/powl/03-network-protocol.md),
  résumé dans [`docs/synthese/05-protocole-et-trame.md`](../../../docs/synthese/05-protocole-et-trame.md).
- **Consommé par** : `dengon-core` (ce dossier), et plus tard le firmware ESP32
  et le dashboard — c'est la base du job CI `cross-vectors` (US-222,
  `docs/synthese/10` §4.7).

À l'US-108, il n'y a **pas de décodeur** (`protocol::codec` = US-201) :
`protocol_vectors.rs` vérifie donc seulement la **cohérence structurelle** des
vecteurs (version, type, drapeaux, longueurs, `Header::wire_len`…). US-201
branchera `decode()` sur le même fichier et comparera à `expect`.

**Emplacement provisoire.** Ces vecteurs ont vocation à rejoindre
`contracts/packet/` (neutre en langage) une fois ce dossier stabilisé sur
`main` — voir `docs/suivi/03-ecarts-conception.md`.
