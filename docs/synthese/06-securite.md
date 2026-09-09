# dengon — Sécurité

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
>
> **`docs/powl/04-security.md` est la doc sécurité de référence** (décision
> C-11). Il ne reste qu'un *delta* à rédiger (résultat du Spike A, mapping des
> colonnes chiffrées, réconciliations `powl` D-1/D-2/D-4/D-5). Ce fichier
> résume la conception retenue.

---

## 1. Modèle de menace

**Ce qu'on protège** : contenu des messages (confidentialité + intégrité +
authenticité) ; qui parle à qui (minimisation — relais et VPS ne doivent pas
savoir) ; statuts/accusés (intégrité + authenticité, pas de faux « distribué ») ;
journal d'activité d'un nœud (infalsifiabilité *a posteriori*) ; identité d'un
contact (non-usurpable, vérifiable hors bande).

| Attaquant | Capacités supposées | Traité par |
| --- | --- | --- |
| **Relais malveillant** (ESP32 compromis / faux relais) | voit tout le trafic BLE qui passe, peut jeter/dupliquer/réordonner, peut mentir au VPS | E2E Noise, padding, tags tournants, journal chaîné signé (le VPS recoupe) |
| **VPS compromis / admin curieux** | voit tous les logs, la BDD, peut les modifier | aucune clé privée ni clair ne transite ; `msgID` haché ; signatures vérifiables côté client |
| **Voisin BLE passif** | sniffe l'air | chiffrement + padding ; `peerID` stable mais pseudonyme |
| **Attaquant actif MITM au premier contact** | intercepte l'échange de clés | vérification **QR + code 60 chiffres** hors bande ; TOFU sinon |
| **Vol de l'appareil déverrouillé** | accès à la base locale | chiffrement au repos (**XChaCha20 champ par champ** + Keystore/Keychain, §5) ; *panic wipe* (post-MVP) |
| **Attaquant réseau global** | corrèle les métadonnées à grande échelle | **hors périmètre MVP** (documenté comme limite) |

**Hors périmètre (assumé)** : adversaire étatique faisant de l'analyse de trafic
mondiale ; déni de service radio (brouillage BLE) ; forward secrecy des
enveloppes scellées ; sécurité physique de l'ESP32 (extraction de sa clé de
relais → au pire, faux logs, jamais de déchiffrement).

**Les 4 propriétés visées** : confidentialité (chiffrement) ; intégrité (AEAD /
hash / signature) ; authenticité (signature) ; non-rejeu (`conv_seq` +
horodatage + `msgID`). Complétées par la **vie privée / anonymat** (tags
tournants, padding) ; la **forward secrecy** est assurée sur la session live
(Noise `XX`), pas sur les enveloppes (borné par `MSG_TTL_S`).

## 2. Identité & clés

Chaque installation génère au premier lancement et stocke dans le coffre de la
plateforme (Android Keystore / Secret Service / NVS chiffrée ESP32) :

| Clé | Algo | Usage |
| --- | --- | --- |
| `static` | X25519 (Curve25519) | accord de clés Noise (`XX` et `X`) |
| `sign` | Ed25519 | signature des paquets `ANNOUNCE`, `INVENTORY`, `ENVELOPE_*`, `LOG_ATTEST` |

```text
peerID = SHA-256(pub_static)[0..8]            // 8 octets (A-8), affiché en base32
fingerprint = SHA-256(pub_static ‖ pub_sign)  // 32 octets, base de la vérification
```

`peerID` **stable** entre redémarrages et réinstallations tant que le coffre
survit ; ne change qu'après régénération explicite (« nouvelle identité »).

**QR code** : `dengon:v1:<base64url( pseudo_len(1) ‖ pseudo ‖ pub_static(32) ‖
pub_sign(32) )>`. Pas de secret.

**Code de vérification (« safety number », 60 chiffres, C-12)** — détecter un
MITM au premier contact sans serveur. Version réconciliée (fondée sur les
**empreintes**, pas les clés brutes — D-1) :

