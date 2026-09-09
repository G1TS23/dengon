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

### 2026-09-09 — Dossier `contracts/` ajouté au layout du dépôt (US-107)

- **Prévu :** `docs/synthese/04` §5 dessine le dépôt sans dossier pour les
  artefacts de contrat inter-langages ; les « vecteurs de conformité » y sont
  seulement évoqués (`synthese/10` §4.7, job `cross-vectors`).
- **Réel :** un dossier **`contracts/`** à la racine, contenant `events/`
  (schémas JSON + `CANONICAL.md` + 20 fixtures signées) et `tools/` (générateur
  + validateur, outillés `uv`).
- **Raison :** ces artefacts sont **neutres en langage** et consommés par trois
  composants (`dashboard/` Python, `crates/` Rust, `firmware/` C). Les mettre
  sous l'un d'eux créerait une dépendance de build inversée ; sous `docs/` ils
  ne seraient pas exécutables par la CI.
- **Conséquences :** un `paths: contracts/**` de plus en CI
  (`.github/workflows/contracts.yml`). Le job `cross-vectors` de `synthese/10`
  §4.7, quand il existera, consommera `contracts/events/fixtures/`.
- **Doc de conception mise à jour ?** non (layout indicatif). À mentionner au
  prochain rafraîchissement de `synthese/04` §5.

---

### 2026-09-09 — `msg_log_id` : 8 octets / 16 hex (contradiction des docs tranchée, US-107)

- **Prévu :** trois formulations incohérentes — `docs/powl/08` §1.3
  `hex(SHA-256(msgID)[0..16])`, `docs/synthese/04` §7 `SHA-256(msgID)[:16]`,
  `docs/synthese/09` §11.2 commente « hex 16 o ». « 16 » = octets ou caractères ?
- **Réel :** le contrat US-107 retient **8 octets → 16 caractères hex**
  (`^[0-9a-f]{16}$`), imposé aux fixtures et à `payloads.schema.json`.
- **Raison :** le **seul exemple concret** du corpus (`docs/synthese/09` §9 :
  `"4d5e6f7a8b9c0d1e"`) fait 16 hex. Et pour de l'observabilité, plus court =
  moins corrélable, tout en gardant de quoi dédupliquer.
- **Conséquences :** `docs/powl/08`, `docs/synthese/04` §7 et `docs/synthese/09`
  §11.2 doivent être alignés sur « 8 octets / 16 hex » (colonne
  `messages.msg_log_id` du schéma dashboard : `TEXT` de 16 caractères).
  `dengon-core` (US-208) et l'ingest (US-216) doivent tronquer à 8 octets.
- **Doc de conception mise à jour ?** pas encore — à répercuter dans
  `docs/powl/` et `docs/synthese/`. Noté dans `contracts/events/CANONICAL.md` §3.

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
