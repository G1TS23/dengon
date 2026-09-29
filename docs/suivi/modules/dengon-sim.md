# Module : `dengon-sim` (`crates/dengon-sim/`)

**Rôle en une phrase :** faire tourner N nœuds dengon sur une seule machine, sans radio, avec un réseau simulé scriptable et **rejouable à l'identique**.
**Correspond à la conception :** [`docs/synthese/10-benchmarks-mvp-tests.md`](../../synthese/10-benchmarks-mvp-tests.md) §4.3 ; [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §5.
**Dernière mise à jour :** 2026-09-29
**État :** partiel — harness, réseau simulé, scénarios et job CI `sim` livrés (US-221) ; **le vrai `dengon-core::api::Node` est câblé** (`NoeudClient`, US-304) pour les scénarios client-à-client (`direct`, `recipient_offline`, `sender_offline`, `tests/scenarios_reel.rs`) ; `multihop`/`partition_merge` attendent un nœud **relais** (`dengon-core::relay::Relay`), pas encore câblé. `Inondation` reste le bouchon des 4 scénarios `.ron` existants.

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
    scenario.rs   — scénarios RON : lecture, validation, exécution, attendus
    cli.rs        — logique de la commande dengon-sim (testée)
    main.rs       — point d'entrée, délègue à cli
  scenarios/      — direct, multihop, partition_merge, lossy_mesh (.ron,
                    Inondation — pas les scénarios réels, voir tests/)
  tests/
    conformite_sim.rs — suite de conformité Transport (US-105) sur SimTransport
    scenarios.rs      — scénarios .ron livrés : réussis + déterministes
    scenarios_reel.rs — US-304 : direct/recipient_offline/sender_offline
                        avec NoeudClient (vrai dengon-core, pas Inondation)
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
  `dengon-core` — **le vrai nœud** (`api::Node`, US-304), plus le squelette
  de lien (`PROTOCOL_VERSION`, test `le_coeur_est_reellement_lie`).
- **Externes (crates) :** `serde` (derive) + `ron` 0.12 — lecture des
  scénarios `.ron` (réseau/`Inondation` seulement, l'aléa réseau reste un
  SplitMix64 maison, sans dépendance). `rand_core` + `rand_chacha` (US-304,
  `noeud_client.rs` seulement) : `dengon-core::api::Node` exige une RNG pour
  les poignées de main Noise et les UUID de message — `ChaCha20Rng` à
  graine fixe, jamais `OsRng`, pour rester déterministe.

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
- **`multihop`/`partition_merge` restent hors périmètre de `NoeudClient`**
  (US-304, écart consigné) : `api::Node` n'appelle jamais `Router::poll_due`
  (elle ne relaie jamais, voir sa doc de module) — un scénario à 3+ sauts
  réels a besoin d'un nœud **relais** (`dengon-core::relay::Relay`), pas
  encore câblé en `Comportement`.
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
  chemin enveloppe, remise à la reconnexion). Rejoués 3× de suite en local,
  stables.
- Commande : `cargo test -p dengon-sim` → **34 passés** (28 unitaires/scénarios
  `.ron` + 1 conformité + 2 liens dengon-core + 3 scénarios réels),
  2026-09-29.
- Couverture (`cargo llvm-cov -p dengon-sim`, local) : **96,4 %** des lignes
  de `dengon-sim` ; `noeud_client.rs` 92,7 % (nouveau, US-304), `reseau.rs`
  97,1 %, `harness.rs` 98,0 %, `scenario.rs` 96,8 %, `cli.rs` 99 %,
  `alea.rs` 100 % ; `main.rs` 0 % (8 lignes, délègue à `cli`).

## Limites connues / TODO

- **`multihop`/`partition_merge` avec de vrais nœuds pas encore livrés**
  (US-304) : `NoeudClient` (façade client, `api::Node`) n'appelle jamais
  `Router::poll_due` — elle ne relaie jamais un paquet, par conception,
  voir la doc de module d'`api.rs`. Un scénario à 3+ sauts réels a besoin
  d'un nœud **relais** (`dengon-core::relay::Relay`, celui du firmware
  ESP32) câblé en `Comportement`, pas encore fait — suite prévue dans une
  session séparée (voir `03-ecarts-conception.md`).
- **Statuts de message pas exposés côté harness** : les assertions des
  scénarios réels ne portent que sur la **livraison** (`sim.livres`, même
  mécanisme que `Inondation`), pas sur les statuts intermédiaires (`Parti`,
  `Distribué`…) — `Simulation` ne rend aucun accès aux `Comportement`
  ajoutés une fois qu'ils lui appartiennent (`ajouter_noeud` en prend
  possession). Suffisant pour ce que `direct`/`recipient_offline`/
  `sender_offline` doivent démontrer ; à revoir si un futur scénario a
  besoin de plus.
- Modèle réseau **sans bande passante, churn ni dérive d'horloge** (prévus
  par §4.3) : latence, gigue, perte et partition seulement — ce que demande
  US-221.

## Pour l'oral

C'est l'outil qui rend le projet démontrable. Un réseau maillé se comporte bien
avec trois appareils et mal avec cinquante — sauf qu'on n'a pas cinquante
appareils. Le simulateur remplace la radio par de la mémoire, et on peut lui
faire vivre ce qu'on ne saurait pas provoquer à la main : couper le réseau en
deux, perdre 40 % des paquets, puis tout reconnecter. Le point fort : tout est
**rejouable**. Deux exécutions donnent la même trace à l'octet près, et la CI
le vérifie à chaque PR. Un bug trouvé une fois est reproductible à volonté.