```text
material = SHA-512( min(fpA, fpB) ‖ max(fpA, fpB) )   // ordre-indépendant
code = 60 chiffres décimaux :
       pour i in 0..12 : groupe_i = ( u16_be(material[i*2 .. i*2+2]) % 100000 )
       affiché : "01234 56789 01234 ..." (12 groupes de 5)
```

Les deux appareils affichent **le même code** ; l'utilisateur compare de visu ou
lit à voix haute. Match → contact marqué **✔ vérifié** (`contacts.verified_at`).
Non-vérifié → **TOFU** (confiance à la première clé vue pour ce `peerID`, alerte
si elle change : `contacts.key_changed`).

**Changement de clé** : si un `ANNOUNCE` présente un `pub_static` différent pour
un `peerID`/pseudo connu → messages livrés **suspendus** vers ce contact ;
bannière UI « la clé de X a changé — re-vérifiez » ; événement
`contact.key_changed` journalisé (chaîne locale).

## 3. Chiffrement des messages

Cadre retenu : **Noise `XX`** (session live) + **Noise `X`** (enveloppes
scellées) + **Ed25519** (signature de paquet), implémenté **une seule fois en
Rust** dans `dengon-core::crypto` (A-3). Repli si le **Spike A** montre que
`snow` ne cross-compile pas pour xtensa : isoler le handshake Noise `XX` de lien
BLE derrière un `trait Crypto` implémenté en C avec mbedTLS **côté firmware
seulement** ; `sha2` + `ed25519-dalek` restent en Rust partout (B-1).

**Session en direct — Noise `XX`** (`Noise_XX_25519_ChaChaPoly_SHA256`) :
handshake 3 messages (`-> e` / `<- e, ee, s, es` / `-> s, se`) → 2 clés de
transport (A→B, B→A). Authentification mutuelle ; **forward secrecy** (clés
éphémères `e`) ; la `pub_static` reçue dans le handshake **doit** correspondre à
celle du contact (vérifié ou TOFU), sinon rejet + alerte ; session réutilisée
tant que le lien BLE tient, re-négociée à la reconnexion ou après `2^n`
messages. **ACK voyage dans la session** (chiffré, authentifié).

**Message pour destinataire absent — Noise `X` (enveloppe scellée)**
(`Noise_X_25519_ChaChaPoly_SHA256`, one-shot vers la clé statique publique du
destinataire) :

```text
SEALED_ENVELOPE.payload = recipient_tag(16) ‖ epoch_day(2) ‖ noise_x_message
noise_x_message chiffre : AppFrame{kind=1, Message{...}} ‖ sender_pub_static(32) ‖ sig
```

Le destinataire déchiffre avec sa clé `static` **hors ligne, plus tard**. Le
paquet `SEALED_ENVELOPE` est **signé Ed25519** par l'expéditeur (le relais
vérifie la signature avant de stocker → anti-pollution). `sender_pub_static` est
**à l'intérieur** du chiffré → un relais ne sait pas qui envoie.

**Adressage anonyme — `recipient_tag`** (formule réconciliée `04 §3.3` — D-2) :

```text
recipient_tag(day) = HMAC-SHA256( pub_static_destinataire , "dengon-tag" ‖ day_u32 )[0..16]
```

Change chaque jour → un relais ne peut pas suivre un destinataire dans le temps.
Le destinataire calcule ses propres tags (J-1, J, J+1) et matche les
`ENVELOPE_OFFER`. **Limite assumée** : pas de forward secrecy sur les
enveloppes ; si la clé `static` du destinataire est volée **avant** qu'il ait
récupéré une enveloppe encore en circulation, cette enveloppe est déchiffrable.
Mitigation : `MSG_TTL_S` = 24 h borne la fenêtre. Montée en gamme possible
(pré-clés type X3DH) en v2.

