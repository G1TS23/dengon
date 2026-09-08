# Décisions de cadrage — v1

> Choix pris avec Olivier le 2026-09-08. À valider/ajuster avec Paul et Tanguy.
> Complète `CONTEXT.md` et `analyse-besoins.md`.

> ⚠️ **Soutenance le 29/09/2026** — soit ~3 semaines à partir du 08/09/2026.
> C'est très court pour tenir « étude **+** démo à parts égales » sur un
> protocole maison + chiffrement + dashboard + Android + ESP32. Voir la
> section « Impact du délai » en bas : le périmètre de démo est
> probablement à réduire fortement.

## Décisions

| # | Sujet | Choix |
|---|-------|-------|
| 1 | Objectif de la v1 | **Étude de conception + démo fonctionnelle**, à parts égales. Périmètre volontairement resserré. |
| 2 | Appareils supportés en v1 | **Téléphones Android + cartes ESP32** (relais). Les iPhone sont reportés (contraintes techniques d'arrière-plan trop fortes pour une v1). |
| 3 | Type d'échanges | **1-à-1 uniquement** : un seul destinataire par message. Pas de groupes, pas de diffusion générale en v1. |
| 4 | Dashboard de suivi | **Inclus dans la v1**, mais en version minimale (voir « Implications »). |
| 5 | Protocole de communication | **Protocole maison**, conçu en s'inspirant des solutions existantes (Meshtastic, Briar, Bluetooth Mesh…). |
| 6 | Chiffrement du contenu | **Dès la v1** : les relais ne peuvent pas lire les messages. |
| 7 | Ajout d'un contact | **En présentiel uniquement** : échange de clés en se rencontrant (ex. scan d'un QR code). |
| 8 | Statut « Lu/vu » | **Reporté en v2**. La v1 gère : En attente → Parti → Distribué. |

## Implications à retenir

- **Le livrable phare de l'étude = la spécification du protocole maison** : format des
  trames, entêtes (ID message, expéditeur, destinataire, nombre de sauts, horodatage),
  règles de rediffusion, déduplication, découpage des messages trop longs.
- **Sécurité (choix 6 + 7)** : modèle simple et cohérent → chaque utilisateur a une paire
  de clés ; on échange les clés publiques en présentiel (confiance « à la première
  rencontre ») ; le contenu est chiffré de bout en bout. Pas de chiffrement de groupe à
  gérer puisque la v1 est 1-à-1.
- **Statut « Distribué » (choix 8)** : même sans le « Lu », il faut qu'un accusé
  *remonte* de l'appareil du destinataire jusqu'à l'expéditeur à travers le réseau.
  C'est la partie la moins triviale de la v1.
- **Dashboard en v1 (choix 4)** : ajoute un petit serveur + un appareil qui fait le pont
  quand il a du wifi. À garder minimal : une page qui liste les messages (par identifiant,
  jamais leur contenu) et leur statut. Pas de carte, pas de temps réel en v1.
- **Android + ESP32 (choix 2)** : le Bluetooth basse consommation (BLE) est le langage
  commun aux deux. Côté Android, l'app devra tourner avec une notification permanente pour
  continuer à relayer en arrière-plan. Côté ESP32, il faut un petit firmware de relais.
- Ne pas avoir d'iPhone en v1 **retire le principal risque technique** identifié dans
  `analyse-besoins.md`.

## Points d'équipe — avis d'Olivier (décision finale à prendre à trois)

| Sujet | Avis d'Olivier | Remarque |
|-------|----------------|----------|
| Scénario d'usage à privilégier | **Campus / bâtiment** | Zone moyenne, téléphones mobiles + quelques relais fixes ; bon terrain de test réaliste. |
| Durée de conservation d'un message non délivré | **≈ 24 h** | Couvre une absence d'une journée. Impact stockage à vérifier côté ESP32. |
| Techno de l'app mobile | **Multiplateforme** (Flutter ou React Native) | Vise à réutiliser le code pour l'iPhone plus tard. À confronter au fait que le Bluetooth en arrière-plan y est moins fiable — Paul et Tanguy tranchent. |
| Hébergement du dashboard | **VPS Debian** de l'équipe | Serveur accessible en ligne, maîtrisé par l'équipe. |
| Partie sur laquelle Olivier veut travailler | **Conception / spec du protocole** + **Dashboard** | Pour préparer la répartition avec Paul et Tanguy. |

## Produit / expérience utilisateur — avis d'Olivier (à valider à trois)

