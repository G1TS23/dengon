# Module : `dengon-core` (`crates/dengon-core/`)

**Rôle en une phrase :** la bibliothèque qui contient **tout le protocole** dengon, sans aucune entrée/sortie.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2 et §5 (décision A-2) ; [`docs/synthese/05-protocole-et-trame.md`](../../synthese/05-protocole-et-trame.md) (format de trame).
**Dernière mise à jour :** 2026-09-16
**État :** esquisse — squelette (US-104) + `protocol::{consts, types}` (US-108).

## À quoi ça sert

C'est le cœur du projet, et la raison pour laquelle il n'y a **qu'une seule**
implémentation du protocole : la même bibliothèque est utilisée par
l'application Android (via `dengon-ffi`), par le nœud en ligne de commande
`dengon-node` et par le firmware de l'ESP32.

La règle qui rend ce partage possible : `dengon-core` ne parle **ni à la radio
ni au réseau**. Il produit et consomme des `Vec<u8>`, que quelqu'un d'autre se
charge de transporter (le trait `Transport` de `dengon-ble`).

## Structure

```
dengon-core/
  src/
    lib.rs              — bascule no_std, alias PROTOCOL_VERSION, VERSION
    protocol/
      mod.rs           — re-exports du module protocol
      consts.rs        — TOUTES les constantes du protocole (synthese/05 §2)
      types.rs         — PacketType, Flags, Header, AppFrameKind, AckStatus,
                         alias PeerId / MsgId / Signature
  tests/
    vectors_v0.json    — vecteurs de conformité v0 (bytes -> champs / rejet)
    protocol_vectors.rs — contrôle structurel de ces vecteurs
```

Modules encore absents : `codec` (US-201), `crypto`, `identity`, `store`,
`sync`, `ledger`, `observability`, `api` (sprint 2).

## Concepts / types importants

| Type / fonction | Fichier | Ce que ça fait |
|---|---|---|
| `protocol::consts::*` | `src/protocol/consts.rs` | ~35 constantes : `PROTO_VERSION`, TTL (`TTL_DEFAULT=7`, clamp densité), fragmentation, `MSG_TTL_S`, anti-inondation, `PAD_BUCKETS`, UUIDs GATT, tailles de champ d'en-tête… Transcription de `synthese/05` §2. |
| `PacketType` (enum `#[repr(u8)]`) | `src/protocol/types.rs` | 13 types `0x01`–`0x0D`. `from_u8` / `to_u8`, `is_mvp()` (les `GOSSIP_*` = v2), `is_always_signed()`, `is_addressed() -> Option<bool>` (`None` = hérite, `Fragment`). **`Inventory = 0x0D`** (numéro figé par cette US). |
| `Flags` (newtype `u8`) | `src/protocol/types.rs` | Bitfield `ADDRESSED / SIGNED / FRAGMENT / RELAY_OK / PADDED` + `RESERVED_MASK`. `from_bits_truncate` (masque), `from_bits_raw` (préserve, diagnostic), `contains`, `has_reserved` (diagnostic — les bits réservés sont **ignorés**, pas rejetés, à la réception). Pas de crate `bitflags` (surface figée, minuscule). |
| `Header` | `src/protocol/types.rs` | En-tête L3 **décodé** (champs, pas d'octets). `header_len()` (22 broadcast / 30 adressé), `wire_len()`, `flags_are_consistent()`. La conversion octets ⇄ `Header` est US-201. |
| `AppFrameKind`, `AckStatus` | `src/protocol/types.rs` | Frames L4 dans Noise (`Message`, `Ack`, `ReadReceipt` v2, `Profile` post-MVP) ; statut d'accusé (`Delivered=2`, `Read=3` v2). |
| `PROTOCOL_VERSION: u8` | `src/lib.rs` | **Alias** de `protocol::consts::PROTO_VERSION` ; gardé parce que les crates sœurs l'utilisent comme test de liaison (US-104). |

## Flux principal (exemple)

Pas encore de flux : US-108 livre les **types**, pas la logique. `sync::routing`
(US-209) s'écrira contre `PacketType` / `Flags` / `Header` sans attendre le
codec (US-201). Le flux visé : `04-architecture.md` §4.

## Dépendances

- **Internes :** aucune. Racine du graphe.
- **Externes (runtime) :** aucune. `protocol` n'utilise que `core`.
- **Externes (dev) :** `serde_json` — lecture de `tests/vectors_v0.json` via
  `Value` (pas de derive, donc `serde` n'est pas tiré comme proc-macro).
  N'affecte pas la compilation `no_std` (`cargo check` ne compile pas les
  dev-deps).

## Décisions d'implémentation

- **`protocol::{consts, types}` séparé de `protocol::codec`** (US-201) : permet
  à `sync::*` de démarrer sans la sérialisation. C'est l'objet même de l'US-108.
- **`no_std` garanti** : `protocol` n'importe que `core` (`core::ops::RangeInclusive`,
  `core::ops::BitOr`). Vérifié par `cargo check -p dengon-core --no-default-features`.
