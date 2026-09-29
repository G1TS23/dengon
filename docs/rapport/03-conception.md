# Conception

> Section du [rapport](README.md) — voir le [plan détaillé](00-plan.md).
> Sources de conception : [`docs/synthese/04-architecture.md`](../synthese/04-architecture.md)
> à [`09-dashboard-et-donnees.md`](../synthese/09-dashboard-et-donnees.md). Le
> détail exhaustif (constantes, formats binaires octet par octet, schémas de
> base de données) reste dans ces fichiers ; cette section en donne la
> synthèse nécessaire à la compréhension du système.

## Architecture générale

Le principe directeur de l'architecture est qu'il n'existe **qu'une seule
implémentation du protocole**, écrite en Rust dans une bibliothèque partagée,
`dengon-core`. Cette bibliothèque contient tout sauf l'entrée-sortie radio et
l'entrée-sortie réseau : elle produit et consomme des octets, transportés par
une abstraction commune, et ne connaît ni le détail d'une puce Bluetooth ni
celui d'une connexion HTTP. Trois exécutables radicalement différents —
l'application Android, un nœud en ligne de commande utile pour les tests, et le
firmware de la carte ESP32 — partagent ce même cœur, ce qui garantit qu'ils
appliquent tous exactement les mêmes règles de routage, de sécurité et de
format de message, et qu'un audit de sécurité n'a besoin d'être mené qu'une
seule fois.

`dengon-core` est découpé en modules aux responsabilités bien séparées : la
sérialisation binaire des paquets (`protocol`), la cryptographie — signatures
Ed25519, sessions chiffrées Noise, scellement d'enveloppes (`crypto`) —,
la gestion de l'identité d'un appareil (`identity`), la persistance locale
chiffrée (`store`), la logique de routage et de réconciliation entre pairs
(`sync`), le journal chaîné et signé (`ledger`), l'émission d'événements
d'observabilité normalisés (`observability`), et enfin une façade publique
(`api`) qui expose aux applications les opérations de haut niveau — envoyer un
message, consulter les événements en attente, réagir à la connexion d'un pair.
Les modules `protocol`, `sync` et `ledger` sont écrits pour pouvoir compiler
dans un environnement sans système d'exploitation complet (contrainte
`no_std` + allocation dynamique), condition nécessaire pour tourner sur la
carte ESP32 dont les ressources sont très limitées.

La seule couture entre ce cœur commun et chaque plateforme est un trait
`Transport`, une interface minimale à quatre opérations : démarrer l'annonce et
l'écoute du service Bluetooth, récupérer les événements survenus (connexion,
déconnexion, trame reçue), envoyer une trame à un pair connecté, et diffuser une
trame à tous les pairs connectés. Chaque plateforme fournit sa propre
implémentation de ce trait — Android s'appuie sur les API Bluetooth natives
dans un service de premier plan, la carte ESP32 sur la pile NimBLE de
l'ESP-IDF, un poste de travail sur une bibliothèque Bluetooth générique. Le
protocole, la cryptographie, le stockage et le journal restent, eux,
strictement identiques partout. Cette séparation est aussi ce qui rendra une
future version iOS peu coûteuse : il suffira d'écrire une implémentation du
trait `Transport` fondée sur CoreBluetooth, sans toucher au cœur du projet.

