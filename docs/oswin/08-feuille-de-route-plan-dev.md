# 08 — Feuille de route & plan de développement

> Cette fiche transforme la recherche des fiches 00–07 en un **plan d'action** : quoi
> construire, dans quel ordre, avec quels jalons, comment se répartir le travail et comment
> savoir qu'on a « fini ». Elle est pensée pour une **équipe débutante** et privilégie une
> **livraison par paliers** (toujours avoir une démo qui marche).

> **À personnaliser.** Ce plan raisonne en **« semaines relatives »** et suppose une équipe
> de **2 à 4 personnes**. Remplacez les durées par vos vraies dates (deadline du projet) et
> ajustez la répartition à votre effectif réel. Une case *« vos dates »* est prévue dans le
> tableau de planning (§5).

---

## 1. Objectif & définition du MVP

**Objectif du projet :** une application Android (`.apk`) permettant d'échanger des messages
**chiffrés** via **Bluetooth**, qui **transitent et sont stockés** de nœud en nœud (mesh
téléphone-à-téléphone + relais ESP32) jusqu'à atteindre le destinataire, avec une
**passerelle ESP32 → dashboard** en ligne pour la supervision.

**MVP (Minimum Viable Product) = ce qu'on garantit de livrer :**

> Un message chiffré part du **téléphone A**, est **reçu et stocké par un ESP32** alors que
> le destinataire est absent, puis **retransmis** (via un 2ᵉ ESP32) au **téléphone B** qui
> le **déchiffre**, avec **remontée des métadonnées sur le dashboard** et **accusé de
> réception** qui purge le stockage.

Ce MVP correspond aux **paliers 1 + 2** de la fiche 07 et couvre **déjà les 6 points du
cahier des charges**. Le mesh **téléphone-à-téléphone** (palier 3) est l'**objectif étendu**,
tenté seulement une fois le MVP solide.

## 2. Principe directeur : livrer par paliers

| Palier | Contenu | Statut visé |
|---|---|---|
| **P1** | Téléphone ↔ 1 ESP32 : envoi chiffré + affichage + dashboard | **socle garanti** |
| **P2** | 2 ESP32 qui **stockent et relaient** (multi-sauts + ACK + purge) | **MVP = note visée** |
| **P3** | Mesh **téléphone-à-téléphone** (rôle périphérique BLE sur Android) | **objectif étendu / bonus** |

Règle d'or : **on ne commence un palier que lorsque le précédent est démontrable.** Ainsi, à
tout moment, il existe une version présentable.

### Correspondance avec le cahier des charges

| Point du cahier des charges | Couvert par |
|---|---|
| Envoyer un message via BT | P1 |
| Sécuriser le message | P1 (chiffrement) transversal |
| Recevoir un message en BT | P1 / P2 |
| Stockage sur les points de transmission | **P2** |
| Transmission entre points (multi-sauts) | **P2** (ESP32↔ESP32), **P3** (tél.↔tél.) |
| Passage par Arduino → dashboard | P1 (passerelle) + P2 (métriques) |

## 3. Les chantiers (workstreams)

Le projet se découpe en **6 chantiers** menés en parallèle. Chacun peut être « porté » par
une personne ou un binôme (voir §6).

1. **Protocole & format de message** — le « contrat » commun app ↔ ESP32 : structure du
   paquet (fiche 02 §3), UUID du service GATT et caractéristiques (fiche 07 §3), règles de
   TTL / ID / ACK (fiches 01 & 03). *C'est le chantier fondateur : à figer tôt, car tout le
   monde en dépend.*
2. **Firmware ESP32** — relais BLE (store-carry-forward) + passerelle Wi-Fi/MQTT. Fiches 05
   & 07.
3. **Application Android** — UI, transport BLE, file de stockage locale, intégration crypto.
   Fiche 07.
4. **Sécurité / cryptographie** — génération & échange de clés, chiffrement AEAD, signatures,
   ACK signés. Fiche 02. *Transversal : concerne l'app et l'ESP32.*