**Padding** : tout paquet `NOISE_MSG` / `NOISE_HS` / `SEALED_ENVELOPE` est
complété (PKCS#7, flag `PADDED`) à la borne supérieure de
`PAD_BUCKETS = [256, 512, 1024, 2048]`. Un message court et un message de 200
caractères sont indistinguables par la taille.

**Montée en gamme v2** : le **Double Ratchet** de Signal (nouvelle clé par
message → forward secrecy + post-compromise security) — bonus, à implémenter si
le temps le permet.

## 4. Journal chaîné signé — la couche « type blockchain »

**But** : donner au dashboard (et à un auditeur) une **trace infalsifiable** de
ce que chaque nœud a fait, **sans** que le dashboard puisse la forger et
**sans** consensus (A-7 / A-15). C'est un *hash-chained append-only log*, pas une
blockchain répliquée.

```text
Entry {
    seq:        u64          // 0, 1, 2, … monotone, sans trou
    ts_ms:      u64
    event:      string       // nom canonique (cf. catalogue d'événements, fichier 09)
    payload:    object       // champs de l'événement, msgID déjà haché
    prev_hash:  bytes32      // hash de l'entrée seq-1  (00…0 pour seq 0)
}
entry_hash = SHA-256( canonical_json(Entry) )
Record {
    entry:  Entry
    hash:   entry_hash
    sig:    Ed25519( sign_key , entry_hash )
}
```

`canonical_json` : clés triées, pas d'espaces, UTF-8, entiers sans zéro superflu.
Le journal est **local** à chaque nœud (client et relais). `ledger_root` =
`entry_hash` de la dernière entrée = résumé de tout l'historique.

**Attestation (`LOG_ATTEST`)** : périodiquement, chaque nœud **diffuse** en BLE un
paquet signé `{ ledger_root, height, node_pub_sign }`. Les relais le captent et
le remontent au VPS → le VPS voit la « hauteur » revendiquée par chaque nœud,
**corroborée par des tiers** (plusieurs relais rapportent la même racine).

**Vérifications côté dashboard** (par le binaire Rust `dengon-verify`, qui
réutilise `ledger::verify_chain`) :

| Contrôle | Détecte |
| --- | --- |
| `prev_hash(seq) == hash(seq-1)` sur toute la séquence reçue | **altération** d'une entrée passée |
| `seq` strictement croissante sans trou | **suppression** d'entrées |
| `sig` valide avec `node_pub_sign` | entrée **forgée** par un tiers (ou le VPS) |
| `ledger_root` reçu ≥ précédent, cohérent avec les `LOG_ATTEST` de plusieurs relais | **fork** (le nœud sert deux historiques différents) |
| recoupement : même `msg_log_id` rapporté par R1 et R2 avec des `prev_hop` cohérents | relais qui **ment** sur un relais |

Une rupture → alerte `integrity.chain_broken` / `integrity.fork_detected`, nœud
marqué « suspect ». **Le dashboard ne peut pas réparer ni réécrire** un journal :
il constate.

**Ce que le journal ne fait pas** : pas d'ordre total entre nœuds (chaque journal
indépendant) ; pas de preuve que le nœud a **tout** journalisé (il peut omettre
*avant* de signer `seq n`) — d'où le recoupement multi-relais comme garde-fou ;
pas de valeur / pas de double-dépense → pas de consensus nécessaire.

## 5. Chiffrement de la base locale

**XChaCha20-Poly1305 champ par champ** sur les colonnes sensibles (contenu des
messages ; les clés privées de préférence dans Keystore/Keychain) — décision
B-3. Motifs : le crate `chacha20poly1305` est **déjà** dans l'arbre de
dépendances (via Noise, §3) et fournit `XChaCha20Poly1305` ; **pas de
dépendance native supplémentaire** dans le build (SQLCipher ajouterait une lib C
à cross-compiler pour chaque ABI Android et pour xtensa) ; **même primitive
réutilisable pour la NVS de l'ESP32**. Les métadonnées locales (horodatage,
`peerID`, statut) restent en clair dans la base — le modèle de menace local est
le vol d'appareil, ciblé sur le contenu et les clés.

