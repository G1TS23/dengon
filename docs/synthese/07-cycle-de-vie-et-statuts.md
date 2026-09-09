# dengon — Cycle de vie d'un message & statuts

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
>
> **Décision A-10 (tranché)** : le statut **« Lu » est reporté en v2**. Le MVP
> s'arrête à *En attente → Parti → Distribué* (+ *Échec/Expiré*). Le type de
> frame `ReadRcpt` et `status = lu` restent **réservés** dans le format (aucune
> rupture pour l'ajouter plus tard).

---

## 1. Les statuts

| Statut (UI) | Code interne | Signification précise | Événement journal | MVP ? |
| --- | --- | --- | --- | --- |
| **En attente** | `QUEUED` | Créé, chiffré, dans l'outbox local. Remis à aucun pair/relais. | `msg.queued` | ✅ |
| **Parti** | `IN_FLIGHT` | Remis à ≥ 1 relais/pair **ou** enveloppe scellée déposée. Sur le réseau, pas encore chez le destinataire. | `msg.handed_off` | ✅ |
| **Distribué** | `DELIVERED` | **ACK signé** de l'appareil destinataire reçu. | `msg.delivered` | ✅ |
| **Lu** | `READ` | **Read-receipt signé** du destinataire reçu (conversation ouverte). | `msg.read` | **v2** |
| *(Échec)* | `EXPIRED` | `MSG_TTL_S` (24 h) écoulé sans `DELIVERED`. | `msg.expired` | ✅ |
| *(Annulé)* | `CANCELLED` | L'expéditeur retire le message avant `IN_FLIGHT`. | `msg.cancelled` | ✅ |

Le suivi porte sur le **`msg_uuid`** (stable de bout en bout), pas le `msgID` L3.
« Parti » ≠ « le destinataire a le message » : c'est « le réseau s'en occupe ».
Transitions **monotones** (jamais en arrière) ; un ACK en double est ignoré.

## 2. Machine à états

**MVP** :

```text
[*] --> QUEUED : send_message()
QUEUED --> CANCELLED : cancel() (avant tout hand-off)
QUEUED --> IN_FLIGHT : 1er paquet remis à un pair OU enveloppe déposée
IN_FLIGHT --> IN_FLIGHT : re-remise à d'autres pairs (resend outbox)
IN_FLIGHT --> DELIVERED : Ack{status=2} signé reçu
IN_FLIGHT --> EXPIRED : MSG_TTL_S dépassé, pas d'Ack
DELIVERED --> [*] ; EXPIRED --> [*] ; CANCELLED --> [*]
```

**Cible v2** (ajout de « Lu ») : `IN_FLIGHT --> READ` (skip delivered si le
read-receipt arrive avant l'ACK, journalise alors `msg.delivered` + `msg.read`) ;
`DELIVERED --> READ` sur read-receipt signé. « Lu implique distribué ».

Implémentation : `dengon-core::sync::status`, une seule fonction
`apply_event(msg_uuid, ev) -> Option<StatusChange>` ; toute mutation passe par
elle et journalise (`ledger.append`).

## 3. Émission

`send_message(dest_peerID, "salut")` → résoudre contact (clé static,
vérifié/TOFU) → si session Noise active : chiffrer AppFrame → `NOISE_MSG` ;
sinon sceller AppFrame → `SEALED_ENVELOPE` (Noise X) + `recipient_tag` du jour →
`outbox.insert(msg_uuid, packet, status=QUEUED)` → `ledger.append("msg.queued")`.
Puis, à **chaque** `PeerConnected` : ANNOUNCE + handshake XX si besoin →
échange d'inventaire → `send(link, packet)` → `outbox.mark(msg_uuid, IN_FLIGHT,
attempts++)` → `ledger.append("msg.handed_off")`.

**Règles de rejeu** : à chaque `PeerConnected`, rejouer tous les `QUEUED`/
`IN_FLIGHT` non expirés dont le pair est destinataire **ou** bon candidat relais ;
`attempts` plafonné (`RESEND_MAX = 8`) par pair, mais illimité dans le temps tant
que `expires_ms` n'est pas atteint ; à réception d'un `Ack{status=2}` →
`DELIVERED`, on **retire** de l'outbox les copies encore en circulation
(best-effort ; l'échange d'inventaire propagera l'absence).

## 4. Réception

`FrameReceived(NOISE_MSG | SEALED_ENVELOPE)` → dédup (`msgID`), vérif →
déchiffrer (Noise X si tag = le mien, sinon Noise transport) →
`messages.insert(msg_uuid, conv, texte, received_ms)` (**idempotent sur
`msg_uuid`**) → `ledger.append("msg.received")` → préparer `Ack{msg_uuid,
status=2}` → envoyer (session si possible, sinon enveloppe vers l'expéditeur).
L'`Ack` est lui-même soumis au store-and-forward.

*(v2 : à l'ouverture de la conversation → préparer + envoyer `ReadReceipt` →
`ledger.append("msg.read_sent")`.)*

## 5. Cas multi-saut avec relais absent du destinataire

Alice → Charlie (éteint), relais R1, R2. Alice n'a pas de session avec Charlie →
`SEALED_ENVELOPE(tag_charlie_J)` ; outbox QUEUED ; Alice→R1 (BLE, signé) →
`IN_FLIGHT` (« parti »). R1 vérifie la sig, stocke l'enveloppe,
`ledger.append("envelope.stored")`, LOG → VPS `pkt.relayed`. R1→R2 :
`ENVELOPE_OFFER [tag_charlie_J]` → `ENVELOPE_REQUEST` → R1 envoie le
`SEALED_ENVELOPE`. Charlie se rallume, entre dans la portée de R2 → `ANNOUNCE` →
R2 `ENVELOPE_OFFER` → Charlie calcule ses tags (J-1,J,J+1) → match →
`ENVELOPE_REQUEST` → R2 envoie → Charlie déchiffre (Noise X), stocke, affiche,
`ledger.append("msg.received")`, émet `Ack{msg_uuid, status=2}` (SEALED vers
Alice, `tag_alice_J`) → R2 stocke l'Ack-enveloppe. Alice se reconnecte plus tard
→ récupère l'enveloppe pour `tag_alice_J` via inventaire/rencontre → déchiffre
l'Ack → statut `DELIVERED`.

Le **dashboard** voit : `msg.handed_off` (Alice, opt-in) → `pkt.relayed` (R1) →
`pkt.relayed`/`envelope.handoff` (R2) → `envelope.delivered` (R2) →
`ack.observed`. Il reconstruit `parti → distribué`.

## 6. Reconnexion — séquence complète

Quand un lien BLE s'établit entre A et B : (1) `ANNOUNCE` mutuels (peerID,
pub_static, pub_sign, pseudo, ledger_height, caps) ; (2) `NOISE_HS ×3` si pas de
session Noise valide en cache ; (3) **en parallèle** — échange d'inventaire
(`INVENTORY` mutuels, puis push de ce qui manque) **et** enveloppes scellées
(`ENVELOPE_OFFER` mutuels, `ENVELOPE_REQUEST` sur les tags qui matchent,
`SEALED_ENVELOPE`) ; (4) vidage d'outbox (`NOISE_MSG` / `SEALED` pour les msgs
QUEUED|IN_FLIGHT destinés à B ou relayables) ; (5) appliquer les Acks reçus →
maj statuts ; (6) si B est un relais avec Wi-Fi : flush buffer littlefs → HTTPS
POST vers le VPS.

C'est **le** mécanisme qui rend « la déconnexion non bloquante » : ce qui
m'était destiné et que je n'ai pas → je le récupère ; ce que je n'ai pas pu
transmettre → je le repousse ; les accusés en retard → ils me rattrapent et mes
statuts avancent.

## 7. Expiration & nettoyage

| Élément | Règle | Effet |
| --- | --- | --- |
| Message outbox | `now > expires_ms` (24 h) sans `DELIVERED` | statut `EXPIRED`, `ledger.append("msg.expired")`, UI « non remis » + bouton renvoyer |
| Enveloppe scellée (chez un porteur) | budget = 0 **et** `now > deposit_ms + MSG_TTL_S` | suppression silencieuse |
| Entrée seen-set | `now > seen_ms + SEEN_TTL_S` | éviction (permet un renvoi légitime ultérieur) |
| Fragment partiel | `now > FRAG_TIMEOUT_S` d'inactivité | abandon |
| Session Noise | lien BLE perdu | oubliée (nouvelle FS à la reconnexion) |
| Cache de réconciliation | `now > 6 h` | éviction (fenêtre de sync) |

## 8. Ordre & cohérence par conversation

`conv_seq` (compteur par conversation, par expéditeur) permet au destinataire de
détecter un **trou** (`… 41, 43 …` → il manque 42) et de le demander au prochain
contact via l'échange d'inventaire. L'affichage suit **l'ordre d'arrivée** en v1
(pas de re-tri par horodatage). Pas d'ordre **global** entre conversations
différentes.

## 9. Table : événement UI ↔ paquet ↔ log

| Action utilisateur / système | Paquet(s) émis | Statut | Événement(s) VPS |
| --- | --- | --- | --- |
| Rédige + envoie | — | `QUEUED` | `msg.queued` (opt-in) |
| 1er pair reçoit le paquet | `NOISE_MSG` / `SEALED_ENVELOPE` | `IN_FLIGHT` | `msg.handed_off` (opt-in), `pkt.relayed` (relais) |
| Relais transporte | `INVENTORY` push / `ENVELOPE_*` | — | `pkt.relayed`, `envelope.stored`, `envelope.handoff` |
| Destinataire reçoit | `ACK{status=2}` | `DELIVERED` (à l'arrivée de l'ACK) | `ack.observed`, `envelope.delivered` |
| 24 h sans ACK | — | `EXPIRED` | `msg.expired` (opt-in) |
| *(v2)* Destinataire ouvre la conv | `ReadReceipt` | `READ` | `read.observed` |
