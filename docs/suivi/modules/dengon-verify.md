# Module : `dengon-verify` (`crates/dengon-verify/`)

**Rôle en une phrase :** un petit programme qui répond à une seule question — ce journal d'événements a-t-il été trafiqué ?
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2, [`09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md) (décisions A-5 et B-5).
**Dernière mise à jour :** 2026-09-09
**État :** esquisse

## À quoi ça sert

Le dashboard est écrit en Python (FastAPI). Réimplémenter en Python la
vérification d'une chaîne de hachage et de signatures Ed25519, ce serait une
deuxième implémentation de la cryptographie du projet — donc un deuxième
endroit où se tromper. La décision B-5 tranche autrement : le dashboard
**appelle ce binaire Rust en sous-processus**, et celui-ci réutilise le code de
`dengon-core`.

Il lit un export de journal accompagné de son `LOG_ATTEST` et rend un verdict.

## Structure

```
dengon-verify/
  src/
    main.rs       — l'enum Verdict + un main qui affiche sa version
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `enum Verdict` |  `src/main.rs:18` | Les quatre réponses possibles, et c'est tout le contrat d'interface avec le dashboard. |
| `Verdict::Ok` | `src/main.rs:20` | Chaîne cohérente, signatures valides. |
| `Verdict::Broken` | `src/main.rs:22` | Une entrée a été modifiée, ou une signature est invalide. |
| `Verdict::Fork` | `src/main.rs:24` | Deux entrées concurrentes revendiquent la même position. |
| `Verdict::Gap` | `src/main.rs:26` | Il manque des positions dans la chaîne. |

## Flux principal (exemple)

Visé : le dashboard reçoit un lot d'événements d'un relais, le stocke, puis
lance `dengon-verify` dessus. Si la réponse est `Broken` ou `Fork`, l'interface
signale que ce relais a menti.

Actuel : le binaire affiche sa version.

## Dépendances

- **Internes :** `dengon-core` (utilisera `ledger::verify_chain`).
- **Externes (crates) :** aucune.

## Décisions d'implémentation

- **Crate à part, pas une sous-commande de `dengon-node`** : le dashboard doit
  pouvoir déployer un binaire de quelques mégaoctets sur le VPS sans embarquer
  la pile Bluetooth.
- Les quatre variantes de `Verdict` sont écrites **avant** l'implémentation :
  c'est le contrat que le dashboard peut déjà bouchonner sans attendre P1.13.
- Limite : `Verdict` est déclaré dans un crate **binaire**, donc non importable
  depuis l'extérieur. Si le dashboard ou des tests ont besoin du type, il faudra
  scinder en `src/lib.rs` + `src/main.rs`. Pour l'instant l'échange se fait par
  la sortie du processus, pas par l'API Rust.

## Tests

- `src/main.rs`, module `tests` : 1 test (les quatre verdicts sont distincts).
- Commande : `cargo test -p dengon-verify` → 1 passé, 0 échec.

## Limites connues / TODO

- Ne lit aucun fichier, ne vérifie aucune chaîne, ne rend aucun verdict réel.
- Le format d'entrée (export de journal + `LOG_ATTEST`) n'est pas figé.

## Pour l'oral

Bon exemple de décision d'architecture assumée. Le tableau de bord est en
Python parce que c'est rapide à écrire ; mais la vérification cryptographique,
elle, reste en Rust, parce qu'on ne veut pas écrire deux fois le même code de
sécurité. Le Python appelle simplement le programme Rust et lit sa réponse :
« intact », « cassé », « fourche » ou « trou ».
