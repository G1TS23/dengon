// Build racine du module Android. Le vrai code applicatif est dans `app/`.
plugins {
    id("com.android.application") version "8.5.2" apply false
    id("org.jetbrains.kotlin.android") version "1.9.24" apply false
}

// Verrouille les versions résolues des dépendances (Sonar text:S8569) :
// `gradle.lockfile` par module, régénéré via `./gradlew :app:dependencies --write-locks`.
subprojects {
    configurations.all {
        resolutionStrategy.activateDependencyLocking()
    }
}
