# Module : `dengon-sim` (`crates/dengon-sim/`)

**Rôle en une phrase :** faire tourner N nœuds dengon sur une seule machine, sans radio, avec un réseau simulé scriptable et **rejouable à l'identique**.
**Correspond à la conception :** [`docs/synthese/10-benchmarks-mvp-tests.md`](../../synthese/10-benchmarks-mvp-tests.md) §4.3 ; [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §5.
**Dernière mise à jour :** 2026-09-29
**État :** harness, réseau simulé, scénarios `.ron` et job CI `sim` livrés (US-221) ; **les 5 scénarios réels du DoD de l'US-304 sont livrés** avec le vrai `dengon-core` : `direct`/`recipient_offline`/`sender_offline` via `NoeudClient` (`api::Node`, `tests/scenarios_reel.rs`), `multihop`/`partition_merge` via `NoeudRelais` (`relay::Relay`, `tests/scenarios_relais.rs`). **Mode démo pour l'oral** (`dengon-sim --demo`, `src/demo.rs`) : rejoue les 5 avec une narration lisible, déterministe (vérifié identique sur 3 exécutions). `Inondation` reste le bouchon des 4 scénarios `.ron` existants (inchangés).

## À quoi ça sert

On ne peut pas tester un réseau maillé avec trois téléphones. `dengon-sim`
remplace la radio par un **réseau en mémoire** : une horloge virtuelle, une
topologie (qui est à portée de qui), et pour chaque lien une latence, une gigue
et un taux de perte. On peut couper le réseau en deux, le réunir, retirer un
lien, et observer ce que font les nœuds.

Tout est **déterministe** : même graine → même trace d'exécution, octet pour
octet. Un scénario qui échoue en CI se rejoue donc à l'identique en local.

## Structure

```
dengon-sim/
  src/
    lib.rs        — re-exports, SEED_PAR_DEFAUT
    alea.rs       — SplitMix64 : le seul hasard du simulateur
    reseau.rs     — Reseau / ReseauPartage (horloge, topologie, partitions,
                    files datées, trace) + SimTransport (impl Transport)
    harness.rs    — Simulation (N nœuds), trait Comportement, Inondation
    noeud_client.rs — NoeudClient : le vrai api::Node en Comportement (US-304)
    noeud_relais.rs — NoeudRelais : le vrai relay::Relay en Comportement (US-304)
    demo.rs       — mode `--demo` : les 5 scénarios réels, narrés pour l'oral
    scenario.rs   — scénarios RON : lecture, validation, exécution, attendus
    cli.rs        — logique de la commande dengon-sim (testée)
    main.rs       — point d'entrée, délègue à cli
  scenarios/      — direct, multihop, partition_merge, lossy_mesh (.ron,
                    Inondation — pas les scénarios réels, voir tests/)
  tests/
    conformite_sim.rs   — suite de conformité Transport (US-105) sur SimTransport
    scenarios.rs        — scénarios .ron livrés : réussis + déterministes
    scenarios_reel.rs   — US-304 : direct/recipient_offline/sender_offline
                          avec NoeudClient (vrai dengon-core, pas Inondation)
    scenarios_relais.rs — US-304 : multihop/partition_merge avec NoeudRelais
                          (vrai relay::Relay — NoeudClient ne relaie jamais)
```

## Concepts / types importants

| Type / fonction | Fichier | Ce que ça fait |
|---|---|---|
| `SimTransport` | `src/reseau.rs` | Implémente le contrat **gelé** `dengon_ble::Transport`. Passe la **même suite de conformité** que `MockTransport` (`tests/conformite_sim.rs`). |
| `ReseauPartage` | `src/reseau.rs` | Poignée sur le réseau partagé : `relier`/`delier` (arêtes radio), `partitionner`/`reunir`, `couper_lien`, `avancer` (horloge), `trace`. |
| `ParametresLien` | `src/reseau.rs` | `latence_ms`, `gigue_ms`, `perte_pour_mille` d'une arête. |
| `EntreeTrace`, `Horodate`, `empreinte()` | `src/reseau.rs` | Trace datée (connexion, coupure, émission, perte, perte en vol, réception, livraison) et son empreinte FNV-1a 64 bits. |
| `Simulation` | `src/harness.rs` | N nœuds (transport + comportement), `emettre`, `pas`, `executer_jusqu_a`, `livres`. |
| `trait Comportement` / `Contexte` | `src/harness.rs` | Ce qu'un nœud fait de son transport à chaque pas. Point d'injection du futur nœud `dengon-core`. |
| `Inondation` | `src/harness.rs` | Relais de démonstration : dédup par contenu, re-diffusion, et poussée de tout ce qui est connu à chaque nouvelle connexion (convergence après partition). |
| `NoeudClient` | `src/noeud_client.rs` | Le vrai `dengon-core::api::Node` en `Comportement` (US-304) : `ANNOUNCE` à l'ouverture d'un lien pour associer `LinkId`↔`PeerId` (le harness ne le donne pas d'avance — même limite que le vrai Bluetooth), Noise `XX`/enveloppe, outbox rejouée à la reconnexion. |
| `charge_adressee` | `src/noeud_client.rs` | Convention locale (pas une extension du format `.ron`) : `PeerId` destinataire (8 o) ‖ texte UTF-8, décodée par `NoeudClient::emettre`. |
| `NoeudRelais` | `src/noeud_relais.rs` | Le vrai `dengon-core::relay::Relay<LinkId>` en `Comportement` (US-304) : découverte de pair et cache d'inventaire déjà gérés en interne par `Relay` (contrairement à `NoeudClient`, pas d'ANNOUNCE à réimplémenter côté simulateur). Observable via `ctx.livrer(MARQUEUR_CACHE\|MARQUEUR_RELAYE)`. |
| `paquet_diffuse` | `src/noeud_relais.rs` | Construit un paquet signé (`SealedEnvelope`, `RELAY_OK`) à injecter via `Simulation::emettre` — un « tiers » qui dépose un paquet sans être lui-même modélisé en nœud. |
| `Scenario`, `Rapport` | `src/scenario.rs` | Scénario RON validé (indices, pertes, pas) → exécution → empreinte + attendus non tenus. |
| `Alea` | `src/alea.rs` | SplitMix64 à graine fixe (vecteur de référence testé). |

## Flux principal (exemple)

`dengon-sim crates/dengon-sim/scenarios/partition_merge.ron` :

1. 4 nœuds en chaîne `0 — 1 — 2 — 3`, 10 ms par saut ; les connexions
   s'établissent dès que les deux transports sont démarrés.
2. À 20 ms : partition `{0,1} | {2,3}` → le lien `1 — 2` tombe
   (`PeerDisconnected`, motif `Brutale`, des deux côtés).
3. À 50 ms : 0 émet « gauche », 3 émet « droite » ; chaque moitié le propage.
4. À 600 ms : réunion → nouvelle connexion `1 — 2` (nouveaux `LinkId`) ;
   `Inondation` pousse tout ce qu'elle connaît au nouveau voisin.
5. Fin : chaque nœud a reçu le message de l'autre moitié ; la sortie donne
   `✓ partition_merge graine=… empreinte=…`.

## Dépendances

- **Internes :** `dengon-ble` (contrat `Transport`, suite de conformité),
  `dengon-core` — **les deux vrais nœuds** (`api::Node` et `relay::Relay`,
  US-304), plus le squelette de lien (`PROTOCOL_VERSION`, test
  `le_coeur_est_reellement_lie`).
- **Externes (crates) :** `serde` (derive) + `ron` 0.12 — lecture des
  scénarios `.ron` (réseau/`Inondation` seulement, l'aléa réseau reste un
  SplitMix64 maison, sans dépendance). `rand_core` + `rand_chacha` (US-304,
  `noeud_client.rs`/`noeud_relais.rs`) : `dengon-core::api::Node` et
  `relay::Relay` exigent tous deux une RNG (poignées de main Noise, UUID de
  message, secrets/signatures) — `ChaCha20Rng` à graine fixe, jamais
  `OsRng`, pour rester déterministe.

## Décisions d'implémentation

- **Aucune source de hasard implicite** : horloge virtuelle (jamais
  `SystemTime`), aléa à graine fixe, `BTreeMap`/`BTreeSet` partout (l'ordre
  d'itération d'un `HashMap` est aléatoire par processus), nœuds servis par
  indice croissant. Le job CI `sim` exécute les scénarios deux fois et compare.
- **`SimTransport` passe la suite de conformité US-105** : le simulateur
  respecte le même contrat que les vrais transports (déconnexion brutale,
  trame reçue avant coupure livrée avant l'événement de fermeture, `LinkId`
  jamais réutilisé…). Un comportement testé en simulation se comportera pareil
  sur un transport réel conforme.
- **Ordre par lien garanti malgré la gigue** : une trame n'arrive jamais avant
  la précédente sur le même sens du même lien (contrat `TransportEvent`).
- **Coupure** : ce qui est déjà arrivé reste livrable ; ce qui est encore en
  vol est jeté et tracé `PerteEnVol`.
- **Rôles et quotas respectés** : deux nœuds ne se connectent que si l'un
  scanne et l'autre annonce, et sous leur `max_connections`.
- **Comportement injecté** (`Comportement`) : `Inondation` (bouchon) et
  `NoeudClient` (vrai `api::Node`, US-304) coexistent, le harness n'a pas eu
  à changer pour accueillir le second.
- **`NoeudClient` découvre ses voisins par `ANNOUNCE`, pas par une triche du
  harness** (US-304) : `api::Node` ne reçoit qu'un `PeerId` explicite, le
  `SimTransport` ne donne qu'un `LinkId` opaque — même limite que le vrai
  Bluetooth. `NoeudClient` envoie son propre `ANNOUNCE`
  (`Node::announce_packet`, déjà livré par US-306) à l'ouverture d'un lien,
  et attend le même du voisin (`api::parse_announce`) avant d'appeler
  `on_peer_connected` — exactement le mécanisme déjà utilisé par
  `relay.rs`/`dengon-node::session.rs`, pas un nouveau à expliquer à l'oral.
- **Adressage par convention locale (`charge_adressee`), pas une extension
  du format `.ron`** (US-304) : `Comportement::emettre` ne porte qu'une
  charge opaque ; les 8 premiers octets valent `PeerId` destinataire côté
  `NoeudClient`, le reste le texte. Les 4 scénarios `.ron` existants restent
  inchangés (ils utilisent `Inondation`, qui diffuse sans adresse). Les
  scénarios réels sont donc écrits en Rust
  (`tests/scenarios_reel.rs`), pas en RON.
- **Contacts pré-partagés à la construction du scénario**
  (`NoeudClient::add_contact`), pas découverts par le réseau : reproduit la
  vraie condition (`api.rs`, « Ce que cette façade N'est PAS ») —
  `send_message` exige un contact connu ou une session ; un `PeerId` jamais
  rencontré n'est jamais un correspondant valable, en simulation comme en
  vrai.
- **`multihop`/`partition_merge` utilisent `NoeudRelais`, pas
  `NoeudClient`** (US-304) : `api::Node` n'appelle jamais `Router::poll_due`
  (elle ne relaie jamais, voir sa doc de module) — même distinction que
  dans le vrai système, téléphone contre relais dédié.
- **`Relay` gère lui-même la découverte de pair et le cache d'inventaire**
  (contrairement à `NoeudClient`) : `NoeudRelais` n'a donc besoin d'aucune
  logique d'ANNOUNCE côté simulateur — `Relay::link_up`/`on_frame` s'en
  chargent en interne, génériques sur `L` (le `LinkId` du simulateur, tel
  quel).
- **Horloge murale décalée de `WALL_CLOCK_MIN_MS`** (US-304, piège
  rencontré) : `Relay` refuse toute décision de routage tant que son
  horloge murale n'est pas **réaliste** (≥ 2024-01-01,
  `Relay::clock`/`WALL_CLOCK_MIN_MS`) — un ESP32 sans Wi-Fi l'apprend du
  premier `ANNOUNCE` reçu au-dessus de ce seuil. L'horloge virtuelle du
  simulateur part de zéro, bien en dessous ; sans compensation, aucun
  relais ne prendrait jamais de décision (tout resterait `clock_unknown`).
  `NoeudRelais::maintenant` ajoute donc `WALL_CLOCK_MIN_MS` à l'horloge
  murale transmise (la monotone reste celle du simulateur telle quelle) —
  équivalent d'un relais qui a déjà une horloge fiable au démarrage.
- **Injection par paquet complet déjà signé (`paquet_diffuse`), pas par
  charge applicative** (US-304) : `Relay` n'a pas d'application — rien à
  composer. `NoeudRelais::emettre` diffuse `charge` telle quelle, comme un
  tiers non modélisé (téléphone hors de portée avant/après) qui vient de la
  déposer.
- **`SealedEnvelope`, pas `LogAttest`, pour observer `MARQUEUR_CACHE`**
  (US-304, piège rencontré en écrivant les scénarios) :
  `sync::inventory::cacheable` ne met en cache que
  `SealedEnvelope`/`NoiseMsg`/`Ack` — `Announce`/`LogAttest`, même bien
  relayés (`MARQUEUR_RELAYE` sort), ne laissent **jamais** de trace dans le
  cache d'inventaire.
- **Observer un relais via `ctx.livrer` détourné, pas une nouvelle API** :
  `Simulation` ne rend aucun accès à un `Comportement` une fois ajouté —
  `NoeudRelais` y pousse `MARQUEUR_CACHE`/`MARQUEUR_RELAYE` à chaque
  variation constatée de `Relay::cache_len()`/`RelayStats::relayed`, seul
  canal d'observation externe disponible.
- **Après une partition, la reprise passe par la réconciliation
  d'inventaire, pas par un nouvel essai du relais jitté** (US-304,
  `partition_merge`) : un relais programmé dont l'échéance passe **sans
  cible** (lien coupé) est définitivement perdu (`relays_without_target`,
  pas retenté) — c'est `sync::inventory` (déjà en cache, poussé au nouveau
  voisin après reconnexion) qui fait traverser le paquet une fois la
  jonction reformée, pas une deuxième tentative de `Router::poll_due`.
