# dengon — Protocole réseau & format binaire de trame

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
>
> **`docs/powl/03-network-protocol.md` est la spec de trame et de routage
> retenue** (décision A-12). Le format octet par octet du brouillon
> `olivier/format-trame.md` est **remplacé** ; il est conservé pour mémoire /
> vecteurs de test en [`11-glossaire-biblio-annexes.md`](11-glossaire-biblio-annexes.md) (annexe A).

---

## 1. Couches

```text
┌─────────────────────────────────────────────┐
│ L4  Application : Message, Ack, ReadReceipt  │  ← contenu chiffré (Noise)
├─────────────────────────────────────────────┤
│ L3  Enveloppe dengon : Packet (signé)       │  ← routage, TTL, dedup, inventaire
├─────────────────────────────────────────────┤
│ L2  Fragmentation protocole (paquet > MTU)   │
├─────────────────────────────────────────────┤
│ L1  BLE GATT (RX/TX characteristics)         │
└─────────────────────────────────────────────┘
```

## 2. Constantes (`protocol::consts`)

| Nom | Valeur | Sens |
| --- | --- | --- |
| `PROTO_VERSION` | `1` | version courante |
| `SERVICE_UUID` | `6d656e67-2d64-656e-676f-6e2d76310000` | service GATT `dengon` (C-2) |
| `CHAR_RX_UUID` | `…0001` | write-without-response, pair → nœud |
| `CHAR_TX_UUID` | `…0002` | notify, nœud → pair |
| `TTL_DEFAULT` | `7` | sauts au départ |
| `TTL_CLAMP_DENSE` | `5` | si ≥ `DENSE_LINKS` voisins |
| `DENSE_LINKS` | `6` | seuil de densité |
| `THIN_LINKS` | `2` | seuil « chaîne fine » (relais à profondeur pleine) |
| `RELAY_JITTER_MS` | `10..=220` | délai aléatoire avant relais |
| `SEEN_SET_CAP` | `1024` | entrées LRU |
| `SEEN_TTL_S` | `300` | expiration d'une entrée |
| `FRAG_SIZE` | `440` | octets de payload par fragment |
| `FRAG_TIMEOUT_S` | `30` | abandon du réassemblage |
| `FRAG_MAX_CONCURRENT` | `64` | assemblages simultanés |
| `MSG_TTL_S` | `86_400` | durée de vie applicative d'un message (24 h) |
| `ENVELOPE_MAX_BYTES` | `4096` | taille max d'une enveloppe scellée (texte court) |
| `FLOOD_MAX_PER_MIN_PEER` | `20` | anti-inondation : nouveaux `msgID`/min/voisin (A-13) |
| `COPY_BUDGET_INIT` / `COPY_BUDGET_MAX` | `4` / `8` | budget de copies d'une enveloppe (**cible v2**, A-13) |
| `PAD_BUCKETS` | `[256, 512, 1024, 2048]` | tailles cibles des paquets chiffrés |
| `ANNOUNCE_ISOLATED_S` | `4` | période d'ANNOUNCE si seul |
| `ANNOUNCE_CONNECTED_S` | `15..=30` | période d'ANNOUNCE si connecté (jitter) |

**Caps mémoire ESP32-WROOM (A-4 / C-1)** : la carte n'a pas de PSRAM → les
caches sont divisés par ~8 par rapport aux valeurs « WROVER » de `powl/06` :
`ENVELOPE_STORE_MAX = 64` (au lieu de 512), cache de réconciliation réduit en
proportion. Détail dans [`08-relais-esp32.md`](08-relais-esp32.md).

## 3. Format du paquet L3 (tous entiers **big-endian**)

C'est aussi le **format binaire de trame** (il n'y a qu'une spec).

```text
 offset  taille  champ
 ------  ------  --------------------------------------------------
   0       1     version            (= PROTO_VERSION)
   1       1     type               (cf. §4)
   2       1     ttl                 (décrémenté à chaque relais)
   3       1     flags              (bitfield, cf. §3.1)
   4       8     timestamp_ms        (UTC ms, horloge de l'émetteur)
  12       8     sender_id           (peerID = SHA-256(pub_static)[0..8])   ← 8 octets (A-8)
  20      [8]    recipient_id        (présent SSI flags.ADDRESSED)
  20|28    2     payload_len         (N)
  22|30    N     payload             (cf. §4 selon type)
  +N      [64]   signature           (présent SSI flags.SIGNED ; Ed25519 sur
                                      octets [0 .. début_signature])
```

