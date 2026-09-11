# Module : `dengon-ble` (`crates/dengon-ble/`)

**Rôle en une phrase :** la couche qui cache la radio Bluetooth au reste du programme.
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §3.
**Dernière mise à jour :** 2026-09-11
**État :** **contrat livré et gelé** (US-105) ; aucune implémentation radio.

## À quoi ça sert

`dengon-core` ne sait pas envoyer un octet. `dengon-ble` porte le trait
`Transport`, décrit dans la conception comme « l'unique couture entre le cœur et
les plateformes » : quatre méthodes (`start`, `poll`, `send`, `broadcast`).
Chaque plateforme en fournit sa propre implémentation — `btleplug` sur PC,
`BluetoothGattServer` côté Android, NimBLE sur l'ESP32 — et le cœur ne voit
jamais la différence.

L'US-105 livre **trois choses** : le contrat lui-même, un bouchon en mémoire
utilisable tout de suite, et une **suite de conformité** que les futures
implémentations rejoueront telles quelles.

## Structure

```
dengon-ble/
  src/
    lib.rs           — ré-exports publics + documentation d'ensemble
    transport.rs     — LE contrat : trait Transport + types partagés
    mock.rs          — MockTransport, bouchon en mémoire
    conformance.rs   — suite de tests réutilisable par les implémentations
  tests/
    conformite_mock.rs — la suite jouée contre MockTransport
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `trait Transport` | `src/transport.rs:307` | Les 4 méthodes. `Send`, non `async`, non bloquant. |
| `LinkId(u64)` | `src/transport.rs:44` | Identifie une **connexion**, pas un nœud. Jamais réutilisé. |
| `TransportConfig` | `src/transport.rs:78` | `local_peer_id`, `advertise`, `scan`, `max_connections`, `preferred_mtu`. |
| `TransportEvent` | `src/transport.rs:167` | `PeerConnected` / `PeerDisconnected` / `FrameReceived`. |
| `DisconnectReason` | `src/transport.rs:146` | `Propre` / `Brutale` / `Locale`. **Ajout par rapport à la conception.** |
| `TransportError` | `src/transport.rs:204` | 6 variantes, `Display` en français, implémente `std::error::Error`. |
| `MockTransport` | `src/mock.rs:57` | Bouchon : implémente `Transport` **et** expose des méthodes de pilotage. |
| `trait BancDEssai` | `src/conformance.rs:67` | Ce qu'une implémentation fournit pour être testée. |
| `suite_complete()` | `src/conformance.rs:307` | Lance les 11 cas de conformité. |

## Flux principal (exemple)

```rust
let mut t = MockTransport::new();
t.start(TransportConfig::default())?;      // 1. démarrer
let lien = t.connecter_pair(Some(-60));    // 2. (pilotage de test) un pair arrive
for e in t.poll() { /* le cœur réagit */ } // 3. le cœur vide la file
t.send(lien, b"paquet")?;                  // 4. et répond
```

Le cœur appelle `poll` dans sa boucle. La radio, elle, vit dans son propre fil
et remplit la file en arrière-plan.

## Dépendances

- **Internes :** `dengon-core` (uniquement pour le test de liaison).
- **Externes (crates) :** **aucune.** Ni `btleplug`, ni `thiserror`.
  `Cargo.lock` est inchangé par cette US — c'est vérifiable : la CI passe avec
  `--locked`.

## Décisions d'implémentation

- **`LinkId` n'est pas un `peerID`.** Le `peerID` (A-8) identifie un nœud, de
  façon stable et cryptographique. Un `LinkId` identifie une connexion, il est
  local au processus. Le transport ne sait pas *qui* est au bout du lien tant
  que le handshake applicatif n'a pas eu lieu : lui faire porter un `peerID`
  reviendrait à lui faire faire de la crypto. **Conséquence utile pour le
  planning :** `dengon-ble` ne dépend pas de `protocol::types` (US-108), donc
  les deux US avancent en parallèle — c'est la règle « 0 dépendance
  intra-sprint ».
- **Un `LinkId` n'est jamais réutilisé.** Sinon une trame en retard sur un
  ancien lien serait attribuée au nouveau pair, et la déduplication en amont ne
  rattraperait pas l'erreur (elle raisonne sur le `msgID`, pas sur l'origine).
  Un compteur monotone suffit, c'est écrit dans le contrat et testé.
- **`poll` ne peut pas échouer**, conformément à `04-architecture.md` §3. Un
  premier jet le faisait rendre un `Result` pour signaler l'oubli de `start` ;
  revenu en arrière : `poll` est appelé en boucle et traverse le FFI vers Kotlin
  et C, où un type résultat coûte cher pour un cas qui ne se produit qu'en cas
  d'erreur de programmation. Avant `start`, `poll` rend un `Vec` vide ; c'est
  `send` qui signale `NotStarted`.
- **Les constantes de protocole ne sont pas dans `TransportConfig`.**
  `SERVICE_UUID`, `CHAR_RX_UUID`, `FRAG_SIZE` sont identiques pour toutes les
  implémentations : ce sont des constantes, pas de la configuration. Elles
  arrivent avec `protocol::consts` (US-108). `TransportConfig` ne porte que ce
  qui **varie d'un nœud à l'autre**.
- **`TransportError::Backend(String)`** plutôt qu'une énumération de codes :
  BlueZ, Android et NimBLE ont chacun les leurs, et les lister ici ferait fuiter
  la plateforme dans un contrat censé être partagé.
- **La suite de conformité est `pub`, pas `#[cfg(test)]`.** Sous `cfg(test)`
  elle ne serait compilée que pour les tests de cette crate et resterait
  inaccessible aux autres — ce qui lui retirerait toute raison d'être.
