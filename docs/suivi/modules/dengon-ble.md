# Module : `dengon-ble` (`crates/dengon-ble/`)

**Rôle en une phrase :** la couche qui cache la radio Bluetooth au reste du programme.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §3.
**Dernière mise à jour :** 2026-09-09
**État :** esquisse

## À quoi ça sert

`dengon-core` ne sait pas envoyer un octet. `dengon-ble` porte le trait
`Transport`, décrit dans la conception comme « l'unique couture entre le cœur et
les plateformes » : quatre méthodes (`start`, `poll`, `send`, `broadcast`).
Chaque plateforme en fournit sa propre implémentation — `btleplug` sur PC,
`BluetoothGattServer` côté Android, NimBLE sur l'ESP32 — et le cœur ne voit
jamais la différence.

## Structure

```
dengon-ble/
  src/
    lib.rs        — pour l'instant : uniquement la constante VERSION
```

Le trait `Transport` et son implémentation `btleplug_transport.rs` **n'existent
pas encore**.

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `VERSION: &str` |  `src/lib.rs:16` | Version de la crate. Seule chose exposée à ce jour. |

## Flux principal (exemple)

Aucun.

## Dépendances

- **Internes :** `dengon-core`.
- **Externes (crates) :** aucune. `btleplug` sera ajouté par l'US-105.

## Décisions d'implémentation

- Le test de la crate référence `dengon_core::PROTOCOL_VERSION` : il ne teste
  rien d'utile en soi, il **vérifie que l'arête du graphe de dépendances est
  réellement câblée**. Une dépendance déclarée mais jamais utilisée n'émet aucun
  avertissement et devient du poids mort invisible.

## Tests

- `src/lib.rs`, module `tests` : 1 test de liaison.
- Commande : `cargo test -p dengon-ble` → 1 passé, 0 échec.

## Limites connues / TODO

- Le trait `Transport` n'est pas écrit : c'est **US-105**, et c'est un
  bloquant pour `dengon-node` comme pour `dengon-sim`.
- Le Spike B (btleplug en rôle GATT *peripheral* sous Linux) n'a pas été fait :
  on ne sait pas encore si la bibliothèque le permet.

## Pour l'oral

Cette petite crate résout un problème d'architecture classique : comment écrire
un programme qui parle Bluetooth sans le coupler à une plateforme précise ? La
réponse est une interface de quatre fonctions. Tout le reste du code ignore s'il
tourne sur un téléphone, un PC ou un microcontrôleur — et c'est ce qui rend le
simulateur possible, puisqu'il suffit d'écrire une implémentation « en mémoire »
de cette même interface.
