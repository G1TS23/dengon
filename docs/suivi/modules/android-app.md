# Module : `android-app` (`android/`)

**Rôle en une phrase :** l'application Android (Kotlin + Compose) — le service
de fond BLE (US-109) possède maintenant une implémentation réelle du contrat
`Transport` (US-213) : GATT server + advertiser + scanner, testée sur 2 vrais
téléphones ; s'y ajoutent les écrans de messagerie (US-214) et d'appairage QR
(US-215) sur le bouchon FFI.
**Correspond à la conception :** `docs/synthese/04-architecture.md` §3
(impl Android du trait `Transport`) et §7 ; `docs/synthese/10-benchmarks-mvp-tests.md`
§2.7 (contraintes d'arrière-plan Android 14/15) ; `docs/olivier/proposition-organisation-github.md`
US-109, US-213 ; `crates/dengon-ble/src/transport.rs` (le contrat, US-105) et
`crates/dengon-ble/src/conformance.rs` (la suite de conformité).
**Dernière mise à jour :** 2026-09-28
**État :** partiel — service de fond (US-109) + transport BLE réel (US-213) +
messagerie sur bouchon FFI (US-214) + appairage QR (US-215) ; pas encore
branché sur `dengon-core` ni sur la vraie identité (`peerID`, US-306) — voir
« Spike C » ci-dessous pour le code GATT jetable qui a dérisqué
`AndroidTransport` avant cette US

## Onboarding (US-103)

**Builder :**
```
cd android && ./gradlew assembleDebug
```
Nécessite un SDK Android local (`local.properties` → `sdk.dir`, non
versionné, voir `local.properties.example` si présent sinon créer le
fichier). APK debug dans `app/build/outputs/apk/debug/app-debug.apk`.

**Tester :**
```
./gradlew testDebugUnitTest    # tests unitaires JVM, rapides
./gradlew assembleRelease      # build release (minify/shrink R8 actifs)
```
Pas de test instrumenté (émulateur/appareil) dans le dépôt pour l'instant —
tout ce qui touche au BLE réel se vérifie **manuellement** sur un appareil
(voir « Spike C » ci-dessous).

**Trois pièges rencontrés (à ne pas refaire) :**
1. **`.gitignore` du wrapper Gradle** : `!gradle/wrapper/gradle-wrapper.jar`
   n'est ancré qu'à la racine d'un dépôt Git ; pour un module non-racine
   (`android/`), il faut `!**/gradle/wrapper/gradle-wrapper.jar` sinon le
   `.jar` du wrapper n'est jamais versionné et `./gradlew` échoue sur un
   clone frais.
2. **`gradle/verification-metadata.xml` régénéré sur un `GRADLE_USER_HOME`
   déjà chaud** : le fichier peut sembler correct localement (le build passe)
   mais être en fait incomplet — un artefact déjà en cache n'est jamais
   re-téléchargé ni re-checksummé pendant `--write-verification-metadata`.
   Toujours régénérer après avoir vidé `~/.gradle/caches/modules-2`, sinon un
   clone frais (ou la future CI) casse au premier build. Détail complet :
   `04-apprentissages.md`.
3. **`foregroundServiceType` doit être déclaré deux fois** : dans
   `AndroidManifest.xml` (`android:foregroundServiceType="connectedDevice"`
   sur le `<service>`) **et** dans le code
   (`ServiceCompat.startForeground(..., FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)`).
   Oublier l'un des deux ne provoque pas d'erreur de compilation — le service
   se fait juste tuer par le système peu après l'extinction de l'écran, ce
   qui ne se voit qu'au test manuel des 5 minutes.

## À quoi ça sert

Android est la plateforme mobile retenue pour le MVP (Kotlin natif, A-1) car
c'est la seule à offrir un contrôle total du rôle GATT double (central +
peripheral) et du service de fond. Le point dur de cette US n'est pas
Compose : c'est que le service qui relaie les messages BLE doit **survivre à
l'écran éteint**, sous les restrictions d'arrière-plan d'Android 14/15
(déclaration `foregroundServiceType`, permissions runtime, notification
permanente obligatoire).

## Structure

