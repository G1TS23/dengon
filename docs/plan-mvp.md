# dengon — Plan de mise en place jusqu'au MVP

> Dérivé de la conception figée dans [`docs/synthese/`](synthese/). Décrit
> **qui fait quoi, quand, dans quel ordre** pour atteindre le MVP, à **3
> personnes** (Paul, Tanguy, Olivier), avec un maximum de tâches en parallèle.
>
> - **Cible** : Definition of Done du MVP —
>   [`synthese/10-benchmarks-mvp-tests.md`](synthese/10-benchmarks-mvp-tests.md) §3.1.
> - **Échéance** : soutenance **29/09/2026** (≈ 3 semaines à partir du
>   2026-09-08).
> - **Suivi réel** : au fil de l'eau dans [`docs/suivi/`](suivi/) (ce document
>   est le *plan*, pas le *journal*).

---

## 1. Contraintes qui pilotent le plan

| Contrainte | Conséquence sur le plan |
| --- | --- |
| 3 personnes, ~3 semaines, équipe qui découvre plusieurs technos | 3 pistes parallèles + points de synchro fréquents ; périmètre resserré ; ordre de repli explicite (§7) |
| Le **cœur Rust `dengon-core`** est partagé par l'app, le nœud CLI et le firmware (A-2) | **Chemin critique** : tout le monde attend le cœur. Il démarre en premier et sa surface (FFI, `trait Transport`, format d'événements) doit être figée tôt |
| 3 spikes de dérisquage sont des **go / no-go** (Lot 0) | Semaine 1 = spikes en priorité absolue, avant de s'engager sur l'implémentation lourde |
| Divergences déjà tranchées (voir [`synthese/00-contexte-global.md`](synthese/00-contexte-global.md) §1) | Pas de re-débat : Kotlin natif, WROOM, FastAPI+SQLite+SSE, HTTPS POST, Noise en Rust, « Lu » et iOS en v2 |

---

## 2. Découpage en 3 pistes parallèles

| Piste | Responsable | Périmètre | Livrables |
| --- | --- | --- | --- |
| **P1 — Cœur Rust + firmware relais** | **Paul** | `crates/dengon-core` (protocol, crypto, sync, ledger, store, observability, api) ; `dengon-ble` ; `dengon-node` ; `dengon-sim` ; `dengon-verify` ; `firmware/dengon-relay` (ESP-IDF + NimBLE + `libdengon_core.a`) | Le protocole qui fait foi ; le simulateur ; le relais matériel |
| **P2 — Application Android** | **Tanguy** | `crates/dengon-ffi` (UniFFI, bindings Kotlin) ; `android/` : `AndroidTransport` (GATT server + advertiser + scanner + foreground service) ; UI Compose (conversations, fil, saisie, statuts, écran QR + vérification, écran réseau) | L'APK qui tourne sur 2 téléphones |
| **P3 — Dashboard + spec + rapport** | **Olivier** | `dashboard/api` (FastAPI + SSE + SQLite) + `dashboard/web` (page légère) ; intégration de `dengon-verify` ; **delta de doc sécurité** (sur la base de `powl/04`) ; rapport écrit + slides + script de démo | Le dashboard déployé sur le VPS ; les livrables écrits |

**Conception commune** (déjà faite) : [`docs/synthese/`](synthese/). Ce qui
reste commun : figer le **sous-ensemble MVP** du format de trame + le type
`INVENTORY` (réunion, S1), les **interfaces entre pistes** (§6), et
l'**intégration E2E** (S3, tous).

**Points d'avancement** : 3×/semaine (lun. / mer. / ven.), 20 min, chacun dit
« fait / en cours / bloqué par ».

---

## 3. Vue d'ensemble — 3 semaines

