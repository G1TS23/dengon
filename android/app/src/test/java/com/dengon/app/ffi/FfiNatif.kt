package com.dengon.app.ffi

import org.junit.Assume.assumeTrue

/**
 * Disponibilité de libdengon_ffi.so pour les tests JVM (US-302).
 *
 * Les bindings UniFFI chargent la bibliothèque native de l'hôte via JNA
 * (`jna.library.path`, fixé par `app/build.gradle.kts`). Construite par
 * `android/scripts/build-ffi.sh hote` sous WSL / Linux et en CI ; absente
 * dans Android Studio sous Windows. Là, les tests qui en dépendent sont
 * **ignorés** (JUnit `Assume`), pas en échec — et la CI, elle, les exécute.
 */
object FfiNatif {

    /** `true` si un appel au vrai FFI aboutit. Évalué une fois. */
    val disponible: Boolean by lazy {
        try {
            generateIdentity("sonde")
            true
        } catch (e: UnsatisfiedLinkError) {
            false
        } catch (e: NoClassDefFoundError) {
            // Échec d'initialisation de la classe JNA qui porte la bibliothèque.
            false
        }
    }

    /** À appeler en tête d'un test (ou dans un `@Before`) qui passe par le FFI. */
    fun exiger() {
        assumeTrue("libdengon_ffi.so absente : lancer android/scripts/build-ffi.sh hote", disponible)
    }
}
