# 06 — Sources & références

Bibliographie classée par thème. Consultée le **8 septembre 2026**. Les liens marqués ⭐
sont les plus utiles pour démarrer.

## Applications de messagerie mesh (études de cas)

- ⭐ **Bitchat** (la référence à imiter) — architecture BLE mesh + TTL + store-and-forward + crypto moderne :
  - [dev.to – Offline messaging reinvented with Bitchat](https://dev.to/grenishrai/offline-messaging-reinvented-with-bitchat-5011) (détails techniques)
  - [TechTarget – What is Bitchat](https://www.techtarget.com/whatis/feature/What-is-Bitchat)
  - [TechRadar – how Bitchat works](https://www.techradar.com/phones/bitchat-is-a-new-private-bluetooth-messaging-app-that-doesnt-need-the-internet-heres-how-it-works)
  - [CNBC – Jack Dorsey launches a Bluetooth messaging rival](https://www.cnbc.com/2025/07/07/jack-dorsey-whatsapp-bluetooth.html)
  - [BeInCrypto – Bitchat expliqué](https://beincrypto.com/learn/bitchat-bluetooth-bitcoin-app/)
- ⭐ **Bridgefy** (le contre-exemple de sécurité à étudier) :
  - [« Breaking Bridgefy » — version abrégée (PDF)](https://martinralbrecht.wordpress.com/wp-content/uploads/2020/08/bridgefy-abridged.pdf)
  - [Article complet — eprint IACR 2021/214 (PDF)](https://eprint.iacr.org/2021/214.pdf)
  - [Springer – Mesh Messaging in Large-Scale Protests: Breaking Bridgefy](https://link.springer.com/chapter/10.1007/978-3-030-75539-3_16)
  - [Royal Holloway – communiqué de vulgarisation](https://www.royalholloway.ac.uk/research-and-education/subjects/information-security/news/using-messaging-service-bridgefy-could-have-dire-consequences-for-users-if-privacy-protection-issues-aren-t-fixed/)

## Bluetooth & Bluetooth Mesh

- ⭐ [Bluetooth SIG – Mesh Networking Primer](https://www.bluetooth.com/bluetooth-mesh-networking-primer/)
- ⭐ [Novel Bits – Bluetooth Mesh: the ultimate guide](https://novelbits.io/bluetooth-mesh-networking-the-ultimate-guide/) (architecture, sécurité, provisioning)
- [Bluetooth SIG – Directed Forwarding](https://www.bluetooth.com/mesh-directed-forwarding/) (routage par routes vs flooding)
- [Bluetooth SIG – Mesh FAQ](https://www.bluetooth.com/learn-about-bluetooth/topology-options/le-mesh/mesh-faq/)
- [MokoSmart – What is Bluetooth Mesh & how it works](https://www.mokosmart.com/what-is-bluetooth-mesh-how-it-works/)
- [MathWorks – Bluetooth Mesh Flooding in WSN](https://www.mathworks.com/help/bluetooth/ug/bluetooth-mesh-flooding-in-wireless-sensor-networks.html)
- [Google Patents – Managed flooding for Bluetooth mesh (US20200314735A1)](https://patents.google.com/patent/US20200314735A1/en)

## Réseaux tolérants aux délais (DTN) & store-and-forward

- ⭐ [RFC 9171 – Bundle Protocol Version 7 (texte intégral)](https://www.rfc-editor.org/rfc/rfc9171.html)
- [RFC 9171 – page d'information RFC Editor](https://www.rfc-editor.org/info/rfc9171/)
- [EmergentMind – Delay/Disruption Tolerant Network protocols](https://www.emergentmind.com/topics/delay-disruption-tolerant-network-dtn-protocols)
- [DTN7 – implémentation open source du Bundle Protocol](https://dtn7.github.io/)
- *Mots-clés pour approfondir le routage : « epidemic routing », « spray-and-wait »,
  « PRoPHET DTN routing ».*

## Cryptographie & sécurité des messages

- ⭐ [Signal – The Double Ratchet Algorithm (spécification)](https://signal.org/docs/specifications/doubleratchet/)
- [Signal Protocol – Wikipedia](https://en.wikipedia.org/wiki/Signal_Protocol)
- [Noise Protocol Framework](https://noiseprotocol.org/) (utilisé par Bitchat, WireGuard…)
- [OMEMO – Wikipedia](https://en.wikipedia.org/wiki/OMEMO) (Signal appliqué à XMPP)
- [positive-intentions – Adapting the Signal Protocol for P2P](https://positive-intentions.com/blog/p2p-signal-protocol/)
- *Bibliothèque conseillée :* **libsodium/NaCl** (X25519, Ed25519, XChaCha20-Poly1305) —
  disponible en C/Arduino, JavaScript, Python.

## Blockchain, hash chains & arbres de Merkle

- [Medium – Blockchain, Hash & Merkle tree : immutability & integrity](https://medium.com/@zlhk100/blockchain-hash-and-merkle-tree-data-immutability-and-integrity-append-only-database-eff7b621b9c3)
- [GeeksforGeeks – Blockchain Merkle Trees](https://www.geeksforgeeks.org/blockchain-merkle-trees/)
- [HackerNoon – Merkle Trees & cryptographic accumulators](https://hackernoon.com/merkle-trees-and-cryptographic-accumulators-the-mathematical-backbone-of-blockchain-integrity)
- [DEV – Merkle tree root for data integrity](https://dev.to/bloxbytes/understanding-the-concept-of-merkle-tree-root-in-blockchain-for-data-integrity-2hp0)

## Arduino / ESP32 & dashboard IoT

- ⭐ [Random Nerd Tutorials – ESP32 MQTT Publish/Subscribe (Arduino IDE)](https://randomnerdtutorials.com/esp32-mqtt-publish-subscribe-arduino-ide/)
- ⭐ [Hackster – Connect ESP32 to ThingsBoard over Wi-Fi](https://www.hackster.io/norvi/connect-esp32-to-thingsboard-over-wi-fi-visualize-iot-data-eae2ed)
- [ThingsBoard – client SDK Arduino/ESP32](https://github.com/thingsboard/thingsboard-client-sdk)
- [Zbotic – ThingsBoard IoT platform avec ESP32](https://zbotic.in/thingsboard-iot-platform-with-esp32-open-source-dashboard/)
- [GitHub – ESP32 ThingsBoard IoT Dashboard (exemple)](https://github.com/hubamatyas/ESP32-Thingsboard-IoT-Dashboard)
- [FlowFuse – Interacting with ESP32 using Node-RED and MQTT (2026)](https://flowfuse.com/blog/2024/11/esp32-with-node-red/)
- [oh2mp/esp32_ble2mqtt – passerelle BLE → MQTT](https://github.com/oh2mp/esp32_ble2mqtt)
- [Theengs OpenMQTTGateway](https://docs.openmqttgateway.com/)

---

## Comment citer dans le rapport (exemples)

- Norme mesh : *Bluetooth SIG, « Bluetooth Mesh Networking Primer », bluetooth.com.*
- DTN : *S. Burleigh et al., « RFC 9171 – Bundle Protocol Version 7 », IETF, 2022.*
- Sécurité (contre-exemple) : *M. Albrecht, J. Blasco, R. B. Jensen, L. Mareková, « Mesh
  Messaging in Large-Scale Protests: Breaking Bridgefy », CT-RSA, 2021.*
- E2EE : *M. Marlinspike, T. Perrin, « The Double Ratchet Algorithm », Signal, 2016.*

> ⚠️ Pense à **revérifier chaque lien** au moment de la rédaction finale et à privilégier,
> pour les points sensibles (sécurité, normes), les **sources primaires** (RFC, spécifs
> officielles, articles académiques) plutôt que les articles de vulgarisation.
