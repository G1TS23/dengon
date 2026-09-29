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
| **littlefs** | Système de fichiers pour flash, résistant aux coupures (copie sur écriture). Porte le journal chaîné du relais ESP32 (`/lfs/ledger.bin`, US-308). |
| **Curseur (du journal)** | `seq` de la prochaine entrée + hash de la dernière, gardé en NVS. Permet au relais de reprendre sa chaîne après un redémarrage sans relire le fichier. Synonyme d'**ancre** (`ledger::Anchor`). |
| **NVS** | *Non-Volatile Storage* d'ESP-IDF : petit magasin clé → valeur en flash. Le relais y garde ses secrets et le curseur du journal. |
| **Sans-IO** (*sans-IO*) | Code qui décide sans faire lui-même d'entrée/sortie : l'appelant lui passe l'heure, les paquets, la graine, et exécute ses décisions. Cas de `sync::routing`. |
| **Anti-inondation** | Plafond de nouveaux `msgID` acceptés par voisin et par minute (`FLOOD_MAX_PER_MIN_PEER = 20`) : un voisin qui inonde est ignoré jusqu'à ce que son débit retombe. |
| **Clamp de densité** | Avec 6 voisins ou plus, le TTL relayé est plafonné à 5 : en zone dense, pas besoin d'aller loin. |
| **Tempête de diffusion** (*broadcast storm*) | Saturation d'un réseau quand chaque nœud rediffuse tout ; combattue par le jitter + l'abandon sur doublons. |
| **peerID** | Identifiant court (8 octets) d'un nœud = début du hash de sa clé publique. Stable, pseudonyme. |
| **msgID** | Identifiant d'un paquet = hash de son contenu. Sert à dédupliquer et à suivre. |
| **msg_uuid** | Identifiant d'un **message applicatif**, stable de bout en bout (le `msgID` peut changer si le paquet est re-scellé). |
| **msg_log_id** | `hex(SHA-256(msgID)[0..16])` : version **hachée et raccourcie** du `msgID`, utilisée uniquement côté dashboard/journal — anti-corrélation, jamais le `msgID` ni le `msg_uuid` en clair. |
| **node_id** (dashboard) | Identifiant **pseudonyme stable** d'un nœud côté dashboard (`relay-3f2a9c`, `client-9c1d40`), différent du code anonyme *par message* d'`docs/olivier/dashboard.md` §6 — les deux modèles coexistent dans la conception, `docs/synthese/` retient le `node_id` stable pour permettre la vérification du journal chaîné par appareil (A-7). |
| **Noise (XX / X)** | Cadre standard pour établir un canal chiffré. `XX` = session bidirectionnelle authentifiée ; `X` = message scellé à sens unique vers une clé connue. |
| **Forward secrecy** | Propriété : voler une clé aujourd'hui ne permet pas de déchiffrer les messages d'hier. |
| **Ed25519 / X25519** | Ed25519 = signatures (prouver qui parle). X25519 = accord de clés (établir un secret partagé). Même courbe (Curve25519), usages différents. |
| **Enveloppe scellée** | Message chiffré pour un destinataire absent, déposé sur des relais en attendant qu'il revienne. |
| **recipient_tag** | Étiquette anonyme et **tournante** (change chaque jour) qui désigne le destinataire d'une enveloppe sans révéler qui c'est. |
| **Courrier (*courier*)** | Rôle d'un nœud qui garde des enveloppes scellées pour d'autres et les remet à la rencontre, sans pouvoir les lire (`sync::courier`). |
| **`ENVELOPE_OFFER` / `ENVELOPE_REQUEST`** | Échange à la rencontre : le porteur annonce les `recipient_tag` qu'il détient, le pair demande ceux qui sont les siens. |
| **Budget de copies** | Nombre max d'exemplaires d'une enveloppe qu'on laisse circuler (inspiré de *Spray-and-Wait*). |
| **TOFU** | *Trust On First Use* : on fait confiance à la première clé vue pour un contact, et on alerte si elle change. |
| **Code de vérification / safety number** | Suite de chiffres identique des deux côtés, à comparer hors bande, pour détecter un intercepteur au premier contact. |
| **Empreinte / fingerprint** | `SHA-256(pub_static ‖ pub_sign)` : 32 octets qui résument les deux clés publiques d'un nœud. Le code de vérification se calcule à partir des empreintes des deux correspondants (`identity::keys`). |
| **Coffre / vault** | Endroit où l'identité (les clés privées) est rangée **chiffrée au repos**. Côté cœur : un trait `Vault` qui range un blob XChaCha20-Poly1305 ; la clé vient de l'appelant (à terme Android Keystore / Secret Service). |
| **Carte de contact (`PublicIdentity`)** | Pseudo + deux clés publiques d'un nœud, sans secret : ce que contient le QR `dengon:v1:…`. |
| **AAD** (*associated data*) | Données passées à un chiffrement authentifié qui ne sont **pas chiffrées** mais **sont authentifiées** : les modifier fait échouer le déchiffrement. |
| **Journal chaîné (hash-chain)** | Liste d'événements où chaque entrée contient le hash de la précédente : impossible d'en modifier une sans casser la suite. |
| **Attestation (`LOG_ATTEST`)** | Un nœud diffuse le « résumé » (racine) de son journal ; d'autres le rapportent au dashboard, ce qui rend le mensonge détectable. |
| **Fork (de journal)** | Un nœud présente deux historiques différents pour la même hauteur → signe de triche. |
| **Statuts** | `en attente` → `parti` → `distribué` → `lu` (+ `échec/expiré`). Voir [`docs/powl/05-message-lifecycle.md`](../powl/05-message-lifecycle.md). |
| **Outbox** | File locale des messages envoyés mais pas encore confirmés distribués ; rejouée à chaque reconnexion. |
| **Statut terminal** | Statut qui n'évolue plus : `delivered`, `expired`, `cancelled`. Le message sort alors de l'outbox. |
| **Property test** | Test qui vérifie une propriété générale (« le statut ne redescend jamais ») sur des centaines d'entrées générées aléatoirement, au lieu de quelques exemples écrits à la main. En Rust : crate `proptest`. |
| **Lecture croisée** | Vérification d'un contact où chaque téléphone scanne le QR de l'autre, puis les deux personnes comparent à voix haute le code de 60 chiffres affiché des deux côtés. |
| **Masque (QR)** | Motif appliqué aux modules d'un QR code (8 possibles) pour éviter les zones qui perturbent la lecture ; le choix du masque change le dessin, pas le contenu. |
| **ACK / read-receipt** | Accusés signés : « reçu par l'appareil » / « ouvert par l'utilisateur ». |
| **Relais / `dengon-relay`** | Nœud fixe (ESP32) branché au secteur : densifie le maillage, met en cache, dépose des enveloppes, remonte des logs. Ne déchiffre rien. |
| **`btleplug::api::Peripheral`** (piège de nommage) | Dans `btleplug`, ce trait désigne l'appareil **distant** trouvé en scannant (le serveur GATT d'en face), **pas** « notre rôle peripheral » : `btleplug` ne sait tenir que le rôle central (voir *Rôle central / peripheral* plus bas et B-6) — [`suivi/spikes/US-102-btleplug-peripheral.md`](spikes/US-102-btleplug-peripheral.md). |
| **BlueZ** | Pile Bluetooth officielle de Linux (démon `bluetoothd`), pilotable via D-Bus. |
| **D-Bus** | Bus de communication inter-processus standard sous Linux ; BlueZ y expose toute son API (scan, connexion, GATT, annonce). |
| **`bluer`** | Bindings Rust officiels du projet BlueZ, au-dessus de D-Bus. Contrairement à `btleplug`, couvre le rôle peripheral (GATT server + annonce) — mais Linux uniquement. |
| **`dengon-core`** | Bibliothèque Rust qui contient toute la logique (protocole, crypto, stockage, synchro, journal). Partagée par l'app, le nœud CLI et le firmware. |
| **`dengon-node`** | Nœud sans interface, en ligne de commande : sert aux tests et de nœud fixe. |
| **Transport (trait)** | Interface qui cache la radio : `dengon-core` envoie/reçoit des octets sans savoir si c'est Android, un PC ou un ESP32 derrière. |
| **UniFFI** | Outil qui génère automatiquement le « pont » pour appeler du Rust depuis Kotlin (ou Swift). |
| **UDL** (*UniFFI Definition Language*) | Fichier (`.udl`) qui décrit le contrat FFI d'UniFFI : types, fonctions, interfaces exposées — indépendant du langage hôte. `build.rs` le lit pour générer le code Rust de pont (*scaffolding*) ; `uniffi-bindgen` (séparé) le lit pour générer les classes Kotlin/Swift. |
| **Scaffolding (UniFFI)** | Code Rust généré depuis le `.udl` (fonctions `extern "C"`, conversions) qui relie les types Rust ordinaires au runtime UniFFI. Ne génère **pas** les bindings Kotlin — ça, c'est `uniffi-bindgen generate`, une étape séparée. |
| **Observabilité** | Capacité à comprendre ce que fait le système de l'extérieur, via des logs/traces. Ici : le dashboard. |
| **Dashboard / VPS** | Serveur qui **observe** le réseau (parcours des messages, santé). Ne transporte aucun message, ne voit aucun contenu. |
| **MQTT** | Protocole léger de publication/abonnement, utilisé par les relais pour envoyer leurs logs au dashboard. |
| **TimescaleDB** | Extension PostgreSQL optimisée pour les données horodatées (le flux d'événements). |
| **ESP-IDF / NimBLE** | ESP-IDF = SDK officiel de l'ESP32. NimBLE = pile Bluetooth légère utilisée dans le firmware. |
| **PSRAM** | Mémoire vive supplémentaire de certains ESP32 (WROVER) ; nécessaire pour nos tampons. |
| **GAP / GATT** | Les deux moitiés du BLE. GAP = se faire voir et se connecter (annonce, découverte). GATT = la structure des données une fois connecté (services et caractéristiques). |
| **Advertising** (annonce) | Petit paquet radio qu'un appareil BLE émet en boucle pour signaler son existence. Limité à **31 octets**, ce qui contraint tout ce qu'on peut y mettre. |
| **Scan response** (réponse de scan) | Second paquet de 31 octets, envoyé seulement si un scanner le demande. Sert de rallonge à l'annonce : c'est là qu'on met le nom du relais, faute de place. |
| **Manufacturer data** | Champ libre d'une annonce BLE, réservé aux données propres au constructeur. Doit commencer par un identifiant de fabricant sur 2 octets ; dengon utilise `0xFFFF`, réservé aux tests. |
| **Caractéristique** (*characteristic*) | Une valeur exposée par un service GATT, avec ses permissions. Le service `dengon` en a deux : `RX` (on écrit dedans) et `TX` (elle notifie). |
| **ATT MTU** | Taille maximale d'un message GATT. Négociée à la connexion : 517 octets visés, 23 au pire. En dessous, il faut fragmenter. |
| **CCCD** (`0x2902`) | Petit interrupteur attaché à une caractéristique notifiable : c'est en l'écrivant qu'un pair s'abonne aux notifications. NimBLE l'ajoute tout seul. |
| **`conn_handle`** | Numéro qu'une pile BLE donne à une connexion ouverte. **Recyclé** dès la fermeture — à ne jamais confondre avec le `LinkId` du contrat `Transport`, qui ne l'est jamais. |
| **`LinkId`** | Identifiant d'un **lien** (une connexion) dans le contrat `Transport` : local au processus, monotone, jamais réutilisé. Ce n'est pas un `peerID`. |
| **Supervision timeout** | Délai de silence radio au-delà duquel une connexion BLE est déclarée morte (code HCI `0x08`). C'est ainsi qu'on détecte une **coupure brutale** : le pair n'envoie rien, on constate son absence. |
| **Règle anti-boucle** | Quand deux nœuds se découvrent, seul celui au plus petit `peerID` initie la connexion — sinon chacun se connecte à l'autre et un lien est gâché. |
| **Cible `linux` (ESP-IDF)** | Mode de compilation d'ESP-IDF qui produit un programme pour le PC au lieu de la carte : sert à exécuter des tests Unity de code sans matériel, en CI. |
| **`sdkconfig` / `sdkconfig.defaults`** | Configuration d'un projet ESP-IDF. Les `defaults` sont écrits à la main et versionnés ; `sdkconfig` en est **généré une seule fois**, et c'est lui que lit la compilation. |
| **usbipd** | Passerelle qui expose un périphérique USB de Windows à WSL2. Sans elle, aucune carte ESP32 n'est visible depuis Linux — donc pas de flash. |
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
| **`no_std`** | Mode de compilation Rust sans bibliothèque standard : pas d'OS, donc ni fichiers, ni threads, ni `/dev/urandom`. Restent `core` et, si on fournit un allocateur, `alloc` (`Vec`, `Box`). C'est le mode du code embarqué sur l'ESP32. |
| **Xtensa** | L'architecture du processeur de l'ESP32 (à ne pas confondre avec l'ESP32-C3, qui est en RISC-V). Cible Rust : `xtensa-esp32-none-elf`, où `none` veut dire « pas de système d'exploitation ». |
| **`espup`** | Installeur officiel de la chaîne d'outils Rust pour Espressif. Nécessaire car Xtensa n'est pas supportée par le Rust amont : il installe un **fork** du compilateur (toolchain `esp`, ~1,9 Go). |
| **Cible tier 3** | Cible que Rust sait viser mais pour laquelle personne ne distribue de `core`/`alloc` précompilés : il faut les recompiler à la volée (`-Z build-std`), ce qui impose une toolchain *nightly*. |
| **`staticlib`** | Format de sortie Rust produisant une archive `.a` de code objet, destinée à être **liée dans un programme C**. C'est sous cette forme que `dengon-core` entrera dans le firmware ESP-IDF. |
| **Features additives (Cargo)** | Règle de Cargo : les *features* demandées par toutes les dépendances s'**additionnent**, jamais l'inverse. Une crate qui exige `std` chez l'une de ses dépendances ne peut donc pas être ramenée en `no_std` de l'extérieur. |
| **Spike** | Tâche de recherche **timeboxée** dont le livrable est une **décision écrite** (oui/non), pas du code. Le code d'essai est explicitement jeté. Voir [`spikes/`](spikes/). |
| **Foreground service (Android)** | Service Android « premier plan » : doit afficher une notification permanente et déclarer un `foregroundServiceType` (ex. `connectedDevice`) pour survivre écran éteint sans être tué par le système. |
| **`START_STICKY`** | Valeur de retour d'`onStartCommand` qui demande à Android de relancer le service (sans son intent d'origine) s'il a dû être tué. |
| **GATT** (*Generic Attribute Profile*) | Couche BLE qui structure les données échangées en **services** (regroupements) et **characteristics** (valeurs lisibles/écrivables/notifiables à l'intérieur d'un service). |
| **Rôle central / peripheral (BLE)** | *Peripheral* : annonce sa présence et publie un service GATT (le « serveur »). *Central* : scanne, trouve, se connecte (le « client »). Un nœud `dengon` tient les **deux** rôles en permanence. |
| **ATT_MTU** | Taille max d'un paquet BLE au niveau attribut (23 o par défaut, jusqu'à 517 si négocié à la connexion). Dimensionne la fragmentation protocole (`FRAG_SIZE`) — mesuré réellement par le Spike C (US-103). |
| **Fragment / réassemblage** | Morceau d'un paquet trop grand pour une écriture BLE (paquet L3 de type `0x09`) ; le réassembleur recolle les morceaux à l'arrivée, dans n'importe quel ordre. |
| **frag_id** | `SHA-256(paquet complet)[0..8]` : identifie les fragments d'un même paquet et sert à vérifier le paquet reconstruit. |
| **Shrinking** | Étape d'un property test qui réduit une entrée en échec au plus petit contre-exemple, pour rendre le bug lisible. |
| **Spike** | Tâche courte et bornée dans le temps (*timebox*) pour répondre à une question technique par l'expérimentation plutôt que par la lecture. Livrable = une décision écrite + des chiffres, pas une fonctionnalité ; le code produit est jetable. |
| **KAT (Known Answer Test)** | Test qui compare la sortie d'un algorithme crypto à un résultat publié par les auteurs de la norme (ici RFC 8032 §7.1). Prouve qu'on implémente bien *le* standard, pas une variante. |
| **Ed25519 déterministe** | Ed25519 ne tire aucun nombre aléatoire à la signature : le même message et la même clé donnent toujours la même signature. Rend les tests reproductibles et supprime toute une classe de failles liées à un mauvais RNG. |
| **Clé de faible ordre** | Point de la courbe qui engendre un tout petit sous-groupe (ex. le point neutre). Sert à forger des signatures « qui passent » sans secret. Refusée par `verify_strict`. |
| **Signature malléable** | Signature valide qu'on peut transformer en une autre signature valide du même message. Refusée par `verify_strict`. |
| **Graine (seed) / déterminisme** | Valeur de départ du générateur pseudo-aléatoire. Même graine → mêmes tirages → même exécution. C'est ce qui rend un scénario de `dengon-sim` rejouable à l'identique (US-221). |
| **Empreinte de trace** | Hachage (FNV-1a 64 bits) de toute la trace d'une simulation. Deux exécutions à même graine doivent avoir la même empreinte : c'est ce que compare le job CI `sim`. |
| **Property test** | Test qui vérifie une **propriété** (ex. « décoder(encoder(p)) = p ») sur des centaines d'entrées générées au hasard, plutôt que sur quelques exemples écrits à la main. En cas d'échec, l'outil (`proptest`) réduit l'entrée au plus petit contre-exemple. Utilisé pour le codec (US-201). |
| **Bucket de padding (`PAD_BUCKETS`)** | Tailles fixes (256, 512, 1024, 2048 o) auxquelles tout clair est complété avant chiffrement : deux messages de longueurs différentes dans le même bucket sont indistinguables par la taille. |
| **epoch_day** | Numéro du jour depuis le 1er janvier 1970 (2 octets sur le fil) ; paramètre du `recipient_tag`, qui change donc chaque jour. |
| **Clé éphémère** | Clé X25519 jetable tirée pour un seul handshake Noise ; c'est elle qui apporte la *forward secrecy*. |
| **CryptoResolver** | Dans `snow`, l'objet qui fournit les primitives (DH, hash, chiffrement, aléa). Le nôtre injecte l'aléa de l'appelant. |
| **Vecteur de conformité** | Entrée/sortie figée (octets exacts) qu'une implémentation doit reproduire ; sert de test de non-régression et d'interopérabilité entre plateformes. |
| **Fenêtre anti-rejeu** | Mémoire des N derniers numéros de message reçus (ici 64) : un message déjà vu ou trop ancien est refusé, un message en retard mais récent est accepté. |
| **Nonce** | Numéro à usage unique qui accompagne chaque chiffrement ; ne doit jamais se répéter avec la même clé. Dans une session dengon, c'est un compteur envoyé en clair devant le chiffré. |
| **ViewModel / StateFlow** | Android : le `ViewModel` garde l'état d'un écran et survit aux rotations ; l'écran observe un `StateFlow` (valeur courante + notifications de changement) et appelle les actions du ViewModel. La messagerie (US-214) en a un seul : `ConversationsViewModel`. |
| **`cargo audit`** | Compare les versions de `Cargo.lock` à la base d'avis **RUSTSEC** et signale les dépendances vulnérables ou non maintenues. |
| **`cargo deny`** | Contrôle quatre choses depuis `Cargo.lock` : avis de sécurité, licences autorisées, doublons de version / dépendances en `"*"`, et provenance des crates. Configuré par `deny.toml`. |
| **RUSTSEC** | Identifiant d'un avis de la *RustSec Advisory Database* (`RUSTSEC-AAAA-NNNN`). Couvre les vulnérabilités **et** les crates abandonnées. |
| **SBOM** | *Software Bill of Materials* — inventaire machine des composants d'un logiciel et de leurs versions, pour retrouver vite qui est touché par une faille. Prévu par `synthese/10` §4.7, **pas encore livré**. |
| **Unification de features (Cargo)** | Cargo compile une dépendance **une seule fois** par graphe, avec l'**union** des features demandées par tous ceux qui en dépendent. Conséquence : `--no-default-features` sur un paquet ne garantit pas que la dépendance soit compilée sans ses features par défaut. |
| **Proxy `no_std`** | Ici : `crates/dengon-conformance/`, crate sans code qui lie `dengon-core` sans `std` pour rejouer les vecteurs dans la configuration que le firmware ESP32 embarquera — en attendant que l'US-307 branche le vrai pont `dengon_core_ffi`. |
| **Inventaire (`INVENTORY`)** | Paquet `0x0D` : la liste des `msgID` qu'un nœud détient, envoyée à un voisin qui arrive. Le voisin répond en poussant ce qui manque. Remplace le gossip GCS au MVP. |
| **Cache de réconciliation** | Paquets récents qu'un nœud porte pour d'autres (octets bruts, 120 max, 6 h), annoncés dans son inventaire et poussés aux voisins qui ne les ont pas. |
| **Push cadencé** | Envoi du manquant limité à 15 paquets/min par voisin, pour rester sous son anti-inondation (20/min). |
| **cbindgen** | Outil Rust qui génère un header C (`.h`) à partir des fonctions `#[no_mangle] extern "C"` d'une crate, en reparsant son code source (pas de compilation). Utilisé par `dengon-core-ffi` pour `include/dengon_core.h` (US-307). |
| **`-Z build-std`** | Option nightly de Cargo qui recompile `core`/`alloc` (et `std` si demandé) depuis les sources, au lieu d'utiliser le sysroot précompilé par rustup. Indispensable pour une cible tier 3 sans std précompilée (`xtensa-esp32-none-elf`, Spike A) ou pour changer une propriété du sysroot lui-même, comme `panic = "abort"` (US-307). |
| **Archive `staticlib` autonome** | Un `.a` Rust `no_std` lié dans un exécutable où aucun autre code Rust ne fournit l'allocateur global ni le panic handler (ex. un programme C, ou le firmware ESP-IDF) : la crate qui produit l'archive doit les fournir elle-même, contrairement à un `rlib` normal qui les délègue au binaire final. `dengon-core-embed` (US-307). |
| **JNA** | *Java Native Access* — bibliothèque Java qui appelle une bibliothèque native (`.so`) sans écrire de code JNI : on déclare les fonctions dans une interface Kotlin/Java, JNA fait le reste à l'exécution. C'est par elle que les bindings UniFFI appellent `libdengon_ffi.so` (US-302). |
| **cargo-ndk** | Sous-commande Cargo qui compile une crate Rust pour les ABI Android (`arm64-v8a`, `x86_64`…) avec le bon compilateur du NDK, et range les `.so` dans l'arborescence `jniLibs/<abi>/` attendue par Gradle. |
| **ABI (Android)** | Famille de processeur visée par une bibliothèque native : `arm64-v8a` (quasi tous les téléphones récents), `armeabi-v7a` (anciens), `x86_64` (émulateur). Un APK doit fournir chaque `.so` pour chaque ABI qu'il déclare. |
| **Coffre d'identité** | Fichier `identity.vault` : les clés privées de l'appareil, chiffrées XChaCha20-Poly1305 par `dengon-core` (US-205). Sur Android, sa clé est elle-même chiffrée par une clé du Keystore (`CleCoffre`, US-302). |
| **ANNOUNCE (de lien)** | Paquet `0x01`, signé : `peerID`, clés publiques, pseudo, hauteur de journal. À l'US-306, chaque côté l'écrit en première trame d'un lien BLE qui s'ouvre ; l'autre le vérifie pour savoir quel `peerID` est au bout du lien (le transport ne connaît que des `LinkId`). |
| **Maillage (classe Android)** | `ble/Maillage.kt` (US-306) : le pont entre le transport BLE et le nœud Rust — relie chaque lien à un `peerID` par l'ANNOUNCE, pousse les trames reçues au nœud, écrit sur la radio ce qu'il produit. À ne pas confondre avec le maillage au sens réseau. |
| **Accusé (Ack) de réception** | `AppFrame::Ack{Delivered}` renvoyé par le destinataire dans la session Noise ; c'est lui qui fait passer un message envoyé de « parti » (`InFlight`) à « distribué » (`Delivered`), US-306. |
