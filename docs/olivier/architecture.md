# Architecture du système — dengon (brouillon v0.1)

> Premier jet, rédigé le 2026-09-08. À relire et compléter avec Paul et Tanguy.
> S'appuie sur `CONTEXT.md`, `analyse-besoins.md`, `decisions-v1.md`,
> `protocole.md`. Les choix marqués « (ouvert) » ne sont pas tranchés.

---

## 1. But

Décrire **les éléments du système et comment ils s'articulent** : l'application
mobile, le firmware des cartes ESP32, et le sous-système de suivi (dashboard).

## 2. Vue d'ensemble

```
   [ Téléphone A ]───BLE───[ ESP32 relais ]───BLE───[ Téléphone B ]
        │  Alice                  │                       │  Bob
        │                         │ (Wi-Fi si dispo)      │ (Wi-Fi si dispo)
        │                         ▼                       │
        │                 ┌───────────────┐               │
        └────Wi-Fi si─────►  Serveur      ◄────Wi-Fi si───┘
             dispo        │  dashboard    │    dispo
                          │  (VPS Debian) │
                          └───────┬───────┘
                                  ▼
                          [ Page web de suivi ]
```

- Les **messages** circulent uniquement de proche en proche en **BLE**
  (voir `protocole.md`). Ils ne passent **jamais** par le serveur.
- Le **dashboard** est un **bonus non bloquant** : les nœuds qui ont du Wi-Fi à
  un instant donné envoient des **métadonnées anonymisées** au serveur.

## 3. Les briques

| Brique | Rôle | Support |
|--------|------|---------|
| **Application mobile** | Écrire / lire les messages, gérer les contacts, relayer, afficher les statuts. | Android (techno multiplateforme — avis Olivier ; **ouvert**). |
| **Firmware ESP32** | Relayer les trames (moteur allégé), éventuellement faire pont Wi-Fi vers le dashboard. | Carte FREENOVE ESP32 WROOM. |
| **Passerelle dashboard** | N'importe quel nœud (téléphone ou ESP32) qui a du Wi-Fi et pousse des événements au serveur. | Fonction, pas une machine dédiée. |
| **Serveur dashboard** | Recevoir les événements, les stocker, servir la page web, pousser les mises à jour en direct. | VPS Debian de l'équipe. |
| **Page web de suivi** | Afficher en direct les messages (identifiant + statut + parcours anonymisé). | Navigateur. |

## 4. Découpage en couches (application et firmware)

De la radio vers l'utilisateur :

```
┌─────────────────────────────────────────────┐
│ Application : conversations, contacts,       │  ← téléphone uniquement
│ statuts, réglages (mode éco), notifications  │
├─────────────────────────────────────────────┤
│ Sécurité : clés, chiffrement/déchiffrement,  │  ← téléphone (déchiffre)
│ signatures, vérification de contact          │     ESP32 (ne déchiffre pas)
├─────────────────────────────────────────────┤
│ Moteur dengon (protocole) : circulation,     │  ← commun téléphone + ESP32
│ déduplication, TTL, file de retransmission,  │
│ réassemblage, accusés                        │
├─────────────────────────────────────────────┤
│ Transport BLE : découverte des voisins,      │  ← commun, mais API différentes
│ connexion, envoi/réception de trames         │     selon la plateforme
├─────────────────────────────────────────────┤
│ Stockage local : messages, contacts, clés,   │  ← téléphone : base chiffrée
│ file, table « déjà vu »                      │     ESP32 : mémoire réduite
└─────────────────────────────────────────────┘
```

## 5. Ce qui est commun, ce qui est spécifique

- **Commun** téléphone ↔ ESP32 : le **format de trame** et la **logique du moteur
  dengon** (`protocole.md`, sections 5 et 6). Objectif : même comportement des
  deux côtés.
- **ESP32** : pas d'interface, **ne déchiffre pas** le contenu (il relaie
  seulement), moteur allégé, forte contrainte mémoire → file de retransmission
  plus petite, table « déjà vu » plus courte.
