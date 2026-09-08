# Journal de développement

Journal **append-only**. Entrée la plus récente en haut. Une entrée par session de
travail sur le code. Modèle : [`templates/entree-journal.md`](templates/entree-journal.md).

> On ne modifie jamais une entrée passée. Pour corriger une info, on ajoute une
> nouvelle entrée qui rectifie.

---

<!-- NOUVELLES ENTRÉES ICI (juste en dessous de cette ligne) -->

---

## 2026-09-08 — Mise en place du dossier de suivi

**Auteur :** Claude (Sonnet 5)
**Périmètre :** documentation seule, aucun code applicatif.

### Fait

- Création de [`docs/suivi/`](.) : journal, état du code, fiches modules, écarts,
  apprentissages, glossaire, support oral, templates.
- Ajout des règles de mise à jour (voir [`README.md`](README.md)).

### Décisions

- Le suivi est séparé de la conception (`docs/powl/`) : ici = ce qui est **codé**,
  là-bas = ce qui est **visé**.
- Journal anti-chronologique, append-only, pour garder l'historique d'apprentissage
  intact jusqu'à l'oral.

### État

- Aucune crate créée. Prochaine étape réelle : **Lot 0** de
  [`docs/powl/10-mvp-scope-roadmap.md`](../powl/10-mvp-scope-roadmap.md) (spikes).

### Vérification

- `ls docs/suivi/` → structure en place. Rien à compiler.
