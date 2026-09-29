# Module : `dengon-ffi` (`crates/dengon-ffi/`)

**Rôle en une phrase :** le pont qui permet à l'application Android, écrite en Kotlin, d'appeler le cœur écrit en Rust.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §3 et §5 ; [`docs/powl/09-data-model.md`](../../powl/09-data-model.md) §1/§3 (formats QR, code de vérification).
**Dernière mise à jour :** 2026-09-29 (US-302 ; corrections revue PR #109)
**État :** **fonctionnel** — vrai FFI branché sur `dengon_core::api` (US-301). Plus aucun bouchon, ni en Rust ni en Kotlin. Contrat étendu en v1 : écart consigné, **à annoncer en point d'équipe**.

> Historique : l'US-106 avait livré un contrat v0 gelé et deux bouchons
> indépendants (Rust et Kotlin) avec des placeholders non cryptographiques.
> L'US-302 les remplace tous les deux. Le détail de l'US-106 reste dans le
> journal (entrées du 2026-09-20 au 2026-09-28).

## À quoi ça sert

L'application Android est en Kotlin, le protocole en Rust. **UniFFI** génère le
code d'interfaçage à partir d'une description d'interface (`src/dengon.udl`) :
côté Rust, le *scaffolding* (compilé dans `libdengon_ffi.so`) ; côté Kotlin, un
fichier `dengon.kt` qui appelle cette bibliothèque via **JNA**.

Ce que fait réellement la crate est mince : **convertir** les types (octets →
chaînes), **fournir l'horloge et l'aléa** que le cœur attend de l'appelant, et
**verrouiller** le nœud pour qu'il soit appelable depuis plusieurs threads.
Protocole, crypto et routage restent dans `dengon-core`.

## Structure

```text
crates/dengon-ffi/
  build.rs            — uniffi::generate_scaffolding("src/dengon.udl")
  uniffi.toml         — paquet Kotlin (com.dengon.app.ffi) + nom de la lib (dengon_ffi)
  uniffi-bindgen.rs   — binaire `uniffi-bindgen` 0.28.3, feature `bindgen`
  src/
    dengon.udl        — contrat v1 (v0 US-106 + extension US-302)
    lib.rs            — types du contrat, DengonNode, fonctions identité/QR, tests
    convert.rs        — conversions core ↔ FFI (peer_id base32, hex, Identity)

android/scripts/build-ffi.sh          — bindings Kotlin + .so Android + .so hôte
android/app/src/main/java/com/dengon/app/ffi/dengon.kt
                                      — bindings GÉNÉRÉS, versionnés (ne pas éditer)
android/app/src/main/jniLibs/<abi>/   — .so construites, NON versionnées
android/app/src/test/.../ffi/
  FfiNatif.kt                         — « la lib hôte est-elle là ? » (Assume)
  DengonNodeIntegrationTest.kt        — test d'intégration Kotlin → JNA → Rust
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `DengonNode` | `src/lib.rs:179` | `Mutex<api::Node>` + origine de l'horloge monotone. Le verrou empoisonné est récupéré (`PoisonError::into_inner`) plutôt que de paniquer à chaque appel suivant. |
| `DengonNode::open` | `src/lib.rs:194` | Constructeur v1. Crée `data_dir`, puis `identity::load_or_create` sur un `FileVault` (`<data_dir>/identity.vault`, clé de 32 o fournie par l'appelant). `peerID` stable d'un lancement à l'autre ; `pseudo` lu au premier lancement seulement. |
| `on_peer_connected` / `on_peer_disconnected` / `on_bytes_received` / `take_outgoing` | `src/lib.rs:239-268` | Le chemin des octets radio : l'appelant (`AndroidTransport`, US-213) pousse ce qu'il reçoit et vide ce qu'il doit émettre. Le nœud ne possède aucun transport. |
| `now` | `src/lib.rs:292` | `Now { wall_ms: SystemTime, mono_ms: Instant écoulé depuis open }`. |
| `generate_identity` | `src/lib.rs:319` | Vraies clés, mais **jetable** : seule la carte publique franchit le FFI. Pour les tests ; l'app passe par `open`. |
| `identity_qr_code` / `identity_from_qr_code` / `verification_code` | `src/lib.rs:329-352` | Délèguent à `dengon_core::identity` (US-205) : QR `dengon:v1:`, code 60 chiffres SHA-512. |
| `peer_id_to_string` / `peer_id_from_str` | `src/convert.rs:17-31` | `peerID` en base32 minuscules (13 caractères), le format affiché par `identity::peer_id_base32`. Chaîne mal formée → `UnknownPeer`. |
| `identity_from_ffi` | `src/convert.rs:60` | Reconstruit une `PublicIdentity` depuis Kotlin en **revérifiant** tout : longueurs, clé Ed25519, pseudo, et `peer_id` annoncé = `peer_id` recalculé, comparés **décodés** : les majuscules sont tolérées comme dans `peer_id_from_str` (revue PR #109) ; sinon `Internal`. |
| `status_to_ffi` | `src/convert.rs:73` | 4 statuts du cœur → 6 du contrat : `Read`/`Cancelled` ne sont jamais émis. |

## Flux principal (exemple)

Alice écrit à Bob, déjà appairé et à portée :

1. `alice.sendMessage(idBob, "bonjour")` (Kotlin) → JNA → `DengonNode::send_message`
   → `peer_id_from_str` → `api::Node::send_message(dest, body, now, OsRng)` :
   chiffré en session Noise, rangé en outbox, `StatusChanged(InFlight)` en file.
2. `alice.takeOutgoing()` rend `[OutgoingFrame(idBob, octets)]` ; le transport
   les écrit sur le lien de Bob.
3. Chez Bob, le transport appelle `bob.onBytesReceived(idAlice, octets)` →
   routeur → déchiffrement → `MessageReceived` en file.
4. `bob.pollEvents()` le rend à l'UI.

C'est exactement ce que rejouent `DengonNodeIntegrationTest` (Kotlin) et
`message_de_bout_en_bout_entre_deux_noeuds` (Rust), la radio étant remplacée
par une boucle qui recopie les trames.

## Dépendances

- `dengon-core` (feature `std`, par défaut) : `api`, `identity`, `sync::routing::Now`.
- `uniffi` =0.28.3 (épinglé : un générateur d'une autre version produirait des
  bindings incompatibles avec le scaffolding compilé).
- `rand_core` avec `getrandom` → `OsRng`. **Seulement ici** : le cœur `no_std`
  (ESP32) reste sans getrandom.
- `data-encoding` : base32 / hexadécimal (déjà dans le workspace).
- Côté app : `net.java.dev.jna:jna` 5.14.0 (AAR dans l'app, JAR dans les tests).

## Décisions d'implémentation

- **Contrat étendu, pas contourné.** Le `.udl` v0 ne pouvait pas porter un vrai
  nœud (pas de clé privée, pas d'octets radio). Choix de l'utilisateur :
  l'étendre plutôt que cacher les méthodes manquantes dans des
  `#[uniffi::export]` hors du `.udl` (une seule source de contrat). Voir
  `03-ecarts-conception.md`.
- **Fonctions libres `[Throws]`.** Une carte `Identity` qui arrive de Kotlin peut
  être invalide ; le bouchon ne vérifiait rien. Côté Kotlin, les exceptions ne
  sont pas vérifiées : aucun appelant n'a eu à changer.
- **Bindings versionnés.** `dengon.kt` est commité pour qu'Android Studio sous
  Windows compile sans toolchain Rust. La CI `android` régénère et échoue en
  cas de dérive. `--no-format` rend la sortie identique d'une machine à l'autre.
- **`cdylib_name = "dengon_ffi"`** dans `uniffi.toml` : sans lui, le générateur
  lancé sur le `.udl` (hors métadonnées Cargo) chargeait `libuniffi_dengon.so`.
- **Binaire `uniffi-bindgen` derrière une feature** : sans elle, `clap` et les
  dépendances du CLI entreraient dans chaque build. Placé à la racine de la
  crate, pas dans `src/bin/` : le `.gitignore` du dépôt ignore tout `bin/`.
- **ABI : arm64-v8a + x86_64.** Les deux téléphones de test sont en arm64 ;
  x86_64 sert à l'émulateur. `abiFilters` dans `app/build.gradle.kts`, sinon
  l'AAR de JNA ajoute armv7/x86/mips et un téléphone armv7 planterait au
  premier appel.

## Tests

Rust (`cargo test -p dengon-ffi`, 13 tests) :

| Test | Vérifie |
|---|---|
| `message_de_bout_en_bout_entre_deux_noeuds` | handshake + message chiffré Alice → Bob, `MessageReceived`, `StatusChanged(InFlight)`, même `conv_id` des deux côtés |
| `evenements_de_connexion_et_deconnexion` | `PeerConnected` puis `PeerDisconnected` |
| `identite_stable_d_une_ouverture_a_l_autre` | réouverture du coffre : même carte, pseudo d'origine |
| `mauvaise_cle_de_coffre_refusee` | mauvaise clé, clé de 31 octets → `Internal` |
| `envoi_a_un_inconnu_refuse` | `UnknownPeer` |
| `peer_id_mal_forme_refuse` | 4 chaînes invalides → `UnknownPeer` ; `list_messages` sur un `conv_id` non hexa → vide |
| `qr_aller_retour`, `carte_au_peer_id_falsifie_refusee`, `carte_au_peer_id_en_majuscules_acceptee`, `code_de_verification_symetrique_60_chiffres`, `pseudo_vide_refuse` | fonctions libres |
| `convert::tests` (2) | base32 et hexadécimal, aller-retour et longueur stricte |

Kotlin (`./gradlew testDebugUnitTest`) : `DengonNodeIntegrationTest` (4 tests,
mêmes scénarios à travers JNA). Ces tests, avec `AppairageViewModelTest` et 3
tests de `QrCodeTest`, sont **ignorés** si `libdengon_ffi.so` hôte manque
(`FfiNatif.exiger()`), par exemple dans Android Studio sous Windows ; le job CI
`android` échoue si `DengonNodeIntegrationTest` a été ignoré.

## Limites connues / TODO

- **Messages non persistés** : `api::Node::attach_store` n'est pas appelé. Les
  conversations vivent le temps du processus ; seule l'identité survit.
- `Read` et `Cancelled` jamais émis (pas de `mark_read` / `cancel_message`).
- Statut final observable : `InFlight` — la façade US-301 n'émet pas d'ACK
  (écart de l'US-301).
- Contacts non persistés non plus : à réenregistrer (`addContact`) après
  redémarrage — l'écran d'appairage devra être rejoué tant que ce n'est pas fait.
- Pas de SBOM ni de signature de l'APK release.

## Pour l'oral

« Le FFI, c'est un traducteur, pas un cerveau : 300 lignes qui convertissent
des tableaux d'octets en chaînes et prennent un verrou. Tout le reste est dans
`dengon-core`. La preuve que le pont tient : un test Kotlin qui ouvre deux
vrais nœuds, les fait se serrer la main en Noise et échanger un message
chiffré, sans radio ni bouchon. »
