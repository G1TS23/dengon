# docs_oswin — Recherche & synthèses techniques (projet **dengon**)

> **dengon** (伝言, « message transmis ») — application de messagerie **hors-ligne** via
> Bluetooth, avec relais entre appareils, stockage temporaire tant que le destinataire
> n'est pas joignable, chiffrement du message, et une passerelle **Arduino** qui remonte
> des informations vers un **dashboard en ligne**.

Ce dossier regroupe la recherche documentaire et les synthèses qui doivent servir de base
technique au projet. Chaque fiche est autonome, sourcée, et se termine par des pistes
concrètes de mise en œuvre.

## Sommaire

| # | Fiche | Contenu |
|---|-------|---------|
| 00 | [Vue d'ensemble & architecture](./00-vue-ensemble-architecture.md) | Le problème, l'architecture cible, le vocabulaire, les briques à construire |
| 01 | [Bluetooth, BLE & mesh](./01-bluetooth-ble-mesh.md) | Classic vs BLE, GATT, Bluetooth Mesh, *managed flooding*, portée, TTL |
| 02 | [Sécuriser le message](./02-securite-messages.md) | Chiffrement de bout en bout, primitives crypto, modèle de menace, leçons Bridgefy |
| 03 | [Stockage & relais (store-and-forward / DTN)](./03-store-and-forward-dtn.md) | Store-carry-forward, DTN, Bundle Protocol, routage épidémique, expiration |
| 04 | [Faut-il une blockchain ? (analyse critique)](./04-blockchain-analyse-critique.md) | Ce que la blockchain apporte / coûte, hash chains & arbres de Merkle, alternatives |
| 05 | [Passerelle Arduino & dashboard](./05-arduino-passerelle-dashboard.md) | HC-05 vs ESP32, pont BLE→MQTT, ThingsBoard / Node-RED |
| 06 | [Sources & références](./06-sources-references.md) | Bibliographie complète, classée par thème |
| 07 | [Matériel ESP32-WROOM-32E & application Android (APK)](./07-esp32-et-application-android.md) | Specs de la carte, techno de l'app (Flutter), obtention de l'APK, feuille de route en 3 paliers |
| 08 | [Feuille de route & plan de développement](./08-feuille-de-route-plan-dev.md) | MVP, chantiers, phases/jalons, planning, répartition des tâches, risques, plan de démo |
| 09 | [Benchmark des technologies & choix conseillés](./09-benchmark-technos.md) | Comparatif couche par couche (app, BLE, firmware, crypto, MQTT, dashboard) + stack recommandée |

## Décisions de l'équipe (mises à jour)

- **Matériel** : carte **ESP32-WROOM-32E** (Freenove). Détails et rôle : fiche 07.
- **Architecture** : mesh **téléphone-à-téléphone**, les **ESP32 servant de relais
  supplémentaires ET de passerelle unique vers le dashboard** (les téléphones ne
  contactent jamais le dashboard directement). Voir fiche 07 §1.
- **Livrable applicatif** : une **application Android (.apk)**. Techno conseillée pour
  débuter : **Flutter**. Voir fiche 07 §6.

## Le point clé à retenir dès maintenant

Ce que le projet décrit (« un message qui saute d'appareil en appareil, est stocké en
route, puis délivré ») porte un nom précis dans la littérature : c'est du
**store-carry-forward** sur un **réseau tolérant aux délais (DTN)**, exactement comme le
fait l'app **Bitchat** de Jack Dorsey (2025) sur du **Bluetooth LE mesh**. C'est le modèle
de référence à étudier en priorité — voir fiches 01 et 03.

L'idée d'une « blockchain » est **séduisante mais probablement surdimensionnée** pour ce
besoin. En revanche, deux briques *issues* de la blockchain sont réellement utiles :
le **chaînage par hash** (intégrité + ordre) et les **arbres de Merkle** (preuve de
réception compacte). La fiche 04 fait le tri honnêtement.

---

*Rédigé le 8 septembre 2026. Sources détaillées dans chaque fiche et dans la fiche 06.*
