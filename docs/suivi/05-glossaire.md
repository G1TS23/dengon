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