- **Téléphone** : interface complète, déchiffrement et affichage, stockage riche,
  notifications, service de fond avec notification permanente.

## 6. Le chemin d'un message, couche par couche

```
Alice (app)        : saisit le texte
  → Sécurité       : chiffre pour Bob, signe
  → Moteur         : crée la trame (ID, empreintes, TTL, horodatage), statut « En attente »
  → Transport BLE  : la pousse aux voisins → statut « Parti »

ESP32 relais       :
  Transport BLE    : reçoit la trame
  → Moteur         : pas destinataire, « déjà vu » ? non ; TTL 7→6 ;
                     met en file ; réémet aux voisins

Bob (app)          :
  Transport BLE    : reçoit
  → Moteur         : destinataire → réassemble
  → Sécurité       : déchiffre, vérifie la signature
  → Application    : affiche le message ; déclenche l'ACCUSÉ
  → Moteur         : crée la trame ACCUSÉ (chiffrée pour Alice) et la diffuse

ACCUSÉ : remonte Bob → ESP32 → Alice (même mécanisme).
  ESP32            : voit passer l'ACCUSÉ → retire le message de sa file
  Alice            : reçoit l'ACCUSÉ → statut « Distribué »
```

## 7. Sous-système dashboard

- **Qui envoie quoi** : quand un nœud a du Wi-Fi, il envoie au serveur des
  **événements** du type : *« message X : créé / relayé / distribué, à telle
  heure, par le nœud N »*.
- **Anonymisation** : jamais le contenu, jamais les empreintes réelles. Chaque
  nœud utilise un **code stable** (pseudonyme) pour se désigner. Le serveur ne
  peut donc pas savoir qui est Alice ni ce qu'elle a écrit — seulement qu'un
  message identifié `X` a suivi un certain parcours de codes.
- **Serveur (VPS Debian)** :
  - une **API** qui reçoit les événements,
  - une **base** qui les stocke,
  - un **canal temps réel** (pour que la page se mette à jour toute seule),
  - la **page web** elle-même.
- **Reconstruction du parcours** : le serveur recolle les événements partiels
  reçus de plusieurs nœuds pour afficher le trajet du message `X` et son statut
  courant.
- **Repli si le temps manque** : page rafraîchie à la main, ou simple maquette
  (voir `decisions-v1.md`, section « Impact du délai »).

## 8. Pistes technologiques (non tranchées — décisions Paul / Tanguy)

| Brique | Pistes | Remarque |
|--------|--------|----------|
| App mobile | Flutter / React Native (multiplateforme) ou natif Kotlin | Le BLE en arrière-plan est plus fiable en natif — arbitrage à faire. |
| Radio BLE (app) | Bibliothèque BLE de la techno choisie | Doit permettre d'être visible **et** chercheur en même temps. |
| Firmware ESP32 | ESP-IDF ou cœur Arduino, pile BLE de l'ESP32 | Évaluer aussi le Bluetooth Mesh natif d'ESP-IDF vs moteur maison. |
| Serveur dashboard | Quelque chose de simple sur le VPS Debian | Au choix de qui prend le dashboard. |
| Page web | Page légère + canal temps réel | Affichage live des statuts. |

## 9. Points ouverts

- **Un seul moteur ou deux ?** Réimplémenter la logique dengon deux fois
  (téléphone + ESP32) ou trouver un noyau partageable entre les deux.
  → Analyse et recommandation dans `etude-stack.md` §3 (reco v1 : deux
  implémentations encadrées par une spec de trame stricte).
- Comment un nœud obtient son **code anonyme stable** pour le dashboard.
- **Authentification** des envois vers le serveur dashboard (empêcher un faux
  reporting qui polluerait l'affichage).
- Repli d'hébergement si le **VPS n'est pas prêt** à temps (serveur local).
- Rôle exact de l'ESP32 dans la **démo** : vrai relais, ou preuve de concept
  séparée (voir `decisions-v1.md`).
