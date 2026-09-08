<!--
Modèle de fiche module. Copier dans docs/suivi/modules/<nom>.md.
But : comprendre le module SANS lire le code. Citer les fichiers en `chemin:ligne`.
Mettre à jour à chaque changement notable du module + la ligne dans modules/_index.md.
-->

# Module : `<nom>` (`crates/<nom>/` ou `<chemin>`)

**Rôle en une phrase :** [à quoi ça sert dans le projet]
**Correspond à la conception :** [docs/powl/NN-....md, section]
**Dernière mise à jour :** AAAA-MM-JJ
**État :** esquisse / partiel / fonctionnel / stable

## À quoi ça sert

[2-4 phrases : le problème que ce module résout, sa place dans le flux global.]

## Structure

```
<nom>/
  src/
    lib.rs        — [ce qu'il expose]
    xxx.rs        — [responsabilité]
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `Xxx` (struct) | `src/xxx.rs:12` | ... |
| `fn yyy(...)` | `src/xxx.rs:40` | ... |

## Flux principal (exemple)

[Décrire un scénario concret de bout en bout : entrée → étapes → sortie.
Diagramme mermaid si utile.]

## Dépendances

- **Internes :** [autres modules dengon]
- **Externes (crates) :** [crate = pourquoi]

## Décisions d'implémentation

- [choix notable + raison, renvoyer au journal si besoin]

## Tests

- [fichiers de test, ce qu'ils couvrent]
- Commande : `cargo test -p <nom>` → [résultat]

## Limites connues / TODO

- [ce qui n'est pas géré, les raccourcis pris, ce qui reste à faire]

## Pour l'oral

[2-3 phrases vulgarisées expliquant ce module à un non-spécialiste + le point
intéressant à mettre en avant.]