```
android/
  settings.gradle.kts        — déclare le module `app`
  build.gradle.kts           — plugins AGP/Kotlin (root, via version catalog)
  gradle/
    libs.versions.toml       — toutes les versions (plugins + dépendances) centralisées
    verification-metadata.xml — checksums de tout ce qui est résolu (plugins inclus)
  app/
    gradle.lockfile          — verrouillage des dépendances résolues de :app
    build.gradle.kts         — applicationId com.dengon.app, minSdk 26, target/compileSdk 34, Compose
    src/main/AndroidManifest.xml
    src/main/java/com/dengon/app/
      DengonApplication.kt   — Application vide (point d'extension futur)
      MainActivity.kt        — Compose : demande permissions, démarre/arrête le service, affiche l'état,
                               ouvre la messagerie (US-214) et l'écran d'appairage (US-215)
      ffi/                   — bouchon FFI (US-106) : DengonTypes.kt, DengonNodeStub.kt
      ui/conversations/      — messagerie (US-214)
        ConversationsViewModel.kt — état (StateFlow) + actions, alimenté par DengonNodeInterface
        ConversationsScreen.kt    — Compose : MessagerieRoute, ListeConversations, FilConversation
        LibelleStatut.kt          — statut → libellé UI (synthese/07 §1)
      identite/
        IdentiteLocale.kt    — identité provisoire par installation (pseudo hexadécimal 8 octets), US-215
      ui/appairage/          — US-215
        AppairageViewModel.kt — étapes de l'appairage, StateFlow, JVM pur
        AppairageScreen.kt   — mon QR, scan caméra, code 60 chiffres, confirmation explicite
        QrCode.kt            — matrice QR (zxing-core) garantie détectable, format du code
      ble/
        BlePermissions.kt        — liste des permissions requises selon Build.VERSION.SDK_INT
        MeshForegroundService.kt — service de fond, notification permanente, foregroundServiceType=connectedDevice
        spike/                   — code JETABLE du Spike C (US-103), à supprimer après la décision
          HelloMeshConstants.kt    — SERVICE_UUID/CHAR_RX/CHAR_TX/MTU visé (docs/powl/03-network-protocol.md §2, §6)
          HelloMeshPeripheral.kt   — BluetoothGattServer + BluetoothLeAdvertiser
          HelloMeshCentral.kt      — BluetoothLeScanner + BluetoothGatt (client)
          HelloMeshSpikeScreen.kt  — écran de debug Compose (accessible depuis MainActivity)
          SpikeResult.kt           — chiffres mesurés (MTU, timings, appareil)
    src/test/java/com/dengon/app/
      ble/BlePermissionsTest.kt  — test unitaire minimal (JVM, sans Robolectric)
      ffi/DengonNodeStubTest.kt  — bouchon FFI (11 tests)
      ui/conversations/ConversationsViewModelTest.kt — ViewModel de messagerie (9 tests)
      ui/appairage/AppairageViewModelTest.kt — parcours, erreurs de scan, lecture croisée (US-215)
      ui/appairage/QrCodeTest.kt          — QR rastérisé puis relu par ZXing, régression « alice »
      identite/IdentiteLocaleTest.kt  — pas de collision de `peerId` entre pseudos hexadécimaux
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `MeshForegroundService` | `app/src/main/java/com/dengon/app/ble/MeshForegroundService.kt:24` | `Service` Android. `onStartCommand` appelle `ServiceCompat.startForeground(..., FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)` et retourne `START_STICKY`. Construit la notification permanente (canal `IMPORTANCE_LOW`). Pas de logique GATT — squelette seulement. |
| `BlePermissions.required()` | `app/src/main/java/com/dengon/app/ble/BlePermissions.kt:17` | Retourne le tableau de permissions à demander : `BLUETOOTH_SCAN/CONNECT/ADVERTISE` sur API 31+, `ACCESS_FINE_LOCATION` en dessous, `+POST_NOTIFICATIONS` sur API 33+. |
| `BlePermissions.allGranted()` | `app/src/main/java/com/dengon/app/ble/BlePermissions.kt:29` | Vérifie si toutes les permissions requises sont déjà accordées. |
| `MainActivity` | `app/src/main/java/com/dengon/app/MainActivity.kt:29` | `ComponentActivity` Compose : lance la demande de permissions (`RequestMultiplePermissions`), démarre `MeshForegroundService` via `ContextCompat.startForegroundService` dès qu'elles sont accordées, bouton Démarrer/Arrêter pour le test manuel. |
| `ConversationsViewModel` | `ui/conversations/ConversationsViewModel.kt` | `StateFlow<ConversationsUiState>` (conversations, conversation ouverte, messages, brouillon, erreur) + actions `ouvrir`/`fermer`/`modifierBrouillon`/`envoyer`/`sonder`. Dépend seulement de `DengonNodeInterface` : le bouchon aujourd'hui, le nœud généré à l'US-306. **Synchrone** : testé en JVM pur, sans dispatcher de test. |
| `MessagerieRoute` | `ui/conversations/ConversationsScreen.kt` | Liste ↔ fil selon l'état ; `pollEvents` toutes les secondes (`LaunchedEffect`) ; retour système : fil → liste → accueil. |
| `libelleStatut` | `ui/conversations/LibelleStatut.kt` | `QUEUED` « En attente », `IN_FLIGHT` « Parti », `DELIVERED` « Distribué », `EXPIRED` « Échec », `CANCELLED` « Annulé » (`READ` « Lu » réservé v2). |
| `HelloMeshPeripheral` | `ble/spike/HelloMeshPeripheral.kt:38` | Publie `SERVICE_UUID` (`BluetoothGattServer` + `BluetoothLeAdvertiser`), expose `CHAR_RX`/`CHAR_TX`, fait l'écho de ce qu'il reçoit. |
| `HelloMeshCentral` | `ble/spike/HelloMeshCentral.kt:31` | Scanne `SERVICE_UUID`, se connecte, négocie le MTU (517 visé), écrit 20 o sur `CHAR_RX`, mesure le round-trip de l'écho sur `CHAR_TX`. |
| `HelloMeshSpikeScreen` | `ble/spike/HelloMeshSpikeScreen.kt:41` | Écran Compose de debug (bouton dédié dans `MainActivity`) : bascule manuelle Central/Peripheral, journal en direct, carte de résultat (MTU, temps). |

## Spike C — hello mesh (US-103)

**But :** dérisquer le double rôle GATT Android avant `AndroidTransport`
(US-213) et **mesurer le MTU réellement négocié** sur du matériel réel — la
mesure dimensionne `FRAG_SIZE`/la fragmentation protocole (US-201/US-202).
Timebox annoncée : 1 jour. Le résultat sert de go/no-go pour A-1 (voir
`docs/synthese/01-sujets-a-trancher.md` §A-1).

**Code jetable** (DoD §7.2, type Spike) : tout `ble/spike/` est voué à être
**supprimé** une fois la décision actée — ce n'est pas la logique BLE finale
de l'app, c'est un harnais de mesure. `AndroidTransport` (US-213)
réimplémentera le double rôle proprement (dynamique, tie-break par
`peerID`), pas en repartant de ce code.

**Simplifications volontaires par rapport à `docs/powl/03-network-protocol.md`
§6.1 :**
- Rôle choisi **manuellement** dans l'écran de debug (bouton « Peripheral »
  ou « Central ») plutôt que la règle anti-boucle par comparaison de
  `peerID` : `dengon-core` n'a pas encore d'identité de nœud (US-205), donc
  pas de `peerID` à comparer.
- Un seul échange mesuré par lancement (le scan s'arrête au premier pair
  trouvé) : suffisant pour le critère d'acceptation (« deux téléphones
  échangent 20 octets »), pas besoin de gérer plusieurs pairs simultanés
  pour ce spike.
- Le MTU négocié n'est lisible que côté **central** (`onMtuChanged` du
  `BluetoothGattCallback`) : l'API Android n'expose pas de callback
  équivalent côté serveur GATT pour relire la valeur après coup — la carte
  de résultat du peripheral affiche donc `n/a` sur ce champ. C'est le
  résultat mesuré côté **central** qui fait foi.

**Correction post-revue (PR #67, 2026-09-20) — séquencement CCCD/écriture :**
`BluetoothGatt` ne met pas ses opérations en file d'attente ; lancer
`writeCharacteristic` pendant qu'un `writeDescriptor` (activation du CCCD)
est encore en vol échoue silencieusement. `HelloMeshCentral` déclenche
maintenant l'écriture de `CHAR_RX` depuis `onDescriptorWrite(...)` (fin
confirmée de l'écriture du CCCD), plus juste après l'avoir lancée.
Symétriquement, `HelloMeshPeripheral` implémente désormais
`onDescriptorWriteRequest` (`sendResponse(GATT_SUCCESS)`), sans quoi
l'écriture du CCCD par le central (en `WRITE_TYPE_DEFAULT`, avec accusé ATT)
ne se termine jamais côté serveur. Voir `docs/suivi/00-journal.md`, entrée du
2026-09-20, et `04-apprentissages.md`.

### Protocole de mesure manuelle (à exécuter sur 2 appareils réels)

1. Installer l'APK debug sur les deux téléphones (`./gradlew installDebug`
   ou copier `app-debug.apk`), accorder les permissions BLE demandées au
   lancement.
2. Sur l'écran principal, taper **« Spike C : hello mesh (debug) »**.
3. Sur le téléphone A : taper **Peripheral**. Sur le téléphone B : taper
   **Central** (dans les ~30 s qui suivent, le temps que l'advertising
   démarre).
4. Relever sur l'écran du téléphone **B** (central, seul côté où le MTU est
   lisible) : MTU négocié, temps scan→connexion, temps connexion→échange.
   Noter le modèle/version des **deux** appareils (visibles en haut de
   l'écran, sur chaque téléphone).
5. Répéter avec au moins un autre couple de modèles/versions si possible
   (matrice d'appareils demandée par le critère d'acceptation).
6. Reporter les chiffres dans le tableau ci-dessous, dater, et mettre à jour
   `docs/synthese/01-sujets-a-trancher.md` §A-1 (statut du Spike C) +
   `docs/suivi/00-journal.md` (nouvelle entrée, ne pas éditer celle-ci).

### Résultats mesurés

**🟡 Partiellement exécuté** le 2026-09-25 : un seul appareil Android
disponible (pas de second Android sous la main), donc le protocole officiel
« 2 téléphones Android » n'a pas pu être suivi à la lettre. Test de repli fait
à la place : le rôle **Peripheral** tourné sur l'Android, avec un **iPhone**
en central via nRF Connect for Mobile (iOS) — un appareil physiquement
distinct, donc sans le problème classique « un téléphone ne peut pas
détecter ses propres annonces BLE » qui aurait invalidé un test sur un seul
appareil.

**Ce qui est confirmé :**
- Annonce BLE démarrée avec succès côté système (`BLE_GAP: ADV_SET_START`
  dans `logcat`, après un cycle stop/restart lié à la navigation dans l'UI).
- Détection et connexion réussies depuis nRF Connect (iOS), en filtrant par
  `SERVICE_UUID` (l'annonce n'inclut volontairement pas de nom d'appareil,
  `setIncludeDeviceName(false)` — HelloMeshPeripheral.kt:102).
- Table GATT lisible depuis nRF Connect : `CHAR_RX` (write) et `CHAR_TX`
  (notify) présentes avec les bons UUID.
- **Échange bout-en-bout réussi** : écriture manuelle des 20 octets ASCII
  (`hello mesh dengon!!!`) sur `CHAR_RX` depuis nRF Connect → écho reçu en
  notification sur `CHAR_TX` (comportement de `onCharacteristicWriteRequest`,
  HelloMeshPeripheral.kt:174-177).

**Ce qui n'a PAS pu être mesuré :** le **MTU négocié**. iOS/CoreBluetooth
n'expose aucune API permettant à l'app (ni donc à nRF Connect côté iOS) de
déclencher ou d'afficher la négociation MTU côté central — contrairement à
Android (`BluetoothGatt.requestMtu()`), c'est une limitation de la
plateforme, pas de l'app ou de la manipulation. nRF Connect pour iOS n'a donc
pas l'écran « Request MTU » présent sur sa version Android. Cette mesure
suppose deux appareils **Android**, comme prévu par le protocole d'origine.

**🟢 Exécuté intégralement** le 2026-09-28, sur 2 appareils Android réels :
Samsung Galaxy A16 (SM-A165F, Android 16/SDK 36) en **Peripheral**, Pixel 8
Pro (Android 17/SDK 37) en **Central**, les deux exécutant réellement
`ble/spike/` (contrairement au test de repli du 25/09, où l'iPhone
n'exécutait qu'un scanner générique). Build installé via
`./gradlew assembleDebug` + `adb install` (branche
`feat/US-103-SpikeC-HelloMesh`, tête `bdf2b95`).

Séquence suivie : app lancée + permissions accordées sur les deux
appareils → écran « Spike C : hello mesh (debug) » ouvert sur les deux →
**Peripheral** sur le Samsung (confirmé par `BLE_GAP: ADV_SET_START` dans
`logcat`) → **Central** sur le Pixel dans la minute qui suit → résultat lu
directement sur l'écran du Pixel (seul côté où le MTU est lisible, voir
« Simplifications volontaires » ci-dessus).

**Donc, sur les 4 critères d'acceptation de l'issue #3, tous démontrés :**
- [x] « 2 appareils échangent 20 octets » — **démontré**, cette fois avec les
  deux appareils exécutant réellement `HelloMeshPeripheral`/`HelloMeshCentral`.
- [x] « MTU réel négocié mesuré » — **517** (le MTU visé, voir
  `HelloMeshConstants.kt`), négociation aboutie côté central.
- [x] « Temps scan → connexion → échange mesuré » — **354 ms** (scan →
  connexion), **1308 ms** (connexion → échange, MTU compris — la négociation
  MTU fait partie de cet intervalle avant l'écriture de `CHAR_RX`).
- [x] « Matrice d'appareils testés » — 1 couple réel (Samsung Galaxy A16 ×
  Pixel 8 Pro), deux fabricants et deux versions Android différentes
  (16 et 17). Un second couple améliorerait la confiance mais n'est pas
  requis pour la décision go/no-go ci-dessous (aucun signal ne suggère un
  comportement dépendant du fabricant sur ce test).

| Date | Appareil (central) | Appareil (peripheral) | Android | MTU négocié | Scan→connexion | Connexion→échange |
|---|---|---|---|---|---|---|
| 2026-09-25 | iPhone 13 Pro Max (iOS 27.2 beta, nRF Connect) — *pas notre code, scanner générique* | Samsung Galaxy A16 (SM-A165F) | 16 (SDK 36) | non mesurable (limitation iOS) | non chronométré | non chronométré |
| 2026-09-28 | **Pixel 8 Pro** — exécute `HelloMeshCentral` | **Samsung Galaxy A16** (SM-A165F) — exécute `HelloMeshPeripheral` | Central : 17 (SDK 37) · Peripheral : 16 (SDK 36) | **517** | **354 ms** | **1308 ms** |

## `AndroidTransport` — transport BLE réel (US-213)

Implémentation Kotlin du contrat `Transport` gelé par US-105
(`crates/dengon-ble/src/transport.rs`), qui remplace le spike jetable
ci-dessus. Un nœud dengon est **en même temps** serveur GATT (annonce le
service `dengon`, rôle périphérique) et scanner/client GATT (rôle central) :
`GattRadio` porte les deux rôles simultanément.

```
android/app/src/main/java/com/dengon/app/ble/transport/
  Transport.kt          — miroir Kotlin du contrat Rust (LinkId, TransportConfig,
                           DisconnectReason, TransportEvent, TransportException)
  AndroidTransport.kt   — toute la logique du contrat : LinkId, file d'événements,
                           quota, réassemblage, règles de déconnexion. Zéro API Android.
  BleRadio.kt            — l'interface entre AndroidTransport et la radio (RadioPeer,
                           RappelsRadio) : ce qui rend le contrat testable en JVM pur
  GattRadio.kt           — la vraie radio : BluetoothGattServer + advertiser + scanner
                           + BluetoothGatt client, une seule écriture en vol par lien
  FragmentationBle.kt   — fragmentation/réassemblage L1 (distincte de la
                           fragmentation protocole de dengon-core), + Reassembleur
  Annonce.kt             — UUIDs GATT, manufacturer data (préfixe de peerID), règle
                           anti-boucle de connexion, motifDeconnexion(status GATT)
  TransportActif.kt      — singleton qui possède le transport du processus : boucle
                           poll(), journal (CRC + aperçu), battement périodique
  TransportDebugScreen.kt — écran Compose de debug (essais manuels 2 téléphones)
