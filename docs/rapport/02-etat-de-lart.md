# État de l'art

> Section du [rapport](README.md) — voir le [plan détaillé](00-plan.md).
> Source de conception : [`docs/synthese/03-etat-de-lart.md`](../synthese/03-etat-de-lart.md).

## Réseaux tolérants aux délais (DTN)

Un réseau où il n'existe pas nécessairement de chemin continu entre un
expéditeur et un destinataire à un instant donné se nomme un **réseau tolérant
aux délais et aux ruptures** (DTN — *Delay/Disruption Tolerant Networking*). Son
principe fondateur, le **store-carry-forward**, tient en trois verbes : un nœud
**reçoit** un message, le **conserve** même s'il n'a personne à qui le
transmettre sur le moment, le **transporte** physiquement en se déplaçant, puis
le **retransmet** dès qu'un voisin utile apparaît. C'est exactement l'exigence
n°4 du cahier des charges de dengon (« stocker le message sur les points de
transmission tant que le destinataire ne l'a pas reçu »). Le concept est issu
des recherches sur les réseaux interplanétaires, où les délais de propagation et
les fenêtres de visibilité rendent toute connexion continue impossible, avant
d'être généralisé aux réseaux terrestres intermittents.

L'IETF a normalisé cette approche dans le **Bundle Protocol version 7** (RFC
9171, 2022) : l'unité transmise, un *bundle*, est un paquet auto-suffisant doté
d'une **durée de vie** au-delà de laquelle il est détruit ; le réseau pratique le
store-and-forward avec conservation en mémoire entre deux contacts ; un
mécanisme de **custody transfer** permet à un nœud de prendre la responsabilité
d'un bundle. dengon ne met pas en œuvre l'intégralité de ce RFC, mais plusieurs
de ses idées — durée de vie d'un message, responsabilité de transport — s'y
retrouvent sous une forme adaptée au BLE.

## Algorithmes de routage en environnement DTN

Plusieurs familles d'algorithmes répondent au problème du routage sans chemin
garanti :

| Algorithme | Principe | Avantage | Inconvénient |
| --- | --- | --- | --- |
| *Epidemic routing* | diffuser une copie à tous les voisins rencontrés | livraison quasi maximale, très simple | inonde le réseau, coûteux en mémoire et en énergie |
| *Spray-and-Wait* | ne diffuser qu'un nombre limité de copies, puis attendre une rencontre avec le destinataire | bon compromis coût/livraison | le nombre de copies à fixer dépend du contexte |
| PRoPHET | relayer selon une probabilité de rencontre estimée à partir de l'historique | efficace si les contacts sont réguliers | plus complexe à mettre en œuvre |
| *Managed flooding* + TTL (approche Bitchat) | inondation bornée par un compteur de sauts et un cache de déduplication | simple, déjà éprouvé sur BLE | pas « intelligent », ne cherche pas de route optimale |

dengon retient l'**inondation contrôlée bornée par TTL** (*managed flooding*)
comme socle pour le MVP, avec un TTL initial de 7 sauts, une fenêtre de
déduplication et un plafond d'anti-inondation par voisin. L'idée de
*Spray-and-Wait* — limiter le nombre de copies en circulation — reste une piste
d'optimisation pour une version future, appliquée spécifiquement au budget de
copies des enveloppes scellées.

## Bitchat : la référence la plus proche

**Bitchat**, publié en juillet 2025, répond à un problème quasiment identique à
celui de dengon : un réseau maillé Bluetooth LE où chaque téléphone joue à la
fois le rôle de client et de serveur, un routage multi-sauts avec un TTL limité
à 7, une mise en cache des messages pour les pairs hors ligne avec livraison
automatique à leur réapparition, un chiffrement de bout en bout construit sur le
cadre **Noise** (motif `XX`) combiné à Ed25519, et des identifiants éphémères
par session pour la vie privée. dengon reprend explicitement le cadre Noise et
le TTL de 7 sauts de cette architecture, tout en assumant une spécificité
propre : la passerelle Arduino vers un tableau de bord d'observabilité, absente
de Bitchat. Le code de Bitchat n'est pas réutilisé — il est écrit en Swift, quand
dengon est développé en Rust — seules les idées d'architecture sont reprises.

## Bridgefy : le contre-exemple à ne pas reproduire

