# dengon — Contexte global (source unique)

> **伝言 (*dengon*)** : « message que l'on confie à quelqu'un pour qu'il le
> transmette ». Un message est confié au réseau, qui le porte de proche en
> proche jusqu'à son destinataire, même si personne n'a Internet.

Ce document **regroupe l'intégralité** de la recherche et de la conception
produites dans les trois dossiers de travail (`docs/powl/`, `docs/oswin/`,
`docs/olivier/`). Objectif : permettre à une nouvelle conversation (ou à un
nouvel arrivant) d'avoir **tout le contexte** sans relire 34 fichiers.

- **Colonne vertébrale** : `docs/powl/` — la conception cible, désignée source de
  vérité par `CLAUDE.md`.
- Le contenu propre à `docs/oswin/` (état de l'art, benchmarks, bibliographie) et
  à `docs/olivier/` (spéc comportementale, format de trame v0.1, analyse de
  besoins, décisions de cadrage) est **fondu** dans les sections thématiques —
  rien n'est jeté.
- **Tout ce qui reste à trancher** (choix divergents entre dossiers, points
  ouverts, incohérences internes) est traité à part dans
  [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md), avec pour chaque sujet
  les options, les arguments et la piste de résolution.

Ce dossier couvre **recherche & conception uniquement**. L'état réel du code et
le méta-projet (échéance, équipe, conventions) restent dans
[`../suivi/`](../suivi/) et `CLAUDE.md`.

---

## Sommaire

1. [Résumé exécutif](#1-résumé-exécutif)
2. [Origine des sources & statut](#2-origine-des-sources--statut)
3. [Problème & besoin métier](#3-problème--besoin-métier)
4. [Concepts & état de l'art](#4-concepts--état-de-lart)
5. [Architecture cible](#5-architecture-cible)
6. [Protocole réseau](#6-protocole-réseau)
7. [Format binaire de trame](#7-format-binaire-de-trame)
8. [Sécurité](#8-sécurité)
9. [Cycle de vie d'un message & statuts](#9-cycle-de-vie-dun-message--statuts)
10. [Relais ESP32](#10-relais-esp32)
11. [Dashboard d'observabilité](#11-dashboard-dobservabilité)
12. [Modèles de données](#12-modèles-de-données)
13. [Benchmarks & choix technologiques](#13-benchmarks--choix-technologiques)
14. [Périmètre MVP & feuille de route](#14-périmètre-mvp--feuille-de-route)
15. [Stratégie de test & CI](#15-stratégie-de-test--ci)
16. [Analyse de besoins & questions de cadrage](#16-analyse-de-besoins--questions-de-cadrage)
17. [Glossaire](#17-glossaire)
18. [Bibliographie complète](#18-bibliographie-complète)
19. [Annexe — correspondance section ↔ fichiers sources](#19-annexe--correspondance-section--fichiers-sources)

---

## 1. Résumé exécutif

**dengon** est une messagerie **texte chiffrée de bout en bout** qui circule en
**gossip** sur un **maillage Bluetooth Low Energy (BLE)** de téléphones et de
**relais ESP32 fixes**, doublée d'un **tableau de bord d'observabilité** sur VPS
qui trace le parcours et l'état de chaque message **sans jamais voir son
contenu**. Le réseau n'est jamais entièrement connecté : c'est un **DTN**
(*Delay-Tolerant Network*) *store-carry-forward* — on stocke, on transporte, on
retransmet à la prochaine occasion.

### Décisions verrouillées (conception `powl`)

| # | Sujet | Décision |
| --- | --- | --- |
| 1 | Nature du réseau | DTN à routage gossip + **journal chaîné signé** par appareil, agrégé et audité par le dashboard. **Pas de consensus, pas de minage.** C'est l'interprétation retenue de « sécurité type blockchain ». |
| 2 | App cliente | Cœur **Rust partagé `dengon-core`** ; app **Android/Kotlin** (MVP) via **UniFFI** ; `dengon-node` CLI (tests + nœud fixe) ; iOS post-MVP. |
| 3 | Crypto | **Noise `XX`** (session live) + **Noise `X`** (enveloppes offline) + **Ed25519** (signature de paquet) ; contact par **QR + code de vérification à 60 chiffres** ; **TOFU**. |
| 4 | Relais | **ESP32-WROVER** + ESP-IDF + NimBLE ; réutilise `dengon-core` ; **ne déchiffre rien** ; remonte des logs signés en **MQTT/TLS**. |
| 5 | Dashboard | **Observabilité seule** — MQTT → **Axum (Rust)** → **PostgreSQL/TimescaleDB** → **React** ; aucun contenu, `msgID` haché ; ne peut ni router ni injecter. Docker Compose + Caddy. |
| 6 | Statuts | *en attente → parti → distribué → lu* (+ *échec/expiré*), pilotés par accusés signés, rattrapés à la reconnexion. |

### Principes directeurs

1. **Offline-first** : toute fonction marche sans Internet ; la connectivité est
   une opportunité, jamais un prérequis.
2. **Zéro confiance dans le transport** : relais, VPS et réseau supposés hostiles
   → E2E systématique, métadonnées minimisées.
3. **Une seule implémentation du protocole** : `dengon-core` en Rust, partagé par
   l'app, le nœud CLI et (autant que possible) le firmware. Auditer une fois.
4. **Le dashboard observe, il n'agit pas** : ni router, ni injecter, ni
   déchiffrer. S'il disparaît, le réseau fonctionne à l'identique.
5. **Traçabilité infalsifiable** : chaque nœud tient un journal chaîné signé ; le
   dashboard le vérifie mais ne peut pas le forger.

### Statut

**Conception — pas encore de code.** Prochaine étape : **Lot 0** (spikes de
dérisquage), cf. §14. Le suivi de ce qui est réellement codé est dans
[`../suivi/`](../suivi/).

---

## 2. Origine des sources & statut

Trois efforts de recherche/conception **parallèles et indépendants**, un par
contributeur, ont été menés avant mise en commun.

| Dossier | Auteur | Nature | Statut / maturité | Stack proposée |
| --- | --- | --- | --- | --- |
| [`docs/powl/`](../powl/) | powl | **Conception cible complète** (13 fichiers), désignée **source de vérité** par `CLAUDE.md`. Sources de vérité internes : `03` (protocole/trame), `08` (événements), `09` (schémas). | Conception figée, pas de code. | Cœur Rust `dengon-core` partagé, Android/Kotlin + UniFFI, Noise XX/X + Ed25519, ESP32-WROVER + ESP-IDF/NimBLE, dashboard MQTT→Axum→PostgreSQL/Timescale→React. |
| [`docs/oswin/`](../oswin/) | oswin | **Recherche documentaire sourcée** (11 fichiers), autonome, chaque fiche se termine par un encadré « À retenir ». Bibliographie complète (~60 références). Rédigé le 8 septembre 2026. | Recherche, pas de décision d'équipe engageante au-delà de 3 points (voir ci-dessous). | Flutter recommandé + `flutter_blue_plus`/`bluetooth_low_energy`, Arduino/PlatformIO, libsodium, Mosquitto/HiveMQ, Node-RED/ThingsBoard. ESP32-WROOM-32E. |
| [`docs/olivier/`](../olivier/) | olivier | **Brouillons v0.1–v0.3**, zone « protocole + dashboard ». Contient une spéc comportementale (`protocole.md` v0.3) et un **format binaire de trame octet par octet** (`format-trame.md` v0.1, marqué « fait foi » de son côté). | Brouillons, points « (ouvert) » explicites, valeurs « (à calibrer) ». À valider avec Paul et Tanguy. | Flutter ou Kotlin natif (décision après proto semaine 1), **deux implémentations** du moteur, libsodium `crypto_box` / mbedTLS, FastAPI/Express + SSE + SQLite. ESP32-WROOM. |

**Trois décisions d'équipe déjà actées côté `oswin`** (README `oswin`) :
matériel = carte **ESP32-WROOM-32E** (Freenove) ; architecture = mesh
téléphone-à-téléphone, les ESP32 servant de relais **et** de passerelle unique
vers le dashboard (les téléphones ne contactent jamais le dashboard
directement) ; livrable applicatif = une **application Android (.apk)**, techno
conseillée pour débuter = **Flutter**.

**`powl` diverge sur plusieurs de ces points** (carte WROVER, un seul cœur Rust,
Android/Kotlin natif, dashboard Rust/Timescale…). Toutes ces divergences, ainsi
que les points restés ouverts et les incohérences internes, sont recensées dans
[`01-sujets-a-trancher.md`](01-sujets-a-trancher.md). **Dans le corps de ce
document, c'est la conception `powl` qui fait référence**, le contenu unique
d'`oswin` et d'`olivier` étant intégré à titre de recherche, d'alternative ou de
complément.

La structure de dépôt proposée par `olivier/mise-en-commun.md` (`app/`,
`firmware/`, `dashboard/serveur/`, `outils/`, `docs/protocole/…`) **n'a pas été
adoptée** : le dépôt réel utilise `docs/powl/` (conception) + `docs/suivi/`
(suivi) et le découpage `crates/` de `powl/02`.

---

## 3. Problème & besoin métier

### 3.1 Le concept (`olivier/CONTEXT.md`)

Application d'envoi de messages par Bluetooth dans un réseau connecté **sans
connexion Internet**.

**Besoins métier :**

- Envoyer un message ; recevoir un message.
- Sécuriser l'envoi des messages ; sécuriser le contenu des messages.
- Pas en connexion directe.
- Pas de connexion Internet ; pas de connexion mobile.
- Être déconnecté ne doit **pas** être bloquant.
- Le message se diffuse sur le réseau.
- Suivre les messages (transit et statut) sur un dashboard si connexion Wi-Fi
  (non bloquant).
- Passer par des téléphones mobiles (Android **et iOS**) et par des cartes
  Arduino avec Bluetooth et Wi-Fi (**FREENOVE ESP32 WROOM**).

**Statuts des messages (liste canonique de `CONTEXT.md`) :**
1. En attente → 2. Parti → 3. Distribué → 4. Lu/vu.

> Tensions à noter dès ici : `CONTEXT.md` met **iOS** dans le périmètre et la
> carte **WROOM** ; `olivier/decisions-v1.md` reporte iOS et le « Lu » en v2 ;
> `powl` spécifie **WROVER** (PSRAM nécessaire) et garde « lu » dans le MVP.

### 3.2 Le problème (`powl/00`)

Échanger des messages texte **sans infrastructure réseau** : réseau cellulaire
saturé, zone blanche, manifestation / catastrophe / festival, ou par principe
(messagerie sans opérateur). Seul média entre appareils proches : le **BLE**,
portée ~10–30 m → il faut un **maillage**. Les appareils bougent, s'éteignent,
sortent de portée → réseau jamais entièrement connecté → **DTN**.

### 3.3 Reformulation `oswin` — 6 exigences du cahier des charges

1. **Émettre** un message en Bluetooth.
2. **Sécuriser** le message (confidentialité + intégrité + authenticité).
3. **Recevoir** un message en Bluetooth.
4. **Stocker** le message sur les points de transmission tant que le
   destinataire ne l'a pas reçu.
5. **Faire circuler** le message de point en point (relais multi-sauts).
6. **Passer par une carte Arduino** équipée d'un module Bluetooth, qui relaie
   l'information vers un **dashboard en ligne**.

### 3.4 Acteurs (`powl/00`)

| Acteur | Rôle | Matériel |
| --- | --- | --- |
| **Utilisateur** | écrit, envoie, lit | smartphone (Android en MVP) |
| **App cliente** `dengon-app` | UI + nœud du réseau | Android + cœur Rust partagé |
| **Nœud CLI** `dengon-node` | nœud sans UI : tests multi-nœuds, bootstrap, PC fixe | PC/Linux (Rust) |
| **Relais** `dengon-relay` | infra fixe : relaie, met en cache, dépose, remonte les logs | ESP32-WROVER + Wi-Fi |
| **Dashboard** `dengon-dashboard` | observe le réseau, trace les messages, surveille la flotte | VPS (déjà possédé) |
| **Opérateur** | exploite le dashboard | navigateur |

### 3.5 Cas d'usage MVP (`powl/00`)

1. **Envoi direct** : A et B à portée BLE → livré en 1 saut.
2. **Envoi multi-saut** : A → B hors de portée, un relais ESP32 (ou téléphone
   tiers) entre les deux → relayé.
3. **Destinataire absent** : Charlie éteint → message déposé en **enveloppe
   scellée** sur des relais, récupéré à son retour.
4. **Expéditeur déconnecté** : A envoie puis coupe le BLE ; à la reconnexion, les
   **accusés** distribué/lu la rattrapent.
5. **Suivi** : l'opérateur cherche un `msgID`, voit le chemin et l'état courant.
6. **Vérification de contact** : scan mutuel du **QR code** + comparaison d'un
   **code de vérification à 60 chiffres** → « vérifiés ».

### 3.6 Périmètre MVP (`powl/00`)

**Dans le MVP :** texte court (< 1 Ko utile), 1-à-1 ; chiffrement E2E (Noise) +
signatures (Ed25519) ; maillage BLE (émission, réception, relais multi-saut,
flood contrôlé + TTL) ; store-and-forward (outbox rejoué, enveloppes scellées,
réconciliation gossip) ; 4 statuts + `échec/expiré` avec accusés signés ; relais
ESP32 fonctionnel ; dashboard (parcours d'un message, carte réseau, flotte de
relais, recherche de logs, vérification d'intégrité des journaux) ; app Android.

**Hors MVP (prévu par l'architecture, pas livré) :** app iOS ; groupes / canaux
publics ; pièces jointes ; ratchet façon Signal ; firmware relais en Rust pur
(`esp-rs`) ; **backhaul** — le VPS ne relaie **pas** de messages (choix explicite
« observabilité seule »).

### 3.7 Portée réaliste (`oswin/00 §5`)

- BLE ~**10 à 30 m** par saut en pratique ; le mesh cumule les portées (3 sauts
  ≈ ~100 m de bout en bout).
- Un message peut **ne jamais arriver** si le chemin physique n'existe pas : un
  DTN garantit du *best-effort*, pas une livraison certaine. Le dashboard et les
  ACK servent à **observer** la réalité.
- Pour une **démo d'école**, 3–5 nœuds (2 téléphones/PC + Arduino + 1–2 relais)
  suffisent à montrer les 6 points du cahier des charges.

---

## 4. Concepts & état de l'art

> Cette section provient très majoritairement de `docs/oswin/`. Elle sert de
> socle « rapport / soutenance » : vocabulaire académique, références à citer,
> contre-exemples.

### 4.1 Store-carry-forward / DTN (`oswin/03`)

Un réseau où il n'existe pas forcément de chemin **continu** entre A et B à un
instant donné = **réseau tolérant aux délais et aux ruptures (DTN — Delay/
Disruption Tolerant Networking)**. Principe fondamental = **store-carry-forward** :
un nœud **reçoit** un message, le **garde** (store) même sans personne à qui le
passer, le **transporte** physiquement (carry), le **retransmet** (forward) dès
qu'un voisin utile apparaît. C'est exactement « stocker sur les points de
transmission tant que le destinataire n'a pas reçu ». Concept issu des travaux
sur les réseaux interplanétaires puis généralisé.

### 4.2 Bundle Protocol v7 — RFC 9171 (2022, IETF) (`oswin/03 §2`)

Standardisation IETF du DTN. Idées réutilisables :

- unité transmise = un **bundle** (paquet auto-suffisant) ;
- chaque bundle a une **durée de vie** (*lifetime*) → au-delà, détruit (= TTL
  temporel) ;
- store-and-forward avec **conservation en mémoire** entre deux contacts ;
- **custody transfer** : un nœud peut prendre la **responsabilité** d'un bundle.

On n'implémente pas tout le RFC 9171, mais s'en inspirer donne de la crédibilité
au rapport ; `lifetime`, `custody`, `bundle` sont directement transposables.
Implémentation open source de référence : **DTN7**.

### 4.3 Algorithmes de routage DTN (`oswin/03 §3`)

| Algorithme | Principe | Avantage | Inconvénient |
| --- | --- | --- | --- |
| **Epidemic routing** | une copie à **tous** les voisins rencontrés | livraison quasi maximale, très simple | **inonde** le réseau, coût mémoire/énergie |
| **Spray-and-Wait** | ne diffuser qu'un **nombre limité L** de copies, puis attendre | bon compromis coût/livraison | règle L à fixer |
| **PRoPHET** | relayer selon une **probabilité de rencontre** (historique) | efficace si contacts réguliers | plus complexe |
| **Managed flooding + TTL** (Bitchat) | inondation **bornée par TTL** et cache | simple, éprouvé sur BLE | pas « intelligent » |

Recommandation `oswin` : commencer par l'**epidemic routing borné par TTL** (=
managed flooding) ; mentionner **Spray-and-Wait** comme optimisation.
`powl` retient précisément cette combinaison, et applique **Spray-and-Wait au
budget de copies des enveloppes scellées** (§6.6).

### 4.4 Bitchat — la référence à copier (`oswin/00 §2`, `oswin/01 §3B`)

Publié en **juillet 2025** par Jack Dorsey. Fait *presque exactement* ce que
décrit le projet :

- réseau **mesh Bluetooth LE**, chaque téléphone client **et** serveur ;
- routage multi-sauts avec **TTL limité à 7 sauts** ;
- **mise en cache** des messages pour les pairs hors-ligne, **livraison
  automatique** dès réapparition (= le « stockage aux points de transmission ») ;
- **chiffrement de bout en bout** : X25519, AES-256-GCM, Ed25519, cadre **Noise**
  (motif XX) ;
- **identifiants éphémères** par session pour la vie privée.

Conséquence : on ne réinvente rien, on s'inspire de l'architecture Bitchat (open
source) + on ajoute la spécificité passerelle Arduino → dashboard. `powl`
reprend explicitement Noise et le TTL 7 de Bitchat ; whitepaper cité :
`github.com/permissionlesstech/bitchat`.

### 4.5 Bridgefy (2020) — le contre-exemple à NE PAS reproduire (`oswin/02 §4`)

Bridgefy, app de messagerie mesh populaire pendant des manifestations, **cassée
par des chercheurs** (Albrecht, Blasco, Jensen, Mareková ; CT-RSA 2021 ; IACR
ePrint 2021/214).

| Faille trouvée | Cause | Ce que dengon doit faire |
| --- | --- | --- |
| **Aucune authentification** | pas de vérification d'identité → usurpation | signer avec **Ed25519**, vérifier les clés |
| **Man-in-the-middle** | identités contradictoires acceptées | *handshake* authentifié (Noise) |
| **Chiffrement cassé** | RSA **PKCS#1 v1.5** obsolète → *padding oracle* (Bleichenbacher) | **AEAD moderne** (AES-GCM / ChaCha20-Poly1305), jamais de crypto maison |
| **Rejeu / falsification** | ciphertexts réordonnables/rejouables | **numéro de séquence + AEAD** |
| **Désanonymisation** | identifiants en clair diffusés en continu | **identifiants éphémères** par session |
| **Déni de service** | *bombe de compression* (gzip) | limiter la taille, méfiance sur compression **avant** chiffrement |

**Les 3 leçons universelles :** (1) ne jamais inventer sa propre cryptographie
— utiliser des bibliothèques éprouvées (libsodium/NaCl, Noise, Web Crypto) ;
(2) authentifier avant de chiffrer les identités, sinon MITM ; (3) attention à
compression + chiffrement, les combiner crée des oracles.

### 4.6 Bluetooth Classic vs BLE (`oswin/01 §1`)

| | Bluetooth Classic (BR/EDR) | BLE |
| --- | --- | --- |
| Usage typique | audio, transfert de flux (SPP série) | capteurs, IoT, messages courts |
| Débit | élevé (~1–3 Mbit/s) | faible (~0,1–1 Mbit/s utile) |
| Consommation | forte | très faible |
| Modèle | connexion point-à-point | connexion **et** diffusion (*advertising*) |
| Module Arduino courant | HC-05 / HC-06 (profil série SPP) | ESP32, nRF52, HM-10 |
| Adapté au mesh ? | mal | **oui** (advertising + GATT) |

Recommandation : viser le **BLE** (ce qu'utilisent Bitchat, Bridgefy ; plus
économe ; là où vit la norme *Bluetooth Mesh*). Le HC-05 (Classic/SPP) reste
possible pour un prototype point-à-point simple mais ne maille pas nativement.
BLE est aussi le **dénominateur commun** avec iOS (`olivier/analyse-besoins §6`).

### 4.7 Couches BLE utiles (`oswin/01 §2`)

- **GAP** (Generic Access Profile) : **découverte** et rôles (advertiser/scanner,
  puis central/peripheral). L'*advertising* signale la présence.
- **GATT** (Generic Attribute Profile) : structure les données en **services** et
  **caractéristiques** (boîtes lisibles/écrivables/notifiables). Un message
  dengon transite par l'écriture d'une caractéristique dédiée.
- **MTU** : taille max d'un paquet applicatif (souvent **~20 à 512 octets**).
  Les messages plus longs doivent être **fragmentés** puis réassemblés.

### 4.8 Norme *Bluetooth Mesh* (SIG, depuis 2017) (`oswin/01 §3A`)

Spécification officielle du Bluetooth SIG pour des centaines d'objets. Routage =
**managed flooding** : message diffusé à tous les voisins ; chaque nœud relais
le retransmet ; anti-boucle par **TTL** décrémenté + **cache des messages déjà
vus** (*network message cache*). Rôles clés :

- **Relay** : retransmet ;
- **Friend / Low-Power** : un nœud « ami » **stocke** les messages destinés à un
  nœud basse consommation endormi — **un vrai store-and-forward déjà prévu par la
  norme** (= point 4 du cahier des charges, à citer) ;
- **Proxy** : permet à un smartphone GATT de parler au mesh.

Évolution : **Directed Forwarding** — crée de vraies **routes** au lieu de tout
inonder (plus efficace sur grands réseaux).

**Pourquoi dengon ne l'adopte pas** (`olivier/etude-stack §2.3`) : c'est un
modèle *publish/subscribe* avec sécurité par clés « réseau »/« application »
partagées — **pas** le modèle dengon (1-à-1, E2E, accusés qui remontent vers
l'expéditeur). L'adopter = adopter tout son modèle. Mieux vaut partir de
**NimBLE brut** (GATT + annonce) et **s'inspirer** des idées.

### 4.9 Analyse critique « faut-il une blockchain ? » (`oswin/04`)

Une blockchain « complète » combine : (1) hachages cryptographiques ; (2)
**chaînage** (chaque bloc contient le hash du précédent → immuabilité + ordre) ;
(3) **arbres de Merkle** (résumer plein de données par une racine) ; (4)
**réplication** (tout le monde a une copie) ; (5) **consensus** (PoW/PoS…) pour
que des acteurs qui ne se font pas confiance s'accordent sur **un** historique
sans autorité centrale. Le point (5) est ce qui rend la blockchain *lourde,
lente, coûteuse* — et ce dont dengon **n'a pas besoin**.

**Ce que la blockchain résout :** accord sur un historique unique, ordonné,
infalsifiable, entre parties sans confiance mutuelle et sans tiers de confiance.
dengon n'a **pas** ce problème (messages privés → confidentialité, pas registre
public ; relais qui transportent sans lire → E2EE + intégrité, pas consensus ;
livrer « au mieux » malgré les ruptures → store-carry-forward, pas registre
répliqué global).

| Contrainte dengon | Ce qu'impose une blockchain | Verdict |
| --- | --- | --- |
| Messages **privés** entre 2 personnes | registre **partagé/visible** par les nœuds | ❌ contredit la confidentialité |
| Nœuds **contraints** (téléphone, Arduino, batterie) | consensus **coûteux** | ❌ trop lourd pour un ESP32 |
| Réseau **intermittent** | consensus a besoin d'une **large connectivité** simultanée | ❌ incompatible DTN |
| On veut **oublier** les messages livrés (purge) | registre **append-only immuable** | ❌ à l'opposé du besoin |
| Passer à l'échelle sur peu de nœuds | registre qui **grossit indéfiniment** | ❌ gaspillage de stockage |

**Ce qui est vraiment utile** — les briques cryptographiques **sans** le
consensus :

- **Chaînage par hash** : chaque message contient le hash du précédent →
  intégrité + ordre (impossible d'insérer/retirer/réordonner sans casser la
  chaîne, le récepteur détecte le trou) ; détection de censure ; **léger** (juste
  des hachages, un ESP32 le fait sans problème).
  ```
  msg1 : {texte, prev = 0000}   h1 = hash(msg1)
  msg2 : {texte, prev = h1  }   h2 = hash(msg2)
  msg3 : {texte, prev = h2  }   h3 = hash(msg3)
  ```
- **Arbres de Merkle** : résumer un lot par une **racine** → **preuve
  d'inclusion compacte** (prouver qu'un message est dans un lot avec seulement
  **log₂(n)** hachages). Usages : accusé de réception groupé ; vérification
  d'intégrité d'un message fragmenté.
- **Signatures & horodatage** (Ed25519, fiche 02) : combinés au chaînage →
  preuve d'origine + d'ordre = 90 % de ce qu'on attend d'une « blockchain » pour
  de la messagerie, **sans** la blockchain.

**Formulation retenue** (`oswin/04 §5`, alignée avec `powl/01 §1.4`) :
> « Nous nous inspirons des **structures de données de la blockchain** (chaînage
> cryptographique et arbres de Merkle) pour garantir l'intégrité et l'ordre des
> messages, **sans** recourir à un mécanisme de consensus distribué, inadapté à
> un réseau Bluetooth intermittent composé de nœuds contraints. »

**Repli si l'énoncé impose vraiment « une blockchain »** (`oswin/04 §6`) : une
**blockchain locale, légère, à autorité de confiance** (pas de PoW) — un registre
chaîné des **événements** (message émis/relayé/livré), pas du contenu ;
hébergé/agrégé par la passerelle → dashboard ; consensus remplacé par la
**signature** de chaque nœud (preuve d'autorité simplifiée). C'est très proche du
**journal chaîné signé** effectivement retenu par `powl` (§8.4).

### 4.10 Ce que `powl` retient de tout ça (`powl/01 §1`)

Le réseau est un **DTN à routage épidémique / gossip** : store-carry-forward,
échange épidémique (deux nœuds se rencontrent → échangent tout ce qu'ils n'ont
pas en commun), flooding contrôlé borné par **TTL** + **fenêtre de
déduplication** + **budget de copies** pour les envois ciblés. Lignée de
recherche : Vahdat & Becker *Epidemic Routing* (2000), PRoPHET, Spray-and-Wait.
Messageries réelles : Briar, Bridgefy, bitchat.

Traits « blockchain-like » **présents** : pas de serveur central (identité = clé
publique, `peerID = SHA-256(pubkey)[:8]`) ; propagation gossip P2P ; messages
signés (Ed25519 par paquet) ; anti-rejeu/anti-doublon par hash de contenu
(`msgID` + seen-set) ; registre append-only chaîné par hash par appareil
(`hash_n = H(entry_n ‖ prev_hash)`) ; réconciliation d'état par filtres compacts
(Bloom/GCS), pull du manquant.

Traits **non empruntés** : consensus global (PoW/PoS/BFT) — incompatible
offline-first, coût prohibitif ; chaîne globale unique / ordre total — un ordre
**causal par conversation** suffit ; résistance Sybil / PoW à l'entrée — pas de
double-dépense, messages idempotents ; réplication totale du registre — chaque
nœud n'a besoin que de ses conversations + un cache court ; smart contracts / VM
— hors sujet.

---

## 5. Architecture cible

### 5.1 Décomposition en briques (backlog `oswin/00 §4`)

1. **Format de message** (« paquet ») : en-tête clair (ID, TTL, expéditeur
   pseudonyme, destinataire, horodatage) + charge utile **chiffrée**.
2. **Transport BLE** : découverte des voisins, envoi/réception.
3. **Logique de relais** : anti-boucle, décrément du TTL, retransmission aux
   nouveaux voisins.
4. **File de stockage** avec expiration (TTL temporel) et purge sur ACK.
5. **Couche cryptographique** : échange de clés, chiffrement authentifié,
   signature.
6. **Accusé de réception (ACK)** qui remonte le mesh pour libérer le stockage.
7. **Passerelle Arduino** + **dashboard**.
8. **(Optionnel) chaînage par hash / Merkle** pour l'intégrité et les preuves.

### 5.2 Vue composants — `dengon-core` (`powl/02 §1-2`)

`dengon-core` (Rust) contient **tout sauf l'I/O radio et l'I/O réseau IP**. Il
produit/consomme des `Vec<u8>` transportés par un `Transport` ; le *log shipper*
(MQTT, relais uniquement) est un consommateur externe du flux d'événements.

| Module | Responsabilité | Dépendances clés |
| --- | --- | --- |
| `protocol` | sérialisation binaire des paquets (§6), types, flags, fragmentation/réassemblage | — (`no_std` compatible) |
| `crypto` | Ed25519 (sign/verify), Noise `XX` & `X` (`snow`), scellage d'enveloppes, `recipient_tag`, padding | `ed25519-dalek`, `snow`, `x25519-dalek`, `chacha20poly1305`, `hmac`, `sha2` |
| `identity` | génération/chargement de l'identité, encodage QR, dérivation du code de vérification 60 chiffres | `crypto`, `qrcode` (côté app) |
| `store` | persistance : messages, conversations, contacts, outbox, enveloppes, seen-set, journal | `rusqlite` (natif) / abstraction pour ESP32 |
| `sync` | routage (flood + TTL + jitter), réconciliation gossip (filtres), rejeu d'outbox, **machine à états des statuts**, collecte d'enveloppes | `protocol`, `store`, `crypto` |
| `ledger` | journal append-only chaîné : `append(event) -> Entry`, `verify_chain()`, `export(range)` | `crypto`, `sha2` |
| `observability` | catalogue d'événements typés, JSON canonique, redaction (hachage `msgID`) | `serde`, `ledger` |
| `api` (façade) | surface publique UI + UniFFI : `send_message`, `poll_events`, `on_peer_connected(transport)`, `mark_read`, … | tous les autres |

**Frontières `no_std`** : pour l'ESP32, `protocol`, `sync` (routage/dedup/
gossip), `ledger` doivent compiler en `no_std` + `alloc`. `store` est derrière un
trait `Store` (impl `rusqlite` natif, impl NVS/flash sur ESP32). `crypto` :
décision au spike du Lot 0.

### 5.3 Le trait `Transport` (`powl/02 §3`)

```rust
/// Abstraction d'un lien BLE. Implémentée nativement par plateforme.
pub trait Transport: Send {
    /// Démarre l'annonce du service `dengon` + le scan des pairs.
    fn start(&mut self, cfg: TransportConfig) -> Result<()>;

    /// Événements remontés vers le core (thread-safe, non bloquant).
    ///  - PeerConnected { peer_link_id, rssi }
    ///  - PeerDisconnected { peer_link_id }
    ///  - FrameReceived { peer_link_id, bytes }   // 1 trame BLE (déjà réassemblée L2)
    fn poll(&mut self) -> Vec<TransportEvent>;

    /// Envoie une trame applicative à un pair connecté.
    /// La fragmentation BLE (MTU) est gérée par l'impl ; la fragmentation
    /// protocole (paquets > MTU) est gérée par `protocol`.
    fn send(&mut self, peer_link_id: LinkId, bytes: &[u8]) -> Result<()>;

    /// Diffusion best-effort à tous les pairs connectés (pour ANNOUNCE, gossip).
    fn broadcast(&mut self, bytes: &[u8]) -> Result<()>;
}
```

| Impl | Fichier | Techno |
| --- | --- | --- |
| Android | `android/.../ble/AndroidTransport.kt` + pont JNI dans `dengon-ffi` | `BluetoothGattServer`, `BluetoothLeScanner`, `BluetoothGatt` |
| Desktop / CLI | `crates/dengon-ble/src/btleplug_transport.rs` | `btleplug` (BlueZ / CoreBluetooth / WinRT) |
| ESP32 | `firmware/dengon-relay/src/transport_nimble.c` (+ FFI vers la lib core) | NimBLE (ESP-IDF) |

**Rôle GATT** : chaque nœud est **serveur ET client**. Service `dengon` (UUID
fixe), 2 caractéristiques : `RX` (write-without-response, pair → nœud), `TX`
(notify, nœud → pair).

### 5.4 Découpage en couches — vision `olivier` (`olivier/architecture §4-6`)

De la radio vers l'utilisateur :

1. **Application** : conversations, contacts, statuts, réglages (mode éco),
   notifications — *téléphone uniquement*.
2. **Sécurité** : clés, chiffrement/déchiffrement, signatures, vérification de
   contact — *téléphone déchiffre ; ESP32 ne déchiffre pas*.
3. **Moteur dengon (protocole)** : circulation, déduplication, TTL, file de
   retransmission, réassemblage, accusés — *commun téléphone + ESP32*.
4. **Transport BLE** : découverte des voisins, connexion, envoi/réception de
   trames — *commun, mais API différentes par plateforme*.
5. **Stockage local** : messages, contacts, clés, file, table « déjà vu » —
   *téléphone : base chiffrée ; ESP32 : mémoire réduite*.

**Commun téléphone ↔ ESP32** : le **format de trame** et la **logique du moteur**
(objectif : même comportement des deux côtés). **ESP32** : pas d'UI, ne déchiffre
pas, moteur allégé, forte contrainte mémoire → file plus petite, table « déjà
vu » plus courte. **Téléphone** : UI complète, déchiffrement/affichage, stockage
riche, notifications, service de fond avec notification permanente.

**Chemin d'un message (`olivier/architecture §6`)** : Alice (app) saisit →
Sécurité chiffre pour Bob, signe → Moteur crée la trame (ID, empreintes, TTL,
horodatage), statut « En attente » → Transport BLE pousse aux voisins → « Parti ».
ESP32 relais : Transport BLE reçoit → Moteur : pas destinataire, « déjà vu » ?
non ; TTL 7→6 ; met en file ; réémet. Bob (app) : reçoit → Moteur : destinataire
→ réassemble → Sécurité : déchiffre, vérifie signature → Application : affiche,
déclenche l'ACCUSÉ → Moteur : crée la trame ACCUSÉ (chiffrée pour Alice) et la
diffuse. ACCUSÉ : remonte Bob → ESP32 (qui le voit passer → retire le message de
sa file) → Alice → statut « Distribué ».

### 5.5 Découpage du dépôt (`powl/02 §4`)

```text
dengon/
├── crates/
│   ├── dengon-core/          # lib Rust — LE protocole
│   │   ├── src/{protocol,crypto,identity,store,sync,ledger,observability,api}.rs
│   │   └── tests/            # tests unitaires + property tests
│   ├── dengon-ble/           # trait Transport + impl btleplug
│   ├── dengon-node/          # binaire CLI : nœud headless (tests, PC fixe, bootstrap)
│   │   └── src/main.rs       # `dengon-node run --name alice --db ./alice.db`
│   ├── dengon-sim/           # simulateur multi-nœuds (transport in-memory, partitions)
│   └── dengon-ffi/           # bindings UniFFI (génère Kotlin + Swift)
├── android/                  # app Kotlin + Jetpack Compose
│   └── app/src/main/{java,kotlin}/…/{ble,ui,service}/
├── firmware/
│   └── dengon-relay/         # ESP-IDF (C) + NimBLE + client MQTT/TLS + lib core statique
│       ├── main/
│       └── components/dengon_core_ffi/
├── dashboard/
│   ├── api/                  # Axum : consumer MQTT + REST + WebSocket + vérif hash-chain
│   ├── web/                  # React + TS + Vite
│   └── deploy/               # docker-compose.yml, Caddyfile, migrations SQL
├── docs/powl/                # conception
└── Cargo.toml                # workspace Rust (crates/* + dashboard/api)
```

### 5.6 Déploiement (`powl/02 §5`)

Terrain (hors ligne) : `dengon-app` (Android), `dengon-node` (PC fixe),
`dengon-relay` (ESP32-WROVER). VPS (Docker Compose) : `caddy` (TLS,
reverse-proxy), `mosquitto` (MQTT 8883), `dengon-dashboard-api` (Axum :8080),
`timescaledb` (:5432), `web` (nginx static).

- Le VPS n'a **aucune** connexion sortante vers le terrain. Il **reçoit** seulement.
- Perte du VPS = perte de l'observabilité, **zéro impact** sur la messagerie.
- Relais authentifiés par **mTLS** (cert client par relais) ou JWT signé ;
  clients opt-in par token éphémère.

### 5.7 Choix transverses (`powl/02 §7`)

| Sujet | Choix | Note |
| --- | --- | --- |
| Langage cœur | Rust (edition 2021, MSRV figée) | audit unique |
| Async | `tokio` côté `dengon-node`/dashboard ; core = **sync + boucle d'événements** (portable ESP32) | le core n'impose pas de runtime |
| Sérialisation événements | JSON canonique (clés triées) pour la signature ; stockage tel quel | déterminisme de signature |
| Sérialisation paquets | binaire maison (§6) | compacité BLE |
| Base locale | SQLite (`rusqlite`, `WAL`) | chiffrée au repos via SQLCipher (option) ou champ-par-champ |
| ID de log | `msgID` haché (`SHA-256(msgID_brut)[:16]`) avant tout envoi au VPS | anti-corrélation |
| Versionnement protocole | champ `version` dans chaque paquet + négociation à l'ANNOUNCE | montée de version progressive |

---

## 6. Protocole réseau

> **`docs/powl/03-network-protocol.md` est la source de vérité** du format
> binaire et des règles de routage. Toute autre partie (firmware, dashboard)
> recopie ces constantes à l'identique. Le comportement décrit par
> `olivier/protocole.md` v0.3 est intégré à la fin (§6.7) ; les deux specs
> binaires divergent → voir [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md)
> §A-12.

### 6.1 Couches

```text
┌─────────────────────────────────────────────┐
│ L4  Application : Message, Ack, ReadReceipt  │  ← contenu chiffré (Noise)
├─────────────────────────────────────────────┤
│ L3  Enveloppe dengon : Packet (signé)       │  ← routage, TTL, dedup, gossip
├─────────────────────────────────────────────┤
│ L2  Fragmentation protocole (paquet > MTU)   │
├─────────────────────────────────────────────┤
│ L1  BLE GATT (RX/TX characteristics)         │
└─────────────────────────────────────────────┘
```

### 6.2 Constantes (`protocol::consts`)

| Nom | Valeur | Sens |
| --- | --- | --- |
| `PROTO_VERSION` | `1` | version courante |
| `SERVICE_UUID` | `6d656e67-2d64-656e-676f-6e2d76310000` (`"meng-den gon-v1"`) | service GATT `dengon` |
| `CHAR_RX_UUID` | `…0001` | write-without-response, pair → nœud |
| `CHAR_TX_UUID` | `…0002` | notify, nœud → pair |
| `TTL_DEFAULT` | `7` | sauts au départ |
| `TTL_CLAMP_DENSE` | `5` | si ≥ `DENSE_LINKS` voisins |
| `DENSE_LINKS` | `6` | seuil de densité |
| `THIN_LINKS` | `2` | seuil « chaîne fine » (relais à profondeur pleine) |
| `RELAY_JITTER_MS` | `10..=220` | délai aléatoire avant relais |
| `SEEN_SET_CAP` | `1024` | entrées LRU |
| `SEEN_TTL_S` | `300` | expiration d'une entrée |
| `FRAG_SIZE` | `440` | octets de payload par fragment |
| `FRAG_TIMEOUT_S` | `30` | abandon du réassemblage |
| `FRAG_MAX_CONCURRENT` | `64` | assemblages simultanés |
| `MSG_TTL_S` | `86_400` | durée de vie applicative d'un message (24 h) |
| `ENVELOPE_MAX_BYTES` | `4096` | taille max d'une enveloppe scellée (texte court) |
| `COPY_BUDGET_INIT` | `4` | copies initiales d'une enveloppe |
| `COPY_BUDGET_MAX` | `8` | plafond |
| `PAD_BUCKETS` | `[256, 512, 1024, 2048]` | tailles cibles des paquets chiffrés |
| `ANNOUNCE_ISOLATED_S` | `4` | période d'ANNOUNCE si seul |
| `ANNOUNCE_CONNECTED_S` | `15..=30` | période d'ANNOUNCE si connecté (jitter) |

### 6.3 Format du paquet L3 (tous entiers **big-endian**)

```text
 offset  taille  champ
 ------  ------  --------------------------------------------------
   0       1     version            (= PROTO_VERSION)
   1       1     type               (cf. §6.4)
   2       1     ttl                 (décrémenté à chaque relais)
   3       1     flags              (bitfield, cf. §6.3.1)
   4       8     timestamp_ms        (UTC ms, horloge de l'émetteur)
  12       8     sender_id           (peerID = SHA-256(pub_static)[0..8])
  20      [8]    recipient_id        (présent SSI flags.ADDRESSED)
  20|28    2     payload_len         (N)
  22|30    N     payload             (cf. §6.4 selon type)
  +N      [64]   signature           (présent SSI flags.SIGNED ; Ed25519 sur
                                      octets [0 .. début_signature])
```

En-tête : **22 o** (broadcast) ou **30 o** (adressé). Avec signature : **+64 o**.

**§6.3.1 `flags` (bitfield)**

| bit | nom | sens |
| --- | --- | --- |
| 0 | `ADDRESSED` | `recipient_id` présent ; paquet à destination d'un `peerID` précis |
| 1 | `SIGNED` | signature Ed25519 en fin de paquet |
| 2 | `FRAGMENT` | le payload est un fragment (cf. §6.5) |
| 3 | `RELAY_OK` | l'émetteur autorise le relais (0 = strictement local, 1 saut) |
| 4 | `PADDED` | payload complété par du PKCS#7 vers un `PAD_BUCKET` |
| 5-7 | réservé | 0 |

**§6.3.2 `msgID` (identifiant de contenu)**

```text
msgID = SHA-256( sender_id ‖ timestamp_ms ‖ type ‖ payload )   // 32 o
```

Sert de clé de **déduplication** (via le seen-set) ; de **référence de suivi**
(l'app l'affiche ; le dashboard le trace **haché** :
`msgID_log = SHA-256(msgID)[0..16]`) ; rend le paquet **idempotent**.

### 6.4 Types de paquets

| `type` | nom | `ADDRESSED` | `SIGNED` | payload |
| --- | --- | --- | --- | --- |
| `0x01` | `ANNOUNCE` | non | oui | `peerID ‖ pub_static(32) ‖ pub_sign(32) ‖ pseudo_len(1) ‖ pseudo ‖ ledger_height(8) ‖ caps(1)` |
| `0x02` | `NOISE_HS` | oui | non | message de handshake Noise `XX` (1 des 3) |
| `0x03` | `NOISE_MSG` | oui | non | ciphertext Noise (transport) ; en clair : un `AppFrame` (§6.4.1) |
| `0x04` | `SEALED_ENVELOPE` | non | oui | `recipient_tag(16) ‖ epoch_day(2) ‖ noise_x_ciphertext` ; en clair : `AppFrame` |
| `0x05` | `ACK` | oui | non | (dans session Noise) `AckFrame` (§6.4.1) |
| `0x06` | `GOSSIP_FILTER` | oui | oui | `kind(1) ‖ golomb_coded_set` — résumé compact des `msgID` connus |
| `0x07` | `GOSSIP_PULL` | oui | oui | `count(2) ‖ msgID[count]` — demande explicite de paquets |
| `0x08` | `GOSSIP_PUSH` | oui | non | `count(2) ‖ (len(2) ‖ packet)[count]` — paquets bruts ré-encapsulés |
| `0x09` | `FRAGMENT` | hérite | non | cf. §6.5 |
| `0x0A` | `LOG_ATTEST` | non | oui | `ledger_root(32) ‖ height(8) ‖ node_pub_sign(32)` — attestation de journal (diffusée, captée par relais → VPS) |
| `0x0B` | `ENVELOPE_OFFER` | oui | oui | `count(2) ‖ recipient_tag(16)[count]` — « je porte ces enveloppes » |
| `0x0C` | `ENVELOPE_REQUEST` | oui | oui | `count(2) ‖ recipient_tag(16)[count]` — « donne-les moi » |

**§6.4.1 Frames applicatives (L4, à l'intérieur de Noise)**

```text
AppFrame  = kind(1) ‖ body
  kind 1 : Message   -> msg_uuid(16) ‖ conv_seq(8) ‖ sent_ms(8) ‖ text_utf8
  kind 2 : Ack       -> AckFrame
  kind 3 : ReadRcpt  -> msg_uuid(16) ‖ read_ms(8)
  kind 4 : Profile   -> pseudo, avatar_hash… (post-MVP)

AckFrame  = msg_uuid(16) ‖ status(1) ‖ at_ms(8)
  status : 2=distribué (reçu par l'appareil destinataire)
```

- `msg_uuid` : UUIDv4 tiré par l'émetteur, **stable de bout en bout** ; c'est lui
  que la machine à états des statuts suit (le `msgID` L3 change si le paquet est
  re-scellé).
- `conv_seq` : compteur par conversation → détection de trous, ordre causal.

### 6.5 Fragmentation protocole (L2)

Nécessaire quand un paquet L3 dépasse le MTU ATT négocié (souvent 185–244 o
utiles, jusqu'à 517 si négocié). Le **texte court** tient généralement en 1
paquet ; la fragmentation sert surtout aux `GOSSIP_PUSH` et aux enveloppes.

```text
Fragment payload (dans un paquet type=0x09, flags.FRAGMENT) :
  frag_id(8)     = SHA-256(paquet_complet)[0..8]
  index(2)
  total(2)
  chunk(<= FRAG_SIZE)
```

Réassemblage : buffer par `frag_id`, complété quand les `total` chunks sont là.
`FRAG_TIMEOUT_S` d'inactivité → abandon. `FRAG_MAX_CONCURRENT` dépassé → on jette
le plus ancien. Un fragment **n'est pas signé** individuellement ; la signature
est dans le paquet reconstruit.

### 6.6 Couche BLE (L1) & routage

**Rôle double** : chaque nœud, en permanence, **Peripheral** (publie
`SERVICE_UUID`, expose `CHAR_RX` write-w/o-response + `CHAR_TX` notify ; annonce
un *manufacturer data* court = `peerID[0..4] ‖ flags`) **ET Central** (scanne le
`SERVICE_UUID`, se connecte aux nouveaux `peerID`, s'abonne à leur `CHAR_TX`).
**Règle anti-boucle de connexion** : quand A et B se découvrent, **celui dont le
`peerID` est le plus petit** initie la connexion GATT.

**MTU** : négocier `ATT_MTU = 517` à la connexion ; retomber sur 23 (→ 20 o
utiles) si refus.

**Routage — à la réception d'un paquet** (`powl/03 §7.1`) :

```
paquet reçu
  → version OK ? + signature OK (si SIGNED) ? — non → jeter + log pkt.rejected
  → msgID dans seen-set ? — oui → jeter (doublon)
  → seen-set.insert(msgID)
  → ADDRESSED && recipient_id == moi ? — oui → traiter localement
       (handshake / message / ack / gossip)  [DELIVER → aussi STORE]
  → sinon : RELAY_OK && ttl > 1 ?
       — non → si SEALED_ENVELOPE : stocker (dépôt) ; sinon : fin
       — oui → ttl' = min(ttl-1, clamp(densité))
            → attendre RELAY_JITTER_MS
            → reçu en double pendant l'attente ? — oui → abandonner le relais
            — non → broadcast(paquet, ttl') + log pkt.relayed
```

- **Directed traffic** (`NOISE_HS`, `NOISE_MSG`, `ACK`, `SEALED_ENVELOPE`,
  `GOSSIP_*`) : relais déterministe `ttl-1`, jitter serré, pas de fanout partiel
  (rediffusion à tous les pairs sauf la source).
- **Broadcast** (`ANNOUNCE`, `LOG_ATTEST`) : idem mais TTL faible (2–3).

**Réconciliation gossip (à chaque nouveau pair)** : après ANNOUNCE + handshake,
(1) chacun envoie un `GOSSIP_FILTER` (Golomb-Coded Set des `msgID` qu'il détient :
messages publics récents, ACK non confirmés livrés, enveloppes) ; (2) chacun
calcule ce que l'autre ne semble pas avoir → `GOSSIP_PULL` ou directement
`GOSSIP_PUSH` si peu ; (3) l'autre répond `GOSSIP_PUSH` avec les paquets bruts ;
(4) les paquets poussés repassent par le pipeline (re-dédup, re-vérif signature,
éventuel re-relais). **GCS choisi plutôt qu'un Bloom filter** : ~20–30 % plus
compact à même taux de faux-positifs. Paramètre `p = 1/64`.

**Collecte d'enveloppes scellées (à chaque rencontre)** : (1) le porteur envoie
`ENVELOPE_OFFER` (liste de `recipient_tag`) ; (2) le pair calcule **ses** tags du
jour (± 1 jour de fenêtre) et compare ; (3) match → `ENVELOPE_REQUEST` → le
porteur renvoie le `SEALED_ENVELOPE` ; (4) le destinataire déchiffre (Noise `X`),
traite, émet un `ACK` ; (5) **budget de copies** : quand deux porteurs d'une même
enveloppe se rencontrent, ils se partagent la moitié du budget restant
(Spray-and-Wait) ; budget épuisé + `MSG_TTL_S` dépassé → suppression.

**Pseudo-code relais minimal (`oswin/01 §4`)** — même logique, formulée simplement :

```text
à la réception d'un paquet P (venant d'un voisin) :
    si P.id est dans mon cache "déjà vu" : ignorer
    sinon :
        ajouter P.id au cache "déjà vu"
        si P.destinataire == moi :
            déchiffrer, vérifier la signature, afficher
            émettre un ACK(P.id) dans le mesh
        sinon :
            stocker P dans ma file locale       # store
            P.ttl = P.ttl - 1
            si P.ttl > 0 et P non expiré :
                pour chaque voisin BLE actuel : envoyer P   # forward
```

Deux garde-fous indispensables (théorie du *broadcast storm*) : **cache des
messages vus** (évite boucles infinies et effondrement) ; **TTL** (en sauts **et**
en temps) garantit qu'un message finit par disparaître.

### 6.7 Anti-abus / robustesse (`powl/03 §8`)

| Menace | Contre-mesure |
| --- | --- |
| Rejeu de paquets | `msgID` + seen-set ; `timestamp_ms` hors fenêtre ±2 h → rejeté |
| Boucle de flood | seen-set + TTL + jitter + abandon si doublon pendant l'attente |
| Paquet forgé (usurpation d'émetteur) | `ANNOUNCE`/`GOSSIP`/`ENVELOPE`/`LOG_ATTEST` signés ; `peerID` doit matcher `SHA-256(pub_static)` |
| Flood volontaire (DoS) | quota par `peerID` et par lien (paquets/s) ; RSSI-gating ; au plus `TTL_DEFAULT` copies relayées |
| Épuisement mémoire (fragments / enveloppes) | caps stricts (`FRAG_MAX_CONCURRENT`, `ENVELOPE` par nœud), éviction LRU |
| Analyse de trafic par un relais | padding `PAD_BUCKETS` ; `recipient_tag` tournant ; pas de `recipient_id` sur les enveloppes |
| Horloge fausse d'un nœud | fenêtre de tolérance ; le dashboard signale les dérives (`node.clock_skew`) |

### 6.8 Cycle de vie d'un message stocké (`oswin/03 §4-5`)

```text
                 message reçu par un relais
                          │
                 [ FILE DE STOCKAGE LOCALE ]  (clé = id_message)
                          │
          ┌───────────────┼─────────────────────────────┐
   nouveau voisin     TTL/lifetime            ACK reçu pour ce message
   → retransmettre    → SUPPRIMER (expiration) → SUPPRIMER (purge → libère mémoire)
```

Trois déclencheurs de suppression : **expiration** (TTL sauts épuisé **ou**
*lifetime* temporel dépassé) ; **ACK** signé du destinataire qui repart dans le
mesh (chaque relais purge sa copie) ; **quota mémoire** (drop-oldest /
drop-most-forwarded).

Détails d'implémentation à ne pas oublier : **déduplication** (index par
`id_message` + cache d'IDs vus) ; **persistance** de la file sur disque/flash
(survie au redémarrage, surtout l'Arduino) ; **fragmentation** (message > MTU) ;
**anti-tempête** (temporiser aléatoirement les retransmissions — *jitter*) ;
**sécurité de l'ACK** (signé par le destinataire, sinon un relais malveillant
efface des messages en forgeant de faux ACK).

### 6.9 Comportement du protocole `olivier` v0.3 (`olivier/protocole.md`)

Périmètre v1 `olivier` : messages **1-à-1**, texte court **~140 à 500
caractères**, statuts *En attente → Parti → Distribué* (**Lu reporté en v2**).

**Trois natures de PDU** : **DONNÉES** (transporte un message/fragment) ;
**ACCUSÉ** (remonte une confirmation vers l'expéditeur) ; **INVENTAIRE** (liste
d'identifiants échangée entre voisins qui se rencontrent).

**À la réception d'une trame DONNÉES** :
1. **Déjà vu ?** ID dans la table « déjà vu » → ignorer.
2. **Message expiré ?** `maintenant − horodatage` > ~24 h → ignorer + purger les
   fragments gardés.
   2 bis. **Limite anti-inondation** : si ce voisin a envoyé > **~20 nouveaux
   messages sur la dernière minute** (à calibrer), ignorer les suivants venant de
   lui jusqu'à ce que le débit retombe.
3. **Réassemblage** : si fragmenté, mettre le fragment de côté ; s'arrêter tant
   que tous les fragments ne sont pas là.
4. **Suis-je le destinataire ?** Oui → déchiffrer, afficher, **émettre un
   ACCUSÉ**, noter l'ID en « déjà vu ». Non → relayage.
5. **Relayage** : si `sauts restants = 0` → ne pas relayer ; sinon décrémenter,
   mettre en **file de retransmission**, réémettre à tous les voisins **sauf
   celui qui vient de nous l'envoyer**. **Écouter avant de rediffuser** : avant
   de réémettre, attendre un court délai aléatoire (**~50 à ~500 ms**) ; si on
   entend un voisin rediffuser **déjà** ce message, **s'abstenir** (idée reprise
   de Meshtastic).
6. Noter l'ID dans la table « déjà vu » (avec l'heure, oubli après ~24 h).

**Conditions d'arrêt** : TTL épuisé ; déjà vu ; expiration (> ~24 h) ; **accusé
passé par là** (si un ACCUSÉ pour ce message transite par le nœud, le message est
retiré de la file).

**Accusés** : le destinataire crée une trame ACCUSÉ contenant l'ID du message, le
statut `DISTRIBUÉ`, une **signature du destinataire** sur (ID + statut + heure),
le tout **chiffré pour l'expéditeur d'origine**. L'ACCUSÉ **se diffuse comme un
message** (son propre TTL, sa propre dédup). **Optimisation** : tout relais qui
voit passer un ACCUSÉ pour X retire X de sa file. À réception, l'expéditeur passe
à **Distribué**.

**Correspondance avec les statuts affichés** : *En attente* = message créé, pas
encore transmis à un voisin ; *Parti* = transmis à ≥ 1 voisin ; *Distribué* =
ACCUSÉ reçu du destinataire ; *Échec* = > ~24 h sans ACCUSÉ ; *Lu* = v2 (2ᵉ type
d'accusé).

**Ordre d'affichage** : en v1, afficher **dans l'ordre où le téléphone les
reçoit** (pas de retri par horodatage). Un message très en retard apparaît en bas.

**Découpage/réassemblage** : fragments de la taille négociée avec le voisin,
chacun portant ID + index + nombre total. Délai max d'attente ~**30 s**. En v1,
un nœud **rassemble le message complet avant de le relayer**. Ordre de grandeur :
un message de 500 caractères tient en **2 à 6 fragments** selon la liaison.

**Store-and-forward** : la **file de retransmission** garde les messages pas
encore connus comme distribués. **Quand un nouveau voisin apparaît**, les deux
nœuds **échangent d'abord la liste des identifiants qu'ils détiennent**
(« inventaire »), puis chacun n'envoie que ce qui **manque** à l'autre.
**Taille maximale de la file** : **~50 messages sur ESP32**, **~300 sur
téléphone** (à calibrer) ; file pleine → retirer le **plus ancien**. Un message
sort de la file quand : un ACCUSÉ le concernant passe par le nœud, **ou** il
expire (~24 h), **ou** il est évincé.

**Garanti en v1** : un relais ne peut pas lire le contenu (E2E) ; ne peut pas
modifier sans que ça se voie (étiquette d'authenticité / signature) ; l'expéditeur
affiché détient bien la clé privée correspondante ; un rejeu est reconnu (ID +
« déjà vu »). **Non garanti en v1** : métadonnées (empreintes et horaires en clair
dans l'en-tête → un relais voit *qui parle à qui* et *quand*) ; pas de secret
persistant ; horodatages non vérifiables ; inondation malveillante seulement
atténuée ; analyse de trafic possible.

**Exemple (scénario campus)** : Alice → Bob, Bob hors de portée directe, Carole
entre les deux. (1) Alice crée + chiffre pour Bob, statut « En attente ». (2)
Alice→Carole : Carole reçoit, pas destinataire, sauts restants 7→6, met en file
et rediffuse ; Alice a transmis à un voisin → « Parti ». (3) Carole→Bob : Bob
reçoit, déchiffre, affiche, émet un ACCUSÉ (signé, chiffré pour Alice). (4)
Bob→Carole→Alice : l'ACCUSÉ remonte ; Carole le voit passer → retire le message
de sa file. (5) Alice : ACCUSÉ reçu → « Distribué ».

---

## 7. Format binaire de trame

Deux specs binaires coexistent. **`powl/03 §3` fait foi côté conception cible**
(reproduit au §6.3). **`olivier/format-trame.md` v0.1** (reproduit ci-dessous) est
la spec « fait foi » du brouillon `olivier`. Elles **divergent** (en-tête,
tailles d'identifiants, types de PDU) → arbitrage dans
[`01-sujets-a-trancher.md`](01-sujets-a-trancher.md) §A-12.

### 7.1 Rappel `powl` (§6.3)

En-tête L3 : **22 o** (broadcast) / **30 o** (adressé), `+64 o` signature Ed25519.
`sender_id` = `peerID` 8 o. `msgID` = hash de contenu 32 o (+ `msg_uuid` 16 o
stable de bout en bout, à l'intérieur du chiffré). 12 types de paquets
(`0x01`–`0x0C`). Fragmentation L2 : en-tête `frag_id(8) ‖ index(2) ‖ total(2)`,
`FRAG_SIZE = 440`.

### 7.2 Spec `olivier/format-trame.md` v0.1 (« fait foi » côté olivier)

**Conventions** : ordre des octets **big-endian** (réseau) pour tout entier
multi-octets ; entiers non signés sauf mention ; tailles en octets ; champ
« réservé » = 0 à l'émission, ignoré à la réception.

**Deux niveaux** : le **PDU dengon** = unité complète (en-tête + corps), avant
découpage — c'est ce qui est chiffré, relayé, dédupliqué. Le **Fragment BLE** =
un morceau de PDU envoyé en un seul envoi Bluetooth (1 à N fragments par PDU).

**Fragment BLE — en-tête = 24 octets**

| Décalage | Taille | Champ | Description |
| ---: | ---: | --- | --- |
| 0 | 1 | `version_proto` | Version du protocole. **= 1**. |
| 1 | 1 | `drapeaux` | bit 0 = corps chiffré ; bits 1-7 réservés. |
| 2 | 16 | `id_message` | Identifiant **unique et stable** du message (128 bits aléatoires). Déduplication + corrélation de l'accusé. **Jamais modifié par un relais.** |
| 18 | 2 | `index_fragment` | Numéro du fragment, à partir de 0. |
| 20 | 2 | `nombre_fragments` | Total de fragments du PDU (≥ 1). |
| 22 | 2 | `longueur_charge` | Nombre d'octets utiles dans ce fragment. |
| 24 | `longueur_charge` | `charge_fragment` | Tranche du PDU dengon. |

Avec une taille utile BLE négociée d'environ **180 octets**, il reste ~**156
octets** de charge par fragment (**à calibrer**). Recollage : concaténer les
`charge_fragment` dans l'ordre des `index_fragment`. Délai max d'attente des
fragments manquants : **~30 s** ; au-delà, on jette.

**PDU dengon — en-tête commun = 44 octets** (obtenu après recollage)

| Décalage | Taille | Champ | Description |
| ---: | ---: | --- | --- |
| 0 | 1 | `type_pdu` | **1** = DONNÉES, **2** = ACCUSÉ, **3** = INVENTAIRE. |
| 1 | 1 | `sauts_restants` | TTL. **7** à l'émission. −1 par relais. À 0, on ne relaie plus. |
| 2 | 16 | `empreinte_expediteur` | Empreinte de clé publique de l'émetteur du PDU. |
| 18 | 16 | `empreinte_destinataire` | Empreinte de clé publique du destinataire du PDU. |
| 34 | 8 | `horodatage_envoi` | Millisecondes depuis 1970-01-01 UTC. Best-effort : affichage + expiration (~24 h). |
| 42 | 1 | `version_contenu` | Version du format du corps. **= 1**. |
| 43 | 1 | réservé | 0. |
| 44 | … | `corps` | Dépend de `type_pdu`. |

> Adressage : pour un PDU **DONNÉES**, `empreinte_expediteur` = l'auteur du
> message, `empreinte_destinataire` = son lecteur. Pour un PDU **ACCUSÉ**, c'est
> **l'inverse**.

**Corps d'un PDU DONNÉES (`type_pdu` = 1)** — bit « chiffré » des `drapeaux` = 1

| Décalage (corps) | Taille | Champ | Description |
| ---: | ---: | --- | --- |
| 0 | 24 | `nonce` | Aléa unique par PDU pour le chiffrement authentifié. |
| 24 | 16 | `etiquette_auth` | Code d'authentification (empêche lecture *et* modification par un relais). |
| 40 | M | `contenu_chiffre` | Le texte du message, chiffré. |

Une fois déchiffré : **texte en clair UTF-8** (≤ ~500 caractères, ≤ ~1000
octets). Primitive envisagée : **libsodium `crypto_box`** (X25519 + chiffrement
authentifié). Disposition exacte `nonce`/`etiquette_auth`/`contenu_chiffre` **à
figer dans la doc sécurité**.

**Corps d'un PDU ACCUSÉ (`type_pdu` = 2)** — a **son propre** `id_message`,
chiffré pour l'auteur d'origine, corps = `nonce(24) ‖ etiquette_auth(16) ‖
contenu_chiffre(P)`. Contenu de l'accusé **en clair** après déchiffrement :

| Décalage | Taille | Champ | Description |
| ---: | ---: | --- | --- |
| 0 | 16 | `id_message_confirme` | L'`id_message` du PDU DONNÉES acquitté. |
| 16 | 1 | `statut` | **3** = DISTRIBUÉ (**4** = LU, réservé v2). |
| 17 | 8 | `horodatage_reception` | Millisecondes UTC de la réception par le destinataire. |
| 25 | 64 | `signature_destinataire` | Signature de (`id_message_confirme` ‖ `statut` ‖ `horodatage_reception`) par la clé du destinataire. |

**Corps d'un PDU INVENTAIRE (`type_pdu` = 3)** — échangé quand deux voisins se
rencontrent, **non chiffré** (bit « chiffré » = 0), corps =
`nombre_entrees(2) ‖ ids(16 × nombre_entrees)`. À réception, le nœud envoie à ce
voisin les messages dont l'`id_message` **n'apparaît pas** dans la liste reçue.
Ordre de grandeur : une file pleine côté téléphone (~300 messages) = ~4,8 Ko
d'identifiants, ~30 fragments (représentation compacte type filtre de Bloom
notée comme optimisation).

**Constantes du protocole `olivier` (v0.3)**

| Constante | Valeur | Note |
| --- | --- | --- |
| `version_proto` | 1 | |
| `version_contenu` | 1 | |
| TTL initial (`sauts_restants`) | 7 | aligné Meshtastic |
| Taille de `id_message` | 16 octets (128 bits) | aléatoire |
| Taille max du texte en clair | ~500 caractères (~1000 octets) | `decisions-v1.md` |
| Charge utile par fragment | ~156 octets | à calibrer selon MTU |
| Délai de réassemblage | ~30 s | à calibrer |
| Expiration d'un message | ~24 h | `protocole.md` §6 |
| Délai « écouter avant de rediffuser » | ~50-500 ms | à calibrer |
| Fenêtre anti-inondation | ~20 nouveaux `id_message` / min / voisin | à calibrer |
| Taille de la file de retransmission | ~50 (ESP32) / ~300 (téléphone) | à calibrer |

**Exemple de tailles — message texte de 500 caractères**

```text
texte en clair              ~700 octets (UTF-8 courant, jusqu'à ~1000 au pire)
+ nonce (24) + étiquette (16)   40 octets
= contenu chiffré           ~740 octets
+ en-tête PDU                 44 octets
= PDU DONNÉES               ~784 octets
découpage en fragments de ~156 octets utiles :  ⌈784 / 156⌉ = 6 fragments
                                        (jusqu'à ~7 dans le pire cas)
```

**Points ouverts de `format-trame.md`** : taille utile réelle d'un fragment
(dépend du MTU BLE négocié) ; disposition exacte `nonce`/`étiquette`/`contenu`
selon la primitive crypto définitive (doc sécurité) ; champ de somme de contrôle
par fragment ou repos sur le CRC BLE ? ; représentation compacte de l'inventaire
(filtre de Bloom) : v1 ou évolution ? ; numéroter les PDU d'un même expéditeur
(compteur anti-rejeu explicite) en plus de l'`id_message` ?

### 7.3 Proposition de format `oswin` (`oswin/02 §3`) — pour mémoire

```text
EN-TÊTE EN CLAIR (lisible par les relais, pour router)
  - id_message        (aléatoire, unique)
  - ttl               (nb de sauts restants)
  - expire_le         (horodatage d'expiration)
  - dest_pubkey_hash  (à qui, sous forme pseudonyme)
  - exp_pubkey_ephem  (clé publique éphémère de l'émetteur)
CHARGE UTILE CHIFFRÉE (AES-256-GCM) — illisible par les relais
  - texte du message
  - numéro de séquence (anti-rejeu)
SIGNATURE Ed25519 sur (en-tête + charge utile)
```

Points d'attention : l'en-tête **doit** être couvert par l'authentification (AEAD
*additional data* ou signature) pour qu'un relais ne puisse pas trafiquer le TTL
ou le destinataire ; numéro de séquence + ID unique bloquent le rejeu ; garder
l'en-tête **minimal** (chaque champ en clair est une fuite de métadonnées).

---

## 8. Sécurité

### 8.1 Modèle de menace (`powl/04 §1`)

**Ce qu'on protège** : contenu des messages (confidentialité + intégrité +
authenticité) ; qui parle à qui (minimisation — relais et VPS ne doivent pas
savoir) ; statuts/accusés (intégrité + authenticité, pas de faux « lu ») ;
journal d'activité d'un nœud (infalsifiabilité *a posteriori*) ; identité d'un
contact (non-usurpable, vérifiable hors bande).

| Attaquant | Capacités supposées | Traité par |
| --- | --- | --- |
| **Relais malveillant** (ESP32 compromis / faux relais) | voit tout le trafic BLE qui passe, peut jeter/dupliquer/réordonner, peut mentir au VPS | E2E Noise, padding, tags tournants, journal chaîné signé (le VPS recoupe) |
| **VPS compromis / admin curieux** | voit tous les logs, la BDD, peut les modifier | aucune clé privée ni clair ne transite ; `msgID` haché ; signatures vérifiables côté client |
| **Voisin BLE passif** | sniffe l'air | chiffrement + padding ; `peerID` stable mais pseudonyme |
| **Attaquant actif MITM au premier contact** | intercepte l'échange de clés | vérification **QR + code 60 chiffres** hors bande ; TOFU sinon |
| **Vol de l'appareil déverrouillé** | accès à la base locale | chiffrement au repos (SQLCipher / Keystore) ; *panic wipe* (post-MVP) |
| **Attaquant réseau global** | corrèle les métadonnées à grande échelle | **hors périmètre MVP** (documenté comme limite) |

**Hors périmètre (assumé)** : adversaire étatique faisant de l'analyse de trafic
mondiale ; déni de service radio (brouillage BLE) ; forward secrecy des
enveloppes scellées ; sécurité physique de l'ESP32 (extraction de sa clé de
relais → au pire, faux logs, jamais de déchiffrement).

**Les 4 propriétés visées** (`oswin/02 §1`) : confidentialité (chiffrement) ;
intégrité (AEAD / hash / signature) ; authenticité (signature) ; non-rejeu (n° de
séquence / horodatage + ID). Souvent complétées par la **vie privée / anonymat**
et le **secret persistant (forward secrecy)**.

### 8.2 Identité & clés (`powl/04 §2`)

Chaque installation génère au premier lancement et stocke dans le coffre de la
plateforme (Android Keystore / Secret Service / NVS chiffrée ESP32) :

| Clé | Algo | Usage |
| --- | --- | --- |
| `static` | X25519 (Curve25519) | accord de clés Noise (`XX` et `X`) |
| `sign` | Ed25519 | signature des paquets `ANNOUNCE`, `GOSSIP_*`, `SEALED_ENVELOPE`, `LOG_ATTEST` |

```
peerID = SHA-256(pub_static)[0..8]           // 8 octets, affiché en base32
fingerprint = SHA-256(pub_static ‖ pub_sign) // 32 octets, base de la vérification
```

`peerID` **stable** entre redémarrages et réinstallations tant que le coffre
survit ; ne change qu'après régénération explicite (« nouvelle identité »).

**QR code** : `dengon:v1:<base64url( pseudo_len(1) ‖ pseudo ‖ pub_static(32) ‖
pub_sign(32) )>`. Pas de secret.

**Code de vérification (« safety number », 60 chiffres)** — détecter un MITM au
premier contact sans serveur :

```
material = SHA-512( min(fpA, fpB) ‖ max(fpA, fpB) )   // ordre-indépendant
code = 60 chiffres décimaux :
       pour i in 0..12 : groupe_i = ( u16_be(material[i*2 .. i*2+2]) % 100000 )
       affiché : "01234 56789 01234 ..." (12 groupes de 5)
```

Les deux appareils affichent **le même code** ; l'utilisateur compare de visu ou
lit à voix haute. Match → contact marqué **✔ vérifié** (`contacts.verified_at`).
Non-vérifié → **TOFU** (confiance à la première clé vue pour ce `peerID`, alerte
si elle change : `contacts.key_changed`).

**Changement de clé** : si un `ANNOUNCE` présente un `pub_static` différent pour
un `peerID`/pseudo connu → messages livrés **suspendus** vers ce contact ;
bannière UI « la clé de X a changé — re-vérifiez » ; événement
`contact.key_changed` journalisé (chaîne locale).

### 8.3 Chiffrement des messages (`powl/04 §3`)

**Session en direct — Noise `XX`** (`Noise_XX_25519_ChaChaPoly_SHA256`) :
handshake 3 messages (`-> e` / `<- e, ee, s, es` / `-> s, se`) → 2 clés de
transport (A→B, B→A). Authentification mutuelle (chacun prouve la possession de
sa clé `static`) ; **forward secrecy** (clés éphémères `e`) ; la `pub_static`
reçue dans le handshake **doit** correspondre à celle du contact (vérifié ou
TOFU), sinon rejet + alerte ; session réutilisée tant que le lien BLE tient,
re-négociée à la reconnexion ou après `2^n` messages / rekey Noise. **ACK et
read-receipts voyagent dans la session** (chiffrés, authentifiés).

**Message pour destinataire absent — Noise `X` (enveloppe scellée)**
(`Noise_X_25519_ChaChaPoly_SHA256`, one-shot vers la clé statique publique du
destinataire) :

```
SEALED_ENVELOPE.payload = recipient_tag(16) ‖ epoch_day(2) ‖ noise_x_message
noise_x_message chiffre : AppFrame{kind=1, Message{...}} ‖ sender_pub_static(32) ‖ sig
```

Le destinataire déchiffre avec sa clé `static` **hors ligne, plus tard**. Le
paquet `SEALED_ENVELOPE` est **signé Ed25519** par l'expéditeur (le relais vérifie
la signature avant de stocker → anti-pollution). `sender_pub_static` est **à
l'intérieur** du chiffré → un relais ne sait pas qui envoie.

**Adressage anonyme — `recipient_tag`** :

```
recipient_tag(day) = HMAC-SHA256( pub_static_destinataire , "dengon-tag" ‖ day_u32 )[0..16]
```

Change chaque jour → un relais ne peut pas suivre un destinataire dans le temps.
Le destinataire calcule ses propres tags (J-1, J, J+1) et matche les
`ENVELOPE_OFFER`. Un relais qui stocke 100 enveloppes voit 100 tags **non
corrélables**. **Limite assumée** : pas de forward secrecy sur les enveloppes ;
si la clé `static` du destinataire est volée **avant** qu'il ait récupéré une
enveloppe encore en circulation, cette enveloppe est déchiffrable. Mitigation :
`MSG_TTL_S` = 24 h borne la fenêtre. Montée en gamme possible (pré-clés type
X3DH) en v2.

**Padding** : tout paquet `NOISE_MSG` / `NOISE_HS` / `SEALED_ENVELOPE` est
complété (PKCS#7, flag `PADDED`) à la borne supérieure de
`PAD_BUCKETS = [256, 512, 1024, 2048]`. Un message « ok » et un message de 200
caractères sont indistinguables par la taille.

**Primitives modernes (`oswin/02 §2`)** : X25519 (échange, Diffie-Hellman sans
transmettre de secret) ; AES-256-GCM ou ChaCha20-Poly1305 (AEAD — chiffre **et**
protège l'intégrité ; ChaCha souvent préféré sur microcontrôleurs sans
accélération AES) ; Ed25519 (signature — authenticité + non-répudiation). Ce sont
les choix de Bitchat, orchestrés par le cadre **Noise (motif XX)**. **Aller plus
loin (optionnel)** : le **Double Ratchet** de Signal (nouvelle clé par message →
forward secrecy + post-compromise security) — bonus ambitieux pour un projet
d'école, à mentionner/implémenter si le temps le permet.

### 8.4 Journal chaîné signé — la couche « type blockchain » (`powl/04 §4`)

**But** : donner au dashboard (et à un auditeur) une **trace infalsifiable** de
ce que chaque nœud a fait, **sans** que le dashboard puisse la forger et
**sans** consensus. C'est un *hash-chained append-only log*, pas une blockchain
répliquée.

```
Entry {
    seq:        u64          // 0, 1, 2, … monotone, sans trou
    ts_ms:      u64
    event:      string       // nom canonique (cf. §11 catalogue d'événements)
    payload:    object       // champs de l'événement, msgID déjà haché
    prev_hash:  bytes32      // hash de l'entrée seq-1  (00…0 pour seq 0)
}
entry_hash = SHA-256( canonical_json(Entry) )
Record {
    entry:  Entry
    hash:   entry_hash
    sig:    Ed25519( sign_key , entry_hash )
}
```

`canonical_json` : clés triées, pas d'espaces, UTF-8, entiers sans zéro superflu.
Le journal est **local** à chaque nœud (client et relais). `ledger_root` =
`entry_hash` de la dernière entrée = résumé de tout l'historique.

**Attestation (`LOG_ATTEST`)** : périodiquement, chaque nœud **diffuse** en BLE un
paquet signé `{ ledger_root, height, node_pub_sign }`. Les relais le captent et
le remontent au VPS → le VPS voit la « hauteur » revendiquée par chaque nœud,
**corroborée par des tiers** (plusieurs relais rapportent la même racine).

**Vérifications côté dashboard**

| Contrôle | Détecte |
| --- | --- |
| `prev_hash(seq) == hash(seq-1)` sur toute la séquence reçue | **altération** d'une entrée passée |
| `seq` strictement croissante sans trou | **suppression** d'entrées |
| `sig` valide avec `node_pub_sign` | entrée **forgée** par un tiers (ou le VPS) |
| `ledger_root` reçu ≥ précédent, cohérent avec les `LOG_ATTEST` de plusieurs relais | **fork** (le nœud sert deux historiques différents) |
| recoupement : même `msgID_log` rapporté par R1 et R2 avec des `prev_hop` cohérents | relais qui **ment** sur un relais |

Une rupture → alerte `integrity.chain_broken` / `integrity.fork_detected`, nœud
marqué « suspect ». **Le dashboard ne peut pas réparer ni réécrire** un journal :
il constate.

**Ce que le journal ne fait pas** : pas d'ordre total entre nœuds (chaque journal
indépendant) ; pas de preuve que le nœud a **tout** journalisé (il peut omettre
*avant* de signer seq n) — d'où le recoupement multi-relais comme garde-fou ;
pas de valeur / pas de double-dépense → pas de consensus nécessaire.

### 8.5 Sécurité des relais ESP32 (`powl/04 §5`)

| Aspect | Choix |
| --- | --- |
| Clé de relais | paire Ed25519 propre, générée à la prod, stockée en **NVS chiffrée** (eFuse flash encryption activé) |
| Enregistrement | `pub_sign` du relais déclarée au dashboard (liste blanche) ; mTLS pour MQTT |
| Compromission physique | **aucune clé utilisateur** sur le relais → pas de déchiffrement possible. Au pire : faux logs (détectés par recoupement), rétention/drop de paquets (le réseau route autour) |
| Mise à jour firmware | OTA signé (vérifié par le bootloader) — post-MVP |
| Secure Boot | activé sur les cartes de prod |

### 8.6 Sécurité du dashboard (`powl/04 §6`)

| Aspect | Choix |
| --- | --- |
| Données au repos | aucun clair de message ; `msgID` haché ; pseudos affichés seulement si le nœud les publie (opt-in) |
| Transport | TLS partout (Caddy + Let's Encrypt) ; MQTT sur 8883 TLS |
| Auth relais | mTLS (cert client par relais) **ou** JWT signé court |
| Auth opérateurs | login + argon2id + TOTP ; rôles `viewer` / `admin` |
| Injection | le dashboard **n'a aucun chemin d'écriture vers le terrain** ; API terrain = *ingest only* |
| Rétention | événements purgés après N jours (config, défaut 90) ; agrégats conservés |
| Audit interne | actions opérateurs journalisées |

### 8.7 Cryptographie — choix de primitives (`powl/04 §7`)

| Fonction | Primitive | Crate Rust |
| --- | --- | --- |
| Signature | Ed25519 | `ed25519-dalek` v2 |
| Accord de clés | X25519 | `x25519-dalek` |
| Framework de session | Noise `XX` / `X` | `snow` |
| AEAD | ChaCha20-Poly1305 | `chacha20poly1305` (via `snow`) |
| Hash | SHA-256 / SHA-512 | `sha2` |
| MAC (tags) | HMAC-SHA256 | `hmac` |
| KDF (au besoin) | HKDF-SHA256 | `hkdf` |
| Aléa | OS CSPRNG | `getrandom` / `rand_core::OsRng` |
| Base locale chiffrée | SQLCipher (AES-256) ou champ-par-champ XChaCha20 | `rusqlite` + `sqlcipher` feature |

Règles : pas de crypto maison ; versions épinglées ; `cargo audit` + `cargo deny`
en CI ; revue par un tiers avant le premier déploiement « produit ».

**Recommandations `oswin/02 §6`** : bibliothèque **`libsodium`** (X25519,
Ed25519, XChaCha20-Poly1305 — dispo C/Arduino, JS, Python, y compris ESP32) ;
MVP crypto = X25519 (échange) → clé de session → AES-256-GCM ou ChaCha20-Poly1305
+ Ed25519 (signature) ; gestion des clés = chaque utilisateur a une paire
Ed25519, le vrai problème pratique est l'**échange initial des clés publiques**
(présentiel, QR code, ou TOFU).

### 8.8 Résumé « qui voit quoi » (`powl/04 §8`)

| Donnée | Expéditeur | Destinataire | Relais BLE | VPS / Dashboard |
| --- | --- | --- | --- | --- |
| Texte du message | ✅ | ✅ | ❌ | ❌ |
| `msg_uuid` | ✅ | ✅ | ❌ (chiffré) | ❌ |
| `msgID` (L3) | ✅ | ✅ | ✅ | ✅ **haché** |
| Qui → qui | ✅ | ✅ | `peerID` src visible ; dest = tag anonyme | ❌ (juste des `peerID` de relais + hachés) |
| Taille réelle | ✅ | ✅ | ❌ (padding) | ❌ |
| Statut (parti/distribué/lu) | ✅ | ✅ | métadonnée de passage | ✅ (reconstruit, best-effort) |
| Journal d'activité nœud | ✅ (le sien) | — | capte les `LOG_ATTEST` | ✅ (vérifie, ne forge pas) |

---

## 9. Cycle de vie d'un message & statuts

### 9.1 Les statuts (`powl/05 §1`)

| Statut (UI) | Code interne | Signification précise | Événement journal |
| --- | --- | --- | --- |
| **En attente** | `QUEUED` | Créé, chiffré, dans l'outbox local. Remis à aucun pair/relais. | `msg.queued` |
| **Parti** | `IN_FLIGHT` | Remis à ≥ 1 relais/pair **ou** enveloppe scellée déposée. Sur le réseau, pas encore chez le destinataire. | `msg.handed_off` |
| **Distribué** | `DELIVERED` | **ACK signé** de l'appareil destinataire reçu. | `msg.delivered` |
| **Lu** | `READ` | **Read-receipt signé** du destinataire reçu (conversation ouverte). | `msg.read` |
| *(Échec)* | `EXPIRED` | `MSG_TTL_S` (24 h) écoulé sans `DELIVERED`, budget de copies épuisé. | `msg.expired` |
| *(Annulé)* | `CANCELLED` | L'expéditeur retire le message avant `IN_FLIGHT`. | `msg.cancelled` |

Le suivi porte sur le **`msg_uuid`** (stable de bout en bout), pas le `msgID` L3.
« Parti » ≠ « le destinataire a le message » : c'est « le réseau s'en occupe ».
Transitions **monotones** (jamais en arrière) ; un ACK en double est ignoré.
**Lu implique distribué** : recevoir un read-receipt sans avoir vu l'ACK fait
passer directement `IN_FLIGHT → READ` (et journalise `msg.delivered` + `msg.read`).

**Correspondance avec les statuts `olivier`** : *En attente* = pas encore
transmis à un voisin ; *Parti* = transmis à ≥ 1 voisin ; *Distribué* = ACCUSÉ
reçu ; *Échec* = > ~24 h sans ACCUSÉ ; *Lu* = v2.

### 9.2 Machine à états (`powl/05 §2`)

```
[*] --> QUEUED : send_message()
QUEUED --> CANCELLED : cancel() (avant tout hand-off)
QUEUED --> IN_FLIGHT : 1er paquet remis à un pair OU enveloppe déposée
IN_FLIGHT --> IN_FLIGHT : re-remise à d'autres pairs (resend outbox)
IN_FLIGHT --> DELIVERED : Ack{status=2} signé reçu
IN_FLIGHT --> READ : ReadReceipt reçu (skip delivered)
IN_FLIGHT --> EXPIRED : MSG_TTL_S dépassé, pas d'Ack
DELIVERED --> READ : ReadReceipt signé reçu
DELIVERED --> [*] ; READ --> [*] ; EXPIRED --> [*] ; CANCELLED --> [*]
```

Implémentation : `dengon-core::sync::status`, une seule fonction
`apply_event(msg_uuid, ev) -> Option<StatusChange>` ; toute mutation passe par
elle et journalise (`ledger.append`).

### 9.3 Émission (`powl/05 §3`)

`send_message(dest_peerID, "salut")` → résoudre contact (clé static,
vérifié/TOFU) → si session Noise active : chiffrer AppFrame → `NOISE_MSG` ; sinon
sceller AppFrame → `SEALED_ENVELOPE` (Noise X) + `recipient_tag` du jour →
`outbox.insert(msg_uuid, packet, status=QUEUED)` → `ledger.append("msg.queued")`.
Puis, à **chaque** `PeerConnected` : ANNOUNCE + handshake XX si besoin →
réconciliation gossip → `send(link, packet)` → `outbox.mark(msg_uuid,
IN_FLIGHT, attempts++)` → `ledger.append("msg.handed_off")`.

**Règles de rejeu** : à chaque `PeerConnected`, rejouer tous les `QUEUED`/
`IN_FLIGHT` non expirés dont le pair est destinataire **ou** bon candidat relais ;
`attempts` plafonné (`RESEND_MAX = 8`) par pair, mais illimité dans le temps tant
que `expires_ms` n'est pas atteint ; à réception d'un `Ack{status=2}` →
`DELIVERED`, on **retire** de l'outbox les copies encore en circulation
(best-effort ; `GOSSIP` propagera l'absence).

### 9.4 Réception (`powl/05 §4`)

`FrameReceived(NOISE_MSG | SEALED_ENVELOPE)` → dédup (`msgID`), vérif → déchiffrer
(Noise X si tag = le mien, sinon Noise transport) → `messages.insert(msg_uuid,
conv, texte, received_ms)` (**idempotent sur `msg_uuid`**) →
`ledger.append("msg.received")` → préparer `Ack{msg_uuid, status=2}` → envoyer
(session si possible, sinon enveloppe vers l'expéditeur). À l'ouverture de la
conversation → préparer + envoyer `ReadReceipt` → `ledger.append("msg.read_sent")`.
L'`Ack` et le `ReadReceipt` sont eux-mêmes soumis au store-and-forward.

### 9.5 Cas multi-saut avec relais absent du destinataire (`powl/05 §5`)

Alice → Charlie (éteint), relais R1, R2. Alice n'a pas de session avec Charlie →
`SEALED_ENVELOPE(tag_charlie_J)` ; outbox QUEUED ; Alice→R1 (BLE, signé) →
`IN_FLIGHT` (« parti »). R1 vérifie la sig, stocke l'enveloppe (`copy_budget=4`),
`ledger.append("envelope.stored")`, LOG → VPS `pkt.relayed`. R1→R2 :
`ENVELOPE_OFFER [tag_charlie_J]` → `ENVELOPE_REQUEST` → R1 envoie le
`SEALED_ENVELOPE` (budget partagé : R1=2, R2=2). Charlie se rallume, entre dans la
portée de R2 → `ANNOUNCE` → R2 `ENVELOPE_OFFER` → Charlie calcule ses tags
(J-1,J,J+1) → match → `ENVELOPE_REQUEST` → R2 envoie → Charlie déchiffre (Noise
X), stocke, affiche, `ledger.append("msg.received")`, émet
`Ack{msg_uuid, status=2}` (SEALED vers Alice, `tag_alice_J`) → R2 stocke
l'Ack-enveloppe. Alice se reconnecte plus tard → récupère l'enveloppe pour
`tag_alice_J` via gossip/rencontre → déchiffre l'Ack → statut `DELIVERED`.

Le **dashboard** voit : `msg.handed_off` (Alice, opt-in) → `pkt.relayed` (R1) →
`pkt.relayed`/`envelope.handoff` (R2) → `envelope.delivered` (R2) →
`ack.observed`. Il reconstruit `parti → distribué`.

### 9.6 Reconnexion — séquence complète (`powl/05 §6`)

Quand un lien BLE s'établit entre A et B : (1) `ANNOUNCE` mutuels (peerID,
pub_static, pub_sign, pseudo, ledger_height, caps) ; (2) `NOISE_HS ×3` si pas de
session Noise valide en cache ; (3) **en parallèle** — réconciliation gossip
(`GOSSIP_FILTER` mutuels, puis `GOSSIP_PULL`/`GOSSIP_PUSH` de ce qui manque) **et**
enveloppes scellées (`ENVELOPE_OFFER` mutuels, `ENVELOPE_REQUEST` sur les tags qui
matchent, `SEALED_ENVELOPE`) ; (4) vidage d'outbox (`NOISE_MSG` / `SEALED` pour
les msgs QUEUED|IN_FLIGHT destinés à B ou relayables) ; (5) appliquer les
Acks/ReadReceipts reçus → maj statuts ; (6) si B est un relais avec Wi-Fi : flush
buffer NVS → MQTT vers le VPS.

C'est **le** mécanisme qui rend « la déconnexion non bloquante » : ce qui m'était
destiné et que je n'ai pas → je le récupère ; ce que je n'ai pas pu transmettre →
je le repousse ; les accusés en retard → ils me rattrapent et mes statuts
avancent.

### 9.7 Expiration & nettoyage (`powl/05 §7`)

| Élément | Règle | Effet |
| --- | --- | --- |
| Message outbox | `now > expires_ms` (24 h) sans `DELIVERED` | statut `EXPIRED`, `ledger.append("msg.expired")`, UI « non remis » + bouton renvoyer |
| Enveloppe scellée (chez un porteur) | budget = 0 **et** `now > deposit_ms + MSG_TTL_S` | suppression silencieuse |
| Entrée seen-set | `now > seen_ms + SEEN_TTL_S` | éviction (permet un renvoi légitime ultérieur) |
| Fragment partiel | `now > FRAG_TIMEOUT_S` d'inactivité | abandon |
| Session Noise | lien BLE perdu | oubliée (nouvelle FS à la reconnexion) |
| Cache gossip public | `now > 6 h` | éviction (fenêtre de sync) |

### 9.8 Ordre & cohérence par conversation (`powl/05 §8`)

`conv_seq` (compteur par conversation, par expéditeur) permet au destinataire de
détecter un **trou** (`… 41, 43 …` → il manque 42) et de le demander via
`GOSSIP_PULL` au prochain contact. L'affichage ordonne par `sent_ms` ; en cas
d'égalité, par `conv_seq`. Pas d'ordre **global** entre conversations différentes.

### 9.9 Table : événement UI ↔ paquet ↔ log (`powl/05 §9`)

| Action utilisateur / système | Paquet(s) émis | Statut | Événement(s) VPS |
| --- | --- | --- | --- |
| Rédige + envoie | — | `QUEUED` | `msg.queued` (opt-in) |
| 1er pair reçoit le paquet | `NOISE_MSG` / `SEALED_ENVELOPE` | `IN_FLIGHT` | `msg.handed_off` (opt-in), `pkt.relayed` (relais) |
| Relais transporte | `GOSSIP_PUSH` / `ENVELOPE_*` | — | `pkt.relayed`, `envelope.stored`, `envelope.handoff` |
| Destinataire reçoit | `ACK{status=2}` | `DELIVERED` (à l'arrivée de l'ACK) | `ack.observed`, `envelope.delivered` |
| Destinataire ouvre la conv | `ReadReceipt` | `READ` | `read.observed` |
| 24 h sans ACK | — | `EXPIRED` | `msg.expired` (opt-in) |

---

## 10. Relais ESP32

### 10.1 Rôle (`powl/06 §1`)

Infrastructure **fixe** : branché au secteur, posé en hauteur, toujours allumé.
Densifie le maillage et sert de **mémoire tampon** du réseau.

| Fonction | Détail |
| --- | --- |
| **Relayer** | reçoit des paquets BLE, applique le pipeline de routage (§6.6), rediffuse |
| **Cacher (gossip)** | garde en RAM/PSRAM les messages publics et paquets récents (fenêtre 6 h) pour la réconciliation |
| **Déposer (courier)** | stocke les `SEALED_ENVELOPE` pour destinataires absents, gère le budget de copies |
| **Attester** | tient son propre journal chaîné, diffuse `LOG_ATTEST` |
| **Remonter les logs** | Wi-Fi station → MQTT/TLS → VPS ; buffer en flash si Wi-Fi coupé |

**Ne déchiffre rien** : aucune clé utilisateur. Ne voit que des paquets chiffrés
+ des métadonnées de passage.

### 10.2 Matériel

**`powl/06 §2`** : module **ESP32-WROVER-E** (4 Mo flash, 8 Mo PSRAM) — buffers
messages + enveloppes + coexistence BLE/Wi-Fi. Alt. prototype : ESP32-WROOM-32
(PSRAM absente → buffers réduits, débit BLE+Wi-Fi plus faible). Alimentation
USB 5 V / secteur (fixe, pas de batterie). Antenne PCB ou U.FL externe (U.FL
recommandé en déploiement réel). Stockage persistant : flash interne (partition
NVS + partition `spiffs`/`littlefs` pour le buffer de logs). **Pas de** LoRa,
écran, batterie (hors MVP).

**Specs ESP32-WROOM-32E (`oswin/07 §2`, carte Freenove — celle de l'équipe côté
`oswin`/`olivier`)**

| Élément | Valeur | Pourquoi ça compte |
| --- | --- | --- |
| Processeur | **double cœur** Xtensa LX6, jusqu'à **240 MHz** | BLE **et** Wi-Fi en parallèle |
| Mémoire vive | **520 Ko SRAM** | limite la taille des files de stockage → prévoir des quotas |
| Mémoire flash | **4 Mo** (typique) | programme + file persistante (NVS/SPIFFS) |
| Wi-Fi | **802.11 b/g/n (2,4 GHz)** | passerelle vers le dashboard (MQTT) |
| Bluetooth | **v4.2 : BR/EDR + BLE** | rôle de relais BLE dans le mesh |
| GPIO | ~34 broches | LED d'état, boutons, écran éventuel |
| Alimentation | 5 V par USB / 3,3 V logique | une bonne alim USB suffit pour la démo |

> Le Bluetooth et le Wi-Fi **partagent la même radio 2,4 GHz** → activer les deux
> en continu peut ralentir ; à tester tôt (on peut alterner, ou dédier un ESP32
> « passerelle » Wi-Fi et d'autres « relais » BLE). La carte Freenove est
> programmable avec l'IDE Arduino ou PlatformIO.

**Matériel comparé (`oswin/05 §1`)** : Arduino Uno + HC-05/HC-06 (Bluetooth
Classic, pas d'Internet) = OK pour prototype point-à-point, pas de BLE mesh ;
Arduino + HM-10 (BLE, pas d'Internet) = possible mais limité en RAM ; **ESP32**
(BLE natif + Wi-Fi intégré, ~5–10 €) = **recommandé**, un seul composant fait
relais BLE **et** passerelle Internet ; Raspberry Pi = plus puissant mais « plus
vraiment de l'Arduino ». C'est le choix standard pour une passerelle **BLE →
MQTT**.

### 10.3 Stack firmware (`powl/06 §3`)

```
ESP-IDF v5.x
├── NimBLE (host BLE)                    → rôle GATT double (serveur + client)
├── esp-wifi (station)                   → connexion au réseau du site
├── esp-mqtt (mqtts, TLS)                → publication des logs
├── nvs_flash (chiffré, eFuse)           → clé de relais, config, curseur de journal
├── littlefs                             → buffer de logs hors-ligne (ring)
└── components/dengon_core_ffi/
        └── libdengon_core.a  (Rust, no_std+alloc, cross-compilé xtensa)
             → protocol, routing/dedup, gossip, ledger
        └── crypto : mbedTLS (déjà dans IDF) OU port libsodium — décision lot 0
```

**Réutilisation de `dengon-core`** : `protocol`, `sync::routing`, `sync::gossip`,
`ledger`, `observability` compilent en `no_std` + `alloc` → archivés en
`libdengon_core.a`, exposés au C via un header `dengon_core.h` (généré par
`cbindgen`). `store` : implémentation `Store` spécifique ESP32 (index en PSRAM,
persistance NVS pour le curseur de journal et les enveloppes critiques).
`crypto` : si `snow` + `ed25519-dalek` cross-compilent proprement pour xtensa
(`getrandom` → source ESP32) → on garde tout en Rust ; sinon, trait `Crypto`
implémenté côté C avec mbedTLS/libsodium. **Spike obligatoire au lot 0.**

**Coexistence BLE + Wi-Fi** : Wi-Fi en **mode station uniquement** (coexistence
radio supportée uniquement en STA) ; débit combiné modeste, acceptable (le relais
BLE prime, le MQTT est bufferisé et envoyé par petits lots) ; PSRAM requise pour
des buffers BLE confortables quand le Wi-Fi tourne.

### 10.4 Architecture logicielle interne (`powl/06 §4`)

Tâches FreeRTOS : `ble_rx_task` (reçoit trames GATT) → `route_task` (dengon_core :
dedup, TTL, relais) → `gossip_task` (filtre GCS, pull/push), `courier_task`
(stock enveloppes, budget), `ledger_task` (append + LOG_ATTEST périodique),
`ship_task` (buffer → MQTT) ; `wifi_task` (reconnexion, NTP). Store : PSRAM
(caches) / NVS (clé, curseur) / littlefs (log ring). Files inter-tâches `xQueue`
bornées (backpressure : si `route_task` sature, on **droppe des paquets
entrants** — jamais des enveloppes déjà acceptées).

### 10.5 Budgets mémoire (cible WROVER) (`powl/06 §5`)

| Poste | Taille | Note |
| --- | --- | --- |
| Cache gossip (paquets récents) | ~512 Ko PSRAM | ~1000 paquets, éviction LRU + 6 h |
| Enveloppes scellées stockées | ~1 Mo PSRAM + persistance NVS des N plus récentes | plafond `ENVELOPE_STORE_MAX = 512` |
| Seen-set (dedup) | ~48 Ko | 1024 × (32 o hash + méta) |
| Réassemblage fragments | ~256 Ko | `FRAG_MAX_CONCURRENT` × 4 Ko |
| Buffer de logs hors-ligne | littlefs, ~512 Ko partition | ring : écrase le plus ancien si plein |
| Pile NimBLE + Wi-Fi | ~64 Ko SRAM | |

Si WROOM (pas de PSRAM) : diviser les caches par ~8, `ENVELOPE_STORE_MAX = 64`.

### 10.6 Connexion au VPS (`powl/06 §6`)

**Provisioning** : à la fabrication, `idf.py` flashe le firmware, active **flash
encryption** + **secure boot** ; premier boot → génère la paire Ed25519 en NVS
chiffrée ; config Wi-Fi + URL du broker via `esp_wifi` provisioning BLE (SoftAP
désactivé) ou fichier de conf flashé ; l'opérateur enregistre `relay_id` +
`pub_sign` + certificat client dans le dashboard (liste blanche).

**MQTT**

| Paramètre | Valeur |
| --- | --- |
| Broker | `mqtts://<vps>:8883` |
| Auth | mTLS (cert client par relais) ; fallback JWT signé |
| Topic logs | `dengon/logs/<relay_id>` |
| Topic attest | `dengon/attest/<relay_id>` |
| Topic santé | `dengon/health/<relay_id>` (uptime, RSSI moyen, tailles de buffers, version fw) toutes les 60 s |
| QoS | 1 (au moins une fois ; le VPS déduplique sur `event_id`) |
| Payload | JSON canonique signé — batch de N événements |
| Hors-ligne | accumulation dans le ring littlefs ; flush FIFO à la reconnexion, débit limité |

**Horloge** : NTP au boot + resync toutes les heures. Les événements portent
`ts_ms` local ; le VPS ajoute `ingested_ms` et signale les dérives
(`node.clock_skew`).

### 10.7 Comportement en cas de panne (`powl/06 §7`)

| Panne | Comportement |
| --- | --- |
| Wi-Fi coupé | relais BLE continue normalement ; logs bufferisés ; retry Wi-Fi backoff |
| Broker injoignable | idem ; buffer ring |
| Buffer de logs plein | on écrase les plus anciens logs (le routage prime) ; compteur `logs_dropped` remonté ensuite |
| PSRAM saturée | éviction LRU du cache gossip ; refus de **nouvelles** enveloppes (existantes protégées) ; `ledger.append("relay.overloaded")` |
| Redémarrage | recharge clé + curseur de journal depuis NVS ; le journal **reprend à `seq` suivant** (continuité de chaîne préservée) ; caches volatils repartent vides |
| Reset d'usine | nouvelle identité de relais → doit être ré-enregistré au dashboard |

### 10.8 Ce que le relais journalise (`powl/06 §8`)

`pkt.seen`, `pkt.relayed`, `pkt.dropped` (raison), `pkt.rejected` (sig invalide) ;
`envelope.stored`, `envelope.handoff`, `envelope.delivered`, `envelope.expired` ;
`peer.connected`, `peer.disconnected` (avec RSSI, `peerID` du pair) ;
`attest.emitted` (sa propre racine), `attest.observed` (racine d'un autre nœud) ;
`relay.boot`, `relay.overloaded`, `relay.wifi_up`, `relay.wifi_down` ;
`node.clock_skew`. **Jamais** : contenu, `msg_uuid`, `recipient` identifiable ; le
`msgID` est **haché** avant émission.

### 10.9 Rôle passerelle (`oswin/05 §3`) & MQTT

L'ESP32 relie le monde Bluetooth (le mesh) au monde Internet (le dashboard) : il
reçoit les paquets en BLE, **extrait les métadonnées** (id, sauts, horodatage,
état), publie en **MQTT / HTTP** via Wi-Fi vers un broker MQTT → dashboard.
**Règle de sécurité** : la passerelle remonte des métadonnées de supervision,
**pas** le texte des messages, sinon on casse le chiffrement de bout en bout.
**MQTT** = protocole de messagerie léger standard de l'IoT (publish/subscribe via
un broker), simple, robuste, bien supporté sur ESP32.

**Scénario de démo minimal en 4 temps (`oswin/05 §6`)** : (1) A chiffre + signe +
publie un message en BLE ; (2) l'ESP32 (hors de portée de B) le reçoit, le
**stocke**, **publie ses métadonnées en MQTT** → « message reçu au nœud Arduino,
1 saut » ; (3) B entre dans la portée de l'ESP32 → retransmission → B déchiffre et
répond par un ACK signé ; (4) l'ACK remonte, l'ESP32 **purge** sa copie et publie
« message livré, purge ». Ce scénario démontre **les 6 points** du cahier des
charges en une manipulation.

**Pièges pratiques (`oswin/05 §7`)** : RAM limitée (files bornées, quota + purge) ;
Wi-Fi + BLE simultanés (radio 2,4 GHz partagée → ralentissements, tester tôt) ;
sécurité de la passerelle (MQTT authentifié + TLS si le broker est sur Internet) ;
alimentation (ESP32 en scan BLE continu + Wi-Fi consomme, prévoir une bonne alim
USB).

---

## 11. Dashboard d'observabilité

### 11.1 Principe (`powl/07 §1`)

Le dashboard **observe** le réseau. Il ne transporte aucun message, ne détient
aucune clé utilisateur, ne peut rien injecter. S'il tombe, la messagerie continue
à l'identique. Il reçoit des **événements de métadonnées signés** (des relais, et
des clients qui l'acceptent explicitement), les vérifie, les stocke, les restitue
en vues : parcours d'un message, santé du réseau, état de la flotte, intégrité
des journaux.

### 11.2 Architecture (`powl/07 §2`)

| Service | Techno | Rôle |
| --- | --- | --- |
| `caddy` | Caddy 2 | TLS auto (Let's Encrypt), reverse-proxy, rate-limit |
| `mosquitto` | Eclipse Mosquitto 2 | broker MQTT, auth mTLS, ACL par topic |
| `api` | **Rust + Axum** | ingest worker (client MQTT) + REST + WebSocket + vérif chaîne |
| `db` | PostgreSQL 16 + TimescaleDB | hypertable événements + tables relationnelles |
| `web` | React + TS + Vite, servi par nginx | UI opérateur |

Le backend Rust **réutilise `dengon-core`** : mêmes structs d'événements
(`observability`), même code de vérification de journal (`ledger::verify_chain`),
mêmes constantes.

### 11.3 Ingestion (`powl/07 §3`)

**Sources** : relais → MQTT `dengon/logs/<relay_id>`, `dengon/attest/*`,
`dengon/health/*`, auth mTLS, volume élevé continu. Client opt-in → `POST
/ingest/batch` (HTTPS), auth token éphémère lié au `peerID`, volume faible
sporadique.

**Pipeline** : message MQTT (batch JSON signé) → parse + schéma (rejet si
malformé) → vérif signature Ed25519 (clé = `pub_sign` du nœud, doit être en liste
blanche) → dédup sur `event_id` (`SHA-256(node ‖ seq)`) → si event de journal :
chaîner (`prev_hash == hash(seq-1)` ?) → statut d'intégrité → INSERT hypertable
`events` → mise à jour des projections (messages, nodes, links) → push WebSocket
aux opérateurs connectés (filtré). Idempotent (rejouer un batch QoS 1 ne crée pas
de doublon).

**Ce qui est refusé** : signature invalide → `events` avec `flag=rejected_sig`,
alerte ; `node` inconnu (pas en liste blanche) → quarantaine, alerte ; `msgID`
non haché (format brut détecté) → **rejet strict** (protection anti-fuite).

### 11.4 Modèle de données (résumé — détail au §12) (`powl/07 §4`)

Relations : `NODES ||--o{ EVENTS` (émet) ; `NODES ||--o{ LEDGER_STATE` ;
`MESSAGES ||--o{ EVENTS` (référencé par) ; `MESSAGES ||--o{ MESSAGE_HOPS` ;
`NODES ||--o{ LINKS`. `events` = **hypertable Timescale** (partition par temps),
rétention configurable ; les autres tables sont des **projections** entretenues
par l'ingest (requêtes rapides), reconstructibles en rejouant `events`.

### 11.5 Écrans (`powl/07 §5`)

- **Parcours d'un message** : entrée = un `msg_log_id` (l'app peut afficher un
  QR/lien « suivre ce message » qui contient déjà le hash) **ou** recherche par
  fenêtre de temps + `peerID` de relais. Affiche : **timeline verticale** (chaque
  saut de `MESSAGE_HOPS` avec nœud, horodatage, TTL in/out, fanout, RSSI) ;
  **statut courant** reconstruit avec l'horodatage de chaque transition et la
  **source** de l'info ; **carte mini** du chemin ; badges d'intégrité.
- **Carte / santé du réseau** : graphe force-directed des `NODES` + `LINKS`
  (relais en gros, clients agrégés) ; couleur = statut (`online`/`stale`/
  `suspect`) ; épaisseur d'arête = trafic ; opacité = fraîcheur ; panneau latéral
  = densité moyenne, latence de relais médiane, taux de livraison, nb
  d'enveloppes en circulation (estimé).
- **Flotte de relais** : tableau `relay_id`, label, version fw, uptime, RSSI
  moyen, tailles de buffers (gossip / enveloppes / logs), `logs_dropped`,
  dernière remontée, statut secure boot/flash-enc. Alertes : relais muet > 5 min,
  buffer > 90 %, version obsolète.
- **Intégrité des journaux** : par nœud — hauteur, racine courante, dernier
  contrôle, verdict (`ok` / `chain_broken` / `fork_detected` / `unverified`). Vue
  « diff » quand un fork est détecté.
- **Recherche de logs** : filtre plein-texte sur `events` (name, node, payload
  jsonb), fenêtre temporelle, export CSV/JSON.

**Alerting** (règles configurables → notif webhook / e-mail) :

| Alerte | Condition |
| --- | --- |
| `relay_silent` | pas de `health` d'un relais depuis 5 min |
| `chain_broken` | `events.integrity = broken` |
| `fork_detected` | 2 racines pour (node, height) |
| `rejected_sig_spike` | > N signatures invalides / min |
| `delivery_drop` | taux de livraison < seuil sur 15 min |
| `clock_skew` | dérive d'horloge d'un nœud > 2 min |

### 11.6 API (Axum) — principales routes (`powl/07 §6`)

| Méthode | Route | Rôle |
| --- | --- | --- |
| `POST` | `/ingest/batch` | ingestion clients opt-in (token) |
| `GET` | `/api/messages/:msg_log_id` | parcours + statut + sauts |
| `GET` | `/api/messages?since=&node=&status=` | liste filtrée |
| `GET` | `/api/network/graph` | nœuds + liens (snapshot) |
| `GET` | `/api/nodes` / `/api/nodes/:id` | flotte |
| `GET` | `/api/integrity` | verdicts par nœud |
| `GET` | `/api/events?…` | recherche de logs |
| `WS` | `/api/stream` | flux temps réel (events, transitions de statut, alertes) |
| `POST` | `/api/nodes` (admin) | enregistrer un relais (pub_sign, cert) |
| `GET` | `/healthz` | liveness |

Auth : session opérateur (cookie httponly + CSRF) ; rôles `viewer` / `admin`.

### 11.7 Déploiement sur le VPS (`powl/07 §7`)

`dashboard/deploy/` = `docker-compose.yml` (caddy, mosquitto, api, db, web),
`Caddyfile`, `mosquitto/` (mosquitto.conf, acl, ca/), `migrations/` (SQL, sqlx
migrate), `.env.example`. Étapes : (1) `docker compose up -d db` → `sqlx migrate
run` ; (2) générer la CA MQTT + certs relais ; (3) `docker compose up -d` (le
reste) ; (4) Caddy obtient le certificat TLS pour `dashboard.<domaine>` ; (5)
enregistrer chaque relais via `POST /api/nodes` ; (6) créer le compte opérateur
admin (CLI `api seed-admin`). Ressources : léger (< 1 Go RAM hors Postgres).

### 11.8 Confidentialité — garanties (`powl/07 §8`)

**Aucun** contenu de message, **aucun** `msg_uuid`, **aucun** `recipient`
identifiable ne peut arriver ni être stocké (rejet au schéma) ; `msgID` toujours
**haché** (`SHA-256(msgID)[:16]`) ; les pseudos n'apparaissent que si un nœud les
publie volontairement (opt-in) ; rétention bornée + purge automatique ; le
dashboard est **read-only vis-à-vis du terrain**.

### 11.9 Catalogue normalisé des événements (`powl/08` — SOURCE DE VÉRITÉ)

**Règles générales** : nommage `<domaine>.<action>` en `snake_case`. Enveloppe
commune (tout événement) :

```json
{
  "event_id": "hex(SHA-256(node_id ‖ seq))",
  "node_id": "relay-3f2a… | client-9c1d…",
  "node_kind": "relay | client",
  "seq": 10432,
  "ts_ms": 1725800000123,
  "name": "pkt.relayed",
  "payload": { }
}
```

Le nœud signe `canonical_json(enveloppe_sans_sig)` avec sa clé `sign` ; `sig`
(base64) ajouté **hors** du JSON canonique, dans le batch MQTT. Les événements
sont **aussi** des entrées de journal ; `payload` inclut `prev_hash`
implicitement via le mécanisme `ledger` (le dashboard recalcule). **Redaction
obligatoire avant émission** : `msgID` → `msg_log_id = hex(SHA-256(msgID)[0..16])` ;
jamais de `msg_uuid`, jamais de texte, jamais de `recipient` en clair ; `peerID`
de pairs gardés (pseudonymes) mais tronqués à 8 o ; `recipient_tag` autorisé
(déjà anonyme et tournant). Batching : N événements par message MQTT ; `event_id`
déduplique côté VPS. Versionnement : champ `schema_version` (entier) dans
l'enveloppe du batch ; le catalogue vit dans
`crates/dengon-core/src/observability/catalog.rs` (ce document doit rester
synchronisé avec lui).

**Domaine `pkt` — trafic de paquets**

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `pkt.seen` | relais, client | `{ msg_log_id, type, ttl_in, size_bucket, from_peer, rssi }` | paquet reçu (avant dédup) |
| `pkt.duplicate` | relais, client | `{ msg_log_id, from_peer }` | déjà dans le seen-set |
| `pkt.relayed` | relais, client | `{ msg_log_id, type, ttl_in, ttl_out, fanout, from_peer }` | rediffusé |
| `pkt.delivered_local` | client | `{ msg_log_id, type }` | paquet pour moi, traité |
| `pkt.dropped` | relais, client | `{ msg_log_id?, reason }` | `reason ∈ {ttl_zero, relay_not_ok, queue_full, quota, too_old}` |
| `pkt.rejected` | relais, client | `{ from_peer, reason }` | `reason ∈ {bad_sig, bad_version, malformed, peerid_mismatch}` |

`size_bucket` ∈ `{256,512,1024,2048}` (jamais la taille exacte).

**Domaine `envelope` — enveloppes scellées**

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `envelope.stored` | relais, client | `{ msg_log_id, recipient_tag, epoch_day, copy_budget }` | enveloppe acceptée en dépôt |
| `envelope.offered` | relais, client | `{ count, to_peer }` | `ENVELOPE_OFFER` envoyé |
| `envelope.handoff` | relais, client | `{ msg_log_id, recipient_tag, to_peer, budget_after }` | copie transmise à un autre porteur |
| `envelope.delivered` | relais, client | `{ msg_log_id, recipient_tag, to_peer }` | remise à un pair dont le tag matche (probable destinataire) |
| `envelope.expired` | relais, client | `{ msg_log_id, recipient_tag, reason }` | `reason ∈ {ttl, budget_zero, store_full}` |

**Domaine `msg` — cycle de vie (clients opt-in surtout)**

| `name` | Producteur | `payload` | Statut résultant |
| --- | --- | --- | --- |
| `msg.queued` | client émetteur | `{ msg_log_id, conv_hash }` | `queued` |
| `msg.handed_off` | client émetteur | `{ msg_log_id, to_peer, via }` (`via ∈ {session, envelope}`) | `in_flight` |
| `msg.received` | client destinataire | `{ msg_log_id, via }` | (contribue à `delivered`) |
| `msg.delivered` | client émetteur | `{ msg_log_id, latency_ms }` | `delivered` |
| `msg.read` | client émetteur | `{ msg_log_id, latency_ms }` | `read` |
| `msg.expired` | client émetteur | `{ msg_log_id }` | `expired` |
| `ack.observed` | relais | `{ msg_log_id }` | indice de `delivered` |
| `read.observed` | relais | `{ msg_log_id }` | indice de `read` |

`conv_hash` = `SHA-256(min(peerA,peerB) ‖ max(peerA,peerB))[0..8]` (grouper une
conversation **sans** savoir qui elle implique). Reconstruction du statut côté
dashboard : priorité aux événements de l'émetteur (`msg.*`) ; à défaut, inférence
best-effort depuis les événements relais ; statut `unknown` si aucune donnée.

**Domaine `peer` / `link` — topologie**

| `name` | Producteur | `payload` |
| --- | --- | --- |
| `peer.connected` | relais, client | `{ peer, rssi, role }` (`role ∈ {central, peripheral}`) |
| `peer.disconnected` | relais, client | `{ peer, duration_s, pkt_exchanged }` |
| `peer.announce_seen` | relais, client | `{ peer, pseudo?, ledger_height, caps }` |

Le dashboard dérive `LINKS` de la corrélation `peer.connected`/`disconnected`
entre deux `node_id` connus.

**Domaine `attest` / `integrity` — journal chaîné**

| `name` | Producteur | `payload` | Sens |
| --- | --- | --- | --- |
| `attest.emitted` | tout nœud | `{ root, height }` | ma racine de journal courante |
| `attest.observed` | relais, client | `{ subject_node, root, height }` | racine d'un **autre** nœud, vue via `LOG_ATTEST` |
| `integrity.chain_broken` | **dashboard** (dérivé) | `{ node, seq, expected_prev, got_prev }` | `prev_hash` incohérent |
| `integrity.fork_detected` | **dashboard** (dérivé) | `{ node, height, roots: [..], reporters: [..] }` | 2 racines pour une hauteur |
| `integrity.gap` | **dashboard** (dérivé) | `{ node, from_seq, to_seq }` | trou dans `seq` |

`integrity.*` ne viennent pas du terrain : ce sont des conclusions de l'ingest.

**Domaine `relay` / `node` — santé**

| `name` | Producteur | `payload` |
| --- | --- | --- |
| `relay.boot` | relais | `{ fw_version, reset_reason, secure_boot, flash_enc }` |
| `relay.health` | relais (60 s) | `{ uptime_s, rssi_avg, peers, gossip_cache, envelope_store, log_buffer_pct, logs_dropped, heap_free }` |
| `relay.wifi_up` / `relay.wifi_down` | relais | `{ ssid?, duration_s? }` |
| `relay.overloaded` | relais | `{ subsystem, action }` (`action ∈ {drop_pkt, refuse_envelope, evict_cache}`) |
| `node.clock_skew` | **dashboard** (dérivé) | `{ node, skew_ms }` |
| `client.summary` | client opt-in (périodique) | `{ msgs_sent, msgs_recv, peers_seen, app_version }` (agrégats anonymes) |

**Exemple de batch MQTT** (`dengon/logs/relay-3f2a`)

```json
{
  "batch_id": "b1f0…",
  "node_id": "relay-3f2a9c",
  "events": [
    { "event_id": "9a1c…", "node_id": "relay-3f2a9c", "node_kind": "relay",
      "seq": 10432, "ts_ms": 1725800000123, "name": "pkt.relayed",
      "payload": { "msg_log_id": "4d5e6f7a8b9c0d1e", "type": 4,
                   "ttl_in": 6, "ttl_out": 5, "fanout": 2,
                   "from_peer": "a1b2c3d4e5f60718" } },
    { "event_id": "9a1d…", "node_id": "relay-3f2a9c", "node_kind": "relay",
      "seq": 10433, "ts_ms": 1725800000455, "name": "envelope.stored",
      "payload": { "msg_log_id": "4d5e6f7a8b9c0d1e",
                   "recipient_tag": "00112233445566778899aabbccddeeff",
                   "epoch_day": 20340, "copy_budget": 4 } }
  ],
  "sig": "base64(ed25519(canonical_json(batch_without_sig)))"
}
```

### 11.10 Version `olivier/dashboard.md` v0.2 (brouillon)

**Principe** : bonus non bloquant ; alimenté opportunément (seuls les nœuds avec
du Wi-Fi envoient) ; **anonymisé** — un nœud se désigne par un **code anonyme
différent pour chaque message** qu'il traite (on reconstitue le parcours d'**un**
message, mais on ne peut pas relier entre eux les messages passés par un même
appareil) ; mise à jour automatique de la page ; hébergement = VPS Debian de
l'équipe.

**Affiche** : liste des messages suivis (id raccourci, statut courant, heure de
création, dernière activité) ; détail d'un message (parcours = suite de nœuds en
codes, horodatage de chaque étape, statut) ; compteurs globaux (messages en
circulation, distribués/expirés, taux de distribution) ; « nombre d'appareils
actifs » = **impossible à déduire** puisque le code change à chaque message →
deux options **(ouvert)** : battement anonyme séparé, ou retirer ce compteur. Pas
de carte géographique, pas de liste nominative en v1.

**Ne montre JAMAIS** : contenu, clé publique / empreinte / pseudo réel, qui a
écrit à qui, position physique. **Montre** : id raccourci, statut, parcours en
codes de nœuds, horodatages des étapes.

**Événements envoyés par les nœuds** : `message-créé` (l'expéditeur),
`message-relayé` (un relais, avec sauts restants), `message-distribué` (le
destinataire), `message-expiré` (tout nœud qui purge). Contenu : id raccourci,
code **pour ce message**, horodatage. Aucun événement ne contient d'empreinte.
Le serveur doit être **tolérant** (événements en retard, dans le désordre, en
double, ou jamais).

**Statut courant déduit par le serveur**

| Statut affiché | Règle |
| --- | --- |
| **En circulation** | Au moins un `message-créé` ou `message-relayé`, ni `distribué` ni `expiré`. |
| **Distribué** | Un `message-distribué` reçu. |
| **Expiré** | Un `message-expiré` reçu, et pas de `message-distribué`. |
| **Inconnu / partiel** | Aucun événement reçu pour ce message. |

**Côté serveur (VPS Debian)** : API de réception HTTPS ; **authentification des
envois — (ouvert)** (jeton partagé ?) ; stockage = événements bruts + état
reconstruit par message ; canal temps réel — piste **SSE** ; page web servie par
le même serveur ; **rétention** = données effacées après chaque session de démo
(remise à zéro manuelle en v1).

**Maquette d'écrans** :

```
┌───────────────────────────── dengon · suivi ─────────────────────────────┐
│ Messages suivis                              nœuds actifs : 6            │
│ id        statut           créé        dernière activité                │
│ 7f3a…     En circulation   10:02       10:04   (3 étapes)   ▶           │
│ b1c8…     Distribué        09:51       09:58   (4 étapes)   ▶           │
│ 42de…     Expiré           hier 22:10  hier 22:41            ▶          │
└─────────────────────────────────────────────────────────────────────────┘
Détail du message 7f3a… :
   [ node-K ] créé 10:02
        ▼ relayé 10:03 (sauts restants 6)
   [ node-M ]
        ▼ relayé 10:04 (sauts restants 5)
   [ node-P ]        … en attente de la suite
```

**Replis si le temps manque** : (1) page rafraîchie à la main (abandon du canal
temps réel) ; (2) maquette statique avec données d'exemple ; (3) mode démo qui
rejoue un scénario enregistré (seulement s'il reste du temps).

**Outils dashboard côté `oswin/05 §4`** : **ThingsBoard** ⭐ (plateforme IoT open
source / cloud, dashboards prêts, MQTT natif, SDK ESP32 officiel) ou **Node-RED**
(outil de flux visuel + dashboard, très pédagogique) ; alternatives Grafana +
MQTT/InfluxDB, page web maison (HTML + MQTT over WebSocket). Widgets suggérés :
journal des événements horodaté ; compteur de messages en circulation /
livrés / expirés ; carte/liste des nœuds actifs ; nombre de sauts par message
livré ; délai de livraison moyen ; occupation mémoire des files ; (si option
blockchain) la chaîne d'événements chaînés par hash, vérifiable.

---

## 12. Modèles de données

> Source : `powl/09-data-model.md`.

### 12.1 Base locale client / nœud (`dengon-core::store`, SQLite)

Fichier `dengon.db` (WAL). Chiffré au repos (SQLCipher ou champ-par-champ).

```sql
CREATE TABLE identity (            -- une seule ligne
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    peer_id         BLOB NOT NULL,          -- 8 o
    priv_static     BLOB NOT NULL,          -- X25519 (chiffré)
    priv_sign       BLOB NOT NULL,          -- Ed25519 (chiffré)
    pub_static      BLOB NOT NULL,
    pub_sign        BLOB NOT NULL,
    pseudo          TEXT NOT NULL,
    created_ms      INTEGER NOT NULL
);

CREATE TABLE contacts (
    peer_id         BLOB PRIMARY KEY,       -- 8 o
    pub_static      BLOB NOT NULL,
    pub_sign        BLOB NOT NULL,
    pseudo          TEXT,
    verified_at     INTEGER,               -- NULL = TOFU non vérifié
    first_seen_ms   INTEGER NOT NULL,
    last_seen_ms    INTEGER,
    key_changed_at  INTEGER,               -- alerte MITM si non NULL
    blocked         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE conversations (
    conv_id         BLOB PRIMARY KEY,      -- SHA-256(min(a,b)‖max(a,b))[:16]
    peer_id         BLOB NOT NULL REFERENCES contacts(peer_id),
    last_msg_ms     INTEGER,
    unread_count    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE messages (
    msg_uuid        BLOB PRIMARY KEY,     -- 16 o, stable bout-en-bout
    conv_id         BLOB NOT NULL REFERENCES conversations(conv_id),
    direction       TEXT NOT NULL CHECK (direction IN ('out','in')),
    author_peer_id  BLOB NOT NULL,
    conv_seq        INTEGER NOT NULL,
    body            TEXT NOT NULL,        -- clair local uniquement
    sent_ms         INTEGER NOT NULL,
    received_ms     INTEGER,
    status          TEXT NOT NULL DEFAULT 'queued'
                    CHECK (status IN ('queued','in_flight','delivered','read','expired','cancelled')),
    status_ms       INTEGER NOT NULL,
    read_ms         INTEGER
);
CREATE INDEX idx_msg_conv ON messages(conv_id, sent_ms);

CREATE TABLE outbox (               -- paquets non confirmés
    msg_uuid        BLOB NOT NULL REFERENCES messages(msg_uuid),
    dest_peer_id    BLOB NOT NULL,
    packet          BLOB NOT NULL,        -- paquet L3 encodé
    kind            TEXT NOT NULL CHECK (kind IN ('session','envelope')),
    attempts        INTEGER NOT NULL DEFAULT 0,
    first_sent_ms   INTEGER,
    last_sent_ms    INTEGER,
    expires_ms      INTEGER NOT NULL,
    PRIMARY KEY (msg_uuid, dest_peer_id)
);

CREATE TABLE held_envelopes (      -- enveloppes portées pour autrui
    msg_log_id      BLOB PRIMARY KEY,     -- SHA-256(msgID)[:16]
    recipient_tag   BLOB NOT NULL,        -- 16 o
    epoch_day       INTEGER NOT NULL,
    packet          BLOB NOT NULL,        -- SEALED_ENVELOPE complet
    copy_budget     INTEGER NOT NULL,
    deposit_ms      INTEGER NOT NULL,
    expires_ms      INTEGER NOT NULL
);
CREATE INDEX idx_env_tag ON held_envelopes(recipient_tag);

CREATE TABLE seen_set (   msg_id BLOB PRIMARY KEY, seen_ms INTEGER NOT NULL );      -- 32 o
CREATE TABLE gossip_cache ( msg_id BLOB PRIMARY KEY, packet BLOB NOT NULL, cached_ms INTEGER NOT NULL );

CREATE TABLE noise_sessions (
    peer_id         BLOB PRIMARY KEY,
    state           BLOB NOT NULL,        -- sérialisation snow (chiffrée)
    established_ms   INTEGER NOT NULL,
    tx_count        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE ledger (              -- journal chaîné signé
    seq             INTEGER PRIMARY KEY,  -- 0,1,2,… sans trou
    ts_ms           INTEGER NOT NULL,
    event_name      TEXT NOT NULL,
    payload_json    TEXT NOT NULL,        -- canonical JSON
    prev_hash       BLOB NOT NULL,        -- 32 o
    entry_hash      BLOB NOT NULL,        -- 32 o
    sig             BLOB NOT NULL         -- 64 o
);

CREATE TABLE ship_cursor ( id INTEGER PRIMARY KEY CHECK (id = 1), last_shipped_seq INTEGER NOT NULL DEFAULT -1 );
```

**Adaptation ESP32** : `identity` → NVS chiffrée ; `held_envelopes` → index en
PSRAM + N plus récentes sérialisées en NVS/littlefs ; `seen_set`, `gossip_cache`
→ RAM/PSRAM, volatile (reconstruit au boot) ; `ledger` → append en littlefs
(fichier séquentiel) + `seq`/`root` en NVS ; `ship_cursor` → NVS ; `messages`,
`outbox`, `contacts`, … → **absents** (le relais ne fait pas de messagerie
utilisateur).

### 12.2 Base dashboard (PostgreSQL 16 + TimescaleDB)

```sql
CREATE TABLE nodes (
    node_id      TEXT PRIMARY KEY,          -- 'relay-3f2a9c' | 'client-…'
    kind         TEXT NOT NULL CHECK (kind IN ('relay','client')),
    pub_sign     BYTEA NOT NULL,
    label        TEXT,
    fw_version   TEXT,
    first_seen   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen    TIMESTAMPTZ,
    status       TEXT NOT NULL DEFAULT 'online'
                 CHECK (status IN ('online','stale','suspect','quarantined')),
    whitelisted  BOOLEAN NOT NULL DEFAULT false,
    client_cert_fp TEXT
);

CREATE TABLE events (
    event_id     UUID NOT NULL,
    ts           TIMESTAMPTZ NOT NULL,
    ingested_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    node_id      TEXT NOT NULL REFERENCES nodes(node_id),
    name         TEXT NOT NULL,
    seq          BIGINT,
    entry_hash   BYTEA,
    prev_hash    BYTEA,
    integrity    TEXT NOT NULL DEFAULT 'unverified'
                 CHECK (integrity IN ('ok','broken','fork','gap','unverified','rejected_sig')),
    payload      JSONB NOT NULL,
    PRIMARY KEY (event_id, ts)
);
SELECT create_hypertable('events', 'ts');
CREATE INDEX idx_events_name    ON events (name, ts DESC);
CREATE INDEX idx_events_node    ON events (node_id, ts DESC);
CREATE INDEX idx_events_msg     ON events ((payload->>'msg_log_id'), ts DESC);
CREATE INDEX idx_events_payload ON events USING gin (payload);
SELECT add_retention_policy('events', INTERVAL '90 days');

CREATE TABLE messages (           -- projection : état par message
    msg_log_id   TEXT PRIMARY KEY,          -- hex 16 o
    conv_hash    TEXT,
    first_seen   TIMESTAMPTZ NOT NULL,
    last_event   TIMESTAMPTZ NOT NULL,
    status       TEXT NOT NULL DEFAULT 'unknown'
                 CHECK (status IN ('queued','in_flight','delivered','read','expired','unknown')),
    status_at    TIMESTAMPTZ,
    hop_count    INT NOT NULL DEFAULT 0,
    delivery_latency_ms BIGINT
);

CREATE TABLE message_hops (
    msg_log_id   TEXT NOT NULL REFERENCES messages(msg_log_id),
    node_id      TEXT NOT NULL REFERENCES nodes(node_id),
    ts           TIMESTAMPTZ NOT NULL,
    ttl_in       INT, ttl_out INT, fanout INT, rssi INT,
    kind         TEXT,   -- 'relay' | 'envelope_store' | 'envelope_handoff' | 'delivered'
    PRIMARY KEY (msg_log_id, node_id, ts)
);

CREATE TABLE links (
    a_node       TEXT NOT NULL REFERENCES nodes(node_id),
    b_node       TEXT NOT NULL REFERENCES nodes(node_id),
    last_seen    TIMESTAMPTZ NOT NULL,
    rssi_avg     INT,
    pkt_count    BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (a_node, b_node)
);

CREATE TABLE ledger_state (
    node_id      TEXT PRIMARY KEY REFERENCES nodes(node_id),
    height       BIGINT NOT NULL DEFAULT 0,
    root         BYTEA,
    integrity    TEXT NOT NULL DEFAULT 'unverified',
    checked_at   TIMESTAMPTZ
);

CREATE TABLE ledger_forks (
    node_id      TEXT NOT NULL,
    height       BIGINT NOT NULL,
    root         BYTEA NOT NULL,
    reporters    TEXT[] NOT NULL,
    first_seen   TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (node_id, height, root)
);

CREATE TABLE operators (
    id           BIGSERIAL PRIMARY KEY,
    email        TEXT UNIQUE NOT NULL,
    pw_hash      TEXT NOT NULL,             -- argon2id
    totp_secret  TEXT,
    role         TEXT NOT NULL CHECK (role IN ('viewer','admin')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE alerts (
    id           BIGSERIAL PRIMARY KEY,
    kind         TEXT NOT NULL,
    node_id      TEXT,
    payload      JSONB NOT NULL,
    raised_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at  TIMESTAMPTZ
);
```

Continuous aggregates Timescale utiles : trafic/minute par nœud, taux de
livraison glissant, densité moyenne.

### 12.3 Formats de fils / échange

**QR code de contact** : `dengon:v1:<base64url( B )>` avec
`B = pseudo_len:u8 ‖ pseudo:utf8 ‖ pub_static:32 ‖ pub_sign:32`.

**Code de vérification** :
```
fpA = SHA-256(pub_static_A ‖ pub_sign_A)         (32 o)   ;  fpB = idem pour B
material = SHA-512( min(fpA,fpB) ‖ max(fpA,fpB) )  (64 o)
pour i in 0..12 : g[i] = be_u16(material[2i..2i+2]) mod 100000
affichage : 12 groupes de 5 chiffres, zero-paddés
```

**Entrée de journal (canonical JSON, avant hash)** :
```json
{"event":"envelope.stored","payload":{"copy_budget":4,"epoch_day":20340,"msg_log_id":"4d5e…","recipient_tag":"0011…"},"prev_hash":"<hex32>","seq":10433,"ts_ms":1725800000455}
```
Clés triées lexicographiquement, pas d'espace, entiers minimaux.
`entry_hash = SHA-256(bytes_utf8)` ; `sig = Ed25519(priv_sign, entry_hash)`.

**Enveloppe scellée (payload de `SEALED_ENVELOPE`)** :
```
recipient_tag:16 ‖ epoch_day:u16 ‖ noise_x_ciphertext
  noise_x en clair : app_frame_len:u16 ‖ AppFrame ‖ sender_pub_static:32 ‖ sender_sig:64
  (sender_sig = Ed25519 sur SHA-256(AppFrame ‖ recipient_pub_static))
```

### 12.4 Rétention & tailles indicatives

| Donnée | Où | Rétention | Ordre de grandeur |
| --- | --- | --- | --- |
| messages (clair) | client | jusqu'à suppression manuelle | ~200 o / msg |
| ledger | client / relais | roulement (garder N dernières ou X jours) | ~300 o / entrée |
| held_envelopes | relais / client porteur | 24 h + budget | ≤ 4 Ko / enveloppe |
| events | dashboard | 90 j (config) | ~400 o / event |
| message_hops | dashboard | suit `messages` | ~80 o / saut |

---

## 13. Benchmarks & choix technologiques

### 13.1 Comparatifs `powl/01` (décisions figées)

**§1 — « Le réseau ressemble à une blockchain »** : voir §4.9-4.10 ci-dessus.
Formulation retenue : *DTN à routage gossip + micro-registre local chaîné et
signé par appareil, agrégé et audité par le dashboard.* Le registre sert à
l'audit, pas au routage.

**§2 — Plateforme de l'app cliente** : contrainte déterminante = un vrai nœud
mesh doit être **simultanément** GATT peripheral + GATT central + actif en
arrière-plan.

| Option | Peripheral + Central | Arrière-plan | Verdict |
| --- | --- | --- | --- |
| **Android natif (Kotlin)** | ✅ `BluetoothGattServer` + `BluetoothLeScanner` | ✅ foreground service (restrictions gérables) | ✅ **Cible n°1 (MVP)** (~70 % du parc mondial) |
| iOS natif (Swift) | ⚠️ central OK ; peripheral **fortement bridé** en fond | ⚠️ throttlé | 🟡 Cible n°2, co-conçue, **hors MVP** |
| Flutter / React Native | ⚠️ dépend de plugins tiers ; peripheral partiel voire absent | ⚠️ pire qu'en natif | ❌ Rejeté pour un **produit** mesh |
| Desktop / CLI (Rust + `btleplug`) | ✅ (Linux/BlueZ, macOS, Windows) | ✅ (process/daemon) | ✅ **Retenu comme `dengon-node`** |
| Web (Web Bluetooth) | ❌ central only, pas d'advertising, rien en fond | ❌ | ❌ Rejeté |

**Décision** : cœur partagé en Rust + liaison BLE native par plateforme. MVP
livré = `dengon-core` + `dengon-node` + `dengon-app` (Android).

**§3 — Modèle de sécurité des messages**

| Option | Verdict |
| --- | --- |
| Signatures seules (Ed25519), contenu en clair | ❌ insuffisant |
| **Noise `XX` (session) + Noise `X` (scellé offline) + Ed25519 (paquet)** | ✅ **Retenu** (FS sur session live ; ❌ sur scellé ; PCS partiel ; complexité moyenne) |
| Signal : X3DH + Double Ratchet | 🟡 v2 (état de ratchet lourd en multi-hop / multi-device) |
| MLS (RFC 9420) | 🟡 seulement si les **groupes** deviennent centraux |
| Registre chiffré répliqué / « vraie » blockchain | ❌ (liveness) |

**§4 — Firmware du relais ESP32**

| Option | Verdict |
| --- | --- |
| **ESP-IDF (C) + host NimBLE** | ✅ **Retenu (MVP)** (production, doc riche, coexistence Wi-Fi STA + BLE) |
| `esp-rs` (`esp-hal` + `esp-wifi` + `bleps`/`trouble`) | 🟡 **cible d'évolution** (BLE encore jeune, API mouvantes) |
| Arduino core + `NimBLE-Arduino` | ❌ (faible réutilisation du core, couches d'abstraction opaques) |

**ESP-IDF + NimBLE** : host léger (~40 Ko RAM de moins que Bluedroid, init plus
rapide). Carte : **ESP32-WROOM-32** (~300 Ko SRAM, pas de PSRAM) = 🟡 prototype
only ; **ESP32-WROVER(-E)** (+ 4–8 Mo PSRAM) = ✅ **Retenu**.

**§5 — Stack du dashboard (VPS)** : ingestion = **MQTT over TLS** (Mosquitto/EMQX)
✅ retenu ; HTTPS POST par batch 🟡 fallback clients opt-in ; gRPC streaming ❌.
Backend = **Rust + Axum** ✅ retenu (réutilise les types d'événements et la vérif
hash-chain de `dengon-core`) ; Node.js + TS (Fastify) 🟡 acceptable si contrainte
d'équipe. Stockage = **PostgreSQL 16 + TimescaleDB** (hypertable événements +
tables relationnelles). Frontend = **React + TS + Vite** + WebSocket temps réel ;
lib graphe `d3-force` / `cytoscape`. Déploiement = **Docker Compose** (mosquitto,
api, db, web, caddy) ; Grafana optionnel pour les métriques système du VPS.

**Récapitulatif des décisions** (identique au §1 ci-dessus).

### 13.2 Benchmark couche par couche `oswin/09`

Critères : facilité débutant, adéquation, communauté/ressources, coût, pérennité
(⭐ faible → ⭐⭐⭐⭐⭐ fort).

**Couche 1 — Framework de l'app mobile** : **Flutter (Dart)** ✅ recommandé
(1 langage, build APK en une commande, UI rapide, communauté énorme) ; **Kotlin
natif** 👍 si contrôle BLE maximal (plus verbeux) ; React Native ➖ (BLE
périphérique faible) ; MIT App Inventor ❌ (pas de crypto/mesh sérieux). Choix
`oswin` : **Flutter**, repli **Kotlin natif** si le rôle périphérique BLE pose
problème au palier 3.

**Couche 2 — Bibliothèque BLE (app Flutter)** : `flutter_blue_plus` (central) ✅
pour démarrer (paliers 1–2) ; `bluetooth_low_energy` (central + périphérique) ✅
pour le mesh complet (palier 3) ; `ble_peripheral` (périphérique) 👍 ;
`flutter_reactive_ble` (central) 👍 alternative. Choix : commencer avec
`flutter_blue_plus`, migrer/compléter avec `bluetooth_low_energy` au palier 3.
(En Kotlin natif, tout passe par `BluetoothLeScanner` / `BluetoothLeAdvertiser`.)

**Couche 3 — Environnement firmware ESP32** : **Arduino (framework) + PlatformIO**
✅ recommandé ; Arduino IDE seul 👍 pour débuter ; **ESP-IDF** ➖ puissant mais
raide (requis seulement pour la pile Bluetooth Mesh normalisée, inutile ici) ;
MicroPython ➖ (BLE/crypto moins matures). Choix `oswin` : framework Arduino édité
via PlatformIO. *(`powl` retient ESP-IDF — voir divergence §A-2 / §13.1 §4.)*

**Couche 4 — Cryptographie** : mêmes primitives des deux côtés — X25519,
AES-256-GCM ou ChaCha20-Poly1305, Ed25519. **Bibliothèque éprouvée, jamais de
crypto maison.** Côté app : `sodium_libs / sodium` (libsodium) ✅ recommandé ;
`cryptography` (pur Dart) 👍 ; `pointycastle` ➖. Côté ESP32 : **Arduino
Cryptography Library (rweather)** ✅ recommandé (Ed25519, Curve25519,
ChaCha20-Poly1305, **AES accéléré matériel ESP32**) ; libsodium port ESP32 👍
(cohérent avec l'app) ; mbedTLS 👍 (déjà présent, API austère). Choix `oswin` :
`sodium_libs` (app) + libsodium ou rweather (ESP32) ; le plus important = **fixer
un format d'octets identique** des deux côtés.

**Couche 5 — Broker MQTT** : **Mosquitto** local ✅ pour dev/démo ; **HiveMQ
Cloud** ✅ si démo « en ligne » ; broker intégré ThingsBoard 👍 ; broker public de
test ➖ (jamais de données réelles). Toujours **auth + TLS** hors réseau local.

**Couche 6 — Dashboard** : **Node-RED (+ dashboard)** ✅ le plus pédagogique
(câblage visuel) ; **ThingsBoard** ✅ le plus « pro » (widgets, gestion
d'appareils, broker MQTT inclus) ; Grafana + InfluxDB 👍 (courbes temporelles) ;
page web maison (MQTT-over-WebSocket) 👍. Choix `oswin` : Node-RED pour aller vite
et bien expliquer ; ThingsBoard si rendu vitrine pour la soutenance. *(`powl`
retient une stack maison React + Axum — voir divergence §A-5.)*

**Couche 7 — Persistance locale** : côté app (Flutter) = Isar ou Hive (bases
locales rapides), ou SQLite (`sqflite`/`drift`) → choix `oswin` : Hive/Isar. Côté
ESP32 = **NVS** (clé-valeur, intégré) pour de petites files ; **LittleFS** ou
**SPIFFS** pour un vrai FS en flash → choix : NVS pour commencer, LittleFS si les
volumes grandissent.

**Stack recommandée `oswin` (§9)** : Flutter + `flutter_blue_plus`/
`bluetooth_low_energy` + Arduino (PlatformIO) + `sodium_libs`/libsodium/rweather +
Mosquitto/HiveMQ + Node-RED/ThingsBoard + Hive/Isar + NVS/LittleFS. **Alternative
« contrôle maximal »** : Kotlin natif + ESP-IDF + libsodium/mbedTLS +
ThingsBoard. Rationale : cohérence crypto (libsodium des deux côtés) ;
progressivité (`flutter_blue_plus` couvre 80 % avant de toucher au périphérique
BLE) ; ressources (Flutter + Arduino + Node-RED = trois écosystèmes les mieux
documentés) ; coût 100 % gratuit / open source.

**Points de vigilance transverses** : fixer le « format de message » avant tout
code ; tester le rôle périphérique BLE des téléphones dès la semaine 1 ; ne
jamais coder sa propre crypto ni bricoler les nonces ; sécuriser MQTT (auth +
TLS) hors réseau local ; versionner les choix (noter les versions exactes des
libs).

### 13.3 Étude de stack `olivier/etude-stack.md` (brouillon v0.1)

**Le point dur = le Bluetooth de l'app mobile.** Un téléphone dans le maillage
doit jouer **deux rôles en même temps** : *chercheur* (scanner) **et**
*visible/serveur* (s'annoncer + accepter les connexions).

**Bibliothèques BLE comparées**

| Option | Chercheur | Visible/serveur | Arrière-plan | Maintenue | Remarque |
| --- | :---: | :---: | :---: | :---: | --- |
| **Flutter · flutter_blue_plus** | ✅ | ❌ | partiel | ✅ très active | La plus populaire, *central only*. |
| **Flutter · bluetooth_low_energy** | ✅ | ✅ (Android/iOS/Windows) | oui, avec config | ✅ | **Seule option multiplateforme couvrant les deux rôles.** Moins éprouvée. |
| **Flutter · ble_peripheral** | ❌ | ✅ | — | ✅ | Complément « serveur » si on garde flutter_blue_plus. |
| **RN · react-native-ble-plx** | ✅ | ❌ | partiel | ✅ | « ne permet pas la communication entre téléphones ». |
| **RN · react-native-peripheral** | ❌ | ✅ | — | ⚠️ petit projet | À combiner avec ble-plx. |
| **RN · react-native-ble-advertiser** | ❌ | annonce seule | — | ❌ ~4 ans sans MAJ | À éviter. |
| **Natif Android · API Kotlin / lib Nordic** | ✅ | ✅ | contrôle total du service de fond | ✅ | Le plus fiable ; non réutilisable iOS. |

**Contraintes d'arrière-plan Android récent** : **Android 14** — un service de
fond BLE doit déclarer `foregroundServiceType="connectedDevice"` **et** la
permission `FOREGROUND_SERVICE_CONNECTED_DEVICE`, sinon tué. **Android 15** —
restrictions supplémentaires sur le scan BLE en fond, lancement direct d'un
service de fond bloqué sans exemption, en **Doze profond** les scans sont différés
et les connexions coupées. Conséquences : notification permanente incontournable ;
un téléphone posé écran éteint relaie mal → le **mode éco** et les **relais ESP32
fixes** prennent tout leur sens ; pour la démo, garder les écrans allumés.

**Pourquoi l'iPhone est reporté** : en arrière-plan, un iPhone en rôle « visible »
**n'annonce plus son nom** et place ses identifiants de service dans une **« zone
de débordement » (*overflow area*)** — découvrables **uniquement par un autre
appareil Apple qui scanne explicitement cet identifiant précis**. Un ESP32 ou un
Android risquent de **ne pas voir** un iPhone en arrière-plan. Comportement
**système, non contournable**.

**Firmware ESP32** : pile Bluetooth — **Bluedroid** (RAM/flash élevées, BLE + BT
Classic) vs **NimBLE** (**~40 à ~100 Ko de moins**, **~50 % de flash en moins**,
BLE seul) → **NimBLE** pour dengon. ESP-IDF (C) recommandé pour du maillage
« sérieux », cœur Arduino plus rapide à prototyper (compromis : prototype Arduino
+ NimBLE-Arduino, puis bascule ESP-IDF). BLE Mesh natif (**ESP-BLE-MESH**, pile
Zephyr) = norme Bluetooth Mesh du SIG (publish/subscribe, clés réseau/application
partagées) → **pas** le modèle dengon ; mieux vaut partir de **NimBLE brut**.

**Un moteur ou deux ?** Option A — deux implémentations (une par langage) : simple
à démarrer, pas d'outillage ; **risque de dérive**. Option B — noyau partagé en
Rust, compilé en bibliothèque native, appelé via **UniFFI** (téléphone) et
bibliothèque C (ESP32) ; une seule source de vérité ; **outillage à monter,
courbe d'apprentissage, lourd pour 3 semaines**. → **Recommandation `olivier`
v1 : Option A**, avec en garde-fou une spec de trame binaire très précise
(le format fait foi, pas le code). **Option B comme évolution post-soutenance.**
*(`powl` retient l'Option B — voir divergence §A-2.)*

**Inspiration Meshtastic** : « managed flood routing » — limite de sauts = 7 (et
non « infini ») ; un nœud ne rediffuse que si la limite de sauts n'est pas nulle
**et** qu'il n'a pas déjà entendu ce paquet ; **« écouter avant de rediffuser »**
(attente courte, si un voisin est déjà en train de rediffuser, s'abstenir → évite
les tempêtes) ; la fenêtre d'attente dépend de la qualité du signal (les nœuds
**lointains** rediffusent en premier, les **proches** se taisent alors). → Impact
sur `protocole.md` : limite de sauts de départ = **7**, ajout de « écouter un
court instant avant de rediffuser » (v0.2 du protocole).

**Serveur du dashboard `olivier`** : petite API HTTP + base + canal temps réel +
page statique. API + logique = Node/Express, Python/FastAPI, Go → **FastAPI** ou
**Express** conviennent (au choix de qui prend le dashboard). Base = **SQLite**
suffit pour le volume d'une démo. Temps réel = **SSE** (sens unique serveur →
page, plus simple). Page = page légère (vanilla ou petit framework).

**Recommandation par brique `olivier` (§7)**

| Brique | Recommandation v1 | Repli / alternative |
| --- | --- | --- |
| App mobile | Flutter + `bluetooth_low_energy` si on vise l'iPhone plus tard ; natif Kotlin si fiabilité BLE maximale | Basculer sur Kotlin natif si `bluetooth_low_energy` déçoit |
| Validation techno | Prototype « hello mesh » en semaine 1 : 2 téléphones + 1 relais, 1 message qui passe. Go / no-go. | — |
| Moteur dengon | **Deux implémentations** encadrées par une spec de trame binaire stricte | Noyau Rust + UniFFI en v2 |
| Firmware ESP32 | ESP-IDF + NimBLE, GATT + annonce « bruts » | Arduino + NimBLE-Arduino pour prototyper |
| Crypto téléphone | libsodium (`crypto_box`) | — |
| Crypto ESP32 | mbedTLS (déjà présent) | libsodium porté ESP-IDF |
| Serveur dashboard | FastAPI ou Express + SSE + SQLite sur le VPS | Page rafraîchie à la main / maquette |

---

## 14. Périmètre MVP & feuille de route

### 14.1 Definition of Done du MVP (`powl/10 §1`)

Le MVP est atteint quand, sur du **vrai matériel** :

1. Deux téléphones Android à portée échangent un message texte chiffré, avec
   passage des statuts **en attente → parti → distribué → lu**.
2. Un 3ᵉ appareil hors de portée est joint via **au moins un relais ESP32**.
3. Un destinataire **éteint** reçoit son message à son retour (enveloppe
   scellée) ; l'expéditeur, **déconnecté** entre-temps, voit ses statuts se
   mettre à jour à sa reconnexion.
4. Le **dashboard** (déployé sur le VPS) montre le parcours du message, la carte
   du réseau, l'état des relais, et **détecte une altération** de journal simulée.
5. Contact établi par **QR + code de vérification** ; changement de clé détecté.
6. Sécurité : E2E vérifié (un relais capturé ne révèle aucun clair), `cargo
   audit` propre, revue crypto tierce passée.
7. CI verte : tests `dengon-core`, simulateur multi-nœuds, build des 4 cibles
   (core, node, android, firmware), lint dashboard.

### 14.2 Lots de livraison (`powl/10 §2`)

| Lot | Contenu | Jalon |
| --- | --- | --- |
| **Lot 0 — Fondations & spikes** | Workspace Rust, CI de base, conventions. **Spike A** : `dengon-core` (`protocol` + `crypto`) cross-compile-t-il pour xtensa-esp32 ? → décide « crypto en Rust » vs « trait `Crypto` + mbedTLS ». **Spike B** : `btleplug` en rôle **peripheral** sur Linux → valider le double rôle GATT. **Spike C** : Android `BluetoothGattServer` + scan en foreground service, 2 appareils échangent 20 o. | Rapport de décision + squelette des crates |
| **Lot 1 — `dengon-core` : protocole & crypto** | `protocol` (encode/decode paquets, fragmentation L2, round-trip + property tests) ; `crypto` (Ed25519, Noise `XX`, Noise `X`, `recipient_tag`, padding) ; `identity` (keypair, QR, code de vérification) ; `ledger` (append + `verify_chain` + export). | Vecteurs Noise, forge de signature rejetée, chaîne rompue détectée |
| **Lot 2 — `dengon-core` : store & sync** | `store` (impl SQLite complète) ; `sync::routing` (pipeline §6.6) ; `sync::gossip` (GCS, pull/push) ; `sync::status` (machine à états, outbox, rejeu) ; `sync::courier` (dépôt/collecte d'enveloppes, budget) ; `observability` (catalogue + JSON canonique). | Tests unitaires par module |
| **Lot 3 — `dengon-sim` + `dengon-node`** | `dengon-sim` (N instances de core reliées par transport en mémoire scriptable : latence, perte, partition, churn) ; scénarios rejoués ; `dengon-node` (CLI, transport `btleplug`). | Les 5 scénarios du DoD passent en simulation |
| **Lot 4 — `dengon-ffi` + app Android** | `dengon-ffi` (surface UniFFI) ; `AndroidTransport` (Kotlin + JNI : GATT server + scanner + foreground service) ; UI Compose (conversations, fil, saisie, statuts, écran QR + vérification, écran « réseau »). | 2 téléphones + 1 `dengon-node` → scénarios 1 et partiellement 2 |
| **Lot 5 — Firmware `dengon-relay`** | ESP-IDF + NimBLE, `libdengon_core.a` liée (selon décision Lot 0) ; tâches route/gossip/courier/ledger/ship ; client MQTT/TLS + buffer littlefs ; provisioning. | Scénarios 2 et 3 du DoD sur vrai matériel |
| **Lot 6 — Dashboard** | `api` (Axum) : ingest MQTT + vérif hash-chain + REST + WebSocket ; migrations Postgres/Timescale ; `web` (React) ; `deploy` (docker-compose + Caddy + Mosquitto, déployé sur le VPS). | Scénario 4 du DoD |
| **Lot 7 — Durcissement produit** | Chiffrement base au repos, quotas anti-DoS, gestion fine des erreurs ; revue crypto tierce ; `cargo deny`, SBOM ; OTA firmware signé, Secure Boot sur cartes de prod ; doc d'exploitation, runbook d'incident ; tests de charge (densité 20+ nœuds), tests de terrain. | — |

**Séquencement indicatif (unités abstraites)** : Lot 0 (0–2) → Lot 1 (3) → Lot 2
(4) ; Lot 3 (2, après Lot 2) → Lot 4 (3) et Lot 5 (3) après Lot 3 ; Lot 6 (4,
après Lot 2) ; Lot 7 (3, après Lot 5). Lots 5 et 6 parallélisables après le
Lot 3.

### 14.3 Explicitement hors MVP (`powl/10 §3`)

| Fonction | Pourquoi reporté | Prévu par l'archi ? |
| --- | --- | --- |
| App **iOS** | BLE périphérique/fond bridé, effort natif conséquent | oui (core + UniFFI/Swift) |
| **Groupes / canaux publics** | complexité (MLS ou signatures de canal), pas dans le brief | partiellement (type de paquet réservable) |
| **Pièces jointes** (images/fichiers) | débit BLE, fragmentation lourde, stockage | oui (fragmentation L2 déjà là) |
| **Double Ratchet** (Signal) pour le canal privé | Noise suffit au MVP ; gain = PCS renforcé | oui (couche `crypto` isolée) |
| Firmware relais **en Rust pur** (`esp-rs`) | écosystème BLE encore jeune | oui (même `dengon-core`) |
| **Backhaul par le VPS** (relai internet entre îlots) | choix explicite « observabilité seule » | non (rupture du modèle de confiance) |
| **Panic wipe / déni plausible** | durcissement avancé | oui |
| Multi-appareil pour un même compte | synchro d'état complexe | non (v2) |

### 14.4 Risques & mitigations (`powl/10 §4`)

| Risque | Impact | Mitigation |
| --- | --- | --- |
| `dengon-core` ne cross-compile pas pour ESP32 | firmware plus coûteux | Spike Lot 0 ; trait `Crypto`/`Store` déjà prévu ; repli mbedTLS |
| BLE périphérique instable selon OEM Android | maillage dégradé sur certains téléphones | matrice d'appareils testés ; `dengon-node` comme relais mobile d'appoint ; relais ESP32 densifient |
| Coexistence BLE+Wi-Fi sur ESP32 = débit faible | remontée de logs lente | WROVER + PSRAM ; batch MQTT ; logs best-effort par nature |
| Faux relais / pollution d'enveloppes | DoS stockage | signature obligatoire des `SEALED_ENVELOPE` ; quotas ; budget de copies |
| Analyse de trafic par corrélation | fuite de métadonnées | padding, tags tournants ; limite documentée (adversaire global hors périmètre) |
| Dérive d'horloge des nœuds | statuts/latences faux | NTP relais ; fenêtre de tolérance ; alerte `clock_skew` |
| Charge dashboard si réseau grandit | perte d'événements | Timescale + rétention + agrégats ; QoS 1 + dédup ; logs pas critiques |
| Périmètre qui gonfle (groupes, fichiers…) | MVP jamais fini | `10-mvp-scope-roadmap.md` fait foi ; tout ajout = après Lot 7 |

### 14.5 Feuille de route `oswin/08` (approche débutants, livraison par paliers)

**MVP `oswin`** : un message chiffré part du **téléphone A**, est **reçu et
stocké par un ESP32** alors que le destinataire est absent, puis **retransmis**
(via un 2ᵉ ESP32) au **téléphone B** qui le **déchiffre**, avec **remontée des
métadonnées sur le dashboard** et **accusé de réception** qui purge le stockage.
= **paliers 1 + 2** = couvre déjà les 6 points du cahier des charges. Mesh
téléphone-à-téléphone (palier 3) = objectif étendu.

**Paliers** : **P1** téléphone ↔ 1 ESP32 (envoi chiffré + affichage + dashboard)
= socle garanti ; **P2** 2 ESP32 qui stockent et relaient (multi-sauts + ACK +
purge) = **MVP = note visée** ; **P3** mesh téléphone-à-téléphone (rôle
périphérique BLE sur Android) = objectif étendu / bonus. Règle d'or : on ne
commence un palier que lorsque le précédent est démontrable.

**6 chantiers (workstreams)** : (1) protocole & format de message (contrat commun
app ↔ ESP32, à figer tôt) ; (2) firmware ESP32 ; (3) application Android ; (4)
sécurité / cryptographie (transversal) ; (5) dashboard & supervision ; (6)
intégration, tests & documentation.

**Phases & jalons (DoD)** : Phase 0 (cadrage & mise en route — figer le format de
message v1) ; Phase 1 (palier 1 : lien de base — DoD : « bonjour » apparaît
déchiffré côté ESP32 **et** un événement s'affiche sur le dashboard, contenu
illisible si sniffé) ; Phase 2 (palier 2 : stockage & relais = le MVP — DoD :
scénario complet « A → (ESP32 stocke) → … → B déchiffre → ACK → purge »
reproductible, visible sur le dashboard) ; Phase 3 (palier 3 : mesh
téléphone-à-téléphone, bonus — prérequis : confirmer que les téléphones
supportent le rôle périphérique BLE) ; Phase 4 (durcissement, tests & démo — DoD :
la démo tourne 3 fois de suite sans intervention, rapport complet).

**Planning indicatif (semaines relatives)** : S1 Phase 0 (environnements OK +
format v1 figé) ; S2 Phase 1 (palier 1 démontrable) ; S3 Phase 2a (file de
stockage + relais ESP32↔ESP32) ; S4 Phase 2b (ACK + purge + métriques → **MVP**) ;
S5 Phase 3 (mesh téléphone-à-téléphone si faisable) ; S6 Phase 4 (durcissement +
démo répétée + rapport).

**Répartition des tâches** : référent protocole & intégration (chantiers 1 + 6) ;
dév. firmware ESP32 (2, +5) ; dév. application Android (3) ; référent sécurité
(4) ; référent dashboard (5). À 2 personnes : (A) firmware + dashboard, (B) app +
protocole ; sécurité et intégration partagées.

**Critères d'acceptation (mapping cahier des charges)** : envoyer via BT (démo +
log) ; sécuriser (montrer qu'un sniff BLE ne donne que du chiffré) ; recevoir via
BT (message déchiffré affiché) ; stockage aux points de transmission (couper le
lien vers B, montrer le message en attente dans l'ESP32, puis livraison à la
reconnexion) ; transmission entre points (scénario multi-sauts tracé sur le
dashboard) ; passage par Arduino → dashboard (événements horodatés visibles).

**Feuille de route en 3 paliers (`oswin/07 §7`)** : **Palier 1** — le lien de base
(téléphone central ↔ 1 ESP32 périphérique) ; **Palier 2** — le relais et le
stockage (2ᵉ ESP32, store-carry-forward, multi-sauts, ACK, purge) ; **Palier 3**
— le mesh téléphone-à-téléphone (rôle périphérique sur les téléphones via
`bluetooth_low_energy`), à tenter **seulement une fois les paliers 1–2 solides**
et **après avoir vérifié** que les téléphones supportent l'advertising.
Recommandation : viser **paliers 1 + 2** comme livrable garanti, présenter le
**palier 3** comme aboutissement / perspective.

### 14.6 Cadrage v1 & planning `olivier/decisions-v1.md`

> ⚠️ **Soutenance le 29/09/2026** — ~3 semaines à partir du 08/09/2026. Très court
> pour « étude + démo à parts égales » sur un protocole maison + chiffrement +
> dashboard + Android + ESP32.

**Décisions de cadrage v1 (à valider Paul/Tanguy)** : (1) objectif = **étude de
conception + démo fonctionnelle** à parts égales, périmètre resserré ; (2)
appareils v1 = **Android + ESP32** (iPhone reportés) ; (3) échanges **1-à-1
uniquement**, pas de groupes ; (4) dashboard de suivi **inclus** mais minimal ;
(5) **protocole maison** (inspiré Meshtastic/Briar/Bluetooth Mesh) ; (6)
**chiffrement du contenu dès la v1** ; (7) ajout de contact **en présentiel
uniquement** (scan QR) ; (8) statut **« Lu » reporté en v2** (v1 : En attente →
Parti → Distribué).

**Planning `olivier` (~3 semaines)** : Conception ~1 semaine (figer la spec du
protocole, priorité = circulation des messages ; cadrer sécurité + architecture ;
dépôt commun à la fin) ; Démo ~1,5 semaine (démo réduite + dashboard minimal, en
commun) ; Rendu ~0,5 semaine (finalisation docs, rapport, slides ; répétition de
la soutenance).

**Impact du délai — orientation `olivier` (à valider à trois)** : *« Recentrer la
démo, garder l'étude complète »* — l'étude (protocole, sécurité, architecture)
reste le socle ; la démo est réduite au minimum. Priorité à creuser = **la
circulation des messages** (propagation, boucles, condition d'arrêt).
**Ordre de repli si le temps manque** : (1) hors-ligne + relais (le cœur) ; (2)
sécurité (relais aveugle) ; (3) dashboard. Pistes : recentrer la démo sur 2
téléphones + 1 relais, 1-à-1 texte, propagation 1-2 sauts, statut jusqu'à
« Distribué » ; dashboard vraiment minimal (liste live des ID + statut) ou
maquette ; ESP32 optionnel pour la démo (peut rester « étude + PoC ») ; mettre
le poids sur l'étude ; se mettre en commun très vite.

**Livrables attendus (avis Olivier)** : spec du protocole, doc sécurité, doc
architecture, **+ rapport écrit + support de présentation** (rapport +
présentation confirmés comme attendus de la soutenance).

### 14.7 Chantier « semaine 1 » proposé (`olivier/mise-en-commun §5`)

| Tâche | Pourquoi maintenant | Proposition |
| --- | --- | --- |
| **Prototype « hello mesh »** : 2 téléphones + 1 relais, 1 message qui passe | go / no-go de la techno mobile | Paul ou Tanguy |
| **Doc sécurité** (`docs/protocole/securite.md`) | référencée par `protocole.md` et `format-trame.md` ; bloque la crypto | Paul ou Tanguy |
| **Monter le dépôt commun** (migration des docs) | fin de la phase conception | Olivier |
| **Attribution des composants** (mobile / ESP32 / dashboard) | conditionne toute la phase démo | Réunion |
| **Plan de la démo réduite** + scénario campus concret | cadre le développement | Réunion, puis Olivier rédige |

---

## 15. Stratégie de test & CI

> Source : `powl/11-testing-strategy.md`, complété par `oswin/08 §9`.

### 15.1 Pyramide (`powl/11 §1`)

E2E terrain (manuel) — scénarios DoD sur vrai matériel ; Intégration — dengon-sim
(multi-nœuds), dashboard e2e, banc ESP32 ; Composant — chaque module de
`dengon-core`, api dashboard ; Unitaire + property — `protocol`, `crypto`,
`ledger`, status FSM. Principe : **le maximum de logique est dans `dengon-core`**,
testable sans radio ni réseau ; BLE et MQTT aux frontières, mockés.

### 15.2 `dengon-core` — tests unitaires & property (`powl/11 §2`)

| Module | Tests clés |
| --- | --- |
| `protocol` | round-trip encode/decode (tout type) ; property `decode(encode(p)) == p` (`proptest`) ; fragmentation/réassemblage avec MTU aléatoire ; rejet des paquets malformés / version inconnue |
| `crypto` | vecteurs de test Noise `XX`/`X` ; sign/verify Ed25519 ; forge rejetée (bit-flip → verify échoue) ; `recipient_tag` stable sur la journée, différent le lendemain ; padding → taille ∈ `PAD_BUCKETS` ; déchiffrement d'enveloppe avec mauvaise clé échoue proprement |
| `identity` | QR encode/decode ; code de vérification identique des deux côtés et ordre-indépendant ; sensible à un bit de clé |
| `ledger` | `verify_chain` OK sur chaîne valide ; détecte entrée modifiée, `seq` manquante, mauvaise signature ; `export(range)` cohérent ; reprise après « redémarrage » (continuité `prev_hash`) |
| `sync::routing` | dedup (même `msgID` 2× → 1 relais) ; TTL décrémenté, 0 → drop ; clamp densité ; `RELAY_OK=0` → pas de relais ; abandon si doublon pendant le jitter |
| `sync::gossip` | GCS : pas de faux négatif ; réconciliation A/B convergente ; `GOSSIP_PUSH` repasse par le pipeline (re-dedup) |
| `sync::status` | FSM exhaustive : toutes transitions valides, toutes invalides rejetées ; monotonie ; ACK en double idempotent ; `read` sans `delivered` → double transition |
| `sync::courier` | budget de copies partagé correctement ; expiration ; collecte sur match de tag ; pas de fuite si tag ne matche pas |
| `observability` | chaque événement round-trip JSON canonique ; redaction : aucun `msg_uuid`/texte/recipient en clair ne peut être sérialisé (test négatif) ; `msg_log_id` bien haché |
| `store` (SQLite) | migrations ; idempotence `messages.insert(msg_uuid)` ; requêtes outbox ; chiffrement au repos actif |

Outils : `cargo test`, `proptest`, `cargo-nextest` en CI, `cargo llvm-cov`
(objectif ≥ 85 % sur `dengon-core`).

### 15.3 `dengon-sim` — intégration multi-nœuds (`powl/11 §3`)

Simulateur : N instances de `dengon-core` reliées par un `Transport` **en
mémoire** avec modèle réseau scriptable. Paramètres injectables : latence par
lien, taux de perte, bande passante, **partition** (couper un sous-ensemble),
**churn** (nœuds qui apparaissent/disparaissent), horloges désynchronisées.
Scénarios versionnés (`sim/scenarios/*.ron`) :

| Scénario | Vérifie |
| --- | --- |
| `direct.ron` | A→B connectés : livré, `delivered` puis `read` |
| `multihop.ron` | A→C via 2 relais : livré, chemin attendu, TTL cohérent |
| `recipient_offline.ron` | C absent → enveloppe déposée → C revient → livré + ACK remonte |
| `sender_offline.ron` | A envoie puis part → à son retour, statuts rattrapés |
| `partition_merge.ron` | réseau coupé en 2, messages des deux côtés, fusion → convergence complète |
| `flood.ron` | 500 msg/s injectés → pas d'explosion mémoire, dedup efficace, TTL borne la charge |
| `dup_paths.ron` | même message par 3 chemins → 1 seul affiché, 1 seul ACK |
| `tamper.ron` | un nœud altère son journal → `verify_chain` échoue, dashboard alerte |
| `key_change.ron` | clé de B change → messages suspendus, alerte MITM |

Déterminisme : seed RNG fixe → rejouable. Exécuté à chaque PR.

### 15.4 Dashboard — tests (`powl/11 §4`)

Unitaire (`api`) : parsing/validation de batch ; vérif signature ; reconstruction
de statut depuis une séquence d'événements ; détection fork/gap/broken (réutilise
`ledger`) ; redaction refusée. Intégration : `testcontainers`
(Postgres+Timescale éphémère), publier des batchs MQTT réels → vérifier
projections, idempotence. API : golden tests sur les réponses REST ; WebSocket :
un event ingéré → push reçu. Front : composants (Vitest + Testing Library) ; e2e
Playwright sur les 4 écrans avec BDD seedée. Charge : `k6` / injecteur (10k
events/min pendant 10 min).

### 15.5 Firmware ESP32 — tests (`powl/11 §5`)

Unitaire host (`libdengon_core` compilée pour l'hôte → mêmes tests que §15.2 pour
les modules embarqués) ; unitaire cible (Unity : `Store` NVS/littlefs, buffer
ring de logs wrap-around, curseur de journal persistant) ; intégration banc (2-3
ESP32 + 1 téléphone + broker MQTT local) ; résilience (coupure Wi-Fi 10 min ;
`esp_restart()` → journal reprend sans rupture de chaîne ; saturer PSRAM → refus
d'enveloppes mais routage OK) ; charge (injecteur BLE) ; conformité (vecteurs de
paquets partagés avec `dengon-core` : mêmes bytes in → mêmes décisions out).

### 15.6 E2E terrain — checklist de recette (`powl/11 §6`)

Recette exécutée avant chaque jalon « produit » : (1) **Contact** (QR, comparer
le code 60 chiffres, marquer « vérifié ») ; (2) **Direct** (A→B à 5 m, statuts
observés, latence notée) ; (3) **Multi-saut** (B s'éloigne à 40 m avec un relais
ESP32 au milieu → livré) ; (4) **Destinataire absent** (Charlie éteint son BLE ;
A→Charlie ; après 5 min, Charlie rallume près d'un relais → reçoit ; ACK revient à
A) ; (5) **Expéditeur absent** (A envoie puis coupe le BLE 5 min ; Charlie lit ;
A rallume → statut « lu ») ; (6) **Dashboard** (parcours de chaque message, carte
réseau, état des relais) ; (7) **Intégrité** (modifier une entrée de journal d'un
relais de test → dashboard lève `chain_broken` sous X minutes) ; (8) **Sécurité**
(capturer le trafic BLE avec nRF Sniffer → aucun clair ; VPS sans `msgID` en
clair ni contenu) ; (9) **Densité** (8-10 appareils dans une salle → tous se
voient, messages croisés livrés, pas d'effondrement). Résultats consignés dans un
rapport de recette daté.

### 15.7 CI (`powl/11 §7`)

| Job | Déclencheur | Contenu |
| --- | --- | --- |
| `core` | PR, push | `cargo fmt --check`, `clippy -D warnings`, `nextest`, `llvm-cov`, `proptest` |
| `sim` | PR, push | tous les scénarios `dengon-sim` (seed fixe) |
| `audit` | PR, quotidien | `cargo audit`, `cargo deny check`, SBOM |
| `android` | PR touchant `android/` ou `dengon-ffi/` | build UniFFI, `./gradlew assembleDebug testDebugUnitTest` |
| `firmware` | PR touchant `firmware/` | `idf.py build`, tests host de `libdengon_core`, tests Unity (QEMU si possible) |
| `dashboard` | PR touchant `dashboard/` | `cargo test` (api, testcontainers), `pnpm test` + `pnpm build` (web), Playwright |
| `cross-vectors` | PR, push | vecteurs de conformité partagés core ↔ firmware ↔ dashboard |
| `release` | tag | build artefacts (node binaries, APK, firmware .bin, images Docker), signe, publie |

Blocage de merge : `core`, `sim`, `audit`, `cross-vectors` verts obligatoires.

### 15.8 Revue de sécurité (`powl/11 §8`)

Avant le premier déploiement « produit » (fin Lot 7) : revue interne du modèle de
menace ; **audit crypto externe** de `dengon-core::crypto` + intégration Noise ;
test d'intrusion du dashboard (VPS) ; revue de la surface FFI (UniFFI) et du
firmware (gestion mémoire C).

### 15.9 Plan de test par brique (`oswin/08 §9`)

Crypto : chiffrer puis déchiffrer → identique ; signature vérifiée ; message
modifié → rejeté. Stockage : un message expiré est supprimé ; un ACK purge la
bonne entrée. Anti-doublon : un même message reçu 2× n'est relayé qu'une fois.
Passerelle : un événement ESP32 apparaît bien sur le dashboard.

---

## 16. Analyse de besoins & questions de cadrage

> Source : `olivier/analyse-besoins.md` (compléments à `CONTEXT.md` — besoins,
> questions ouvertes, risques à cadrer « dans un premier temps ») + les tables
> « avis d'Olivier » de `decisions-v1.md`. Beaucoup de ces questions sont
> aujourd'hui tranchées par `powl` ; ce qui reste ouvert est repris dans
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).

### 16.1 Les 10 sections de questionnement (`olivier/analyse-besoins`)

1. **Identité et adressage** : comment désigne-t-on un destinataire (clé
   publique, pseudo, identifiant court) ? un ou plusieurs destinataires
   (groupe) ? comment ajoute-t-on un contact (QR / NFC en présentiel) ? comment
   empêche-t-on l'usurpation d'un expéditeur ? multi-appareil ? *(→ `powl` :
   `peerID` 8 o, 1-à-1, QR + code 60 chiffres, signature Ed25519, multi-appareil
   hors v2.)*
2. **Format du message et du protocole** : le livrable central = la spéc du
   protocole (format binaire) commun aux 3 familles d'appareils. BLE MTU faible
   (~20 à ~500 octets) → fragmentation/réassemblage. Entêtes nécessaires : ID
   unique, expéditeur, destinataire, TTL/sauts, horodatage, numéro de séquence /
   nonce.
3. **Propagation (routage mesh)** : flooding vs routage → **flooding contrôlé**
   en v1 ; TTL / limite de sauts ; déduplication (table des ID vus) ; détection /
   cassure de boucles ; l'accusé de livraison doit **remonter** dans le mesh
   (routage retour, plus difficile) ; durée de vie / expiration ; priorité entre
   messages ?
4. **Store-and-forward** : chaque nœud stocke les messages non délivrés et les
   rejoue ; combien de temps ? quelle taille de file ? **contrainte forte
   ESP32** : RAM ~520 Ko, flash limitée → tampon réduit, politique d'éviction
   (FIFO, par TTL, par priorité) ; persistance locale sur mobile (base chiffrée).
5. **Sécurité** : E2EE du contenu (primitive éprouvée, ex. X25519 + AES-GCM /
   libsodium) ; intégrité / authenticité (signature ou MAC) ; anti-rejeu (nonce /
   compteur / horodatage) ; métadonnées (jusqu'où masquer expéditeur /
   destinataire ?) ; provisioning des clés (appairage hors-bande, QR en
   présentiel) ; stockage des clés (NVS chiffrée ESP32 ; Keystore / Keychain
   mobile) ; révocation si appareil compromis/perdu ; anti-spam / anti-DoS ;
   forward secrecy (probablement trop pour une v1).
6. **Contraintes des plateformes** (à valider **avant** de s'engager) : **iOS —
   risque technique n°1** (BLE en arrière-plan très bridé par CoreBluetooth →
   faire une preuve de faisabilité iOS dès le départ) ; **Android** (permissions
   BLE + localisation, services foreground avec notification obligatoire, Doze
   mode) ; **ESP32 FREENOVE WROOM** (BLE + BT Classic + Wi-Fi, ressources
   limitées, pas d'OS ; évaluer Bluetooth Mesh natif ESP-IDF vs protocole
   maison) ; **BLE = dénominateur commun** (Classic pas exploitable librement sur
   iOS) ; chaque téléphone doit être **central et périphérique en même temps** ;
   portée BLE réaliste intérieur ~10–30 m.
7. **Dashboard / observabilité** : qui pousse les données (les nœuds avec du
   Wi-Fi) ? où est hébergé le backend d'agrégation (le dashboard suppose une
   connexion, bonus non bloquant) ? quelles données (**ID de message, statut,
   horodatage, sauts, nœuds traversés — jamais le contenu** ; métadonnées
   sensibles) ? reconstituer le trajet à partir de rapports **partiels** ;
   **horloge** (pas de temps global fiable → horloges désynchronisées, prévoir
   une horloge logique type Lamport ou des horodatages best-effort assumés) ;
   temps réel vs remontée par lots.
8. **Expérience utilisateur** : notification à la réception (contrainte
   arrière-plan) ; comment l'utilisateur voit un message « En attente » depuis
   longtemps ; retour visuel sur l'état du réseau (nombre de voisins, « à
   portée » / « isolé ») ; historique local ; onboarding = ajout de contact et
   échange de clés en présentiel ; mode dégradé quand l'appareil est seul.
9. **Cadrage projet / non-fonctionnel** : écrire 2–3 scénarios d'usage concrets
   (festival, zone blanche, manifestation, catastrophe, campus…) ; définir un
   MVP resserré ; métriques de succès (taux de livraison, latence de bout en
   bout, nombre de sauts, impact batterie) ; **batterie** (consommation du
   scan/advertising BLE continu = critère de viabilité) ; stratégie de test
   (simulateur ? bancs d'essai ? tests terrain ?) ; aspects légaux (radio,
   chiffrement, RGPD sur les métadonnées du dashboard) ; licence / ouverture.
10. **Décisions à trancher en premier** : (1) protocole maison ou réutilisation
    d'un existant ? (2) BLE seul ou BLE + Classic ? (3) 1-à-1 uniquement ou aussi
    des groupes en v1 ? (4) modèle de confiance (TOFU en présentiel ? autorité ?)
    (5) relais iOS en arrière-plan = objectif v1 ou évolution ? (6) périmètre
    exact du MVP et répartition du travail entre Paul, Tanguy, Olivier.

### 16.2 Tables « avis d'Olivier » (`decisions-v1.md`) — à valider à trois

**Points d'équipe** : scénario d'usage à privilégier = **campus / bâtiment** ;
durée de conservation d'un message non délivré = **≈ 24 h** ; techno de l'app
mobile = **multiplateforme (Flutter ou React Native)** [Paul/Tanguy tranchent] ;
hébergement du dashboard = **VPS Debian de l'équipe** ; Olivier travaille sur
**protocole + dashboard**.

**Produit / UX** : données affichées par le dashboard = **parcours anonymisé**
(ID message, statut, appareils traversés en codes ; jamais expéditeur/
destinataire/contenu) ; taille de la démo finale = **5-8 appareils** ; type de
contenu (v1) = **texte court ~140-500 caractères** ; identité utilisateur =
**pseudo libre, sans vérification** ; fonctionnement en arrière-plan = **service
de fond avec notification permanente (Android)** ; conservation des messages sur
le téléphone = **jusqu'à suppression manuelle** ; info réseau montrée à
l'utilisateur = **indicateur simple** « Connecté au réseau » / « Isolé » ; langue
de l'interface (v1) = **français uniquement** ; relais par un téléphone sans
contact = **oui, toujours** ; réglage de consommation batterie = **oui, dès la
v1 : mode « économie » activable** ; vérification lors de l'échange de clés =
**oui, code court identique à comparer de visu après le scan** ; nom du projet =
**à rediscuter** (« dengon » reste le nom de travail) ; message expiré (~24 h) →
statut **« Échec »** sans notification ; relance d'un message non parti = **bouton
« Renvoyer »** ; blocage d'un contact indésirable = **reporté** après la v1 ;
dashboard : rafraîchissement = **mise à jour automatique**.

**Organisation** : livrables = spec du protocole, doc sécurité, doc architecture,
+ rapport écrit + support de présentation ; répartition **par composant** (mobile
/ ESP32 / dashboard), conception commune ; simulateur de réseau **léger**
(quelques scripts de test) ; rythme de mise en commun = **fusion dans un dépôt
commun après la phase de conception** (~fin de la semaine 1), puis travail en
commun.

**Points ouverts tranchés depuis (avis d'Olivier — reportés dans `protocole.md`
v0.3 et `dashboard.md` v0.2)** : ordre d'affichage = par **ordre d'arrivée** ;
taille de la file de retransmission = **~50 ESP32 / ~300 téléphone**, éviction du
**plus ancien** ; **échange d'inventaire** entre voisins = inclus en v1 ;
**anti-inondation** = protection simple en v1 (~20 messages/min/voisin) ; **code
anonyme du dashboard** = un code différent par message ; **rétention des données
du dashboard** = effacées après chaque session de démo ; **compteurs globaux du
dashboard** = inclus (sauf « nombre d'appareils actifs » — à décider) ; **mode
démo** = seulement s'il reste du temps.

### 16.3 Mise en commun proposée (`olivier/mise-en-commun.md`)

**Structure de dépôt proposée (NON adoptée)** : `docs/concept.md` (ex-CONTEXT),
`docs/decisions.md` (décisions VALIDÉES), `docs/planning.md`,
`docs/protocole/{comportement,format-trame,securite}.md`, `docs/architecture.md`,
`docs/dashboard.md`, `docs/stack.md`, `docs/{olivier,paul,tanguy}/` (brouillons
en archive), `app/`, `firmware/`, `dashboard/{serveur,page}/`, `outils/`,
`.githooks/`. Principe : les brouillons `olivier/` deviennent la **base** des
documents partagés. *(Réel : `docs/powl/` + `docs/suivi/`, découpage `crates/`.)*

**Ordre du jour proposé pour la réunion** : (1) confirmer / corriger le cadrage
v1 et les points « avis d'Olivier » ; (2) trancher les décisions techniques ; (3)
attribuer les composants et les tâches de la semaine 1 ; (4) valider la structure
du dépôt commun et la date de fusion ; (5) fixer un point d'avancement récurrent
(2-3 fois par semaine).

---

## 17. Glossaire

Fusion des vocabulaires de conception : termes de `powl` (disséminés dans
`00`–`11`), glossaire minimal `oswin/00 §6`, vocabulaire protocole
`olivier/protocole §2`.

| Terme | Définition |
| --- | --- |
| **dengon (伝言)** | « message que l'on confie à quelqu'un pour qu'il le transmette ». Nom du projet. |
| **BLE** | Bluetooth Low Energy. Radio courte portée, basse consommation, base du réseau. |
| **Maillage / mesh** | Réseau où chaque nœud relaie les messages des autres, sans point central. |
| **Nœud** | Un appareil qui fait tourner dengon (téléphone ou ESP32). |
| **Voisin** | Un nœud actuellement à portée Bluetooth directe. |
| **Saut / hop** | Un passage d'un nœud à un voisin. |
| **DTN** | *Delay-Tolerant Network*. Fonctionne même sans chemin complet à un instant donné : on stocke et on retransmet plus tard. |
| **Store-carry-forward** | Garder un message, le transporter physiquement (en se déplaçant), le retransmettre à la prochaine rencontre. |
| **Bundle Protocol v7 (RFC 9171)** | Standard IETF du DTN. Notions réutilisées : `bundle`, `lifetime`, `custody transfer`. |
| **Routage épidémique / gossip** | Propagation « de proche en proche » : à chaque rencontre, deux nœuds échangent ce qu'ils n'ont pas en commun. |
| **Managed flooding** | Diffusion à tous les voisins + relais, bornée par TTL + cache anti-doublon (routage de la norme Bluetooth Mesh, de Bitchat, de Meshtastic). |
| **Spray-and-Wait** | Ne diffuser qu'un nombre limité **L** de copies, puis attendre. Utilisé pour le budget de copies des enveloppes. |
| **TTL (time to live)** | Compteur de sauts d'un paquet ; décrémenté à chaque relais, jeté à 0. Empêche les boucles infinies. TTL initial = **7** (`TTL_DEFAULT`, aligné Bitchat/Meshtastic). |
| **Déduplication / seen-set / table « déjà vu »** | Mémoire des messages déjà vus (par `msgID`) pour ne pas les relayer en boucle. |
| **Jitter de relais** | Petit délai aléatoire avant de relayer, pour que les doublons s'annulent (« écouter avant de rediffuser »). |
| **Flood contrôlé / diffusion contrôlée** | Diffusion à tous les voisins, bornée par TTL + dédup + budget. |
| **peerID** | Identifiant court d'un nœud (`powl` : **8 octets** = début du hash de la clé publique ; `olivier` : « empreinte de clé » **16 octets**). Stable, pseudonyme. |
| **msgID** | Identifiant d'un paquet L3 = `SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload)` (32 o). Pour la dédup et le suivi. Tracé **haché** vers le dashboard (`msg_log_id = SHA-256(msgID)[0..16]`). |
| **msg_uuid** | Identifiant d'un **message applicatif**, stable de bout en bout (16 o, UUIDv4). C'est lui que suit la machine à états des statuts (le `msgID` change si le paquet est re-scellé). |
| **id_message** (olivier) | Identifiant d'un message dans la spec `olivier` : 16 octets aléatoires, stable réseau, jamais modifié par un relais. |
| **AppFrame** | Contenu applicatif L4 (à l'intérieur de Noise) : `Message`, `Ack`, `ReadReceipt`, `Profile`. |
| **Noise (XX / X)** | Cadre standard pour un canal chiffré. `XX` = session authentifiée bidirectionnelle (forward secrecy) ; `X` = message one-shot scellé vers une clé connue (enveloppe scellée, pas de FS). |
| **Forward secrecy** | Voler une clé aujourd'hui ne permet pas de déchiffrer les messages d'hier. |
| **Ed25519 / X25519** | Ed25519 = signatures (prouver qui parle) ; X25519 = accord de clés (établir un secret partagé). Même courbe (Curve25519), usages différents. |
| **AEAD** | Chiffrement authentifié (AES-256-GCM ou ChaCha20-Poly1305) : chiffre **et** protège l'intégrité en une opération. |
| **Enveloppe scellée** (`SEALED_ENVELOPE`) | Message chiffré pour un destinataire absent, laissé sur des relais jusqu'à son retour. Noise `X`. |
| **recipient_tag** | Tag anonyme **tournant** (change chaque jour : `HMAC-SHA256(pub_static_dest, "dengon-tag" ‖ day_u32)[0..16]`) désignant le destinataire d'une enveloppe sans révéler qui. |
| **Budget de copies** (`copy_budget`) | Nombre max de copies d'une enveloppe en circulation (inspiré de Spray-and-Wait). `COPY_BUDGET_INIT = 4`, `COPY_BUDGET_MAX = 8`. |
| **Outbox / file de retransmission** | File locale des messages envoyés-mais-pas-confirmés-livrés ; rejouée à chaque reconnexion. Taille max `olivier` : ~50 (ESP32) / ~300 (téléphone). |
| **Inventaire** (olivier) | PDU listant les `id_message` détenus, échangé entre voisins qui se rencontrent, pour n'envoyer que ce qui manque à l'autre. |
| **Fragmentation (L2)** | Découpage d'un paquet > MTU BLE en fragments (`frag_id`, `index`, `total`), réassemblés par le récepteur. `FRAG_SIZE = 440` (`powl`) / charge utile ~156 o (`olivier`). |
| **GATT / GAP** | Couches BLE : GATT = services & caractéristiques (données) ; GAP = découverte & rôles (central/peripheral). |
| **Central / Peripheral** | Rôles BLE : central scanne et se connecte ; peripheral s'annonce et accepte des connexions. Un nœud mesh doit être **les deux à la fois**. |
| **MTU** | Taille max d'un paquet applicatif BLE (~20 à 512 octets ; `powl` négocie `ATT_MTU = 517`, repli 23). |
| **ANNOUNCE** | Paquet de présence (`0x01`) : `peerID`, clés publiques, pseudo, `ledger_height`, `caps`. |
| **GOSSIP_FILTER / _PULL / _PUSH** | Paquets de réconciliation d'état : filtre compact (GCS) des `msgID` connus, puis demande / envoi des paquets manquants. |
| **GCS (Golomb-Coded Set)** | Filtre probabiliste compact (~20–30 % plus petit qu'un Bloom filter à même taux de faux-positifs). Paramètre `p = 1/64`. |
| **LOG_ATTEST** | Paquet (`0x0A`) diffusé périodiquement : `{ ledger_root, height, node_pub_sign }`. Capté par les relais → VPS. |
| **TOFU** | *Trust On First Use* : faire confiance à la première clé vue pour un contact, alerter si elle change ensuite. |
| **Code de vérification / safety number** | Chaîne de 60 chiffres identique des deux côtés, comparée hors bande, pour détecter un intercepteur au premier contact. |
| **Journal chaîné (hash-chain / ledger)** | Liste d'événements où chaque entrée contient le hash de la précédente : impossible d'en modifier une sans casser la chaîne. Local à chaque nœud. |
| **ledger_root** | `entry_hash` de la dernière entrée = résumé de tout l'historique d'un nœud. |
| **Attestation** | Diffusion par un nœud du résumé (racine) de son journal ; d'autres la rapportent au dashboard, rendant le mensonge détectable. |
| **Fork (de journal)** | Un nœud présente deux historiques différents pour la même hauteur → signe de triche. |
| **Statuts** | `en attente` (`QUEUED`) → `parti` (`IN_FLIGHT`) → `distribué` (`DELIVERED`) → `lu` (`READ`) (+ `échec/expiré` = `EXPIRED`, `annulé` = `CANCELLED`). |
| **ACK / read-receipt** | Accusés signés : « reçu par l'appareil » / « ouvert par l'utilisateur ». Voyagent dans la session Noise ou en enveloppe scellée. |
| **conv_seq** | Compteur par conversation (par expéditeur) → détection de trous, ordre causal. |
| **conv_hash** | `SHA-256(min(peerA,peerB) ‖ max(peerA,peerB))[0..8]` : groupe une conversation sans savoir qui elle implique. |
| **Padding** | Complétion PKCS#7 des paquets chiffrés vers `PAD_BUCKETS = [256, 512, 1024, 2048]`. |
| **Relais / `dengon-relay`** | Nœud fixe (ESP32) branché au secteur : densifie le mesh, cache, dépose des enveloppes, remonte les logs. Ne déchiffre rien. |
| **`dengon-core`** | Bibliothèque Rust contenant toute la logique (protocole, crypto, stockage, synchro, journal). Partagée par l'app, le nœud CLI et le firmware. |
| **`dengon-node`** | Nœud en ligne de commande sans UI : pour les tests et comme nœud fixe. |
| **`dengon-sim`** | Simulateur multi-nœuds (transport en mémoire, partitions, churn). |
| **`dengon-ffi`** | Bindings UniFFI (génère Kotlin + Swift). |
| **Transport (trait)** | Interface qui cache la radio : `dengon-core` envoie/reçoit des octets sans savoir si Android, un PC ou un ESP32 est derrière. |
| **UniFFI** | Outil qui génère automatiquement le « pont » pour appeler du Rust depuis Kotlin (ou Swift). |
| **Observabilité** | Capacité à comprendre ce que fait le système depuis l'extérieur, via les logs/traces. Ici : le dashboard. |
| **Dashboard / VPS** | Serveur qui **observe** le réseau (parcours des messages, santé). Ne transporte aucun message, ne voit aucun contenu. |
| **MQTT** | Protocole publish/subscribe léger, utilisé par les relais pour envoyer leurs logs au dashboard (`mqtts://<vps>:8883`, mTLS, QoS 1). |
| **TimescaleDB** | Extension PostgreSQL optimisée pour les données horodatées (le flux d'événements). Hypertable, rétention, agrégats continus. |
| **ESP-IDF / NimBLE** | ESP-IDF = SDK officiel ESP32. NimBLE = pile Bluetooth légère (host BLE) utilisée dans le firmware (~40 Ko de RAM de moins que Bluedroid). |
| **PSRAM** | RAM supplémentaire sur certains ESP32 (**WROVER**) ; nécessaire pour les buffers de `powl`. |
| **eFuse / Secure Boot / flash encryption** | Mécanismes de sécurisation matérielle de l'ESP32 (clé de relais en NVS chiffrée, firmware signé). |
| **E2EE** | Chiffrement de bout en bout (seuls l'expéditeur et le destinataire lisent le contenu). |
| **Merkle (arbre de)** | Résumé d'un lot de données par une racine ; preuve d'inclusion en `log₂(n)` hachages. Envisagé pour les accusés groupés / l'intégrité des fragments. |

---

## 18. Bibliographie complète

> Base : `oswin/06-sources-references.md` (consultée le 8 septembre 2026),
> complétée par `powl/01 §1.5` et `olivier/etude-stack.md` (sources). ⭐ = plus
> utile pour démarrer. Revérifier chaque lien à la rédaction finale et
> privilégier les sources primaires (RFC, spécifs officielles, articles
> académiques) pour les points sensibles.

### 18.1 Applications de messagerie mesh (études de cas)

- ⭐ **Bitchat** (la référence à imiter — BLE mesh + TTL + store-and-forward +
  crypto moderne) :
  - dev.to – Offline messaging reinvented with Bitchat — https://dev.to/grenishrai/offline-messaging-reinvented-with-bitchat-5011 (détails techniques, crypto)
  - TechTarget – What is Bitchat — https://www.techtarget.com/whatis/feature/What-is-Bitchat
  - TechRadar – how Bitchat works — https://www.techradar.com/phones/bitchat-is-a-new-private-bluetooth-messaging-app-that-doesnt-need-the-internet-heres-how-it-works
  - CNBC – Jack Dorsey launches a Bluetooth messaging rival — https://www.cnbc.com/2025/07/07/jack-dorsey-whatsapp-bluetooth.html
  - BeInCrypto – Bitchat expliqué — https://beincrypto.com/learn/bitchat-bluetooth-bitcoin-app/
  - `WHITEPAPER.md` — github.com/permissionlesstech/bitchat (cité par `powl/01`)
- ⭐ **Bridgefy** (le contre-exemple de sécurité à étudier) :
  - « Breaking Bridgefy » — version abrégée (PDF) — https://martinralbrecht.wordpress.com/wp-content/uploads/2020/08/bridgefy-abridged.pdf
  - Article complet — eprint IACR 2021/214 (PDF) — https://eprint.iacr.org/2021/214.pdf
  - Springer – Mesh Messaging in Large-Scale Protests: Breaking Bridgefy — https://link.springer.com/chapter/10.1007/978-3-030-75539-3_16
  - Royal Holloway – communiqué — https://www.royalholloway.ac.uk/research-and-education/subjects/information-security/news/using-messaging-service-bridgefy-could-have-dire-consequences-for-users-if-privacy-protection-issues-aren-t-fixed/
  - Bridgefy SDK Android — https://github.com/bridgefy/sdk-android
- **Briar** — code.briarproject.org, audit Cure53 ; BLE + Wi-Fi Direct + Tor —
  https://havenmessenger.com/blog/posts/mesh-networking-briar/
- *Survey of Mesh Networking Messengers* — TUM NET-2021-05-1 (cité par `powl/01`)

### 18.2 Bluetooth & Bluetooth Mesh

- ⭐ Bluetooth SIG – Mesh Networking Primer — https://www.bluetooth.com/bluetooth-mesh-networking-primer/
- ⭐ Novel Bits – Bluetooth Mesh: the ultimate guide — https://novelbits.io/bluetooth-mesh-networking-the-ultimate-guide/
- Bluetooth SIG – Directed Forwarding — https://www.bluetooth.com/mesh-directed-forwarding/
- Bluetooth SIG – Mesh FAQ — https://www.bluetooth.com/learn-about-bluetooth/topology-options/le-mesh/mesh-faq/
- MokoSmart – What is Bluetooth Mesh & how it works — https://www.mokosmart.com/what-is-bluetooth-mesh-how-it-works/
- MathWorks – Bluetooth Mesh Flooding in WSN — https://www.mathworks.com/help/bluetooth/ug/bluetooth-mesh-flooding-in-wireless-sensor-networks.html
- Google Patents – Managed flooding for Bluetooth mesh (US20200314735A1) — https://patents.google.com/patent/US20200314735A1/en
- Meshtastic – « managed flood routing » — https://meshtastic.org/blog/why-meshtastic-uses-managed-flood-routing/ et https://meshtastic.org/docs/overview/mesh-algo/
- Argenox – Android 5.0 BLE improvements — https://argenox.com/blog/android-5-0-lollipop-brings-ble-improvements
- Argenox – pourquoi le BLE Mesh peine à percer — https://argenox.com/blog/10-reasons-why-ble-mesh-has-struggled-to-gain-traction

### 18.3 Réseaux tolérants aux délais (DTN) & store-and-forward

- ⭐ RFC 9171 – Bundle Protocol Version 7 (texte intégral) — https://www.rfc-editor.org/rfc/rfc9171.html
- RFC 9171 – page d'information RFC Editor — https://www.rfc-editor.org/info/rfc9171/
- EmergentMind – Delay/Disruption Tolerant Network protocols — https://www.emergentmind.com/topics/delay-disruption-tolerant-network-dtn-protocols
- DTN7 – implémentation open source du Bundle Protocol — https://dtn7.github.io/
- Vahdat, Becker – *Epidemic Routing for Partially-Connected Ad Hoc Networks*, 2000 (cité par `powl/01`)
- *A Secure Epidemic Routing using Blockchain in Opportunistic IoT* — Springer, 2020 (cité par `powl/01`)
- Mots-clés pour approfondir le routage : « epidemic routing », « spray-and-wait », « PRoPHET DTN routing ».

### 18.4 Cryptographie & sécurité des messages

- ⭐ Signal – The Double Ratchet Algorithm (spécification) — https://signal.org/docs/specifications/doubleratchet/
- Signal Protocol – Wikipedia — https://en.wikipedia.org/wiki/Signal_Protocol
- Noise Protocol Framework — https://noiseprotocol.org/ (utilisé par Bitchat, WireGuard…)
- OMEMO – Wikipedia — https://en.wikipedia.org/wiki/OMEMO
- positive-intentions – Adapting the Signal Protocol for P2P — https://positive-intentions.com/blog/p2p-signal-protocol/
- Bibliothèque conseillée : **libsodium/NaCl** (X25519, Ed25519, XChaCha20-Poly1305) — C/Arduino, JavaScript, Python.
- Crates Rust `powl` : `ed25519-dalek` v2, `x25519-dalek`, `snow`, `chacha20poly1305`, `sha2`, `hmac`, `hkdf`, `getrandom`, `rusqlite` + `sqlcipher`.

### 18.5 Blockchain, hash chains & arbres de Merkle

- Medium – Blockchain, Hash & Merkle tree : immutability & integrity — https://medium.com/@zlhk100/blockchain-hash-and-merkle-tree-data-immutability-and-integrity-append-only-database-eff7b621b9c3
- GeeksforGeeks – Blockchain Merkle Trees — https://www.geeksforgeeks.org/blockchain-merkle-trees/
- HackerNoon – Merkle Trees & cryptographic accumulators — https://hackernoon.com/merkle-trees-and-cryptographic-accumulators-the-mathematical-backbone-of-blockchain-integrity
- DEV – Merkle tree root for data integrity — https://dev.to/bloxbytes/understanding-the-concept-of-merkle-tree-root-in-blockchain-for-data-integrity-2hp0

### 18.6 Arduino / ESP32 & dashboard IoT

- ⭐ Random Nerd Tutorials – ESP32 MQTT Publish/Subscribe (Arduino IDE) — https://randomnerdtutorials.com/esp32-mqtt-publish-subscribe-arduino-ide/
- ⭐ Hackster – Connect ESP32 to ThingsBoard over Wi-Fi — https://www.hackster.io/norvi/connect-esp32-to-thingsboard-over-wi-fi-visualize-iot-data-eae2ed
- ThingsBoard – client SDK Arduino/ESP32 — https://github.com/thingsboard/thingsboard-client-sdk
- Zbotic – ThingsBoard IoT platform avec ESP32 — https://zbotic.in/thingsboard-iot-platform-with-esp32-open-source-dashboard/
- GitHub – ESP32 ThingsBoard IoT Dashboard (exemple) — https://github.com/hubamatyas/ESP32-Thingsboard-IoT-Dashboard
- FlowFuse – Interacting with ESP32 using Node-RED and MQTT (2026) — https://flowfuse.com/blog/2024/11/esp32-with-node-red/
- oh2mp/esp32_ble2mqtt – passerelle BLE → MQTT — https://github.com/oh2mp/esp32_ble2mqtt
- Theengs OpenMQTTGateway — https://docs.openmqttgateway.com/
- Datasheet ESP32-WROOM-32E (Espressif) — https://www.espressif.com/sites/default/files/documentation/esp32-wroom-32e_esp32-wroom-32ue_datasheet_en.pdf
- Datasheet HTML — https://documentation.espressif.com/esp32-wroom-32e_esp32-wroom-32ue_datasheet_en.html
- Grid Connect – module 4MB Flash — https://www.gridconnect.com/products/esp32-wroom-32e-combo-wi-fi-bt-ble-module
- Espressif – *ESP-BLE-MESH Architecture* — https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/esp-ble-mesh/ble-mesh-index.html
- ESP-IDF – NimBLE vs Bluedroid (empreinte mémoire) — https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/ble/overview.html
- NimBLE-Arduino (empreinte réduite) — https://osrtos.com/projects/nimble-arduino/
- thingshost – ThingsBoard vs Grafana (2026) — https://thingshost.de/en/blog/thingsboard-vs-grafana
- wz-it – plateformes IoT open source comparées — https://wz-it.com/en/knowledge/iot/open-source-iot-platforms-compared/
- wz-it – Node-RED MQTT dashboard — https://wz-it.com/en/knowledge/iot/node-red-mqtt-dashboard/
- Steve's Internet Guide – IoT MQTT dashboards — http://www.steves-internet-guide.com/iot-mqtt-dashboards/
- rweather – AES accéléré ESP32 — https://rweather.github.io/arduinolibs/AESEsp32_8cpp_source.html
- Arduino Cryptography Library – Ed25519 (rweather) — https://rweather.github.io/arduinolibs/classEd25519.html
- libsodium sur ESP32 (forum) — https://esp32.com/viewtopic.php?t=9243

### 18.7 App mobile & bibliothèques BLE (`olivier/etude-stack`, `oswin/07 & 09`)

- flutter_blue_plus — https://pub.dev/packages/flutter_blue_plus et https://github.com/chipweinberger/flutter_blue_plus
- bluetooth_low_energy (Flutter, central + peripheral) — https://pub.dev/packages/bluetooth_low_energy
- ble_peripheral (Flutter) — https://pub.dev/packages/ble_peripheral et https://github.com/rohitsangwan01/ble_peripheral
- react-native-ble-plx (central only) — https://github.com/dotintent/react-native-ble-plx/blob/master/README.md et https://www.npmjs.com/package/react-native-ble-plx
- react-native-ble-plx – mode arrière-plan iOS — https://github.com/dotintent/react-native-ble-plx/wiki/Background-mode-(iOS)
- react-native-peripheral — https://github.com/petrbela/react-native-peripheral
- react-native-ble-advertiser (non maintenu) — https://www.npmjs.com/package/react-native-ble-advertiser
- iOS « overflow area » pour l'annonce en arrière-plan — https://github.com/davidgyoung/ios-overflow-area et https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
- Android 14 – types de services de fond obligatoires — https://developer.android.com/about/versions/14/changes/fgs-types-required et https://developer.android.com/develop/background-work/services/fgs/service-types
- Android 15 – restrictions scan BLE en arrière-plan — https://bleadvertiserapp.medium.com/android-15-broke-your-ble-app-new-permission-rules-3d8cb3c9ba86
- Android – communiquer en arrière-plan (doc officielle) — https://developer.android.com/develop/connectivity/bluetooth/ble/background
- Android Open Source Project – BLE advertising — https://source.android.com/docs/core/connect/bluetooth/ble_advertising
- liste de modèles sans mode périphérique (Corona-Warn-App) — https://github.com/corona-warn-app/cwa-app-android/issues/688
- flutter_sodium / libsodium pour Flutter — https://github.com/firstfloorsoftware/flutter_sodium
- pub.dev – sodium_libs — https://pub.dev/packages/sodium_libs ; pub.dev – sodium — https://pub.dev/packages/sodium
- Rust partagé mobile via UniFFI — https://github.com/ivnsch/rust_android_ios et https://appstimes.in/rust-in-mobile-development-2026-is-it-ready-to-replace-kotlin-and-swift-for-core-logic/
- Medium – BLE avec Flutter + Arduino — https://medium.com/@danielwolf.dev/get-started-with-bluetooth-low-energy-using-flutter-arduino-bdf5d790edc

### 18.8 Comment citer dans le rapport (exemples `oswin/06`)

- Norme mesh : *Bluetooth SIG, « Bluetooth Mesh Networking Primer », bluetooth.com.*
- DTN : *S. Burleigh et al., « RFC 9171 – Bundle Protocol Version 7 », IETF, 2022.*
- Sécurité (contre-exemple) : *M. Albrecht, J. Blasco, R. B. Jensen, L. Mareková, « Mesh Messaging in Large-Scale Protests: Breaking Bridgefy », CT-RSA, 2021.*
- E2EE : *M. Marlinspike, T. Perrin, « The Double Ratchet Algorithm », Signal, 2016.*

---

## 19. Annexe — correspondance section ↔ fichiers sources

| Section de ce document | `docs/powl/` | `docs/oswin/` | `docs/olivier/` |
| --- | --- | --- | --- |
| §1 Résumé exécutif | `README.md`, `01 §Récap` | `README.md`, `00` | — |
| §2 Origine des sources | `README.md` | `README.md` | `README.md`, `CONTEXT.md`, `mise-en-commun.md` |
| §3 Problème & besoin métier | `00-overview.md` | `00-vue-ensemble-architecture.md` | `CONTEXT.md`, `analyse-besoins.md` |
| §4 Concepts & état de l'art | `01-benchmarks.md §1` | `00`, `01-bluetooth-ble-mesh.md`, `02 §4`, `03-store-and-forward-dtn.md`, `04-blockchain-analyse-critique.md` | `analyse-besoins.md §6`, `etude-stack.md §2.3, §5` |
| §5 Architecture cible | `02-architecture.md` | `00 §3-4` | `architecture.md` |
| §6 Protocole réseau | `03-network-protocol.md` **(source de vérité)** | `01 §4`, `03 §4-5` | `protocole.md` v0.3 |
| §7 Format binaire de trame | `03 §3` | `02 §3` | `format-trame.md` v0.1 **(« fait foi » olivier)** |
| §8 Sécurité | `04-security.md` | `02-securite-messages.md` | `analyse-besoins.md §5`, `protocole.md §10` |
| §9 Cycle de vie & statuts | `05-message-lifecycle.md` | `03 §4` | `protocole.md §7` |
| §10 Relais ESP32 | `06-relay-esp32.md` | `05-arduino-passerelle-dashboard.md`, `07 §2` | `architecture.md §3` |
| §11 Dashboard | `07-dashboard.md`, `08-observability-events.md` **(source de vérité)** | `05 §3-5` | `dashboard.md` v0.2 |
| §12 Modèles de données | `09-data-model.md` **(source de vérité)** | — | — |
| §13 Benchmarks & choix techno | `01-benchmarks.md` | `09-benchmark-technos.md` | `etude-stack.md` v0.1 |
| §14 Périmètre MVP & feuille de route | `10-mvp-scope-roadmap.md` | `07 §7`, `08-feuille-de-route-plan-dev.md` | `decisions-v1.md`, `mise-en-commun.md §5` |
| §15 Tests & CI | `11-testing-strategy.md` | `08 §9` | — |
| §16 Analyse de besoins & cadrage | `00 §8` | — | `analyse-besoins.md`, `decisions-v1.md`, `mise-en-commun.md` |
| §17 Glossaire | termes disséminés dans `00`–`11` | `00 §6` | `protocole.md §2` |
| §18 Bibliographie | `01 §1.5` | `06-sources-references.md` | `etude-stack.md §Sources` |

**Vérification de complétude** : les 13 fichiers de `docs/powl/`, les 11 de
`docs/oswin/` et les 10 de `docs/olivier/` (README, CONTEXT, analyse-besoins,
decisions-v1, protocole, format-trame, architecture, dashboard, etude-stack,
mise-en-commun) sont tous référencés par au moins une section ci-dessus.