- **Logique CLI dans `cli.rs`** (testée) ; `main.rs` ne fait que déléguer.

## Tests

- `src/alea.rs` — 3 : vecteur de référence SplitMix64, même graine → même
  suite, borne de `sous`.
- `src/reseau.rs` — 10 : latence, ordre par lien sous gigue, perte totale,
  partition/réunion (nouveaux `LinkId`), perte en vol à la coupure, quota et
  rôles, broadcast et taille max, motifs symétriques de `couper_lien`, nœud
  inconnu / non démarré, empreinte.
- `src/harness.rs` — 4 : multi-saut sur une chaîne, convergence après
  partition, **même graine → même trace** (réseau à pertes et gigue), cas
  limites.
- `src/scenario.rs` — 3 : scénario direct réussi et rejoué à l'identique,
  attendus non tenus rapportés + toutes les actions, 10 scénarios incohérents
  refusés.
- `src/cli.rs` — 2 : usage / graine invalide, exécution multi-fichiers
  (succès, échec d'attendu, fichier illisible, fichier absent).
- `tests/conformite_sim.rs` — `suite_complete` de `dengon-ble` sur
  `SimTransport`.
- `tests/scenarios.rs` — 3 : scénarios `.ron` livrés réussis, même graine →
  même trace pour chacun, graine différente → empreinte différente sur
  `lossy_mesh` (et pertes réellement tirées).
- `src/noeud_client.rs` — 3 : `charge_adressee` intacte, identités
  distinctes/identiques selon la graine.
- `tests/scenarios_reel.rs` — 3 (US-304, `NoeudClient`) : `direct` (remise
  immédiate, lien déjà ouvert), `recipient_offline` (destinataire jamais
  connecté au moment de l'envoi → rien n'arrive → remise automatique dès la
  connexion), `sender_offline` (émetteur déconnecté puis reconnecté →
  chemin enveloppe, remise à la reconnexion).
- `src/noeud_relais.rs` — 4 : identités distinctes/identiques selon la
  graine de secrets, `paquet_diffuse` se décode et se vérifie, signature
  déterministe à graine fixe.
- `tests/scenarios_relais.rs` — 2 (US-304, `NoeudRelais`) : `multihop`
  (chaîne à 3 relais, le maillon du milieu reçoit **puis** relaie, le
  dernier reçoit sans relayer plus loin faute de cible), `partition_merge`
  (chaîne à 4 relais partitionnée en deux, confinement vérifié pendant la
  partition, traversée complète vérifiée après réunion via la
  réconciliation d'inventaire).
- Les 5 scénarios réels (`scenarios_reel.rs` + `scenarios_relais.rs`)
  rejoués 3× de suite en local : stables.
- `src/demo.rs` — 1 : les 5 scénarios réels réussissent et se narrent
  (mêmes graines que les tests ci-dessus — même résultat garanti, pas une
  version parallèle qui pourrait diverger). `cli.rs` — 1 de plus :
  `--demo` rejoue bien les 5, `--demo` + un fichier `.ron` refusé.
- **`dengon-sim --demo` lancé 3× de suite en dehors de `cargo test`** :
  sortie **identique à l'octet près** (`diff` sur les 3 sorties), code de
  sortie 0 — c'est la commande à lancer devant le jury (voir « Pour
  l'oral »).
- Commande : `cargo test -p dengon-sim` → **42 passés** (7 suites : lib +
  `conformite_sim` + `scenarios` + `scenarios_reel` + `scenarios_relais` +
  intégration `dengon-core`), 2026-09-29.
- Couverture (`cargo llvm-cov -p dengon-sim`, local) : **96,6 %** des lignes
  de `dengon-sim` ; `noeud_client.rs` 92,7 %, `noeud_relais.rs` 97,5 %,
  `demo.rs` 99,5 % (les trois, US-304), `reseau.rs` 97,1 %, `harness.rs`
  98,0 %, `scenario.rs` 96,8 %, `cli.rs` 99,2 %, `alea.rs` 100 % ;
  `main.rs` 0 % (8 lignes, délègue à `cli`).

## Limites connues / TODO

- **Statuts de message pas exposés côté harness** : les assertions des
  scénarios réels ne portent que sur la **livraison**/le **relais observé**
  (`sim.livres`, même mécanisme que `Inondation`), pas sur les statuts
  intermédiaires côté client (`Parti`, `Distribué`…) ni les compteurs
  détaillés côté relais (`RelayStats` complet, `envelopes_held`…) —
  `Simulation` ne rend aucun accès aux `Comportement` ajoutés une fois
  qu'ils lui appartiennent (`ajouter_noeud` en prend possession).
  Suffisant pour les 5 scénarios du DoD de l'US-304 ; à revoir si un futur
  scénario a besoin de plus.
- **Aucun scénario ne mélange `NoeudClient` et `NoeudRelais`** : un vrai
  téléphone qui traverse un vrai relais (bout en bout, chiffré/déchiffré
  aux deux extrémités) n'est pas démontré — `api::Node` ne pose jamais
  `Flags::RELAY_OK` sur ses paquets de session (écart déjà documenté côté
  `api.rs`), ce qui aurait exigé d'y toucher pour ce scénario précis.
  `multihop`/`partition_merge` démontrent le maillage relais-à-relais avec
  un paquet injecté directement (`paquet_diffuse`), pas une conversation
  client réelle bout en bout.
- Modèle réseau **sans bande passante, churn ni dérive d'horloge** (prévus
  par §4.3) : latence, gigue, perte et partition seulement — ce que demande
  US-221.

## Pour l'oral

**Commande à lancer devant le jury** (le vrai `dengon-core`, pas
`Inondation`) :

```
$ cargo run -p dengon-sim -- --demo
```

Rejoue les 5 scénarios du DoD (`direct`, `recipient_offline`,
`sender_offline`, `multihop`, `partition_merge`) avec une narration ligne
par ligne (qui parle à qui, quand, quel résultat) — pas un `cargo test`
silencieux. Déterministe : trois lancements donnent la sortie identique à
l'octet près.

C'est l'outil qui rend le projet démontrable. Un réseau maillé se comporte bien
avec trois appareils et mal avec cinquante — sauf qu'on n'a pas cinquante
appareils. Le simulateur remplace la radio par de la mémoire, et on peut lui
faire vivre ce qu'on ne saurait pas provoquer à la main : couper le réseau en
deux, perdre 40 % des paquets, puis tout reconnecter. Le point fort : tout est
**rejouable**. Deux exécutions donnent la même trace à l'octet près, et la CI
le vérifie à chaque PR. Un bug trouvé une fois est reproductible à volonté.
