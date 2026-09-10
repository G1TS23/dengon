# Module : `dengon-sim` (`crates/dengon-sim/`)

**Rôle en une phrase :** faire tourner des dizaines de nœuds dengon sur une seule machine, sans radio, pour tester des scénarios réseau impossibles à reproduire à la main.
**Correspond à la conception :** [`docs/synthese/10-benchmarks-mvp-tests.md`](../../synthese/10-benchmarks-mvp-tests.md) §4.3.
**Dernière mise à jour :** 2026-09-09
**État :** esquisse

## À quoi ça sert

On ne peut pas tester un réseau maillé avec trois téléphones. `dengon-sim` lance
N instances de `dengon-core` reliées par un `Transport` **en mémoire**, et
applique un modèle réseau scriptable : latence, perte de paquets, bande
passante, **partition** du réseau, **churn** (nœuds qui apparaissent et
disparaissent), horloges désynchronisées.

Neuf scénarios sont prévus (`direct`, `multihop`, `recipient_offline`,
`sender_offline`, `partition_merge`, `flood`, `dup_paths`, `tamper`,
`key_change`), versionnés en `.ron` et rejoués à chaque PR.

## Structure

```
dengon-sim/
  src/
    lib.rs        — la logique de simulation (aujourd'hui : SEED_PAR_DEFAUT)
    main.rs       — point d'entrée CLI
  scenarios/      — vide (.gitkeep) ; les .ron arrivent avec P1.12
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `SEED_PAR_DEFAUT: u64` | `src/lib.rs:23` | Graine du générateur aléatoire. Fixe, et c'est le point important. |
| `fn main()` | `src/main.rs:6` | Affiche la version et la graine. |

## Flux principal (exemple)

Visé : `dengon-sim crates/dengon-sim/scenarios/partition_merge.ron` → le
simulateur coupe le réseau en deux, laisse chaque moitié accumuler des messages,
reconnecte, et vérifie que tout le monde finit par tout recevoir.

Actuel : le binaire affiche sa version.

## Dépendances

- **Internes :** `dengon-core`.
- **Externes (crates) :** aucune. `ron` (lecture des scénarios) et `rand`
  arriveront avec P1.12.

## Décisions d'implémentation

- **La graine est une constante, pas un aléa.** C'est contre-intuitif pour un
  simulateur, mais indispensable : un scénario qui échoue en CI doit pouvoir
  être rejoué à l'identique en local. Un test non déterministe qui échoue une
  fois sur vingt est pire que pas de test du tout.

## Tests

- `src/lib.rs`, module `tests` : 2 tests (la graine est non nulle, le cœur est lié).
- Commande : `cargo test -p dengon-sim` → 2 passés, 0 échec.

## Limites connues / TODO

- Le simulateur ne simule rien : ni transport en mémoire, ni modèle réseau, ni
  lecture de scénario. Tout est P1.12.
- Le job CI `sim` n'existe pas encore (US-222).

## Pour l'oral

C'est l'outil qui rend le projet démontrable. Un réseau maillé se comporte bien
avec trois appareils et mal avec cinquante — sauf qu'on n'a pas cinquante
appareils. Le simulateur remplace la radio par de la mémoire et permet de poser
des questions qu'on ne pourrait pas poser autrement : que se passe-t-il si le
réseau se coupe en deux pendant une heure puis se reconnecte ? Et comme la
graine aléatoire est fixe, un bug trouvé une fois est reproductible à volonté.