| Semaine | Objectif | P1 (Paul) | P2 (Tanguy) | P3 (Olivier) |
| --- | --- | --- | --- | --- |
| **S1** (08–14/09) | **Dérisquer + fondations** | Workspace + CI ; **Spike A** ; `protocol` (encode/decode) ; figer `trait Transport` + FFI + format d'événements | **Spike C** (hello mesh Android) ; squelette app + service de fond | **Spike B** (btleplug peripheral) ; squelette `api` FastAPI + SQLite + `/ingest/batch` ; **delta doc sécurité** |
| **S2** (15–21/09) | **Construire en parallèle** | `crypto`, `identity`, `sync` (routing + inventory + status + courier), `ledger`, `store` ; `dengon-sim` + scénarios ; `dengon-node` | `dengon-ffi` complet ; `AndroidTransport` fonctionnel ; UI : envoyer / recevoir / statuts / QR | `api` : ingestion + reconstruction de statut + SSE ; `dengon-verify` branché ; `web` : parcours d'un message + carte réseau ; déploiement VPS |
| **S3** (22–28/09) | **Intégrer, démontrer, rédiger** | firmware `dengon-relay` sur WROOM (lib core + transport NimBLE + HTTPS) ; support intégration | intégration app ↔ relais ; écran réseau ; polish UI ; recette | **rapport + slides + script de démo** ; dashboard : intégrité des journaux + alerting ; répétition de la démo |
| **29/09** | **Soutenance** | — démo répétée 3× sans intervention + rapport rendu — | | |

---

## 4. Détail des tâches par piste

### P1 — Cœur Rust + firmware (Paul)

**S1 — fondations & spike (bloquant pour tous)**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P1.1 | Créer le workspace `crates/` (layout [`synthese/04-architecture.md`](synthese/04-architecture.md) §5), `Cargo.toml`, `rustfmt`/`clippy`, CI `core` (fmt + clippy -D warnings + nextest) | — | dépôt qui compile |
| P1.2 | **Spike A** : `dengon-core` (`protocol` + `crypto` minimal) cross-compile pour `xtensa-esp32-none-elf` (`no_std`) ? `snow` + `ed25519-dalek` + `sha2` ? | P1.1 | **rapport de décision** (B-1 : tout Rust / repli `trait Crypto` + mbedTLS pour le seul handshake de lien) |
| P1.3 | `protocol` : types de paquets (12 + `INVENTORY`), `flags`, en-tête L3, `msgID`, encode/decode, fragmentation L2 ([`synthese/05-protocole-et-trame.md`](synthese/05-protocole-et-trame.md)) + round-trip + property tests | P1.1 | crate `protocol` testé |
| P1.4 | **Figer les 3 interfaces inter-pistes** (§6) : signature `trait Transport`, surface `dengon-ffi` (v0), enveloppe d'événement + endpoint `/ingest/batch` | P1.3 | 3 contrats écrits → débloque P2 et P3 |

**S2 — le cœur**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P1.5 | `crypto` : Ed25519 sign/verify, Noise `XX` (session), Noise `X` (enveloppe scellée), `recipient_tag`, padding `PAD_BUCKETS`, XChaCha20 champ par champ | P1.2 | crate `crypto` + vecteurs de test |
| P1.6 | `identity` : keypair + coffre, QR encode/decode, code de vérification 60 chiffres (empreintes — D-1) | P1.5 | crate `identity` |
| P1.7 | `ledger` : `append(event)`, `verify_chain()`, `export(range)`, reprise après redémarrage | P1.5 | crate `ledger` |
| P1.8 | `store` (SQLite via `rusqlite`) : schéma [`synthese/09-dashboard-et-donnees.md`](synthese/09-dashboard-et-donnees.md) §11.1, chiffrement champ par champ | P1.5 | crate `store` |
| P1.9 | `sync` : `routing` (pipeline §6.1, TTL, dedup, jitter, clamp densité, anti-inondation), `inventory` (échange d'inventaire + push du manquant), `status` (machine à états **MVP sans READ**), `courier` (dépôt/collecte d'enveloppes) | P1.3, P1.7, P1.8 | crate `sync` |
| P1.10 | `observability` : catalogue d'événements ([`synthese/09-dashboard-et-donnees.md`](synthese/09-dashboard-et-donnees.md) §9), JSON canonique, redaction (test négatif : aucun clair ne sort) | P1.7 | crate `observability` |
| P1.11 | `api` (façade) : `send_message`, `poll_events`, `on_peer_connected`, `mark_read` (no-op v1)… ; `dengon-node` (CLI, transport `btleplug` via `dengon-ble`) | P1.5–P1.10 | binaire `dengon-node` |
| P1.12 | `dengon-sim` : N cœurs reliés par transport en mémoire scriptable ; scénarios `direct`, `multihop`, `recipient_offline`, `sender_offline`, `partition_merge`, `flood`, `dup_paths`, `tamper`, `key_change` ; job CI `sim` | P1.11 | **les 5 scénarios du DoD passent en simulation** |
| P1.13 | `dengon-verify` : binaire qui lit un export de journal + `LOG_ATTEST` et rend `ok / broken / fork / gap` | P1.7 | binaire pour P3 |

