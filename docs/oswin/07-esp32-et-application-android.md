# 07 — Matériel ESP32-WROOM-32E & application Android (APK)

> Cette fiche répond à deux questions concrètes : **que peut faire notre carte
> ESP32-WROOM-32E** et **comment fabriquer l'application Android (.apk)**. Elle tient compte
> de l'architecture retenue pour dengon (voir §1) et elle est écrite pour des débutants :
> on privilégie **une progression par étapes** plutôt qu'un grand saut.

## 1. Architecture retenue (décidée par l'équipe)

- Les **téléphones se relaient les messages entre eux** (vrai mesh téléphone-à-téléphone,
  comme Bitchat).
- Les **cartes ESP32 sont aussi des relais** du mesh **et** jouent le rôle de **passerelle
  unique vers le dashboard** en ligne.
- **Les téléphones ne parlent jamais directement au dashboard** : toute remontée
  d'information passe par un ESP32 (Wi-Fi/MQTT, fiche 05).

```
 [Téléphone A] ⇄ BLE ⇄ [Téléphone C] ⇄ BLE ⇄ [Téléphone B]
       ⇅ BLE                  ⇅ BLE
   [ESP32 #1] ⇄ BLE ⇄ [ESP32 #2]
       │ Wi-Fi (MQTT)             (les ESP32 = relais + porte vers Internet)
       ▼
   [Dashboard en ligne]     ← métadonnées uniquement, jamais le contenu déchiffré
```

> ⚠️ **Honnêteté technique.** C'est l'option **la plus ambitieuse**. La difficulté ne vient
> pas de l'ESP32 (qui fait ça très bien) mais du **téléphone Android** qui doit jouer *en
> même temps* deux rôles BLE : « central » (il scanne/se connecte) **et** « périphérique »
> (il se fait voir des autres). Voir §5 : c'est là que se cachent les vrais pièges, et la
> raison pour laquelle on propose une **feuille de route en 3 paliers** (§7).

## 2. La carte : ESP32-WROOM-32E (module) sur carte Freenove

Le « cerveau » est le module **ESP32-WROOM-32E** d'Espressif. Caractéristiques utiles :

| Élément | Valeur | Pourquoi ça compte pour dengon |
|---|---|---|
| Processeur | **double cœur** Xtensa LX6, jusqu'à **240 MHz** | peut gérer BLE **et** Wi-Fi en parallèle |
| Mémoire vive | **520 Ko SRAM** | limite la taille des files de stockage (fiche 03) → prévoir des quotas |
| Mémoire flash | **4 Mo** (typique) | stocke le programme + une file persistante (NVS/SPIFFS) |
| Wi-Fi | **802.11 b/g/n (2,4 GHz)** | passerelle vers le dashboard (MQTT) |
| Bluetooth | **v4.2 : BR/EDR + BLE** | rôle de relais BLE dans le mesh |
| GPIO | ~34 broches | LED d'état, boutons, écran éventuel pour la démo |
| Alimentation | 5 V par USB / 3,3 V logique | une bonne alim USB suffit pour la démo |

