# Stratégie de test & CI

## 1. Pyramide

```
        ┌───────────────────────┐
        │  E2E terrain (manuel) │   scénarios DoD sur vrai matériel
        ├───────────────────────┤
        │  Intégration          │   dengon-sim (multi-nœuds), dashboard e2e, banc ESP32
        ├───────────────────────┤
        │  Composant            │   chaque module de dengon-core, api dashboard
        ├───────────────────────┤
        │  Unitaire + property  │   protocol, crypto, ledger, status FSM
        └───────────────────────┘
```

Principe : **le maximum de logique est dans `dengon-core`**, testable sans radio ni
réseau. La radio (BLE) et le réseau (MQTT) sont aux frontières, mockés.

---

## 2. `dengon-core` — tests unitaires & property

| Module | Tests clés |
| --- | --- |
| `protocol` | round-trip encode/decode (tout type) ; **property** : `decode(encode(p)) == p` (`proptest`) ; fragmentation/réassemblage avec MTU aléatoire ; rejet des paquets malformés / version inconnue |
| `crypto` | vecteurs de test Noise `XX`/`X` ; sign/verify Ed25519 ; **forge rejetée** (bit-flip → verify échoue) ; `recipient_tag` stable sur la journée, différent le lendemain ; padding → taille ∈ `PAD_BUCKETS` ; déchiffrement d'enveloppe avec mauvaise clé échoue proprement |
| `identity` | QR encode/decode ; code de vérification **identique des deux côtés** et **ordre-indépendant** ; sensible à un bit de clé |
| `ledger` | `verify_chain` OK sur chaîne valide ; détecte entrée modifiée, `seq` manquante, mauvaise signature ; `export(range)` cohérent ; reprise après « redémarrage » (continuité `prev_hash`) |
| `sync::routing` | dedup (même `msgID` 2× → 1 relais) ; TTL décrémenté, 0 → drop ; clamp densité ; `RELAY_OK=0` → pas de relais ; abandon si doublon pendant le jitter |
| `sync::gossip` | GCS : pas de faux négatif ; réconciliation A/B convergente ; `GOSSIP_PUSH` repasse par le pipeline (re-dedup) |
| `sync::status` | **FSM exhaustive** : toutes transitions valides, toutes invalides rejetées ; monotonie ; ACK en double idempotent ; `read` sans `delivered` → double transition |
| `sync::courier` | budget de copies partagé correctement ; expiration ; collecte sur match de tag ; pas de fuite si tag ne matche pas |
| `observability` | chaque événement round-trip JSON canonique ; **redaction** : aucun `msg_uuid`/texte/recipient en clair ne peut être sérialisé (test négatif) ; `msg_log_id` bien haché |
| `store` (SQLite) | migrations ; idempotence `messages.insert(msg_uuid)` ; requêtes outbox ; chiffrement au repos actif |

Outils : `cargo test`, `proptest`, `cargo-nextest` en CI, `cargo llvm-cov`
(objectif ≥ 85 % sur `dengon-core`).

---

## 3. `dengon-sim` — intégration multi-nœuds

Simulateur : N instances de `dengon-core` reliées par un `Transport` **en mémoire**
avec modèle réseau scriptable.

Paramètres injectables : latence par lien, taux de perte, bande passante, **partition**
(couper un sous-ensemble), **churn** (nœuds qui apparaissent/disparaissent), horloges
désynchronisées.

Scénarios versionnés (fichiers `sim/scenarios/*.ron`) :

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

Chaque scénario = assertions sur l'état final de chaque nœud (messages, statuts,
journaux) + sur le flux d'événements produit.

**Determinisme** : seed RNG fixe → rejouable. Exécuté à chaque PR.

---

## 4. Dashboard — tests

| Niveau | Contenu |
| --- | --- |
| Unitaire (`api`) | parsing/validation de batch ; vérif signature ; reconstruction de statut depuis une séquence d'événements ; détection fork/gap/broken (réutilise `ledger`) ; redaction refusée |
| Intégration | `testcontainers` : Postgres+Timescale éphémère ; publier des batchs MQTT réels (broker de test) → vérifier projections `messages`/`message_hops`/`links` ; idempotence (rejouer un batch) |
| API | golden tests sur les réponses REST ; WebSocket : un event ingéré → push reçu |
| Front | composants (Vitest + Testing Library) ; e2e Playwright sur les 4 écrans avec BDD seedée ; parcours d'un `msg_log_id` connu |
| Charge | `k6` / injecteur : 10k events/min pendant 10 min → latence d'ingestion, pas de perte (QoS1+dédup), taille BDD |