5. **Dashboard & supervision** — broker MQTT + tableau de bord (ThingsBoard ou Node-RED),
   widgets de métriques. Fiche 05.
6. **Intégration, tests & documentation** — assembler les briques, tester bout-en-bout,
   préparer la démo, tenir le dépôt et les fiches à jour.

## 4. Découpage en phases & jalons

Chaque jalon a une **« definition of done » (DoD)** : un critère observable qui prouve qu'il
est atteint.

### Phase 0 — Cadrage & mise en route
- Installer les environnements (§10), cloner le dépôt, activer les hooks git.
- **Figer le format de message v1** (chantier 1) et l'écrire dans une fiche/README technique.
- Répartir les rôles (§6), créer le tableau de suivi (kanban).
- **DoD :** chaque membre a son environnement qui compile un « hello world » (LED ESP32 +
  app Flutter vide qui s'installe sur un téléphone) ; le format de message v1 est écrit et
  validé par l'équipe.

### Phase 1 — Palier 1 : le lien de base
- ESP32 expose le service GATT « dengon » ; l'app (rôle central) s'y connecte.
- L'app **envoie un message chiffré** ; l'ESP32 le reçoit et l'affiche (log série).
- L'ESP32 se connecte au Wi-Fi et **publie une métadonnée en MQTT** ; le dashboard l'affiche.
- **DoD :** depuis un téléphone, on envoie « bonjour » → il apparaît (déchiffré) côté ESP32
  **et** un événement s'affiche sur le dashboard. Le contenu est **illisible** si on sniffe
  le BLE (preuve du chiffrement).

### Phase 2 — Palier 2 : stockage & relais (le MVP)
- L'ESP32 implémente la **file de stockage** (avec ID, TTL, expiration) et l'anti-doublon.
- Ajouter un **2ᵉ ESP32** : un message destiné à B est **gardé** par l'ESP32 #1 puis
  **retransmis** à l'ESP32 #2 (puis au téléphone B) quand le chemin s'ouvre.
- Implémenter l'**ACK signé** qui remonte et **purge** les copies.
- Le dashboard affiche les **métriques** (sauts, délai, état, occupation mémoire).
- **DoD :** scénario complet « A → (ESP32 stocke) → … → B déchiffre → ACK → purge »
  reproductible, visible sur le dashboard. **= MVP atteint.**

### Phase 3 — Palier 3 : mesh téléphone-à-téléphone (bonus)
- **Prérequis (à vérifier dès la phase 0) :** confirmer que les téléphones supportent le rôle
  **périphérique BLE** (fiche 07 §5). Si non → rester au plan B (ESP32 = relais).
- Activer sur l'app le double rôle central + périphérique (`bluetooth_low_energy`).
- Deux téléphones se relaient un message **sans** ESP32 sur le trajet.
- **DoD :** A et B hors de portée directe communiquent via un téléphone C intermédiaire.

### Phase 4 — Durcissement, tests & démo
- Robustesse : reconnexions, fragmentation des messages longs, quotas mémoire, *jitter*.
- Revue de **sécurité** (checklist fiche 02, éviter les erreurs type Bridgefy).
- **Répétition de la démo** (scénario §9) + finalisation des fiches et du rapport.
- **DoD :** la démo tourne 3 fois de suite sans intervention ; le rapport est complet.

## 5. Planning indicatif (à adapter à vos dates)

| Semaine | Phase | Livrable de fin de semaine | Vos dates |
|---|---|---|---|
| S1 | Phase 0 | Environnements OK + format de message v1 figé | ____ |
| S2 | Phase 1 | Palier 1 démontrable (BLE + dashboard) | ____ |
| S3 | Phase 2 (a) | File de stockage + relais ESP32↔ESP32 | ____ |
| S4 | Phase 2 (b) | ACK + purge + métriques → **MVP** | ____ |
| S5 | Phase 3 | Mesh téléphone-à-téléphone (si faisable) | ____ |
| S6 | Phase 4 | Durcissement + démo répétée + rapport | ____ |

> Si le temps est plus court, **compressez la phase 3** (elle est optionnelle) et sécurisez
> S1–S4. Si une seule personne code l'app et une autre le firmware, gardez ce rythme ;
> à 3–4, avancez dashboard et sécurité **en parallèle** dès S2.

## 6. Répartition des tâches (proposition)

À adapter à votre effectif. Idée : chacun **pilote** un chantier mais tout le monde
**contribue** à l'intégration.

| Rôle | Chantiers principaux | Compétences développées |
|---|---|---|
| **Référent protocole & intégration** | 1 + 6 | vision d'ensemble, format de paquet, git, tests bout-en-bout |
| **Dév. firmware ESP32** | 2 (+5) | Arduino/C++, BLE, MQTT |
| **Dév. application Android** | 3 | Flutter/Dart, BLE côté mobile, UI |
| **Référent sécurité** | 4 | cryptographie appliquée, modèle de menace |
| **Référent dashboard** | 5 | MQTT, ThingsBoard/Node-RED, visualisation |

- À **2 personnes** : (A) firmware + dashboard, (B) app + protocole ; sécurité et intégration
  partagées.
- À **3–4 personnes** : un rôle chacun, le « référent protocole & intégration » coordonne.
- **Tout le monde** relit la fiche sécurité (02) : c'est le point qui fait gagner ou perdre
  des points.

## 7. Critères d'acceptation (mapping cahier des charges)

Pour le rapport/soutenance, voici comment **prouver** chaque exigence :

| Exigence | Preuve attendue |
|---|---|
| Envoyer via BT | démonstration en direct + capture du log |
| Sécuriser | montrer qu'un **sniff BLE** ne donne que du chiffré ; expliquer X25519/AEAD/Ed25519 (fiche 02) |
| Recevoir via BT | le destinataire affiche le message **déchiffré** |
| Stockage aux points de transmission | couper le lien vers B, montrer le message **en attente** dans l'ESP32, puis livraison à la reconnexion |
| Transmission entre points | scénario **multi-sauts** (A → ESP32#1 → ESP32#2 → B) tracé sur le dashboard |
| Passage par Arduino → dashboard | événements **horodatés** visibles sur le tableau de bord |

## 8. Gestion des risques

| Risque | Prob. | Impact | Mitigation |
|---|---|---|---|
| Téléphones **incompatibles** avec le rôle périphérique BLE | moyenne | élevé (bloque P3) | **tester dès S1** ; plan B = ESP32 relais ; P3 en bonus |
| **Coexistence Wi-Fi + BLE** sur ESP32 (ralentissements) | moyenne | moyen | dédier un ESP32 « passerelle » ; alterner ; tester tôt |
| **Crypto mal implémentée** (faille type Bridgefy) | moyenne | élevé | **utiliser libsodium**, ne rien coder soi-même (fiche 02) |
| **Mémoire ESP32** saturée par la file | faible | moyen | quotas + purge + expiration (fiche 03) |
| Échange initial des **clés publiques** compliqué | moyenne | moyen | QR code / présentiel / TOFU ; documenter le choix |
| **Retard** sur le planning | moyenne | moyen | le MVP (P1+P2) est prioritaire ; P3 sacrifiable |
| Messages **> MTU** BLE mal gérés | moyenne | moyen | fragmentation/réassemblage prévue dès le format v1 |

## 9. Plan de test & scénario de démonstration

**Tests unitaires / manuels par brique :**
- crypto : chiffrer puis déchiffrer un message → identique ; signature vérifiée ; message
  modifié → **rejeté**.
- stockage : un message expiré est **supprimé** ; un ACK **purge** la bonne entrée.
- anti-doublon : un même message reçu 2× n'est relayé **qu'une** fois.
- passerelle : un événement ESP32 apparaît bien sur le dashboard.

**Scénario de démo (le fil rouge de la soutenance), en 4 temps (fiche 05 §6) :**
1. A envoie un message chiffré ; B est **hors de portée**.
2. L'ESP32 **stocke** le message → le dashboard affiche « en attente, 1 saut ».
3. B arrive à portée → **retransmission** → B **déchiffre** et **répond par un ACK**.
4. L'ACK **purge** les copies → le dashboard affiche « livré ».

> Astuce démo : placez physiquement les nœuds pour que **A ne voie pas B directement**, afin
> de **forcer** le passage par un relais. C'est la preuve visuelle du mesh.

## 10. Checklist de mise en route (environnements)

**Poste de dev (chaque membre) :**
- [ ] Git configuré + `git config core.hooksPath .githooks` (le dépôt impose déjà les
      **Conventional Commits**, voir `.githooks/README.md`).
- [ ] Éditeur : VS Code (recommandé).

**Firmware ESP32 :**
- [ ] Arduino IDE **ou** PlatformIO, avec le **support ESP32** installé.
- [ ] Test « blink » téléversé par USB sur la carte Freenove.
- [ ] Libs : BLE intégré, client MQTT (PubSubClient), ArduinoJson, (crypto si besoin).

**Application Android :**
- [ ] Flutter SDK + Android Studio (ou VS Code + extension Flutter).
- [ ] `flutter doctor` sans erreur ; un téléphone en **mode développeur** / débogage USB.
- [ ] `flutter build apk` produit un `.apk` installable (test « sideload »).

**Dashboard :**
- [ ] Un **broker MQTT** (public de test, ou Mosquitto local, ou intégré à ThingsBoard).
- [ ] Instance **ThingsBoard** (cloud gratuit ou Docker) **ou** **Node-RED**.

## 11. Backlog résumé (user stories)

- *En tant qu'utilisateur, je veux **saisir et envoyer** un message pour communiquer sans
  Internet.* (P1)
- *… je veux que mon message soit **chiffré** pour que les relais ne le lisent pas.* (P1)
- *… je veux **recevoir et lire** les messages qui me sont destinés.* (P1)
- *… je veux que mon message soit **gardé** en route si le destinataire est absent.* (P2)
- *… je veux que le message **franchisse plusieurs relais** pour atteindre plus loin.* (P2)
- *… je veux un **accusé de réception** pour savoir qu'il est arrivé.* (P2)
- *En tant qu'organisateur, je veux un **dashboard** montrant l'état du réseau.* (P1/P2)
- *… je veux que **les téléphones se relaient** entre eux, ESP32 en renfort.* (P3)

## 12. Suivi & outils

- **Dépôt Git** `dengon` avec **Conventional Commits** déjà imposés par le hook `commit-msg`
  (types : `feat`, `fix`, `docs`, `refactor`…). Exemple : `feat(esp32): relais store-and-forward`.
- **Branches** : une branche par fonctionnalité, fusion via *pull/merge request* relue par
  un binôme.
- **Kanban** (GitHub Projects, Trello…) avec les colonnes *À faire / En cours / En revue /
  Fait*, alimenté par le backlog (§11).
- **Documentation vivante** : mettre à jour les fiches `docs/` au fil de l'eau (surtout le
  **format de message**, qui va évoluer).
- **Rituels** : un point d'équipe court en début de chaque semaine pour synchroniser les
  chantiers et lever les blocages.

---

### À retenir
On avance **par paliers** pour toujours avoir une démo : **P1** (lien de base) → **P2** (le
**MVP** : stockage + relais + ACK, qui couvre déjà tout le cahier des charges) → **P3** (mesh
téléphone-à-téléphone, en bonus). Le **format de message** est le chantier fondateur à figer
en premier ; la **sécurité** est le point qui fait la différence ; le **test du rôle
périphérique BLE** doit se faire **dès la première semaine** pour éviter une mauvaise surprise.