Sur le plan du déploiement, l'ensemble des composants qui opèrent sur le
terrain — l'application, le nœud en ligne de commande, le relais — fonctionnent
entièrement hors ligne. Seul le tableau de bord, hébergé sur un serveur de
l'équipe, nécessite une connexion réseau classique ; il ne reçoit des données
que dans un sens (les relais lui envoient des lots d'événements par HTTPS), et
sa disparition n'a **aucun impact** sur le fonctionnement du réseau de
messagerie lui-même — c'est un principe directeur du projet : le tableau de
bord observe, il n'agit jamais.

## Protocole réseau et format de trame

Le protocole réseau de dengon s'organise en quatre couches, de la radio vers
l'application : la couche BLE elle-même (**L1**), qui transporte des trames
opaques entre deux appareils ; une couche de fragmentation (**L2**), qui
découpe un paquet trop grand pour tenir dans une seule trame BLE et le
réassemble à l'arrivée ; l'**enveloppe dengon** (**L3**), un paquet signé qui
porte le routage, le compteur de sauts et la déduplication ; et enfin le
contenu applicatif (**L4**) — le texte d'un message ou un accusé de réception —
qui circule chiffré à l'intérieur de la couche L3.

Un paquet L3 est composé d'un en-tête binaire compact (22 octets pour un
paquet diffusé, 30 pour un paquet adressé à un destinataire précis, auxquels
s'ajoutent 64 octets si le paquet est signé) suivi d'une charge utile dont la
forme dépend du type de paquet. L'en-tête porte notamment un numéro de version
du protocole, un compteur de sauts restants (*TTL*, initialisé à 7), un
horodatage, l'identifiant de l'émetteur, et éventuellement celui du
destinataire. L'identifiant d'un appareil (`peerID`) tient sur 8 octets et se
calcule à partir de sa clé publique — il est donc à la fois stable et
pseudonyme, sans jamais révéler d'information sur l'identité réelle de son
porteur.

Une dizaine de types de paquets couvrent les besoins du MVP : l'annonce d'un
appareil à ses voisins, les trois messages de l'échange de clés Noise, un
message chiffré en session, une enveloppe scellée pour un destinataire hors de
portée, un accusé de réception, un échange d'inventaire des messages déjà
détenus entre deux voisins, une attestation de journal, et l'offre puis la
demande d'une enveloppe déposée par un tiers. Deux identifiants distincts
accompagnent chaque message applicatif : un identifiant de **paquet**, dérivé
d'un hachage de son contenu, qui sert à la déduplication au niveau du réseau ;
et un identifiant de **message**, un UUID tiré une fois pour toutes par
l'expéditeur, stable de bout en bout, qui sert au suivi du statut d'un message
même si son enveloppe est re-scellée en cours de route.

Le comportement d'un nœud à la réception d'un paquet suit toujours le même
enchaînement : vérifier que le paquet n'a pas déjà été vu (grâce à une fenêtre
de déduplication), vérifier qu'il n'est pas expiré, s'assurer que l'émetteur
voisin ne dépasse pas un quota anti-inondation, réassembler les éventuels
fragments, puis déterminer si le nœud est le destinataire — auquel cas le
paquet est traité localement et un accusé de réception est produit — ou s'il
doit être relayé aux autres voisins, après un court délai aléatoire qui permet
de renoncer si un autre voisin a déjà rediffusé le même paquet entre-temps.
Cette dernière règle, dite « écouter avant de rediffuser », limite
naturellement le nombre de copies inutiles d'un même message qui circulent en
même temps sur le réseau.

Quand deux appareils se rencontrent, ils échangent en outre la liste des
identifiants de messages qu'ils détiennent chacun, puis se transmettent
mutuellement ce qui manque à l'autre — un mécanisme de réconciliation simple,
suffisant pour le volume de messages visé par le MVP, et qui pourra être
remplacé plus tard par un résumé compact plus économe en bande passante sans
changer le format des autres paquets. Un mécanisme voisin gère les enveloppes
scellées déposées pour un destinataire absent : un appareil qui en porte une
annonce le tag anonyme qui l'identifie, le voisin compare ce tag à ceux qu'il
calcule pour lui-même, et en cas de correspondance demande l'enveloppe
correspondante.

Face aux menaces classiques d'un tel réseau, plusieurs protections sont
intégrées au protocole lui-même : le rejeu de paquets est empêché par la
combinaison d'un identifiant unique, d'une fenêtre de déduplication et d'une
tolérance d'horloge limitée ; les boucles de diffusion sont évitées par le
compteur de sauts et la fenêtre d'attente avant rediffusion ; la falsification
d'un émetteur est rendue impossible par la signature systématique des paquets
sensibles, combinée à la vérification que l'identifiant annoncé correspond
réellement à la clé publique fournie ; une inondation volontaire est contenue
par un quota de paquets par voisin et par seconde ; l'épuisement de la mémoire
d'un appareil face à des fragments ou des enveloppes malveillantes est borné par
des plafonds stricts, avec une politique d'éviction explicite lorsqu'ils sont
atteints.

## Sécurité

Le modèle de menace retenu part du principe qu'aucun relais et qu'aucun
composant réseau ne doit être digne de confiance : un relais peut voir passer
tout le trafic qui le traverse, le dupliquer, le retarder ou le réordonner ;
un serveur du tableau de bord peut être compromis ou son administrateur
curieux ; un voisin Bluetooth passif peut écouter les ondes ; un attaquant
actif peut tenter de s'interposer lors du tout premier contact entre deux
utilisateurs. Face à chacune de ces menaces, dengon vise quatre propriétés
classiques — confidentialité, intégrité, authenticité et résistance au rejeu —
complétées par un effort de minimisation des métadonnées et par la
confidentialité persistante (*forward secrecy*) sur les sessions actives.
Certaines menaces restent explicitement hors du périmètre du MVP, en
particulier une analyse de trafic menée à l'échelle mondiale par un adversaire
étatique, ou la sécurité physique d'un relais dérobé — au pire, un relais
compromis peut produire de faux journaux, jamais déchiffrer un message, faute
de détenir la moindre clé d'un utilisateur.

