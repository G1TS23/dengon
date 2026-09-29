# Conclusion et perspectives

> Section du [rapport](README.md) — voir le [plan détaillé](00-plan.md).
> Source : [`docs/synthese/02-probleme-et-besoins.md`](../synthese/02-probleme-et-besoins.md)
> §3 (cahier des charges) et §6 (périmètre hors MVP),
> [`docs/synthese/01-sujets-a-trancher.md`](../synthese/01-sujets-a-trancher.md),
> [Réalisation](04-realisation.md).

## Face aux six exigences du cahier des charges

Le cahier des charges initial posait six exigences : émettre un message en
Bluetooth, le sécuriser, le recevoir, le stocker le temps que le destinataire
ne l'ait pas reçu, le faire circuler de proche en proche, et passer par une
carte équipée d'un module Bluetooth qui relaie l'information vers un tableau
de bord en ligne. Les cinq premières sont couvertes par le cœur du protocole,
livré et testé — voir [Réalisation](04-realisation.md) : émission et
réception réelles entre deux appareils Android, chiffrement de bout en bout
et signatures pour chaque paquet qui en a besoin, stockage en attente
(*outbox*, enveloppes scellées) tant qu'un message n'a pas été distribué, et
un routage multi-saut testé aussi bien en simulation qu'entre trois relais
réels. La sixième — la carte, le relais, et le tableau de bord — est elle
aussi livrée : un firmware ESP32 complet, avec ses quatre tâches
concurrentes, et un tableau de bord qui ingère, vérifie et restitue ce que ce
relais lui remonte.

Ce que ces cinq exigences ne disent pas, en revanche, c'est si l'**assemblage**
de toutes ces pièces, prises ensemble, se comporte comme prévu sur le
terrain — c'est précisément le rôle de la recette (voir
[Recette et tests](05-recette.md)), qui n'a pas encore eu lieu au moment de
la rédaction de cette section.

## Ce que le projet a préféré ne pas faire, et pourquoi

Certains choix de conception méritent d'être assumés explicitement plutôt que
laissés comme des absences muettes. Le tableau de bord n'**agit** jamais sur
le réseau — il observe, ne relaie rien, et sa disparition n'a aucun effet sur
la messagerie elle-même : un choix délibéré, pas une limitation technique,
pour qu'aucun composant connecté à Internet ne devienne un point de défaillance
ou de contrôle du réseau de messagerie hors-ligne. De même, le relais ESP32
ne détient jamais la moindre clé d'un utilisateur : au pire, un relais
physiquement compromis peut mentir sur son propre journal — un mensonge
détectable par recoupement d'attestations entre plusieurs relais indépendants
— mais ne peut déchiffrer aucun message. Ces deux décisions, prises tôt dans
la conception, ont guidé une bonne partie des choix d'architecture qui ont
suivi, jusque dans le format binaire des paquets et le schéma d'ingestion du
tableau de bord.

## Limites connues du système livré

Au-delà de ce qui était **prévu** comme hors périmètre du MVP dès la
conception (application iOS, statut « Lu », groupes, pièces jointes, ratchet
façon Signal, firmware entièrement réécrit en Rust), le système tel que livré
porte des limites qu'il faut garder à l'esprit pour ne pas surinterpréter ce
qui a été démontré :

- Un message peut ne **jamais** arriver si aucun chemin physique ne relie
  l'expéditeur au destinataire — un réseau tolérant aux ruptures de connexion
  garantit une remise au mieux, jamais une remise certaine. Le tableau de
  bord et les accusés de réception servent à **observer** cette réalité, pas
  à la corriger.
- La portée pratique d'un saut Bluetooth (environ 10 à 30 mètres) borne
  directement la portée totale du maillage — quelques centaines de mètres
  cumulés sur plusieurs sauts, pas des kilomètres.
- Un téléphone ne peut pas encore se faire relayer par un relais ESP32 réel :
  voir la section correspondante de [Réalisation](04-realisation.md).
- Aucun audit de sécurité externe n'a eu lieu sur la cryptographie ni sur la
  surface d'entrée du firmware — une étape que la conception identifie
  explicitement comme nécessaire avant tout déploiement au-delà d'une
  démonstration.

## Perspectives (v2)

Plusieurs directions, déjà anticipées dans l'architecture sans être
implémentées, prolongeraient naturellement ce travail : une réconciliation
par filtre compact (Gossip GCS) qui remplacerait l'échange d'inventaire brut
retenu pour le MVP, économe en bande passante à mesure que le nombre de
messages en circulation grandit ; un budget de copies façon Spray-and-Wait
pour borner la diffusion des enveloppes scellées plutôt que de s'appuyer
uniquement sur leur expiration ; le statut « Lu », déjà prévu dans le format
du protocole mais volontairement reporté ; une implémentation iOS du même
trait `Transport`, rendue possible sans toucher au cœur du projet
puisqu'aucune plateforme ne connaît le protocole autrement qu'à travers cette
seule interface ; et, côté sécurité, un audit externe de la
cryptographie et un test d'intrusion du tableau de bord avant tout
déploiement destiné à des utilisateurs réels plutôt qu'à une démonstration.

Le choix architectural le plus structurant du projet — une seule
implémentation du protocole, en Rust, partagée par trois exécutables
radicalement différents à travers une interface minimale de transport — est
aussi ce qui rend chacune de ces directions réalisable sans reprise en
profondeur : la sécurité, le format des paquets et la logique de routage
n'ont besoin d'être ni relus ni réécrits pour chaque nouvelle plateforme qui
s'ajouterait.