En-tête : **22 o** (broadcast) ou **30 o** (adressé). Avec signature : **+64 o**.
Endianness big-endian (réseau). Champ « réservé » = 0 à l'émission, ignoré à la
réception. **Pas de somme de contrôle par fragment** : on s'appuie sur le CRC
BLE de la couche liaison + la signature du paquet reconstruit (C-6).

### 3.1 `flags` (bitfield)

| bit | nom | sens |
| --- | --- | --- |
| 0 | `ADDRESSED` | `recipient_id` présent ; paquet à destination d'un `peerID` précis |
| 1 | `SIGNED` | signature Ed25519 en fin de paquet |
| 2 | `FRAGMENT` | le payload est un fragment (cf. §5) |
| 3 | `RELAY_OK` | l'émetteur autorise le relais (0 = strictement local, 1 saut) |
| 4 | `PADDED` | payload complété par du PKCS#7 vers un `PAD_BUCKET` |
| 5-7 | réservé | 0 |

### 3.2 Identifiants

```text
msgID = SHA-256( sender_id ‖ timestamp_ms ‖ type ‖ payload )   // 32 o (A-9 / D-3)
```

`msgID` sert de clé de **déduplication** (via le seen-set) ; de **référence de
suivi** (l'app l'affiche ; le dashboard le trace **haché** :
`msg_log_id = SHA-256(msgID)[0..16]`) ; rend le paquet **idempotent**.