**Bridgefy**, une application de messagerie mesh devenue populaire pendant des
mouvements de contestation en 2020, a été analysée et cassée par des
chercheurs académiques (Albrecht, Blasco, Jensen et Mareková, publié à CT-RSA
2021). Les failles trouvées et les leçons qui en découlent pour dengon :

| Faille identifiée | Cause | Réponse retenue par dengon |
| --- | --- | --- |
| Absence d'authentification | pas de vérification d'identité, usurpation possible | signature Ed25519 systématique, vérification des clés |
| Attaque de l'homme du milieu | des identités contradictoires étaient acceptées sans détection | *handshake* authentifié (Noise) |
| Chiffrement cassé | usage de RSA PKCS#1 v1.5, vulnérable à une attaque de type *padding oracle* | chiffrement authentifié moderne (ChaCha20-Poly1305), aucune cryptographie développée en interne |
| Rejeu et falsification | les textes chiffrés pouvaient être réordonnés ou rejoués | numéro de séquence par conversation combiné au chiffrement authentifié |
| Désanonymisation | des identifiants étaient diffusés en clair en continu | identifiants de destinataire tournants, renouvelés chaque jour |
| Déni de service | vulnérabilité de type bombe de compression | taille de message plafonnée, vigilance sur toute compression appliquée avant chiffrement |

Trois leçons universelles s'en dégagent, appliquées telles quelles dans la
conception de dengon : ne jamais concevoir sa propre cryptographie, toujours
s'appuyer sur des bibliothèques éprouvées ; authentifier les identités avant de
chiffrer, faute de quoi une attaque de l'homme du milieu reste possible ; se
méfier de toute combinaison entre compression et chiffrement, qui peut ouvrir
un canal d'attaque.

## Bluetooth Classic, BLE et la norme *Bluetooth Mesh*

Le Bluetooth existe sous deux formes aux usages distincts : le **Bluetooth
Classic** (BR/EDR), adapté au transfert de flux continu (audio, liaison série)
avec un débit élevé mais une consommation forte et un modèle strictement
point-à-point ; le **Bluetooth Low Energy** (BLE), pensé pour les capteurs et
l'Internet des objets, avec un débit plus faible mais une consommation très
réduite et un modèle qui combine connexion et diffusion (*advertising*). Le BLE
est le choix naturel pour un réseau maillé mobile — c'est également le
dénominateur commun avec une future version iOS.

Deux couches du BLE structurent l'usage qu'en fait dengon : **GAP** (*Generic
Access Profile*), qui gère la découverte des pairs et leurs rôles
(annonceur/scanneur, puis central/périphérique) ; et **GATT** (*Generic
Attribute Profile*), qui organise les données échangées en services et
caractéristiques — un message dengon transite par l'écriture d'une
caractéristique dédiée. La taille maximale d'un paquet applicatif (le MTU),
généralement comprise entre 20 et 512 octets selon la négociation, impose de
fragmenter puis réassembler les messages qui la dépassent.

Le Bluetooth SIG maintient par ailleurs, depuis 2017, une norme officielle
**Bluetooth Mesh**, pensée pour des réseaux de centaines d'objets connectés.
Son routage par inondation contrôlée, avec TTL décrémenté et cache des messages
déjà vus, a inspiré des choix de dengon. Elle prévoit même un rôle *Friend*, où
un nœud « ami » stocke les messages destinés à un nœud économe en énergie
endormi — un vrai mécanisme de store-and-forward déjà prévu par cette norme.
dengon ne l'adopte cependant pas telle quelle : c'est un modèle de type
publication/abonnement avec une sécurité fondée sur des clés partagées au niveau
du réseau et de l'application, incompatible avec le modèle retenu par dengon
(échanges strictement 1-à-1, chiffrés de bout en bout, avec des accusés de
réception qui remontent jusqu'à l'expéditeur). Adopter Bluetooth Mesh
reviendrait à adopter tout son modèle de sécurité ; dengon préfère partir d'une
pile BLE nue (GATT et annonce) et s'inspirer des idées de routage sans en
reprendre le cadre de sécurité.

## Blockchain : une analyse critique