**S3 — firmware relais**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P1.14 | `libdengon_core.a` : cross-compile `protocol` + `sync` + `ledger` + `observability` pour xtensa ; header C via `cbindgen` | P1.2, P1.9 | lib + header |
| P1.15 | `firmware/dengon-relay` (ESP-IDF + NimBLE) : `transport_nimble.c`, tâches FreeRTOS (route / inventory / courier / ledger / ship), `Store` ESP32 (SRAM + NVS + littlefs) | P1.14 | firmware qui relaie en BLE |
| P1.16 | Client HTTPS (`esp_http_client`) + buffer ring littlefs + JWT + signature Ed25519 des batchs ; provisioning (Wi-Fi + endpoint + clé) | P1.15, P3 endpoint | firmware qui remonte les logs |
| P1.17 | Banc de test : 2 WROOM + 1 téléphone → **scénarios 2 et 3 du DoD sur vrai matériel** | P1.16, P2 | démo relais |

### P2 — Application Android (Tanguy)

**S1**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P2.1 | Projet Android (Kotlin + Compose), `foregroundServiceType="connectedDevice"` + permissions BLE/localisation, notification permanente | — | app qui démarre |
| P2.2 | **Spike C** (« hello mesh ») : `BluetoothGattServer` + `BluetoothLeAdvertiser` + `BluetoothLeScanner` dans un foreground service ; 2 téléphones échangent 20 octets ; **mesurer le MTU réel négocié** | P2.1 | **go / no-go** de l'app native + valeur MTU (C-1) |
| P2.3 | Maquettes des écrans (conversations, fil + saisie, écran QR + code de vérification, écran réseau) — statiques, données bidon | — | UI navigable |

**S2**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P2.4 | `dengon-ffi` (avec Paul) : générer les bindings Kotlin, charger le `.so` (JNA), câbler `Node(config, transport, observer)` | P1.4 (contrat FFI), P1.11 | pont Rust ↔ Kotlin |
| P2.5 | `AndroidTransport : Transport` : implémenter `start` / `poll` / `send` / `broadcast` sur GATT server + scanner + advertiser ; règle du plus petit `peerID` initie ; fragmentation BLE selon MTU | P2.2, P2.4 | transport BLE réel |
| P2.6 | UI branchée au cœur : envoyer un message (`send_message`), recevoir (`poll_events` → `NodeEvent`), afficher les statuts, écran QR (scan + affichage), comparaison du code 60 chiffres, alerte changement de clé | P2.4 | **scénario 1 du DoD** (2 téléphones à portée) |
| P2.7 | Persistance : base `store` sur le téléphone (via le cœur), historique jusqu'à suppression manuelle | P2.6 | messages persistés |

**S3**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P2.8 | Intégration app ↔ relais ESP32 : multi-saut, destinataire absent, expéditeur déconnecté | P1.17 | **scénarios 2 et 3** côté app |
| P2.9 | Écran réseau (pairs visibles, « Connecté » / « Isolé »), mode éco, bouton « Renvoyer », statut « Échec » | P2.6 | UX complète |
| P2.10 | Polish, gestion d'erreurs, matrice d'appareils testés ; recette (checklist [`synthese/10-benchmarks-mvp-tests.md`](synthese/10-benchmarks-mvp-tests.md) §4.6) | P2.8 | APK de démo |

### P3 — Dashboard + spec + rapport (Olivier)

**S1**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P3.1 | **Spike B** : `btleplug` en rôle **peripheral** sur Linux (aide Paul à valider `dengon-node` comme nœud complet) | — | rapport court |
| P3.2 | **Delta de doc sécurité** : partir de `powl/04-security.md`, figer le résultat attendu du Spike A, la formule `recipient_tag` (D-2), le mapping des colonnes chiffrées (B-3), les incohérences D-1/D-4/D-5 | conception | `docs/…/securite.md` (delta) |
| P3.3 | Squelette `dashboard/api` (FastAPI) : `POST /ingest/batch` (parse + schéma), base SQLite (migration, schéma [`synthese/09-dashboard-et-donnees.md`](synthese/09-dashboard-et-donnees.md) §11.2), `GET /healthz` | P1.4 (contrat événements) | api qui accepte des batchs bidon |
| P3.4 | Page web statique (liste des messages suivis + détail, maquette [`synthese/11-glossaire-biblio-annexes.md`](synthese/11-glossaire-biblio-annexes.md) Annexe B) | — | `dashboard/web` |

