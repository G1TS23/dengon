# Module : `android-app` (`android/`)

**Rôle en une phrase :** l'application Android (Kotlin + Compose) — pour
l'instant, un squelette qui déclare et fait tourner le service de fond BLE
requis pour un nœud mesh (US-109).
**Correspond à la conception :** `docs/synthese/04-architecture.md` §3
(impl Android du trait `Transport`) et §7 ; `docs/synthese/10-benchmarks-mvp-tests.md`
§2.7 (contraintes d'arrière-plan Android 14/15) ; `docs/olivier/proposition-organisation-github.md`
US-109.
**Dernière mise à jour :** 2026-09-11
**État :** esquisse (squelette du service de fond ; pas de logique BLE réelle
dans l'app elle-même — voir « Spike C » ci-dessous pour le code GATT jetable
qui dérisque `AndroidTransport`)

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
      MainActivity.kt        — Compose : demande permissions, démarre/arrête le service, affiche l'état
      ble/
        BlePermissions.kt        — liste des permissions requises selon Build.VERSION.SDK_INT
        MeshForegroundService.kt — service de fond, notification permanente, foregroundServiceType=connectedDevice
        spike/                   — code JETABLE du Spike C (US-103), à supprimer après la décision
          HelloMeshConstants.kt    — SERVICE_UUID/CHAR_RX/CHAR_TX/MTU visé (docs/powl/03-network-protocol.md §2, §6)
          HelloMeshPeripheral.kt   — BluetoothGattServer + BluetoothLeAdvertiser
          HelloMeshCentral.kt      — BluetoothLeScanner + BluetoothGatt (client)
          HelloMeshSpikeScreen.kt  — écran de debug Compose (accessible depuis MainActivity)
          SpikeResult.kt           — chiffres mesurés (MTU, timings, appareil)
    src/test/java/com/dengon/app/ble/
      BlePermissionsTest.kt  — test unitaire minimal (JVM, sans Robolectric)
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `MeshForegroundService` | `app/src/main/java/com/dengon/app/ble/MeshForegroundService.kt:24` | `Service` Android. `onStartCommand` appelle `ServiceCompat.startForeground(..., FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)` et retourne `START_STICKY`. Construit la notification permanente (canal `IMPORTANCE_LOW`). Pas de logique GATT — squelette seulement. |
| `BlePermissions.required()` | `app/src/main/java/com/dengon/app/ble/BlePermissions.kt:17` | Retourne le tableau de permissions à demander : `BLUETOOTH_SCAN/CONNECT/ADVERTISE` sur API 31+, `ACCESS_FINE_LOCATION` en dessous, `+POST_NOTIFICATIONS` sur API 33+. |
| `BlePermissions.allGranted()` | `app/src/main/java/com/dengon/app/ble/BlePermissions.kt:29` | Vérifie si toutes les permissions requises sont déjà accordées. |
| `MainActivity` | `app/src/main/java/com/dengon/app/MainActivity.kt:29` | `ComponentActivity` Compose : lance la demande de permissions (`RequestMultiplePermissions`), démarre `MeshForegroundService` via `ContextCompat.startForegroundService` dès qu'elles sont accordées, bouton Démarrer/Arrêter pour le test manuel. |
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

**⚠️ Non exécuté** : aucun appareil Android physique disponible dans
l'environnement où ce code a été écrit (contrainte dure de l'issue). Les
critères d'acceptation « deux téléphones échangent 20 octets », « MTU
négocié mesuré », « temps d'établissement mesuré » et « matrice d'appareils »
**ne sont donc pas encore satisfaits** — voir `docs/suivi/00-journal.md` et
« Limites connues / TODO » ci-dessous.

| Date | Appareil (central) | Appareil (peripheral) | Android | MTU négocié | Scan→connexion | Connexion→échange |
|---|---|---|---|---|---|---|
| _à remplir_ | | | | | | |

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
- **Non fait** : test manuel « ≥ 5 min écran éteint sur appareil réel »
  (critère d'acceptation US-109) — aucun appareil Android disponible dans
  l'environnement où ce squelette a été écrit. Reste à faire avant de
  clore l'US.

## Limites connues / TODO

- Aucune logique BLE réelle (`BluetoothGattServer`/`Scanner`/`Advertiser`) :
  prévue pour US-213 (`AndroidTransport`), qui implémentera le contrat
  `Transport` de `docs/synthese/04-architecture.md` §3 et remplacera ce
  squelette de service par le vrai relais.
- Pas de branchement `dengon-ffi` (bouchon US-106 pas encore fait).
- Pas de CI Android (`android.yml`) — relève de US-113/US-222.
- Test manuel des 5 minutes écran éteint non réalisé (voir ci-dessus).
- SDK Android installé localement pour vérifier le build de cette session,
  mais **pas dans le dépôt** (outillage machine ; chaque poste/CI devra
  installer le sien, ou la CI Android future s'en chargera).
- **Spike C (US-103) écrit mais non exécuté** : `ble/spike/` compile et
  `assembleDebug`/`assembleRelease`/`testDebugUnitTest` passent, mais aucun
  appareil Android physique disponible dans l'environnement où ce code a
  été écrit — impossible de produire les chiffres exigés par les critères
  d'acceptation (MTU réel, timing, matrice d'appareils). Voir « Spike C »
  ci-dessus pour le protocole de mesure à exécuter, et
  `docs/suivi/00-journal.md` pour le détail. Tant que ce n'est pas fait,
  US-103 ne peut pas être clos ni le go/no-go A-1 confirmé.

## Pour l'oral

C'est le squelette qui prouve qu'Android peut faire tourner un service qui
« écoute » en Bluetooth même écran éteint — la contrainte technique qui a
fait éliminer Flutter/React Native et l'app iOS du MVP (voir
`docs/synthese/10-benchmarks-mvp-tests.md` §2.2). Le point à montrer : la
déclaration double (manifest + code) du type de service `connectedDevice`,
imposée par Android 14, et pourquoi une notification permanente est
incontournable (l'utilisateur doit savoir que son téléphone relaie du
trafic pour d'autres).
