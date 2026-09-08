# 09 — Benchmark des technologies & choix conseillés

> Cette fiche compare, **couche par couche**, les technologies possibles pour dengon, puis
> propose une **stack recommandée** cohérente pour une équipe débutante. Chaque comparatif
> est noté selon des critères simples et se termine par un choix argumenté. La synthèse
> finale (§9) donne **une** pile à adopter et **une** alternative.

## 1. Méthode & critères de notation

Chaque technologie est évaluée sur 5 critères (note ⭐ = faible → ⭐⭐⭐⭐⭐ = fort) :

- **Facilité débutant** — courbe d'apprentissage, qualité de la doc.
- **Adéquation** — colle-t-elle au besoin précis de dengon ?
- **Communauté / ressources** — tutoriels, exemples, réponses en ligne.
- **Coût** — idéalement gratuit / open source.
- **Pérennité** — projet vivant, maintenu.

Rappel des contraintes (fiches 07–08) : livrable = **APK Android**, nœuds = **téléphones +
ESP32**, réseau = **BLE mesh + store-and-forward**, passerelle = **ESP32 → MQTT → dashboard**,
équipe **débutante**.

---

## 2. Couche 1 — Framework de l'application mobile

| Techno | Facilité | Adéquation | Communauté | Coût | Pérennité | Verdict |
|---|---|---|---|---|---|---|
| **Flutter (Dart)** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | gratuit | ⭐⭐⭐⭐⭐ | ✅ **recommandé** |
| **Kotlin natif (Android)** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | gratuit | ⭐⭐⭐⭐⭐ | 👍 si contrôle BLE maximal |
| **React Native (JS/TS)** | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ | gratuit | ⭐⭐⭐⭐ | ➖ BLE périphérique faible |
| **MIT App Inventor** | ⭐⭐⭐⭐⭐ | ⭐ | ⭐⭐⭐ | gratuit | ⭐⭐⭐ | ❌ trop limité (pas de crypto/mesh sérieux) |

**Analyse.** Flutter offre le meilleur compromis pour débuter : un seul langage (Dart), une
compilation `.apk` en une commande, une UI rapide et d'excellentes ressources. Kotlin natif
donne **le** contrôle BLE le plus complet (utile pour le rôle « périphérique » du palier 3),
mais il est plus verbeux. React Native gère mal le BLE périphérique. App Inventor est parfait
pour un premier contact mais ne permettra ni la crypto sérieuse ni la logique de mesh.

> **Choix : Flutter.** Repli possible vers **Kotlin natif** si le rôle périphérique BLE pose
> problème en Flutter au palier 3.

## 3. Couche 2 — Bibliothèque BLE (côté app Flutter)

| Plugin | Rôles BLE | Facilité | Adéquation | Verdict |
|---|---|---|---|---|
| **flutter_blue_plus** | **central** | ⭐⭐⭐⭐ | paliers 1–2 | ✅ pour démarrer |
| **bluetooth_low_energy** | **central + périphérique** | ⭐⭐⭐ | palier 3 (mesh tél.↔tél.) | ✅ pour le mesh complet |
| **ble_peripheral** | **périphérique** | ⭐⭐⭐ | complément | 👍 si besoin ciblé |
| **flutter_reactive_ble** | central | ⭐⭐⭐⭐ | alternative à flutter_blue_plus | 👍 alternative |

**Analyse.** `flutter_blue_plus` est le plus populaire et le plus documenté, mais il ne fait
que « central » (scanner/se connecter) → parfait pour **téléphone ↔ ESP32** (paliers 1–2).
Pour le **mesh téléphone-à-téléphone** (palier 3), il faut le rôle « périphérique » →
`bluetooth_low_energy` (central **et** périphérique) ou `ble_peripheral`.

> **Choix : commencer avec `flutter_blue_plus`**, migrer/compléter avec
> `bluetooth_low_energy` au palier 3. (En Kotlin natif, tout passe par l'API
> `BluetoothLeScanner` / `BluetoothLeAdvertiser` officielle — pas de plugin tiers.)

