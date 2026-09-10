# État du code — comment lire l'état courant

Ce fichier ne contient que des **pointeurs** (il change rarement, donc pas de
conflit de merge). L'état réel se lit ici :

| Pour savoir… | Regarder |
|---|---|
| l'avancement par composant + l'outillage | [`02-avancement.md`](02-avancement.md) |
| le détail d'un module (structs, flux, tests) | [`modules/`](modules/) et son [index](modules/_index.md) |
| ce qui est **en cours** (branches, PR ouvertes) | le board GitHub · `gh pr list` |
| pourquoi le code diverge de la conception | [`03-ecarts-conception.md`](03-ecarts-conception.md) |
| l'historique des sessions | [`00-journal.md`](00-journal.md) |

## Résumé (1 ligne)

Lot 0 en cours (fondations & spikes). **Aucun composant applicatif n'est encore
sur `main`.**

## Commandes utiles

_(à enrichir quand il y aura du code : comment builder, lancer, tester chaque
partie — voir aussi la note d'onboarding en tête de chaque fiche `modules/`.)_

```bash
# Labels du dépôt vs fichier versionné
gh label list --limit 100 --json name,color,description

# État de fusion d'une PR (fiable même sans droit admin)
gh pr view <n> --json mergeStateStatus,statusCheckRollup

# Les attributs de merge sont-ils actifs ?
git check-attr merge -- docs/suivi/00-journal.md   # -> merge: union
```

## Prochaines étapes

[`docs/synthese/10-benchmarks-mvp-tests.md`](../synthese/10-benchmarks-mvp-tests.md)
§3.2 — **Lot 0** : Spike A (`dengon-core` cross-compile xtensa ?), Spike B
(`btleplug` peripheral), Spike C (Android GATT server en foreground).
