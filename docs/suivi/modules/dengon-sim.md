# Module : `dengon-sim` (`crates/dengon-sim/`)

**Rôle en une phrase :** faire tourner N nœuds dengon sur une seule machine, sans radio, avec un réseau simulé scriptable et **rejouable à l'identique**.
**Correspond à la conception :** [`docs/synthese/10-benchmarks-mvp-tests.md`](../../synthese/10-benchmarks-mvp-tests.md) §4.3 ; [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §5.
**Dernière mise à jour :** 2026-09-28
**État :** partiel — harness, réseau simulé, scénarios et job CI `sim` livrés (US-221) ; les nœuds exécutent un relais de démonstration (`Inondation`), pas encore `dengon-core`.

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
    scenario.rs   — scénarios RON : lecture, validation, exécution, attendus
    cli.rs        — logique de la commande dengon-sim (testée)
    main.rs       — point d'entrée, délègue à cli
  scenarios/      — direct, multihop, partition_merge, lossy_mesh (.ron)
  tests/
    conformite_sim.rs — suite de conformité Transport (US-105) sur SimTransport
    scenarios.rs      — scénarios livrés : réussis + déterministes
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
  `dengon-core` (lien de squelette, en attendant le vrai nœud).
- **Externes (crates) :** `serde` (derive) + `ron` 0.12 — lecture des
  scénarios. Pas de `rand` : l'aléa est un SplitMix64 maison, pour que la
  reproductibilité ne dépende pas d'une mise à jour de crate.

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
- **Comportement injecté** (`Comportement`) plutôt que `dengon-core` en dur :
  `sync::routing` (US-209) et la façade `api` (US-301) n'existent pas encore.
  Écart consigné dans `03-ecarts-conception.md`.
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
- `tests/scenarios.rs` — 3 : scénarios livrés réussis, même graine → même
  trace pour chacun, graine différente → empreinte différente sur
  `lossy_mesh` (et pertes réellement tirées).
- Commande : `cargo test -p dengon-sim` → **28 passés** (24 unitaires + 1
  conformité + 3 scénarios), 2026-09-28.

## Limites connues / TODO

- **Pas encore de nœud `dengon-core`** dans la simulation : `Inondation` n'a ni
  TTL, ni signature, ni inventaire. Les scénarios de `synthese/10` §4.3 qui
  dépendent du protocole (`recipient_offline`, `sender_offline`, `flood`,
  `dup_paths`, `tamper`, `key_change`) attendent US-209 / US-301.
- Modèle réseau **sans bande passante, churn ni dérive d'horloge** (prévus
  par §4.3) : latence, gigue, perte et partition seulement — ce que demande
  US-221.
- Couverture : mesurée par le job CI `core` (`cargo-llvm-cov` absent du poste
  de dev).

## Pour l'oral

C'est l'outil qui rend le projet démontrable. Un réseau maillé se comporte bien
avec trois appareils et mal avec cinquante — sauf qu'on n'a pas cinquante
appareils. Le simulateur remplace la radio par de la mémoire, et on peut lui
faire vivre ce qu'on ne saurait pas provoquer à la main : couper le réseau en
deux, perdre 40 % des paquets, puis tout reconnecter. Le point fort : tout est
**rejouable**. Deux exécutions donnent la même trace à l'octet près, et la CI
le vérifie à chaque PR. Un bug trouvé une fois est reproductible à volonté.
