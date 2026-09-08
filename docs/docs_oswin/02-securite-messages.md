# 02 — Sécuriser le message

> C'est le point le plus important — et le plus facile à rater. Un message qui traverse
> **des appareils inconnus** (les relais) doit rester **illisible** et **infalsifiable**
> par ces relais. La règle d'or : **les points de transmission transportent des octets
> chiffrés, ils ne doivent jamais pouvoir lire ni modifier le contenu.**

## 1. Les 4 propriétés de sécurité visées

| Propriété | Question à laquelle elle répond | Brique |
|---|---|---|
| **Confidentialité** | « Un relais peut-il lire le message ? » → non | chiffrement |
| **Intégrité** | « Le message a-t-il été modifié en route ? » → détectable | AEAD / hash / signature |
| **Authenticité** | « Vient-il vraiment de l'expéditeur annoncé ? » | signature |
| **Non-rejeu** | « Un vieux message peut-il être renvoyé pour tromper B ? » | numéro/horodatage + ID |

On y ajoute souvent **la vie privée / anonymat** (ne pas révéler qui parle à qui) et le
**secret persistant** (*forward secrecy* : compromettre une clé aujourd'hui ne doit pas
déchiffrer les messages d'hier).

## 2. Le chiffrement de bout en bout (E2EE)

Principe : **seuls A (émetteur) et B (destinataire) possèdent les clés**. Les relais et
l'Arduino ne voient qu'un blob chiffré + un en-tête minimal de routage.

Le schéma standard, éprouvé, se compose de trois primitives modernes :

1. **Échange de clés — X25519 (courbe Curve25519, Diffie-Hellman).**
   A et B combinent leur clé privée avec la clé publique de l'autre pour obtenir un
   **secret partagé** *sans jamais transmettre de secret sur le réseau*.
2. **Chiffrement authentifié — AES-256-GCM (ou ChaCha20-Poly1305).**
   C'est un **AEAD** : il chiffre **et** protège l'intégrité en une seule opération (si un
   bit est modifié, le déchiffrement échoue). ChaCha20-Poly1305 est souvent préféré sur les
   microcontrôleurs sans accélération AES.
3. **Signature — Ed25519.**
   Prouve **qui** a émis le message (authenticité + non-répudiation).

Ce sont **exactement** les primitives retenues par Bitchat : *X25519* pour l'échange,
*AES-256-GCM* pour les messages privés, *Ed25519* pour les signatures, le tout orchestré
par le **cadre Noise (motif XX)** pour établir une session mutuellement authentifiée et
chiffrée de bout en bout.

*Sources : [dev.to – Bitchat crypto](https://dev.to/grenishrai/offline-messaging-reinvented-with-bitchat-5011),
[Noise Protocol Framework](https://noiseprotocol.org/),
[Signal – Double Ratchet](https://signal.org/docs/specifications/doubleratchet/).*

### Aller plus loin (optionnel) : le protocole Signal

Pour une **conversation** (plusieurs messages), le **protocole Signal** ajoute le
**Double Ratchet** : une **nouvelle clé par message**, ce qui donne le *forward secrecy* et
la *post-compromise security*. C'est l'état de l'art (WhatsApp, Signal, OMEMO l'utilisent).
Pour un projet d'école, c'est **un bonus ambitieux** : commencer par un E2EE simple
(X25519 + AES-GCM + Ed25519) puis, si le temps le permet, mentionner/implémenter le
ratchet.

*Sources : [Signal Protocol – Wikipedia](https://en.wikipedia.org/wiki/Signal_Protocol),
[OMEMO](https://en.wikipedia.org/wiki/OMEMO).*

## 3. Le format de message chiffré (proposition)

```text
+--------------------------------------------------------------+
| EN-TÊTE EN CLAIR (lisible par les relais, pour router)        |
|   - id_message        (aléatoire, unique)                     |
|   - ttl               (nb de sauts restants)                  |
|   - expire_le         (horodatage d'expiration)               |
|   - dest_pubkey_hash  (à qui, sous forme pseudonyme)          |
|   - exp_pubkey_ephem  (clé publique éphémère de l'émetteur)   |
+--------------------------------------------------------------+
| CHARGE UTILE CHIFFRÉE (AES-256-GCM) — illisible par les relais|
|   - texte du message                                          |
|   - numéro de séquence (anti-rejeu)                           |
+--------------------------------------------------------------+
| SIGNATURE Ed25519 sur (en-tête + charge utile)               |
+--------------------------------------------------------------+
```

Points d'attention :
- l'en-tête **doit** être couvert par l'authentification (AEAD « additional data » ou
  signature) pour qu'un relais ne puisse pas trafiquer le TTL ou le destinataire ;
- le **numéro de séquence + l'ID unique** bloquent le **rejeu** ;
- garder l'en-tête **minimal** : chaque champ en clair est une **fuite de métadonnées**
  (qui parle à qui) — voir §5.

## 4. Étude de cas à NE PAS reproduire : Bridgefy (2020)

Bridgefy, une app de messagerie mesh populaire pendant les manifestations, a été
**cassée par des chercheurs** (Albrecht, Blasco, Jensen, Mareková). C'est le **contre-exemple
parfait** à citer dans le rapport pour justifier vos choix :

| Faille trouvée | Cause | Ce que dengon doit faire |
|---|---|---|
| **Aucune authentification** | pas de vérification d'identité → usurpation | signer avec **Ed25519**, vérifier les clés |
| **Man-in-the-middle** | identités contradictoires acceptées | *handshake* authentifié (Noise) |
| **Chiffrement cassé** | RSA **PKCS#1 v1.5** obsolète → *padding oracle* (Bleichenbacher) | **AEAD moderne** (AES-GCM / ChaCha20-Poly1305), jamais de crypto « maison » |
| **Rejeu / falsification** | ciphertexts réordonnables/rejouables | **numéro de séquence + AEAD** |
| **Désanonymisation** | identifiants en clair diffusés en continu | **identifiants éphémères** par session (comme Bitchat) |
| **Déni de service** | *bombe de compression* (gzip) | limiter la taille, se méfier de compression **avant** chiffrement |

**Les 3 leçons universelles :**
1. **Ne jamais inventer sa propre cryptographie** — utiliser des bibliothèques éprouvées
   (libsodium/NaCl, Noise, l'API Web Crypto…).
2. **Authentifier avant de chiffrer les identités**, sous peine de MITM.
3. **Attention à compression + chiffrement** : les combiner crée des oracles.

*Sources : [Breaking Bridgefy (version abrégée, PDF)](https://martinralbrecht.wordpress.com/wp-content/uploads/2020/08/bridgefy-abridged.pdf),
[eprint IACR 2021/214](https://eprint.iacr.org/2021/214.pdf),
[Royal Holloway – communiqué](https://www.royalholloway.ac.uk/research-and-education/subjects/information-security/news/using-messaging-service-bridgefy-could-have-dire-consequences-for-users-if-privacy-protection-issues-aren-t-fixed/).*

## 5. Modèle de menace (à mettre dans le rapport)

Qui est l'attaquant, et que veut-on empêcher ?

- **Un relais curieux** (un des points de transmission) : ne doit **pas lire** le contenu →
  **E2EE**. C'est le cœur du sujet.
- **Un relais malveillant** : ne doit **pas modifier** le message sans être détecté →
  **AEAD + signature** ; peut au pire **refuser de relayer** (le mesh contourne via d'autres
  chemins).
- **Un espion passif** qui écoute les ondes : ne doit **pas savoir qui parle à qui** →
  **identifiants éphémères**, en-tête minimal.
- **Un attaquant actif** qui rejoue/injecte : bloqué par **ID unique + n° de séquence +
  signature**.

Ce qu'un système mesh **ne peut pas** garantir facilement : cacher **l'existence** d'un
trafic (analyse de trafic), ni empêcher un adversaire de **brouiller** physiquement le
Bluetooth. À assumer honnêtement dans les limites du projet.

## 6. Recommandations concrètes

- **Bibliothèque** : `libsodium` (dispo en C/Arduino, JS, Python…) fournit X25519, Ed25519,
  et un AEAD (XChaCha20-Poly1305) — idéal, y compris côté ESP32.
- **MVP crypto** : X25519 (échange) → clé de session → AES-256-GCM ou ChaCha20-Poly1305
  (chiffrement) + Ed25519 (signature). Simple, correct, défendable.
- **Bonus** : Double Ratchet (Signal) pour le *forward secrecy*.
- **Gestion des clés** : chaque utilisateur a une paire de clés d'identité (Ed25519). Le
  vrai problème pratique est l'**échange initial des clés publiques** (en présentiel, QR
  code, ou TOFU « trust on first use ») — à discuter dans le rapport.

---

### À retenir
Chiffrement **de bout en bout** obligatoire : les relais transportent des octets illisibles.
Trio gagnant : **X25519 + AES/ChaCha AEAD + Ed25519**, orchestré par **Noise**. Ne **jamais**
coder sa propre crypto — l'échec de **Bridgefy** est là pour le rappeler.
