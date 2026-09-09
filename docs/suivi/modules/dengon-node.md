# Module : `dengon-node` (`crates/dengon-node/`)

**Rôle en une phrase :** un nœud dengon complet, sans interface graphique, lancé en ligne de commande.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §5.
**Dernière mise à jour :** 2026-09-09
**État :** esquisse

## À quoi ça sert

Trois usages, tous pratiques : servir de **banc de test** (deux nœuds sur deux
PC valent mieux que deux téléphones pour déboguer), tenir le rôle de **nœud
fixe** sur une machine branchée en permanence, et **amorcer le maillage** quand
il n'y a encore personne d'autre.

Invocation visée par la conception :
`dengon-node run --name alice --db ./alice.db`.

## Structure

```
dengon-node/
  src/
    main.rs       — pour l'instant : affiche la version et s'arrête
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `fn main()` | `src/main.rs:17` | Affiche `dengon-node 0.1.0 — squelette (protocole v1)`. |

## Flux principal (exemple)

```
$ cargo run -p dengon-node
dengon-node 0.1.0 — squelette (protocole v1)
```

## Dépendances

- **Internes :** `dengon-core` et `dengon-ble`. C'est la seule crate qui dépend
  des deux — normal, c'est elle qui assemble le protocole et la radio.
- **Externes (crates) :** aucune. `clap` (arguments) et `tokio` (boucle async)
  arriveront avec P1.11.

## Décisions d'implémentation

- `tokio` est délibérément absent du cœur : la conception réserve l'async à
  `dengon-node`, pour que `dengon-core` n'impose aucun runtime au firmware
  ESP32 ([`04-architecture.md`](../../synthese/04-architecture.md) §7).

## Tests

- `src/main.rs`, module `tests` : 1 test qui vérifie que les **deux**
  dépendances sont réellement liées.
- Commande : `cargo test -p dengon-node` → 1 passé, 0 échec.

## Limites connues / TODO

- Aucune analyse d'arguments : la sous-commande `run` et les options `--name` /
  `--db` n'existent pas.
- Pas de boucle d'événements, pas de base SQLite, pas de transport.

## Pour l'oral

C'est la version « ligne de commande » de l'application : mêmes capacités, sans
l'interface. Son intérêt est surtout méthodologique — c'est beaucoup plus simple
de démontrer et de déboguer un protocole réseau avec deux fenêtres de terminal
côte à côte qu'avec deux téléphones dans les mains.
