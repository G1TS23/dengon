# 00 — Vue d'ensemble & architecture

## 1. Reformulation du besoin

Le projet **dengon** consiste à envoyer un message texte d'un utilisateur A vers un
utilisateur B **sans passer par Internet ni par un serveur central**, en s'appuyant sur le
**Bluetooth** des appareils qui se trouvent physiquement à portée. Les points marquants du
cahier des charges :

1. **Émettre** un message en Bluetooth.
2. **Sécuriser** le message (confidentialité + intégrité + authenticité).
3. **Recevoir** un message en Bluetooth.
4. **Stocker** le message sur les différents *points de transmission* tant que le
   destinataire ne l'a pas reçu.
5. **Faire circuler** le message de point de transmission en point de transmission
   (relais multi-sauts).
6. **Passer par une carte Arduino** équipée d'un module Bluetooth émetteur/récepteur, qui
   relaie l'information vers un **dashboard en ligne**.

> **Le nom académique de ce comportement.** Un message qui est *reçu, gardé, transporté
> puis retransmis quand un voisin apparaît* décrit un réseau **store-carry-forward**, cas
> particulier des **réseaux tolérants aux délais / aux ruptures (DTN — Delay/Disruption
> Tolerant Networking)**. C'est le vocabulaire à utiliser dans le rapport, et c'est ce qui
> permet de trouver la bonne littérature (voir fiche 03).

## 2. La référence à copier : Bitchat

En juillet 2025, Jack Dorsey a publié **Bitchat**, une messagerie qui fait *presque
exactement* ce que décrit ce projet. C'est la meilleure étude de cas disponible :

- réseau **mesh Bluetooth LE** où chaque téléphone est à la fois client et serveur ;
- **routage multi-sauts** avec un **TTL** (Time To Live) limité à **7 sauts** ;
- **mise en cache des messages** pour les pairs hors-ligne, avec **livraison automatique**
  dès qu'ils réapparaissent (= le « stockage aux points de transmission » du cahier des
  charges) ;
- **chiffrement de bout en bout** avec des primitives modernes (X25519, AES-256-GCM,
  Ed25519, cadre Noise) ;
- **identifiants éphémères** par session pour la vie privée.

*Sources : [TechTarget – What is Bitchat](https://www.techtarget.com/whatis/feature/What-is-Bitchat),
[dev.to – Offline messaging reinvented with Bitchat](https://dev.to/grenishrai/offline-messaging-reinvented-with-bitchat-5011),
[CNBC](https://www.cnbc.com/2025/07/07/jack-dorsey-whatsapp-bluetooth.html).*

**Conséquence pour dengon :** on ne réinvente rien. On s'inspire de l'architecture Bitchat
(open source) et on y ajoute la spécificité du projet : la **passerelle Arduino → dashboard**.

## 3. Architecture cible (vue logique)

```
   [A] téléphone/PC émetteur
     │  (1) chiffre + signe le message, lui donne un ID + un TTL
     ▼
   ┌───────────────────────────────────────────────┐
   │      RÉSEAU MESH BLUETOOTH (multi-sauts)        │
   │                                                 │
   │   [Relais 1] ──BLE──> [Relais 2] ──BLE──> ...   │   chaque relais :
   │      (stocke)          (stocke)                 │   - stocke le message
   │        │                  │                     │   - décrémente le TTL
   │        └───> [Arduino + module BT] <────────────┘   - le retransmet aux voisins
   │                    │                                 - ne peut PAS lire le contenu
   └────────────────────┼────────────────────────────────
                        │ (Wi-Fi / USB-série / GSM)
                        ▼
                 [Dashboard en ligne]      <- métadonnées uniquement :
                 (métriques, journal)         ID msg, sauts, horodatage, état
                        ▲
   [B] destinataire ────┘ (3) reçoit en BLE, déchiffre, vérifie la signature,
                             émet un ACCUSÉ DE RÉCEPTION qui repart dans le mesh
                             pour dire aux relais « vous pouvez oublier ce message »
```

### Rôles

| Élément | Rôle | Détails |
|---------|------|---------|
| **Émetteur (A)** | crée, chiffre, signe, publie | Fiches 02 (crypto) |
| **Points de transmission / relais** | stockent + retransmettent | Fiches 01 (mesh) et 03 (store-and-forward) |
| **Destinataire (B)** | déchiffre + accuse réception | L'ACK purge les copies en route |
| **Passerelle Arduino** | pont mesh ↔ Internet | Fiche 05 |
| **Dashboard** | supervision, démo, métriques | Fiche 05 |

## 4. Décomposition en briques (backlog technique)

1. **Format de message** (« paquet ») : en-tête clair (ID, TTL, expéditeur pseudonyme,
   destinataire, horodatage) + charge utile **chiffrée**. → fiches 02 et 03.
2. **Transport BLE** : découverte des voisins, envoi/réception. → fiche 01.
3. **Logique de relais** : anti-boucle (ne pas retransmettre 2× le même message),
   décrément du TTL, retransmission aux nouveaux voisins. → fiches 01 et 03.
4. **File de stockage** avec expiration (TTL temporel) et purge sur accusé de réception.
   → fiche 03.
5. **Couche cryptographique** : échange de clés, chiffrement authentifié, signature.
   → fiche 02.
6. **Accusé de réception (ACK)** qui remonte le mesh pour libérer le stockage. → fiche 03.
7. **Passerelle Arduino** + **dashboard**. → fiche 05.
8. **(Optionnel) chaînage par hash / Merkle** pour l'intégrité et les preuves. → fiche 04.

## 5. Portée réaliste (attentes vs réalité)

- Le BLE, c'est **~10 à 30 m** en pratique par saut (voir fiche 01). Le mesh sert
  justement à dépasser cette limite en enchaînant les sauts.
- Un message peut **ne jamais arriver** si le chemin physique n'existe pas : un réseau DTN
  garantit un effort « au mieux » (*best effort*), pas une livraison certaine. Le dashboard
  et les ACK servent à *observer* ce qui se passe réellement.
- Pour une **démo d'école**, 3–5 nœuds (2 téléphones/PC + l'Arduino + 1–2 relais) suffisent
  amplement à montrer les 6 points du cahier des charges.

## 6. Vocabulaire (glossaire minimal)

- **Mesh** : réseau maillé où chaque nœud peut relayer pour les autres.
- **Saut (hop)** : un relais d'un nœud vers un autre.
- **TTL (Time To Live)** : nombre max de sauts (ou durée) avant qu'un message soit abandonné.
- **Store-carry-forward** : stocker, transporter, retransmettre.
- **DTN** : réseau tolérant aux délais/ruptures de connexion.
- **E2EE** : chiffrement de bout en bout (seuls A et B lisent le contenu).
- **ACK** : accusé de réception.
- **GATT / GAP** : couches applicatives/découverte du Bluetooth LE (fiche 01).

---

### À retenir
dengon = **Bitchat + une passerelle Arduino vers un dashboard**. Le cœur technique n'est
pas la « blockchain » mais le trio **mesh BLE + store-carry-forward + chiffrement de bout
en bout**. Les fiches suivantes détaillent chaque brique.
