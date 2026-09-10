# Module : `dengon-core` (`crates/dengon-core/`)

**Rôle en une phrase :** la bibliothèque qui contient **tout le protocole** dengon, sans aucune entrée/sortie.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2 et §5 (décision A-2) ; [`docs/synthese/05-protocole-et-trame.md`](../../synthese/05-protocole-et-trame.md) (format de trame).
**Dernière mise à jour :** 2026-09-10
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
| `PacketType` (enum `#[repr(u8)]`) | `src/protocol/types.rs` | 13 types `0x01`–`0x0D`. `from_u8` / `to_u8`, `is_mvp()` (les `GOSSIP_*` = v2), `is_always_signed()`, `is_addressed()`. **`Inventory = 0x0D`** (numéro figé par cette US). |
| `Flags` (newtype `u8`) | `src/protocol/types.rs` | Bitfield `ADDRESSED / SIGNED / FRAGMENT / RELAY_OK / PADDED` + `RESERVED_MASK`. `from_bits_truncate`, `contains`, `has_reserved`. Pas de crate `bitflags` (surface figée, minuscule). |
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
- **`Flags::from_bits_truncate` ignore les bits réservés** ; c'est un contrôle
  explicite (`has_reserved()`) qui les rejette, pas la construction.
- **`Header` porte `recipient_id: Option<PeerId>`** et non un `PeerId` + booléen :
  rend l'invariant « présent ⇔ `ADDRESSED` » vérifiable (`flags_are_consistent`).
- **Vecteurs v0 dans `crates/dengon-core/tests/`** et non `contracts/packet/` :
  le dossier `contracts/` n'est pas encore sur `main` (PR #60). Déplacement
  prévu, consigné dans `03-ecarts-conception.md`.

## Tests

- `src/protocol/consts.rs` — 5 tests : valeurs de référence, cohérence des
  tailles d'en-tête, UUIDs GATT, sens des plages.
- `src/protocol/types.rs` — 7 tests : discriminants contigus `0x01`–`0x0D`,
  `from_u8` inverse de `to_u8`, périmètre MVP, bits de `Flags`, opérations,
  `Header::{header_len, wire_len, flags_are_consistent}`, `AppFrameKind`/`AckStatus`.
- `tests/protocol_vectors.rs` — 3 tests : les 7 vecteurs `accept` sont
  structurellement cohérents avec leurs `expect` (via `protocol::{consts,
  types}`) ; les 6 vecteurs `reject` violent chacun une règle du format ;
  `Inventory` a bien le type `0x0D`.
- `src/lib.rs` — 2 tests fumigènes (inchangés).
- Commande : `cargo test -p dengon-core` → **19 passés** (14 lib + 3 intégration
  + … ), vérifié le 2026-09-10. `clippy -D warnings` propre.

## Limites connues / TODO

- **Pas de codec** : aucun `encode`/`decode` — c'est US-201. Le test des
  vecteurs est donc *structurel* (pas « `decode(bytes) == expect` »).
- Pas de property test (la DoD §7.2 en exigera dès qu'il y aura de la logique
  de sérialisation).
- Couverture ≥ 85 % non mesurée ni imposée.
- `no_std` vérifié sur cible hôte seulement ; xtensa = Spike A (US-101).

## Pour l'oral

US-108 fige le **vocabulaire du protocole** : les 13 types de paquets, les 5
drapeaux, la forme de l'en-tête, et une trentaine de constantes (durée de vie
d'un message, seuils d'anti-inondation, TTL de départ…). Rien ne « fonctionne »
encore, mais c'est le contrat sur lequel les trois implémentations (téléphone,
nœud, firmware) vont s'accorder — d'où les **vecteurs de conformité** : des
paquets d'exemple en octets, avec la bonne réponse, que chaque implémentation
devra savoir lire à l'identique.