- **Bitfield maison** plutôt que la crate `bitflags` : 5 bits, API figée.
- **`Flags::from_bits_truncate` ignore les bits réservés ; `from_bits_raw` les
  préserve** (retour de revue #63, point de Paul) : avant, aucun constructeur
  public ne pouvait poser un bit 5-7, donc `has_reserved()` était inatteignable
  hors du module — un test devait lire `raw[3] & Flags::RESERVED_MASK`
  directement au lieu de l'API.
- **Bit réservé posé ⇒ `has_reserved() == true`, mais plus un motif de rejet**
  (retour de revue #63, point de Paul) : `synthese/05:80` — « ignoré à la
  réception », pas « non conforme ». `Header::flags_are_consistent()` ne
  vérifie plus l'absence de bits réservés ; sans ce correctif, un bit v1.1
  futur aurait fait jeter 100 % du trafic v1.1 par un nœud v1.0.
- **`is_always_signed()` ne compte plus `GossipPush`** (retour de revue #63,
  point de Paul) : `synthese/05:122` le dit non signé (son payload est une
  liste de paquets déjà signés individuellement).
- **`is_addressed() -> Option<bool>`**, pas `bool` (retour de revue #63,
  point de Paul) : `Fragment` **hérite** de l'adressage du paquet transporté
  (`synthese/05:123`) — ni oui ni non, un troisième cas qu'un booléen ne peut
  pas représenter. Avant, le test d'intégration devait court-circuiter
  `Fragment` avec un `if pt != Fragment` pour contourner l'API.
- **`Header` porte `recipient_id: Option<PeerId>`** et non un `PeerId` + booléen :
  rend l'invariant « présent ⇔ `ADDRESSED` » vérifiable (`flags_are_consistent`).
- **Vecteurs v0 dans `crates/dengon-core/tests/`** et non `contracts/packet/` :
  le dossier `contracts/` n'est pas encore sur `main` (PR #60). Déplacement
  prévu, consigné dans `03-ecarts-conception.md`.
- **`is_rejected()` (test) : garde de longueur `raw.len() < hdr`, pas
  `hdr + 2`** (retour de revue #63, point de Paul) : `hdr` (`HEADER_LEN_
  BROADCAST`/`_ADDRESSED`) inclut déjà les 2 octets de `payload_len`
  (`consts::tailles_den_tete_coherentes`). L'ancienne garde rejetait à tort
  tout paquet valide avec `payload_len ∈ {0, 1}`, et rendait 3 des 6 vecteurs
  `reject` détectables par cette règle de longueur plutôt que par celle
  qu'ils nomment (version/type/réservé) — vérifié en isolant chaque contrôle.
- **`reserved-flag-set` a quitté `reject` pour `accept`** (renommé
  `noise-msg-addressed-reserved-bit-ignored`, retour de revue #63, point de
  Paul) — c'est la conséquence directe du point précédent : un bit réservé
  posé n'est plus une raison de rejet. `bad-version`/`unknown-type` allongés
  de 2 octets pour rester rejetés par leur propre règle même sous une garde
  de longueur hypothétiquement encore buguée.
- **`announce-broadcast-signed`/`log-attest-broadcast-signed` : `RELAY_OK`
  ajouté** (`flags` `0x02`→`0x0a`, retour de revue #63, point de Paul) : ces
  deux paquets broadcast avaient `RELAY_OK` absent avec un TTL de 2-3 — ils
  seraient morts au premier saut (`synthese/05:203`, `RELAY_OK && ttl > 1 ?`),
  rendant le TTL non nul incohérent avec un relais impossible.
- **`cast_possible_truncation`/`cast_sign_loss`/`cast_possible_wrap` activés**
  dans `Cargo.toml` racine (retour de revue #63, point de Paul) : commentés
  « à activer avec `protocol` (US-108) » — c'est cette US. Un seul site
  touché (`i as u8` dans un test → `u8::try_from(i).unwrap()`).

## Tests

- `src/protocol/consts.rs` — 5 tests : valeurs de référence, cohérence des
  tailles d'en-tête, UUIDs GATT, sens des plages.
- `src/protocol/types.rs` — 8 tests : discriminants contigus `0x01`–`0x0D`,
  `from_u8` inverse de `to_u8`, périmètre MVP, bits de `Flags`, opérations
  (dont `from_bits_raw` vs `from_bits_truncate` sur un bit réservé),
  `Header::{header_len, wire_len, flags_are_consistent}`, **un bit réservé
  posé n'invalide plus `flags_are_consistent()`** (nouveau, retour de revue
  #63), `AppFrameKind`/`AckStatus`.
- `tests/protocol_vectors.rs` — 3 tests : les 8 vecteurs `accept` sont
  structurellement cohérents avec leurs `expect` (via `protocol::{consts,
  types}`) ; les 5 vecteurs `reject` violent chacun une règle du format ;
  `Inventory` a bien le type `0x0D`.
- `src/lib.rs` — 2 tests fumigènes (inchangés).
- Commande : `cargo test -p dengon-core` → **18 passés** (15 lib + 3 intégration
  + 0 doc), vérifié le 2026-09-16. `clippy -D warnings` propre, y compris avec
  `cast_possible_truncation`/`cast_sign_loss`/`cast_possible_wrap` activés.
- Négatif vérifié en local : la garde de longueur `hdr + 2` réintroduite
  temporairement fait échouer `accept_vectors_are_structurally_consistent`
  sur le nouveau vecteur `noise-msg-addressed-reserved-bit-ignored` (30
  octets, exactement `hdr`) — confirme le « mirror bug » signalé par Paul
  (un paquet valide à `payload_len` faible rejeté à tort).

## Limites connues / TODO

- **Pas de codec** : aucun `encode`/`decode` — c'est US-201. Le test des
  vecteurs est donc *structurel* (pas « `decode(bytes) == expect` »).
- Pas de property test (la DoD §7.2 en exigera dès qu'il y aura de la logique
  de sérialisation).
- Couverture ≥ 85 % non mesurée ni imposée.
- `no_std` vérifié sur cible hôte seulement ; xtensa = Spike A (US-101).
- **`timestamp_ms` des vecteurs `accept` figé à une date fixe (2024-07-29),
  hors tolérance anti-rejeu `TIMESTAMP_TOLERANCE_MS` (±2 h)** — signalé
  hors-diff par Paul (revue PR #63) : un décodeur qui appliquerait l'anti-rejeu
  (US-201) rejetterait les 8 vecteurs `accept` tels quels. Pas corrigé dans
  cette session : la bonne solution (un `reference_now_ms` racine dans
  `vectors_v0.json`, lu par le futur décodeur au lieu de l'horloge système)
  relève du design du codec, pas d'un ajustement de constante — mieux traité
  avec US-201 qui en aura l'usage réel. Idem pour `expect.msg_id` (absent des
  vecteurs — seul endroit où une divergence d'endianness serait visible entre
  Rust/C/Python).

## Pour l'oral

US-108 fige le **vocabulaire du protocole** : les 13 types de paquets, les 5
drapeaux, la forme de l'en-tête, et une trentaine de constantes (durée de vie
d'un message, seuils d'anti-inondation, TTL de départ…). Rien ne « fonctionne »
encore, mais c'est le contrat sur lequel les trois implémentations (téléphone,
nœud, firmware) vont s'accorder — d'où les **vecteurs de conformité** : des
paquets d'exemple en octets, avec la bonne réponse, que chaque implémentation
devra savoir lire à l'identique.
