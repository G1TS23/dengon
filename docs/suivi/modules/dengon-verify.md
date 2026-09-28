# Module : `dengon-verify` (`crates/dengon-verify/`)

**Rôle en une phrase :** un petit programme qui répond à une seule question — ce journal d'événements a-t-il été trafiqué ?
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2, [`09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md) §2-3 (décisions A-5 et B-5).
**Dernière mise à jour :** 2026-09-28
**État :** fonctionnel (US-305) ; pas encore appelé par le dashboard (US-310)

## À quoi ça sert

Le dashboard est écrit en Python (FastAPI). Réimplémenter en Python la
vérification d'une chaîne de hachage et de signatures Ed25519, ce serait une
deuxième implémentation de la cryptographie du projet — donc un deuxième
endroit où se tromper. La décision B-5 tranche autrement : le dashboard
**appelle ce binaire Rust en sous-processus**, et celui-ci réutilise
`dengon_core::ledger`.

Il lit un export de journal, rend un verdict (`ok`, `broken`, `fork`, `gap`)
sous forme d'une ligne JSON, et un **code de sortie** que le dashboard peut
tester sans analyser de texte.

## Structure

```
dengon-verify/
  src/
    lib.rs        — arguments, lecture de l'export, vérification, rapport JSON
    main.rs       — appelle `run`, imprime, rend le code de sortie
    tests.rs      — tests unitaires de lib.rs
  tests/
    cli.rs        — tests de bout en bout du vrai binaire (un par verdict)
    fixtures/     — journaux de démonstration ok / broken / fork / gap / signed (.bin)
```

## Usage

```text
dengon-verify [--from-seq N --prev-hash HEX64] [--pubkey HEX64] [FICHIER | -]
```

| Code | Sens | Sortie standard |
|---|---|---|
| `0` | `ok` — chaîne intègre | ligne JSON |
| `1` | `broken` — entrée modifiée, `prev_hash` incohérent, ou signature invalide | ligne JSON |
| `2` | `fork` — deux entrées pour une même position (ou antérieure à l'ancre) | ligne JSON |
| `3` | `gap` — position(s) manquante(s) | ligne JSON |
| `64` | appel invalide (`EX_USAGE`) | vide ; message sur stderr |
| `65` | export illisible, tronqué (`EX_DATAERR`) | vide ; message sur stderr |
| `66` | fichier introuvable (`EX_NOINPUT`) | vide ; message sur stderr |

Exemple : `{"verdict":"gap","entries":4,"first_seq":0,"last_seq":4,"signatures":"unchecked"}`.
`signatures` vaut `verified`, `invalid` ou `unchecked` (pas de `--pubkey`).

## Concepts / types importants

| Type / fonction | Fichier | Ce que ça fait |
|---|---|---|
| `run(args, stdin)` | `src/lib.rs` | Point d'entrée testable : analyse, lit, vérifie, rend un `Report` ou une `Error`. |
| `parse_args` | `src/lib.rs` | Options → `Options { anchor, pubkey, input }` ; `--from-seq` et `--prev-hash` vont ensemble. |
| `parse_export` | `src/lib.rs` | Découpe l'export en `Entry` (`Entry::from_bytes`) ; erreur avec rang et octet fautifs. |
| `verify` | `src/lib.rs` | `ledger::verify_entries`, puis `ledger::verify_signatures` si clé et chaîne intègre. |
| `exit_code` / `Error::exit_code` | `src/lib.rs` | Le contrat avec le dashboard. |
| `ledger::Anchor`, `verify_entries`, `verify_signatures` | `dengon-core/src/ledger.rs` | La règle de vérification elle-même (voir `dengon-core.md`). |

## Flux principal (exemple)

Le dashboard reçoit un lot d'entrées de journal d'un relais (`seq` 120 à 180).
Il a déjà vérifié jusqu'à 119 : il lance
`dengon-verify --from-seq 120 --prev-hash <entry_hash de 119> --pubkey <pub_sign du relais>`
en passant le lot sur l'entrée standard, lit le code de sortie et la ligne
JSON, et met à jour `ledger_state.integrity`.

## Dépendances

- **Internes :** `dengon-core` (`ledger`, `crypto`).
- **Externes (crates) :** aucune (analyse d'arguments à la main).

## Décisions d'implémentation

- **Crate à part, pas une sous-commande de `dengon-node`** : le dashboard doit
  pouvoir déployer un binaire léger sur le VPS sans la pile Bluetooth.
- **La règle de vérification vit dans `dengon-core::ledger`** (ancre et
  signatures comprises), le binaire ne fait qu'entrée / sortie (B-5).
- **Un code de sortie par verdict**, et rien sur la sortie standard en cas
  d'erreur : le dashboard distingue « journal cassé » de « appel raté » sans
  analyser de texte.
- **Signatures vérifiées seulement si la chaîne est intègre.**
- **Fixtures commitées et vérifiées par un test** (`fixtures_a_jour`).

## Tests

- `src/tests.rs` — 9 tests unitaires : arguments (par défaut, complets,
  invalides), lecture d'export (tronqué → erreur localisée), codes et noms
  de verdicts, rapport JSON, signatures nulles rejetées avec une vraie clé,
  clé publique invalide, entrée standard et fichier absent.
- `tests/cli.rs` — 9 tests de bout en bout sur le vrai binaire : **un par
  verdict** (`ok`, `broken`, `fork`, `gap`), signatures (bonne et mauvaise
  clé), tranche ancrée, codes 64 / 65 / 66, fixtures à jour et lues depuis
  un fichier.
- Commande : `cargo test -p dengon-verify` → 18 passés. Couverture
  `src/lib.rs` : 97 % des lignes.

## Limites connues / TODO

- Format d'entrée = export binaire `Entry::to_bytes`, pas le JSON canonique
  de `synthese/09` §11 (voir `03-ecarts-conception.md`).
- Pas de `LOG_ATTEST` (racine + hauteur attestées) : à ajouter quand le
  format existera côté `protocol`.
- Les journaux réels sont encore signés par `NullSigner` (le `Signer`
  Ed25519 n'est pas branché dans `ledger`) : sans `--pubkey`, les
  signatures ne sont pas vérifiées, et la sortie le dit.

## Pour l'oral

Bon exemple de décision d'architecture assumée. Le tableau de bord est en
Python parce que c'est rapide à écrire ; mais la vérification cryptographique
reste en Rust, parce qu'on ne veut pas écrire deux fois le même code de
sécurité. Le Python lance le programme Rust et lit sa réponse : « intact »,
« cassé », « fourche » ou « trou ». Démo : `dengon-verify
tests/fixtures/broken.bin` → `broken`, code de sortie 1.