Chaque appareil génère, dès sa première utilisation, deux paires de clés
conservées dans le coffre sécurisé de sa plateforme : une clé d'échange
(courbe X25519), utilisée pour établir des sessions chiffrées, et une clé de
signature (Ed25519), utilisée pour authentifier les paquets qui en ont besoin.
L'identifiant public d'un appareil se déduit directement de sa clé d'échange
par une fonction de hachage, ce qui le rend stable tant que ses clés ne sont
pas régénérées. Pour établir un contact de confiance, deux utilisateurs
présents physiquement l'un en face de l'autre scannent mutuellement un QR code
contenant les clés publiques de l'autre, puis comparent de vive voix un code de
vérification à soixante chiffres calculé à partir des deux identités — un
mécanisme qui protège contre une interposition au tout premier contact, sans
nécessiter le moindre serveur. En dehors de cette vérification explicite, la
confiance repose par défaut sur le principe de la **confiance à la première
rencontre** : si la clé publique annoncée pour un contact change ensuite sans
explication, l'application suspend les messages vers lui et alerte
l'utilisateur.

Le chiffrement du contenu des messages repose sur le cadre **Noise**, une
famille de protocoles de poignée de main cryptographique éprouvée. Deux motifs
de ce cadre sont employés selon la situation : le motif `XX`, un échange
interactif en trois messages qui établit une session bidirectionnelle
authentifiée avec confidentialité persistante, utilisé tant que les deux
appareils restent connectés l'un à l'autre ; et le motif `X`, un scellement en
un seul message vers la clé publique d'un destinataire absent, utilisé pour les
enveloppes déposées sur le réseau en son absence. Dans les deux cas, l'algorithme
de chiffrement authentifié sous-jacent est ChaCha20-Poly1305, et chaque paquet
sensible est en outre signé avec Ed25519. Pour éviter qu'un relais ne puisse
suivre un destinataire absent dans le temps, l'étiquette qui identifie une
enveloppe scellée change chaque jour, calculée à partir de la clé publique du
destinataire et de la date — seul le véritable destinataire peut reconnaître
ses propres enveloppes parmi celles en circulation. Enfin, tout paquet chiffré
est complété (*padding*) jusqu'à l'une de quelques tailles standard, de sorte
qu'un message court et un message plus long restent indiscernables par leur
seule taille sur le réseau.