```

### Ce qui a été vérifié sur 2 vrais téléphones (Google Pixel 8 Pro Android 17,
Samsung Galaxy A16 Android 16)

| Vérification | Résultat |
|---|---|
| Découverte + connexion | Un **seul** lien s'ouvre entre les deux nœuds (règle anti-boucle confirmée) : le nœud au plus petit préfixe de `peerID` initie en central. |
| Message court (< MTU) | Reçu intact des deux côtés, CRC32 vérifié. |
| Grande trame (5000 o, fragmentée) | Reçue intacte des deux côtés, CRC32 vérifié — la fragmentation/réassemblage BLE fonctionne sur un vrai MTU négocié (517). |
| Déconnexion brutale (éloignement physique réel, pas un `disconnect()` propre) | **Asymétrique** : `BRUTALE` côté central, `PROPRE` côté périphérique pour le **même** événement — limite Android, voir « Décisions » et l'écart consigné. |
| Service de fond, écran éteint | Battement 30 s sans interruption pendant 5 min 40 (12 allers-retours, CRC vérifié à chaque fois). |

Détail complet, y compris le lien dupliqué par rotation d'adresse BLE trouvé
pendant l'essai « écran éteint » : `00-journal.md`, entrée du 2026-09-28
« US-213 ».

### Suite de conformité (US-105) — transcrite, pas encore exécutée depuis Rust

`crates/dengon-ble/src/conformance.rs` est écrite en Rust et ne peut piloter un
objet Kotlin qu'à travers une *callback interface* UniFFI (US-302). En
attendant, `AndroidTransportConformiteTest` **transcrit** les 12 cas un pour
un (mêmes noms `cas_…`, mêmes messages), pilotés par une `FauxRadio` qui
fragmente réellement les trames comme le ferait l'autre téléphone. Écart
assumé, à résorber à l'US-302 (le fichier disparaîtra au profit de la suite
Rust elle-même).

## Flux principal (exemple)

1. L'utilisateur ouvre l'app → `MainActivity.onCreate` vérifie
   `BlePermissions.allGranted()`.
2. Si non accordées → écran affiche l'explication + bouton « Autoriser » →
   `ActivityResultContracts.RequestMultiplePermissions` ouvre les dialogues
   système un par un.
3. Une fois toutes accordées → `startMeshService()` lance
   `MeshForegroundService` via `startForegroundService` (Android 8+).
4. Le service crée son canal de notification, publie la notification
   permanente, appelle `ServiceCompat.startForeground(...)` avec le type
   `connectedDevice`, retourne `START_STICKY`.
5. Écran éteint : le service continue de tourner car il est un vrai
   foreground service avec le bon type déclaré côté manifest **et** côté
   code — c'est ce que le test manuel de 5 minutes doit démontrer.

## Dépendances

- **Internes :** aucune (le squelette n'appelle pas encore `dengon-ffi`,
  qui n'existe pas encore — US-106/US-302).
- **Externes (Gradle) :** `androidx.core:core-ktx`, `androidx.activity:activity-compose`,
  `androidx.compose.material3`, BOM Compose `2024.06.00`. AGP `8.5.2`,
  Kotlin `1.9.24`, Gradle `8.9`.

## Décisions d'implémentation

- **Release obfuscée + shrinkée** (`isMinifyEnabled = true`, `isShrinkResources
  = true` dans `app/build.gradle.kts`) et **`usesCleartextTraffic="false"`**
  explicite dans le manifest : corrections suite au Quality Gate SonarCloud
  de la PR #56 (`Security Rating on New Code` = C, règles `kotlin:S7204` et
  `xml:S5332`). Vérifié : `./gradlew assembleRelease` passe avec R8 activé,
  aucune règle proguard custom nécessaire (consumer rules AndroidX/Compose
  suffisent). Voir `docs/suivi/00-journal.md`, entrée du 2026-09-09.
- **Dependency locking activé** (`android/build.gradle.kts`,
  `resolutionStrategy.activateDependencyLocking()` sur tous les
  sous-projets) + `app/gradle.lockfile` versionné : corrige `text:S8569`
  (versions de dépendances non verrouillées) pour les dépendances de
  `:app`. Régénérer avec `./gradlew :app:dependencies --write-locks` après
  tout changement de dépendance dans `app/build.gradle.kts`.
- **`text:S8569` réapparu ensuite sur `android/build.gradle.kts` (le
  fichier racine)** : le locking ci-dessus ne couvre pas la résolution des
  **plugins** (`plugins{}` du build racine), mécanisme séparé des
  configurations de dépendances d'un sous-projet. Fixé avec la
  **dependency verification** de Gradle : `gradle/verification-metadata.xml`
  généré via `./gradlew --write-verification-metadata sha256 <tasks>`, qui
  couvre tout ce qui est résolu, plugins compris. Régénérer après tout
  changement de version dans `gradle/libs.versions.toml` (sinon le build
  échoue : entrée manquante dans les checksums).
- **`gradle/verification-metadata.xml` incomplet sur clone frais** (relevé en
  revue de la PR #56) : la première génération ne contenait le checksum que
  du `.pom` de `org.junit:junit-bom` (5.9.2 et 5.9.3), pas du `.module`
  (Gradle Module Metadata), que Gradle préfère depuis la version 6 dès qu'il
  est présent — donc échec dès qu'un environnement (clone frais, future CI)
  doit réellement le résoudre. Cause : le fichier avait été régénéré sur un
  `GRADLE_USER_HOME` déjà chaud pour ce module (`.module` déjà en cache
  local, jamais re-téléchargé donc jamais re-checksummé). Fixé en vidant
  `~/.gradle/caches/modules-2` avant de relancer `./gradlew
  --write-verification-metadata sha256 clean assembleDebug
  testDebugUnitTest assembleRelease` — voir `04-apprentissages.md`.
- **`aapt2` (et tout artefact avec classifier `os`/`arch`) : le checksum
  manque toujours pour les plateformes autres que celle qui a régénéré le
  fichier.** `--write-verification-metadata` ne consigne que ce qui est
  **résolu sur la machine qui le lance** — `aapt2-<version>-osx.jar`,
  `-linux.jar`, `-windows.jar` sont trois artefacts Maven distincts, pas des
  variantes d'un seul. La procédure « vider `~/.gradle/caches/modules-2` +
  régénérer » ci-dessus (issue de la PR #56, faite sur macOS) n'a donc
  produit que le checksum `osx` : le fichier restait sans checksum Linux
  (seul `windows` existait déjà avant #56, origine inconnue) — cassé au premier clone frais sur
  Linux/CI (retour de revue #72, round 1, point d'OswinFreyr : **ce piège
  reviendra à chaque montée de version d'AGP** tant que personne ne le
  documente). Pas de parade générique côté Gradle : pour chaque classifier
  qu'on n'a pas la machine pour régénérer soi-même, télécharger le jar
  officiel depuis `dl.google.com/android/maven2/...` et calculer
  `sha256sum` à la main (c'est ce qui a été fait pour Linux, voir
  `00-journal.md`, entrée du 2026-09-28 « US-109 : retours de revue
  d'OswinFreyr sur la PR #72 ») — ou demander à quelqu'un qui a la
  bonne plateforme de régénérer et fournir juste sa nouvelle entrée
  `verification-metadata.xml`.
- **Version catalog** (`gradle/libs.versions.toml`) : corrige `kotlin:S6624`
  (« Do not hardcode version numbers ») en centralisant toutes les versions
  (AGP, Kotlin, Compose, dépendances) à un seul endroit, référencées via
  `libs.xxx` / `libs.plugins.xxx` dans les scripts Gradle.
- **`applicationId`/`namespace` = `com.dengon.app`, `minSdk=26`,
  `compileSdk`/`targetSdk=34`** : non fixés par `docs/synthese/` → choisis
  ici (voir journal du 2026-09-09). À valider en équipe si un autre nom de
  package est préféré (ex. reprenant `G1TS23`).
- **`BLUETOOTH_SCAN` avec `usesPermissionFlags="neverForLocation"`** : le
  scan ne sert qu'à trouver le service GATT `dengon`, jamais à calculer une
  position → pas besoin d'exiger la localisation sur Android 12+.
- **`START_STICKY`** : un relais doit rester joignable ; si le système tue
  le service faute de mémoire, on veut qu'il redémarre.
- **Icône de lancement en vecteurs** (`drawable/ic_launcher_*.xml` +
  `mipmap-anydpi-v26/`) plutôt qu'en PNG : évite de fournir des assets
  bitmap pour un squelette, `minSdk=26` rend l'adaptive icon toujours
  disponible (pas besoin de repli `mipmap-mdpi/…`).

## Tests

- **Messagerie (US-214, 2026-09-28)** : `ConversationsViewModelTest`, 9
  tests JVM alimentés par le bouchon : liste initiale, ouvrir/fermer,
  conversation inconnue, envoi (message ajouté au fil, statut `QUEUED`,
  saisie vidée, dernier message suivi), pair connecté → `IN_FLIGHT` via
  `sonder`, brouillon blanc sans effet, erreur du nœud affichée avec
  brouillon conservé, libellés des 6 statuts, fabrique. + 1 test de
  régression du bouchon (réponse dans la conversation canned). Mutation :
  retirer le correctif du bouchon fait échouer 3 tests.
  `./gradlew testDebugUnitTest assembleDebug assembleRelease` → BUILD
  SUCCESSFUL (21 tests). **Rendu sur appareil non vérifié** (aucun appareil
  ni émulateur sur le poste) : à faire sur la matrice, captures dans la PR.
- `BlePermissionsTest` (`src/test/.../BlePermissionsTest.kt`) : vérifie que
  l'ensemble de permissions renvoyé couvre soit le triplet BLE moderne, soit
  la localisation legacy. Tourne en JVM pur (`unitTests.isReturnDefaultValues
  = true` dans `app/build.gradle.kts`), sans Robolectric — donc ne teste
  **pas** le branchement réel par version d'API (nécessiterait un test
  instrumenté ou Robolectric, pas fait ici).
- Commande : `cd android && ./gradlew testDebugUnitTest` → **BUILD
  SUCCESSFUL** (voir `docs/suivi/00-journal.md`, entrée du 2026-09-09).
- `./gradlew assembleDebug` → **BUILD SUCCESSFUL**, APK généré dans
  `app/build/outputs/apk/debug/app-debug.apk`.
- `./gradlew clean assembleDebug testDebugUnitTest assembleRelease` rejoué
  avec `~/.gradle/caches/modules-2` vidé (dependency verification
  effectivement testée à froid, pas juste régénérée) → **BUILD SUCCESSFUL**
  (voir `00-journal.md`, entrée « corrections revue PR #56 »).
- **Fait, sur 2 appareils** : test manuel « ≥ 5 min écran éteint sur
  appareil réel ».
  - **2026-09-16, Paul, Pixel 8 Pro (Android 17)** : service démarré
    09:39:53 heure de Paris (PID 25615, `isForeground=true`), écran éteint
    5 min 25 s, revérifié à 09:59:19 — **même PID**, notification
    permanente toujours présente. Limite notée par Paul : appareil **en
    charge** pendant le test, donc jamais entré en Doze réel (voir
    commentaire de l'issue #9 du 16/09, posté à 08:03 UTC).
  - **2026-09-26, Olivier, Samsung Galaxy A16 / SM-A165F (Android 16)** :
    service démarré 12:14:36, écran éteint, revérifié à 12:21:06 (6 min 30) —
    même `ServiceRecord`/PID, notification `ONGOING_EVENT` toujours
    affichée. **Connexion `adb` en USB** (poste de travail branché toute la
    session) : **même limite que le test de Paul**, l'appareil était en
    charge — pas de vrai Doze non plus sur ce second test (retour de revue
    #74, point d'OswinFreyr).
  - Le résultat de Paul a bien été obtenu et commenté sur l'issue #9 le
    16/09, mais **jamais reporté dans cette fiche ni dans le journal**
    avant cette correction (voir `00-journal.md`, entrée du 26/09). Les
    deux tests ci-dessus couvrent un début de matrice d'appareils
    (2 modèles, 2 versions Android) mais **aucun des deux ne couvre Doze
    réel** — voir « Limites connues » ci-dessous pour la procédure à
    suivre plus tard.
- **US-213, 2026-09-28, même couple d'appareils (Pixel 8 Pro / Galaxy A16),
  toujours en charge USB** : 48 tests JVM (`AndroidTransportConformiteTest`,
  `AndroidTransportTest`, `FragmentationBleTest`), 0 échec, plus une session
  d'essais réels — connexion (un seul lien, règle anti-boucle confirmée),
  message court et grande trame (5000 o) dans les deux sens vérifiés par
  CRC32, déconnexion brutale par éloignement physique réel (asymétrique
  `BRUTALE`/`PROPRE` selon le rôle GATT — limite Android, pas un bug),
  battement 30 s sans interruption pendant 5 min 40 écran éteint (12
  allers-retours). **Même limite « en charge »** que les tests US-109
  ci-dessus : pas de vrai Doze testé. Détail complet, y compris le lien
  dupliqué par rotation d'adresse BLE trouvé en cours d'essai :
  `00-journal.md`, entrée du 2026-09-28 « US-213 ».

## Limites connues / TODO

- ~~Aucune logique BLE réelle~~ **`BluetoothGattServer`/`Scanner`/`Advertiser`
  implémentés par US-213** (`AndroidTransport` + `GattRadio`), testés sur 2
  appareils réels — voir la section dédiée ci-dessus. Reste : brancher sur
  `dengon-core` (US-306) et la vraie identité (`peerID`).
- **Détection de déconnexion brutale asymétrique selon le rôle GATT** (limite
  Android, US-213) : fiable côté central, pas côté périphérique (le rappel
  serveur `onConnectionStateChange` rend quasi toujours `status=0`). Écart
  consigné dans `03-ecarts-conception.md`.
- **Liens dupliqués possibles** si l'adresse BLE annoncée par un pair change
  en cours de session (rotation d'adresse privée résolvable côté Android) :
  `GattRadio` déduplique par adresse MAC, pas par identité cryptographique.
  Observé une fois en test réel (US-213). Résolution prévue côté `sync`
  (identité par `peerID`/`ANNOUNCE`), pas côté `Transport`. Écart consigné.
- Messagerie et appairage branchés sur le **bouchon** FFI (US-106), pas sur le
  vrai nœud (US-306). Le bouchon ne produit jamais de message entrant ni de
  changement de statut : l'écran les affiche s'ils arrivent, mais la démo
  ne montre que « En attente » / « Parti » à l'envoi. `AndroidTransport`
  (US-213) n'est pas non plus branché sur `dengon-core` : c'est un
  `Transport` qui fonctionne, sans encore transporter le protocole dengon.
- `unreadCount` n'est jamais remis à zéro : le contrat v0 n'a pas de
  `mark_read` (US-214 hors périmètre, à ajouter au contrat).
- Pas de navigation Compose (`navigation-compose` non ajouté, pour ne pas
  toucher au verrouillage des dépendances) : quatre états booléens dans
  `MainActivity` (messagerie, appairage, transport, spike). À revoir si un
  cinquième écran s'ajoute.
- Pas de CI Android (`android.yml`) — relève de US-113/US-222 (issue #79
  créée pour un job `android.yml` minimal, retour de revue PR #72).
- SDK Android installé localement pour vérifier le build de cette session,
  mais **pas dans le dépôt** (outillage machine ; chaque poste/CI devra
  installer le sien, ou la CI Android future s'en chargera).
- **Doze réel non testé** (les deux tests US-109 de la section « Tests »
  ci-dessus ont été faits appareil en charge, via `adb` USB) — au-delà du
  critère d'acceptation de US-109 (« ≥ 5 min écran éteint », rempli), mais
  c'est la vraie contrainte visée par
  `docs/synthese/10-benchmarks-mvp-tests.md` §2.7. Pour le couvrir sans
  attendre 30 min débranché : `adb shell dumpsys battery unplug` puis `adb
  shell dumpsys deviceidle force-idle`, revérifier le service, puis `adb
  shell dumpsys deviceidle unforce` et `adb shell dumpsys battery reset`
  (retour de revue d'OswinFreyr, PR #74).
- ~~Spike C (US-103) exécuté partiellement~~ **exécuté intégralement le
  2026-09-28** (2 vrais Android, 4/4 critères de l'issue #3 démontrés — voir
  « Spike C » ci-dessus). Cette puce était encore au stade « partiel » ici
  après le merge de la PR #67, corrigé au passage.

## Appairage par QR + code 60 chiffres (US-215)

Le scénario 1 du DoD commence ici : deux personnes vérifient qu'elles
parlent bien l'une à l'autre, **sans serveur** (`powl/04` §2.2-2.3).

1. Chacun affiche son QR (`dengon:v1:<base64url(pseudo ‖ pub_static ‖ pub_sign)>`).
2. Chacun scanne celui de l'autre (caméra, `zxing-android-embedded`).
3. Les deux écrans affichent le **même** code de 60 chiffres (3 lignes de 4
   groupes) ; on le lit à voix haute et on **confirme explicitement** —
   ou on signale une différence (possible interception).

Sous le code, l'écran rappelle mon QR : l'autre téléphone doit encore le
scanner (défaut trouvé sur appareil, voir ci-dessous).

| Élément | Fichier | Rôle |
|---|---|---|
| `AppairageViewModel` | `ui/appairage/AppairageViewModel.kt` | État `StateFlow` ; `onQrScanne`, `confirmer`, `refuser`, `recommencer`. JVM pur. |
| `AppairageScreen` | `ui/appairage/AppairageScreen.kt` | Affichage, lancement du scanner (`ScanContract`), boutons. |
| `matriceQr` | `ui/appairage/QrCode.kt` | QR garanti détectable : essaie les 8 masques si besoin. |
| `IdentiteLocale` | `identite/IdentiteLocale.kt` | Identité provisoire, pseudo hexadécimal (8 octets, tous aléatoires). |

**Testé sur Pixel 8 Pro (Android 17) + Galaxy A16 (Android 16)** : lecture
croisée caméra, codes identiques (`99083 88326 31081 93212 49943 35746 33429 06232 54259 64334 82577 91716`), contacts marqués vérifiés.
Captures : [`../assets/us-215/`](../assets/us-215/).

**Défauts trouvés en route :** QR indétectable pour certains contenus
(corrigé par le choix du masque) ; même `peerId` pour tous les téléphones
(pseudo `appareil-xxxx`, corrigé en `tel-xxxx`) ; mon QR masqué après mon
scan (corrigé). Détail : `00-journal.md`, entrée US-215.

**Corrections de la revue de la PR #94 (2026-09-28) :**
1. `tel-xxxx` ne faisait encore varier que 2 des 8 octets du `peerId` (le
   préfixe `tel-` étant constant, 2^16 valeurs) — pseudo maintenant purement
   hexadécimal, sans préfixe, les 4 octets de source aléatoire couvrant
   toute la fenêtre du `peerId` (2^32 valeurs).
2. Repli silencieux de `matriceQr` sur un QR connu indétectable → `Log.w`
   ajouté.
3. Encodage du QR (`matriceQr`) exécuté sur le thread UI dans `remember` →
   déplacé sur `Dispatchers.Default` via `produceState`.

**Limites :** contacts vérifiés en mémoire seulement (pas d'appel FFI
« marquer vérifié ») ; identité et code = bouchon (vraie crypto à l'US-306) ;
navigation minimale, à fusionner avec celle de US-214 (PR #87) ; corrections
ci-dessus non revérifiées sur appareil réel (couvertes par les tests JVM
existants/adaptés).

## Pour l'oral

C'est le squelette qui prouve qu'Android peut faire tourner un service qui
« écoute » en Bluetooth même écran éteint — la contrainte technique qui a
fait éliminer Flutter/React Native et l'app iOS du MVP (voir
`docs/synthese/10-benchmarks-mvp-tests.md` §2.2). Le point à montrer : la
déclaration double (manifest + code) du type de service `connectedDevice`,
imposée par Android 14, et pourquoi une notification permanente est
incontournable (l'utilisateur doit savoir que son téléphone relaie du
trafic pour d'autres).