**S2**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P3.5 | Ingestion complète : vérif signature Ed25519, dédup `event_id`, liste blanche + JWT, rejet `msgID` non haché | P3.3 | ingestion sûre |
| P3.6 | Projections + reconstruction de statut ([`synthese/09-dashboard-et-donnees.md`](synthese/09-dashboard-et-donnees.md) §10) : `messages`, `message_hops`, `links`, `nodes` ; tolérance aux événements en retard / désordre / doublon | P3.5 | statut par message |
| P3.7 | `GET /api/stream` en **SSE** ; la page web se rafraîchit en direct | P3.6 | temps réel |
| P3.8 | Brancher `dengon-verify` en sous-processus : `GET /api/integrity` ; écran « intégrité des journaux » | P1.13 | vérif de chaîne |
| P3.9 | Déploiement VPS : reverse-proxy (Caddy/nginx) + TLS, `uvicorn`, script de purge par session, enregistrement des relais (`POST /api/nodes`) | P3.7 | **dashboard en ligne** |

**S3**

| # | Tâche | Dépend de | Sort quoi |
| --- | --- | --- | --- |
| P3.10 | Carte réseau + flotte de relais + alerting (`relay_silent`, `chain_broken`, `fork_detected`…) | P3.6, P1.17 | **scénario 4 du DoD** |
| P3.11 | **Rapport écrit** : problème, état de l'art, conception, sécurité, choix technologiques (s'appuyer sur [`synthese/`](synthese/) et `01-sujets-a-trancher.md` pour le « pourquoi ») | conception | rapport |
| P3.12 | **Slides + script de démo** : scénario campus concret, enchaînement des 6 points du cahier des charges, plan B si une manip échoue | P3.11 | support de soutenance |
| P3.13 | Consolider [`docs/suivi/06-support-oral.md`](suivi/06-support-oral.md) à partir du journal | tous | support oral |

---

## 5. Jalons (Definition of Done partielle)

| Jalon | Quand | Critère « fait » |
| --- | --- | --- |
| **J0 — Go/No-go** | fin S1 | Spike A tranché (crypto) ; Spike C réussi (2 Android échangent 20 o en foreground) ; workspace + CI verts ; 3 interfaces inter-pistes écrites |
| **J1 — Le cœur en simulation** | mi-S2 | `dengon-sim` : `direct` + `multihop` + `recipient_offline` + `sender_offline` + `partition_merge` passent |
| **J2 — 2 téléphones parlent** | fin S2 | Scénario 1 du DoD sur 2 vrais téléphones (message chiffré, statuts en attente → parti → distribué, contact QR + code 60 chiffres) |
| **J3 — Dashboard en ligne** | fin S2 | Dashboard déployé sur le VPS, reçoit des batchs du `dengon-node`, reconstruit un statut, SSE fonctionne |
| **J4 — MVP matériel** | mi-S3 | Scénarios 2, 3, 4 du DoD sur vrai matériel (2 Android + ≥ 1 relais WROOM + dashboard qui trace le parcours et détecte une altération) |
| **J5 — Prêt pour la soutenance** | fin S3 | Recette E2E (9 points) passée ; démo répétée **3× sans intervention** ; rapport + slides rendus |

---

## 6. Interfaces entre pistes (à figer en S1 — Paul + réunion)

Ces contrats permettent aux 3 pistes d'avancer **sans se bloquer**. Une fois
écrits, ils ne changent qu'en réunion.

| Interface | Entre | Contenu figé | Fichier de référence |
| --- | --- | --- | --- |
| **`trait Transport`** | P1 ↔ P2, P1 ↔ firmware | signatures `start` / `poll` / `send` / `broadcast`, types `TransportEvent`, `LinkId` | [`synthese/04-architecture.md`](synthese/04-architecture.md) §3 |
| **Surface `dengon-ffi`** | P1 ↔ P2 | objets exportés (`Node`, `TransportConfig`), callback interfaces (`Transport`, `NodeObserver`), enum `NodeEvent` | [`synthese/04-architecture.md`](synthese/04-architecture.md) §2-3 |
| **Format d'événement + `/ingest/batch`** | P1/firmware ↔ P3 | enveloppe JSON (`event_id`, `node_id`, `seq`, `ts_ms`, `name`, `payload`), signature du batch, noms d'événements | [`synthese/09-dashboard-et-donnees.md`](synthese/09-dashboard-et-donnees.md) §9 |
| **Sous-ensemble MVP du format de trame** | tous | types de paquets actifs au MVP + numéro exact du type `INVENTORY` | [`synthese/05-protocole-et-trame.md`](synthese/05-protocole-et-trame.md) §4 |
| **Vecteurs de conformité** | P1 ↔ firmware ↔ P3 | jeux de « bytes in → décision out » partagés (job CI `cross-vectors`) | [`synthese/10-benchmarks-mvp-tests.md`](synthese/10-benchmarks-mvp-tests.md) §4.7 |

