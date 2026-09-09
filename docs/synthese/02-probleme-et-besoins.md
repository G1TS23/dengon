# dengon — Problème & analyse de besoins

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
> Ce document décrit la **conception retenue** (décisions de cadrage prises par
> Paul le 2026-09-08, ratification d'équipe à venir).

---

## 1. Le concept

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
- Passer par des téléphones mobiles et par des cartes Arduino avec Bluetooth et
  Wi-Fi (**FREENOVE ESP32-WROOM**).

**Décisions de périmètre** (voir [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md)) :

- **Android + relais ESP32** dans le MVP ; **iOS reporté en v2** (A-11) —
  l'architecture (cœur Rust + `trait Transport` + bindings Swift UniFFI) garde
  la porte ouverte.
- **Carte ESP32-WROOM-32E** (Freenove, déjà en possession de l'équipe), pas de
  WROVER (A-4).
- **Statuts v1** : *En attente → Parti → Distribué* (+ *Échec / Expiré*). Le
  statut **« Lu » est reporté en v2** (A-10) ; `statut = 4 (LU)` reste réservé
  dans le format de trame.

## 2. Le problème

Échanger des messages texte **sans infrastructure réseau** : réseau cellulaire
saturé, zone blanche, manifestation / catastrophe / festival, ou par principe
(messagerie sans opérateur). Seul média entre appareils proches : le **BLE**,
portée ~10–30 m → il faut un **maillage**. Les appareils bougent, s'éteignent,
sortent de portée → réseau jamais entièrement connecté → **DTN** (réseau
tolérant aux délais, voir [`03-etat-de-lart.md`](03-etat-de-lart.md)).

## 3. Les 6 exigences du cahier des charges

1. **Émettre** un message en Bluetooth.
2. **Sécuriser** le message (confidentialité + intégrité + authenticité).
3. **Recevoir** un message en Bluetooth.
4. **Stocker** le message sur les points de transmission tant que le
   destinataire ne l'a pas reçu.
5. **Faire circuler** le message de point en point (relais multi-sauts).
6. **Passer par une carte Arduino** équipée d'un module Bluetooth, qui relaie
   l'information vers un **dashboard en ligne**.

## 4. Acteurs

| Acteur | Rôle | Matériel |
| --- | --- | --- |
| **Utilisateur** | écrit, envoie, lit | smartphone (Android en MVP) |
| **App cliente** `dengon-app` | UI + nœud du réseau | Android + cœur Rust partagé |
| **Nœud CLI** `dengon-node` | nœud sans UI : tests multi-nœuds, bootstrap, PC fixe | PC/Linux (Rust) |
| **Relais** `dengon-relay` | infra fixe : relaie, met en cache, dépose, remonte les logs | ESP32-WROOM-32E + Wi-Fi |
| **Dashboard** `dengon-dashboard` | observe le réseau, trace les messages, surveille la flotte | VPS (déjà possédé) |
| **Opérateur** | exploite le dashboard | navigateur |

## 5. Cas d'usage MVP

1. **Envoi direct** : A et B à portée BLE → livré en 1 saut.
2. **Envoi multi-saut** : A → B hors de portée, un relais ESP32 (ou téléphone
   tiers) entre les deux → relayé.
3. **Destinataire absent** : Charlie éteint → message déposé en **enveloppe
   scellée** sur des relais, récupéré à son retour.
4. **Expéditeur déconnecté** : A envoie puis coupe le BLE ; à la reconnexion, les
   **accusés** distribué la rattrapent.
5. **Suivi** : l'opérateur cherche un `msgID`, voit le chemin et l'état courant.
6. **Vérification de contact** : scan mutuel du **QR code** + comparaison d'un
   **code de vérification à 60 chiffres** → « vérifiés ».

## 6. Périmètre MVP

**Dans le MVP :** texte court (< 1 Ko utile), 1-à-1 ; chiffrement E2E (Noise) +
signatures (Ed25519) ; maillage BLE (émission, réception, relais multi-saut,
flood contrôlé + TTL) ; store-and-forward (outbox rejoué, enveloppes scellées,
réconciliation d'inventaire) ; statuts *En attente → Parti → Distribué* +
*Échec/Expiré* avec accusés signés ; **relais ESP32 fonctionnel** (livrable
complet, pas une simple preuve de concept — voir A-9/C-9) ; dashboard (parcours
d'un message, carte réseau, flotte de relais, recherche de logs, vérification
d'intégrité des journaux) ; app Android.

**Hors MVP (prévu par l'architecture, pas livré) :** app iOS (A-11) ; statut
**« Lu »** (A-10) ; réconciliation **gossip GCS** et **budget de copies
Spray-and-Wait** (cible v2, l'échange d'inventaire brut suffit au MVP — A-13) ;
groupes / canaux publics ; pièces jointes ; ratchet façon Signal ; firmware
relais en Rust pur (`esp-rs`) ; **backhaul** — le VPS ne relaie **pas** de
messages (choix explicite « observabilité seule »).

## 7. Portée réaliste

- BLE ~**10 à 30 m** par saut en pratique ; le mesh cumule les portées (3 sauts
  ≈ ~100 m de bout en bout).
- Un message peut **ne jamais arriver** si le chemin physique n'existe pas : un
  DTN garantit du *best-effort*, pas une livraison certaine. Le dashboard et les
  ACK servent à **observer** la réalité.
- Pour une **démo d'école**, 3–5 nœuds (2 téléphones/PC + Arduino + 1–2 relais)
  suffisent à montrer les 6 points du cahier des charges ; la démo finale visée
  est de **5-8 appareils**.

---

## 8. Analyse de besoins — les 10 axes de questionnement

> Source : `olivier/analyse-besoins.md`. Ces axes ont servi à cadrer le projet ;
> entre parenthèses, la **réponse retenue** (détails dans les autres fichiers de
> ce dossier et dans [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md)).

1. **Identité et adressage** : comment désigne-t-on un destinataire ? un ou
   plusieurs destinataires (groupe) ? comment ajoute-t-on un contact ? comment
   empêche-t-on l'usurpation d'un expéditeur ? multi-appareil ?
   *(→ `peerID` 8 o (A-8), 1-à-1, QR + code 60 chiffres + TOFU (C-12), signature
   Ed25519, multi-appareil hors v2.)*
2. **Format du message et du protocole** : le livrable central = la spéc du
   protocole (format binaire) commun aux 3 familles d'appareils. BLE MTU faible
   → fragmentation/réassemblage. Entêtes nécessaires : ID unique, expéditeur,
   destinataire, TTL/sauts, horodatage, numéro de séquence / nonce.
   *(→ format `powl/03` unique (A-12), voir [`05-protocole-et-trame.md`](05-protocole-et-trame.md).)*
3. **Propagation (routage mesh)** : flooding vs routage → **flooding contrôlé**
   en v1 ; TTL / limite de sauts ; déduplication (table des ID vus) ; cassure de
   boucles ; l'accusé de livraison doit **remonter** dans le mesh ; durée de vie
   / expiration ; priorité entre messages ?
   *(→ socle flood + TTL 7 + seen-set + jitter, clamp de densité, quotas,
   anti-inondation 20/min/voisin, échange d'inventaire (A-13). Pas de priorité
   entre messages en v1.)*
4. **Store-and-forward** : chaque nœud stocke les messages non délivrés et les
   rejoue ; combien de temps ? quelle taille de file ? **contrainte forte
   ESP32** : RAM ~520 Ko → tampon réduit, politique d'éviction ; persistance
   locale sur mobile (base chiffrée).
   *(→ expiration ~24 h (`MSG_TTL_S`), file ~50 (ESP32) / ~300 (téléphone),
   éviction du plus ancien, base locale chiffrée XChaCha20 champ par champ (B-3).)*
5. **Sécurité** : E2EE du contenu ; intégrité / authenticité ; anti-rejeu ;
   métadonnées (jusqu'où masquer expéditeur / destinataire ?) ; provisioning des
   clés ; stockage des clés ; révocation ; anti-spam / anti-DoS ; forward
   secrecy.
   *(→ Noise `XX`/`X` + Ed25519 (A-3), `conv_seq` anti-rejeu (C-7),
   `recipient_tag` tournant + padding, QR présentiel + TOFU (C-12), Keystore /
   Keychain / NVS chiffrée, quotas + anti-inondation. Doc de référence :
   `powl/04-security.md` (C-11), voir [`06-securite.md`](06-securite.md).)*
6. **Contraintes des plateformes** : **iOS** — BLE arrière-plan bridé par
   CoreBluetooth (*overflow area*) → **reporté v2** (A-11) ; **Android** —
   permissions BLE + localisation, service *foreground* avec notification
   obligatoire, Doze ; **ESP32-WROOM** — BLE + Wi-Fi, ressources limitées ;
   **BLE = dénominateur commun** ; chaque téléphone **central et périphérique en
   même temps** ; portée intérieure ~10–30 m.
7. **Dashboard / observabilité** : qui pousse les données (les nœuds avec du
   Wi-Fi) ? où est hébergé le backend (VPS, bonus non bloquant) ? quelles
   données (**ID de message, statut, horodatage, sauts, nœuds traversés — jamais
   le contenu**) ? reconstituer le trajet à partir de rapports **partiels** ;
   **horloge** désynchronisée (horodatages best-effort assumés) ; temps réel vs
   lots.
   *(→ FastAPI + SSE + SQLite (A-5/C-8), ingestion HTTPS POST par batch (A-6),
   voir [`09-dashboard-et-donnees.md`](09-dashboard-et-donnees.md).)*
8. **Expérience utilisateur** : notification à la réception ; visibilité d'un
   message « En attente » ancien ; retour visuel sur l'état du réseau
   (« Connecté » / « Isolé ») ; historique local ; onboarding = échange de clés
   en présentiel ; mode dégradé quand l'appareil est seul.
9. **Cadrage projet / non-fonctionnel** : 2–3 scénarios d'usage concrets
   (scénario retenu = **campus / bâtiment**) ; MVP resserré ; métriques de
   succès (taux de livraison, latence, sauts, impact batterie) ; **batterie**
   (scan/advertising BLE continu = critère de viabilité → mode éco) ; stratégie
   de test (voir [`10-benchmarks-mvp-tests.md`](10-benchmarks-mvp-tests.md)) ;
   aspects légaux (radio, chiffrement, RGPD sur les métadonnées du dashboard).
10. **Décisions à trancher en premier** : (1) protocole maison ✅ (inspiré
    Meshtastic/Briar/Bitchat, code propre en Rust — C-10) ; (2) **BLE seul** ;
    (3) **1-à-1 uniquement** en v1 ; (4) modèle de confiance = **TOFU en
    présentiel** (C-12) ; (5) relais iOS = **évolution v2** (A-11) ; (6)
    périmètre MVP et répartition des tâches — voir
    [`10-benchmarks-mvp-tests.md`](10-benchmarks-mvp-tests.md).

## 9. Points produit / UX

Décisions produit retenues (issues de l'analyse `olivier` et validées comme
cadre de travail) :

| Sujet | Décision v1 |
| --- | --- |
| Scénario d'usage prioritaire | **campus / bâtiment** (zone moyenne, téléphones + quelques relais fixes) |
| Type de contenu | **texte court ~140–500 caractères** (≤ ~1 Ko utile) |
| Identité utilisateur | **pseudo libre, sans vérification** (l'identité réelle = la clé échangée en personne) |
| Fonctionnement en arrière-plan (Android) | **service de fond + notification permanente** (incontournable Android 14/15) |
| Conservation des messages sur le téléphone | **jusqu'à suppression manuelle** |
| Info réseau montrée à l'utilisateur | **indicateur simple** « Connecté au réseau » / « Isolé » |
| Langue de l'interface (v1) | **français uniquement** |
| Relais par un téléphone sans contact | **oui, toujours** — participe dès l'installation |
| Mode « économie » batterie | **activable dès la v1** (couche Application) |
| Vérification de clé | **code 60 chiffres à comparer de visu** après le scan QR (C-12) |
| Message expiré (~24 h sans livraison) | statut **« Échec »** dans la conversation, sans notification |
| Relance d'un message non parti | **bouton « Renvoyer »** déclenché par l'utilisateur |
| Blocage d'un contact | **reporté après la v1** (colonne `contacts.blocked` déjà prévue au schéma) |
| Hébergement du dashboard | **VPS Debian de l'équipe** |
| Rafraîchissement du dashboard | **automatique** (SSE) |

**Encore à valider en équipe** (section E de
[`01-sujets-a-trancher.md`](01-sujets-a-trancher.md#e-avis-dolivier-à-valider-en-équipe)) :
le **nom du projet** (« dengon » = nom de travail), et la validation formelle à
trois de l'ensemble de ces points produit.

> La structure de dépôt proposée par `olivier/mise-en-commun.md` (`app/`,
> `firmware/`, `dashboard/serveur/`, `outils/`, `docs/protocole/…`) **n'a pas
> été retenue** : le dépôt suit le découpage `crates/` de `powl/02` (voir A-14
> et [`05-protocole-et-trame.md`](05-protocole-et-trame.md) pour l'architecture).
