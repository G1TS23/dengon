# Module : `dengon-node` (`crates/dengon-node/`)

**Rôle en une phrase :** un nœud dengon sans interface graphique, lancé en ligne de commande, qui parle BLE (rôle central) à un pair qui annonce.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §5.
**Dernière mise à jour :** 2026-09-29
**État :** CLI fonctionnelle sur bouchon (US-303) ; **lien BLE réel ouvert avec un téléphone Android** (essai du 2026-09-29), pas d'échange de message.

## À quoi ça sert

Trois usages visés : **banc de test** (un PC contre un téléphone ou un relais
ESP32), **nœud fixe**, **amorçage du maillage**. L'US-303 livre le premier :
identité persistante, envoi et réception d'un message contre un pair.

```
dengon-node identity --name alice --db ./alice.db
dengon-node run --name alice --db ./alice.db --peer <QR-du-pair> --send "salut" --duree 30
```

## Structure

```
dengon-node/
  src/
    main.rs      — CLI clap : sous-commandes `identity` et `run`
    session.rs   — Session<T: Transport> : la boucle Node ⇄ Transport
    etat.rs      — clé locale, coffre d'identité, base SQLite à côté de --db
```

## Concepts / types importants

| Type / fonction | Fichier | Ce que ça fait |
|---|---|---|
| `Session<T: Transport>` | `src/session.rs` | Relève `Transport::poll`, appelle `Node::on_peer_connected` / `on_bytes_received`, envoie `Node::take_outgoing` sur le lien. |
| `Session::envoyer` | `src/session.rs` | `Node::send_message` vers le pair configuré. |
| `ouvrir_noeud` | `src/etat.rs` | `load_or_create` (identité stable entre redémarrages) + `Store::open` + `attach_store`. |
| `lancer` | `src/main.rs` | Boucle de `run` ; derrière la feature `ble`. |

## Flux principal (exemple)

```
$ dengon-node identity --name bob --db bob.db    # chez le pair : peerID + QR
$ dengon-node run --name alice --db alice.db --peer "dengon:v1:…" --send salut
en écoute du pair … (rôle central : le pair doit annoncer)…
lien ouvert
→ salut
← bonjour alice
```

## Dépendances

- **Internes :** `dengon-core` (`Node`, identité, `store`), `dengon-ble`
  (`Transport`, `BtleplugRadio`).
- **Externes (crates) :** `clap` (arguments), `rand_core` avec `getrandom`
  (`OsRng`). Le transport `btleplug` est derrière la feature `ble` (défaut).

## Décisions d'implémentation

- **Central seulement.** `btleplug` ne sait pas annoncer (Spike B) : le nœud
  joint un pair qui annonce, il ne peut pas être joint.
- **Un seul pair, donné par `--peer`.** Le contrat `Transport` ne remonte pas de
  `peerID`, et `Node::on_peer_connected` en exige un ; l'`ANNOUNCE` n'est pas
  câblé. `max_connections = 1`.
- **`Session` générique sur `Transport`** : la même boucle tourne sur
  `btleplug` en production et sur `MockTransport` en test.
- **Clé locale `<db>.key` en clair** (`0600` sous Unix) : nœud de test, pas de
  trousseau.
- **Feature `ble` par défaut** ; `--no-default-features` compile sans `btleplug`
  (la commande `run` refuse alors de démarrer).
- `tokio` reste hors de la boucle du nœud : il vit dans le fil de
  `BtleplugRadio` (conception : pas d'exécuteur imposé au cœur).

## Tests

- `session::tests::deux_noeuds_s_echangent_un_message_de_bout_en_bout` : Alice
  et Bob (deux `Node`, deux `MockTransport` câblés face à face) — Alice écrit,
  Bob reçoit le texte, le bon auteur, `outgoing = false`.
- `session::tests::un_envoi_sans_lien_est_garde_puis_remis_a_la_connexion`.
- `etat::tests::le_peer_id_survit_a_un_redemarrage`.
- Commande : `cargo test -p dengon-node` → 3 passés, 0 échec.

## Limites connues / TODO

- **Non essayé sur BLE réel** : aucun pair annonçant disponible ; sur le poste
  Windows de dev, `run` échoue proprement (« Le périphérique n'est pas prêt »).
- Pas d'essai sous Linux/BlueZ (le spike B demandait de le revalider ici).
- Pas de retour d'accusé de réception affiché : la façade `Node` n'en émet pas.
- Pas d'entrée interactive : les messages sont passés par `--send`.
- Les frames au-delà de 512 octets sont refusées (`FrameTooLarge`) ; aucune
  fragmentation BLE.
- `cargo deny` / `cargo audit` non lancés en local : licences des dépendances de
  `btleplug` non contrôlées.

## Pour l'oral

C'est la version « ligne de commande » de l'application : mêmes capacités, sans
l'interface. Le point à raconter : la boucle `Session` ne sait pas si elle parle
à de vrais capteurs BLE ou à un bouchon, et c'est pour cela qu'on a pu prouver
l'échange de bout en bout sans matériel — au prix de ne rien affirmer sur la
radio elle-même, qui reste à essayer sur appareils.