Pour garantir la traçabilité de ce qui se passe sur le réseau sans faire
confiance au tableau de bord qui l'observe, chaque appareil tient son propre
**journal chaîné signé**, directement inspiré des structures de données d'une
blockchain sans en reprendre le mécanisme de consensus (voir la discussion
détaillée dans la section [État de l'art](02-etat-de-lart.md#blockchain--une-analyse-critique)).
Chaque entrée de ce journal contient l'empreinte cryptographique de l'entrée
précédente, ce qui rend toute altération, suppression ou réordonnancement
détectable par quiconque possède la suite du journal. Chaque appareil diffuse
périodiquement une attestation signée résumant la hauteur et la racine de son
propre journal ; le tableau de bord, en recoupant les attestations rapportées
par plusieurs relais indépendants pour un même appareil, peut détecter une
tentative de falsification ou de double historique, sans jamais avoir la
capacité de réécrire quoi que ce soit lui-même — il **constate**, il ne
**forge** pas.

Sur l'appareil lui-même, la base de données locale chiffre champ par champ les
colonnes sensibles — au premier chef le texte des messages — avec l'algorithme
XChaCha20-Poly1305, tandis que les clés privées elles-mêmes restent confiées au
coffre matériel de la plateforme (Android Keystore, ou l'équivalent sur les
autres cibles), jugé strictement supérieur à un chiffrement logiciel pour ce
usage précis. Côté relais, la clé de signature de l'appareil est elle-même
stockée dans une mémoire flash chiffrée, et un relais compromis physiquement ne
peut produire que de faux journaux — détectables par recoupement — jamais
déchiffrer un message, faute de détenir la moindre clé d'utilisateur. Côté
tableau de bord enfin, l'authentification des relais qui poussent des données
repose sur un jeton de courte durée doublé d'une signature Ed25519 de chaque
lot d'événements envoyé, et la base du tableau de bord est intégralement
effacée après chaque session de démonstration.

## Cycle de vie des messages et statuts

Un message suit, du point de vue de son expéditeur, une progression de
statuts strictement monotone : **En attente** dès sa création et son
chiffrement, avant toute remise à un pair ou un relais ; **Parti** dès qu'il a
été remis à au moins un pair ou déposé sous forme d'enveloppe scellée — ce
statut signifie que le réseau s'en occupe, pas que le destinataire l'a reçu ;
**Distribué** dès la réception d'un accusé signé émis par l'appareil
destinataire lui-même. Deux statuts terminaux complètent ce cycle : **Échec**,
si aucun accusé n'est arrivé au bout de vingt-quatre heures, et **Annulé**, si
l'expéditeur retire le message avant qu'il ne soit parti. Un statut
supplémentaire, « Lu », est prévu dans le format du protocole mais
explicitement reporté à une version ultérieure du projet.

À chaque connexion entre deux appareils, une séquence complète se déroule :
annonce mutuelle, poignée de main cryptographique si aucune session valide
n'est déjà en cache, échange d'inventaire des messages détenus, échange des
enveloppes scellées susceptibles de concerner l'un des deux appareils, puis
remise de tous les messages en attente destinés à l'autre appareil ou
relayables par son intermédiaire. C'est ce mécanisme, rejoué systématiquement à
chaque nouvelle connexion, qui rend la déconnexion **non bloquante** : ce qui
était destiné à un appareil et qu'il n'a pas encore reçu lui parvient dès qu'il
se reconnecte quelque part ; ce qu'un appareil n'a pas pu transmettre est
repoussé à la prochaine occasion ; les accusés de réception produits en son
absence finissent par le rattraper et faire progresser les statuts qu'il
suit.

Un scénario type illustre l'ensemble du mécanisme : une expéditrice envoie un
message à un destinataire hors de portée directe, mais un troisième appareil se
trouve entre les deux. L'enveloppe atteint ce relais intermédiaire, qui la
stocke sans pouvoir la lire et la retransmet dès qu'il croise le destinataire ;
ce dernier la déchiffre, l'affiche, et émet aussitôt un accusé de réception
signé et chiffré à l'attention de l'expéditrice ; cet accusé refait le chemin
inverse à travers le même relais intermédiaire, qui en profite pour retirer sa
copie du message de sa propre file d'attente, jusqu'à atteindre l'expéditrice
et faire passer le message au statut « Distribué ». Le tableau de bord, de son
côté, observe cette même séquence sous la forme d'événements successifs — remis
à un pair, relayé, livré, accusé observé — et en reconstruit le statut courant
sans jamais avoir accès au contenu échangé.

## Relais ESP32

Le relais est un composant fixe du réseau, branché en permanence sur le
secteur, qui densifie le maillage et sert de mémoire tampon. Il remplit cinq
fonctions : **relayer** les paquets Bluetooth selon le même pipeline de routage
que n'importe quel autre nœud ; **réconcilier** régulièrement son inventaire
de messages récents avec ses voisins ; **déposer** les enveloppes scellées à
l'attention de destinataires actuellement absents ; **attester** son propre
journal chaîné en le diffusant périodiquement ; et **remonter** ses journaux
d'activité au tableau de bord par lots, via une connexion Wi-Fi, dès qu'elle
est disponible. Le relais ne déchiffre jamais rien : il ne détient aucune clé
d'utilisateur et ne voit que des paquets chiffrés accompagnés de métadonnées
de passage.

Le matériel retenu est une carte **ESP32-WROOM-32E**, déjà en possession de
l'équipe, dotée d'un double cœur cadencé jusqu'à 240 MHz, de 520 kilooctets de
mémoire vive sans mémoire additionnelle, de 4 mégaoctets de mémoire flash, ainsi
que du Wi-Fi et du Bluetooth. Cette contrainte mémoire, nettement plus stricte
que sur un module doté de mémoire vive supplémentaire, impose de réduire tous
les plafonds du protocole d'un facteur d'environ huit par rapport à une cible
plus généreuse en mémoire : la taille du cache de réconciliation, le nombre
d'enveloppes scellées conservées simultanément, ou encore la fenêtre de
déduplication sont tous dimensionnés en conséquence.

Le firmware s'appuie sur l'ESP-IDF, avec la pile Bluetooth NimBLE pour le rôle
double de serveur et de client GATT, une connexion Wi-Fi en mode station
activée par fenêtres pour ne pas concurrencer en permanence le Bluetooth sur la
même antenne radio, et un client HTTPS pour transmettre les journaux au tableau
de bord. La partie du cœur commun compatible avec un environnement sans système
d'exploitation complet — le protocole, le routage, le journal chaîné, les
événements d'observabilité — est compilée en une bibliothèque statique liée au
firmware en C, ce qui garantit que le relais applique exactement les mêmes
règles que l'application mobile. Un ensemble de tâches concurrentes se répartit
le travail : réception des trames Bluetooth, routage, réconciliation
d'inventaire, dépôt et remise des enveloppes, tenue du journal, et envoi des
lots vers le tableau de bord, chacune connectée aux autres par des files
d'attente bornées — en cas de saturation, ce sont les nouveaux paquets entrants
qui sont abandonnés en priorité, jamais les enveloppes déjà acceptées par le
relais.

Le relais tolère plusieurs types de pannes sans compromettre sa fonction
première : une coupure du Wi-Fi n'interrompt jamais le relais Bluetooth, elle
se contente d'accumuler les journaux dans un tampon local en attendant la
reconnexion ; un redémarrage recharge la clé et la position dans le journal
depuis la mémoire flash, préservant la continuité de la chaîne de journal, seuls
les caches volatils repartant à vide ; une saturation de la mémoire vive
déclenche d'abord l'éviction des entrées les plus anciennes du cache de
réconciliation, puis le refus de nouvelles enveloppes tout en protégeant celles
déjà stockées. Le relais ne journalise jamais le contenu d'un message, son
identifiant de suivi de bout en bout, ni l'identité d'un destinataire — les
identifiants de paquets transmis au tableau de bord sont systématiquement
hachés avant d'être envoyés.

## Dashboard d'observabilité

Le tableau de bord observe le réseau sans jamais y participer : il ne
transporte aucun message, ne détient aucune clé d'utilisateur et ne peut rien
injecter sur le terrain — sa disparition n'a strictement aucun effet sur le
fonctionnement de la messagerie elle-même. Il reçoit des lots d'événements de
métadonnées signés, envoyés par les relais et, sur une base volontaire, par les
applications clientes qui l'acceptent explicitement ; il les vérifie, les
stocke, puis les restitue sous forme de vues utiles à un opérateur : le
parcours détaillé d'un message donné, la santé générale du réseau, l'état de la
flotte de relais, une recherche libre dans les journaux, et l'intégrité des
journaux chaînés de chaque appareil.

Son architecture repose sur une application **FastAPI** en Python, qui expose à
la fois un point d'entrée d'ingestion HTTPS, une API de consultation classique,
et un flux d'événements en temps réel (*Server-Sent Events*) pour rafraîchir
automatiquement l'interface d'un opérateur connecté. La vérification
cryptographique des journaux chaînés reçus n'est pas réimplémentée en Python :
elle est déléguée, en sous-processus, à un binaire écrit en Rust qui réutilise
directement la même logique de vérification que celle employée par chaque
appareil du réseau — garantissant une seule source de vérité pour cette
opération, sans imposer un backend entier écrit en Rust. Les données sont
conservées dans une base **SQLite** simple, entièrement réinitialisée après
chaque session de démonstration.

Le point d'entrée d'ingestion accepte des lots d'événements authentifiés par
un jeton de courte durée et signés par la clé de l'appareil émetteur ; chaque
lot est vérifié, dédupliqué, puis inséré, avant de mettre à jour plusieurs vues
reconstruites (messages, nœuds, liens du réseau, sauts d'un message) et de
notifier en temps réel les opérateurs connectés. Un événement dont la signature
est invalide, qui provient d'un appareil non enregistré, ou qui contiendrait
par erreur un identifiant de message non haché est systématiquement rejeté —
cette dernière règle constitue une protection explicite contre toute fuite
accidentelle de métadonnée trop précise. Aucune donnée reçue ne peut jamais
contenir le texte d'un message, son identifiant stable de bout en bout, ou
l'identité d'un destinataire : ces informations sont bloquées au niveau du
schéma d'ingestion lui-même, pas seulement par convention.

L'interface propose cinq vues principales : le parcours d'un message donné,
présenté comme une chronologie verticale de ses sauts successifs avec le nœud
traversé, l'horodatage et le compteur de sauts à chaque étape ; une carte du
réseau, sous forme de graphe des appareils et de leurs liens, colorée selon
leur état de santé ; un tableau de la flotte des relais avec leur version de
firmware, leur disponibilité et l'état de leurs tampons internes ; un tableau
de l'intégrité des journaux chaînés de chaque appareil, avec le verdict de la
dernière vérification ; et une recherche libre dans l'ensemble des
événements reçus, avec export possible des résultats.