Une blockchain complète combine cinq éléments : des empreintes cryptographiques
(hachage), un **chaînage** de ces empreintes qui garantit l'ordre et
l'immuabilité, des **arbres de Merkle** pour résumer de grands volumes de
données, une **réplication** intégrale du registre entre tous les participants,
et un mécanisme de **consensus** qui permet à des acteurs qui ne se font pas
confiance de s'accorder sur un historique unique sans autorité centrale. C'est
précisément ce dernier point — le consensus — qui rend une blockchain lourde,
lente et coûteuse en ressources, et c'est aussi celui dont dengon n'a pas
besoin.

Le tableau ci-dessous confronte les contraintes propres à dengon aux exigences
d'une blockchain :

| Contrainte de dengon | Ce qu'imposerait une blockchain | Verdict |
| --- | --- | --- |
| Messages privés entre deux personnes | un registre partagé et visible par les nœuds | contredit la confidentialité recherchée |
| Nœuds aux ressources limitées (téléphone, carte Arduino, batterie) | un consensus coûteux en calcul | trop lourd pour une carte ESP32 |
| Réseau intermittent, jamais entièrement connecté | un consensus suppose une large connectivité simultanée | incompatible avec un réseau tolérant aux délais |
| Volonté de purger les messages déjà livrés | un registre immuable et append-only par nature | à l'opposé du besoin |
| Faible nombre de nœuds à l'échelle visée | un registre qui grossit indéfiniment pour tout le monde | gaspillage de stockage |

Ce qui reste réellement utile, en revanche, ce sont les briques cryptographiques
de la blockchain **prises isolément, sans le consensus** : le **chaînage par
hachage**, où chaque entrée contient l'empreinte de la précédente, garantit
l'intégrité et l'ordre d'un historique — impossible d'y insérer, retirer ou
réordonner une entrée sans casser la chaîne, ce qu'un vérificateur détecte
immédiatement, et le coût de calcul reste dérisoire, à la portée d'une carte
ESP32 ; les **arbres de Merkle** permettent de résumer un lot d'éléments par une
racine unique et de prouver qu'un élément en fait partie avec un nombre de
hachages logarithmique, utile par exemple pour un accusé de réception groupé ;
combinées à des **signatures et des horodatages** (Ed25519), ces deux briques
suffisent à obtenir l'essentiel de ce qu'on attend d'une « blockchain » pour de
la messagerie — la preuve d'origine et d'ordre — sans le poids d'un consensus
distribué.

La formulation retenue pour décrire ce choix : dengon s'inspire des
**structures de données de la blockchain** (chaînage cryptographique et arbres
de Merkle) pour garantir l'intégrité et l'ordre des messages, **sans** recourir
à un mécanisme de consensus distribué, inadapté à un réseau Bluetooth
intermittent composé de nœuds aux ressources contraintes. Concrètement, dengon
met en œuvre un **journal chaîné signé par appareil**, agrégé et audité par le
tableau de bord mais jamais falsifiable par lui, sans aucun consensus — voir la
section [Sécurité](03-conception.md#sécurité) pour le détail de ce mécanisme.
Ce choix de formulation reste un point à confirmer en équipe si l'énoncé du
projet impose littéralement le terme « blockchain » (voir
[`synthese/01-sujets-a-trancher.md`](../synthese/01-sujets-a-trancher.md)).

## Synthèse : ce que dengon retient de cette recherche

Le réseau de dengon est un **DTN à routage épidémique contrôlé** :
store-carry-forward, inondation bornée par un compteur de sauts, une fenêtre de
déduplication et, pour une version future, un budget de copies limité pour les
envois ciblés. Cette lignée de recherche remonte aux travaux fondateurs de
Vahdat et Becker sur l'*Epidemic Routing* (2000), en passant par PRoPHET et
Spray-and-Wait, et trouve des applications concrètes dans des messageries
réelles comme Briar, Bridgefy et Bitchat.

dengon emprunte plusieurs traits qui évoquent une blockchain — absence de
serveur central pour l'identité (une clé publique en tient lieu), propagation
de type pair-à-pair par gossip, messages signés, détection de rejeu par hachage
de contenu, registre chaîné par hachage propre à chaque appareil — mais laisse
volontairement de côté tout ce qui rend une blockchain coûteuse : aucun
consensus global, aucune chaîne unique à l'échelle de tout le réseau (un ordre
causal par conversation suffit), aucune résistance de type *Sybil* fondée sur
une preuve de travail, aucune réplication totale du registre (chaque appareil
ne conserve que ses propres conversations et un court historique de cache).