- **Le test d'intégration est dans `tests/`**, pas dans la crate : un test
  d'intégration ne voit que l'API publique. Si la suite cessait d'être
  utilisable de l'extérieur, ce fichier ne compilerait plus.

## Tests

- `src/transport.rs` : 5 tests (affichage, valeurs par défaut, messages d'erreur).
- `src/mock.rs` : 13 tests (cycle de vie, quota, taille max, unicité du `LinkId`).
- `tests/conformite_mock.rs` : la suite complète + 2 cas isolés.
- 2 doctests (`MockTransport`, `conformance`) — ils servent d'exemple copiable.
- Commande : `cargo test --workspace` → **31 passés, 0 échec** ; `--doc` → 2 passés.

**Ce que les tests ne prouvent pas :** aucune radio n'est touchée. Toute la
conformité est vérifiée contre un bouchon qui, par construction, respecte le
contrat — c'est utile pour figer l'énoncé, ça ne dit rien du comportement de
`btleplug` ou de NimBLE.

## Limites connues / TODO

- **La suite n'a encore été exécutée contre aucune implémentation réelle.**
  Elle *peut* les atteindre toutes les trois — `btleplug` directement,
  `AndroidTransport` parce que `Transport` est une **callback interface**
  UniFFI (`docs/plan-mvp.md:172`, l'objet Kotlin est vu comme un `Transport`
  côté Rust), NimBLE via un adaptateur Rust mince au-dessus du *shim*
  `extern "C"` que le firmware câble déjà. Ce sont donc les mêmes assertions,
  pas un portage par plateforme. Mais aucune de ces implémentations n'existe
  encore, et chacune devra fournir son `BancDEssai` sur **matériel réel** :
  provoquer une vraie coupure brutale demande de couper l'alimentation d'une
  carte. C'est US-213, US-220 et US-303.
- Le Spike B (US-102, `btleplug` en rôle GATT *peripheral* sous Linux) n'est pas
  fait : on ne sait toujours pas si la bibliothèque le permet. Si la réponse est
  non, c'est l'implémentation US-303 qui change, pas ce contrat.
- Pas de méthode `stop()` ni de reconfiguration à chaud : hors périmètre de
  `04-architecture.md` §3. À rouvrir si `dengon-node` en a besoin.
- Couverture non mesurée localement (`cargo-llvm-cov` n'est pas installé sur le
  poste) — c'est la CI qui la rapporte.

## Pour l'oral

Cette petite crate résout un problème d'architecture classique : comment écrire
un programme qui parle Bluetooth sans le coupler à une plateforme précise ? La
réponse est une interface de quatre fonctions. Tout le reste du code ignore s'il
tourne sur un téléphone, un PC ou un microcontrôleur.

Le point intéressant à raconter n'est pas le trait lui-même — quatre méthodes,
c'est banal — mais **les deux choses qu'on a mises autour**. D'abord un bouchon,
qui permet de développer et tester tout le cœur du protocole avant qu'une seule
ligne de Bluetooth existe. Ensuite une suite de conformité : le même jeu
d'assertions que les trois implémentations réelles devront passer. Sans elle,
chaque plateforme dériverait dans son coin et l'écart se découvrirait en
intégration, c'est-à-dire trop tard.

Le cas qui a demandé le plus de réflexion est la **déconnexion brutale** —
quelqu'un s'éloigne, un téléphone se verrouille. Dans un réseau maillé mobile
c'est le cas **normal**, pas l'exception. Le contrat dit précisément quoi faire :
livrer d'abord les trames déjà reçues (elles sont valides, les jeter perdrait un
message que le réseau a déjà transporté), jeter les fragments incomplets, puis
fermer le lien définitivement.
