# Cycle de vie d'un message & statuts

## 1. Les statuts

| Statut (UI) | Code interne | Signification précise | Événement journal |
| --- | --- | --- | --- |
| **En attente** | `QUEUED` | Créé, chiffré, dans l'outbox local. **Remis à aucun** pair/relais. | `msg.queued` |
| **Parti** | `IN_FLIGHT` | Remis à ≥ 1 relais/pair **ou** enveloppe scellée déposée. Sur le réseau, pas encore chez le destinataire. | `msg.handed_off` |
| **Distribué** | `DELIVERED` | **ACK signé** de l'appareil destinataire reçu (le message est arrivé sur son téléphone). | `msg.delivered` |
| **Lu** | `READ` | **Read-receipt signé** du destinataire reçu (l'utilisateur a ouvert la conversation). | `msg.read` |
| *(Échec)* | `EXPIRED` | `MSG_TTL_S` (24 h) écoulé sans `DELIVERED`, budget de copies épuisé. | `msg.expired` |
| *(Annulé)* | `CANCELLED` | L'expéditeur retire le message avant `IN_FLIGHT`. | `msg.cancelled` |

Notes :

- Le suivi porte sur le **`msg_uuid`** (stable de bout en bout), pas le `msgID` L3
  (qui change si le paquet est re-scellé). Cf. `03-network-protocol.md` §4.1.
- « Parti » ≠ « le destinataire a le message ». C'est « le réseau s'en occupe ».
- Les transitions sont **monotones** : on ne revient jamais en arrière
  (`QUEUED → IN_FLIGHT → DELIVERED → READ`). Un ACK en double est ignoré.
- **Lu implique distribué** : recevoir un read-receipt sans avoir vu l'ACK fait
  passer directement `IN_FLIGHT → READ` (et journalise `msg.delivered` +
  `msg.read`).

---

## 2. Machine à états

```mermaid
stateDiagram-v2
    [*] --> QUEUED : send_message()
    QUEUED --> CANCELLED : cancel() (avant tout hand-off)
    QUEUED --> IN_FLIGHT : 1er paquet remis à un pair\nOU enveloppe déposée
    IN_FLIGHT --> IN_FLIGHT : re-remise à d'autres pairs\n(resend outbox)
    IN_FLIGHT --> DELIVERED : Ack{status=2} signé reçu
    IN_FLIGHT --> READ : ReadReceipt reçu (skip delivered)
    IN_FLIGHT --> EXPIRED : MSG_TTL_S dépassé, pas d'Ack
    DELIVERED --> READ : ReadReceipt signé reçu
    DELIVERED --> [*]
    READ --> [*]
    EXPIRED --> [*]
    CANCELLED --> [*]
```

Implémentation : `dengon-core::sync::status`. Une seule fonction
`apply_event(msg_uuid, ev) -> Option<StatusChange>` ; toute mutation passe par elle
et journalise (`ledger.append`).

---

## 3. Émission (expéditeur)

```mermaid
sequenceDiagram
    participant UI
    participant Core as dengon-core
    participant Store
    participant TR as Transport

    UI->>Core: send_message(dest_peerID, "salut")
    Core->>Core: résoudre contact (clé static, vérifié/TOFU)
    alt session Noise active avec dest
        Core->>Core: chiffrer AppFrame -> NOISE_MSG
    else pas de session
        Core->>Core: sceller AppFrame -> SEALED_ENVELOPE (Noise X)\n+ recipient_tag du jour
    end
    Core->>Store: outbox.insert(msg_uuid, packet, status=QUEUED)
    Core->>Core: ledger.append("msg.queued")
    Core-->>UI: msg_uuid, status=QUEUED

    loop à chaque pair connecté / reconnecté
        TR-->>Core: PeerConnected(link)
        Core->>Core: ANNOUNCE + handshake XX si besoin
        Core->>Core: gossip: réconciliation
        Core->>TR: send(link, packet)  (ou ENVELOPE_OFFER/…)
        Core->>Store: outbox.mark(msg_uuid, IN_FLIGHT, attempts++)
        Core->>Core: ledger.append("msg.handed_off")
        Core-->>UI: status=IN_FLIGHT
    end
```

**Outbox** (`store.outbox`) : `{ msg_uuid, dest_peerID, packet_bytes, status,
attempts, first_sent_ms, last_sent_ms, expires_ms }`.

Règles de rejeu :

- à **chaque** `PeerConnected`, on rejoue tous les `QUEUED`/`IN_FLIGHT` non expirés
  dont le pair est destinataire **ou** bon candidat relais ;
- `attempts` plafonné (`RESEND_MAX = 8`) par pair, mais illimité dans le temps tant
  que `expires_ms` n'est pas atteint ;
- à réception d'un `Ack{status=2}` → `DELIVERED`, on **retire** de l'outbox les
  copies encore en circulation (best-effort : `GOSSIP` propagera l'absence).

---

## 4. Réception (destinataire)

```mermaid
sequenceDiagram
    participant TR as Transport
    participant Core as dengon-core
    participant Store
    participant UI

    TR-->>Core: FrameReceived(NOISE_MSG | SEALED_ENVELOPE)
    Core->>Core: dédup (msgID), vérif
    alt SEALED_ENVELOPE et tag = le mien
        Core->>Core: déchiffrer Noise X
    else NOISE_MSG dans session
        Core->>Core: déchiffrer Noise transport
    end
    Core->>Store: messages.insert(msg_uuid, conv, texte, received_ms)\n(idempotent sur msg_uuid)
    Core->>Core: ledger.append("msg.received")
    Core->>Core: préparer Ack{msg_uuid, status=2}
    Core->>TR: envoyer Ack (session si possible, sinon enveloppe vers l'expéditeur)
    Core-->>UI: nouveau message (badge non-lu)

    UI->>Core: ouvrir la conversation
    Core->>Core: préparer ReadReceipt{msg_uuid}
    Core->>TR: envoyer ReadReceipt
    Core->>Core: ledger.append("msg.read_sent")
```

- L'`Ack` et le `ReadReceipt` sont eux-mêmes soumis au **store-and-forward** : si
  l'expéditeur est déconnecté, ils partent en `SEALED_ENVELOPE` adressée à lui, ou
  attendent dans l'outbox du destinataire.
- `messages.insert` est **idempotent** sur `msg_uuid` : recevoir le même message par
  deux chemins n'affiche qu'un exemplaire et n'émet qu'un `Ack`.

---

## 5. Cas multi-saut avec relais absent du destinataire

Scénario : **Alice → Charlie**, Charlie éteint. Relais R1, R2 (ESP32).

```mermaid
sequenceDiagram
    participant A as Alice
    participant R1 as Relais R1
    participant R2 as Relais R2
    participant C as Charlie

    A->>A: pas de session avec Charlie -> SEALED_ENVELOPE(tag_charlie_J)
    A->>A: outbox: QUEUED
    A->>R1: BLE: SEALED_ENVELOPE (signé)
    A->>A: status -> IN_FLIGHT ("parti")
    R1->>R1: vérif sig, stocker enveloppe (copy_budget=4)
    R1->>R1: ledger.append("envelope.stored") ; LOG -> VPS: pkt.relayed
    R1->>R2: gossip: ENVELOPE_OFFER [tag_charlie_J]
    R2->>R1: ENVELOPE_REQUEST
    R1->>R2: SEALED_ENVELOPE  (budget partagé : R1=2, R2=2)
    Note over C: Charlie se rallume, entre dans la portée de R2
    C->>R2: ANNOUNCE
    R2->>C: ENVELOPE_OFFER [tag_charlie_J, …]
    C->>C: calcule ses tags (J-1,J,J+1) -> match
    C->>R2: ENVELOPE_REQUEST [tag_charlie_J]
    R2->>C: SEALED_ENVELOPE
    C->>C: déchiffre (Noise X), stocke, affiche
    C->>C: ledger.append("msg.received")
    C->>R2: Ack{msg_uuid, status=2}  (SEALED vers Alice, tag_alice_J)
    R2->>R2: stocker Ack-enveloppe
    Note over A: Alice se reconnecte plus tard
    A->>R2: (via gossip / rencontre) ENVELOPE pour tag_alice_J
    A->>A: déchiffre Ack -> status DELIVERED ("distribué")
```

Statut vu par Alice : `en attente → parti` (dès R1) `→ distribué` (quand l'Ack de
Charlie la rattrape). `lu` suivra pareil si Charlie ouvre la conversation.

Le **dashboard** voit : `msg.handed_off` (Alice, opt-in) → `pkt.relayed` (R1) →
`pkt.relayed`/`envelope.handoff` (R2) → `envelope.delivered` (R2, quand Charlie
récupère) → `ack.observed`. Il reconstruit `parti → distribué`.

---

## 6. Reconnexion — séquence complète

Quand un lien BLE s'établit entre **A** et **B** (peu importe qui est « client ») :

```mermaid
sequenceDiagram
    participant A
    participant B

    A->>B: ANNOUNCE (peerID, pub_static, pub_sign, pseudo, ledger_height, caps)
    B->>A: ANNOUNCE
    opt pas de session Noise valide en cache
        A->>B: NOISE_HS ×3 (handshake XX)
    end

    par Réconciliation gossip
        A->>B: GOSSIP_FILTER (GCS des msgID connus)
        B->>A: GOSSIP_FILTER
        A->>B: GOSSIP_PULL / GOSSIP_PUSH (ce qui manque à B)
        B->>A: GOSSIP_PULL / GOSSIP_PUSH (ce qui manque à A)
    and Enveloppes scellées
        A->>B: ENVELOPE_OFFER (tags portés par A)
        B->>A: ENVELOPE_OFFER (tags portés par B)
        A->>B: ENVELOPE_REQUEST (tags de A qui matchent)
        B->>A: SEALED_ENVELOPE(s)
        B->>A: ENVELOPE_REQUEST (tags de B qui matchent)
        A->>B: SEALED_ENVELOPE(s)
    end

    A->>B: vidage outbox : NOISE_MSG / SEALED pour msgs QUEUED|IN_FLIGHT destinés à B ou relayables
    B->>A: idem
    A->>A: appliquer Acks/ReadReceipts reçus -> maj statuts
    B->>B: idem

    opt B est un relais avec Wi-Fi
        B->>B: flush buffer NVS -> MQTT vers le VPS
    end
```

C'est **le** mécanisme qui rend « la déconnexion non bloquante » :

- ce qui **m'était destiné et que je n'ai pas** → je le récupère (gossip + enveloppes) ;
- ce que **je n'ai pas pu transmettre** → je le repousse (vidage d'outbox) ;
- les **accusés en retard** → ils me rattrapent et mes statuts avancent.

---

## 7. Expiration & nettoyage

| Élément | Règle | Effet |
| --- | --- | --- |
| Message outbox | `now > expires_ms` (24 h) sans `DELIVERED` | statut `EXPIRED`, `ledger.append("msg.expired")`, UI « non remis » + bouton renvoyer |
| Enveloppe scellée (chez un porteur) | budget = 0 **et** `now > deposit_ms + MSG_TTL_S` | suppression silencieuse |
| Entrée seen-set | `now > seen_ms + SEEN_TTL_S` | éviction (permet un renvoi légitime ultérieur) |
| Fragment partiel | `now > FRAG_TIMEOUT_S` d'inactivité | abandon |
| Session Noise | lien BLE perdu | oubliée (nouvelle FS à la reconnexion) |
| Cache gossip public | `now > 6 h` | éviction (fenêtre de sync) |

---

## 8. Ordre & cohérence par conversation

- `conv_seq` (compteur par conversation, par expéditeur) permet au destinataire de
  détecter un **trou** (`… 41, 43 …` → il manque 42) et de le demander explicitement
  via `GOSSIP_PULL` au prochain contact.
- L'affichage ordonne par `sent_ms` ; en cas d'égalité, par `conv_seq`.
- Pas d'ordre **global** entre conversations différentes (inutile, cf.
  `01-benchmarks.md` §1.3).

---

## 9. Table : événement UI ↔ paquet ↔ log

| Action utilisateur / système | Paquet(s) émis | Statut | Événement(s) VPS |
| --- | --- | --- | --- |
| Rédige + envoie | — | `QUEUED` | `msg.queued` (opt-in) |
| 1er pair reçoit le paquet | `NOISE_MSG` / `SEALED_ENVELOPE` | `IN_FLIGHT` | `msg.handed_off` (opt-in), `pkt.relayed` (relais) |
| Relais transporte | `GOSSIP_PUSH` / `ENVELOPE_*` | — | `pkt.relayed`, `envelope.stored`, `envelope.handoff` |
| Destinataire reçoit | `ACK{status=2}` | `DELIVERED` (à l'arrivée de l'ACK) | `ack.observed`, `envelope.delivered` |
| Destinataire ouvre la conv | `ReadReceipt` | `READ` | `read.observed` |
| 24 h sans ACK | — | `EXPIRED` | `msg.expired` (opt-in) |
