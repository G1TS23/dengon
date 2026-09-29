# Écarts entre le code et la conception

La conception ([`docs/powl/`](../powl/)) est une cible, pas un contrat. Quand
l'implémentation s'en écarte (contrainte technique, simplification, meilleure idée,
erreur de conception découverte), on le note **ici**, avec la raison, pour :

- ne pas se faire piéger par une doc de conception périmée ;
- pouvoir l'expliquer à l'oral (« on avait prévu X, en pratique Y parce que Z »).

Si un écart est structurant, mettre aussi à jour le document `docs/powl/` concerné
et le mentionner dans l'entrée de journal.

---

## Modèle d'entrée



---

### 2026-09-29 — Relais : preuve de possession légère, initiation par le téléphone, liaisons fantômes (US-312, essai sur carte et revue PR #129)

- **Prévu :**
  - `docs/powl/03` §6.1 : règle anti-boucle symétrique, le plus petit `peerID` initie, quel que soit le type de nœud ;
  - `synthese/06` : les pairs s'authentifient par un handshake `XX`. Rien ne dit comment un client authentifie un relais, qui n'ouvre pas de session.
- **Réel :**
  1. **Relais lié en deux temps** (`api::Node::prove_relay`). L'`ANNOUNCE` ne sert qu'à enregistrer la clé. Rien n'est confié au relais avant un paquet signé par cette clé, adressé à nous et daté à ±2 min de notre horloge : en pratique, l'`INVENTORY` qu'il envoie à la liaison.
     - **Risque restant, accepté pour le prototype :** un attaquant présent qui **retransmet en direct** l'`ANNOUNCE` puis l'`INVENTORY` du vrai relais passe cette preuve. Il reçoit alors des enveloppes, toujours chiffrées : il apprend qui écrit, et peut les jeter.
     - Même risque si un faux relais se lie avant le vrai : `Maillage` ne garde que le premier lien vers ce `peerID`, ce qui masque le vrai relais.
     - Parade complète, non faite : un défi signé à la liaison (nonce du client, repris dans un paquet signé du relais), ou des clés de relais épinglées dans la configuration.
  2. **Asymétrie d'initiation.** Un téléphone initie toujours vers un relais (drapeau `RELAY` de l'annonce). Un relais n'initie que vers un relais, avec la règle anti-boucle. Deux raisons :
     - sur carte, le relais cessait de scanner et ne rappelait jamais un téléphone de préfixe plus grand ;
     - sinon, chaque téléphone occupait deux des trois liens du relais.
  3. **Annonce du téléphone sans octet de flags** (6 octets). Le relais l'accepte, flags à 0. Le format de `docs/powl/03` §6.1 (7 octets) n'est tenu que par le relais.
  4. **Liaison fantôme** : à l'arrêt du service Android, le serveur GATT est fermé mais la liaison BLE reste ouverte côté relais jusqu'à la coupure du Bluetooth du téléphone. Elle occupe un lien du relais. Non corrigé : c'est la pile Android qui la tient.
  5. **Morceau d'abandon L1** (`0x40`, 1 octet) ajouté au format de morceau, pour qu'un échec d'émission au milieu d'une trame ne corrompe pas la suivante.
  6. **Carte de contact par intent**, en build debug seulement (`--es dengon.carte_debug`). La carte publique locale est écrite dans logcat. Cela sert à piloter l'essai par `adb` sans caméra. Ignoré hors build debuggable.
  7. Après un redémarrage du Bluetooth, le transport Android n'est pas reconstruit (il faut arrêter puis relancer le service). L'app ne plante plus, elle jette la file du lien.
- **Pourquoi :** trouvé pendant l'essai sur matériel et la revue de la PR #129 (Oswin).
- **Impact :** `crates/dengon-core/src/api.rs`, `firmware/dengon-relay/components/dengon_transport_core/`, `main/transport_nimble.c`, `android/.../ble/transport/{GattRadio,Annonce,FragmentationBle}.kt`, `MainActivity.kt`.
### 2026-09-29 — Statut « Parti » affiché « Envoyé » (US-321)

- **Prévu :** `docs/synthese/07-cycle-de-vie-et-statuts.md` §1 : `IN_FLIGHT` = « Parti ».
- **Réel :** l'app affiche « Envoyé » (`libelleStatut`). Le sens (remis à ≥ 1 relais
  ou pair, pas encore chez le destinataire) est inchangé.
- **Pourquoi :** US-321 demande un langage courant ; « envoyé » est le mot des
  messageries.
- **Doc de conception mise à jour ?** non : à reporter dans la synthèse si retenu.

---

### 2026-09-29 — « Renvoyer » crée un nouveau message, relais reconnu par son pseudo (US-313)

- **Prévu :** US-313 — bouton « Renvoyer » sur un message en échec ; écran
  réseau avec « relais atteints ».
- **Réel :** le cœur n'a pas de reprise d'un message `EXPIRED` (terminal) :
  « Renvoyer » réémet le texte comme **nouveau** message, l'ancien reste
  « Échec » dans le fil. Le téléphone ne sait pas qu'un pair est un relais
  (l'`ANNOUNCE` n'a pas de capacité exploitable) : on le devine au pseudo
  `relay-…`.
- **Pourquoi :** évite d'étendre le contrat FFI v1 pour une US `Should`.
- **Conséquence :** un pair qui choisit le pseudo `relay-x` apparaît comme
  relais (affichage seulement, aucun effet de sécurité). À remplacer par le
  champ `caps` de l'`ANNOUNCE` (synthese/09) quand il sera lu.
### 2026-09-29 — Le téléphone se sert du relais : ce qui diffère de la conception (US-312)

- **Prévu :** `synthese/07` §5 et `synthese/10` §3.1 (DoD 2 et 3). Un 3ᵉ
  téléphone hors de portée est joint via un relais. L'enveloppe scellée est
  déposée, offerte (`ENVELOPE_OFFER`), demandée (`ENVELOPE_REQUEST`), remise.
  L'accusé revient « par session si possible, sinon par enveloppe »
  (`synthese/07` §4).
