# Module : `android-app` (`android/`)

**Rôle en une phrase :** l'application Android (Kotlin + Compose) — pour
l'instant, un squelette qui déclare et fait tourner le service de fond BLE
requis pour un nœud mesh (US-109).
**Correspond à la conception :** `docs/synthese/04-architecture.md` §3
(impl Android du trait `Transport`) et §7 ; `docs/synthese/10-benchmarks-mvp-tests.md`
§2.7 (contraintes d'arrière-plan Android 14/15) ; `docs/olivier/proposition-organisation-github.md`
US-109.
**Dernière mise à jour :** 2026-09-09
**État :** esquisse (squelette du service de fond ; pas de logique BLE réelle)

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
  build.gradle.kts           — plugins AGP/Kotlin (root)
  app/
    build.gradle.kts         — applicationId com.dengon.app, minSdk 26, target/compileSdk 34, Compose
    src/main/AndroidManifest.xml
    src/main/java/com/dengon/app/
      DengonApplication.kt   — Application vide (point d'extension futur)
      MainActivity.kt        — Compose : demande permissions, démarre/arrête le service, affiche l'état
      ble/
        BlePermissions.kt        — liste des permissions requises selon Build.VERSION.SDK_INT
        MeshForegroundService.kt — service de fond, notification permanente, foregroundServiceType=connectedDevice
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

## Pour l'oral

C'est le squelette qui prouve qu'Android peut faire tourner un service qui
« écoute » en Bluetooth même écran éteint — la contrainte technique qui a
fait éliminer Flutter/React Native et l'app iOS du MVP (voir
`docs/synthese/10-benchmarks-mvp-tests.md` §2.2). Le point à montrer : la
déclaration double (manifest + code) du type de service `connectedDevice`,
imposée par Android 14, et pourquoi une notification permanente est
incontournable (l'utilisateur doit savoir que son téléphone relaie du
trafic pour d'autres).
