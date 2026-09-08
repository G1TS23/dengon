# dengon — instructions projet

Messagerie **Bluetooth maillée** hors-ligne + **relais ESP32** + **dashboard
d'observabilité** (VPS). Voir [`docs/powl/`](docs/powl/) pour la conception complète.

## Documents de référence

- **Conception (cible)** : [`docs/powl/`](docs/powl/) — commencer par
  [`docs/powl/README.md`](docs/powl/README.md). Sources de vérité : le format de
  paquet (`03`), le catalogue d'événements (`08`), les schémas de données (`09`).
- **Suivi (réel)** : [`docs/suivi/`](docs/suivi/) — ce qui est réellement codé.

## Règle : tenir le suivi technique à jour

**À la fin de toute tâche qui touche au code applicatif**, mettre à jour
[`docs/suivi/`](docs/suivi/) en suivant [`docs/suivi/README.md`](docs/suivi/README.md#règles-de-mise-à-jour) :

1. Ajouter une entrée dans [`docs/suivi/00-journal.md`](docs/suivi/00-journal.md)
   (en haut, append-only, modèle dans `templates/entree-journal.md`).
2. Rafraîchir [`docs/suivi/01-etat-du-code.md`](docs/suivi/01-etat-du-code.md).
3. Créer / mettre à jour la fiche du module dans
   [`docs/suivi/modules/`](docs/suivi/modules/) (modèle `templates/module.md`) +
   la ligne d'index dans `modules/_index.md`.
4. Écart vs conception → [`docs/suivi/03-ecarts-conception.md`](docs/suivi/03-ecarts-conception.md).
5. Notion apprise → [`docs/suivi/04-apprentissages.md`](docs/suivi/04-apprentissages.md).
6. Nouveau terme → [`docs/suivi/05-glossaire.md`](docs/suivi/05-glossaire.md).

Rester factuel : consigner les tests qui échouent, les étapes bâclées, les
commandes réellement exécutées. Le suivi doit permettre une présentation orale
complète du code réel en fin de projet.

## Conventions

- **Commits** : Conventional Commits, imposé par le hook `.githooks/commit-msg`.
  Activer les hooks : `git config core.hooksPath .githooks`.
  Format : `type(scope): description` (types : feat, fix, docs, style, refactor,
  perf, test, build, ci, chore, revert).
- **Langue** : documentation et commentaires en français.
- **Rust** : `cargo fmt`, `clippy -D warnings`, tests avant de considérer une tâche
  finie.
- Ne pas committer ni pousser sans demande explicite de l'utilisateur.