`msg_uuid` (16 o, UUIDv4, à l'intérieur du chiffré, voir §4.1) est
l'identifiant du **message applicatif**, **stable de bout en bout** — c'est lui
que suit la machine à états des statuts (le `msgID` L3 change si le paquet est
re-scellé). Les deux identifiants sont nécessaires parce que les enveloppes
scellées Noise `X` peuvent être re-scellées (A-9).

## 4. Types de paquets

| `type` | nom | `ADDRESSED` | `SIGNED` | payload | MVP ? |
| --- | --- | --- | --- | --- | --- |
| `0x01` | `ANNOUNCE` | non | oui | `peerID ‖ pub_static(32) ‖ pub_sign(32) ‖ pseudo_len(1) ‖ pseudo ‖ ledger_height(8) ‖ caps(1)` | ✅ |
| `0x02` | `NOISE_HS` | oui | non | message de handshake Noise `XX` (1 des 3) | ✅ |
| `0x03` | `NOISE_MSG` | oui | non | ciphertext Noise (transport) ; en clair : un `AppFrame` (§4.1) | ✅ |
| `0x04` | `SEALED_ENVELOPE` | non | oui | `recipient_tag(16) ‖ epoch_day(2) ‖ noise_x_ciphertext` ; en clair : `AppFrame` | ✅ |
| `0x05` | `ACK` | oui | non | (dans session Noise) `AckFrame` (§4.1) | ✅ |
| `0x06` | `GOSSIP_FILTER` | oui | oui | `kind(1) ‖ golomb_coded_set` — résumé compact des `msgID` connus | v2 |
| `0x07` | `GOSSIP_PULL` | oui | oui | `count(2) ‖ msgID[count]` — demande explicite de paquets | v2 |
| `0x08` | `GOSSIP_PUSH` | oui | non | `count(2) ‖ (len(2) ‖ packet)[count]` — paquets bruts ré-encapsulés | v2 |
| `0x09` | `FRAGMENT` | hérite | non | cf. §5 | ✅ |
| `0x0A` | `LOG_ATTEST` | non | oui | `ledger_root(32) ‖ height(8) ‖ node_pub_sign(32)` — attestation de journal | ✅ |
| `0x0B` | `ENVELOPE_OFFER` | oui | oui | `count(2) ‖ recipient_tag(16)[count]` — « je porte ces enveloppes » | ✅ |
| `0x0C` | `ENVELOPE_REQUEST` | oui | oui | `count(2) ‖ recipient_tag(16)[count]` — « donne-les moi » | ✅ |
| `0x0D`* | `INVENTORY` | oui | oui | `count(2) ‖ msgID[count]` — liste des `msgID` détenus, échangée entre voisins | ✅ (remplace `GOSSIP_*` au MVP, A-13) |

\* Type `INVENTORY` ajouté au MVP (l'`INVENTAIRE` du brouillon `olivier`) ;
numéro exact à figer dans la doc protocole. `GOSSIP_*` (`0x06`–`0x08`) restent
réservés pour la cible v2.

Au **MVP**, la réconciliation d'état se fait par **échange d'inventaire brut**
(`INVENTORY` : chacun envoie la liste de ses `msgID`, l'autre pousse ce qui
manque). Le **gossip GCS** (`GOSSIP_FILTER/PULL/PUSH`, ~20-30 % plus compact) est
une optimisation **v2** qui s'ajoute sans changer le format des autres paquets.

### 4.1 Frames applicatives (L4, à l'intérieur de Noise)

```text
AppFrame  = kind(1) ‖ body
  kind 1 : Message   -> msg_uuid(16) ‖ conv_seq(8) ‖ sent_ms(8) ‖ text_utf8
  kind 2 : Ack       -> AckFrame
  kind 3 : ReadRcpt  -> msg_uuid(16) ‖ read_ms(8)      (réservé — v2, statut « Lu »)
  kind 4 : Profile   -> pseudo, avatar_hash…           (post-MVP)

AckFrame  = msg_uuid(16) ‖ status(1) ‖ at_ms(8)
  status : 2 = distribué (reçu par l'appareil destinataire)
           (status 3 = lu, réservé v2)
```

- `msg_uuid` : UUIDv4 tiré par l'émetteur, **stable de bout en bout**.
- `conv_seq` : compteur par conversation → détection de trous, ordre causal,
  **anti-rejeu** (C-7 : pas de compteur global par expéditeur en clair dans
  l'en-tête).

## 5. Fragmentation protocole (L2)

Nécessaire quand un paquet L3 dépasse le MTU ATT négocié (souvent 185–244 o
utiles, jusqu'à 517 si négocié). Le **texte court** tient généralement en 1
paquet ; la fragmentation sert surtout aux enveloppes et à `INVENTORY` quand la
file est grande.

```text
Fragment payload (dans un paquet type=0x09, flags.FRAGMENT) :
  frag_id(8)     = SHA-256(paquet_complet)[0..8]
  index(2)
  total(2)
  chunk(<= FRAG_SIZE)
```

Réassemblage : buffer par `frag_id`, complété quand les `total` chunks sont là.
`FRAG_TIMEOUT_S` (~30 s) d'inactivité → abandon. `FRAG_MAX_CONCURRENT` dépassé →
on jette le plus ancien. Un fragment **n'est pas signé** individuellement ; la
signature est dans le paquet reconstruit. **Au MVP, un nœud réassemble le
paquet complet avant de le relayer** (plus simple pour vérifier destinataire,
signature et dédup — A-13). La charge utile réelle par écriture BLE dépend du
MTU négocié sur les appareils cibles — **à mesurer au Spike C** (C-1).

## 6. Couche BLE (L1) & routage

**Rôle double** : chaque nœud, en permanence, **Peripheral** (publie
`SERVICE_UUID`, expose `CHAR_RX` write-w/o-response + `CHAR_TX` notify ; annonce
un *manufacturer data* court = `peerID[0..4] ‖ flags`) **ET Central** (scanne le
`SERVICE_UUID`, se connecte aux nouveaux `peerID`, s'abonne à leur `CHAR_TX`).
**Règle anti-boucle de connexion** : quand A et B se découvrent, **celui dont le
`peerID` est le plus petit** initie la connexion GATT.

**MTU** : négocier `ATT_MTU = 517` à la connexion ; retomber sur 23 (→ 20 o
utiles) si refus.

### 6.1 Pipeline à la réception d'un paquet

```text
paquet reçu
  → version OK ? + signature OK (si SIGNED) ? — non → jeter + log pkt.rejected
  → msgID dans seen-set ? — oui → jeter (doublon)
  → seen-set.insert(msgID)
  → ce voisin a-t-il dépassé FLOOD_MAX_PER_MIN_PEER nouveaux msgID sur 60 s ?
       — oui → ignorer ses paquets jusqu'à ce que son débit retombe (anti-inondation)
  → ADDRESSED && recipient_id == moi ? — oui → traiter localement
       (handshake / message / ack / inventaire)  [DELIVER → aussi STORE]
  → sinon : RELAY_OK && ttl > 1 ?
       — non → si SEALED_ENVELOPE : stocker (dépôt) ; sinon : fin
       — oui → ttl' = min(ttl-1, clamp(densité))
            → attendre RELAY_JITTER_MS (« écouter avant de rediffuser »)
            → reçu en double pendant l'attente ? — oui → abandonner le relais
            — non → broadcast(paquet, ttl') + log pkt.relayed
```

- **Directed traffic** (`NOISE_HS`, `NOISE_MSG`, `ACK`, `SEALED_ENVELOPE`,
  `INVENTORY`) : relais déterministe `ttl-1`, jitter serré, rediffusion à tous
  les pairs **sauf la source**.
- **Broadcast** (`ANNOUNCE`, `LOG_ATTEST`) : idem mais TTL faible (2–3).
- **Clamp de densité** : `ttl' = min(ttl-1, TTL_CLAMP_DENSE=5)` si ≥ 6 voisins.
- **Conditions d'arrêt** : TTL épuisé ; déjà vu ; expiration (> `MSG_TTL_S`) ;
  **un ACCUSÉ pour ce message est passé par le nœud** (le message sort de la
  file de retransmission).

### 6.2 Réconciliation d'inventaire (à chaque nouveau pair) — MVP

Après ANNOUNCE + handshake, les deux nœuds **échangent la liste des `msgID`
qu'ils détiennent** (paquet `INVENTORY`), puis chacun **pousse à l'autre ce qui
lui manque** (paquets bruts, qui repassent par le pipeline §6.1 : re-dédup,
re-vérif de signature, éventuel re-relais). Repli si la file est trop grande :
pousser toute la file. **Cible v2** : remplacer l'inventaire brut par un filtre
**GCS** (`GOSSIP_FILTER`, Golomb-Coded Set, `p = 1/64`).

### 6.3 Collecte d'enveloppes scellées (à chaque rencontre)

1. Le porteur envoie `ENVELOPE_OFFER` (liste de `recipient_tag`).
2. Le pair calcule **ses** tags du jour (± 1 jour de fenêtre) et compare.
3. Match → `ENVELOPE_REQUEST` → le porteur renvoie le `SEALED_ENVELOPE`.
4. Le destinataire déchiffre (Noise `X`), traite, émet un `ACK`.
5. **Budget de copies (cible v2)** : quand deux porteurs d'une même enveloppe se
   rencontrent, ils se partagent la moitié du budget restant (Spray-and-Wait).
   Au MVP : flood des enveloppes borné par `MSG_TTL_S` + dédup. Budget épuisé +
   `MSG_TTL_S` dépassé → suppression.

### 6.4 Anti-abus / robustesse

| Menace | Contre-mesure |
| --- | --- |
| Rejeu de paquets | `msgID` + seen-set ; `timestamp_ms` hors fenêtre ±2 h → rejeté ; `conv_seq` |
| Boucle de flood | seen-set + TTL + jitter + abandon si doublon pendant l'attente |
| Paquet forgé (usurpation d'émetteur) | `ANNOUNCE`/`INVENTORY`/`ENVELOPE`/`LOG_ATTEST` signés ; `peerID` doit matcher `SHA-256(pub_static)` |
| Flood volontaire (DoS) | quota par `peerID` et par lien (paquets/s) + anti-inondation `FLOOD_MAX_PER_MIN_PEER` ; RSSI-gating (optionnel MVP) ; au plus `TTL_DEFAULT` copies relayées |
| Épuisement mémoire (fragments / enveloppes) | caps stricts (`FRAG_MAX_CONCURRENT`, `ENVELOPE_STORE_MAX`), éviction LRU / plus ancien |
| Analyse de trafic par un relais | padding `PAD_BUCKETS` ; `recipient_tag` tournant ; pas de `recipient_id` sur les enveloppes |
| Horloge fausse d'un nœud | fenêtre de tolérance ; le dashboard signale les dérives (`node.clock_skew`) |

### 6.5 Cycle de vie d'un message stocké (dans un relais ou un porteur)

```text
                 message reçu par un nœud
                          │
                 [ FILE DE STOCKAGE LOCALE ]  (clé = msgID)
                          │
          ┌───────────────┼─────────────────────────────┐
   nouveau voisin     TTL/lifetime            ACK reçu pour ce message
   → retransmettre    → SUPPRIMER (expiration) → SUPPRIMER (purge → libère mémoire)
```

Trois déclencheurs de suppression : **expiration** (TTL sauts épuisé **ou**
*lifetime* temporel `MSG_TTL_S` dépassé) ; **ACK** signé du destinataire qui
repart dans le mesh (chaque relais purge sa copie) ; **quota mémoire**
(éviction du plus ancien / du plus relayé). **Sécurité de l'ACK** : il est signé
par le destinataire, sinon un relais malveillant effacerait des messages en
forgeant de faux ACK.

## 7. Comportement détaillé au MVP

Périmètre v1 : messages **1-à-1**, texte court **~140 à 500 caractères**,
statuts *En attente → Parti → Distribué* (Lu reporté en v2).

**À la réception d'une trame de données (message) :**

1. **Déjà vu ?** `msgID` dans le seen-set → ignorer.
2. **Message expiré ?** `maintenant − timestamp_ms` > `MSG_TTL_S` → ignorer +
   purger les fragments gardés.
3. **Limite anti-inondation** : si ce voisin a dépassé `FLOOD_MAX_PER_MIN_PEER`
   nouveaux `msgID` sur la dernière minute, ignorer les suivants venant de lui
   jusqu'à ce que le débit retombe.
4. **Réassemblage** : si fragmenté, mettre le fragment de côté ; s'arrêter tant
   que tous les fragments ne sont pas là. Délai max ~30 s, au-delà on jette.
5. **Suis-je le destinataire ?** Oui → déchiffrer, vérifier la signature,
   afficher, **émettre un ACCUSÉ**, noter le `msgID` en « déjà vu ». Non →
   relayage.
6. **Relayage** : si TTL = 0 → ne pas relayer ; sinon décrémenter (avec clamp de
   densité), mettre en **file de retransmission**, réémettre à tous les voisins
   **sauf celui qui vient de nous l'envoyer**, après un court délai aléatoire
   (`RELAY_JITTER_MS`, « écouter avant de rediffuser » — si un voisin rediffuse
   déjà ce message, s'abstenir).
7. Noter le `msgID` dans le seen-set (oubli après `SEEN_TTL_S`).

**Accusés** : le destinataire crée une trame ACCUSÉ contenant le `msg_uuid`, le
statut `DISTRIBUÉ`, une **signature du destinataire** sur (uuid + statut +
heure), le tout **chiffré pour l'expéditeur d'origine**. L'ACCUSÉ **se diffuse
comme un message** (son propre TTL, sa propre dédup). Tout relais qui voit
passer un ACCUSÉ pour X retire X de sa file. À réception, l'expéditeur passe à
**Distribué**.

**Store-and-forward** : la file de retransmission garde les messages pas encore
connus comme distribués. À chaque nouveau voisin → échange d'inventaire (§6.2).
File pleine → retirer le **plus ancien**. Un message sort de la file quand : un
ACCUSÉ le concernant passe par le nœud, **ou** il expire, **ou** il est évincé.

**Ordre d'affichage** : en v1, afficher **dans l'ordre où le téléphone reçoit
les messages** (pas de re-tri par horodatage). Un message très en retard
apparaît en bas.

**Garanti en v1** : un relais ne peut pas lire le contenu (E2E) ; ne peut pas
modifier sans que ça se voie (signature) ; l'expéditeur affiché détient bien la
clé privée correspondante ; un rejeu est reconnu (`msgID` + seen-set +
`conv_seq`). **Non garanti en v1** : métadonnées (`peerID` source et horaires en
clair dans l'en-tête → un relais voit *qui parle* et *quand* ; le destinataire
d'une enveloppe est masqué par `recipient_tag`) ; pas de forward secrecy sur les
enveloppes ; horodatages non vérifiables ; inondation seulement atténuée ;
analyse de trafic possible par un adversaire global (hors périmètre).

**Exemple (scénario campus)** : Alice → Bob, Bob hors de portée directe, Carole
entre les deux. (1) Alice crée + chiffre pour Bob, statut « En attente ». (2)
Alice→Carole : Carole reçoit, pas destinataire, TTL 7→6, met en file et
rediffuse ; Alice a transmis à un voisin → « Parti ». (3) Carole→Bob : Bob
reçoit, déchiffre, affiche, émet un ACCUSÉ (signé, chiffré pour Alice). (4)
Bob→Carole→Alice : l'ACCUSÉ remonte ; Carole le voit passer → retire le message
de sa file. (5) Alice : ACCUSÉ reçu → « Distribué ».
