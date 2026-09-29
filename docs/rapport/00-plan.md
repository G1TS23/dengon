# Plan détaillé du rapport — dengon

> **Statut : brouillon, non validé en équipe** (critère d'acceptation n°1 de
> l'issue #37 / US-223, pas encore satisfait). Ce plan doit être discuté et
> amendé collectivement avant d'être considéré comme la structure définitive du
> rapport. Voir [`README.md`](README.md) pour le contexte de cette anticipation.

## Objectif du rapport

Documenter, pour un lecteur qui n'a pas suivi le projet au jour le jour : le
problème traité, la recherche qui a informé les choix, la conception retenue,
ce qui a été réellement construit, comment cela a été validé, et ce que l'équipe
en retient. Le rapport doit être **autoportant** : aucune section ne doit
supposer que le lecteur a déjà ouvert `docs/synthese/` ou `docs/suivi/`, même si
ce rapport s'appuie sur ces deux dossiers comme sources.

## Table des matières prévue

| # | Section | Source principale | Rédaction |
| --- | --- | --- | --- |
| 1 | Introduction et contexte | ce plan + `synthese/00-contexte-global.md` §1 | à rédiger |
| 2 | Problème et besoins | `synthese/02-probleme-et-besoins.md` | **[`01-probleme.md`](01-probleme.md)** |
| 3 | État de l'art | `synthese/03-etat-de-lart.md` | **[`02-etat-de-lart.md`](02-etat-de-lart.md)** |
| 4 | Conception | `synthese/04` à `09` | **[`03-conception.md`](03-conception.md)** |
| 4.1 | Architecture générale | `synthese/04-architecture.md` | inclus dans 03-conception.md |
| 4.2 | Protocole réseau et format de trame | `synthese/05-protocole-et-trame.md` | inclus dans 03-conception.md |
| 4.3 | Sécurité | `synthese/06-securite.md` | inclus dans 03-conception.md |
| 4.4 | Cycle de vie des messages et statuts | `synthese/07-cycle-de-vie-et-statuts.md` | inclus dans 03-conception.md |
| 4.5 | Relais ESP32 | `synthese/08-relais-esp32.md` | inclus dans 03-conception.md |
| 4.6 | Dashboard d'observabilité | `synthese/09-dashboard-et-donnees.md` | inclus dans 03-conception.md |
| 5 | Réalisation | `docs/suivi/` (journal, avancement, fiches modules) | **[`04-realisation.md`](04-realisation.md)** |
| 6 | Recette et tests | US-314 (recette E2E, checklist 9 points de `synthese/10-benchmarks-mvp-tests.md` §4.6) | **[`05-recette.md`](05-recette.md)** — en attente du résultat réel de US-314, pas encore exécutée |
| 7 | Difficultés rencontrées et apprentissages | `docs/suivi/04-apprentissages.md`, `docs/suivi/03-ecarts-conception.md` | **[`06-difficultes.md`](06-difficultes.md)** |
| 8 | Conclusion et perspectives (v2) | périmètre hors-MVP de `synthese/02` §6, points ouverts de `synthese/01-sujets-a-trancher.md` | **[`07-conclusion.md`](07-conclusion.md)** |
| 9 | Bibliographie | `synthese/11-glossaire-biblio-annexes.md` | à assembler (renvoi direct possible) |
| 10 | Annexes | glossaire (`synthese/11`), formats abandonnés (annexes A/B de `synthese/11`), captures d'écran de démo | à assembler en fin de projet |

## État des sections

- **Rédigées en anticipation de la fin du Sprint 2 (US-223)** : 2 (Problème),
  3 (État de l'art), 4 (Conception). Les trois s'appuient sur des décisions
  déjà **tranchées** dans `docs/synthese/00-contexte-global.md` — rien dans
  ces sections ne dépend du résultat du développement.
- **Rédigées en anticipation de la fin du Sprint 3 (US-315, sur demande de
  l'équipe, le développement se poursuivant encore)** : 5 (Réalisation), 6
  (Recette — rédigée comme un **protocole prévu et un état de préparation**,
  pas un résultat : la recette US-314 elle-même n'a pas encore eu lieu), 7
  (Difficultés) et 8 (Conclusion). Toutes s'appuient exclusivement sur des
  faits déjà vérifiés dans `docs/suivi/` à la date de rédaction — voir la
  garde en tête de chacun de ces fichiers.
- **Non encore faites, indépendantes du développement** : 1 (Introduction —
  courte, à écrire en dernier une fois le reste stable, pour rester cohérente
  avec le contenu final), 9 et 10 (mécaniques, à assembler depuis
  `synthese/11-glossaire-biblio-annexes.md`).

## Points de vigilance pour la validation en équipe

1. **Formulation « blockchain »** : `synthese/03-etat-de-lart.md` §9 propose une
   formulation précise (« on s'inspire des structures de données de la
   blockchain… sans consensus ») avec un repli lexical si l'énoncé impose
   littéralement le mot. À trancher avant la rédaction finale de la section 4.3
   — la section 4 de ce brouillon retient déjà cette formulation, mais c'est un
   point explicitement marqué « encore ouvert » dans
   `synthese/00-contexte-global.md`.
2. **Sections 5 à 8** ne doivent être écrites qu'à partir de faits vérifiés dans
   `docs/suivi/` — jamais en extrapolant depuis ce plan ou depuis
   `docs/synthese/` (qui décrit une *cible*, pas ce qui a été construit).
3. **Relecture croisée** (stratégie de test de l'US-223, DoR §7) : chaque
   section doit être relue par quelqu'un qui n'est pas son auteur, avant que
   l'issue soit considérée comme terminée.
4. **Vérifier tous les liens** avant la clôture de l'US (critère d'acceptation
   « aucun renvoi vers un fichier ou une section inexistante ») : les sections
   9 et 10 de ce plan ne pointent volontairement vers aucun fichier de rapport
   qui n'existe pas encore (5 à 8 existent désormais, voir la table
   ci-dessus).
