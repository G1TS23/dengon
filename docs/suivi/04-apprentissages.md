# Apprentissages

Volet « apprentissage » du suivi. On y note les **notions qu'il a fallu comprendre**
pour coder le projet : algorithmes, protocoles, API, pièges, subtilités de langage.

But : pouvoir **réexpliquer** ces notions à l'oral, et éviter de réapprendre deux
fois la même chose.

Format libre mais court. Une note = un concept. Toujours répondre à : *c'est quoi ?*,
*pourquoi on en a besoin ici ?*, *qu'est-ce qui nous a surpris ?*.

---

## Modèle

### [Titre du concept]

**C'est quoi :** définition en 2-3 phrases, avec ses mots.
**Pourquoi dans dengon :** à quoi ça sert concrètement dans notre code.
**Piège / surprise :** ce qui n'était pas évident, l'erreur qu'on a faite.
**Où c'est utilisé :** `chemin:ligne`.
**Pour aller plus loin :** lien(s).

---

## Notes

*(à remplir au fil du développement)*

Sujets probables (d'après la conception) — à traiter quand on les rencontre :

- Routage épidémique / gossip / store-carry-forward (DTN).
- TTL, déduplication, jitter de relais : pourquoi chacun est nécessaire.
- Noise Protocol Framework : motifs `XX` vs `X`, ce que « forward secrecy » veut dire.
- Ed25519 vs X25519 : signature vs accord de clés.
- Hash-chain (journal chaîné) : en quoi ça rend une trace « infalsifiable », et ses
  limites (n'empêche pas d'omettre avant de signer).
- Golomb-Coded Set / filtre de Bloom : résumer un ensemble de façon compacte.
- BLE GATT : rôle central vs périphérique, MTU, notify vs write, pourquoi un nœud
  doit être les deux.
- ESP-IDF / FreeRTOS : tâches, files, coexistence BLE + Wi-Fi.
- UniFFI : comment un cœur Rust est appelé depuis Kotlin.
- TimescaleDB : hypertable, rétention, agrégats continus.
- MQTT : QoS, topics, mTLS.
