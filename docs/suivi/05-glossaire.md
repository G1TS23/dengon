# Glossaire

Termes du projet et du domaine, définis simplement. Complété au fil du code.
Si un terme apparaît dans une fiche module ou le journal sans être ici, on l'ajoute.

| Terme | Définition courte |
|---|---|
| **dengon** (伝言) | « message confié à quelqu'un pour qu'il le transmette ». Nom du projet. |
| **BLE** | Bluetooth Low Energy. Radio courte portée, faible conso, base de tout le réseau. |
| **Maillage / mesh** | Réseau où chaque nœud relaie les messages des autres, sans point central. |
| **DTN** | *Delay-Tolerant Network*. Réseau qui fonctionne même sans chemin complet à un instant donné : on stocke et on transmet plus tard. |
| **Store-carry-forward** | Garder un message, le transporter (physiquement, en bougeant), le retransmettre à la prochaine rencontre. |
| **Routage épidémique / gossip** | Propagation « de proche en proche » : à chaque rencontre, deux nœuds s'échangent ce qu'ils n'ont pas en commun. |
| **TTL** (*time to live*) | Compteur de sauts d'un paquet ; décrémenté à chaque relais, jeté à 0. Empêche les boucles infinies. |
| **Déduplication / seen-set** | Mémoire des messages déjà vus (par leur `msgID`) pour ne pas les relayer en boucle. |
| **Jitter de relais** | Petit délai aléatoire avant de relayer, pour que les doublons s'annulent. |
| **Flood contrôlé** | Diffusion à tous les voisins, mais bornée par TTL + dedup + budget. |
| **peerID** | Identifiant court (8 octets) d'un nœud = début du hash de sa clé publique. Stable, pseudonyme. |
| **msgID** | Identifiant d'un paquet = hash de son contenu. Sert à dédupliquer et à suivre. |
| **msg_uuid** | Identifiant d'un **message applicatif**, stable de bout en bout (le `msgID` peut changer si le paquet est re-scellé). |
| **Noise (XX / X)** | Cadre standard pour établir un canal chiffré. `XX` = session bidirectionnelle authentifiée ; `X` = message scellé à sens unique vers une clé connue. |
| **Forward secrecy** | Propriété : voler une clé aujourd'hui ne permet pas de déchiffrer les messages d'hier. |
| **Ed25519 / X25519** | Ed25519 = signatures (prouver qui parle). X25519 = accord de clés (établir un secret partagé). Même courbe (Curve25519), usages différents. |
| **Enveloppe scellée** | Message chiffré pour un destinataire absent, déposé sur des relais en attendant qu'il revienne. |
| **recipient_tag** | Étiquette anonyme et **tournante** (change chaque jour) qui désigne le destinataire d'une enveloppe sans révéler qui c'est. |
| **Budget de copies** | Nombre max d'exemplaires d'une enveloppe qu'on laisse circuler (inspiré de *Spray-and-Wait*). |
| **TOFU** | *Trust On First Use* : on fait confiance à la première clé vue pour un contact, et on alerte si elle change. |
| **Code de vérification / safety number** | Suite de chiffres identique des deux côtés, à comparer hors bande, pour détecter un intercepteur au premier contact. |
| **Journal chaîné (hash-chain)** | Liste d'événements où chaque entrée contient le hash de la précédente : impossible d'en modifier une sans casser la suite. |
| **Attestation (`LOG_ATTEST`)** | Un nœud diffuse le « résumé » (racine) de son journal ; d'autres le rapportent au dashboard, ce qui rend le mensonge détectable. |
| **Fork (de journal)** | Un nœud présente deux historiques différents pour la même hauteur → signe de triche. |
| **Statuts** | `en attente` → `parti` → `distribué` → `lu` (+ `échec/expiré`). Voir [`docs/powl/05-message-lifecycle.md`](../powl/05-message-lifecycle.md). |
| **Outbox** | File locale des messages envoyés mais pas encore confirmés distribués ; rejouée à chaque reconnexion. |
| **ACK / read-receipt** | Accusés signés : « reçu par l'appareil » / « ouvert par l'utilisateur ». |
| **Relais / `dengon-relay`** | Nœud fixe (ESP32) branché au secteur : densifie le maillage, met en cache, dépose des enveloppes, remonte des logs. Ne déchiffre rien. |
| **`dengon-core`** | Bibliothèque Rust qui contient toute la logique (protocole, crypto, stockage, synchro, journal). Partagée par l'app, le nœud CLI et le firmware. |
| **`dengon-node`** | Nœud sans interface, en ligne de commande : sert aux tests et de nœud fixe. |
| **Transport (trait)** | Interface qui cache la radio : `dengon-core` envoie/reçoit des octets sans savoir si c'est Android, un PC ou un ESP32 derrière. |
| **UniFFI** | Outil qui génère automatiquement le « pont » pour appeler du Rust depuis Kotlin (ou Swift). |
| **Observabilité** | Capacité à comprendre ce que fait le système de l'extérieur, via des logs/traces. Ici : le dashboard. |
| **Dashboard / VPS** | Serveur qui **observe** le réseau (parcours des messages, santé). Ne transporte aucun message, ne voit aucun contenu. |
| **MQTT** | Protocole léger de publication/abonnement, utilisé par les relais pour envoyer leurs logs au dashboard. |
| **TimescaleDB** | Extension PostgreSQL optimisée pour les données horodatées (le flux d'événements). |
| **ESP-IDF / NimBLE** | ESP-IDF = SDK officiel de l'ESP32. NimBLE = pile Bluetooth légère utilisée dans le firmware. |
| **PSRAM** | Mémoire vive supplémentaire de certains ESP32 (WROVER) ; nécessaire pour nos tampons. |
| **DoR** (*Definition of Ready*) | Les 8 conditions pour qu'une issue entre dans un sprint (§6) : livrable nommé, critères vérifiables, référence documentaire, dépendances fermées, contrat disponible, estimation, stratégie de test, contrainte dure. |
| **DoD** (*Definition of Done*) | Les 8 conditions pour qu'une PR soit fusionnable (§7.1), plus des ajouts par type d'US (§7.2). |
| **Issue form** | Formulaire d'issue GitHub décrit en YAML (champs typés, certains obligatoires), par opposition au template Markdown qu'on peut soumettre vide. |
| **`CODEOWNERS`** | Fichier qui associe des chemins à des relecteurs. Sollicite automatiquement la bonne personne, et interdit à l'auteur d'approuver sa propre PR — c'est ce qui rend la revue croisée mécanique. |
| **Revue croisée** | Règle du projet : le relecteur d'une PR n'est jamais de la même `area:` que l'auteur (§10.3). Sert autant l'apprentissage que la qualité. |
| **Doublure** | Deuxième personne nommée sur une US : celle qui **consommera son artefact au sprint suivant**, donc celle à qui la relecture sert vraiment (§13). |
| **Protection de branche** | Réglage GitHub qui interdit de pousser directement sur `main` : PR obligatoire, approbations, checks verts, historique linéaire. |
| **Check requis** (*required status check*) | Job de CI dont le succès conditionne le merge. Identifié par le nom du **job**, pas du workflow. Piège : un check jamais rapporté bloque la PR indéfiniment, il n'échoue pas. |
| **Historique linéaire** | Interdiction des commits de fusion sur `main` : chaque PR y entre comme un seul commit (squash), l'historique se lit comme une liste. |
| **Ruleset** | Forme moderne de la protection de branche chez GitHub, cumulable et applicable à plusieurs branches. Non utilisée ici : la protection classique suffit à trois. |
| **`workflow_dispatch`** | Déclencheur manuel d'un workflow GitHub Actions. N'apparaît que si le fichier est présent sur la branche par défaut. |
| **Épinglage par SHA** | Référencer une action tierce par le hash complet de son commit plutôt que par un tag. Un tag est mutable : son auteur peut le repointer vers du code arbitraire, qui s'exécuterait dans notre CI. |

## Outillage Rust et CI (ajouté 2026-09-09, US-104)

| Terme | Définition courte |
|---|---|
| **Workspace Cargo** | Un seul projet Rust regroupant plusieurs bibliothèques et programmes (« crates »), qui partagent une configuration et un fichier de verrouillage communs. Ici : les six `dengon-*`. |
| **Crate** | Unité de compilation Rust : soit une bibliothèque, soit un exécutable. |
| **`crate-type`** | Forme sous laquelle une bibliothèque est produite : `lib` (utilisable par du Rust), `cdylib` (bibliothèque partagée chargeable depuis Kotlin/C), `staticlib` (à lier dans un firmware). |
| **`no_std`** | Mode de compilation Rust sans la bibliothèque standard, pour les microcontrôleurs qui n'ont ni système de fichiers ni allocateur par défaut. `no_std + alloc` autorise quand même les `Vec` et les `String`. |
| **MSRV** (*minimum supported Rust version*) | La plus ancienne version de Rust avec laquelle le projet accepte de compiler. C'est un plancher, à ne pas confondre avec la version exacte utilisée au quotidien. |
| **`rustfmt`** | Formateur automatique de code Rust. `cargo fmt --check` échoue si le code n'est pas formaté : ça supprime les débats de style en revue. |
| **`clippy`** | Analyseur qui repère les tournures suspectes ou maladroites. `-D warnings` transforme chaque avertissement en erreur, donc en échec de CI. |
| **Lint** | Règle d'analyse statique. Elle peut être en `allow`, `warn`, `deny` (erreur, contournable localement) ou `forbid` (erreur, incontournable). |
| **`cargo-nextest`** | Lanceur de tests plus rapide que `cargo test`, qui isole chaque test dans son propre processus. N'exécute pas les doctests. |
| **`cargo-llvm-cov`** | Mesure de la couverture de tests : quel pourcentage des lignes de code est réellement exécuté par la suite de tests. |
| **`Cargo.lock`** | Fichier qui fige la version exacte de chaque dépendance. Versionné ici, pour que tout le monde et la CI compilent strictement la même chose. |
| **Doctest** | Exemple de code écrit dans un commentaire de documentation, et exécuté comme un test. Garantit que la doc ne ment pas. |