Points d'attention :
- Le **Bluetooth et le Wi-Fi partagent la même radio 2,4 GHz** → activer les deux en continu
  peut ralentir ; à tester tôt (on peut alterner, ou dédier un ESP32 « passerelle » Wi-Fi et
  d'autres « relais » BLE si on en a plusieurs).
- La carte Freenove est programmable avec l'**IDE Arduino** ou **PlatformIO** (voir §4).
- L'ESP32 peut faire du BLE « brut » (GATT) **ou** de la norme Bluetooth Mesh
  (**ESP-BLE-MESH** dans ESP-IDF). Pour interopérer avec des téléphones, on utilisera du
  **BLE GATT « maison »** (§3), pas la pile Bluetooth Mesh normalisée.

*Sources : [Datasheet ESP32-WROOM-32E (Espressif)](https://www.espressif.com/sites/default/files/documentation/esp32-wroom-32e_esp32-wroom-32ue_datasheet_en.pdf),
[Datasheet HTML](https://documentation.espressif.com/esp32-wroom-32e_esp32-wroom-32ue_datasheet_en.html),
[Grid Connect – module 4MB Flash](https://www.gridconnect.com/products/esp32-wroom-32e-combo-wi-fi-bt-ble-module).*

## 3. Le langage commun : un « service GATT » partagé

Pour que téléphones et ESP32 se comprennent, tous parlent le **même petit protocole BLE**.
En BLE, on expose un **service GATT** avec des **caractéristiques** (des « boîtes aux
lettres »). Proposition minimale :

```text
Service "dengon"  (un UUID inventé, ex. 0000d3n6-....)
 ├─ Caractéristique "MESSAGE"  (write + notify)
 │     → on ÉCRIT un paquet chiffré dedans pour l'envoyer
 │     → on est NOTIFIÉ quand un paquet arrive
 └─ Caractéristique "ACK"      (write + notify)
       → accusés de réception (fiche 03)
```

- Chaque nœud (téléphone **ou** ESP32) expose ce service **et** sait s'y connecter chez les
  autres → c'est ce qui crée le maillage.
- Le contenu échangé est le **paquet chiffré** défini fiche 02 (en-tête clair + charge utile
  chiffrée + signature). L'ESP32 **relaie sans lire**.
- Attention à la **taille (MTU)** : un message long doit être **fragmenté** (fiche 01 §2).

## 4. La chaîne d'outils (ce qu'il faut installer)

### Côté ESP32 (le firmware)
1. **Arduino IDE** (le plus simple pour débuter) ou **PlatformIO** (dans VS Code, plus
   confortable pour un vrai projet).
2. Ajouter le **support ESP32** (« ESP32 by Espressif Systems » dans le gestionnaire de
   cartes).
3. Bibliothèques utiles : le **BLE intégré** de l'ESP32 (`BLEDevice`…), **PubSubClient** ou
   la lib MQTT pour la passerelle, **ArduinoJson**, et une lib crypto (voir fiche 02) si
   l'ESP32 doit vérifier des signatures.
4. On téléverse le programme par **USB**.

### Côté application Android (l'APK)
1. Installer le **SDK de développement** (voir §6 pour le choix de la techno).
2. Écrire l'app (UI + logique BLE + crypto).
3. **Compiler** → cela produit un fichier **`.apk`** (ou `.aab`).
4. **Installer l'APK** sur un téléphone :
   - activer « **Sources inconnues / installer des apps inconnues** » dans Android ;
   - copier le `.apk` sur le téléphone (câble, e-mail, lien) et l'ouvrir ;
   - pour une vraie diffusion (au-delà de la démo), il faudrait un **compte Google Play
     Developer** (payant) — inutile pour un projet d'école.

> Un `.apk` **non signé par le Play Store** s'installe très bien « à la main » (*sideload*).
> C'est parfait pour une démo. Prévoir juste d'expliquer aux camarades comment autoriser
> l'installation.

## 5. Le vrai piège : les rôles BLE sur Android

En BLE il y a deux rôles :
- **Central** : scanne et se connecte (le rôle « classique » d'un téléphone). **Bien
  supporté partout.**
- **Périphérique** (*peripheral* / *advertiser*) : se rend visible et **accepte** des
  connexions. **Nécessaire pour le mesh téléphone-à-téléphone**… mais :

> ⚠️ **Tous les téléphones Android ne savent pas être « périphérique »** (faire de
> l'*advertising* BLE). C'est une limitation **matérielle/pilote** bien connue : certains
> modèles renvoient `isMultipleAdvertisementSupported() = false`. Il faut donc **tester vos
> téléphones réels** tôt, et prévoir un plan B.

Autres contraintes Android à connaître :
- **Permissions** : depuis Android 12, il faut demander `BLUETOOTH_SCAN`,
  `BLUETOOTH_ADVERTISE`, `BLUETOOTH_CONNECT` (et, sur versions plus anciennes, la
  **localisation** pour scanner). L'app doit les réclamer au lancement.
- **Arrière-plan** : Android **restreint fortement** le BLE quand l'app n'est pas au premier
  plan. Pour la démo, garder **l'app ouverte à l'écran**.
- **Débit/MTU** variables selon les téléphones.

*Sources : [Android Open Source Project – BLE advertising](https://source.android.com/docs/core/connect/bluetooth/ble_advertising),
[Argenox – Android 5.0 BLE improvements](https://argenox.com/blog/android-5-0-lollipop-brings-ble-improvements),
[liste de modèles sans mode périphérique (Corona-Warn-App)](https://github.com/corona-warn-app/cwa-app-android/issues/688),
[Argenox – pourquoi le BLE Mesh peine à percer](https://argenox.com/blog/10-reasons-why-ble-mesh-has-struggled-to-gain-traction).*

## 6. Quelle techno pour l'app ? — Recommandation

Comme vous ne voulez qu'un **APK** (donc Android seulement) et que vous débutez, voici le
comparatif honnête :

| Techno | Pour | Contre | Verdict |
|---|---|---|---|
| **Flutter (Dart)** | 1 seul code, **build APK en une commande**, UI rapide, communauté énorme | le BLE **périphérique** dépend d'un plugin tiers | ✅ **recommandé pour débuter** |
| **Kotlin natif (Android Studio)** | **meilleur contrôle** du BLE (central + périphérique), API officielle | plus verbeux, courbe d'apprentissage plus raide | 👍 si vous voulez le contrôle max |
| **React Native** | JS/TS | BLE périphérique mal supporté | ➖ pas idéal ici |

**Conseil : commencez en Flutter.** Plugins BLE à connaître :
- **`flutter_blue_plus`** — rôle **central** uniquement (scanner/se connecter). Parfait pour
  le **palier 1** (téléphone ↔ ESP32).
- **`bluetooth_low_energy`** — gère **central ET périphérique** (multi-plateforme) → requis
  pour le mesh téléphone-à-téléphone.
- **`ble_peripheral`** — spécialisé rôle **périphérique**.

Si, après tests, vos téléphones ne savent pas être « périphérique » (§5), **passez le cœur
du mesh sur les ESP32** (qui, eux, savent parfaitement être périphérique) : les téléphones
restent « central », et ce sont les ESP32 qui assurent le relais. C'est le **plan B**
réaliste, et il reste fidèle à l'esprit du projet.

*Sources : [pub.dev – bluetooth_low_energy](https://pub.dev/packages/bluetooth_low_energy),
[pub.dev – flutter_blue_plus](https://pub.dev/packages/flutter_blue_plus),
[GitHub – ble_peripheral](https://github.com/rohitsangwan01/ble_peripheral),
[Medium – BLE avec Flutter + Arduino](https://medium.com/@danielwolf.dev/get-started-with-bluetooth-low-energy-using-flutter-arduino-bdf5d790edc).*

## 7. Feuille de route en 3 paliers (du plus sûr au plus ambitieux)

Cette progression garantit d'avoir **toujours une démo qui marche**, même si le palier
suivant coince.

**Palier 1 — Le lien de base (facile, à faire en premier).**
Téléphone (central) ↔ **1 ESP32** (périphérique). L'app envoie un message chiffré à l'ESP32,
qui l'affiche/relaie et **publie une métadonnée sur le dashboard**. → Prouve : émission BLE,
sécurité, passerelle + dashboard. *Rien de bloquant ici.*

**Palier 2 — Le relais et le stockage (cœur du sujet).**
Ajouter un **2ᵉ ESP32**. Un message part du téléphone A, l'ESP32 #1 le **stocke** (le
destinataire n'est pas là), puis le **retransmet** à l'ESP32 #2 quand il apparaît, qui le
donne au téléphone B. → Prouve : **store-carry-forward**, multi-sauts, ACK, purge (fiche 03).
*Les ESP32 portent le mesh : très fiable.*

**Palier 3 — Le mesh téléphone-à-téléphone (ambitieux).**
Activer le rôle **périphérique sur les téléphones** (plugin `bluetooth_low_energy`) pour
qu'ils se relaient directement, ESP32 en plus. → C'est l'objectif complet, **à tenter seulement
une fois les paliers 1–2 solides**, et **après avoir vérifié** que vos téléphones supportent
l'advertising (§5).

> **Recommandation :** viser **paliers 1 + 2** comme livrable garanti (ils couvrent déjà
> **tous** les points du cahier des charges), et présenter le **palier 3** comme
> l'aboutissement / la perspective. Ça sécurise la note tout en montrant l'ambition.

## 8. Prochaines actions concrètes

1. **Tester** dès maintenant si vos téléphones savent être « périphérique » BLE (une petite
   app de test, ou l'app *nRF Connect*, le montre).
2. Installer **Arduino IDE + support ESP32** et faire clignoter une LED (le « hello world »
   de l'ESP32).
3. Faire un premier **échange BLE téléphone ↔ ESP32** en clair (palier 1 sans crypto), puis
   **ajouter le chiffrement** (fiche 02).
4. Définir précisément le **format de paquet** (fiches 02 et 03) — c'est le contrat commun
   app ↔ ESP32.
5. Brancher la **passerelle MQTT → dashboard** (fiche 05).

---

### À retenir
L'**ESP32-WROOM-32E** est un excellent nœud : double cœur, **BLE 4.2 + Wi-Fi**, il fait
**relais BLE** et **passerelle MQTT** sans peine. Le point délicat est **côté téléphone** :
le mesh téléphone-à-téléphone exige le rôle **périphérique BLE**, que **certains Android ne
supportent pas** — d'où une **progression en 3 paliers**. Pour l'app, **Flutter** est le
meilleur point de départ pour débuter ; l'`.apk` s'installe en *sideload* sans compte Play.
