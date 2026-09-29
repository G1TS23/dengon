# `isMinifyEnabled` / `isShrinkResources` sont actifs en release (Sonar
# kotlin:S7204). AndroidX/Compose apportent leurs propres règles.

# US-302 : JNA retrouve par réflexion les classes qu'il remplit depuis le code
# natif (Structure, Callback) et les méthodes de l'interface `Library`
# générée par UniFFI. Renommées ou retirées par R8, les appels à
# libdengon_ffi.so échoueraient à l'exécution, pas à la compilation.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class com.dengon.app.ffi.** { *; }
-dontwarn java.awt.**
