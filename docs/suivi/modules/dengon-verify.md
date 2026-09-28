# Module : `dengon-verify` (`crates/dengon-verify/`)

**Rôle en une phrase :** un petit programme qui répond à une seule question — ce journal d'événements a-t-il été trafiqué ?
**Correspond à la conception :** [`docs/synthese/04-architecture.md`](../../synthese/04-architecture.md) §2, [`09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md) (décisions A-5 et B-5).
**Dernière mise à jour :** 2026-09-26
**État :** esquisse ; `Verdict` branché sur `dengon_core::ledger` (US-206)

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
    main.rs       — réexporte dengon_core::ledger::Verdict, main affiche sa version
```

## Concepts / types importants

| Type / fonction | Fichier:ligne | Ce que ça fait |
|---|---|---|
| `pub use dengon_core::ledger::Verdict` | `src/main.rs` | Les quatre réponses possibles (`Ok`/`Broken`/`Fork`/`Gap`), et c'est tout le contrat d'interface avec le dashboard. Depuis l'US-206, ce n'est plus une copie locale : c'est littéralement le type que `ledger::verify_chain` renvoie. |

## Flux principal (exemple)

Visé : le dashboard reçoit un lot d'événements d'un relais, le stocke, puis
lance `dengon-verify` dessus. Si la réponse est `Broken` ou `Fork`, l'interface
signale que ce relais a menti.

Actuel : le binaire affiche sa version.

## Dépendances

- **Internes :** `dengon-core` — utilise réellement `ledger::Verdict` depuis
  l'US-206 (avant : copie locale du même nom).
- **Externes (crates) :** aucune.

## Décisions d'implémentation

- **Crate à part, pas une sous-commande de `dengon-node`** : le dashboard doit
  pouvoir déployer un binaire de quelques mégaoctets sur le VPS sans embarquer
  la pile Bluetooth.
- Les quatre variantes de `Verdict` sont écrites **avant** l'implémentation :
  c'est le contrat que le dashboard peut déjà bouchonner sans attendre P1.13.
- **`Verdict` réexporté depuis `dengon_core::ledger`** (US-206) plutôt que
  redéclaré ici : la copie locale précédente (même nom, même variantes)
  aurait fini par diverger silencieusement de celle de `ledger` — le point
  relevé ci-dessous (« crate binaire, non importable ») ne se pose donc plus
  pour `Verdict` lui-même, seulement pour un futur code spécifique à
  `dengon-verify` qui voudrait être testé depuis l'extérieur.

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
