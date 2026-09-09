# Écarts entre le code et la conception

La conception ([`docs/powl/`](../powl/)) est une cible, pas un contrat. Quand
l'implémentation s'en écarte (contrainte technique, simplification, meilleure idée,
erreur de conception découverte), on le note **ici**, avec la raison, pour :

- ne pas se faire piéger par une doc de conception périmée ;
- pouvoir l'expliquer à l'oral (« on avait prévu X, en pratique Y parce que Z »).

Si un écart est structurant, mettre aussi à jour le document `docs/powl/` concerné
et le mentionner dans l'entrée de journal.

---

## Modèle d'entrée

### [date] — [titre court de l'écart]

- **Prévu :** ce que dit `docs/powl/NN-....md` (référence précise).
- **Réel :** ce qui est codé.
- **Raison :** pourquoi.
- **Conséquences :** impact sur le reste (autres modules, sécurité, perfs, planning).
- **Doc de conception mise à jour ?** oui / non (+ lien).

---

_(aucun écart pour l'instant)_

---

### 2026-09-09 — Dashboard : migrations en littéral Python, pas en fichiers `.sql`

- **Prévu :** l'issue #10 (US-110) demande une « migration SQLite initiale
  **versionnée** ». Le premier jet a suivi le pattern classique : un dossier
  `dashboard/api/migrations/` avec `0001_initial.sql`, lu et exécuté au
  démarrage.
- **Réel :** les migrations sont une liste `(version, nom, sql)` dans
  `dashboard/api/app/migrations.py`, où `sql` est un littéral de chaîne. `db.py`
  itère cette liste. Plus de dossier `migrations/`.
- **Raison :** SonarCloud (`pythonsecurity:S3649`, **BLOCKER**, gate de sécurité
  du nouveau code) flaggait le chemin `Path.read_text()` → `executescript()`
  comme « SQL construit depuis une donnée contrôlée par l'utilisateur ». C'est
  un faux positif — la donnée est un fichier versionné, pas de l'entrée
  requête — mais corriger à la racine est ici plus propre qu'une suppression
  `# NOSONAR` : un littéral de module est tout aussi versionné, n'ajoute aucune
  dépendance au système de fichiers au déploiement, et supprime le motif que
  l'analyseur (à raison, en général) surveille.
- **Conséquences :** le mot « versionné » de l'AC reste satisfait (git + numéro
  de version + table `schema_migrations`). Si le nombre de migrations grossit
  au point de rendre un seul fichier Python pénible, repasser à des `.sql`
  lus est un refactor localisé à `db.py` + `migrations.py`.
- **Doc de conception mise à jour ?** non — `docs/synthese/09` ne prescrit pas
  la forme des migrations, seulement le schéma cible (§11.2).

### 2026-09-09 — Protection de `main` : un seul check requis (`core`) au lieu de quatre

- **Prévu :** l'issue #13 (US-113) et
  [`docs/olivier/proposition-organisation-github.md`](../olivier/proposition-organisation-github.md)
  §4.3 demandent une protection de `main` avec **quatre checks requis** :
  `core`, `sim`, `audit` et `cross-vectors`. La DoD §7.1 point 3 les redit.
- **Réel :** les commandes livrées dans
  [`modules/processus-github.md`](modules/processus-github.md) ne déclarent que
  `"contexts": ["core"]`.
- **Raison :** `sim`, `audit` et `cross-vectors` **n'existent pas**. §4.3 les
  rattache à d'autres US, non commencées. Or un check requis qu'aucun workflow
  ne rapporte laisse la PR sur « Expected — Waiting for status to be reported »
  **indéfiniment** : GitHub attend un statut que personne n'enverra jamais. Les
  déclarer « pour être conforme à la DoD » gèlerait la totalité du dépôt, y
  compris les PR documentaires — c'est-à-dire la majorité de nos PR.
- **Conséquences :** l'intention de la DoD est préservée pour le seul job qui
  existe. Chaque nouveau workflow devra être ajouté à la liste **au moment où
  il est mergé**, en répétant la liste complète (l'API `PATCH` remplace le
  tableau `contexts`, elle ne l'enrichit pas). La commande d'élargissement est
  écrite dans la fiche. Condition de levée : à la fermeture de l'US qui livre
  chaque workflow.
- **Doc de conception mise à jour ?** non — la cible reste bien quatre checks ;
  seul le calendrier d'activation change. Le point est signalé dans le corps de
  la PR de l'US-113 et dans `modules/processus-github.md`.

---

### 2026-09-09 — Le label est `good first issue`, pas `good-first-issue`

- **Prévu :** §5.3 liste le label `good-first-issue`, avec des traits d'union.
- **Réel :** `.github/labels.yml` déclare `good first issue`, avec des espaces.
- **Raison :** c'est le label **par défaut de GitHub**, il existe déjà sur le
  dépôt et il est utilisé. Créer la variante à traits d'union donnerait deux
  labels au sens identique, dont un vide, et casserait la vue du board qui
  filtre sur le nom réel. Le fichier `labels.yml` a pour rôle de décrire
  l'existant : le corriger ici aurait fait mentir sa promesse de « zéro
  changement au premier sync ».
- **Conséquences :** aucune, sinon qu'il faut écrire le nom avec des espaces
  quand on filtre. La politique de déblocage de §10.3 point 5 (« prendre une
  `good-first-issue` d'une autre area ») s'applique inchangée.
- **Doc de conception mise à jour ?** non — coquille de nommage, sans effet sur
  l'organisation décrite.