---

## 7. Chemin critique & ordre de repli

**Chemin critique** : `protocol` (P1.3) → interfaces figées (P1.4) → `crypto` +
`sync` (P1.5, P1.9) → `dengon-ffi` (P2.4) → app branchée (P2.6) → intégration
relais (P1.17 + P2.8). Si P1 prend du retard, **tout** glisse — d'où le cœur en
premier et à temps plein sur P1.

**Ordre de repli** (si un jalon dérape ; reprend l'arbitrage
[`synthese/10-benchmarks-mvp-tests.md`](synthese/10-benchmarks-mvp-tests.md) §3.6) :

1. **On garde** : hors-ligne + relais (le cœur du sujet). Scénarios 1, 2, 3 du
   DoD.
2. **On allège si besoin** : la **sécurité** peut passer d'un déchiffrement
   complet à « le relais ne voit que du chiffré + signature vérifiée » (Noise
   `X` d'abord, session `XX` ensuite).
3. **On sacrifie en dernier** : le **dashboard** — repli sur une liste live des
   `msg_log_id` + statut (SSE), voire une page rafraîchie à la main, voire une
   maquette statique. Olivier bascule alors sur le **rapport / les slides**
   (aussi son périmètre).
4. **Déjà hors MVP** (ne pas y toucher) : statut « Lu », iOS, gossip GCS, budget
   de copies Spray-and-Wait, groupes, pièces jointes.

**Ce qu'on ne coupe pas** : les 3 spikes de S1 (ce sont eux qui disent si le
plan tient), la CI `core` + `sim` (le filet de sécurité), la répétition de la
démo en fin de S3.

---

## 8. Definition of Done du MVP (rappel) & checklist de démo

**DoD** — [`synthese/10-benchmarks-mvp-tests.md`](synthese/10-benchmarks-mvp-tests.md) §3.1 :
2 Android échangent un message chiffré (statuts en attente → parti → distribué) ;
3ᵉ appareil hors portée joint via ≥ 1 relais ESP32 ; destinataire éteint reçoit
au retour + expéditeur déconnecté voit ses statuts à la reconnexion ; dashboard
montre parcours + carte + état des relais + **détecte une altération de journal**
simulée ; contact par QR + code 60 chiffres + détection de changement de clé ;
E2E vérifié (relais capturé = aucun clair), `cargo audit` propre ; CI verte.

**Script de démo (scénario campus, ~5 min)** :

1. Alice et Bob scannent leur QR, comparent le code 60 chiffres → « vérifiés ».
2. Alice → Bob à portée : message chiffré, statuts *en attente → parti →
   distribué*. Montrer un sniff BLE en parallèle : illisible.
3. Bob s'éloigne derrière un relais WROOM : Alice → Bob multi-saut → livré ; le
   dashboard affiche le parcours (Alice → relais → Bob).
4. Charlie coupe son BLE. Alice → Charlie → enveloppe déposée sur le relais
   (dashboard : `envelope.stored`). Charlie rallume près du relais → reçoit ;
   l'ACK remonte → statut *distribué* chez Alice.
5. On modifie une entrée du journal d'un relais de test → le dashboard lève
   `chain_broken`.

---

## 9. Suivi

- Ce document = le **plan**. Il n'est mis à jour que si l'équipe re-planifie
  (réunion).
- Le **journal de ce qui est réellement fait** : une entrée par session dans
  [`docs/suivi/00-journal.md`](suivi/00-journal.md), + [`docs/suivi/01-etat-du-code.md`](suivi/01-etat-du-code.md)
  rafraîchi, + fiche module dans [`docs/suivi/modules/`](suivi/modules/) (règles :
  [`docs/suivi/README.md`](suivi/README.md)).
- Les **écarts** entre le code et la conception → [`docs/suivi/03-ecarts-conception.md`](suivi/03-ecarts-conception.md).
