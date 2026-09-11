// Build racine du module Android. Le vrai code applicatif est dans `app/`.
// Versions centralisées dans gradle/libs.versions.toml (Sonar kotlin:S6624 :
// pas de numéro de version en dur dans les scripts).
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.android) apply false
}

// Verrouille les versions résolues des dépendances (Sonar text:S8569) :
// `gradle.lockfile` par module, régénéré via `./gradlew :app:dependencies --write-locks`.
subprojects {
    configurations.all {
        resolutionStrategy.activateDependencyLocking()
    }
}
