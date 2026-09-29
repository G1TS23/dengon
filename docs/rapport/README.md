# docs/rapport — Rapport écrit du projet

Ce dossier contient le **texte du rapport** destiné au jury/à l'enseignant
(US-223, puis US-315 pour la version finalisée) — à distinguer de
[`docs/synthese/`](../synthese/), qui est la **base de conception interne** sur
laquelle ce rapport s'appuie, et de [`docs/suivi/`](../suivi/), qui trace ce qui
est **réellement codé** au jour le jour.

## Pourquoi maintenant (anticipation)

US-223 était planifiée pour la fin du Sprint 2 (jalon **J5**, 21/09) : rédiger
tôt ce qui ne dépend pas des résultats de développement, puisque le problème,
l'état de l'art et la conception sont déjà figés dans `docs/synthese/`. Les
sections 2 à 4 ont été produites **en avance** sur cette échéance, à la
demande de l'équipe. Les sections 5 à 8 (Réalisation, Recette, Difficultés,
Conclusion), elles, relèvent normalement de l'US-315 — prévue seulement une
fois le développement terminé — mais ont, elles aussi, été rédigées par
anticipation, sur demande de l'équipe, alors que le développement se
poursuivait encore : chaque fait qu'elles avancent est vérifié dans
`docs/suivi/` à la date de rédaction (voir la garde en tête de chaque
fichier), pas projeté.

Ce dossier ne remplace pas les étapes restantes de la Definition of Ready /
Definition of Done des issues
[US-223 (#37)](https://github.com/G1TS23/dengon/issues/37) et
[US-315 (#53)](https://github.com/G1TS23/dengon/issues/53) : **le plan n'est
pas encore validé en équipe**, et **aucune section n'a encore été relue par
une autre personne** — critères d'acceptation explicites des deux US. Statut
réel de chaque fichier : voir [`00-plan.md`](00-plan.md#état-des-sections).

## Contenu

| Fichier | Contenu | Statut |
| --- | --- | --- |
| [`00-plan.md`](00-plan.md) | Plan détaillé du rapport complet | brouillon, **à valider en équipe** |
| [`01-probleme.md`](01-probleme.md) | Section « Problème » | rédigé, à relire |
| [`02-etat-de-lart.md`](02-etat-de-lart.md) | Section « État de l'art » | rédigé, à relire |
| [`03-conception.md`](03-conception.md) | Section « Conception » | rédigé, à relire |
| [`04-realisation.md`](04-realisation.md) | Section « Réalisation » | rédigé, à relire |
| [`05-recette.md`](05-recette.md) | Section « Recette et tests » | protocole + état de préparation ; **résultats réels en attente de US-314** |
| [`06-difficultes.md`](06-difficultes.md) | Section « Difficultés rencontrées et apprentissages » | rédigé, à relire |
| [`07-conclusion.md`](07-conclusion.md) | Section « Conclusion et perspectives » | rédigé, à relire |

## Règle de mise à jour

Ce dossier décrit la conception **telle que figée dans `docs/synthese/`**, pas
l'état du code. Si une décision de `docs/synthese/` change, répercuter ici
seulement une fois `docs/synthese/` lui-même mis à jour (voir sa propre règle de
mise à jour). Les sections qui ne peuvent être écrites qu'une fois le
développement terminé (réalisation, recette, difficultés, conclusion) sont
listées dans le plan mais **ne doivent pas être anticipées** au-delà de ce qui
est déjà vrai — pas d'affirmation sur un résultat qui n'existe pas encore.
