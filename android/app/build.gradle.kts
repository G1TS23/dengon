plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

android {
    namespace = "com.dengon.app"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.dengon.app"
        // BLUETOOTH_SCAN / CONNECT / ADVERTISE (API 31+) et permissions de
        // localisation legacy (< API 31) sont gérées à l'exécution — voir
        // ble/BlePermissions.kt. minSdk 26 = premières API BLE peripheral
        // (BluetoothGattServer/Advertiser) stables sur la majorité des OEM.
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // US-302 : seules ABI pour lesquelles build-ffi.sh construit
        // libdengon_ffi.so. Sans ce filtre, l'AAR de JNA ajoute son
        // libjnidispatch pour armeabi-v7a, x86, mips… : un téléphone armv7
        // installerait l'APK puis planterait au premier appel FFI.
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            // Obfuscation/shrinking en release (Sonar kotlin:S7204) : rend la
            // rétro-ingénierie de l'APK plus coûteuse. Les règles consumer-proguard
            // fournies par AndroidX/Compose couvrent les dépendances utilisées ici.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
        unitTests.all { test ->
            // US-302 : les tests JVM qui passent par le vrai FFI chargent la
            // libdengon_ffi.so construite pour l'hôte par
            // `android/scripts/build-ffi.sh hote` (target/debug/ du workspace
            // Cargo). Absente (Android Studio sous Windows), ces tests sont
            // ignorés, pas en échec — voir `FfiNatif` dans les tests.
            val libHote = providers.gradleProperty("dengon.ffi.libHote")
                .getOrElse(rootProject.file("../target/debug").absolutePath)
            test.systemProperty("jna.library.path", libHote)
        }
    }

    buildFeatures {
        compose = true
    }

    composeOptions {
        kotlinCompilerExtensionVersion = libs.versions.composeCompiler.get()
    }

    packaging {
        resources {
            excludes += "/META-INF/{AL2.0,LGPL2.1}"
        }
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.activity.compose)

    val composeBom = platform(libs.androidx.compose.bom)
    implementation(composeBom)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    // US-215 : QR d'identité. `zxing-core` (Java pur) dessine le QR et le
    // décode dans les tests JVM ; `zxing-android-embedded` fournit l'écran de
    // scan caméra (et demande lui-même la permission CAMERA).
    implementation(libs.zxing.core)
    implementation(libs.zxing.android.embedded)
    // US-302 : pont vers libdengon_ffi.so. L'AAR embarque le jnidispatch
    // Android ; les tests JVM prennent le JAR, qui embarque celui de l'hôte.
    implementation(libs.jna) { artifact { type = "aar" } }

    testImplementation(libs.junit)
    testImplementation(libs.jna) { artifact { type = "jar" } }

    androidTestImplementation(composeBom)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.espresso.core)

    debugImplementation(libs.androidx.compose.ui.tooling)
}
