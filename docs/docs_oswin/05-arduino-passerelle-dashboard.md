# 05 — Passerelle Arduino & dashboard en ligne

> Point 6 du cahier des charges : « le message passera par une Arduino équipée d'un
> récepteur/émetteur Bluetooth, qui transmettra des informations sur un dashboard en
> ligne. » L'Arduino joue **deux rôles** : (a) un **nœud relais** de plus dans le mesh
> Bluetooth, et (b) une **passerelle** (gateway) qui remonte des **métadonnées** vers
> Internet pour la supervision.

## 1. Quel matériel choisir ?

| Option | Bluetooth | Internet ? | Verdict pour dengon |
|---|---|---|---|
| **Arduino Uno + HC-05/HC-06** | Bluetooth **Classic** (série SPP) | non (nécessite un module Wi-Fi/Ethernet en plus) | OK pour un **prototype point-à-point** ; ne fait pas de BLE mesh |
| **Arduino + HM-10** | **BLE** | non | possible, mais limité en RAM |
| **ESP32** ⭐ | **BLE natif** + Bluetooth Classic | **Wi-Fi intégré** | **recommandé** : un seul composant fait relais BLE **et** passerelle Internet |
| **Raspberry Pi** | BLE (via l'OS) | Wi-Fi/Ethernet | plus puissant mais « ce n'est plus vraiment de l'Arduino » |

**Recommandation : ESP32.** Il est programmable avec l'IDE Arduino, gère le **BLE** (donc
il s'intègre au mesh de la fiche 01) **et** le **Wi-Fi** (donc il parle au dashboard) — le
tout sur une carte à ~5–10 €. C'est le choix standard pour une passerelle **BLE → MQTT**.

*Exemples : [oh2mp/esp32_ble2mqtt](https://github.com/oh2mp/esp32_ble2mqtt) (passerelle qui
écoute des balises BLE et publie en MQTT), [Theengs OpenMQTTGateway](https://docs.openmqttgateway.com/).*

## 2. Rôle (a) — l'Arduino comme nœud relais du mesh

L'ESP32 exécute la **même logique de relais** que les autres nœuds (fiche 01) :

- il **scanne** en BLE, reçoit les paquets, vérifie l'`id_message` (anti-doublon) ;
- il **stocke** les messages non délivrés (store-and-forward, fiche 03) — en RAM et/ou en
  mémoire flash (SPIFFS/NVS) pour la persistance ;
- il **décrémente le TTL** et **retransmet** ;
- il **ne peut pas lire** le contenu (chiffré de bout en bout, fiche 02) — il ne manipule
  que l'en-tête de routage et les métadonnées.

C'est un excellent point de démo : placer l'ESP32 **entre** A et B (hors de portée directe)
pour prouver que le message **transite par l'Arduino**.

## 3. Rôle (b) — l'Arduino comme passerelle vers le dashboard

L'ESP32 relie le **monde Bluetooth** (le mesh) au **monde Internet** (le dashboard) :

```
   Mesh BLE                    ESP32 (passerelle)                 Internet
 ──────────────      ┌────────────────────────────────┐      ─────────────────
  paquets chiffrés   │  - reçoit les paquets en BLE     │      ┌──────────────┐
  + en-têtes  ─────► │  - extrait les MÉTADONNÉES       │─────►│  Broker MQTT  │
                     │    (id, sauts, horodatage, état) │ Wi-Fi│      ▼        │
                     │  - publie en MQTT / HTTP         │      │  Dashboard    │
                     └────────────────────────────────┘      │ (ThingsBoard, │
                                                              │  Node-RED…)   │
   ⚠ NE publie JAMAIS le contenu déchiffré                    └──────────────┘
```

**Règle de sécurité (rappel fiche 02) :** la passerelle remonte des **métadonnées de
supervision**, **pas** le texte des messages. Le dashboard sert à *observer le réseau*, pas
à *lire les conversations*. Sinon, on casse le chiffrement de bout en bout.

### Le protocole conseillé : MQTT

**MQTT** est le protocole de messagerie léger standard de l'IoT (publish/subscribe via un
*broker*). L'ESP32 **publie** des messages MQTT ; le dashboard **s'abonne** aux *topics*.
C'est simple, robuste, et parfaitement supporté sur ESP32.

*Sources : [Random Nerd Tutorials – ESP32 MQTT](https://randomnerdtutorials.com/esp32-mqtt-publish-subscribe-arduino-ide/),
[FlowFuse – ESP32 avec Node-RED](https://flowfuse.com/blog/2024/11/esp32-with-node-red/).*

## 4. Quel dashboard ?

| Outil | Type | Pourquoi |
|---|---|---|
| **ThingsBoard** ⭐ | plateforme IoT (open source / cloud) | dashboards prêts à l'emploi, widgets, gère MQTT nativement, SDK ESP32 officiel |
| **Node-RED** | outil de flux visuel + dashboard | très pédagogique, on « câble » les traitements à la souris |
| **Grafana + MQTT/InfluxDB** | visualisation de séries temporelles | joli pour les métriques/temps, un peu plus à configurer |
| **Page web maison** (HTML + MQTT over WebSocket) | sur-mesure | contrôle total, plus de dev |

**Recommandation : ThingsBoard** (le plus rapide pour un rendu « pro ») ou **Node-RED** (le
plus pédagogique). Les deux ont d'abondants tutoriels ESP32.

*Sources : [Hackster – ESP32 vers ThingsBoard](https://www.hackster.io/norvi/connect-esp32-to-thingsboard-over-wi-fi-visualize-iot-data-eae2ed),
[ThingsBoard client SDK (Arduino/ESP32)](https://github.com/thingsboard/thingsboard-client-sdk),
[Zbotic – ThingsBoard + ESP32](https://zbotic.in/thingsboard-iot-platform-with-esp32-open-source-dashboard/),
[GitHub – ESP32 ThingsBoard IoT Dashboard](https://github.com/hubamatyas/ESP32-Thingsboard-IoT-Dashboard).*

## 5. Que montrer sur le dashboard ? (idées de widgets)

Reprend les métriques de la fiche 03 — c'est ce qui **prouve** que le système fonctionne :

- **journal des événements** : `message X reçu / relayé / livré`, horodaté ;
- **compteur de messages** en circulation, livrés, expirés ;
- **carte / liste des nœuds** actifs et leur dernière activité ;
- **nombre de sauts** par message livré ;
- **délai de livraison** moyen (A→B) ;
- **occupation mémoire** des files de stockage des relais ;
- (si option blockchain de la fiche 04) **la chaîne d'événements** chaînés par hash,
  vérifiable — un très beau visuel pour la soutenance.

## 6. Schéma de câblage / flux minimal pour la démo

1. **A** (téléphone/PC) chiffre + signe + publie un message en BLE.
2. **ESP32** (hors de portée de B) le reçoit, le **stocke**, **publie ses métadonnées en
   MQTT** → le dashboard affiche « message reçu au nœud Arduino, 1 saut ».
3. **B** entre dans la portée de l'ESP32 → l'ESP32 **retransmet** → B **déchiffre** et
   **répond par un ACK** signé.
4. L'ACK remonte, l'ESP32 **purge** sa copie et **publie** « message livré, purge » → le
   dashboard affiche l'état final.

Ce scénario en 4 temps démontre **les 6 points** du cahier des charges en une seule
manipulation.

## 7. Pièges pratiques

- **RAM limitée** : sur ESP32, garder les files de stockage bornées (quota + purge, fiche 3).
- **Wi-Fi + BLE simultanés** : l'ESP32 partage la même radio 2,4 GHz pour Wi-Fi et BLE →
  possibles ralentissements ; tester tôt, prévoir des temporisations.
- **Sécurité de la passerelle** : MQTT doit être **authentifié + TLS** si le broker est sur
  Internet ; ne pas exposer un broker ouvert.
- **Alimentation** : un ESP32 en scan BLE continu + Wi-Fi consomme ; prévoir une bonne
  alim USB pour la démo.

---

### À retenir
Choisir un **ESP32** : il est à la fois **nœud relais BLE** du mesh **et** **passerelle
Wi-Fi**. Il remonte des **métadonnées** (jamais le contenu déchiffré) en **MQTT** vers un
dashboard **ThingsBoard** ou **Node-RED**. Le dashboard sert à **prouver et visualiser** le
store-carry-forward et, en option, la chaîne d'événements par hash.
