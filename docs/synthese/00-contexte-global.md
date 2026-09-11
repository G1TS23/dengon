# dengon — Contexte global (point d'entrée)

> **伝言 (*dengon*)** : « message que l'on confie à quelqu'un pour qu'il le
> transmette ». Un message est confié au réseau, qui le porte de proche en
> proche jusqu'à son destinataire, même si personne n'a Internet.

Ce dossier **regroupe l'intégralité** de la recherche et de la conception
produites dans les trois dossiers de travail (`docs/powl/`, `docs/oswin/`,
`docs/olivier/`). Objectif : permettre à une nouvelle conversation (ou à un
nouvel arrivant) d'avoir **tout le contexte** sans relire 34 fichiers.

- Les **divergences entre les 3 dossiers ont été tranchées** (par Paul, le
  2026-09-08 — ratification d'équipe à venir). Ce dossier décrit la
  **conception retenue** comme un fait. Le détail du « pourquoi » (options,
  arguments, alternatives écartées) reste dans
  [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
- Le contenu propre à `oswin` (état de l'art, benchmarks, bibliographie) et à
  `olivier` (analyse de besoins, format de trame v0.1, cadrage) est **intégré**
  — rien n'est jeté. Ce qui a été remplacé par une décision est conservé soit
  dans `01-sujets-a-trancher.md`, soit en annexe « pour mémoire » du fichier
  [`11-glossaire-biblio-annexes.md`](11-glossaire-biblio-annexes.md), soit dans
  les dossiers sources `docs/{powl,oswin,olivier}/` (intacts).

Ce dossier couvre **recherche & conception uniquement**. L'état réel du code et
le méta-projet (échéance, équipe, conventions) restent dans
[`../suivi/`](../suivi/) et `CLAUDE.md`.

---

## 1. Résumé exécutif

**dengon** est une messagerie **texte chiffrée de bout en bout** qui circule en
**gossip** sur un **maillage Bluetooth Low Energy (BLE)** de téléphones et de
**relais ESP32 fixes**, doublée d'un **tableau de bord d'observabilité** sur VPS
qui trace le parcours et l'état de chaque message **sans jamais voir son
contenu**. Le réseau n'est jamais entièrement connecté : c'est un **DTN**
(*Delay-Tolerant Network*) *store-carry-forward* — on stocke, on transporte, on
retransmet à la prochaine occasion.

### Décisions de conception (équipe, 2026-09)

> Arguments détaillés et options écartées : [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
> Statut : `tranché` = décidé par Paul (à ratifier en réunion) ; `spike` =
> conditionné au résultat d'un spike du Lot 0.

| Réf | Décision | Statut |
| --- | --- | --- |
| A-1 | App **Android : Kotlin natif + Jetpack Compose** ; iOS (Swift/SwiftUI) en v2, même cœur. Flutter / React Native écartés. | tranché |
| A-2 | **Un seul cœur Rust `dengon-core`** (`no_std + alloc`), partagé par l'app (UniFFI), le nœud CLI et le firmware (lib C). | tranché |
| A-3 / B-1 | Crypto : **Noise `XX`** (session) + **Noise `X`** (enveloppes) + **Ed25519** (paquet), en Rust. Repli si le Spike A échoue : `trait Crypto` + mbedTLS **limité au handshake de lien BLE ESP32** ; `sha2` + `ed25519` restent en Rust partout. | tranché / spike |
| A-4 | Relais = **ESP32-WROOM-32E** (Freenove, déjà en possession) ; caches ~8× plus petits que WROVER. WROVER seulement si saturation constatée. | tranché |
| A-5 / B-5 / C-8 | Dashboard = **FastAPI (Python) + SSE + SQLite** + page web légère ; vérif de journal chaîné par le binaire Rust **`dengon-verify`**. Stack Axum/Timescale/MQTT écartée. | tranché |
| A-6 | Transport nœud → dashboard = **HTTPS POST par batch** (signé). Pas de broker MQTT. | tranché |
| A-7 / A-15 | Intégrité du suivi = **journal chaîné signé par appareil**, agrégé et audité par le dashboard, **sans consensus**. C'est l'interprétation retenue de « sécurité type blockchain ». | tranché |
| A-8 | Identifiant de nœud : `peerID` = `SHA-256(pub_static)[0..8]`, **8 octets**. | tranché |
| A-9 | Identifiant de message : `msgID` 32 o (hash de contenu, dédup) + `msg_uuid` 16 o (UUIDv4, stable de bout en bout, suivi des statuts). | tranché |
| A-10 | Statut **« Lu » reporté en v2**. MVP : *En attente → Parti → Distribué* (+ *Échec/Expiré*). | tranché |
| A-11 | **iOS reporté en v2**, sans preuve de faisabilité anticipée. L'archi (cœur Rust + `trait Transport` + UniFFI/Swift) garde la porte ouverte. | tranché |
| A-12 | Format de trame = **`powl/03`** (12 types de paquets + `INVENTORY` au MVP). Le format `olivier/format-trame` v0.1 est remplacé (conservé comme vecteurs de test). | tranché |
| A-13 | Routage : socle flood + TTL 7 + seen-set + jitter + clamp densité + quotas + **anti-inondation 20/min/voisin** + **échange d'inventaire brut** au MVP. **GCS gossip** et **budget de copies Spray-and-Wait** = cible v2. | tranché |
| A-14 | Dépôt = workspace `crates/` de `powl/02` + `crates/dengon-verify` ; `dashboard/` en Python. | tranché |
| B-2 / C-5 | Auth des nœuds auprès du dashboard = **jeton/JWT court** (liste blanche) + **signature Ed25519 des batchs**. mTLS abandonné. | tranché |
| B-3 | Base locale chiffrée par **XChaCha20-Poly1305 champ par champ** (colonnes sensibles) ; clés privées dans Keystore/Keychain. Pas de SQLCipher. | tranché |
| B-4 | Base du dashboard **effacée après chaque session de démo** (pas de rétention automatique). | tranché |
| C-1 | Valeurs numériques = constantes de `powl/03 §2` + caps réduits WROOM. Résiduel non bloquant : MTU réel (Spike C). | tranché |
| C-2 | UUID BLE = ceux de `powl/03 §2` (`SERVICE_UUID`, `CHAR_RX`/`CHAR_TX`). | tranché |
| C-6 | Pas de checksum par fragment (CRC BLE + signature du paquet reconstruit). | tranché |
| C-7 | Anti-rejeu = `conv_seq` + `msgID` + seen-set + fenêtre timestamp ±2 h. | tranché |
| C-9 | Le **relais ESP32 fait partie du MVP** (livrable complet, pas une PoC). | tranché |
| C-10 | On **s'inspire des idées** de Meshtastic / Bitchat, on ne reprend pas de code (langages différents). | tranché |
| C-11 | Doc sécurité de référence = **`powl/04-security.md`** ; delta rédigé dans [`06-securite.md`](06-securite.md) (US-112). Résultat du Spike A (US-101) disponible dans PR #64, en revue. | tranché |
| C-12 | Échange de clés = **QR scanné en présentiel + code de vérification 60 chiffres + TOFU**. | tranché |

**Encore ouvert** : D-1/D-2/D-4/D-5 (réconciliations de rédaction internes à
`powl`, appliquées dans ce dossier, à répercuter dans `docs/powl/`) ; section E
(« avis d'Olivier » produit/UX à valider formellement à trois) ; formulation
« blockchain » dans le rapport/la soutenance ; framework exact de la page web du
dashboard.

### Principes directeurs

1. **Offline-first** : toute fonction marche sans Internet ; la connectivité est
   une opportunité, jamais un prérequis.
2. **Zéro confiance dans le transport** : relais, VPS et réseau supposés hostiles
   → E2E systématique, métadonnées minimisées.
3. **Une seule implémentation du protocole** : `dengon-core` en Rust, partagé par
   l'app, le nœud CLI et le firmware. Auditer une fois.
4. **Le dashboard observe, il n'agit pas** : ni router, ni injecter, ni
   déchiffrer. S'il disparaît, le réseau fonctionne à l'identique.
5. **Traçabilité infalsifiable** : chaque nœud tient un journal chaîné signé ; le
   dashboard le vérifie mais ne peut pas le forger.

### Statut

**Conception — pas encore de code.** Décisions de cadrage prises par Paul le
2026-09-08 ; ratification d'équipe à venir. Prochaine étape : **Lot 0** (spikes
de dérisquage A/B/C), cf.
[`10-benchmarks-mvp-tests.md`](10-benchmarks-mvp-tests.md) §3.2. Le suivi de ce
qui est réellement codé est dans [`../suivi/`](../suivi/).

---

## 2. Origine des sources

Trois efforts de recherche/conception **parallèles et indépendants**, un par
contributeur, menés avant mise en commun.

| Dossier | Auteur | Nature | Stack proposée à l'origine |
| --- | --- | --- | --- |
| [`docs/powl/`](../powl/) | powl | **Conception cible complète** (13 fichiers). Sources de vérité internes : `03` (protocole/trame), `08` (événements), `09` (schémas), `04` (sécurité). | Cœur Rust partagé, Android/Kotlin + UniFFI, Noise XX/X + Ed25519, ESP32-WROVER + ESP-IDF/NimBLE, dashboard MQTT→Axum→PostgreSQL/Timescale→React. |
| [`docs/oswin/`](../oswin/) | oswin | **Recherche documentaire sourcée** (11 fichiers), chaque fiche avec un encadré « À retenir ». Bibliographie ~60 références. | Flutter + `flutter_blue_plus`/`bluetooth_low_energy`, Arduino/PlatformIO, libsodium, Mosquitto/HiveMQ, Node-RED/ThingsBoard. ESP32-WROOM-32E. |
| [`docs/olivier/`](../olivier/) | olivier | **Brouillons v0.1–v0.3**, zone « protocole + dashboard ». Spéc comportementale (`protocole.md` v0.3), format binaire octet par octet (`format-trame.md` v0.1). | Flutter ou Kotlin (décision après proto), **deux implémentations** du moteur, libsodium `crypto_box` / mbedTLS, FastAPI/Express + SSE + SQLite. ESP32-WROOM. |

**Les divergences ont été tranchées** (table §1 ci-dessus, arguments dans
[`01-sujets-a-trancher.md`](01-sujets-a-trancher.md)). Les dossiers `powl`,
`oswin` et `olivier` **restent la matière première** : ils ne sont pas modifiés,
et le rapport / la soutenance peuvent s'y appuyer pour justifier les choix.

---

## 3. Guide de lecture

Le contexte détaillé est découpé en fichiers thématiques :

| Fichier | Contenu |
| --- | --- |
| [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md) | **Transversal.** Toutes les décisions avec leurs options, arguments et statut ; les points encore ouverts. À lire en parallèle du reste. |
| [`02-probleme-et-besoins.md`](02-probleme-et-besoins.md) | Concept, besoins métier, cas d'usage, périmètre MVP, portée réaliste ; analyse de besoins (10 axes) ; points produit / UX. |
| [`03-etat-de-lart.md`](03-etat-de-lart.md) | Recherche : DTN / RFC 9171, algorithmes de routage, Bitchat (référence), Bridgefy (contre-exemple), Bluetooth Mesh, analyse critique « blockchain ». |
| [`04-architecture.md`](04-architecture.md) | Modules de `dengon-core`, trait `Transport`, découpage en couches, découpage du dépôt, déploiement, choix transverses. |
| [`05-protocole-et-trame.md`](05-protocole-et-trame.md) | Couches L1–L4, constantes, format de paquet L3 octet par octet, 12 types de paquets, fragmentation, routage, comportement détaillé au MVP. |
| [`06-securite.md`](06-securite.md) | Modèle de menace, identité & clés, code de vérification, Noise `XX`/`X`, `recipient_tag`, journal chaîné signé, chiffrement de la base locale, sécurité relais & dashboard, primitives crypto, « qui voit quoi ». |
| [`07-cycle-de-vie-et-statuts.md`](07-cycle-de-vie-et-statuts.md) | Les statuts, machine à états (MVP + cible v2), émission, réception, cas multi-saut, reconnexion, expiration, table événement ↔ paquet ↔ log. |
| [`08-relais-esp32.md`](08-relais-esp32.md) | Rôle, matériel WROOM, stack firmware, architecture interne, budgets mémoire, connexion au VPS (HTTPS), pannes, ce que le relais journalise. |
| [`09-dashboard-et-donnees.md`](09-dashboard-et-donnees.md) | Architecture FastAPI + SSE + SQLite, ingestion HTTPS, écrans, API, catalogue normalisé des événements (`powl/08`), reconstruction du statut ; schémas de données (base locale + dashboard), formats de fils. |
| [`10-benchmarks-mvp-tests.md`](10-benchmarks-mvp-tests.md) | Décisions techniques, comparatifs (recherche), Definition of Done, 8 lots de livraison, hors MVP, risques, feuilles de route ; stratégie de test & CI. |
| [`11-glossaire-biblio-annexes.md`](11-glossaire-biblio-annexes.md) | Glossaire (~70 termes), bibliographie complète (~90 liens), correspondance fichier ↔ sources ; **Annexe A** (format `olivier/format-trame` v0.1, pour mémoire), **Annexe B** (dashboard `olivier` v0.2, pour mémoire). |

**Parcours conseillé** pour un nouvel arrivant : ce fichier → `02` → `03` → `04`
→ `05` → `06` → `07` → `08` → `09` → `10`, en gardant `01` ouvert à côté pour le
« pourquoi » de chaque choix.
