# Problème et besoins

> Section du [rapport](README.md) — voir le [plan détaillé](00-plan.md).
> Source de conception : [`docs/synthese/02-probleme-et-besoins.md`](../synthese/02-probleme-et-besoins.md).

## Le concept

**dengon** (伝言, « message que l'on confie à quelqu'un pour qu'il le
transmette ») est une application d'échange de messages texte par Bluetooth,
qui fonctionne **sans connexion Internet**. Un message est confié au réseau, qui
le porte de proche en proche jusqu'à son destinataire, même si aucun appareil
n'a jamais de connexion directe de bout en bout.

## Le problème

Échanger des messages texte lorsque l'infrastructure réseau habituelle est
absente ou indisponible : réseau cellulaire saturé (affluence, manifestation),
zone blanche, contexte de catastrophe, festival, ou simplement par choix d'une
messagerie qui ne dépend d'aucun opérateur. Le seul média disponible entre
appareils proches est alors le **Bluetooth Low Energy (BLE)**, dont la portée ne
dépasse pas 10 à 30 mètres — il faut donc faire coopérer plusieurs appareils en
**réseau maillé** (*mesh*) pour couvrir une distance utile. Les appareils qui
composent ce réseau bougent, s'éteignent, sortent de portée : il n'existe donc
jamais de chemin complet et stable entre un expéditeur et un destinataire à un
instant donné. C'est la définition d'un **réseau tolérant aux délais** (DTN —
*Delay-Tolerant Network*), développé en détail dans la section
[« État de l'art »](02-etat-de-lart.md).

## Cahier des charges

Le projet répond à six exigences :

1. **Émettre** un message en Bluetooth.
2. **Sécuriser** le message : confidentialité, intégrité et authenticité.
3. **Recevoir** un message en Bluetooth.
4. **Stocker** le message sur les points de transmission successifs tant que le
   destinataire final ne l'a pas reçu.
5. **Faire circuler** le message de point en point, sur plusieurs sauts si
   nécessaire.
6. **Passer par une carte Arduino** équipée d'un module Bluetooth, qui relaie
   l'information vers un **tableau de bord** accessible en ligne.

## Acteurs

| Acteur | Rôle | Support |
| --- | --- | --- |
| Utilisateur | écrit, envoie et lit des messages | smartphone Android |
| Application cliente (`dengon-app`) | interface utilisateur et nœud à part entière du réseau | Android, cœur partagé en Rust |
| Nœud en ligne de commande (`dengon-node`) | nœud sans interface graphique, utile pour les tests multi-nœuds et pour un poste fixe | PC / Linux |
| Relais (`dengon-relay`) | infrastructure fixe : relaie les messages, les met en cache pour un destinataire absent, remonte des journaux au tableau de bord | carte ESP32-WROOM-32E, Wi-Fi |
| Tableau de bord (`dengon-dashboard`) | observe le réseau, trace le parcours des messages, surveille l'état de la flotte de relais | serveur (VPS) de l'équipe |
| Opérateur | consulte le tableau de bord | navigateur web |

## Cas d'usage

Le périmètre du MVP couvre six scénarios représentatifs :

1. **Envoi direct** : les deux appareils sont à portée BLE l'un de l'autre, le
   message est livré en un seul saut.
2. **Envoi multi-saut** : le destinataire est hors de portée directe, un relais
   ESP32 ou un téléphone tiers se trouve entre les deux et transmet le message.
3. **Destinataire absent** : l'appareil du destinataire est éteint au moment de
   l'envoi ; le message est déposé, sous forme d'**enveloppe scellée**
   illisible pour qui la transporte, sur les relais rencontrés, et récupéré au
   retour du destinataire.
4. **Expéditeur déconnecté après l'envoi** : l'expéditeur coupe son Bluetooth
   juste après avoir envoyé ; à sa reconnexion, les accusés de réception déjà
   produits par le réseau le rattrapent et mettent à jour le statut du message.
5. **Suivi d'un message** : un opérateur recherche l'identifiant d'un message
   sur le tableau de bord et visualise le chemin qu'il a emprunté ainsi que son
   état courant.
6. **Vérification d'un contact** : deux utilisateurs qui se rencontrent
   physiquement scannent mutuellement un QR code puis comparent de vive voix un
   code de vérification à 60 chiffres, pour s'assurer qu'aucun tiers ne
   s'interpose sur l'échange de clés.

## Périmètre du MVP

**Inclus** : messages texte courts (moins de 1 Ko utile), échangés en tête à
tête (1-à-1) ; chiffrement de bout en bout et signatures ; maillage BLE avec
émission, réception, relais multi-saut et diffusion contrôlée ; conservation
des messages non livrés et réémission automatique ; statuts *En attente → Parti
→ Distribué* (avec les cas *Échec* et *Expiré*), assortis d'accusés de réception
signés ; un relais ESP32 pleinement fonctionnel, livré comme un composant
complet du projet et non comme une simple démonstration de faisabilité ;
tableau de bord (parcours d'un message, carte du réseau, état de la flotte de
relais, recherche de journaux, vérification d'intégrité) ; application Android.

**Explicitement hors MVP**, mais prévu par l'architecture pour une évolution
ultérieure : application iOS ; statut « Lu » ; optimisations de propagation
(filtres compacts, budget de copies limité par message) — un échange
d'inventaire plus simple suffit pour le volume visé par le MVP ; conversations
de groupe ; pièces jointes ; renouvellement de clé à chaque message façon
Signal ; firmware du relais entièrement écrit en Rust ; relais de messages par
le serveur — le tableau de bord observe, il ne transporte jamais de message.

## Portée réaliste

La portée BLE, de l'ordre de 10 à 30 mètres par saut, se cumule à travers le
maillage : trois sauts couvrent environ une centaine de mètres de bout en bout.
Un message peut ne jamais arriver si aucun chemin physique ne relie
l'expéditeur au destinataire à un moment donné — un réseau tolérant aux délais
garantit une livraison *au mieux*, jamais une livraison certaine ; le tableau de
bord et les accusés de réception servent à **observer** cette réalité, pas à la
forcer. Pour une démonstration, trois à cinq appareils (deux téléphones ou PC,
une carte Arduino, un ou deux relais supplémentaires) suffisent à illustrer les
six points du cahier des charges ; la démonstration finale visée réunit cinq à
huit appareils.

## Choix produit retenus

Au-delà des exigences techniques, plusieurs décisions de produit cadrent
l'expérience utilisateur du MVP : le scénario d'usage prioritaire est un
**campus ou un bâtiment** (zone de taille moyenne, mélange de téléphones et de
quelques relais fixes) ; l'identité d'un utilisateur est un **pseudonyme libre,
sans vérification** — l'identité réelle repose sur la clé échangée en
personne ; l'interface est en **français** pour cette première version ; un
indicateur simple signale si l'appareil est « connecté au réseau » ou « isolé » ;
un message resté sans livraison après 24 heures passe au statut « Échec »,
relançable manuellement par l'utilisateur. Le détail complet de ces choix
figure dans `synthese/02-probleme-et-besoins.md` §9.
