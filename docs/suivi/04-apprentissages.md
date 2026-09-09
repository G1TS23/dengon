# Apprentissages

Volet « apprentissage » du suivi. On y note les **notions qu'il a fallu comprendre**
pour coder le projet : algorithmes, protocoles, API, pièges, subtilités de langage.

But : pouvoir **réexpliquer** ces notions à l'oral, et éviter de réapprendre deux
fois la même chose.

Format libre mais court. Une note = un concept. Toujours répondre à : *c'est quoi ?*,
*pourquoi on en a besoin ici ?*, *qu'est-ce qui nous a surpris ?*.

---

## Modèle

### [Titre du concept]

**C'est quoi :** définition en 2-3 phrases, avec ses mots.
**Pourquoi dans dengon :** à quoi ça sert concrètement dans notre code.
**Piège / surprise :** ce qui n'était pas évident, l'erreur qu'on a faite.
**Où c'est utilisé :** `chemin:ligne`.
**Pour aller plus loin :** lien(s).

---

## Notes

### Formulaire d'issue (*issue form*) vs template Markdown

**C'est quoi :** deux façons de pré-remplir une issue GitHub. Le template
Markdown est un fichier `.md` recopié dans la zone de texte. Le formulaire est
un fichier `.yml` décrivant des champs typés (`input`, `textarea`, `dropdown`,
`checkboxes`) que GitHub transforme en vrai formulaire web.
**Pourquoi dans dengon :** la Definition of Ready (§6) exige huit informations.
Avec un template Markdown, on peut tout effacer et soumettre une issue vide en
une seconde — la DoR reste une politesse. Avec un formulaire, un champ
`validations: required: true` **empêche la soumission**. C'est la seule
différence, et c'est toute l'US-113.
**Piège / surprise :** deux choses. D'abord `required` ne se met pas à la racine
de l'élément mais sous `validations:` — placé ailleurs, il est ignoré en
silence, sans erreur. Ensuite, **GitHub ne lit `ISSUE_TEMPLATE/` que sur la
branche par défaut** : sur une branche de travail, aucune prévisualisation n'est
possible, il faut merger pour voir le rendu.
**Où c'est utilisé :** `.github/ISSUE_TEMPLATE/user-story.yml`, `spike.yml`,
`bug.yml`.

### `CODEOWNERS` : dernière ligne gagnante, et pas d'auto-approbation

**C'est quoi :** un fichier qui associe des motifs de chemins à des comptes
GitHub. Quand une PR touche un chemin, ses propriétaires sont automatiquement
sollicités pour la revue.
**Pourquoi dans dengon :** c'est le mécanisme qui rend la DoD §7.1 point 4 —
« revue par une personne d'une autre `area:` » — obligatoire au lieu
d'optionnelle.
**Piège / surprise :** trois.
1. La syntaxe des motifs ressemble à `.gitignore`, et comme lui **la dernière
   ligne qui correspond l'emporte**. Une règle générique `*` écrite en bas du
   fichier écrase donc silencieusement toutes les règles précises. Elle se met
   en tête.
2. **L'auteur d'une PR ne peut jamais s'auto-approuver au titre de
   `CODEOWNERS`.** C'est ce qui permet de mettre deux noms par chemin sans
   affaiblir la règle : le second est toujours quelqu'un d'autre que l'auteur.
3. Un compte **sans droit d'écriture** sur le dépôt est ignoré, sans message.
   GitHub expose un endpoint pour le détecter :
   `gh api repos/OWNER/REPO/codeowners/errors?ref=BRANCHE`.
**Où c'est utilisé :** `.github/CODEOWNERS`.

### Un check requis inexistant fige un dépôt

**C'est quoi :** dans la protection de branche, `required_status_checks.contexts`
liste des **noms de jobs** qui doivent être verts pour merger.
**Pourquoi dans dengon :** la DoD nomme quatre checks (`core`, `sim`, `audit`,
`cross-vectors`) ; trois n'ont pas encore de workflow.
**Piège / surprise :** GitHub n'exige pas que le check existe. Il l'**attend**.
Une PR reste alors sur « Expected — Waiting for status to be reported », sans
message d'erreur, sans échec — juste un bouton de merge grisé pour toujours.
Corollaire déjà rencontré à l'US-104 : un workflow filtré par
`on.pull_request.paths` **ne démarre pas** sur une PR hors périmètre, donc ne
rapporte aucun check, donc produit exactement le même blocage. D'où le filtrage
fait *dans* le job, pas dans le déclencheur (voir `03-ecarts-conception.md`).
Deuxième subtilité : le `PATCH` de `required_status_checks` **remplace** le
tableau `contexts`, il n'y ajoute pas — il faut toujours réécrire la liste
complète.
**Où c'est utilisé :** `docs/suivi/modules/processus-github.md`, commandes de
protection de `main`.

### `workflow_dispatch` n'existe que sur la branche par défaut

**C'est quoi :** le déclencheur qui met un bouton « Run workflow » dans
l'onglet Actions.
**Pourquoi dans dengon :** la synchro des labels est volontairement manuelle —
elle peut supprimer des labels posés sur 55 issues, on ne veut pas qu'un push
la lance.
**Piège / surprise :** GitHub ne propose le bouton que si le fichier est déjà
présent sur la **branche par défaut**. Un workflow `workflow_dispatch` créé dans
une PR est donc **intestable avant merge** — il n'apparaît nulle part. Il faut
l'écrire avec soin, et prévoir un mode simulation par défaut plutôt que de
compter sur un essai préalable.
**Où c'est utilisé :** `.github/workflows/labels.yml`.

Sujets probables (d'après la conception) — à traiter quand on les rencontre :

- Routage épidémique / gossip / store-carry-forward (DTN).
- TTL, déduplication, jitter de relais : pourquoi chacun est nécessaire.
- Noise Protocol Framework : motifs `XX` vs `X`, ce que « forward secrecy » veut dire.
- Ed25519 vs X25519 : signature vs accord de clés.
- Hash-chain (journal chaîné) : en quoi ça rend une trace « infalsifiable », et ses
  limites (n'empêche pas d'omettre avant de signer).
- Golomb-Coded Set / filtre de Bloom : résumer un ensemble de façon compacte.
- BLE GATT : rôle central vs périphérique, MTU, notify vs write, pourquoi un nœud
  doit être les deux.
- ESP-IDF / FreeRTOS : tâches, files, coexistence BLE + Wi-Fi.
- UniFFI : comment un cœur Rust est appelé depuis Kotlin.
- TimescaleDB : hypertable, rétention, agrégats continus.
- MQTT : QoS, topics, mTLS.