- **Réel :**
  1. **Découpage L1 du relais aligné sur Android.** L'US-220 posait
     « 1 trame = 1 PDU ATT, pas d'en-tête » (entrée du 2026-09-28 ci-dessous),
     ce qui la rendait incompatible avec `FragmentationBle.kt` de l'app.
     `dengon_transport_core` réassemble désormais les morceaux Android
     (1 octet d'en-tête, bit 7 = suite) et découpe à l'émission. Une trame
     vaut au plus 514 o, quel que soit le MTU. Le réassemblage est borné à
     514 o, contre 8 192 côté Android, pour préserver le tas.
  2. **Messages courts seulement via le relais.** Une enveloppe au palier de
     padding 256 fait ≈ 456 o. Au palier 512, soit un texte d'environ
     220 octets UTF-8 ou plus, elle dépasse une trame du relais : elle ne lui
     est pas confiée et reste « En attente » tant que le destinataire n'est
     pas lié en direct. Aucune erreur n'est levée à l'envoi
     (`texte_trop_long_pour_une_trame_du_relais_reste_en_attente`).
  3. **Rôle client seulement.** Le téléphone demande (`REQUEST`) ce qu'un
     relais lui offre. Il n'offre pas lui-même les enveloppes qu'il porte pour
     un tiers, et n'échange pas d'`INVENTORY` : l'INVENTORY du relais est
     ignoré.
  4. **Aléa de l'accusé par enveloppe : `OsRng`.** `on_bytes_received` ne
     reçoit pas de RNG de l'appelant. Plutôt que de changer sa signature, et
     donc le FFI, `dengon-node` et les tests, `api` (en `std` seulement) tire
     `rand_core::OsRng`, comme `store` pour ses nonces. Feature `std` →
     `rand_core/getrandom`.
  5. **Pas de session XX avec un relais.** Un voisin `CAP_RELAY` est lié sur
     la foi de son `ANNOUNCE` signé (`Node::on_relay_connected`), sans preuve
     de possession de clé par handshake. Ce que le relais nous adresse est
     vérifié avec la clé de cet `ANNOUNCE`. Un faux « relais » ne reçoit que
     des enveloppes chiffrées, qu'un porteur tiers voit de toute façon.
  6. **`.udl` v1 étendu** : `announce_is_relay`, `on_neighbor_announced`.
     À annoncer en point d'équipe, comme les extensions de l'US-302 et de
     l'US-306.
  7. **Démo à 2 téléphones + 1 ESP32.** L'ESP32 compte comme 3ᵉ appareil du
     scénario 2. Pour garantir « hors de portée » sur une table, une option
     debug « Relais seulement » (écran debug transport) ignore les liens
     vers un téléphone. Elle est désactivée par défaut et perdue au
     redémarrage de l'app.
  8. **Les événements des téléphones ne remontent pas au dashboard.** L'app
     n'a pas de client HTTP : seuls les événements du relais (US-309)
     prouvent le parcours côté dashboard.
- **Pourquoi :** l'US-312 est le premier moment où l'app et le relais se
  parlent. Les trous ci-dessus n'avaient été visibles qu'en lisant les deux
  côtés ensemble. On a choisi de les combler dans l'US plutôt que d'ouvrir
  une US du même sprint, ce qu'interdit la règle « une US ne dépend jamais
  d'une US du même sprint ». L'estimation de 5 points est dépassée.
- **Impact :** code dans `crates/dengon-core/src/api.rs`,
  `firmware/dengon-relay/components/dengon_transport_core/`,
  `android/app/src/main/java/com/dengon/app/ble/Maillage.kt`. Procédure
  d'essai : [`e2e/US-312-scenarios-2-3.md`](e2e/US-312-scenarios-2-3.md).

---

### 2026-09-29 — Clé du coffre perdue : l'identité est réinitialisée, sans écran dédié (US-302, revue PR #109)

- **Prévu :** `docs/synthese/06-securite.md` — l'identité de l'appareil est
  stable ; rien ne décrit le cas où la clé du Keystore qui protège le coffre
  disparaît.
- **Réel :** si l'enveloppe de la clé du coffre ne s'ouvre plus (clé du
  Keystore effacée ou invalidée, `AEADBadTagException`/`InvalidKeyException`),
  ou si le coffre existe sans clé enregistrée, `IdentiteLocale.cleDuCoffre`
  **oublie** clé et coffre et en tire une neuve : nouvelle identité, nouveau
  `peerId`, avertissement dans logcat seulement. Aucun écran « identité
  irrécupérable » n'est montré.
- **Pourquoi :** avant la revue, l'app plantait à chaque lancement, sans
  issue (`by lazy` relance l'exception à chaque accès). Le coffre étant de
  toute façon illisible, il n'y a rien à sauver ; les contacts et messages ne
  sont pas persistés (écart ci-dessous), donc la perte se limite à l'identité.
  Un écran de confirmation demanderait de rendre l'ouverture du nœud
  asynchrone côté UI, prévu avec l'US-306.
- **Conséquence :** les pairs qui avaient appairé l'ancienne identité doivent
  refaire l'appairage QR. À revoir quand contacts et messages seront persistés
  (la réinitialisation les rendrait alors orphelins).
### 2026-09-29 — US-309 : jeton et mot de passe Wi-Fi en clair sur la console série (revue PR #120)

- **Prévu :** `docs/synthese/06-securite.md` : secrets protégés sur l'appareil
  (NVS chiffré, clés hors de portée).
- **Réel :** le JWT (`dash token <jwt>`) et le mot de passe Wi-Fi
  (`wifi <ssid> <mdp>`) sont tapés **en clair** sur la console série, et
  stockés en NVS **non chiffré** (écart NVS déjà consigné à l'US-308).
  Quiconque branche un câble série lit l'écho de la console ou la NVS.
- **Raison :** prototype sans provisionnement ; la console est le seul canal
  d'administration.
- **Conséquences :** relais de démo seulement. Le jeton ne donne que le droit
  de poster les événements **de ce relais** (le dashboard vérifie en plus la
  signature Ed25519, dont la clé ne sort pas du handle Rust). Pistes : NVS
  chiffré (eFuse), provisionnement par BLE authentifié.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-29 — US-309 sur carte : horodatage avant SNTP, reconnexion lente, intermédiaire TLS épinglé

- **Prévu :** `docs/synthese/08` §6 : NTP au boot, événements horodatés en
  temps réel ; racine du dashboard embarquée.
- **Réel :**
  1. Tant que le Wi-Fi n'a pas donné l'heure (ou qu'aucun `ANNOUNCE` ne l'a
     apprise), `ts_ms` = uptime en ms : sur le VPS, `relay.boot` et les
     premiers `relay.health` sont datés de 1970.
  2. Reconnexion Wi-Fi par backoff 1 s → 60 s (+ 25 % de gigue) : jusqu'à
     ~75 s d'attente après le retour du réseau (mesuré ~45 s).
  3. Essai fait avec l'**intermédiaire** Caddy épinglé (valide jusqu'au
     2026-10-05) faute d'accès à la racine dans la session.
- **Raison :** (1) pas d'horloge RTC sauvegardée sur WROOM ; (2) le backoff
  ménage l'antenne partagée avec le BLE ; (3) contrainte d'accès.
- **Conséquences :** (1) dates fausses au dashboard pour les événements de
  démarrage — piste : ne pas exporter avant heure connue, ou recaler au
  moment de l'envoi ; (2) acceptable pour un export différé ; (3) reflasher
  avec la racine avant le 2026-10-05.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-29 — Export du relais vers le dashboard (US-309) : ring en RAM, jeton manuel, `rssi_avg` Wi-Fi

- **Prévu :** `docs/synthese/08-relais-esp32.md` §4-5 : une tâche `ship_task`
  qui envoie un **anneau de logs en littlefs** (~256 Ko) avec un **curseur en
  NVS** ; §6 : `relay.health` porte `rssi_avg` (RSSI moyen des voisins).
  `docs/synthese/09` §7 : « enregistrer chaque relais via `POST /api/nodes` et
  lui remettre un JWT ». `09` §9 décrit une signature **par enveloppe**.
- **Réel :**
  1. **Buffer ring en RAM** (8 Kio par défaut depuis la revue #120, `CONFIG_DENGON_SHIP_RING_BYTES`),
     alimenté par `ledger_task`. Le journal complet reste en littlefs (US-308),
     mais le ring d'export est perdu à un redémarrage : ce qui n'était pas
     encore parti n'est pas renvoyé (le journal, lui, reste vérifiable par
     `dengon-verify`). Hors ligne long, les plus anciens sont écrasés et
     comptés (`logs_dropped`).
  2. **Enregistrement manuel** : `dash id` sur la console →
     `tools/register_relay.py` → `dash token <jwt>`. Le dashboard (US-216)
     émet des jetons de **24 h**, sans renouvellement, et refuse de
     réenregistrer un `node_id` connu (409) : il faut réenregistrer après
     chaque purge de la base de démo, et un relais laissé plus de 24 h
     s'arrête d'exporter (401, il garde ses événements et attend un jeton).
  3. **`rssi_avg` = RSSI du point d'accès Wi-Fi** (-120 hors connexion) : le
     transport NimBLE ne remonte pas le RSSI des voisins BLE.
  4. Signature **par batch** (`CANONICAL.md` §2, ce que vérifie le
     dashboard) : `09` §9 est périmé sur ce point.
- **Raison :** (1) le critère de l'US est « sans dépasser la mémoire » ; un
  ring statique en RAM le tient par construction et se teste sur l'hôte, sans
  toucher au format du journal ni ajouter un second curseur à garder
  cohérent. (2) le dashboard n'offre ni renouvellement ni réémission (hors
  périmètre firmware). (3) pas d'API RSSI dans le contrat `Transport`.
- **Conséquences :** perte possible des événements en attente au
  redémarrage ; démo à préparer par un enregistrement le jour même. Pistes :
  relire `ledger.bin` depuis un curseur NVS « dernier `seq` accepté » (le
  format le permet), endpoint de renouvellement côté dashboard.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-29 — Relais ESP32 (US-308) : pas de `CryptoResolver`, pas de trait `Store`, heure apprise, client pas prêt

- **Prévu :** l'issue #46 demande un « `CryptoResolver` custom branché sur
  `esp_fill_random()` » et un « `Store` implémenté sur NVS + littlefs ».
  `docs/synthese/08-relais-esp32.md` §3 prévoit un NVS **chiffré par eFuse**,
  le journal en littlefs, et le Wi-Fi/SNTP pour l'heure. §4 prévoit un relais
  qui dialogue avec les téléphones (`INVENTORY`, `ENVELOPE_OFFER/REQUEST`).
- **Réel :**
  1. **Pas de second `CryptoResolver`.** `crypto::rng::CallerResolver`
     (US-204) remet déjà à `snow` n'importe quel `RngCore + CryptoRng`.
     `dengon-core-ffi` fournit `PlatformRng` (au-dessus d'un pointeur de
     fonction C) et `dengon_noise_selftest`, que le firmware appelle au boot
     avec `esp_fill_random`, radio allumée. Les secrets du premier boot sont
     tirés sous `bootloader_random_enable()`, avant la radio.
  2. **Pas de trait `Store` générique.** La persistance reste au firmware,
     comme le prévoyait la doc de `ledger.rs`. Le composant C `dengon_store`
     met les secrets et le curseur en NVS, et le journal (anneau de 2 × 96 Ko)
     en littlefs. `ledger::Ledger::resume` permet de reprendre depuis la seule
     ancre.
  3. **NVS non chiffré** (pas d'eFuse brûlé) : irréversible sur la carte,
     trop tôt pour un prototype.
  4. **Heure apprise d'un `ANNOUNCE` authentique** tant que celle de l'ESP32
     est antérieure à 2024 (pas de SNTP avant US-309). Sans heure, le relais
     ignore les paquets : le routeur les rejetterait tous en `ClockSkew`.
     Risque : un pair légitime qui annonce une heure future peut faire
     avancer l'horloge du relais.
  5. **Voisin identifié par le premier `ANNOUNCE` authentique reçu sur un
     lien** : le transport ne donne pas le `peerID`. Un `ANNOUNCE` relayé
     arrivé en premier lierait le lien au mauvais pair.
  6. **Pas de fragmentation ni de réassemblage côté relais** : les fragments
     sont relayés comme des paquets ordinaires, et les paquets du relais
     sont bornés à une trame (13 `msgID` par `INVENTORY`, 26 tags par offre).
  7. Remise d'enveloppe **considérée faite dès la mise en file d'émission**
     (le transport ne confirme pas la réception).
  8. `copy_budget` / `budget_after` journalisés à 0 (Spray-and-Wait = v2).
  9. `peer.connected` / `peer.disconnected` non journalisés : le `peerID` est
     inconnu à la connexion. `peer.announce_seen` en tient lieu.
  10. `msg_log_id` du relais = `SHA-256(msgID)[0..8]` : il ne voit jamais le
      `msg_uuid`, qui est chiffré.
  11. **Côté client (hors périmètre, décidé avec Paul)** : `api.rs` ne pose
      jamais `RELAY_OK` et n'émet ni `ANNOUNCE`, ni `INVENTORY`, ni
      `ENVELOPE_REQUEST`. Un téléphone ne peut donc pas encore faire relayer
      un message par ce relais. Le critère « un message traverse le relais »
      n'est démontré qu'en Rust, avec des téléphones simulés.
- **Pourquoi :** tenir la logique dans `dengon-core` (testable sous
  `cargo test`) plutôt que dans le C, et ne pas élargir US-308 à la couche
  client.
- **Conséquence :** une US client doit poser `RELAY_OK`, émettre `ANNOUNCE`
  et traiter `ENVELOPE_OFFER`. US-309 doit ajouter SNTP.

---

### 2026-09-29 — `dengon-core-embed` : racine de workspace séparée, hors du workspace principal (US-307)

- **Prévu :** `docs/synthese/08-relais-esp32.md` §3 attend `libdengon_core.a`
  comme un livrable de `crates/`, sans préciser son intégration Cargo — rien
  ne prévoyait un second workspace dans le dépôt.
- **Réel :** `dengon-core-embed` (la crate qui assemble réellement
  `libdengon_core.a`) déclare son propre `[workspace]` (vide) et est exclue
  du workspace principal (`exclude` dans le `Cargo.toml` racine). Elle a son
  propre `Cargo.lock`, sa propre toolchain (`rust-toolchain.toml` : nightly
  amont, PAS la 1.98.1 figée du reste du dépôt), et duplique à la main les
  champs de paquet et les lints qu'elle ne peut plus hériter par
  `*.workspace = true`.
- **Pourquoi :** `libdengon_core.a` est un artefact `no_std` **autonome** :
  aucun autre code Rust ne fournit l'allocateur global ni le panic handler
  dans le binaire final (un exécutable C, hôte ou ESP32). Une archive
  `staticlib` doit résoudre ces deux lang items AU MOMENT de sa compilation
  par rustc (contrainte du format), pas au moment du lien final comme un
  `rlib` normal. Or le sysroot `core`/`alloc` précompilé par rustup est
  construit avec `panic = "unwind"` (le défaut amont) : même en recompilant
  cette seule crate en `panic = "abort"`, l'éditeur de liens réclame encore
  `rust_eh_personality` (vérifié sur cible hôte `x86_64-pc-windows-gnu`,
  message exact : `undefined reference to 'rust_eh_personality'`). La seule
  façon stable de lever cette référence est de recompiler `core`/`alloc`
  eux-mêmes en `panic = "abort"` (`-Z build-std`, donc nightly) — et
  `panic`/`build-std` sont des réglages **par invocation cargo entière**, pas
  par crate : les imposer au workspace principal aurait cassé
  `cargo build --workspace` pour tout le reste (`std`, `panic = "unwind"`).
- **Essayé avant, écarté :** une feature Cargo `standalone` dans **une
  seule** crate (`dengon-core-ffi`), activant panic handler + allocateur
  seulement quand demandée. Casse dès qu'une commande unifie les features de
  tout le graphe — notamment `cargo clippy --workspace --all-targets
  --all-features`, qui active la feature à la fois pour le `rlib` (testé
  dans le harnais `std`, qui a DÉJÀ les deux) et pour le `staticlib`, d'où
  `error[E0152]: found duplicate lang item 'panic_impl'`. D'où la scission
  en deux crates : `dengon-core-ffi` (rlib normal, jamais de panic
  handler/allocateur) et `dengon-core-embed` (staticlib, `test = false`, les
  fournit en permanence, jamais mêlée à un binaire `std`).
- **Conséquence acceptée :** deux `Cargo.lock`, deux jeux de lints à tenir à
  jour en parallèle (piège documenté dans `dengon-core-embed/Cargo.toml`).
  La toolchain `esp` réelle (`espup`, cible `xtensa-esp32-none-elf`) n'est
  exercée que par le job CI `firmware` — voir
  `docs/suivi/modules/dengon-core-ffi.md`, section Tests, pour ce qui a
  (et n'a pas) pu être vérifié en local sur ce poste Windows sans `espup`.
### 2026-09-29 — Le `peerID` d'un lien n'est prouvé qu'à la fin du handshake (US-306, revue PR #111)

- **Prévu :** la doc de `parse_announce` s'appuyait sur « le handshake `XX`
  qui suit prouvera la possession de la clé » ; `on_peer_connected` liait le
  lien au `peerID` (`bind_peer`) dès la connexion.
- **Réel :** ce contrôle n'existait pas : la session était rangée sous le
  `peerID` fourni par l'appelant sans comparer `remote_static`. Un ANNOUNCE
  étant signé mais rejouable, un pair à portée pouvait le rejouer et
  terminer le `XX` avec sa propre clé sous le `peerID` d'un tiers.
  Corrigé : `Node::finish_handshake` refuse la session si
  `peer_id_of(remote_static) != peerID` ou si `remote_static` diffère de la
  clé du contact connu ; `bind_peer` n'a lieu qu'à ce moment-là.
- **Conséquence :** tests `handshake_usurpant_un_peer_id_inconnu_est_rejete`
  (échoue sans le contrôle sur le `peerID`) et
  `handshake_avec_une_autre_cle_sous_le_peer_id_d_un_tiers_est_rejete`
  (contact connu). L'anti-inondation compte par lien jusqu'à la preuve.
- **Reste ouvert (déni de service) :** `Maillage` ignore un 2ᵉ lien vers un
  pair déjà relié, donc un lien fantôme ou rejoué occupe la place jusqu'à
  son `PeerDisconnected`. Le remplacer « si la session n'est pas établie »
  demande un état de session que le `.udl` v1 n'expose pas, et un
  remplacement systématique casserait le cas légitime des deux rôles GATT.
  À traiter avec l'US-312. `pending_acks` est borné par pair mais pas en
  nombre de pairs : idem. Une conversation ouverte par `add_contact` restera
  orpheline si une US de suppression de contact ne la retire pas.

---

### 2026-09-29 — `add_contact` ouvre la conversation (US-306, trouvé sur téléphones)

- **Prévu :** `api::Node` (US-301) ne crée une conversation qu'au premier
  message envoyé ou reçu ; l'UI de messagerie (US-214) ne liste que les
  conversations existantes.
- **Réel :** `add_contact` ouvre aussi une conversation **vide** avec le
  contact (et complète le pseudo d'une conversation ouverte par un message
  reçu avant l'appairage).
- **Pourquoi :** constaté pendant l'essai sur deux téléphones : après
  l'appairage, « Aucune conversation » — **aucun moyen** d'écrire un premier
  message. Le bouchon masquait le trou avec une conversation factice.
  Correctif dans le cœur plutôt qu'un écran « nouveau message » : une ligne
  d'état, aucune UI nouvelle.
- **Conséquence :** tout contact appairé apparaît dans la liste, même sans
  message. Test `un_contact_ajoute_a_sa_conversation_vide_nommee`.

---

### 2026-09-29 — `ANNOUNCE` de lien seulement, ajouté au `.udl` v1 (US-306)

- **Prévu :** `synthese/07` §6 — à chaque lien : `ANNOUNCE` mutuels, puis
  handshake ; `synthese/05` §2 — `ANNOUNCE` périodique (`ANNOUNCE_ISOLATED_S`
  = 4 s, `ANNOUNCE_CONNECTED_S` = 15–30 s), TTL faible (2–3), traité par le
  pipeline de réception du nœud.
- **Réel :** l'`ANNOUNCE` n'est émis **qu'une fois**, en première trame de
  chaque lien, avec **TTL 1**, et il est lu par l'**app** (`Maillage`, via
  `identity_from_announce`) pour relier le `LinkId` au `peerID` — pas par
  `Node::on_bytes_received`, qui l'ignore toujours. `ledger_height` est
  renseigné, `caps` vaut 0. Deux appels ajoutés au `.udl` v1 :
  `DengonNode.announce_frame()` et `identity_from_announce(frame)`.
- **Pourquoi :** le seul besoin de l'US-306 est de savoir qui est au bout du
  lien ; `api::Node` identifie un lien par `PeerId` (doc de module US-301).
  L'annonce périodique sert la découverte à plusieurs sauts, hors scénario 1.
  Alternative écartée (avec Paul) : une trame « hello » Kotlin non signée.
- **Conséquence :** **à annoncer en point d'équipe** (même règle que
  l'extension v1 de l'US-302). Le relais (US-308/US-312) devra lire le même
  `ANNOUNCE` en tête de lien.

---

### 2026-09-29 — Accusés de réception : en session uniquement (US-306)

- **Prévu :** `synthese/07` §4 — à réception : `Ack{msg_uuid, status=2}`
  envoyé en session si possible, **sinon en enveloppe** vers l'expéditeur ;
  l'`Ack` est soumis au store-and-forward.
- **Réel :** l'`Ack` part **dans la session Noise** avec l'auteur (paquet
  `ACK`). Sans session (message reçu par enveloppe avant la fin du
  handshake), il attend dans `pending_acks` (64 max par pair, le plus ancien
  oublié) et part dès que la session s'établit. Pas d'accusé par enveloppe.
- **Pourquoi :** sceller une enveloppe demande un `rng`, absent du chemin de
  réception (`on_bytes_received`) ; changer sa signature touchait tout le
  FFI. Pour le scénario 1 (deux téléphones à portée), la session existe
  toujours.
- **Conséquence :** un auteur jamais connecté **en direct** au destinataire
  (message passé par un relais, scénarios 2 et 3, US-312) ne recevra pas son
  accusé : reste `InFlight`. À reprendre avec l'US-312.

---

### 2026-09-29 — L'UI reste seule lectrice de `pollEvents` (US-306)

- **Prévu :** `docs/suivi/modules/android-app.md` (US-302) : le nœud est
  « partagé par l'UI et, à l'US-306, par le service de premier plan ».
- **Réel :** le service (`TransportActif` → `Maillage`) appelle
  `onPeerConnected` / `onBytesReceived` / `takeOutgoing`, mais **jamais**
  `pollEvents`. Écran fermé, les événements s'accumulent dans le nœud ; ils
  sont lus au retour de l'UI. L'expiration des messages (faite dans
  `poll_events`) ne tourne donc que quand l'UI est ouverte.
- **Pourquoi :** deux lecteurs se partageraient les événements (chacun n'en
  verrait qu'une partie). Le `Maillage` n'en a pas besoin.
- **Conséquence :** pas de notification « message reçu » app fermée (hors
  critères d'acceptation) ; à revoir si on en veut une (flux partagé
  alimenté par un seul lecteur).

---

### 2026-09-29 — Contrat FFI étendu en v1 : `open` + coffre, chemin des octets radio (US-302)

- **Prévu :** US-302 — « bindings UniFFI générés depuis le `.udl` de US-106,
  **sans modifier ce `.udl`** (ou l'écart est consigné et annoncé) ». Le `.udl`
  v0 était gelé : toute évolution passe par une réunion.
- **Réel :** le `.udl` est **étendu** (v1) :
  - `constructor(Identity)` → `[Name=open] constructor(data_dir, vault_key, pseudo)` ;
  - ajouts : `local_identity`, `add_contact`, `on_peer_disconnected`,
    `on_bytes_received`, `take_outgoing` (+ `dictionary OutgoingFrame`) ;
  - `on_peer_connected` et les fonctions libres `generate_identity`,
    `identity_qr_code`, `verification_code` deviennent `[Throws=DengonError]`.
  Les types v0 (`Identity`, `Message`, `Conversation`, `NodeEvent`,
  `MessageStatus`, `DengonError`) sont inchangés.
- **Pourquoi :** le vrai nœud (`dengon_core::api::Node`, US-301) a besoin de
  ses clés **privées**, que la carte `Identity` du v0 ne porte pas, et d'un
  chemin pour les octets radio : le nœud ne possède aucun transport, c'est
  l'app qui pousse et tire les trames. La doc de l'US-301 annonçait déjà ces
  deux manques. Les `[Throws]` ajoutés : une carte venue de Kotlin peut être
  invalide, ce que le bouchon ne vérifiait pas. Alternative écartée : laisser
  le `.udl` intact et exposer le reste par `#[uniffi::export]` — le contrat
  aurait été éclaté sur deux sources.
- **Conséquence :** **à annoncer en point d'équipe** (règle du gel US-106).
  Côté Kotlin, aucun appelant de l'UI n'a changé (exceptions non vérifiées) ;
  `DengonNode(identity)` n'existe plus, remplacé par `DengonNode.open(...)`.

---

### 2026-09-29 — FFI : messages et contacts non persistés (US-302)

- **Prévu :** `docs/synthese/04-architecture.md` — le nœud persiste messages
  et contacts (`store`, US-207).
- **Réel :** seule l'**identité** est persistée (coffre `identity.vault`).
  `api::Node::attach_store` n'est pas appelé : conversations, messages et
  contacts vivent le temps du processus. `Read` et `Cancelled` ne sont
  jamais émis (pas de `mark_read` / `cancel_message` dans la façade).
- **Pourquoi :** hors critères d'acceptation de l'US-302 ; brancher `store`
  demande une clé de base (autre que celle du coffre) et une relecture des
  contacts au démarrage, qui touchent au périmètre de l'US-306.
- **Conséquence :** après un redémarrage de l'app, il faut refaire
  l'appairage QR pour pouvoir écrire à un contact.

---

### 2026-09-29 — ABI Android limitées à arm64-v8a + x86_64 ; tests FFI ignorés sous Windows (US-302)

- **Prévu :** US-302 — « build de la bibliothèque native pour les ABI Android
  ciblées », sans liste.
- **Réel :** `arm64-v8a` (Pixel 8 Pro, Galaxy A16) et `x86_64` (émulateur).
  `abiFilters` les impose dans l'APK. Les tests JVM qui passent par le FFI
  sont **ignorés** (`Assume`) quand la lib hôte manque : c'est le cas dans
  Android Studio sous Windows, où le dépôt est ouvert depuis WSL.
- **Pourquoi :** pas d'appareil armv7 dans l'équipe. Sans `abiFilters`, l'AAR
  de JNA ajoutait armv7/x86/mips et un téléphone armv7 aurait planté au
  premier appel. Côté tests, construire une `dengon_ffi.dll` Windows n'apporte
  rien que la CI ne vérifie déjà.
- **Conséquence :** le job CI `android` est la référence et échoue si le test
  d'intégration a été ignoré ; en local, lancer Gradle depuis WSL après
  `android/scripts/build-ffi.sh`.

---

### 2026-09-28 — `cross-vectors` : la patte « firmware » est un proxy `no_std`, pas le firmware (US-222)

- **Prévu :** `docs/synthese/10-benchmarks-mvp-tests.md` §4.7 — le job
  `cross-vectors` compare « core ↔ firmware ↔ dashboard » sur les mêmes
  vecteurs de conformité. DoD §7.2, ligne *Firmware* : « vecteurs de
  conformité identiques au core ».
- **Réel :** le firmware ne lit **aucun** vecteur. `firmware/dengon-relay/`
  ne contient que le BLE (advertising, table GATT) ; le pont
  `dengon_core_ffi` qui lui donnerait un décodeur est l'US-307, non livrée.
  La troisième lecture est donc faite par `crates/dengon-conformance/`, une
  crate sans code dont la seule fonction est de lier `dengon-core` en
  `default-features = false` — la configuration `no_std` + `alloc` que
  l'ESP32 embarquera — et de rejouer les mêmes
  `contracts/packet/vectors_v0.json` contre ce build.
- **Pourquoi :** c'est ce qui s'approche le plus de la promesse « mêmes
  octets in → mêmes décisions out » sans attendre l'US-307, qui est au
  sprint 3. Le décodeur exercé est **le même code** que celui que le
  firmware liera ; ce qui manque, c'est la traversée du FFI et l'exécution
  sur la cible.
- **Détail technique qui a dicté la forme :** `cargo test -p dengon-core
  --no-default-features` ne donne **pas** un build `no_std`. La
  dev-dependency `dengon-ble` dépend de `dengon-core` avec ses features par
  défaut, et l'unification du résolveur v2 réactive `std` dans le même
  graphe — visible avec `cargo tree -p dengon-core --no-default-features -e
  features | grep rusqlite`. Un paquet séparé est le seul moyen d'obtenir
  la bonne résolution. Deux conséquences :
  - la dépendance y est écrite **en chemin**, pas en `{ workspace = true }` :
    avec l'héritage de workspace, `default-features = false` est ignoré tant
    que `[workspace.dependencies]` ne le déclare pas, et le déclarer là-bas
    priverait les autres crates de `std` ;
  - aucune assertion « je suis sans `std` » n'est compilée dans la crate :
    un build `--workspace` (celui de `core.yml`) unifie les features et
    ferait échouer l'assertion à tort. Le garde-fou est une étape du job
    `cross-vectors`, qui inspecte la résolution **isolée** :
    `cargo tree -p dengon-conformance -e features | grep rusqlite`.
- **Conséquence :** à lever par l'**US-307**. Le jour où `dengon_core_ffi`
  est branché, ajouter au job une étape qui fait tourner les vecteurs
  **sur la cible** (tests Unity, ou host-tests de `libdengon_core`) ; le
  filtre de chemins de `cross-vectors.yml` couvre déjà `firmware/**`.
  `crates/dengon-conformance/` garde alors sa valeur propre : il prouve la
  compilation `no_std` sans matériel.

---

### 2026-09-28 — CLÔTURE : les vecteurs de conformité ont rejoint `contracts/packet/` (US-222)

- **Écart clos :** celui du 10/09, « Vecteurs de conformité v0 dans
  `crates/dengon-core/tests/`, pas `contracts/packet/` (US-108) ». Il était
  motivé par le fait que `contracts/` n'était pas encore sur `main`
  (PR #60 en vol) et par le conflit prévisible sur `pyproject.toml` /
  `uv.lock` / `contracts.yml`.
- **Fait :** `#60` est mergée depuis. `vectors_v0.json`, `crypto_v0.json`
  et `identity_v0.json` sont désormais dans `contracts/packet/`, avec leur
  `README.md`. Les trois tests Rust pointent l'emplacement final, le
  `validate_packets.py` annoncé existe, et il est branché dans
  `contracts.yml` **et** dans `cross-vectors.yml`.
- **Reste :** le filtre de chemins de `core.yml` a dû être élargi à
  `contracts/packet/**` et `contracts/events/**` — les tests Rust lisent ces
  fichiers, donc les modifier doit déclencher `core`. Sans ça, un vecteur
  cassé n'aurait plus fait rougir `core` sur une PR qui ne touche que
  `contracts/`.

---

### 2026-09-28 — `audit` : deux avis RUSTSEC acceptés, tenus par l'épinglage d'uniffi (US-222)

- **Prévu :** `docs/synthese/10` §4.7 — job `audit` = `cargo audit`,
  `cargo deny check`, **SBOM**, sur PR et en quotidien.
- **Réel :** `cargo audit` et `cargo deny` sont livrés et **bloquants**.
  Le **SBOM ne l'est pas** : il n'apparaît dans aucun critère d'acceptation
  de l'US-222, et le livrer à la va-vite en fin de sprint aurait donné un
  artefact que personne ne consomme. À ouvrir en issue de suite.
- **Deux exceptions consignées dans `deny.toml`**, la seule soupape prévue :
  `RUSTSEC-2024-0436` (`paste` non maintenu) et `RUSTSEC-2025-0141`
  (`bincode` 1.3.3 non maintenu). Ni l'une ni l'autre n'est une
  vulnérabilité : ce sont deux dépendances de **macros de compilation**,
  arrivées en transitif par `uniffi 0.28.3`, épinglé à l'exact `=0.28.3`
  parce que les bindings Kotlin générés doivent correspondre au runtime de
  l'APK. Les lever demande une montée d'uniffi et une régénération des
  bindings Android — hors périmètre, et inopportun au milieu du sprint
  Android. À rouvrir à la prochaine montée d'uniffi.
- **`multiple-versions = "warn"`, pas `"deny"` :** deux versions de `syn`
  cohabitent aujourd'hui, en transitif, et rien dans notre code ne peut le
  corriger. Bloquer là-dessus aurait rendu le job inutilisable dès le
  premier jour.
- **`[licenses.private].ignore = true` :** nos sept crates sont
  `publish = false` et n'ont pas de champ `license`, la licence du projet
  n'étant pas tranchée (`synthese/01-sujets-a-trancher.md`). Sans cette
  ligne, cargo-deny les compterait comme *unlicensed* et échouerait sur
  notre propre code.

---

### 2026-09-28 — Transport NimBLE (US-220) : pas de fragmentation BLE, `PeerConnected` à l'abonnement, quota borné, anti-boucle sur 4 octets

- **Prévu :** `docs/synthese/04-architecture.md` §3 et `docs/powl/03` §6.2 :
  « `Transport::send` gère le découpage **BLE** (MTU) de façon transparente » ;
  la rustdoc de `TransportEvent::FrameReceived` parle de fragmentation BLE
  « déjà réassemblée par l'implémentation ». `TransportConfig::default()` :
  8 liens. `docs/powl/03` §6.1 : « celui dont le **`peerID`** est le plus petit
  initie ». `PeerConnected` = « un lien est établi et utilisable », sans plus.
- **Réel :**
  1. **1 trame = 1 PDU ATT.** `send` au-delà de `ATT_MTU - 3` rend
     `FRAME_TOO_LARGE { max: mtu - 3 }` ; `broadcast` refuse au-delà de 514 et
     saute les liens au MTU trop petit. Aucune fragmentation BLE dans le
     transport.
  2. **`PeerConnected` émis quand le lien marche dans les deux sens** : côté
     central après échange MTU + découverte + abonnement au `CHAR_TX` du
     pair ; côté périphérique à l'abonnement du pair. Si le pair écrit avant
     de s'abonner, `PeerConnected` est émis juste avant sa première trame. Un
     lien jamais annoncé qui tombe ne produit **aucun** `PeerDisconnected`.
  3. **`max_connections` borné** à `CONFIG_BT_NIMBLE_MAX_CONNECTIONS` (3) avec
     un avertissement, au lieu d'échouer ; quota plein = annonce et scan
     suspendus, connexion entrante excédentaire fermée sans être annoncée.
  4. **Anti-boucle sur `peerID[0..4]`** (seuls 4 octets sont dans l'annonce),
     égalité départagée par l'adresse BLE.
  5. **Motifs** : HCI `0x13`/`0x14`/`0x15` → `Propre`, `0x16` → `Locale`, tout
     le reste (dont `0x08` supervision timeout) → `Brutale`.
- **Raison :** (1) `protocol::fragment` (US-202) découpe déjà à `ATT_MTU - 3`
  (`chunk_capacity`) : une seconde fragmentation dans le transport serait
  morte, et imposerait un format de trame BLE à partager avec Android. En
  prime, la règle n°3 de la déconnexion brutale (jeter les trames partielles)
  est tenue par construction. (2) Émettre `PeerConnected` à la connexion GAP
  laisserait le cœur envoyer son `ANNOUNCE` avant que le pair soit abonné :
  perdu. (3) Le contrôleur de l'ESP32 est configuré à 3 liens (US-114) ; un
  relais qui refuse de démarrer avec la configuration par défaut du contrat
  serait pire. (4) C'est tout ce que l'annonce transporte. (5) Seul ce que le
  pair *annonce* (`LL_TERMINATE_IND`) est propre.
- **Conséquences :** l'appelant **doit** fragmenter (c'est le cas de
  `dengon-core`). L'implémentation Android (US-213) doit faire les mêmes choix
  (1) et (2) pour que les deux moitiés du maillage se comprennent. Les écarts
  d'US-114 sur le manufacturer data (Company ID `0xFFFF` à sauter, bitfield
  `flags`) sont maintenant **lus** par du code (`dengon_adv_parse_mfg`) et
  vérifiés par des tests Unity : ils passent de « contrat de fait non vérifié »
  à « contrat vérifié côté firmware ».
- **Doc de conception mise à jour ?** non. À remonter dans `docs/powl/03` §6
  (point 1, format du manufacturer data) et dans la rustdoc de
  `TransportEvent::FrameReceived` (point 1) — changement de contrat US-105 à
  proposer en point d'équipe, pas à faire seul.

---

### 2026-09-28 — `api` (US-301) : construction, extensions hors `.udl`, périmètre réduit

- **Prévu :** `.udl` v0 gelé (US-106) : `DengonNode` construit à partir du
  dictionnaire `Identity` (sans secrets), `send_message`/`poll_events`/
  `on_peer_connected`/`list_conversations`/`list_messages` seules méthodes,
  quatre variantes de `NodeEvent`, `DengonError` à 3 variantes.
- **Réel :**
  1. `Node::new` prend `identity::Identity` (avec secrets) : le `.udl` n'a
     pas de méthode pour fournir des secrets, et l'en-tête du `.udl` dit
     lui-même que la construction est différée à cette US.
  2. Deux méthodes Rust hors `.udl` : `on_bytes_received(from, bytes, now)`
     et `take_outgoing() -> Vec<(PeerId, Vec<u8>)>`, plus
     `on_peer_disconnected` (le `.udl` v0 ne modélise pas la déconnexion).
  3. Messagerie en session (`Noise XX`) câblée de bout en bout ; messagerie
     par enveloppe (`Noise X`) câblée seulement pour « on est déjà connecté à
     qui on écrit » (avant même la fin du handshake `XX`, l'enveloppe n'en
     dépend pas) ; `ENVELOPE_OFFER`/`ENVELOPE_REQUEST` avec un porteur tiers
     pas câblés.
  4. `sync::courier` câblé en réception pour un pair tiers (dépôt), pas en
     remise de ce qu'on porte à son propriétaire.
  5. `sync::inventory` (US-210) pas câblé du tout.
  6. Aucun accusé de réception émis (seulement reçu/traité côté
     `apply_ack`) : `MessageStatus::Delivered` n'est jamais atteint par
     cette façade, `InFlight` est le statut final observable.
  7. Module `api` entier `#[cfg(feature = "std")]`.
  8. `PeerId` utilisé directement comme identifiant de lien pour
     `sync::routing::Router`, au lieu de `dengon-ble::LinkId`.
- **Raison :** (1) le `.udl`'s `Identity` sans secrets rendrait la
  construction impossible autrement — écart voulu, pas subi ; (2)
  `dengon-core` n'a aucune dépendance non-test à `dengon-ble::Transport`,
  donc `Node` ne peut pas piloter un transport lui-même : l'appelant (futur
  `dengon-ffi`/`dengon-node`) doit lui donner la main ; (3)/(4)/(5)/(6)
  garder le périmètre de cette US tractable — chacun de ces points est soit
  bloqué par une autre PR pas encore mergée (5), soit plus proche du rôle
  d'un relais dédié (US-308) que d'une façade client (3, 4), soit
  simplement pas encore fait (6) ; (7) aucun consommateur `no_std` de cette
  façade n'existe (le firmware câble `sync::*` directement) ; (8) un nœud n'a
  qu'une connexion active par pair dans cette implémentation.
- **Impact :** un message envoyé par enveloppe à un pair jamais rencontré
  directement (uniquement via un porteur) n'arrive pas — acceptable pour
  cette US, à lever quand le porteur tiers sera câblé. Le statut d'un
  message sortant plafonne à `InFlight`, jamais `Delivered`, tant qu'aucune
  façade n'émet d'accusé de réception.
- **À surveiller :** si `sync::inventory` (US-210, PR #96) est mergée avant
  la clôture du sprint, envisager de la câbler dans cette façade dans une US
  de suivi plutôt que d'attendre US-302.

---

### 2026-09-28 — `sync::courier` (US-212) : échéance bornée par l'origine, remise en deux temps, test négatif sur AEAD de substitution

- **Prévu :** `docs/synthese/07-cycle-de-vie-et-statuts.md` §7 : une
  enveloppe expire à `deposit_ms + MSG_TTL_S` (et budget de copies = 0, v2) ;
  `synthese/05` §6.3 : le porteur « renvoie le `SEALED_ENVELOPE` » sur
  `ENVELOPE_REQUEST` ; `synthese/05` §6.4 et `synthese/08` §7 : éviction
  « LRU / plus ancien » **ou** « refus des nouvelles, existantes
  protégées » (les deux docs divergent).
- **Réel :**
  1. Échéance = `min(timestamp_ms + TTL, deposit_ms + TTL)`.
  2. La remise se fait en deux temps : `matching` (lecture) puis
     `confirm_handoff` (retrait) après envoi réussi.
  3. Politique d'éviction = réglage `EvictionPolicy`, `RejectNew` par défaut.
  4. Pas de `copy_budget` (colonne `held_envelopes.copy_budget`) : v2 (A-13).
  5. Le test négatif « le courrier ne peut pas déchiffrer » simule le
     scellement avec XChaCha20-Poly1305, Noise `X` (US-204, PR #81) n'étant
     pas sur `main`.
- **Raison :** (1) avec le seul `deposit_ms`, une enveloppe re-déposée de
  courrier en courrier vivrait indéfiniment ; (2) un lien BLE qui tombe
  pendant l'envoi ne doit pas perdre l'enveloppe ; (3) rendre visible le
  désaccord entre `synthese/05` et `synthese/08` ; (5) ne pas bloquer l'US
  sur une PR non mergée — la propriété testée (le courrier ne reçoit aucune
  clé, rend le ciphertext intact, ne détient aucun octet du clair) ne
  dépend pas de l'AEAD.
- **Conséquences :** après le merge d'US-204, remplacer l'AEAD du test par
  un vrai scellement Noise `X`. L'équipe doit trancher la politique
  d'éviction par défaut (téléphone vs ESP32).
- **Doc de conception mise à jour ?** non.
### [date] — [titre court de l'écart]

- **Prévu :** ce que dit `docs/powl/NN-....md` (référence précise).
- **Réel :** ce qui est codé.
- **Raison :** pourquoi.
- **Conséquences :** impact sur le reste (autres modules, sécurité, perfs, planning).
- **Doc de conception mise à jour ?** oui / non (+ lien).

---

_(aucun écart pour l'instant)_

---

### 2026-09-28 — `AndroidTransport` (US-213) : détection de déconnexion brutale asymétrique selon le rôle GATT

- **Prévu :** `crates/dengon-ble/src/transport.rs`, rustdoc de `Transport`,
  règle 1 : « émettre exactement un `PeerDisconnected` avec
  `DisconnectReason::Brutale`, au plus tard au *supervision timeout* BLE »,
  sans distinction de rôle (central/périphérique).
- **Réel :** sur appareil réel, en provoquant une vraie perte radio
  (éloignement physique, pas un `disconnect()` logiciel), le **même**
  événement de coupure est rapporté différemment selon le rôle GATT du
  nœud sur ce lien :
  - côté **central** (`BluetoothGattCallback.onConnectionStateChange`) :
    code de statut HCI exploitable, correctement traduit en `BRUTALE` par
    `motifDeconnexion` — confirmé (Samsung Galaxy A16, 2026-09-28).
  - côté **périphérique** (`BluetoothGattServerCallback
    .onConnectionStateChange`) : Android rend quasi systématiquement
    `status=0`, quelle que soit la cause réelle de la coupure — le même
    événement, vu du Pixel 8 Pro (périphérique sur ce lien), a été rapporté
    `PROPRE`.
- **Raison :** limitation documentée de l'API Android (le rappel serveur ne
  reçoit pas les codes HCI détaillés que reçoit le rappel client) — pas un
  bug de `motifDeconnexion`, dont la logique de traduction status→motif est
  correcte pour les deux rôles et a été écrite en anticipant ce cas (voir
  le commentaire du code, confirmé par cet essai).
- **Conséquences :** un nœud Android **ne peut pas garantir de façon fiable**
  la règle 1 du contrat quand il joue le rôle périphérique sur un lien
  donné — seul le rôle central le peut. Comme la règle anti-boucle de
  connexion (`Annonce.doitInitier`) fait déjà qu'un seul des deux nœuds
  initie (donc est central), la moitié des liens d'un nœud donné n'auront
  pas de détection fiable de coupure brutale. Impact pour `sync` (US-209) :
  ne pas se fier uniquement à `DisconnectReason` pour décider de retenter —
  un timeout applicatif (pas de trafic depuis N secondes) reste nécessaire
  en complément, y compris pour capturer les cas mal classés `PROPRE`.
- **Doc de conception mise à jour ?** non — c'est une contrainte de
  plateforme, pas un choix de conception à documenter dans `synthese/`.

---

### 2026-09-28 — `GattRadio` (US-213) : dédup de connexion par adresse MAC, pas par identité de nœud

- **Prévu :** `crates/dengon-ble/src/transport.rs`, rustdoc de `LinkId` :
  « un `LinkId` ne doit jamais être réutilisé pour un autre pair », et
  implicitement, un nœud physique ne devrait porter qu'un lien actif à la
  fois (l'esprit de la règle anti-boucle de `synthese/05` §6, reprise dans
  `TransportConfig::local_peer_id`).
- **Réel :** observé une fois en test réel (session écran éteint,
  2026-09-28) : un **second** lien GATT s'est ouvert entre les deux mêmes
  téléphones déjà connectés et actifs (Pixel : `link#2` + `link#3` tous deux
  vivants ; Samsung : `link#1` + `link#2`), les deux liens relayant le même
  battement en double. `GattRadio.rappelScan` déduplique les connexions
  entrantes par adresse BLE (`RadioPeer.adresse`) ; l'hypothèse la plus
  probable est qu'Android a fait tourner l'adresse privée résolvable
  annoncée par le pair entre deux scans, et que le pair est alors apparu
  comme un « nouvel » appareil sous cette nouvelle adresse.
- **Raison :** `Transport` ne connaît **par contrat** que des identifiants
  de lien locaux et une adresse radio — jamais un `peerID` cryptographique
  (rustdoc du contrat : « il ne route pas », « il ne fait pas de crypto »).
  Il ne peut donc pas, par construction, savoir que deux adresses
  différentes désignent le même nœud : c'est le rôle d'`ANNOUNCE` et de la
  couche `sync` (US-209/210), qui verront le même `peerID` sur les deux
  liens et pourront fermer le doublon.
- **Conséquences :** aucune donnée corrompue ni crash — juste un lien
  redondant (trafic et batterie gaspillés tant qu'il n'est pas fermé).
  `sync::routing` devra fermer explicitement un lien dont le `peerID`
  annoncé fait déjà l'objet d'un autre lien actif ; ce n'est pas fait
  aujourd'hui (aucun code `sync` n'existe encore côté Android). À garder en
  tête pour US-213 → US-306 (branchement réel) et pour `sync::routing`.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-28 — `sync::status` (US-211) : deux transitions en plus, `msg.cancelled` hors catalogue, `RESEND_MAX` local, outbox en clé → octets

- **Prévu :** `docs/synthese/07-cycle-de-vie-et-statuts.md` §2 : `DELIVERED`
  et `EXPIRED` ne s'atteignent que depuis `IN_FLIGHT` ; §1 : événement
  `msg.cancelled` ; §3 : `RESEND_MAX = 8` ; `synthese/09` : table `outbox` en
  colonnes SQL.
- **Réel :**
  1. `QUEUED → EXPIRED` accepté : un message jamais remis pendant 24 h
     expire aussi (la règle du §7 ne distingue pas le statut).
  2. `QUEUED → DELIVERED` accepté : si l'appareil s'arrête entre l'envoi
     radio et l'écriture de `IN_FLIGHT`, l'Ack peut arriver sur un message
     resté `QUEUED` ; l'ignorer perdrait une remise réelle. La monotonie
     reste garantie (le rang ne fait que monter).
  3. `Status::Cancelled.event_name()` = `msg.cancelled`, mais cet événement
     **n'est pas** dans le catalogue VPS (`contracts/events/payloads.schema.json`) :
     journal local uniquement.
  4. `RESEND_MAX` est défini dans `sync::status`, pas dans `protocol::consts`
     (règle locale d'outbox, jamais échangée sur le fil).
  5. L'outbox est persistée via un trait `OutboxStore` clé `msg_uuid` →
     enregistrement binaire versionné (`OutboxRecord::encode`), pas en
     colonnes. Mêmes champs que `synthese/09`, plus `status`/`status_ms` et
     un compteur de remises **par pair** (au lieu d'un `attempts` unique).
- **Raison :** (1)(2) robustesse aux arrêts brutaux ; (3) l'annulation est
  une action locale sans intérêt pour le dashboard ; (4) pas un changement de
  contrat ; (5) un seul trait pour SQLite (Android/PC) et NVS (ESP32), et le
  plafond `RESEND_MAX` est défini par pair dans `synthese/07` §3.
- **Conséquences :** US-207 (`store`) devra fournir une implémentation
  d'`OutboxStore` (une table `outbox(msg_uuid BLOB PRIMARY KEY, record
  BLOB)` suffit). Si le dashboard veut `msg.cancelled`, l'ajouter au
  catalogue (US-107 / `contracts/events`).
### 2026-09-28 — `protocol::fragment` (US-202) : MTU minimal 46, budget mémoire global, mémoire des `frag_id` terminés

- **Prévu :** `docs/synthese/05-protocole-et-trame.md` §5 : fragments de
  `FRAG_SIZE` au plus, réassemblage borné par `FRAG_TIMEOUT_S` et
  `FRAG_MAX_CONCURRENT` ; §2 : `ATT_MTU_MIN = 23` comme repli.
- **Réel :**
  1. La taille de chunk dépend du MTU : `min(FRAG_SIZE, ATT_MTU − 3 − 30 −
     12)`. Au MTU minimal BLE (23 → 20 octets utiles), **même l'en-tête L3
     (30 o) ne tient pas** : aucun paquet dengon ne passe. Le plus petit MTU
     utilisable est 46. Sans effet pratique (Spike C : 517 négocié), mais le
     « repli sur 23 » de `synthese/05` §2 n'est pas viable.
  2. Budget mémoire **global** en octets (`max_bytes`, 128 Kio par défaut,
     avec un coût forfaitaire de 32 o par chunk) en plus des deux bornes
     prévues : sans lui, 64 réassemblages × `PACKET_MAX_LEN` ≈ 4 Mio, hors
     de portée d'un ESP32.
  3. Le réassembleur retient les `frag_id` terminés (64 au plus, pendant
     `timeout_ms`) pour ignorer leurs fragments en retard : sans cela, un
     paquet ressortait deux fois et un doublon tardif rouvrait un
     réassemblage inutile.
  4. Seul le **payload** du fragment est produit ; l'en-tête L3 `0x09` est
     laissé au codec (US-201).
- **Raison :** (1) arithmétique du format ; (2)(3) critère « mémoire bornée »
  de l'US ; (4) codec pas encore mergé, et indépendant.
- **Conséquences :** `dengon-ble` / l'appli Android doivent refuser (ou
  signaler) un lien dont le MTU négocié est < 46. La déduplication de paquets
  reste le rôle du seen-set de `sync::routing` ; celle du réassembleur ne
  vaut que pour `timeout_ms`.
### 2026-09-28 — Appairage (US-215) : contact vérifié en mémoire, identité provisoire, code placeholder

- **Prévu :** `powl/04` §2.3 : « Match → contact marqué ✔ vérifié (stocké
  dans `contacts.verified_at`) » ; code = `SHA-512(min(fpA,fpB) ‖ max(fpA,fpB))`,
  12 groupes de `u16 mod 100000`.
- **Réel :**
  1. Le contrat FFI v0 (US-106) n'a pas d'appel « marquer vérifié » : les
     contacts vérifiés vivent dans `AppairageViewModel` (perdus au
     redémarrage de l'app).
  2. Identité locale provisoire : `generateIdentity("tel-xxxx")` du
     bouchon, pseudo aléatoire par installation. Le bouchon dérivant le
     `peerId` des 8 premiers octets du pseudo, l'aléa doit y tenir.
  3. Le code affiché est celui du bouchon (FNV-1a) : forme conforme (12 × 5
     chiffres, identique des deux côtés, ordre-indépendant), calcul non
     conforme — comme prévu par US-106.
- **Raison :** l'US est explicitement « sur bouchon FFI » ; le branchement
  réel est l'US-306.
- **Conséquences :** US-302/US-306 devront ajouter au FFI un appel
  « marquer vérifié » (et la persistance `contacts.verified_at`), et
  remplacer `IdentiteLocale` par la vraie identité (US-205).
### 2026-09-28 — `dengon-verify` (US-305) : export binaire, pas de `LOG_ATTEST` ; écart « export partiel » d'US-206 résolu

- **Prévu :** `synthese/09` §11 : entrée de journal hachée sous forme de
  **JSON canonique** (`entry_hash = SHA-256(json_utf8)`) ; `dengon-verify`
  (doc de module d'US-104) « lit un export de journal accompagné de son
  `LOG_ATTEST` ».
- **Réel :**
  1. L'export lu est la suite binaire des `Entry::to_bytes` de `ledger`, et
     `entry_hash` est celui que `ledger` calcule réellement (encodage
     binaire à longueurs préfixées, US-206) — pas le JSON canonique de
     `synthese/09` §11. Le binaire suit le code, seule source de vérité (B-5).
  2. Pas de `LOG_ATTEST` : ce paquet n'a pas encore de format côté
     `protocol` ni de producteur.
  3. **Écart US-206 « `verify_chain()` ne peut pas re-vérifier un export
     partiel » résolu** : `ledger::Anchor` + `verify_entries(entries,
     anchor)` ; `dengon-verify --from-seq N --prev-hash HEX`.
- **Raison :** (1) aligner le vérificateur sur ce que le journal produit
  vraiment ; (2) rien à consommer ; (3) le dashboard reçoit des tranches.
- **Conséquences :** le dashboard (US-310) devra transmettre l'export
  binaire (ou le reconstruire depuis les colonnes `seq`, `ts_ms`,
  `event_name`, `payload_json`, `prev_hash`, `entry_hash`, `sig`). L'écart
  JSON canonique vs binaire entre `synthese/09` §11 et `ledger` reste à
  trancher en équipe.

### 2026-09-28 — `GET /api/integrity` (US-310) : extension additive d'`envelope.schema.json`, pas de transmission de l'export binaire

- **Prévu :** aucune conception écrite ne tranchait ce point (l'entrée
  `dengon-verify` ci-dessus le laissait explicitement « à trancher en
  équipe ») ; deux options étaient sur la table : (a) transmettre l'export
  binaire brut sur un nouveau canal, ou (b) enrichir `/ingest/batch` /
  `envelope.schema.json` pour que le dashboard reconstruise l'export.
- **Réel :** option (b) retenue (choix utilisateur explicite, pas une US
  écrite). `envelope.schema.json` gagne deux champs **optionnels** par
  événement : `entry_hash` (hex 64) et `sig` (base64 64 octets) ; `prev_hash`
  (déjà présent, optionnel) est redéfini pour porter la sémantique de
  `ledger::Entry.prev_hash` plutôt que « `event_id` de l'événement
  précédent ». Absence de ces champs → événement exclu de la vérification
  d'intégrité (rétro-compatible avec les producteurs existants qui ne les
  envoient pas). Côté `dashboard/api`, `app/integrity.py::_build_export`
  reconstruit la suite binaire `Entry::to_bytes` à partir des colonnes
  SQLite (`seq`, `ts_ms`, `name`, `payload`, `prev_hash`, `entry_hash`,
  `sig`) et l'envoie à `dengon-verify` en sous-processus, plutôt que de
  transmettre/stocker l'export binaire tel quel.
- **Raison :** additif (pas de rupture de compatibilité sur un contrat déjà
  utilisé par plusieurs producteurs potentiels) ; `dengon-verify` reste
  inchangé (aucune US ouverte dessus) ; le dashboard connaît déjà tous les
  octets nécessaires à la reconstruction (US-206 : `to_bytes()` est un
  encodage déterministe et sans perte des mêmes champs).
- **Conséquences / limites :**
  1. **Reconstruction du `payload_json`** : `_build_export` suppose que le
     JSON stocké en base correspond octet pour octet au JSON canonique
     signé à l'origine (`Value::to_canonical_bytes()` côté Rust,
     `canonical_json()` côté Python — vérifié identique pour tous les
     appels `ledger.append()` de `dengon-core` actuels). Non vérifié contre
     un producteur réel (Android/firmware encore des bouchons) : un
     producteur qui sérialiserait le payload différemment (ordre de clés,
     espaces) casserait la vérification côté dashboard sans que la chaîne
     soit réellement altérée.
  2. **Détection de fork non démontrable via l'ingestion normale** :
     `event_id = SHA-256(node_id ‖ seq)` est déterministe ; deux entrées en
     conflit sur le même `(node_id, seq)` produisent le même `event_id`,
     donc la seconde est silencieusement ignorée par `INSERT OR IGNORE`
     avant d'atteindre la vérification de chaîne. Le verdict `fork` existe
     et est testé (insertion SQL directe dans `test_integrity.py`,
     contournant `/ingest/batch`), mais un vrai relais malveillant/fourché
     ne peut pas le déclencher par le flux d'ingestion actuel — limitation
     structurelle du schéma `events` (clé primaire `event_id`), pas un bug
     de `integrity.py`.
  3. **Binaire `dengon-verify` absent de l'image Docker** : `dashboard/api/Dockerfile`
     (US-224) ne construit/n'embarque pas ce binaire Rust ; sur le VPS réel
     tel que déployé aujourd'hui, `GET /api/integrity` répondrait `503`
     (`DENGON_VERIFY_BIN` introuvable dans le `PATH` du conteneur). La CI
     (`.github/workflows/dashboard.yml`) le compile pour les tests, mais
     rien ne le fait pour l'image de production.

### 2026-09-28 — Appairage (US-215) : contact vérifié en mémoire, identité provisoire, code placeholder

- **Prévu :** `powl/04` §2.3 : « Match → contact marqué ✔ vérifié (stocké
  dans `contacts.verified_at`) » ; code = `SHA-512(min(fpA,fpB) ‖ max(fpA,fpB))`,
  12 groupes de `u16 mod 100000`.
- **Réel :**
  1. Le contrat FFI v0 (US-106) n'a pas d'appel « marquer vérifié » : les
     contacts vérifiés vivent dans `AppairageViewModel` (perdus au
     redémarrage de l'app).
  2. Identité locale provisoire : `generateIdentity("tel-xxxx")` du
     bouchon, pseudo aléatoire par installation. Le bouchon dérivant le
     `peerId` des 8 premiers octets du pseudo, l'aléa doit y tenir.
  3. Le code affiché est celui du bouchon (FNV-1a) : forme conforme (12 × 5
     chiffres, identique des deux côtés, ordre-indépendant), calcul non
     conforme — comme prévu par US-106.
- **Raison :** l'US est explicitement « sur bouchon FFI » ; le branchement
  réel est l'US-306.
- **Conséquences :** US-302/US-306 devront ajouter au FFI un appel
  « marquer vérifié » (et la persistance `contacts.verified_at`), et
  remplacer `IdentiteLocale` par la vraie identité (US-205).
- **Doc de conception mise à jour ?** non.

---

### 2026-09-09 — Dashboard : migrations en littéral Python, pas en fichiers `.sql`

- **Prévu :** l'issue #10 (US-110) demande une « migration SQLite initiale
  **versionnée** ». Le premier jet a suivi le pattern classique : un dossier
  `dashboard/api/migrations/` avec `0001_initial.sql`, lu et exécuté au
  démarrage.
- **Réel :** les migrations sont une liste `(version, nom, sql)` dans
  `dashboard/api/app/migrations.py`, où `sql` est un littéral de chaîne. `db.py`
  itère cette liste. Plus de dossier `migrations/`.
- **Raison :** SonarCloud (`pythonsecurity:S3649`, **BLOCKER**, gate de sécurité
  du nouveau code) flaggait le chemin `Path.read_text()` → `executescript()`
  comme « SQL construit depuis une donnée contrôlée par l'utilisateur ». C'est
  un faux positif — la donnée est un fichier versionné, pas de l'entrée
  requête — mais corriger à la racine est ici plus propre qu'une suppression
  `# NOSONAR` : un littéral de module est tout aussi versionné, n'ajoute aucune
  dépendance au système de fichiers au déploiement, et supprime le motif que
  l'analyseur (à raison, en général) surveille.
- **Conséquences :** le mot « versionné » de l'AC reste satisfait (git + numéro
  de version + table `schema_migrations`). Si le nombre de migrations grossit
  au point de rendre un seul fichier Python pénible, repasser à des `.sql`
  lus est un refactor localisé à `db.py` + `migrations.py`.
- **Doc de conception mise à jour ?** non — `docs/synthese/09` ne prescrit pas
  la forme des migrations, seulement le schéma cible (§11.2).

### 2026-09-09 — Protection de `main` : un seul check requis (`core`) au lieu de quatre

- **Prévu :** l'issue #13 (US-113) et
  [`docs/olivier/proposition-organisation-github.md`](../olivier/proposition-organisation-github.md)
  §4.3 demandent une protection de `main` avec **quatre checks requis** :
  `core`, `sim`, `audit` et `cross-vectors`. La DoD §7.1 point 3 les redit.
- **Réel :** les commandes livrées dans
  [`modules/processus-github.md`](modules/processus-github.md) ne déclarent que
  `"contexts": ["core"]`.
- **Raison :** `sim`, `audit` et `cross-vectors` **n'existent pas**. §4.3 les
  rattache à d'autres US, non commencées. Or un check requis qu'aucun workflow
  ne rapporte laisse la PR sur « Expected — Waiting for status to be reported »
  **indéfiniment** : GitHub attend un statut que personne n'enverra jamais. Les
  déclarer « pour être conforme à la DoD » gèlerait la totalité du dépôt, y
  compris les PR documentaires — c'est-à-dire la majorité de nos PR.
- **Conséquences :** l'intention de la DoD est préservée pour le seul job qui
  existe. Chaque nouveau workflow devra être ajouté à la liste **au moment où
  il est mergé**, en répétant la liste complète (l'API `PATCH` remplace le
  tableau `contexts`, elle ne l'enrichit pas). La commande d'élargissement est
  écrite dans la fiche. Condition de levée : à la fermeture de l'US qui livre
  chaque workflow.
- **Doc de conception mise à jour ?** non — la cible reste bien quatre checks ;
  seul le calendrier d'activation change. Le point est signalé dans le corps de
  la PR de l'US-113 et dans `modules/processus-github.md`.

---

### 2026-09-09 — Dossier `contracts/` ajouté au layout du dépôt (US-107)

- **Prévu :** `docs/synthese/04` §5 dessine le dépôt sans dossier pour les
  artefacts de contrat inter-langages ; les « vecteurs de conformité » y sont
  seulement évoqués (`synthese/10` §4.7, job `cross-vectors`).
- **Réel :** un dossier **`contracts/`** à la racine, contenant `events/`
  (schémas JSON + `CANONICAL.md` + 20 fixtures signées) et `tools/` (générateur
  + validateur, outillés `uv`).
- **Raison :** ces artefacts sont **neutres en langage** et consommés par trois
  composants (`dashboard/` Python, `crates/` Rust, `firmware/` C). Les mettre
  sous l'un d'eux créerait une dépendance de build inversée ; sous `docs/` ils
  ne seraient pas exécutables par la CI.
- **Conséquences :** un `paths: contracts/**` de plus en CI
  (`.github/workflows/contracts.yml`). Le job `cross-vectors` de `synthese/10`
  §4.7, quand il existera, consommera `contracts/events/fixtures/`.
- **Doc de conception mise à jour ?** non (layout indicatif). À mentionner au
  prochain rafraîchissement de `synthese/04` §5.

---

### 2026-09-09 — Identifiants pseudonymes tronqués : tous à 8 octets / 16 hex (US-107)

- **Prévu :** notations incohérentes selon le document —
  `msg_log_id` : `docs/powl/08` §1.3 `hex(SHA-256(msgID)[0..16])`,
  `docs/synthese/04` §7 `SHA-256(msgID)[:16]`, `docs/synthese/09` §11.2
  « hex 16 o » ;
  `conv_hash` : `docs/synthese/09` §9 `SHA-256(min‖max)[0..8]` ;
  `from_peer` / `peer` / `to_peer` : « `peerID` tronqué à 8 o » (`docs/synthese/09`
  §9). « 16 », « 8 » = octets ou caractères hex ? Les trois se lisent dans les
  deux sens.
- **Réel :** le contrat US-107 fixe **une seule règle pour tous les
  identifiants pseudonymes tronqués : 8 octets → 16 caractères hex**
  (`^[0-9a-f]{16}$`). `recipient_tag` reste à 16 octets / 32 hex (déjà défini
  ainsi, `docs/synthese/06`).
- **Raison :** le **seul exemple concret** du corpus (`docs/synthese/09` §9,
  `msg_log_id` = `"4d5e6f7a8b9c0d1e"` et `from_peer` = `"a1b2c3d4e5f60718"`)
  fait 16 hex dans les deux cas. Uniformiser évite d'avoir des largeurs
  différentes pour des objets de même nature (empreintes SHA-256 tronquées).
  Plus court = moins corrélable, suffisant pour dédupliquer / grouper de
  l'observabilité.
- **Conséquences :** `docs/powl/08`, `docs/synthese/04` §7 et `docs/synthese/09`
  (§9 pour `conv_hash`, §11.2 pour `messages.msg_log_id` : `TEXT` de 16
  caractères) doivent être alignés sur « 8 octets / 16 hex » pour **tous** ces
  champs. `dengon-core` (US-208) et l'ingest (US-216) tronquent à 8 octets.
- **Doc de conception mise à jour ?** pas encore — à répercuter dans
  `docs/powl/` et `docs/synthese/`. Résumé dans `contracts/events/CANONICAL.md` §3.

---

### 2026-09-09 — Le label est `good first issue`, pas `good-first-issue`

- **Prévu :** §5.3 liste le label `good-first-issue`, avec des traits d'union.
- **Réel :** `.github/labels.yml` déclare `good first issue`, avec des espaces.
- **Raison :** c'est le label **par défaut de GitHub**, il existe déjà sur le
  dépôt et il est utilisé. Créer la variante à traits d'union donnerait deux
  labels au sens identique, dont un vide, et casserait la vue du board qui
  filtre sur le nom réel. Le fichier `labels.yml` a pour rôle de décrire
  l'existant : le corriger ici aurait fait mentir sa promesse de « zéro
  changement au premier sync ».
- **Conséquences :** aucune, sinon qu'il faut écrire le nom avec des espaces
  quand on filtre. La politique de déblocage de §10.3 point 5 (« prendre une
  `good-first-issue` d'une autre area ») s'applique inchangée.
- **Doc de conception mise à jour ?** non — coquille de nommage, sans effet sur
  l'organisation décrite.

---

### 2026-09-09 — Filtrage par chemin du workflow `core` : dans le job, pas dans le déclencheur

- **Prévu :** l'issue #4 (US-104) demande littéralement « le workflow est filtré
  par chemin (`paths: crates/**`) », c'est-à-dire un filtre au niveau de
  `on.pull_request.paths`. Même formulation dans
  [`docs/olivier/proposition-organisation-github.md`](../olivier/proposition-organisation-github.md) §4.3.
- **Réel :** `.github/workflows/core.yml` n'a **aucun** `paths:` sur ses
  déclencheurs. Le filtrage se fait à la première étape du job, via
  `dorny/paths-filter`, et chaque étape lourde porte un
  `if: steps.filtre.outputs.rust == 'true'`.
- **Raison :** avec un `paths:` sur le déclencheur, GitHub **ne démarre pas du
  tout** le workflow quand aucun fichier ne correspond. Aucun *check run* nommé
  `core` n'est alors créé. Or la protection de `main` (US-113) exigera un check
  `core` : toute PR ne touchant que `docs/` resterait bloquée indéfiniment sur
  « Expected — Waiting for status to be reported », même approuvée. Et sur ce
  projet, **la majorité des PR ne touchent que `docs/`** (trois dossiers de
  conception, plus la règle de mise à jour du suivi imposée par `CLAUDE.md`).
  La parade « officielle » — un workflow jumeau en `paths-ignore` avec un job
  homonyme — est pire : `paths` et `paths-ignore` ne sont pas complémentaires,
  une PR touchant `docs/` **et** `crates/` déclencherait les deux et créerait
  deux checks `core` concurrents.
- **Conséquences :** l'intention du critère est préservée — aucune commande
  `cargo` ne tourne sur une PR qui ne touche pas Rust. Le coût est d'environ
  15 s de runner sur ces PR, contre 0 s avec un filtre au déclencheur. En
  échange, le check `core` est **toujours** rapporté et peut être rendu
  obligatoire sans piéger l'équipe. **Le même patron devra être appliqué à
  `sim`, `audit` et `cross-vectors`** quand ils seront écrits.
- **Doc de conception mise à jour ?** non — le point est trop fin pour
  `docs/synthese/`. Il est signalé dans le corps de la PR de l'US-104 et en
  commentaire en tête de `.github/workflows/core.yml`.

---

### 2026-09-09 — `Cargo.lock` versionné

- **Prévu :** le `.gitignore` d'origine (ligne 186, commit `c0e2a9b`, écrit
  quand la stack n'était pas encore choisie) ignorait `Cargo.lock`.
- **Réel :** la ligne a été retirée ; `Cargo.lock` est versionné.
- **Raison :** la recommandation Rust est de committer le lock dès qu'un dépôt
  produit un exécutable — ici il y en a trois (`dengon-node`, `dengon-sim`,
  `dengon-verify`). Trois raisons propres au projet s'ajoutent : (a) la CI lance
  `clippy -D warnings`, donc une version patch d'une dépendance transitive
  publiée n'importe quel jour pourrait faire virer `main` au rouge sans qu'une
  ligne du dépôt ait changé ; (b) `cargo audit` et `cargo deny` du futur job
  `audit` lisent le lock — sans lui, un rapport de vulnérabilité ne correspond
  à ce que personne n'a construit ; (c) le job `cross-vectors` suppose une
  reproductibilité entre core, firmware et dashboard.
- **Conséquences :** la CI passe `--locked` partout, ce qui devient un
  garde-fou : une PR qui modifie `Cargo.toml` sans régénérer le lock échoue
  immédiatement avec un message clair. Contrepartie : les montées de version de
  dépendances deviennent des diffs explicites à relire.
- **Doc de conception mise à jour ?** sans objet (le `.gitignore` n'est pas un
  document de conception) ; la raison est écrite en commentaire dans le fichier
  lui-même.

---

### 2026-09-10 — `.gitignore` : les fixtures de clés de test sont ré-incluses

- **Prévu :** rien. Le `.gitignore` générique (commit `c0e2a9b`, écrit quand la
  stack n'était pas choisie) ignore `*.pem`, `*.key`, `*.p12` et `*.pfx` pour
  éviter qu'un vrai secret soit commité.
- **Réel :** quatre négations limitées aux répertoires `tests/` des crates
  (`!crates/**/tests/**/*.pem` et les trois autres).
- **Raison :** signalé en **revue de la PR #57** par `G1TS23`. Une clé
  d'exemple servant de fixture n'est pas un secret, mais les motifs génériques
  l'avalaient **en silence** : le fichier n'était jamais ajouté, sans erreur ni
  avertissement. Le symptôme ne serait apparu que plus tard et ailleurs — tests
  verts en local, rouges en CI — au moment de l'**US-108 (crypto)**, qui est sur
  le chemin critique du sprint 2. Corriger ici coûtait quatre lignes ; découvrir
  le problème pendant une US de gate aurait coûté une demi-journée.
- **Conséquences :** la portée est volontairement étroite — uniquement sous
  `tests/`, uniquement dans `crates/`. Vérifié dans les deux sens :
  `crates/dengon-core/tests/fixtures/alice.key` est désormais ajoutable
  (`git add --dry-run` → `add '...'`), tandis que `crates/dengon-core/prod.pem`,
  `dashboard/api/prod.pem` et `secret.key` restent refusés. GitGuardian, actif
  sur chaque PR, sert de second filet. Les autres areas (`dashboard/`,
  `contracts/`) devront faire le même geste quand elles y toucheront.
- **Doc de conception mise à jour ?** sans objet ; la raison est en commentaire
  dans le `.gitignore` lui-même.

---

### 2026-09-16 — Arborescence du firmware : `main/`, pas `src/transport_nimble.c`

- **Prévu :** [`docs/synthese/04-architecture.md`](../synthese/04-architecture.md)
  ligne 87 place l'implémentation NimBLE du trait `Transport` dans
  `firmware/dengon-relay/src/transport_nimble.c`.
- **Réel :** `firmware/dengon-relay/main/` (`main.c`, `dengon_gatt.c`,
  `dengon_peer_id.c`), avec un `CMakeLists.txt` à la racine du projet et un
  autre dans `main/`.
- **Raison :** c'est la convention ESP-IDF, et elle n'est pas négociable :
  `idf.py`, `project.cmake` et `idf_component_register` supposent tous un
  composant nommé `main`. Un dossier `src/` à la racine d'un projet ESP-IDF
  n'est compris par aucun outil de la chaîne. Le **même fichier** de conception
  se contredit d'ailleurs : son §5 (lignes 148-151) décrit déjà
  `main/` + `components/dengon_core_ffi/`. La ligne 87 est la coquille.
- **Conséquences :** aucune sur le fond — le trait `Transport` reste le contrat.
  L'US-220 écrira son `transport_nimble.c` dans `main/`, et l'US-307 posera
  `libdengon_core.a` dans `components/dengon_core_ffi/` comme le prévoit le §5.
- **Doc de conception mise à jour ?** non — une ligne d'un tableau récapitulatif
  contredite par le §5 du même document. Signalé ici et dans la fiche du module.

---

### 2026-09-16 — Nom d'annonce BLE : inventé ici, et relégué en réponse de scan

- **Prévu :** rien. Aucun document de conception ne spécifie le *local name*
  annoncé par l'ESP32. `docs/powl/03-network-protocol.md` §6.1 ne décrit que
  l'UUID de service et le manufacturer data.
- **Réel :** `dengon-relay-XXXX`, où `XXXX` est la forme hexadécimale des deux
  premiers octets du peerID, émis dans la **réponse de scan** et non dans le
  paquet d'annonce principal.
- **Raison :** le paquet d'annonce est limité à 31 octets et il est déjà saturé
  par ce que la conception impose : Flags (3) + liste complète d'UUID 128 bits
  (18) + manufacturer data (9) = **30 octets**. Il ne reste pas la place d'un
  nom. Le reléguer en réponse de scan ne coûte rien : un scanner émet un
  `SCAN_REQ` sur une annonce `ADV_IND` et affiche le nom malgré tout. Le suffixe
  distingue deux cartes posées côte à côte — indispensable dès l'US-220.
- **Conséquences :** l'UUID de service reste dans le paquet principal, donc le
  filtrage de scan des pairs (§6.1) fonctionne sans devoir interroger chaque
  annonceur. En revanche, tout ajout futur au paquet principal fera échouer
  `ble_gap_adv_set_fields` avec `BLE_HS_EMSGSIZE` — **à l'exécution**.
- **Doc de conception mise à jour ?** non, pas encore : à remonter dans
  `docs/powl/03` §6.1 quand l'US-220 figera le format d'annonce pour de bon.

---

### 2026-09-16 — Manufacturer data : 7 octets et non 5, à cause du Company ID

- **Prévu :** `docs/powl/03-network-protocol.md` §6.1 décrit un *manufacturer
  data* valant `peerID[0..4] ‖ flags`, soit **5 octets**.
- **Réel :** 7 octets — `Company ID (2) ‖ peerID[0..4] (4) ‖ flags (1)`, avec
  `Company ID = 0xFFFF`.
- **Raison :** le champ AD de type `0xFF` du Core Bluetooth **exige** un
  identifiant de fabricant sur ses deux premiers octets, et NimBLE ne préfixe
  rien : il émet tel quel le tampon qu'on lui donne. Sans Company ID, certains
  analyseurs rejettent le champ ou décalent leur lecture de deux octets.
  `0xFFFF` est l'identifiant que le Bluetooth SIG réserve aux tests et à l'usage
  interne : c'est le seul choix légitime tant que le projet n'a pas d'identifiant
  attribué.
- **Conséquences :** le budget d'annonce passe à 30 octets sur 31 (voir l'écart
  précédent). Tout code qui décodera ce champ — l'application Android en
  US-213, le `transport_nimble.c` en US-220 — doit sauter les **deux premiers**
  octets pour retrouver le `peerID[0..4]` de la conception.
- **Doc de conception mise à jour ?** non, pas encore ; à corriger dans
  `docs/powl/03` §6.1, car l'omission y est une vraie erreur de spécification,
  pas un choix.

---

### 2026-09-16 — Octet `flags` de l'annonce : bitfield défini faute de spécification

- **Prévu :** `docs/powl/03-network-protocol.md` §6.1 nomme un octet `flags`
  dans le manufacturer data, sans jamais en définir les bits.
- **Réel :** bitfield posé dans `firmware/dengon-relay/main/main.c` —
  `0x01` RELAY (nœud d'infrastructure fixe), `0x02` COURIER (porte des
  enveloppes en dépôt), `0x04` ACCEPTS_CONN (accepte les connexions GATT),
  `0x08` HAS_UPLINK (dispose d'un lien IP vers le VPS), bits 4 à 7 réservés.
  Le squelette émet `0x05` (RELAY | ACCEPTS_CONN).
- **Raison :** il fallait bien émettre un octet. Les quatre bits retenus sont
  ceux qui servent au filtrage de scan décrit par la conception : distinguer un
  relais d'un téléphone, et savoir avant de se connecter si le pair acceptera.
- **Conséquences :** c'est un contrat **de fait** entre le firmware et
  l'application Android. À ne pas confondre avec les `flags` du paquet de
  couche 3 (`docs/powl/03` §3.1), qui n'ont aucun rapport — la collision de nom
  est un piège en soi. Tant que l'US-213 et l'US-220 n'ont pas repris ces
  valeurs, rien ne les vérifie.
- **Doc de conception mise à jour ?** non, pas encore ; à remonter dans
  `docs/powl/03` §6.1 dès que l'US-220 s'en sert.

---

### 2026-09-16 — `peerID` bouchonné sur la MAC eFuse au lieu de la clé statique

- **Prévu :** `docs/powl/03-network-protocol.md` §3 —
  `peerID = SHA-256(pub_static)[0..8]`.
- **Réel :** `SHA-256(MAC eFuse d'usine)[0..8]`, dans
  `firmware/dengon-relay/main/dengon_peer_id.c`.
- **Raison :** `pub_static` n'existe pas encore. La génération de la paire
  Ed25519/X25519 et son stockage en NVS chiffrée sont l'US-307. Le squelette a
  besoin d'un identifiant qui soit seulement stable d'un redémarrage à l'autre
  et distinct d'une carte à l'autre, pour remplir le manufacturer data et le nom
  d'annonce : la MAC d'usine remplit ces deux conditions.
- **Conséquences :** **ce n'est pas une identité cryptographique.** Une adresse
  MAC est publique et prédictible ; rien ne doit s'en servir pour authentifier
  quoi que ce soit. Le risque réel est qu'on l'oublie : l'avertissement est en
  tête de `dengon_peer_id.h`, répété dans la fiche du module.
  **Condition de levée : US-307**, qui doit remplacer l'implémentation sans
  toucher à l'interface `dengon_peer_id_get()`.
- **Doc de conception mise à jour ?** non — la conception est juste, c'est
  l'implémentation qui est provisoire.

---

### Piège de `.gitignore` repéré mais **non corrigé** (dette assumée)

- Ligne `bin/` (section .NET, non ancrée) → ignorerait `crates/*/src/bin/` le
  jour où une crate aura des binaires secondaires. Laissé en l'état : aucune
  crate n'a de binaire secondaire, et le mode d'échec est bruyant (le fichier
  manque, la compilation échoue) — contrairement à celui des clés, qui était
  silencieux.

---

### 2026-09-16 — US-112 : deux écarts vs `powl/09` omis du journal (retour de revue #66, point de Paul)

- **Prévu :** [`docs/powl/09-data-model.md`](../powl/09-data-model.md) §1
  annote `messages.body` d'un commentaire SQL `-- clair local uniquement`, et
  marque `identity.priv_static` / `identity.priv_sign` comme `BLOB`
  « (chiffré) », sans préciser le mécanisme de chiffrement.
- **Réel :** `06-securite.md` §5.1 (US-112) traite les deux différemment :
  `messages.body` est reclassé **XChaCha20-Poly1305 champ par champ** (B-3),
  le commentaire `powl/09` étant réinterprété comme décrivant le *contenu*
  (texte déchiffré côté app) et non l'état de chiffrement au repos ; et
  `identity.priv_static`/`priv_sign` sont confiées au **stockage de clés du
  téléphone (Keystore/Keychain, Android/iOS)** plutôt qu'à un chiffrement
  logiciel générique (XChaCha20 comme les autres colonnes sensibles) — c'est
  le mécanisme qui change, pas le fait qu'elles soient chiffrées.
- **Raison :** B-3 (chiffrement champ par champ des données sensibles) est une
  décision postérieure à `powl/09`, qui ne pouvait pas l'anticiper ; et les
  clés privées d'identité justifient le coffre matériel de la plateforme
  plutôt qu'un chiffrement logiciel générique — c'est *le* cas d'usage du
  Keystore.
- **Conséquences :** aucune régression — les deux reclassements sont des
  renforcements (chiffrement au repos de `messages.body` ; clés privées dans
  le coffre matériel au lieu d'un chiffrement logiciel), pas des
  affaiblissements, de ce que `powl/09` décrivait.
  L'erreur signalée par Paul n'est pas dans le contenu de `06-securite.md`
  (correct dès la PR initiale) mais dans **le journal** : les deux entrées du
  2026-09-11 pour US-112 déclarent toutes deux « Écarts vs conception :
  Aucun », alors que ces deux reclassements en sont, et auraient dû être
  consignés ici dès leur rédaction plutôt que découverts en revue.
- **Doc de conception mise à jour ?** non — `docs/powl/` reste inchangé par
  convention (matière première figée) ; `06-securite.md` (le delta) portait
  déjà la bonne information, seul le suivi (`00-journal.md`) était en faute.

---

### 2026-09-10 — Vecteurs de conformité v0 dans `crates/dengon-core/tests/`, pas `contracts/packet/` (US-108)

- **Prévu :** l'US-108 demande « un fichier partagé, consommé par le core, le
  firmware et le dashboard (base du job CI `cross-vectors`) ». Le dossier
  `contracts/` (créé par US-107) est l'emplacement naturel, neutre en langage.
- **Réel :** les vecteurs sont dans `crates/dengon-core/tests/vectors_v0.json`,
  contrôlés par `tests/protocol_vectors.rs`.
- **Raison :** `contracts/` **n'est pas encore sur `main`** (PR #60 en revue).
  Y déposer `contracts/packet/` depuis cette branche entraînerait un conflit sur
  `contracts/pyproject.toml` / `uv.lock` / `.github/workflows/contracts.yml` au
  rebase. Le fichier reste consommable par le firmware (C) et le dashboard
  (Python) à ce chemin ; il est juste rangé dans la crate qui le produit.
- **Conséquences :** déplacement vers `contracts/packet/` prévu une fois #60
  mergé (refactor localisé : `git mv` + ajout d'un `validate_packets.py` +
  ligne dans `contracts.yml`). Le job `cross-vectors` (US-222) pointera sur
  l'emplacement final.
- **Doc de conception mise à jour ?** sans objet.

---

### 2026-09-10 — `serde_json` en dev-dependency de `dengon-core` (US-108)

- **Prévu :** `docs/suivi/…` (entrée US-104) : « on ajoute une dépendance
  externe au moment où une PR l'utilise réellement ».
- **Réel :** `serde_json = "1"` en `[dev-dependencies]` de `dengon-core`.
- **Raison :** `tests/protocol_vectors.rs` lit `vectors_v0.json`. Via
  `serde_json::Value` (pas de `#[derive]`), donc `serde` n'est pas tiré comme
  dépendance de proc-macro. `cargo check` ne compile pas les dev-deps → **aucun
  effet sur `no_std`** ni sur l'artefact firmware.
- **Conséquences :** `Cargo.lock` gagne `serde_json`, `itoa`, `memchr`, `ryu`,
  `serde` (dev uniquement). `dengon-core` en dépendra de toute façon en runtime
  pour `observability` (US-208) — JSON canonique des événements.
- **Doc de conception mise à jour ?** non — usage attendu.
### 2026-09-11 — `pkt.seen.rssi` reste optionnel dans le catalogue (US-107)

- **Prévu :** `docs/powl/08-observability-events.md:43` et
  `docs/synthese/09-dashboard-et-donnees.md:181` listent le payload de
  `pkt.seen` comme `{ msg_log_id, type, ttl_in, size_bucket, from_peer, rssi }`
  — sans `?` sur `rssi`, contrairement à `pseudo?` (`peer.announce_seen`) ou
  `ssid?`/`duration_s?` (`relay.wifi_up`/`down`) dans les mêmes tableaux. Par
  la convention du document, `rssi` y est donc **requis**.
- **Réel :** `contracts/tools/catalogue.py`, l'entrée `pkt.seen` ne liste pas
  `rssi` dans `"required"` — seulement dans `"props"`. Un événement `pkt.seen`
  sans `rssi` passe `validate.py`.
- **Raison :** signalé en **revue de la PR #60** par `OswinFreyr`. Le RSSI
  n'est pas toujours disponible à la couche transport : `TransportEvent::
  PeerConnected.rssi` (US-105, `crates/dengon-ble/src/transport.rs`) est déjà
  typé `Option<i16>`, avec la même raison documentée en rustdoc — Android ne
  fournit le RSSI qu'à la demande, NimBLE pas du tout sur une connexion
  entrante. Rendre `rssi` requis dans le contrat d'événement obligerait à
  inventer une valeur sur les chemins où le transport n'en a pas, ce qui
  serait plus trompeur qu'un champ absent.
- **Conséquences :** c'est la **doc de conception** qui est en retard, pas le
  contrat. `docs/synthese/09-dashboard-et-donnees.md:181` mis à jour avec
  `rssi?` pour refléter ce que `powl/03-network-protocol.md`/US-105 ont déjà
  établi côté transport ; `docs/powl/08` non touché (dossier figé, cf.
  `CLAUDE.md`).
- **Doc de conception mise à jour ?** oui — `docs/synthese/09-dashboard-et-donnees.md`.

---

### Piège JSON Schema repéré mais **partiellement corrigé** (dette assumée) — `node_id`/`name`

- Les motifs `^(relay|client)-[0-9a-f]{6,}$` (`node_id`) et `^[a-z]+\.[a-z_]+$`
  (`name`) dans `contracts/events/envelope.schema.json` restent vulnérables
  au même piège que `HEX16`/`HEX32`/`HEX64`/`sig`/`batch_id` (un `$` Python
  matche juste avant un `\n` final — retour de revue #60) : ces deux-là n'ont
  **pas** reçu de `minLength`/`maxLength` correctif, contrairement aux
  champs de longueur fixe.
- **Raison de ne pas corriger pareil :** ces deux motifs sont **ouverts**
  (`{6,}` sans borne haute, `name` sans longueur fixe) — `minLength` seul ne
  fermerait pas le trou. La seule fermeture complète serait `\Z` au lieu de
  `$`, une extension **Python**, absente d'ECMA 262 (la norme visée par
  `pattern` en JSON Schema) — l'introduire irait à l'encontre de l'objectif
  même de `contracts/` (neutre en langage, potentiellement validé un jour par
  un moteur non-Python).
- **Impact réel :** faible. Un `node_id`/`name` avec un `\n` final ne casse
  rien silencieusement : `name` sert de clé dans `CATALOGUE`, donc un nom
  suffixé échouerait de toute façon au lookup (`événement hors catalogue`) ;
  `node_id` n'entre dans aucun calcul qui plante dessus (contrairement à
  `seq`/`payload`, corrigés). C'est une strictness manquante, pas un chemin
  de crash.
- **Condition de levée :** si `contracts/` gagne un jour un second
  consommateur non-Python qui a besoin d'une validation stricte de ces deux
  champs, ou si le motif `node_id` gagne une borne haute naturelle.
- **Mise à jour 2026-09-16 (retour de revue #60, round 3) :** le trou côté
  `name` est refermé, mais pas par ce mécanisme — `payloads_json_schema()`
  contraint désormais `name` à `{"enum": sorted(CATALOGUE)}` (comparaison de
  chaîne exacte, pas une regex). Un `name` avec un `\n` final, ou toute autre
  valeur hors catalogue, échoue à l'égalité de chaîne peu importe la regex du
  champ dans `envelope.schema.json`. Le trou décrit ci-dessus ne s'applique
  donc plus qu'à `node_id`.
- **Mise à jour 2026-09-28 (retour de revue #60, round 5, point 1
  d'OswinFreyr) :** `name` a en réalité toujours deux protections
  indépendantes maintenant — l'`enum` ci-dessus côté `payloads.schema.json`,
  **et** `envelope.schema.json` lui-même ferme le trou avec `"not":
  {"pattern": "\n"}` sur la propriété `name`. Le motif `node_id` reste seul
  concerné par ce piège ; voir l'entrée dédiée ci-dessous sur la longueur de
  `node_id`, qui documente pourquoi il n'a pas reçu le même traitement.
- **Mise à jour 2026-09-28 (retour de revue #60, round de suivi
  d'OswinFreyr) :** cette mise à jour n'était déjà plus exacte au moment où
  elle a été écrite — le round 4 (avant elle) avait **aussi** fermé le trou
  côté `node_id` : `$defs/node_id` est passé d'un motif ouvert `{6,}` à un
  `anyOf` de deux branches à longueur exacte
  (`^relay-[0-9a-f]{6}$`/`minLength=maxLength=12` et
  `^client-[0-9a-f]{6}$`/`minLength=maxLength=13`), qui ferme le trou du `$`
  par construction (une longueur exacte élimine tout suffixe, `\n` compris)
  — revérifié avec `jsonschema` : `relay-abcdef\n` et `client-abcdef\n`
  sont bien rejetés. **Le titre de cette entrée (« partiellement corrigé »,
  « dette assumée ») et son corps (qui décrit encore le motif `{6,}`
  d'origine) sont donc obsolètes pour les deux champs** — conservés
  tels quels par discipline append-only, mais à ne plus lire comme l'état
  réel : `node_id` a une longueur fixe (6 hex, voir l'entrée dédiée
  ci-dessous) et `name` a deux protections indépendantes, plus rien de
  « partiel » ni « assumé » comme dette pour aucun des deux champs.

---

### 2026-09-28 — `node_id` : longueur hexadécimale gelée à 6 caractères (24 bits), risque de collision assumé pour le MVP (retour de revue #60, round 5, point 2 d'OswinFreyr)

- **Constat :** `docs/synthese/09-dashboard-et-donnees.md:64` décrit
  `node_id` comme « un hash », sans fixer de longueur. Le contrat
  (`contracts/events/envelope.schema.json`, `$defs/node_id`) fige la partie
  hexadécimale à exactement 6 caractères (`relay-[0-9a-f]{6}` /
  `client-[0-9a-f]{6}`) — un choix fait au round 4 pour fermer le trou du
  `$`/`\n` avec une longueur exacte par branche (`minLength == maxLength`),
  mais jamais tranché comme décision de contrat en tant que telle : ce
  round 4 corrigeait un piège regex, pas la taille de l'espace de noms.
- **`node_id` sert de clé primaire** (`nodes.node_id`), entre dans le calcul
  d'`event_id` (`SHA-256(node_id ‖ seq)`) et identifie le journal chaîné par
  appareil (A-7). Une collision entre deux nœuds mélangerait leurs deux
  chaînes côté dashboard et produirait de faux `integrity.chain_broken`.
- **Risque quantifié (approximation des anniversaires,
  `p ≈ 1 - exp(-n²/2N)`, `N = 16 777 216` pour 24 bits) :** ~0,03 % pour
  100 nœuds, ~2,9 % pour 1 000 nœuds, ~66 % pour 6 000 nœuds (valeurs
  corrigées au round de suivi d'OswinFreyr, 2026-09-28 — les deux premières
  citées à tort comme ~0,003 % et ~63 % dans la version initiale de cette
  entrée). Le MVP vise une démo de 5-8 appareils (B-4) — risque
  négligeable à cette échelle, mais 6 caractères ne passerait pas à
  l'échelle d'un déploiement réel de plusieurs centaines de nœuds sans
  revoir la longueur.
- **Décision : garder 6 hex (24 bits) pour le MVP**, plutôt que de rouvrir le
  motif en longueur variable (`{6,}` + `"not": {"pattern": "\n"}` + une borne
  haute explicite, ex. `maxLength: 40`, qui aurait aussi fermé le trou sans
  figer la longueur). Une longueur fixe est plus simple à documenter et à
  tester (les fixtures golden et `docs/powl/08` utilisent déjà tous 6 hex),
  et le MVP ne dépassera pas quelques appareils physiques.
- **Condition de levée :** avant tout déploiement au-delà de quelques
  centaines de nœuds simultanés, augmenter la longueur hexadécimale de
  `node_id` (le format `relay-`/`client-` + longueur exacte reste le même
  mécanisme, seule la borne change) et régénérer les fixtures golden en
  conséquence.
- **Doc de conception mise à jour ?** non — `docs/synthese/09` reste
  volontairement vague (« un hash ») ; ce fichier est la référence pour la
  longueur réellement figée par le contrat.

---

### 2026-09-16 — `prev_hash` ajouté à l'enveloppe, optionnel (US-107, retour de revue #60 round 3)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §D (`integrity.chain_broken`
  → `{expected_prev, got_prev}`) et `:430` (colonne `events.prev_hash`)
  supposent qu'un événement porte le hash de l'événement précédent du même
  nœud.
- **Réel :** `envelope.schema.json` était strict (`additionalProperties: false`)
  sans déclarer `prev_hash` — aucun consommateur ne pouvait le faire
  transiter. `prev_hash` (HEX64) a été ajouté aux `properties`, **sans** le
  mettre dans `required` : optionnel.
- **Raison :** fermer la possibilité de transit avant le gel du contrat plutôt
  que de découvrir après coup qu'`integrity.chain_broken` est indérivable
  pour de bon. Rester optionnel (au lieu de required) parce qu'aucun
  producteur (`dengon-core`, US-208) n'existe encore pour le remplir, et
  qu'aucune des 20 fixtures ne le porte — le rendre requis casserait le
  contrat sans qu'un vrai producteur en bénéficie.
- **Conséquences :** `chain_broken` reste **non exercé** par les fixtures
  (`got_prev` n'apparaît dans aucun batch golden) — seule la possibilité de
  transit est acquise, pas la vérification bout-en-bout. À revisiter quand
  US-208 (journal chaîné côté nœud) existera : soit une 21ᵉ fixture porteuse
  de `prev_hash`, soit le passage en `required`.
- **Doc de conception mise à jour ?** non — `synthese/09` décrivait déjà ce
  besoin, c'est le contrat qui le rattrape.
### `TransportEvent::PeerDisconnected` porte un **motif**, absent de la conception

- **Conception :** [`docs/synthese/04-architecture.md`](../synthese/04-architecture.md)
  §3 décrit `PeerDisconnected { peer_link_id }` — le lien, rien d'autre.
- **Code :** `crates/dengon-ble/src/transport.rs:181` ajoute un champ
  `reason: DisconnectReason`, à trois valeurs : `Propre`, `Brutale`, `Locale`.
- **Pourquoi :** le critère d'acceptation n°4 de l'US-105 demande que « le
  comportement attendu en déconnexion brutale soit spécifié ». On peut le
  spécifier en rustdoc — c'est fait — mais la couche du dessus doit aussi
  pouvoir **distinguer les cas à l'exécution** : une coupure brutale justifie de
  retenter tout de suite (le pair est peut-être encore à portée), un départ
  propre non. Sans ce champ, `sync::routing` (US-209) ne peut que deviner, et
  devinerait mal dans le cas le plus fréquent — un réseau maillé mobile passe
  son temps à perdre des liens brutalement.
- **Conséquences :** c'est un **élargissement du contrat gelé**, à assumer comme
  tel : les quatre implémentations (`btleplug`, Android, NimBLE, bouchon)
  devront remplir ce champ correctement, et la suite de conformité les y oblige
  (`cas_deconnexion_brutale`, `cas_deconnexion_propre_est_distinguee`). Sur
  matériel réel, produire une `Brutale` veut dire couper l'alimentation de la
  carte d'en face, pas appeler `disconnect` — c'est écrit dans la doc du trait
  `BancDEssai`. Le coût est réel mais borné ; le bénéfice est qu'un
  comportement central du maillage cesse d'être implicite.
- **Doc de conception mise à jour ?** **Non.** `04-architecture.md` §3 garde sa
  version. À trancher au point d'équipe qui gèle le contrat : soit on met la
  conception à jour, soit on retire le champ. Le laisser diverger en silence
  serait le pire des trois.

---

### 2026-09-20 — `dengon-ffi` v0 (US-106) : identité/QR/code de vérification sans vraie cryptographie

- **Prévu :** [`docs/synthese/06-securite.md`](../synthese/06-securite.md)
  décrit `peer_id = SHA-256(pub_static)[0..8]`, des clés X25519/Ed25519
  réelles, un QR `dengon:v1:<base64url(...)>` avec de vraies clés publiques,
  et un code de vérification 60 chiffres dérivé de
  `SHA-512(min(fpA,fpB)‖max(fpA,fpB))`.
- **Réel :** `crates/dengon-ffi/src/lib.rs` (`generate_identity`,
  `verification_code`) et `android/.../ffi/DengonNodeStub.kt`
  (`DengonIdentity`) produisent des octets **déterministes mais non
  cryptographiques** (XOR du pseudo pour les « clés », FNV-1a pour le code de
  vérification). Le format (types, `dengon:v1:` + base64url, 12 groupes de 5
  chiffres) est bien celui de la conception ; le contenu ne l'est pas.
- **Raison :** l'US-106 est un contrat FFI (`.udl` + bouchon), pas l'US
  crypto. `dengon-core::identity`/`crypto` n'existent pas encore (US-108,
  US-203, US-205, tous en sprint 2/S2). Or la DoR de l'US-106 exige un
  bouchon qui **renvoie des valeurs typées exploitables par l'UI**
  (US-214/US-215) dès maintenant — attendre la vraie crypto aurait bloqué
  tout le sprint 2 côté Android sur le sprint 2 côté Rust, exactement ce que
  le contrat gelé est censé éviter.
- **Conséquences :** le format des types FFI est stable et peut être
  développé contre dès maintenant. Le **contenu** des identités/codes générés
  aujourd'hui est sans valeur de sécurité et **change complètement** quand
  `identity`/`crypto` seront branchés (US-108/US-203/US-205 puis US-301/302) —
  aucun test ni donnée canned actuelle ne doit être considéré comme un
  vecteur de test cryptographique. Les implémentations Rust et Kotlin sont
  volontairement **indépendantes** (pas le même algorithme, pas le même
  résultat numérique pour la même identité) : documenté en commentaire des
  deux côtés pour éviter la confusion le jour où on les compare.
- **Doc de conception mise à jour ?** non — `06-securite.md` reste la cible
  réelle. Le point est documenté dans `modules/dengon-ffi.md` et l'entrée de
  journal du 2026-09-20.

---

### `TransportConfig` et `LinkId` sont inventés ici, sans référence de conception

- **Conception :** `04-architecture.md` §3 **les nomme** dans la signature du
  trait (`fn start(&mut self, cfg: TransportConfig)`, `peer_link_id: LinkId`)
  mais ne les définit nulle part. Aucun autre document ne les décrit.
- **Code :** `crates/dengon-ble/src/transport.rs:44` (`LinkId`) et `:78`
  (`TransportConfig`, 5 champs).
- **Pourquoi :** il fallait bien choisir. Les partis pris, justifiés en rustdoc
  et dans [`modules/dengon-ble.md`](modules/dengon-ble.md) : `LinkId` identifie
  une **connexion** et non un nœud (le `peerID` d'A-8 reste au protocole) ;
  `TransportConfig` ne porte que ce qui **varie d'un nœud à l'autre**, pas les
  constantes de protocole, qui arrivent avec `protocol::consts` (US-108).
- **Conséquences :** ce découplage est ce qui permet à US-105 et US-108
  d'avancer **en parallèle** dans le même sprint, sans dépendance croisée.
  Contrepartie : `TransportConfig::local_peer_id` est typé `[u8; 8]` et non
  `PeerId` ; le remplacer quand US-108 aura livré ce type est un changement de
  signature, pas de sémantique.
- **Doc de conception mise à jour ?** Non — ce n'est pas une divergence mais un
  **comblement**. À verser dans `04-architecture.md` §3 si l'équipe veut que la
  conception reste la référence complète du contrat.

---

### 2026-09-28 — `dengon-sim` : les nœuds simulés exécutent un relais de démonstration, pas `dengon-core` (US-221)

- **Prévu :** `docs/synthese/10` §4.3 — « N instances de `dengon-core` reliées
  par un `Transport` en mémoire ».
- **Réel :** le harness fait tourner des nœuds dont le comportement est
  **injecté** (`trait Comportement`) ; les scénarios utilisent `Inondation`,
  un relais naïf (dédup par contenu, re-diffusion, poussée de tout le connu à
  chaque connexion). Ni TTL, ni signature, ni inventaire.
- **Raison :** `dengon-core` n'a pas encore de nœud exécutable : `sync::routing`
  (US-209) et la façade `api` (US-301) n'existent pas. Le réseau simulé, lui,
  est complet et conforme au contrat `Transport`.
- **Conséquences :** les scénarios livrés valident le **réseau simulé** et le
  harness, pas le protocole. Brancher le vrai nœud = implémenter
  `Comportement` pour lui, sans toucher au harness.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-28 — `dengon-sim` : sous-ensemble du modèle réseau et des scénarios de §4.3 (US-221)

- **Prévu :** `docs/synthese/10` §4.3 — modèle avec latence, perte, **bande
  passante**, partition, **churn**, **horloges désynchronisées** ; 9 scénarios
  dans `sim/scenarios/*.ron`.
- **Réel :** latence, gigue, perte, partition, retrait/ajout d'arête ; 4
  scénarios (`direct`, `multihop`, `partition_merge`, `lossy_mesh`) dans
  `crates/dengon-sim/scenarios/`.
- **Raison :** US-221 demande latence, perte et partition. Les autres
  scénarios (`recipient_offline`, `flood`, `tamper`…) testent le protocole et
  attendent le vrai nœud (entrée précédente). Le dossier `scenarios/` du crate
  existait depuis US-104.
- **Conséquences :** bande passante, churn et dérive d'horloge à ajouter avec
  les scénarios qui en ont besoin.
- **Doc de conception mise à jour ?** non.
### `store` : clé de chiffrement différée derrière un trait `KeySource` (US-207)

- **Conception :** `04-architecture.md` §2 dit `store : persistance ...
  chiffrement XChaCha20-Poly1305 champ par champ des colonnes sensibles
  (B-3)` — dépend de `rusqlite`. `docs/synthese/06-securite.md` (et le
  tableau repris en `03-ecarts-conception.md` pour l'US-112) attend que la
  clé privée du nœud (donc, transitivement, celle qui protège les colonnes
  chiffrées) vive de préférence en Keystore/Keychain, gérée par `identity`.
- **Code :** `crates/dengon-core/src/store.rs` — `Store<K: KeySource>` est
  paramétré par un trait `KeySource` (une méthode `field_key(&self) -> [u8;
  32]`), pas câblé sur une vraie dérivation Keystore/Keychain.
  `FixedKeySource` (clé fixe codée en dur) sert de bouchon pour les tests.
- **Pourquoi :** `identity` (US-205), qui génère et garde la vraie clé, est
  dans le **même sprint** (S2) que `store` (US-207) — même raison que
  l'écart symétrique sur `ledger::Signer` (US-206, voir l'entrée
  précédente) : la règle du projet interdit qu'une US dépende d'une autre
  US du même sprint. Les deux US sont d'ailleurs attribuées à des personnes
  différentes ce sprint (`docs/suivi/repartition-sprint2.md`).
- **Conséquences :** le **mécanisme** de chiffrement est réel et vérifié
  (XChaCha20-Poly1305, nonce aléatoire par appel, test négatif qui prouve
  qu'un message écrit n'apparaît pas en clair dans le fichier `.db`) — ce
  n'est pas un bouchon qui ne chiffre rien. Ce qui est un bouchon, c'est la
  **clé** : `FixedKeySource` est une clé fixe, connue de quiconque lit le
  code source. Tant qu'`identity` ne fournit pas une vraie clé dérivée
  (idéalement jamais lisible en clair par l'application elle-même, via
  Keystore/Keychain), les colonnes chiffrées ne protègent que contre une
  lecture accidentelle du fichier `.db`, pas contre un attaquant qui a lu
  le code source.
- **Doc de conception mise à jour ?** Non — la conception reste la cible
  réelle (clé dérivée, gérée par `identity`/Keystore). Documenté ici et
  dans `modules/dengon-core.md`.
### `ledger` : signature différée derrière un trait `Signer` (US-206)

- **Conception :** `04-architecture.md` §2 dit `ledger : append(event) ->
  Entry, verify_chain(), export(range)` — dépend de `crypto`. Chaque entrée
  du journal (`docs/powl/09-data-model.md` §1) porte un champ `sig` (Ed25519,
  64 o).
- **Code :** `crates/dengon-core/src/ledger.rs` — `Ledger<S: Signer>` est
  paramétré par un trait `Signer` (une méthode `sign(&mut self, message:
  &[u8]) -> Signature`), pas câblé sur `ed25519-dalek` ou toute autre
  implémentation concrète. `NullSigner` (signature à zéro) sert de bouchon
  pour les tests. `verify_chain()` ne vérifie **pas** la signature — il ne
  vérifie que la chaîne de hash et l'absence de trou/fork.
- **Pourquoi :** `crypto` (US-203, Ed25519) est dans le **même sprint**
  (S2) que `ledger` (US-206), et la règle du projet interdit qu'une US
  dépende d'une autre US du même sprint (`docs/olivier/proposition-organisation-github.md`
  §5.2). Attendre `crypto` aurait bloqué `ledger` sans raison de fond — les
  deux US sont attribuées à des personnes différentes ce sprint (voir
  `docs/suivi/repartition-sprint2.md`) et n'ont aucune raison de se
  séquencer.
- **Conséquences :** la **forme** du contrat (un champ `sig` de 64 octets
  par entrée, une méthode qui vérifie la chaîne) est déjà correcte et
  stable. Le **contenu** cryptographique ne l'est pas : une entrée signée
  par `NullSigner` ne prouve rien, et `verify_chain() == Verdict::Ok`
  aujourd'hui ne garantit **que** l'intégrité du hash-chaînage, pas
  l'authenticité de l'auteur. Quand `crypto` livrera une vraie
  implémentation `Signer` (Ed25519), elle se branchera sur `Ledger<S>` sans
  changer sa forme — et `verify_chain()` devra alors être étendue pour
  vérifier la signature de chaque entrée, ce qui n'est pas fait ici.
  **Mise à jour (US-203, retour de revue #78) :** `crypto::SigningKey`
  implémente désormais `ledger::Signer` (test `signe_le_journal_chaine`).
  `verify_chain()` ne vérifie toujours pas les signatures.
- **Doc de conception mise à jour ?** Non — la conception reste la cible
  réelle (signature Ed25519 vérifiée). Le point est documenté ici et dans
  `modules/dengon-core.md` (« Décisions d'implémentation » et « Limites
  connues »).

---

### `ledger::verify_chain()` ne peut pas re-vérifier un export partiel (US-206, retour de revue #75)

- **Conception :** `04-architecture.md` §2 dit `ledger : ... export(range)`
  et `dengon-verify` (`crates/dengon-verify/src/main.rs`, doc de module)
  affiche l'intention de « lire un export de journal ... et rendre l'un des
  quatre verdicts ».
- **Code :** `Ledger::export(range)` renvoie n'importe quelle tranche
  `seq ∈ range` des entrées en mémoire. `Ledger::verify_chain()`, lui,
  suppose toujours que la chaîne fournie démarre à `seq = 0` avec
  `prev_hash == GENESIS_HASH` : reconstruire un `Ledger` via
  `from_entries()` à partir d'un export dont `range` ne commence pas à 0
  (ex. `export(3..6)`) fait donc rapporter `Gap` ou `Broken` par
  `verify_chain()`, même si la tranche exportée est parfaitement intègre.
- **Pourquoi :** au moment d'écrire `ledger`, il n'existait aucun appelant
  réel de `export()` en dehors des tests (`dengon-verify::main` n'est pas
  encore implémenté) — la question « comment vérifier une tranche qui ne
  part pas de la genèse » n'avait donc pas de cas d'usage concret pour
  trancher la bonne API (un point d'ancrage en paramètre de
  `verify_chain` ? un `export` qui redémarre sa propre chaîne de hash
  depuis l'ancre ?).
- **Conséquences :** tant que `dengon-verify` (ou tout autre appelant) n'a
  besoin que de vérifier un export **complet** depuis `seq = 0` (le cas
  couvert par les tests actuels), rien n'est cassé. Le jour où un besoin
  réel de vérifier un export partiel apparaît (ex. le dashboard ne
  redemande que les entrées manquantes plutôt que tout le journal),
  `verify_chain()` devra être étendu avant de pouvoir servir tel quel.
- **Doc de conception mise à jour ?** Non — documenté ici et dans le
  docstring d'`export()` (`src/ledger.rs`), à trancher quand
  `dengon-verify` aura un vrai appelant.

---

### Vérification Ed25519 stricte (`verify_strict`)

- **Conception :** `06-securite.md` §8 impose `ed25519-dalek` v2 mais ne dit rien
  du mode de vérification.
- **Code :** `crates/dengon-core/src/crypto.rs`, `VerifyingKey::verify` appelle
  `verify_strict`.
- **Pourquoi :** `verify_strict` rejette en plus les clés de faible ordre et les
  signatures malléables. Le journal chaîné est un objet d'audit : accepter deux
  signatures distinctes d'un même message, ou une clé qui « valide » n'importe quoi,
  serait un défaut. Constaté par test : le point neutre (`y = 1`) se décode sans
  erreur mais ne valide aucune signature.
- **Conséquences :** une implémentation tierce (dashboard Python, `dengon-verify`)
  qui vérifierait en mode non strict acceptera des signatures que le cœur rejette,
  jamais l'inverse. À garder en tête pour le job `cross-vectors` (US-222) : les
  vecteurs partagés doivent inclure des cas de rejet.
- **Doc de conception mise à jour ?** Non — à verser dans `06-securite.md` §8 si
  l'équipe valide le choix.

---

### Signatures sur octets bruts, sans séparation de domaine

- **Conception :** rien n'est spécifié. Les signatures portent sur des octets
  bruts (préfixe de paquet, `entry_hash`, JSON canonique d'un batch). Le seul
  préfixe de domaine de la conception est `"dengon-tag"` (HMAC `recipient_tag`, D-2).
- **Code :** `SigningKey::sign(&self, message: &[u8])` signe exactement les octets
  reçus.
- **Pourquoi :** ne pas inventer un format de signature que ni le ledger (US-206)
  ni le dashboard (US-216) n'attendent, sous peine de casser l'interopérabilité
  entre Rust et Python.
- **Conséquences :** c'est à l'appelant de garantir qu'un message signé pour un
  usage (ex. batch) ne peut pas être rejoué comme un autre (ex. entrée de journal).
  Aujourd'hui les formats sont assez distincts pour que ce soit peu probable, mais
  ce n'est pas garanti par construction.
- **Doc de conception mise à jour ?** Non — question ouverte à poser en réunion.
### 2026-09-28 — Codec : `FRAGMENT` ⇔ type `0x09` imposé dans les deux sens (US-201)

- **Prévu :** `docs/synthese/05` §3.1 définit le bit `FRAGMENT` (« le payload
  est un fragment ») et §4 le type `0x09 FRAGMENT`, sans dire explicitement
  qu'ils vont ensemble.
- **Réel :** `protocol::codec` (`FrameRule::FragmentFlag`) refuse un paquet de
  type `0x09` sans `FRAGMENT`, et un `FRAGMENT` posé sur un autre type.
- **Raison :** deux façons de dire « ceci est un fragment » qui divergent
  laisseraient le réassemblage (US-202) et le routage choisir chacun la
  sienne. Tous les vecteurs v0 respectent déjà la règle.
- **Conséquences :** aucune sur les vecteurs. Si une v2 veut fragmenter
  autrement (fragment « dans » un autre type), il faudra lever la règle.
- **Doc de conception mise à jour ?** non — précision d'implémentation.

---

### 2026-09-28 — Constat (non tranché) : la signature couvre `ttl`, qu'un relais décrémente

- **Prévu :** `docs/powl/03` §3 / `synthese/05` §3 : signature Ed25519 « sur
  octets `[0 .. début_signature]` », donc **y compris l'octet `ttl`**
  (offset 2). `docs/oswin/02-securite-messages.md:80` le justifie (« pour
  qu'un relais ne puisse pas trafiquer le TTL »). Mais `synthese/05` §6
  fait **décrémenter** le TTL à chaque relais.
- **Réel :** le codec (US-201) implémente la spec à la lettre :
  `signed_len(&header)` = en-tête complet + payload. Un paquet signé relayé
  une fois (TTL - 1) **ne se vérifie plus** avec cette définition.
- **Raison :** ce n'est pas au codec de trancher ; relevé en l'implémentant.
- **Conséquences :** bloquant pour US-203 (vérification) + US-209 (relais) sur
  les types signés (`ANNOUNCE`, `SEALED_ENVELOPE`, `LOG_ATTEST`,
  `INVENTORY`…). Options vues : (a) signer avec l'octet `ttl` mis à 0 (seule
  modification locale, `signed_len` inchangé) ; (b) exclure `ttl` de la zone
  signée ; (c) signature d'origine + TTL non protégé (un relais malveillant
  peut alors le remonter — borné par la dédup). **À trancher en équipe avant
  US-203/US-209.**
- **Doc de conception mise à jour ?** non — décision d'équipe requise.
- **Mise à jour 2026-09-28 (revue PR #80, point bloquant de Paul) : tranché,
  option (a).** L'octet `ttl` est mis à 0 dans l'entrée de signature
  (`signing_input` / `received_signing_input`, `protocol::codec`) ;
  `signed_len` n'est plus public. `docs/synthese/05` §3 corrigé ;
  `docs/powl/03` §3 laissé tel quel (matière première figée) — c'est donc
  désormais un **écart** entre le code et `powl/03`, dans ce sens : la zone
  signée exclut le TTL. Le TTL n'est plus protégé contre un relais
  malveillant, ce que borne la dédup du seen-set.

---

### Padding : préfixe de longueur `u16` au lieu de PKCS#7 (US-204)

- **Conception :** `05-protocole-et-trame.md` §3.1 (flag `PADDED`) et
  `06-securite.md` §3 : payload « complété par du PKCS#7 » vers la borne
  supérieure de `PAD_BUCKETS = [256, 512, 1024, 2048]`.
- **Code :** `crypto::pad` (`crates/dengon-core/src/crypto/pad.rs`) :
  `len(u16 BE) ‖ données ‖ 0x00…` jusqu'au plus petit bucket ≥ `len + 2`.
  Clair utile maximal = 2046 octets. Le padding est appliqué au **clair,
  avant** chiffrement Noise (donc authentifié et invisible pour un relais) ;
  chiffré de session = bucket + 24 (nonce 8 + tag 16), enveloppe `X` =
  bucket + 96. Les messages de handshake `XX` ne sont **pas** paddés (écart
  suivant).
- **Pourquoi :** PKCS#7 code la longueur du bourrage sur **un** octet
  (1 à 255). Passer de 513 à 1024 octets demande jusqu'à 511 octets de
  bourrage, de 1025 à 2048 jusqu'à 1023 : impossible à encoder. La spec était
  donc inapplicable pour les deux derniers buckets. Choix validé par Paul
  (2026-09-28) contre l'alternative ISO/IEC 7816-4 (`0x80` puis zéros).
- **Conséquences :** toute autre implémentation (firmware, vecteurs
  `cross-vectors`) doit suivre ce format — il est figé dans
  `crates/dengon-core/tests/vectors/crypto_v0.json`. La signification exacte du
  flag `PADDED` dans l'en-tête (le padding étant à l'intérieur du chiffré, il
  est toujours présent sur `NOISE_*`/`SEALED_ENVELOPE`) reste à préciser par
  `protocol::codec` (US-201).
- **Doc de conception mise à jour ?** Non — à reporter dans `06-securite.md` §3
  et `05-protocole-et-trame.md` §3.1 (PR doc séparée ou #66).

---

### Messages de handshake `XX` non paddés, payload interdit au message 1 (US-204, revue #81)

- **Conception :** `06-securite.md` §3 (l.164) : « tout paquet `NOISE_MSG` /
  `NOISE_HS` / `SEALED_ENVELOPE` est complété […] à la borne supérieure de
  `PAD_BUCKETS` ».
- **Code :** `crypto::noise::Handshake::write_message` / `read_message` ne
  passent plus par `pad`/`unpad`. Un handshake `XX` fait 32 + 96 + 64 =
  **192 octets** (au lieu de 288 + 352 + 320 = 960). Le message 1 refuse tout
  payload (`CryptoError::PayloadNotAllowed`, à l'écriture comme à la lecture).
- **Pourquoi :** relevé en revue de #81 (OswinFreyr). (1) Le message 1
  (`-> e`) part **en clair** : son payload était lisible par tout relais,
  padding ou non (vérifié : `b"SECRETPAYLOAD"` retrouvé tel quel dans la
  trame). (2) Avec des payloads vides, les tailles sont déjà fixées par le
  motif : le padding n'apportait rien et coûtait 768 octets par handshake, soit
  plusieurs fragments BLE. Choix validé par Paul (2026-09-28).
- **Conséquences :** un payload non vide aux messages 2 ou 3 a une taille
  visible. Le message 2 est chiffré mais vers un initiateur pas encore
  authentifié. Tout ce qui doit rester confidentiel (version, capacités,
  inventaire de `sync`) passe par la `Session`. Vecteurs
  `noise_xx.handshake` de `crypto_v0.json` régénérés (transport et enveloppe
  inchangés) et recoupés avec `noiseprotocol` (Python), identiques octet par
  octet.
- **Doc de conception mise à jour ?** Non — à reporter dans 06 §3 : retirer
  `NOISE_HS` de la liste des paquets paddés.

---

### `snow` 0.10.0 n'efface aucune clé (US-204, revue #81)

- **Conception :** `06-securite.md` demande d'effacer les secrets de la
  mémoire après usage.
- **Code :** `StaticKeypair` efface **sa** copie du secret (`Drop` +
  `zeroize`). Mais `snow` 0.10.0 n'a ni `Drop` ni `zeroize` : le secret
  statique recopié dans chaque `HandshakeState` (`local_private_key`), la clé
  éphémère et les clés des `CipherState` restent en mémoire après
  destruction. La clé publique de `StaticKeypair::from_secret` est maintenant
  calculée avec `curve25519-dalek` (`MontgomeryPoint::mul_base_clamped`, la
  crate que `snow` utilise déjà), ce qui supprime la copie temporaire dans un
  `Dh` de `snow`.
- **Pourquoi :** pas d'alternative Noise `no_std` maintenue qui efface ses
  clés ; forker `snow` n'est pas raisonnable pour le MVP.
- **Conséquences :** un attaquant capable de lire la mémoire du processus
  (dump, swap non chiffré) peut retrouver des clés après la fin d'une session.
  Hors du modèle de menace MVP (pas d'adversaire local), mais à ne pas
  présenter comme « secrets effacés ». La doc de `StaticKeypair` et du module
  le dit.
- **Doc de conception mise à jour ?** Non — à mentionner dans 06 (limites).

---

### `recipient_tag` : `epoch_day` manipulé en `u16`, encodé `u32` BE dans le HMAC (US-204)

- **Conception :** `06-securite.md` §3 écrit `"dengon-tag" ‖ day_u32` sans
  préciser l'endianness ; le champ `epoch_day` de `SEALED_ENVELOPE` fait
  2 octets.
- **Code :** `crypto::tag::recipient_tag(pub_static, day: u16)` encode
  `u32::from(day)` en **big-endian** (endianness de toute la trame, 05 §3).
  `epoch_day(ts_ms)` sature à `u16::MAX` (an 2149).
- **Pourquoi :** précision nécessaire pour l'interopérabilité ; big-endian est
  la convention du protocole.
- **Conséquences :** recoupé en Python (`hmac` + `hashlib`) sur les vecteurs.
- **Doc de conception mise à jour ?** Non — à préciser dans 06 §3.

---

### Hors module `crypto` : signature d'enveloppe, décision TOFU, `2^n` rekey (US-204)

- **Conception :** `06-securite.md` §3 : l'enveloppe est signée Ed25519 ; la
  `pub_static` reçue en `XX` doit correspondre au contact (sinon rejet +
  alerte) ; re-négociation après `2^n` messages.
- **Code :** `crypto::noise` expose `seal`/`open` (Noise `X` brut) et
  `Session::remote_static()`, mais **ne signe pas** l'enveloppe, **ne compare
  pas** la clé au contact et **ne compte pas** les messages pour re-négocier.
  `open` n'a **aucun anti-rejeu** (inhérent au one-shot `X`) : un relais peut
  réinjecter la même `SEALED_ENVELOPE` indéfiniment et elle s'ouvrira à chaque
  fois (revue #81).
- **Pourquoi :** ce sont des décisions de trame (US-201/US-208) et de
  confiance (`identity`, US-205 ; `sync`), qui ont besoin d'un état que
  `crypto` n'a pas.
- **Conséquences :** les US consommatrices doivent le faire ; noté dans la
  fiche `dengon-core`. La déduplication des enveloppes se fait par `msg_id`
  dans `sync` / `store`.
- **Doc de conception mise à jour ?** Sans objet.

---

### `NOISE_MSG` : nonce explicite de 8 octets et fenêtre anti-rejeu (US-204, revue)

- **Conception :** `05-protocole-et-trame.md` §4 : `NOISE_MSG` = « ciphertext
  Noise (transport) », sans champ nonce ; §6.1 : `NOISE_MSG` est du trafic
  dirigé **relayé** (`ttl-1`) ; `06-securite.md` §1 : un relais peut
  « jeter/dupliquer/réordonner ».
- **Code :** `crypto::noise::Session` utilise `snow::StatelessTransportState` ;
  chiffré = `nonce(u64 BE) ‖ ChaCha20-Poly1305(padded)` (bucket + 24 o au lieu
  de bucket + 16) ; fenêtre anti-rejeu de 64 nonces, mise à jour après
  authentification.
- **Pourquoi :** avec le transport Noise standard (nonce implicite), une seule
  perte ou inversion désynchronise la session pour toujours — constaté par test
  lors de la revue (message 1 perdu → messages 2 et 3 en `Err`). Incompatible
  avec un transport multi-sauts. Même solution que WireGuard / DTLS.
- **Conséquences :** 8 octets de plus par `NOISE_MSG` ; la fenêtre tolère
  jusqu'à 64 messages de désordre, au-delà le message est perdu (le protocole
  a de toute façon des ACK et des retransmissions, 07). Le nonce en clair
  révèle à un relais le rang du message dans la session (pas son contenu).
- **Doc de conception mise à jour ?** Non — à reporter dans 05 §4 (format du
  payload `NOISE_MSG`).

---

### 2026-09-28 — Code de vérification : 5 octets par groupe au lieu d'un `u16` (US-205)

- **Prévu :** `docs/powl/04-security.md` §2.3, `docs/synthese/06-securite.md`
  §2 et `docs/synthese/09-dashboard-et-donnees.md` §11.3 :
  `groupe_i = u16_be(material[2i..2i+2]) % 100000`.
- **Réel :** `groupe_i = u40_be(material[5i..5i+5]) % 100000`
  (`crates/dengon-core/src/identity/safety.rs:80`). On consomme 60 des 64
  octets du SHA-512. Le reste de la formule ne change pas : `min`/`max` des
  empreintes, SHA-512, 12 groupes de 5 chiffres complétés par des zéros.
- **Raison :** un `u16` ne dépasse pas 65 535, donc `% 100000` ne fait rien.
  Aucun groupe ne peut commencer par 7, 8 ou 9, et chaque groupe « de 5
  chiffres » ne porte que 16 bits. Avec 40 bits par groupe, comme dans le
  safety number de Signal, le biais du modulo devient négligeable. La
  sécurité de la formule d'origine restait correcte (12 × 16 = 192 bits),
  mais un code dont 30 % des valeurs sont impossibles, c'est une question
  certaine à l'oral. Décision de Paul, 2026-09-28.
- **Conséquences :** tout client non-Rust (Kotlin, Python) doit suivre la
  nouvelle formule ; les vecteurs `tests/vectors/identity_v0.json` font
  référence. Rien d'autre n'était encore codé.
- **Doc de conception mise à jour ?** Oui : `docs/synthese/06-securite.md`
  §2 et `docs/synthese/09-dashboard-et-donnees.md` §11.3. `docs/powl/04`
  reste inchangé, comme matière d'origine.

---

### 2026-09-28 — Coffre d'identité : trait `Vault` et clé fournie par l'appelant (US-205)

- **Prévu :** `docs/synthese/06-securite.md` §2 : les clés sont stockées
  « dans le coffre de la plateforme (Android Keystore / Secret Service / NVS
  chiffrée ESP32) ». `09` §11.1 prévoit la table `identity`, avec
  `priv_static`/`priv_sign` chiffrés.
- **Réel :** `identity` chiffre lui-même l'identité en un blob
  XChaCha20-Poly1305 (`Identity::seal`/`unseal`, `vault.rs:74`/`:117`). Ce
  blob est rangé derrière un trait `Vault` (`vault.rs:161`), avec deux
  implémentations : `MemoryVault` et `FileVault`. La clé de 32 octets est
  **fournie par l'appelant**. Aucun coffre plateforme n'est branché.
- **Raison :** le Keystore et Secret Service ne sont joignables que depuis
  `dengon-ffi` (Kotlin) et `dengon-node`. De son côté, `store` (US-207,
  PR #76) est du même sprint, et la règle d'or interdit d'en dépendre. On
  reprend le principe de son `KeySource`, sans le partager.
- **Conséquences :** l'US d'intégration devra choisir entre deux options :
  le blob dans `FileVault` et la clé dans le Keystore, ou bien les secrets
  dans la table `identity` de `store`. La première est la plus simple : un
  seul appel à `load_or_create`.
  **Mise à jour (rebase de #82 sur `main`, 2026-09-28) :** `store` est
  maintenant sur `main`, donc les **deux** rangements de l'identité au repos
  existent dans le code : `Identity::seal` + `Vault` (blob unique, clé de
  l'appelant) et `Store::set_identity` / `get_identity_private_keys`
  (colonnes `priv_static`/`priv_sign` chiffrées par `KeySource`, pseudo et
  clés publiques en clair dans la table). Aucun appelant n'utilise encore
  l'un ou l'autre. À unifier dans l'US d'intégration (US-301/302) : garder
  un seul chemin, et faire fournir la clé du coffre et celle de `store` par
  la même source plateforme.
- **Doc de conception mise à jour ?** Non (la cible plateforme reste valable).

---

### 2026-09-28 — QR : base64url sans padding, pseudo de 1 à 255 octets (US-205)

- **Prévu :** `09` §11.3 : `dengon:v1:<base64url( B )>`, avec
  `B = pseudo_len:u8 ‖ pseudo:utf8 ‖ …`. Le padding et le pseudo vide ne
  sont pas précisés.
- **Réel :**
  - base64url **sans** `=` et **canonique** (bits de fin nuls vérifiés) : un
    QR avec padding est refusé (`QrEncoding`) ;
  - pseudo UTF-8 de **1 à 255 octets**, le pseudo vide étant refusé
    (`InvalidPseudo`).
  (`crates/dengon-core/src/identity/qr.rs:58`, `keys.rs:27`.)
- **Raison :** avec un encodage unique par carte, la comparaison de deux QR
  reste sûre et l'aller-retour est exact dans les deux sens. Un pseudo vide
  n'a pas de sens à l'affichage, et 255 est la limite imposée par le `u8`.
- **Conséquences :** l'app Android doit encoder sans padding
  (`Base64.URL_SAFE or NO_PADDING or NO_WRAP`).
- **Doc de conception mise à jour ?** Non (précision, pas contradiction).

---

### 2026-09-28 — Déploiement sur ports 8080/8443, pas 80/443 (US-224)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §7 décrit un
  reverse-proxy classique devant `uvicorn`, sans préciser de port — l'usage
  implicite pour un reverse-proxy TLS est 80/443.
- **Réel :** Caddy publie 8080 (HTTP) et 8443 (HTTPS) sur l'hôte
  (`dashboard/deploy/docker-compose.yml`).
- **Raison :** en investiguant le VPS attribué au groupe (US-224), les ports
  80/443 se sont révélés déjà occupés par un processus **root** — confirmé
  via `/proc/net/tcp` (uid du socket = 0) — alors qu'aucun conteneur Docker
  visible (`docker ps -a`, qui montre pourtant des conteneurs d'autres
  groupes du cours) ne les publie. Le VPS est **partagé**, sans accès
  `sudo` pour nous ; prendre 80/443 nous-mêmes est impossible sans risquer
  de casser ou d'entrer en conflit avec ce processus système, dont nous ne
  connaissons ni le rôle ni le propriétaire.
- **Conséquences :** `GET /healthz` reste joignable en HTTPS depuis
  l'extérieur (critère d'acceptation de l'issue #38), juste pas sur le port
  443 standard — une URL de démo doit préciser `:8443`. Si l'équipe obtient
  un jour un accès `sudo` ou une convention documentée pour ce VPS partagé,
  reprendre 80/443 est un changement de deux lignes dans
  `docker-compose.yml`.
- **Doc de conception mise à jour ?** non — `docs/synthese/09` ne
  spécifiait pas de port ; documenté ici et dans
  `docs/suivi/modules/deploiement-vps.md`.

---

### 2026-09-28 — TLS auto-signé (CA interne Caddy), pas Let's Encrypt (US-224)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §7 : « le
  reverse-proxy obtient le certificat TLS pour `dashboard.<domaine>` » —
  implicitement un certificat public (Let's Encrypt étant l'option standard
  et gratuite pour ce cas).
- **Réel :** `dashboard/deploy/Caddyfile` utilise `tls internal` : Caddy
  émet un certificat depuis sa propre CA locale, jamais soumis à une
  autorité publique.
- **Raison :** aucun nom de domaine n'est disponible pour ce VPS — identifié
  uniquement par son IP (`51.255.38.214`). Let's Encrypt (HTTP-01 et
  TLS-ALPN-01, les deux défis que Caddy sait automatiser) exige un nom
  d'hôte résolvable, pas seulement une IP.
- **Conséquences :** `/healthz` est bien joignable en HTTPS (chiffré), mais
  un vrai navigateur affiche un avertissement de sécurité (certificat non
  reconnu) — à anticiper pour la démo (importer la CA interne à l'avance,
  ou simplement cliquer « continuer »). Si l'équipe obtient un nom de
  domaine pointant vers ce VPS, remplacer `tls internal` par l'adresse du
  domaine (Caddy gère alors Let's Encrypt automatiquement) est un
  changement d'une ligne.
- **Doc de conception mise à jour ?** non — `docs/synthese/09` visait un
  déploiement avec domaine, non disponible ici ; documenté dans
  `docs/suivi/modules/deploiement-vps.md`.
### 2026-09-28 — `sync::routing` : un doublon pendant le jitter n'annule plus le relais, il en faut deux (US-209)

- **Prévu :** `docs/synthese/05-protocole-et-trame.md` §6.1 et `docs/powl/03`
  §7.1 : « reçu en double pendant l'attente ? — oui → abandonner le relais »,
  soit un seuil de **1** doublon.
- **Réel :** `RoutingConfig::dup_cancel_threshold`, par défaut
  `DUP_CANCEL_THRESHOLD = 2` (`crates/dengon-core/src/sync/routing.rs:87`).
  `1` reste disponible (règle littérale), `0` désactive l'annulation.
- **Raison :** mesurée, pas supposée. Dans un losange A→{B,C}→D→E, D reçoit
  la copie de B, programme son relais, entend la copie de C pendant son
  jitter et s'abstient : **E ne reçoit jamais le message**. Le test
  `seuil_litteral_affame_le_losange` (`crates/dengon-core/tests/routing_mock.rs`)
  le reproduit à chaque exécution ; `plusieurs_chemins_un_seul_relais_par_noeud`
  montre qu'avec 2, E est servi et chaque nœud relaie au plus une fois. C'est
  le défaut connu du schéma « à compteur » avec un seuil de 1 (Ni et al.,
  *The broadcast storm problem*, 1999, recommandent 3 à 4).
- **Conséquences :** un peu plus de relais redondants en zone dense (au plus
  un par nœud et par `msgID`, la dédup ne change pas). Un seuil de 2 ne
  garantit pas tout : un nœud qui a 3 entrées et une seule sortie peut encore
  s'abstenir — la réconciliation d'inventaire (US-210) reste le filet.
  **À valider à trois** (point d'équipe) ; revenir à 1 = une ligne.
- **Doc de conception mise à jour ?** non — à faire si l'équipe valide.

---

### 2026-09-28 — `sync::routing` : routeur sans I/O, générique sur le lien, au lieu d'appeler `Transport` (US-209)

- **Prévu :** l'issue #23 : « s'écrit contre `MockTransport` (US-105) » ;
  `docs/synthese/04` §2 place le routage dans `dengon-core::sync`.
- **Réel :** `Router<L>` ne connaît pas `Transport`. Il prend un en-tête
  décodé et rend une `Decision` ; `poll_due` rend des `RelayOrder<L>` que
  l'appelant envoie. `L` = `dengon_ble::LinkId` côté appelant. Le test de
  bout en bout contre `MockTransport` est dans `dengon-core/tests/`, avec
  `dengon-ble` en **dev-dependency** de `dengon-core`.
- **Raison :** `dengon-ble` dépend de `dengon-core` (et de `std`) : importer
  `Transport` dans `dengon-core` créerait un cycle et casserait le `no_std`.
  Le cycle limité aux dev-dependencies est accepté par Cargo.
- **Conséquences :** la boucle d'événements (poll du transport → routeur →
  send) est à écrire par chaque hôte (`dengon-node`, `dengon-sim`, FFI).
  Elle fait ~30 lignes (`Noeud::tick` dans `tests/routing_mock.rs`).
- **Doc de conception mise à jour ?** non.

---

### 2026-09-28 — `sync::routing` : trois réglages sans constante de conception (US-209)

- **Prévu :** `synthese/05` §6.4 : « quota par `peerID` et par lien
  (paquets/s) » ; §6.1 : broadcast « TTL faible (2–3) ». Aucune valeur dans
  `protocol::consts`.
- **Réel :** constantes **du routeur** (pas du contrat `protocol::consts`) et
  champs de `RoutingConfig` : `LINK_MAX_PKT_PER_S = 50` (quota brut par
  lien, doublons compris, fenêtre 1 s), `BROADCAST_TTL_MAX = 3` (TTL relayé
  max. pour `ANNOUNCE` / `LOG_ATTEST`), `DUP_CANCEL_THRESHOLD = 2` (voir
  l'écart ci-dessus). Le quota « par `peerID` » n'est pas distinct du quota
  par lien : un lien = un pair au MVP. **Corrigé en revue #85** : voir
  l'entrée « anti-inondation par `peerID` » plus bas.
- **Raison :** `protocol::consts` est un contrat « revue à trois » ; ces
  valeurs ne sont pas lues par les autres implémentations (firmware,
  dashboard) et peuvent varier sans casser l'interopérabilité.
- **Conséquences :** à promouvoir dans `consts` si le firmware doit les
  partager.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-28 — `sync::routing` : la tolérance d'horloge ±2 h ne s'applique que vers le futur (US-209)

- **Prévu :** `synthese/05` §6.4 : « `timestamp_ms` hors fenêtre ±2 h →
  rejeté ».
- **Réel :** futur au-delà de `now + TIMESTAMP_TOLERANCE_MS` → `ClockSkew` ;
  passé : rejet seulement au-delà de `MSG_TTL_S` (24 h) → `Expired`.
- **Raison :** appliquée vers le passé, la fenêtre ±2 h rejetterait tout
  message porté plus de 2 h en store-and-forward, alors que la même
  conception lui donne 24 h de vie (§6.5, §7 étape 2). Les deux règles se
  contredisent ; on garde celle qui permet le DTN. L'anti-rejeu reste
  assuré par le seen-set + `msgID`.
- **Conséquences :** un paquet rejoué entre 5 min (`SEEN_TTL_S`) et 24 h
  après son premier passage peut être accepté une seconde fois par un
  nœud qui l'a oublié. Déjà le cas dans la conception (seen-set à 300 s) ;
  `conv_seq` (couche applicative) le rattrape côté destinataire.
  **Mise à jour (revue #85, point 2)** : ce second passage n'est plus
  **relayé** — voir l'entrée « horizon du seen-set » ci-dessous.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-28 — `sync::routing` : un paquet plus vieux que l'horizon du seen-set est accepté mais pas relayé (US-209, revue #85 point 2)

- **Prévu :** `synthese/05` §6.1 : seen-set de `SEEN_TTL_S` = 300 s ; §6.5 /
  §7 : un message vit `MSG_TTL_S` = 24 h. La conception ne dit pas ce qui
  arrive quand un porteur revient après l'oubli du seen-set.
- **Réel :** si l'âge du paquet (horloge murale − `timestamp_ms`) atteint
  `seen_ttl_ms`, le routeur l'accepte (`Deliver` s'il est pour moi, `Store`
  pour une enveloppe, `NoRelay(Late)` sinon) mais **ne le relaie pas**.
  L'entrée du seen-set vit jusqu'à `max(réception, horodatage) +
  SEEN_TTL_S` (index trié par échéance), donc un paquet encore relayable
  est toujours reconnu, même horodaté jusqu'à 2 h dans le futur.
- **Raison :** signalé en revue par `OswinFreyr` : sans cette règle, chaque
  retour d'un porteur (1 h après, par ex.) relançait un flood complet chez
  tous les nœuds qui l'avaient déjà relayé, et le destinataire recevait un
  second `Deliver`. Aligner le seen-set sur 24 h coûterait trop de mémoire
  (ESP32 sans PSRAM) ; un filtre de Bloom 24 h ajoute des faux positifs
  (messages perdus). Ne relayer que du frais borne le coût à zéro flood
  supplémentaire, et le transport du tardif passe par la livraison directe
  ou le dépôt (`sync::courier`, US-212).
- **Conséquences :** (1) un `Deliver` peut se **répéter** au-delà de
  l'horizon : la dédup longue durée des messages livrés revient au `store`
  (clé `msgID`) — à brancher à l'intégration (US-211). (2) Un message non
  scellé (`NOISE_MSG`) porté plus de 5 min ne progresse plus en multi-saut :
  il n'atteint son destinataire que par contact direct avec le porteur.
  Le store-and-forward multi-saut passe par `SEALED_ENVELOPE`. (3) Un
  recul de l'horloge murale entre deux passages peut encore laisser passer
  un relais de plus (entrée expirée en monotone, paquet redevenu « frais »
  en mural) : un par saut d'horloge, borné.
- **Doc de conception mise à jour ?** non — **à valider à trois** avec le
  seuil de doublons ; à reporter dans `synthese/05` §6.1 si retenu.

---

### 2026-09-28 — `sync::routing` : anti-inondation par `peerID` du voisin, conservé après déconnexion (US-209, revue #85 point 1)

- **Prévu :** `synthese/05` §6.4 : `FLOOD_MAX_PER_MIN_PEER` nouveaux
  `msgID` par minute **par `peerID`**. La première version comptait par
  **lien** et supprimait la fenêtre à `link_down` ; l'écart « trois
  réglages » ci-dessus disait « un lien = un pair au MVP ».
- **Réel :** `Router::bind_peer(link, peer, mono_ms)` associe un lien au
  `peerID` authentifié du voisin. L'anti-inondation est alors compté par
  `peerID` (les `msgID` déjà comptés sur le lien sont reportés, plusieurs
  liens vers le même pair partagent le quota) et la fenêtre **survit à la
  déconnexion** jusqu'à se vider (60 s). Un lien jamais lié garde une
  fenêtre par lien.
- **Raison :** signalé en revue par `OswinFreyr` : `LinkId` est un compteur
  monotone, donc un voisin malveillant pouvait envoyer 20 `msgID`, se
  reconnecter, et repartir avec un quota neuf. Garder la fenêtre du lien
  après `link_down` n'aurait rien changé (le lien suivant a un autre
  `LinkId`) : il faut l'identité du pair, que le transport ignore
  volontairement (`LinkId` n'est pas un `peerID`).
- **Conséquences :** l'appelant doit appeler `bind_peer` dès qu'il a
  authentifié le voisin (handshake Noise, ou `ANNOUNCE` signé reçu en
  direct) — à brancher à l'intégration. Avant ce moment, un pair peut
  toujours contourner le quota en se reconnectant. Un attaquant qui change
  d'identité à chaque reconnexion (Sybil) n'est pas couvert non plus :
  c'est hors de portée d'un quota par voisin. La table des pairs est
  purgée à chaque `bind_peer` (pairs sans lien dont la fenêtre est vide).
- **Doc de conception mise à jour ?** non — c'est l'implémentation qui
  rejoint la conception.

---

### 2026-09-28 — `sync::routing` : deux horloges, murale et monotone (US-209, revue #85 point 3)

- **Prévu :** rien de précis ; la première version prenait un seul
  `now_ms` (UTC) pour tout.
- **Réel :** `Now { wall_ms, mono_ms }`. La murale ne sert qu'à comparer à
  `timestamp_ms` (`ClockSkew`, `Expired`, `Late`) ; quotas, seen-set et
  échéances du jitter suivent la monotone. `poll_due`, `next_deadline` et
  `RelayScheduled::at_ms` sont en temps **monotone**.
- **Raison :** signalé en revue par `OswinFreyr` : un recul de l'heure
  (réglage manuel, synchro réseau, ESP32 qui reçoit l'heure après son boot)
  gelait les relais en attente, empêchait les fenêtres de quota et le
  seen-set d'expirer, et bloquait un lien saturé pendant toute la durée
  du saut. Changer la signature coûte peu avant l'intégration.
- **Conséquences :** chaque hôte fournit les deux horloges
  (`Instant`/uptime + heure système sur desktop, `esp_timer_get_time` +
  heure SNTP sur ESP32).
- **Doc de conception mise à jour ?** non.

---

### `observability` : aucun site d'appel réel, catalogue Rust non vérifié automatiquement contre `catalogue.py` (US-208)

- **Conception :** `docs/synthese/04-architecture.md` §2 et
  `docs/powl/08-observability-events.md` décrivent `observability` comme le
  module qui **produit** les événements aux « bons endroits » —
  implicitement, depuis le code qui gère le trafic (`sync::routing`,
  `sync::inventory`, etc.) et la santé du nœud.
- **Code :** `crates/dengon-core/src/observability/` livre le
  **mécanisme** — `Envelope`, la redaction structurelle (`msg_log_id`), la
  sérialisation JSON canonique (vérifiée octet à octet contre 3 fixtures
  golden réelles de l'US-107) — et 4 constructeurs de payload
  représentatifs (`pkt_seen`, `pkt_relayed`, `msg_queued`,
  `peer_connected`) sur les 28 du catalogue. **Aucun appelant réel
  n'existe** : `sync::routing`/`sync::inventory` (US-209/US-210), qui
  produiraient réellement du trafic `pkt.*`, ne sont pas encore livrés à
  l'heure où ce module est écrit.
- **Pourquoi :** `observability` (US-208) n'a de dépendance formelle
  qu'envers `US-104`/`US-107` (sprint antérieur) — rien n'empêchait de
  l'écrire avant `sync`, et attendre `sync` (même sprint, US-209/US-210)
  aurait été une dépendance intra-sprint interdite par la règle du projet.
  Le module est donc écrit en **fournisseur de mécanisme**, prêt à être
  appelé dès que `sync` existe, plutôt qu'en essayant de deviner par
  avance la forme exacte des appels depuis un code qui n'existe pas
  encore.
- **Deuxième écart, apparenté :** `observability::catalog::EVENT_NAMES`
  (28 noms) a été comparé **manuellement** à
  `contracts/tools/catalogue.py::CATALOGUE` (28 clés des deux côtés,
  vérifié à l'écriture de ce module) — aucun outil ne garantit que les
  deux listes resteront synchronisées si l'une des deux évolue sans
  l'autre. Le job CI `cross-vectors` (US-222, pas encore livré) est
  l'endroit naturel pour l'automatiser, sur le même principe que
  `tests/protocol_vectors.rs` (comparaison structurelle contre des
  vecteurs partagés).
- **Conséquences :** aucun risque immédiat — la **forme** du contrat
  (l'enveloppe, la redaction, le JSON canonique) est déjà correcte et
  testée contre le vrai contrat Python. Ce qui manque est la
  **couverture** (24 événements sur 28 sans constructeur dédié) et
  l'**intégration** (aucun code ne les émet encore). Les deux sont des
  suites mécaniques une fois `sync` livré, pas des inconnues de conception.
- **Condition de levée :** quand `sync::routing`/`sync::inventory`
  arriveront (US-209/US-210), câbler les appels réels à
  `observability::*` et écrire les constructeurs manquants au fil de l'eau
  ; envisager la vérification cross-langage automatique au moment de
  US-222 (CI `cross-vectors`).
- **Doc de conception mise à jour ?** Non — le mécanisme correspond déjà à
  la conception, seule l'intégration reste à faire quand son code appelant
  existera.

---

### 2026-09-28 — `POST /api/nodes` sans authentification opérateur (US-216)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §7 décrit
  l'enregistrement d'un relais via `POST /api/nodes` comme une étape du
  déploiement, sans préciser qui a le droit de l'appeler.
- **Réel :** la route enregistre n'importe quel `node_id`/`pub_sign` reçu
  sans vérifier l'identité de l'appelant, et renvoie un JWT valide en
  retour — quiconque atteint l'API peut se créer un nœud whitelisté.
- **Raison :** l'US-216 couvre l'authentification des **nœuds** pour
  `/ingest/batch` (JWT + signature Ed25519), pas l'authentification d'un
  **opérateur humain** (session/cookie/admin). Aucune US du backlog actuel
  ne couvre ce second cas.
- **Conséquences :** acceptable pour une démo locale (B-4), mais c'est un
  vrai trou avant tout déploiement exposé (US-224, VPS) : n'importe qui sur
  le réseau peut fabriquer un nœud de confiance. À couvrir par une future US
  (auth admin) avant toute exposition publique.
- **Doc de conception mise à jour ?** non — signalé ici, dans le docstring
  de la route (`app/main.py`) et dans `modules/dashboard-api.md` (Limites).
- **Mise à jour 2026-09-28 (revue de la PR #91) :** le trou était en réalité
  plus grave que « se créer un nœud » — l'`ON CONFLICT(node_id) DO UPDATE`
  remplaçait la clé publique d'un nœud **déjà enregistré** et le
  re-whitelistait, y compris un nœud qu'un opérateur aurait retiré
  (prise de contrôle, pas seulement création). Corrigé : un `node_id` déjà
  pris renvoie désormais **409**, plus d'upsert. L'écart lui-même (pas
  d'auth opérateur sur `POST /api/nodes`, donc n'importe qui peut encore
  enregistrer un `node_id` **inédit**) reste entier et n'a pas de US pour
  le couvrir.

---

### 2026-09-28 — Nœud inconnu : rejet direct 401, pas la quarantaine décrite par la conception (US-216)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §3 décrit, pour un
  batch venant d'un `node_id` inconnu, une mise en **quarantaine** avec
  alerte opérateur (donc un état intermédiaire, pas un rejet).
- **Réel :** `app/ingest.py::_lookup_node_pub_sign` rejette directement avec
  un 401, message identique à « nœud connu mais non whitelisté » (pour ne
  pas renseigner un attaquant qui devine des `node_id`).
- **Raison :** aucun écran opérateur pour lever une quarantaine n'existe
  dans le périmètre de l'US-216 (ni d'aucune US actuelle) — une quarantaine
  sans moyen de la lever serait un état mort.
- **Conséquences :** un relais légitime pas encore enregistré via
  `/api/nodes` voit ses batchs rejetés (401) plutôt que mis en attente —
  l'opérateur doit enregistrer le nœud avant qu'il ne pousse des données,
  pas après coup.
- **Doc de conception mise à jour ?** non — à trancher si une US future
  ajoute un écran opérateur de gestion des nœuds.

---

### 2026-09-28 — `links`/`message_hops` (§11.2) non créées, `hop_count` approximatif (US-217)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §11.2 décrit, en
  plus de `messages`, deux tables : `message_hops` (un événement de saut par
  ligne — `node_id`, `kind`, `ttl_in`/`ttl_out`/`fanout`/`rssi`) et `links`
  (topologie dérivée de `peer.connected`/`disconnected`).
- **Réel :** seule `messages` existe (migration v3,
  `dashboard/api/app/migrations.py`). `messages.hop_count` est une simple
  approximation — le nombre d'événements `pkt.relayed` vus pour ce
  `msg_log_id`, tous nœuds confondus — pas un journal par nœud avec
  TTL/fanout/RSSI. `links` n'existe pas du tout.
- **Raison :** l'US-217 (« Dashboard — projections + reconstruction de
  statut ») porte formellement sur la reconstruction de **statut d'un
  message** (critères d'acceptation de l'issue #31), pas sur la topologie ni
  l'historique détaillé des sauts. Créer les deux tables sans US qui les
  remplit et les lit aurait été un bouchon vide, la même discipline que pour
  `store`/`ledger` (US-206/207).
- **Conséquences :** le dashboard peut afficher un statut de message et un
  compte de sauts grossier, mais pas encore « quel nœud a relayé ce message,
  quand, avec quel TTL » ni un graphe de topologie. Aucune US actuelle du
  backlog sprint 2 ne couvre `links`/`message_hops` — à planifier si l'écran
  « parcours d'un message » (US-219) en a besoin.
- **Doc de conception mise à jour ?** non — la cible reste `message_hops`/
  `links` tels que décrits par §11.2.

---

### 2026-09-28 — Statut `read` (v2) jamais produit par la projection (US-217)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §9 catalogue
  `msg.read`/`read.observed` (marqués *v2*) et §11.2 inclut `read` dans
  l'enum `messages.status`.
- **Réel :** `app/projections.py` ne traite que
  `queued`/`in_flight`/`delivered`/`expired`/`unknown` — exactement les
  statuts couverts par la table de déduction de §10. `msg.read`/
  `read.observed` sont ignorés (ni erreur, ni effet sur le statut) ; `read`
  reste déclaré dans la contrainte `CHECK` SQL (fidèle au schéma cible) mais
  aucun code ne le produit.
- **Raison :** §9 marque explicitement ces événements *v2* — hors périmètre
  MVP — et §10, la référence normative de l'US-217, ne les mentionne pas du
  tout dans sa table de déduction.
- **Conséquences :** un message marqué lu par son destinataire reste affiché
  `delivered` côté dashboard. Aucune perte de données : les événements
  `msg.read`/`read.observed`, s'ils sont un jour émis, sont insérés dans
  `events` comme les autres (l'ingestion, elle, ne filtre par nom), seule la
  projection les ignore.
- **Doc de conception mise à jour ?** non — cohérent avec le marquage *v2*
  déjà présent dans `docs/synthese/09`.

---

### 2026-09-28 — `GET /api/stream` sans authentification opérateur (US-218)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §6 liste
  `GET /api/stream` parmi les routes de l'API sans préciser d'exigence
  d'authentification particulière pour la lecture ; §9 rappelle que les
  événements diffusés sont déjà anonymisés/redigés à la source (aucun
  `msg_uuid`, texte ou identifiant de destinataire en clair).
- **Réel :** la route ne vérifie aucune identité — quiconque atteint l'API
  peut ouvrir le flux SSE et voir tous les événements ingérés (bruts, pas
  seulement ceux d'un nœud particulier).
- **Raison :** même situation que `POST /api/nodes` (écart déjà consigné,
  US-216) — l'auth opérateur/admin (session, cookie, rôle) n'est couverte
  par aucune US du backlog actuel. Contrairement à `/ingest/batch`
  (authentifie un NŒUD), `/api/stream` sert un OPÉRATEUR humain, cas que
  l'US-218 ne couvre pas.
- **Conséquences :** acceptable pour une démo locale (B-4, réseau de
  confiance), mais un vrai trou avant tout déploiement exposé (US-224) : le
  flux d'événements (topologie, statuts de messages, santé des relais) est
  lisible par quiconque atteint le port. Les événements restent redigés
  (pas de fuite de contenu de message), mais la topologie/l'activité du
  réseau ne l'est pas.
- **Doc de conception mise à jour ?** non — à couvrir par une future US
  d'auth opérateur, si elle est priorisée.
### 2026-09-28 — `sync::inventory` : le push du manquant est cadencé (US-210)

- **Prévu :** `synthese/05` §6.2 — après l'échange d'`INVENTORY`, « chacun
  pousse à l'autre ce qui lui manque » ; repli « pousser toute la file ».
  Rien sur le débit.
- **Réel :** file de push par lien, vidée à au plus `PUSH_MAX_PER_MIN = 15`
  paquets par minute (`src/sync/inventory.rs:70`).
- **Raison :** le même document impose `FLOOD_MAX_PER_MIN_PEER = 20`
  nouveaux `msgID`/min par voisin (§6.1). Les deux règles se contredisent
  dès que le manquant dépasse 20 paquets : mesuré par
  `temoin_sans_cadence_l_anti_inondation_rejette`, 6 paquets sur 25 rejetés
  en une rencontre. 15 laisse de la place à l'`INVENTORY` lui-même et au
  trafic direct.
- **Conséquences :** un cache plein (120 paquets) met ~8 min à passer à un
  voisin ; ce qui n'est pas passé avant la séparation l'est à la rencontre
  suivante. Réglable (`InventoryConfig::push_max_per_min`).
- **Doc de conception mise à jour ?** non — à valider à trois.

---

### 2026-09-28 — `sync::inventory` : réglages du cache sans constante de conception (US-210)

- **Prévu :** `synthese/08` §5 — « cache de réconciliation ~120 paquets,
  éviction LRU + 6 h », sans constante dans `protocol::consts`.
- **Réel :** `INVENTORY_CACHE_CAP = 120`, `INVENTORY_WINDOW_MS = 6 h`,
  `PUSH_MAX_PER_MIN = 15`, `INVENTORY_MAX_IDS = 2047` : constantes du
  module, portées par `InventoryConfig`, pas dans `protocol::consts`
  (contrat « revue à trois »). Éviction du **plus ancien reçu** (FIFO), pas
  LRU : un paquet n'est jamais « utilisé » autrement que poussé.
- **Raison :** même choix que les trois réglages de `sync::routing`.
- **Conséquences :** un téléphone peut monter le cap ; l'inventaire annoncé
  est tronqué à `max_ids` (les plus récents) — le receveur pousse alors
  aussi les plus anciens, rejetés en `Duplicate` s'il les a encore.

---

### 2026-09-28 — `GET /api/messages`/`GET /api/messages/{id}` sans authentification opérateur (US-219)

- **Prévu :** même situation que `GET /api/stream` (écart ci-dessus) —
  `docs/synthese/09` ne précise pas d'exigence d'auth pour la lecture des
  projections.
- **Réel :** aucune vérification d'identité sur ces deux routes non plus.
- **Raison :** identique à `GET /api/stream` — l'auth opérateur n'est
  couverte par aucune US actuelle.
- **Conséquences :** identiques — acceptable pour une démo locale, à
  couvrir avant tout déploiement exposé.
- **Doc de conception mise à jour ?** non.

---

### 2026-09-28 — `sync::inventory` : ce qui entre au cache (US-210)

- **Prévu :** `powl/03` §7.2 — le cache contient « messages publics
  récents, ACK non encore confirmés livrés, enveloppes » ; `synthese/05`
  §6.2 ne précise pas.
- **Réel :** `cacheable` (`src/sync/inventory.rs:183`) retient les
  `SEALED_ENVELOPE`, `NOISE_MSG` et `ACK` acceptés par le routeur **et non
  livrés ici**, avec `RELAY_OK` et `ttl > 1` ; TTL poussé = `ttl − 1` (ou
  celui du relais programmé). Exclus : `ANNOUNCE`, `LOG_ATTEST`
  (périodiques), `NOISE_HS` (propre à une session), `ENVELOPE_*` et
  `INVENTORY` (autres mécanismes), `FRAGMENT` (réassemblé avant).
- **Raison :** pousser un paquet sans `RELAY_OK` ou à TTL épuisé
  contournerait la portée voulue par l'émetteur.
- **Conséquences :** le payload `INVENTORY` est codé dans `sync::inventory`
  (`encode_payload` / `decode_payload`) et non dans `protocol::codec`, qui
  laisse les payloads opaques.
- **Doc de conception mise à jour ?** non.
### 2026-09-28 — `message_hops` (§11.2) dérivée à la lecture, jamais stockée (US-219)

- **Prévu :** `docs/synthese/09-dashboard-et-donnees.md` §11.2 décrit
  `message_hops` comme une table à part, avec une ligne par saut,
  alimentée à l'ingestion (même logique que `messages`).
- **Réel :** `app/messages_api.py::get_message_hops()` reconstruit le
  parcours d'un message **à la lecture**, en relisant `events` et en
  mappant chaque événement pertinent (`pkt.relayed`, `envelope.stored`,
  `envelope.handoff`, `msg.delivered`/`ack.observed`, et tout autre
  événement portant ce `msg_log_id`) vers la forme `message_hops`. Aucune
  table `message_hops` n'existe, aucune migration ne l'a créée.
- **Raison :** au volume visé (démo 5-8 appareils, B-4), reconstruire à la
  lecture coûte moins cher que de maintenir une table à l'écriture (pas de
  nouvelle migration, pas de nouvel `INSERT` à greffer dans le chemin
  d'ingestion déjà chargé de `_refresh_message_projection`) — même
  discipline que le choix déjà fait pour `messages` (US-217, recalcul
  complet plutôt qu'incrémental).
- **Conséquences :** un `GET /api/messages/{id}` coûte un `SELECT` de plus
  sur `events` filtré par `json_extract` — négligeable au volume visé, à
  revoir en priorité si le nombre d'événements par message grossissait
  significativement.
- **Doc de conception mise à jour ?** non — `docs/synthese/09` §11.2 reste
  la description du modèle cible ; ce fichier documente que
  l'implémentation obtient le même résultat par un autre chemin.

---

### 2026-09-28 — Vérification visuelle US-219 non refaite dans un navigateur

- **Prévu :** l'US-219 demande un « rendu correct sur mobile », comme
  l'US-111 (vérifiée le 2026-09-25 avec Chromium headless, voir
  `docs/suivi/modules/dashboard-web.md`).
- **Réel :** aucun outil de navigation n'était disponible dans cette
  session — la vérification s'est limitée à `node --check` (syntaxe JS) et
  à des appels `curl` bout en bout contre une vraie instance de l'API
  (fixtures golden ingérées, réponses JSON conformes, CORS vérifié entre
  deux ports différents).
- **Raison :** contrainte d'environnement, pas un choix de conception — les
  gabarits HTML/CSS n'ont pas changé depuis la vérification visuelle de
  l'US-111 (seule la source des données change, via `fetch`), donc le
  risque de régression purement visuelle est faible, mais pas nul (états
  « Chargement… »/« Erreur » sont nouveaux, jamais vus dans un vrai
  navigateur).
- **Conséquences :** à revérifier visuellement dès qu'un navigateur est
  disponible, en particulier les nouveaux états de chargement/erreur.
- **Doc de conception mise à jour ?** sans objet.


---

## US-311 — Flotte et carte réseau : périmètre réduit vs conception

- **Conception :** `docs/synthese/09-dashboard-et-donnees.md` §5–6 prévoit
  `GET /api/nodes/:id`, une table `links`, les alertes « version obsolète »,
  et un alerting configurable vers webhook / e-mail.
- **Réalisé :** `GET /api/nodes` et `GET /api/network/graph` seulement, liens
  dérivés de `events` à la lecture, alertes `relay_silent` et `buffer_high`
  affichées dans le dashboard (aucune notification sortante). Les pairs
  (peerID) ne sont pas rapprochés des `node_id`.
- **Pourquoi :** aucune version de référence pour « obsolète » ; webhook/e-mail
  hors périmètre de l'US ; volume de démo.
- **Conséquences :** un relais muet n'est vu que si quelqu'un regarde l'écran.
- **Doc de conception mise à jour ?** non.
---

### 2026-09-29 — `pkt.relayed` et `sync::inventory` restent non câblés à `observability` (US-318)

- **Prévu :** US-318 (issue #113) demandait de brancher `sync::routing` et
  `sync::inventory` sur `observability` pour que « les événements du
  catalogue sont émis aux bons endroits » (critère resté partiel de #22,
  US-208, PR #89).
- **Réel :** seuls `pkt.seen` (`on_bytes_received`) et `msg.queued`
  (`send_message`) sont câblés, dans `dengon-core::api` (façade client,
  US-301). `pkt.relayed` et les événements de `sync::inventory` restent
  **non émis**.
- **Raison :** `dengon-core::api` documente déjà (doc de module, §« Portée
  de cette implémentation ») qu'elle n'appelle ni `Router::poll_due` (le
  relais effectif) ni `sync::inventory` — ce sont des responsabilités du
  firmware relais dédié (US-308, C/ESP-IDF), pas de la façade côté client
  (téléphone). Câbler `pkt.relayed`/les événements d'inventaire à cet
  endroit aurait été un contresens architectural (émettre un événement pour
  une action que ce nœud n'effectue jamais), pas juste un oubli à corriger.
- **Conséquences :** #22 (US-208) reste ouverte après cette US : sur les 28
  événements du catalogue, seuls 2 ont un site d'appel réel démontré (contre
  4 constructeurs disponibles au total). Le câblage `pkt.relayed`/
  `sync::inventory` doit être fait côté firmware (US-308, C/ESP-IDF,
  `crates/dengon-core-embed` ou les tâches FreeRTOS elles-mêmes) — hors
  périmètre `core-rust`/`skill:rust`, nécessite `skill:c-embarqué`.
- **Doc de conception mise à jour ?** non — `docs/powl/08` ne précise pas
  quel composant émet quel événement, seulement le catalogue lui-même.

---

### 2026-09-29 — `dengon-node` : rôle central seul, règle anti-boucle non appliquée (US-303)

- **Prévu :** `05-protocole-et-trame.md` §6 : chaque nœud est Peripheral **et**
  Central ; le plus petit `peerID` initie la connexion.
- **Réel :** `BtleplugRadio` n'annonce rien (Spike B : `btleplug` central-only).
  Il se connecte à **tout** pair qui annonce le service, sans comparer les
  `peerID`. Deux `dengon-node` ne se voient donc pas.
- **Pourquoi :** appliquer la règle priverait de connexion tout pair au
  `peerID` plus petit, qui attendrait vainement d'être joint par un nœud qui
  n'annonce pas.
- **Conséquence :** utile contre Android / ESP32, pas nœud à nœud. Le repli
  `bluer` (Linux, peripheral) recommandé par B-6 reste à ratifier.

### 2026-09-29 — `dengon-node` : un seul pair par session, désigné par `--peer` (US-303)

- **Prévu :** un nœud dialogue avec tous les voisins du maillage.
- **Réel :** `Session` attribue tout lien ouvert au pair donné par `--peer`
  (`max_connections = 1`).
- **Pourquoi :** `TransportEvent::PeerConnected` ne porte qu'un `LinkId`, et
  `Node::on_peer_connected` exige un `peerID` ; l'`ANNOUNCE` qui associerait
  les deux n'est pas câblé (`api.rs`, portée d'US-301). Banc de test à deux.
- **À reprendre :** quand l'`ANNOUNCE` sera câblé, apprendre le `peerID` sur le
  lien plutôt que de le passer en argument.

### 2026-09-29 — `Transport` desktop : motif `Propre` jamais émis, pas de MTU négocié (US-303)

- **Prévu :** le contrat distingue coupure propre / brutale / locale ; le MTU
  517 est négocié (`preferred_mtu`).
- **Réel :** `btleplug` n'expose ni la cause d'une déconnexion (toute perte de
  lien est `Brutale`, une fermeture décidée localement est `Locale`) ni la
  négociation du MTU (faite par l'OS). Taille max d'une trame = 512, limite
  d'une valeur d'attribut GATT ; la pile refuse ce qui ne passe pas.
- **Conséquence :** le cas `cas_deconnexion_propre_est_distinguee` passe sur la
  fausse radio mais ne peut pas être satisfait par la radio réelle.

### 2026-09-29 — `dengon-node` : clé locale en clair à côté de la base (US-303)

- **Prévu :** clé du coffre issue d'un trousseau (Keystore Android, etc.).
- **Réel :** `<db>.key` (32 octets aléatoires, `0600` sous Unix) sert de clé de
  coffre et de clé des champs sensibles du `store`. Pas de trousseau desktop
  dans le périmètre ; nœud de test uniquement.

### 2026-09-29 — Couverture `dengon-core` déjà mesurée en CI, contrairement à ce que disait PR #89 (US-319)

- **Prévu :** PR #89 (US-208) et l'issue #22 affirmaient que la couverture
  n'avait « jamais été mesurée avec un outil dédié (`cargo llvm-cov` pas
  encore posé ce sprint) ».
- **Réel :** `cargo-llvm-cov` est installé et exécuté dans le job CI `core`
  depuis `PR #57` (US-104), bien avant US-208 — chaque run publie un
  résumé (`$GITHUB_STEP_SUMMARY`) et un artefact `lcov.info`. Mesure locale
  (US-319) : `dengon-core` à 96.29 % régions / 97.46 % lignes, largement
  au-dessus des 85 % visés.
- **Raison :** l'auteur de PR #89 n'a probablement pas relu le job CI
  existant, ou l'a confondu avec l'absence d'un **seuil bloquant** (qui,
  lui, n'existe effectivement pas — voir le commentaire dans
  `.github/workflows/core.yml`, « Pas de seuil bloquant pour l'instant »).
- **Conséquences :** le critère « couverture ≥ 85 % » de #22 est
  maintenant vérifiable par un chiffre réel, pas une estimation. Reste
  ouvert (décision d'équipe, pas traité ici) : ajouter
  `--fail-under-lines 85` scopé à `dengon-core` dans le workflow `core`
  pour le rendre bloquant.
- **Doc de conception mise à jour ?** sans objet.

---

### 2026-09-29 — `pkt.relayed` était déjà câblé en parallèle par #110 (US-308), écart précédent corrigé

- **Prévu (entrée précédente, US-318, même jour) :** « `pkt.relayed` reste
  non émis... doit être fait côté firmware (US-308, C/ESP-IDF) — hors
  périmètre `core-rust`/`skill:rust` » ; issue #118 créée en conséquence.
- **Réel :** `crates/dengon-core/src/relay.rs` (Rust, `no_std`, pas C —
  autre correction : le C ne garde que radio/stockage/ordonnancement) câble
  déjà `pkt.relayed` dans `Relay::poll_routing`, sur `Router::poll_due`,
  avec `fanout` = nombre de cibles réellement visées. Livré par Paul dans
  la PR #110 (US-308, branche `feat/US-308-relay`), en cours au même
  moment que ma propre session sur US-318/US-319, sans que je le sache.
- **Raison :** travail concurrent non coordonné en temps réel — chacun
  travaillait sur sa branche. Pas un problème de conception, juste un
  besoin de vérifier l'état des PR en cours avant de créer une issue de
  suivi.
- **Conséquences :** #118 (US-320) refermée sans travail supplémentaire,
  référencée vers PR #110. Une fois #110 mergée, il ne restera plus aucun
  des 4 constructeurs représentatifs de `observability` sans site d'appel
  réel (`pkt.seen`/`msg.queued`/`peer.connected` côté `api.rs`,
  `pkt.relayed`/`pkt.rejected`/`envelope.expired` côté `relay.rs`) — #22
  (US-208) redeviendra fermable une fois #114/#119/#110 tous mergés.
- **Doc de conception mise à jour ?** sans objet.

---

### 2026-09-29 — `peer.connected` sans `peer.disconnected` côté client (US-319)

- **Prévu :** `docs/powl/08` §5 : « le dashboard dérive `LINKS` de la
  corrélation `peer.connected`/`disconnected` entre deux `node_id`
  connus ». Les deux événements sont donc attendus en paire.
- **Réel :** `Node::record_peer_connected` (US-319) émet `peer.connected`,
  mais rien n'émet `peer.disconnected` côté façade client. Pas propre à
  cette PR : `on_peer_disconnected` existe (`Node::on_peer_disconnected`,
  US-301) mais n'a jamais construit d'`Envelope` non plus, avant comme
  après US-318/US-319.
- **Raison :** `record_peer_connected` n'a de toute façon aucun appelant
  réel aujourd'hui (voir entrée `dengon-core.md` correspondante, revue de
  PR #119) — le pendant `disconnected` n'a pas été priorisé avant que le
  premier événement ait lui-même un site d'appel.
- **Conséquences :** un lien resterait ouvert indéfiniment dans `LINKS` côté
  dashboard une fois `peer.connected` réellement câblé, sans un
  `peer.disconnected` symétrique. À traiter dans le même effort que le
  câblage réel de `peer.connected` (probablement `dengon-node::session.rs`,
  PR #116, qui a déjà `Node::on_peer_disconnected` appelé au bon endroit —
  il suffirait d'y ajouter l'émission).
- **Doc de conception mise à jour ?** non.

---

### 2026-09-29 — US-304 livrée en deux temps : client d'abord, relais ensuite

- **Prévu :** l'issue #42 (US-304) demande les 5 scénarios `direct`,
  `multihop`, `recipient_offline`, `sender_offline`, `partition_merge` avec
  le vrai `dengon-core`, dans une seule US.
- **Réel :** cette session livre `NoeudClient` (le vrai `api::Node` en
  `Comportement`) et 3 scénarios entièrement réels :
  `direct`/`recipient_offline`/`sender_offline` — les trois réalisables
  avec deux correspondants, sans relais. `multihop`/`partition_merge`
  restent sur `Inondation` (bouchon de flood, scénarios `.ron` existants).
- **Raison :** `multihop`/`partition_merge` exigent un nœud qui **relaie**
  réellement — `dengon-core::api::Node` n'appelle jamais `Router::poll_due`
  (documenté dans `api.rs` : ce n'est pas son rôle, c'est celui de
  `relay.rs`/le firmware ESP32). Écrire aussi le `Comportement` relais dans
  la même session aurait doublé la taille du changement sans que les 3
  premiers scénarios (déjà complets et vérifiés) n'aient à en dépendre —
  découpage discuté et validé avec l'utilisateur avant de commencer.
- **Conséquences :** #42 reste ouverte après cette session : 3 critères sur
  4 partiellement couverts (3/5 scénarios, pas les 5 ; le reste — CI à
  graine fixe, traces exploitables, résultats précis — est satisfait pour
  ces 3). Suite prévue : un `Comportement` relais (`relay::Relay`) pour
  `multihop`/`partition_merge`.
- **Doc de conception mise à jour ?** non — `synthese/10` §4.3/§4.4 décrit
  déjà les 5 scénarios comme la cible ; ce fichier documente l'ordre de
  livraison, pas un changement de cible.