| Sujet | Avis d'Olivier | Remarque |
|-------|----------------|----------|
| Données affichées par le dashboard | **Parcours anonymisé** : ID message, statut, appareils traversés en codes. Jamais expéditeur/destinataire/contenu. | Cohérent avec le besoin de confidentialité. |
| Taille de la démo finale | **5-8 appareils** | Montre les sauts multiples et les chemins alternatifs. Prévoir d'emprunter du matériel. |
| Type de contenu d'un message (v1) | **Texte court** (~140-500 caractères) | Léger pour le réseau et les ESP32 ; limite le découpage. |
| Identité d'un utilisateur | **Pseudo libre**, sans vérification | L'identité réelle reste la clé échangée en personne. |
| Fonctionnement en arrière-plan | **Service de fond avec notification permanente** (Android) | Réaliste ; impose la notification obligatoire Android. |
| Conservation des messages sur le téléphone | **Jusqu'à suppression manuelle** par l'utilisateur | Historique complet, comportement de messagerie classique. |
| Info réseau montrée à l'utilisateur | **Indicateur simple** « Connecté au réseau » / « Isolé » | Rien de plus en v1. |
| Langue de l'interface (v1) | **Français uniquement** | Suffisant pour le projet et la soutenance. |
| Relais par un téléphone sans contact | **Oui, toujours** — participe au réseau dès l'installation | À concilier avec le mode batterie ci-dessous. |
| Réglage de consommation batterie | **Oui, dès la v1** : mode « économie » activable | Relaie moins souvent quand il est activé. |
| Vérification lors de l'échange de clés | **Oui** : code court identique à comparer de visu après le scan | Protège contre une falsification pendant la rencontre. |
| Nom du projet / de l'appli | **À rediscuter** en équipe | « dengon » (伝言) reste le nom de travail en attendant. |
| Message expiré (~24 h sans livraison) | Passe au statut **« Échec »** dans la conversation, sans notification | |
| Relance d'un message non parti | **Bouton « Renvoyer »** déclenché par l'utilisateur | |
| Blocage d'un contact indésirable | **Reporté** après la v1 | |
| Dashboard : rafraîchissement | **Mise à jour automatique** (statuts en direct à l'écran) | Plus parlant en démo. |

## Organisation d'équipe — avis d'Olivier (à valider à trois)

| Sujet | Avis d'Olivier | Remarque |
|-------|----------------|----------|
| Livrables | **Spec du protocole**, **doc sécurité**, **doc architecture**, **+ rapport écrit + support de présentation** | Rapport + présentation confirmés comme attendus de la soutenance. |
| Répartition du travail | **Par composant** : un sur l'appli mobile, un sur les ESP32, un sur le dashboard ; la conception se fait ensemble | Olivier se verrait sur protocole + dashboard (voir plus haut). |
| Simulateur de réseau | **Léger** : quelques scripts de test, pas un vrai simulateur | |
| Rythme de mise en commun | **Fusion dans un dépôt commun après la phase de conception** (~fin de la semaine 1), puis travail en commun | Révise le « une grosse fusion à la fin » : trop risqué sur 3 semaines. En attendant, chacun dans son dossier `docs/<prénom>` — voir [[no-remote-git-ops]]. |

## Planning — ~3 semaines (08/09 → soutenance 29/09/2026)

Découpage retenu par Olivier (à valider à trois) :

| Phase | Durée | Contenu |
|-------|-------|---------|
| Conception | ~1 semaine | Figer la spec du protocole (priorité : circulation des messages), cadrer sécurité + architecture. Mise en place du dépôt commun à la fin. |
| Démo | ~1,5 semaine | Développement de la démo réduite + dashboard minimal, en commun. |
| Rendu | ~0,5 semaine | Finalisation des documents, du rapport et des slides ; répétition de la soutenance. |

## Encore à trancher

- **Place mémoire** allouée aux messages en attente, surtout sur ESP32.
- Ordre d'affichage des messages reçus dans le désordre (par date d'envoi
  indiquée, ou par ordre d'arrivée).

## Impact du délai (soutenance 29/09/2026)

~3 semaines. Construire de zéro un protocole mesh maison **fiable**, avec
chiffrement de bout en bout, accusé « Distribué » qui remonte le réseau,
app multiplateforme relayant en arrière-plan, firmware ESP32 **et** un
dashboard temps réel hébergé sur VPS — le tout démontrable sur 5-8
appareils — n'est pas réaliste dans ce temps à trois.

**Orientation retenue par Olivier (à valider à trois) :**
*« Recentrer la démo, garder l'étude complète »* — l'étude (protocole,
sécurité, architecture) reste le socle ; la démo est réduite au minimum.
Dans l'étude, l'aspect à **creuser en priorité = la circulation des
messages** (propagation, boucles, condition d'arrêt) ; le reste est
survolé. Pour la démo réduite, Olivier veut idéalement montrer **les
quatre** aspects : relais, hors-ligne, suivi dashboard, sécurité.
→ si le temps manque, ordre de repli suggéré : 1) hors-ligne + relais
(le cœur), 2) sécurité (relais aveugle), 3) dashboard.

Pistes à discuter en équipe pour réduire le risque :

- **Recentrer la démo** sur le minimum qui raconte l'histoire : 2 téléphones
  + 1 relais (téléphone ou ESP32), message texte 1-à-1, propagation en 1-2
  sauts, statut jusqu'à « Distribué ». Chiffrement inclus si le temps le
  permet, sinon présenté dans l'étude.
- **Dashboard** : version vraiment minimale (liste live des ID + statut),
  ou simple maquette si le temps manque.
- **ESP32** : optionnel pour la démo ; peut rester au stade « étude +
  preuve de concept » si les téléphones suffisent à montrer le relais.
- Mettre le poids sur l'**étude** (spec protocole, sécurité, architecture),
  qui est la partie robuste au manque de temps, et assumer une démo
  réduite.
- **Se mettre en commun très vite** (pas « une grosse fusion à la fin ») :
  avec 3 semaines, il faut un dépôt partagé et des points fréquents.