---

## 5. Firmware ESP32 — tests

| Niveau | Contenu |
| --- | --- |
| Unitaire host | `libdengon_core` compilée pour l'hôte → mêmes tests que §2 pour les modules embarqués (`protocol`, `routing`, `gossip`, `ledger`) |
| Unitaire cible | Unity (framework ESP-IDF) : `Store` NVS/littlefs, buffer ring de logs (wrap-around), curseur de journal persistant |
| Intégration banc | 2-3 ESP32 + 1 téléphone + broker MQTT local : relais multi-saut, dépôt/collecte d'enveloppe, remontée MQTT |
| Résilience | couper le Wi-Fi 10 min → logs bufferisés puis flushés dans l'ordre ; `esp_restart()` → journal reprend sans rupture de chaîne ; saturer PSRAM → refus d'enveloppes mais routage OK |
| Charge | injecteur BLE (nRF52 ou 2ᵉ ESP32) : X paquets/s → heap stable, dedup OK, pas de watchdog |
| Conformité | vecteurs de paquets partagés avec `dengon-core` (mêmes bytes in → mêmes décisions out) |

---

## 6. E2E terrain (manuel, checklist de recette)

Recette exécutée avant chaque jalon « produit » :

1. **Contact** : Alice et Bob scannent le QR, comparent le code (60 chiffres),
   marquent « vérifié ».
2. **Direct** : Alice → Bob (à 5 m). Statuts observés : en attente → parti →
   distribué → lu. Latence notée.
3. **Multi-saut** : Bob s'éloigne à 40 m avec un relais ESP32 au milieu. Nouveau
   message → livré.
4. **Destinataire absent** : Charlie éteint son BLE. Alice → Charlie. Après 5 min,
   Charlie rallume près d'un relais → reçoit. ACK revient à Alice.
5. **Expéditeur absent** : Alice envoie puis coupe le BLE 5 min. Charlie lit. Alice
   rallume → statut passe à « lu ».
6. **Dashboard** : ouvrir le parcours de chacun de ces messages ; vérifier chemin,
   statuts, horodatages ; vérifier la carte réseau et l'état des relais.
7. **Intégrité** : modifier une entrée de journal d'un relais de test (outil debug)
   → le dashboard lève `chain_broken` sous X minutes.
8. **Sécurité** : capturer le trafic BLE (nRF Sniffer) pendant l'échange → confirmer
   qu'aucun clair n'apparaît ; confirmer que le VPS n'a aucun `msgID` en clair ni
   contenu.
9. **Densité** : 8-10 appareils dans une salle → tous se voient, messages croisés
   livrés, pas d'effondrement.

Résultats consignés (latences, taux de livraison, anomalies) dans un rapport de
recette daté.

---

## 7. CI (GitHub Actions ou équivalent)

| Job | Déclencheur | Contenu |
| --- | --- | --- |
| `core` | PR, push | `cargo fmt --check`, `clippy -D warnings`, `nextest`, `llvm-cov`, `proptest` |
| `sim` | PR, push | tous les scénarios `dengon-sim` (seed fixe) |
| `audit` | PR, quotidien | `cargo audit`, `cargo deny check`, SBOM |
| `android` | PR touchant `android/` ou `dengon-ffi/` | build UniFFI, `./gradlew assembleDebug testDebugUnitTest` |
| `firmware` | PR touchant `firmware/` | `idf.py build`, tests host de `libdengon_core`, tests Unity (QEMU si possible) |
| `dashboard` | PR touchant `dashboard/` | `cargo test` (api, testcontainers), `pnpm test` + `pnpm build` (web), Playwright |
| `cross-vectors` | PR, push | vecteurs de conformité partagés core ↔ firmware ↔ dashboard (mêmes entrées → mêmes sorties) |
| `release` | tag | build artefacts (node binaries, APK, firmware .bin, images Docker), signe, publie |

Blocage de merge : `core`, `sim`, `audit`, `cross-vectors` verts obligatoires.

---

## 8. Revue de sécurité

Avant le premier déploiement « produit » (fin Lot 7) :

- revue interne du modèle de menace (`04-security.md`) ;
- **audit crypto externe** de `dengon-core::crypto` + intégration Noise ;
- test d'intrusion du dashboard (VPS) ;
- revue de la surface FFI (UniFFI) et du firmware (gestion mémoire C).