## 6. Sécurité des relais ESP32

| Aspect | Choix |
| --- | --- |
| Clé de relais | paire Ed25519 propre, générée à la prod, stockée en **NVS chiffrée** (eFuse flash encryption activé) |
| Enregistrement | `pub_sign` du relais déclarée au dashboard (liste blanche) ; jeton/JWT court pour l'endpoint HTTPS |
| Compromission physique | **aucune clé utilisateur** sur le relais → pas de déchiffrement possible. Au pire : faux logs (détectés par recoupement), rétention/drop de paquets (le réseau route autour) |
| Mise à jour firmware | OTA signé (vérifié par le bootloader) — post-MVP |
| Secure Boot | activé sur les cartes de prod |

## 7. Sécurité du dashboard

| Aspect | Choix |
| --- | --- |
| Données au repos | aucun clair de message ; `msgID` haché ; pseudos affichés seulement si le nœud les publie (opt-in) |
| Transport | HTTPS (TLS) partout (reverse-proxy Caddy/nginx + Let's Encrypt) |
| Auth des nœuds | **jeton/JWT court par nœud** (liste blanche côté serveur) + **signature Ed25519 des batchs** d'événements (B-2 / C-5) |
| Auth opérateurs | login + argon2id + TOTP ; rôles `viewer` / `admin` |
| Injection | le dashboard **n'a aucun chemin d'écriture vers le terrain** ; API terrain = *ingest only* |
| Rétention | base **effacée après chaque session de démo** (B-4) — pas de politique de rétention automatique en v1 |
| Audit interne | actions opérateurs journalisées |

## 8. Cryptographie — choix de primitives

| Fonction | Primitive | Crate Rust |
| --- | --- | --- |
| Signature | Ed25519 | `ed25519-dalek` v2 |
| Accord de clés | X25519 | `x25519-dalek` |
| Framework de session | Noise `XX` / `X` | `snow` |
| AEAD | ChaCha20-Poly1305 | `chacha20poly1305` (via `snow`) |
| Chiffrement base locale | XChaCha20-Poly1305 (champ par champ) | `chacha20poly1305` |
| Hash | SHA-256 / SHA-512 | `sha2` |
| MAC (tags) | HMAC-SHA256 | `hmac` |
| KDF (au besoin) | HKDF-SHA256 | `hkdf` |
| Aléa | OS CSPRNG | `getrandom` / `rand_core::OsRng` |

Règles : **pas de crypto maison** ; versions épinglées ; `cargo audit` +
`cargo deny` en CI ; revue par un tiers avant le premier déploiement « produit ».

> `oswin/02 §6` recommandait `libsodium` (X25519, Ed25519, XChaCha20-Poly1305) et
> `olivier` la piste `crypto_box`. Le projet retient **Noise + les crates Rust
> ci-dessus** (une seule implémentation, cadre de session standard) ; la
> convergence de fond (X25519 + AEAD moderne + Ed25519, jamais de crypto maison)
> est respectée. Arguments détaillés : fiche A-3 de
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).

## 9. Résumé « qui voit quoi »

| Donnée | Expéditeur | Destinataire | Relais BLE | VPS / Dashboard |
| --- | --- | --- | --- | --- |
| Texte du message | ✅ | ✅ | ❌ | ❌ |
| `msg_uuid` | ✅ | ✅ | ❌ (chiffré) | ❌ |
| `msgID` (L3) | ✅ | ✅ | ✅ | ✅ **haché** |
| Qui → qui | ✅ | ✅ | `peerID` src visible ; dest = tag anonyme | ❌ (juste des `peerID` de relais + hachés) |
| Taille réelle | ✅ | ✅ | ❌ (padding) | ❌ |
| Statut (parti / distribué) | ✅ | ✅ | métadonnée de passage | ✅ (reconstruit, best-effort) |
| Journal d'activité nœud | ✅ (le sien) | — | capte les `LOG_ATTEST` | ✅ (vérifie, ne forge pas) |