*Sources : [pub.dev – flutter_blue_plus](https://pub.dev/packages/flutter_blue_plus),
[pub.dev – bluetooth_low_energy](https://pub.dev/packages/bluetooth_low_energy),
[GitHub – ble_peripheral](https://github.com/rohitsangwan01/ble_peripheral).*

## 4. Couche 3 — Environnement de développement du firmware ESP32

| Techno | Facilité | Adéquation | Communauté | Verdict |
|---|---|---|---|---|
| **Arduino (framework) + PlatformIO** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ✅ **recommandé** |
| **Arduino IDE seul** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | 👍 pour tout débuter |
| **ESP-IDF (natif Espressif)** | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ➖ puissant mais raide |
| **MicroPython** | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ➖ BLE/crypto moins matures sur ESP32 |

**Analyse.** Le **framework Arduino** (langage C++) est de loin le mieux documenté pour
l'ESP32 et suffit largement pour du BLE + MQTT. **PlatformIO** (extension de VS Code)
l'enveloppe dans un vrai environnement de projet (gestion des libs, versions, multi-cartes) —
idéal dès qu'on dépasse le simple test. **ESP-IDF** est l'outil « pro » d'Espressif (requis
seulement si l'on veut la pile *Bluetooth Mesh normalisée* ESP-BLE-MESH — inutile ici, cf.
fiche 07 §2). MicroPython est sympathique mais son support BLE/crypto sur ESP32 est moins
solide.

> **Choix : framework Arduino, édité via PlatformIO** (démarrer les tout premiers essais dans
> l'Arduino IDE si c'est plus rassurant).

## 5. Couche 4 — Cryptographie

Il faut les **mêmes primitives** des deux côtés (fiche 02) : échange **X25519**, chiffrement
authentifié **AES-256-GCM** ou **ChaCha20-Poly1305**, signatures **Ed25519**. Règle absolue :
**bibliothèque éprouvée, jamais de crypto maison** (leçon Bridgefy, fiche 02).

### Côté application (Flutter/Dart)

| Lib | Contenu | Verdict |
|---|---|---|
| **sodium_libs / sodium** (libsodium) | X25519, Ed25519, (X)ChaCha20-Poly1305, AES-GCM | ✅ **recommandé** (libsodium = référence) |
| **cryptography** (pur Dart) | X25519, Ed25519, AES-GCM, ChaCha20 | 👍 alternative sans code natif |
| **pointycastle** | boîte à outils bas niveau | ➖ plus verbeux |

### Côté ESP32 (Arduino/C++)

| Lib | Contenu | Verdict |
|---|---|---|
| **Arduino Cryptography Library (rweather)** | Ed25519, Curve25519 (X25519), ChaCha20-Poly1305, **AES accéléré par le matériel ESP32** | ✅ **recommandé** |
| **libsodium (port ESP32)** | suite complète, même API que côté app | 👍 cohérent avec l'app, un peu plus lourd |
| **mbedTLS** (inclus dans l'ESP32) | TLS, AES, ECC | 👍 déjà présent, API plus austère |

**Analyse.** Utiliser **libsodium** des deux côtés (app **et** ESP32) donne la **même API**
et évite les incompatibilités de format — c'est le choix le plus sûr. Si libsodium pèse trop
sur l'ESP32, la lib **rweather** couvre exactement les mêmes algorithmes et profite de
**l'accélération AES matérielle** de l'ESP32.

> **Choix : `sodium_libs` (app) + libsodium ou rweather (ESP32).** Le plus important :
> **fixer un format d'octets identique** des deux côtés (tailles de clés, nonces, ordre des
> champs) et le documenter dans le « format de message ».

*Sources : [pub.dev – sodium_libs](https://pub.dev/packages/sodium_libs),
[pub.dev – sodium](https://pub.dev/packages/sodium),
[Arduino Cryptography Library – Ed25519 (rweather)](https://rweather.github.io/arduinolibs/classEd25519.html),
[rweather – AES accéléré ESP32](https://rweather.github.io/arduinolibs/AESEsp32_8cpp_source.html),
[libsodium sur ESP32 (forum)](https://esp32.com/viewtopic.php?t=9243).*

## 6. Couche 5 — Broker MQTT (le tuyau ESP32 → dashboard)

| Broker | Type | Facilité | Coût | Verdict |
|---|---|---|---|---|
| **Mosquitto** | local (à installer) | ⭐⭐⭐⭐ | gratuit | ✅ pour dev/démo en local |
| **HiveMQ Cloud** | cloud (offre gratuite) | ⭐⭐⭐⭐ | gratuit (petit) | ✅ si démo « en ligne » |
| **Broker intégré ThingsBoard** | inclus | ⭐⭐⭐⭐ | gratuit | 👍 si on choisit ThingsBoard |
| **broker public de test** | cloud ouvert | ⭐⭐⭐⭐⭐ | gratuit | ➖ **jamais** de données réelles (non privé) |

> **Choix : Mosquitto en local** pour développer, **HiveMQ Cloud (gratuit)** si la démo doit
> être visible « depuis Internet ». Toujours activer **authentification + TLS** dès qu'on
> sort du réseau local (fiche 05).

## 7. Couche 6 — Dashboard / supervision

| Outil | Facilité | Rendu | Adéquation | Coût | Verdict |
|---|---|---|---|---|---|
| **Node-RED (+ dashboard)** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | gratuit | ✅ **le plus pédagogique** |
| **ThingsBoard** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | gratuit (CE) / cloud | ✅ **le plus « pro »** |
| **Grafana + InfluxDB** | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ (séries temporelles) | gratuit | 👍 joli mais plus à configurer |
| **Page web maison (MQTT-over-WebSocket)** | ⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | gratuit | 👍 contrôle total, plus de dev |

**Analyse.** **Node-RED** se « câble » visuellement à la souris (MQTT in → traitement →
widget) : idéal pour comprendre et montrer, avec un dashboard intégré. **ThingsBoard** donne
le rendu le plus professionnel (widgets, gestion d'appareils, broker MQTT inclus) au prix
d'une prise en main un peu plus longue. Grafana brille sur les **courbes temporelles** mais
demande une base type InfluxDB. Une page maison offre un contrôle total mais coûte du temps.

> **Choix : Node-RED** pour aller vite et bien expliquer ; **ThingsBoard** si vous visez un
> rendu vitrine pour la soutenance.

*Sources : [thingshost – ThingsBoard vs Grafana (2026)](https://thingshost.de/en/blog/thingsboard-vs-grafana),
[wz-it – plateformes IoT open source comparées](https://wz-it.com/en/knowledge/iot/open-source-iot-platforms-compared/),
[wz-it – Node-RED MQTT dashboard](https://wz-it.com/en/knowledge/iot/node-red-mqtt-dashboard/),
[Steve's Internet Guide – IoT MQTT dashboards](http://www.steves-internet-guide.com/iot-mqtt-dashboards/).*

## 8. Couche 7 — Persistance locale (files de stockage)

Le store-and-forward (fiche 03) a besoin de **garder les messages** même après un
redémarrage.

- **Côté app (Flutter)** : **Isar** ou **Hive** (bases locales rapides et simples) ;
  **SQLite** (`sqflite`/`drift`) si l'on préfère du SQL. → *Choix : Hive/Isar pour la
  simplicité.*
- **Côté ESP32** : **NVS** (clé-valeur, intégré) pour de petites files ; **LittleFS** ou
  **SPIFFS** pour un vrai système de fichiers en flash. → *Choix : NVS pour commencer,
  LittleFS si les volumes grandissent.*

## 9. Synthèse — la stack recommandée pour dengon

> **Pile « débutant, robuste, cohérente » (recommandée) :**
>
> | Couche | Choix |
> |---|---|
> | Application Android | **Flutter (Dart)** |
> | BLE (app) | **flutter_blue_plus** (P1–P2) → **bluetooth_low_energy** (P3) |
> | Firmware ESP32 | **framework Arduino via PlatformIO** |
> | Cryptographie | **sodium_libs** (app) + **libsodium / rweather** (ESP32) |
> | Broker MQTT | **Mosquitto** (local) / **HiveMQ Cloud** (en ligne) |
> | Dashboard | **Node-RED** (ou **ThingsBoard** pour la vitrine) |
> | Persistance | **Hive/Isar** (app) + **NVS/LittleFS** (ESP32) |

> **Pile alternative « contrôle maximal » :** Application **Kotlin natif** (API BLE
> officielle, meilleur pour le rôle périphérique du palier 3) + ESP32 **ESP-IDF** + crypto
> **libsodium/mbedTLS** + **ThingsBoard**. Plus exigeante, à réserver si l'équipe est à
> l'aise en programmation.

### Pourquoi cette pile ?
- **Cohérence crypto** : libsodium des deux côtés = mêmes formats, moins de bugs.
- **Progressivité** : `flutter_blue_plus` couvre 80 % du projet (paliers 1–2) avant de
  toucher au plus dur (périphérique BLE).
- **Ressources** : Flutter + Arduino + Node-RED = les trois écosystèmes les mieux documentés
  pour débuter, avec des milliers de tutoriels.
- **Coût** : **100 % gratuit / open source**.

## 10. Points de vigilance transverses

- **Fixer le « format de message » avant tout code** (fiche 08 §3) : c'est le contrat qui
  lie app et ESP32, surtout pour la crypto (tailles de clés, nonces, ordre des octets).
- **Tester le rôle périphérique BLE des téléphones dès la semaine 1** (fiche 07 §5).
- **Ne jamais coder sa propre crypto** ni bricoler les nonces (fiche 02).
- **Sécuriser MQTT** (auth + TLS) hors réseau local.
- **Versionner les choix** : noter dans le dépôt les versions exactes des libs (elles
  évoluent vite).

---

### À retenir
Pile conseillée : **Flutter + flutter_blue_plus/bluetooth_low_energy + Arduino(PlatformIO) +
libsodium + Mosquitto/HiveMQ + Node-RED/ThingsBoard**. Tout est **gratuit**, **bien
documenté** et **cohérent** (même crypto des deux côtés). L'alternative « contrôle maximal »
(Kotlin natif + ESP-IDF) n'est justifiée que si l'équipe est déjà à l'aise.
