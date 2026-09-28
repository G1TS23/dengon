# Module : `process` — outillage GitHub (`.github/`)

**Rôle en une phrase :** rendre la Definition of Ready et la revue croisée
**mécaniques** — imposées par l'outil — au lieu de déclaratives.
**Correspond à la conception :** [`docs/olivier/proposition-organisation-github.md`](../../olivier/proposition-organisation-github.md)
§4.3 (contenu de `.github/`), §5.3 (labels), §6 (DoR), §7 (DoD), §10.3 (revue croisée).
**Dernière mise à jour :** 2026-09-28 (US-222 — jobs `audit` et `cross-vectors`)
**État :** les **quatre workflows requis par la DoD §7.1 point 3 existent
désormais** — `core` (US-104), `sim` (US-221), `audit` et `cross-vectors`
(US-222). La protection de `main` n'en exige toujours **qu'un seul** (`core`,
posé par `G1TS23` le 09/09 à 15:18) : l'élargissement est une commande
d'admin, à jouer **après** le merge de l'US-222 — voir *Protection de `main`*.

Cette fiche tient aussi lieu de **note d'onboarding de l'area `process`**
(§10.3 point 3) : comment le dépôt est réglé, et les trois pièges.

## À quoi ça sert

La DoD §7.1 point 4 exige qu'une PR soit relue « par une personne d'une autre
`area:` ». Tant que c'est seulement écrit dans un document, rien n'empêche
techniquement de merger sa propre PR sans relecture — et sur trois semaines,
c'est exactement ce qui finit par arriver un soir de rush. Le contenu de
`.github/` déplace ces règles du document vers l'outil : les formulaires
d'issue refusent une issue sans critères vérifiables, `CODEOWNERS` désigne
automatiquement un relecteur d'une autre area, et la protection de branche
refuse le merge sans son approbation.

## Structure

```
.github/
  ISSUE_TEMPLATE/
    user-story.yml   formulaire d'US : 8 champs + DoR en 8 cases
    spike.yml        question fermée, timebox, critère d'arrêt
    bug.yml          observé / attendu / repro / où
    config.yml       coupe l'issue vierge, renvoie vers le board et la spec
  PULL_REQUEST_TEMPLATE.md   la DoD §7.1 en checklist + §7.2 en repli
  CODEOWNERS                 qui relit quoi (revue croisée)
  labels.yml                 les 32 labels, versionnés
  workflows/
    core.yml           qualité du workspace Rust (US-104, PR #57)
    sim.yml            scénarios dengon-sim à graine fixe, joués 2× (US-221)
    audit.yml          cargo audit + cargo deny, PR + cron quotidien (US-222)
    cross-vectors.yml  conformité core ↔ firmware ↔ dashboard (US-222)
    contracts.yml      schémas, fixtures golden, vecteurs de trame (US-107/222)
    dashboard.yml      ruff + pytest de dashboard/api (US-110)
    firmware.yml       idf.py build via image Docker épinglée (US-114)
    labels.yml         synchro des labels, manuelle et en simulation par défaut
```

Des huit workflows de `proposition-organisation-github.md` §4.3, il manque
`android.yml` et `deploy-vps.yml`.

## Fichiers importants

| Fichier | Ce que ça fait |
| --- | --- |
| `ISSUE_TEMPLATE/user-story.yml` | Formulaire GitHub (*issue form*). Les champs `required` — dépendances, référence documentaire, stratégie de test — ne peuvent pas rester vides : c'est la DoR n°3, 4 et 7 rendues obligatoires à la saisie. |
| `ISSUE_TEMPLATE/config.yml` | `blank_issues_enabled: false`. Sans ça, « Open a blank issue » contourne tout le formulaire. |
| `PULL_REQUEST_TEMPLATE.md` | Les 8 points de la DoD §7.1 en cases à cocher, la DoD §7.2 par type d'US en repli, et un champ « ce que la revue doit regarder en priorité ». |
| `CODEOWNERS` | Un relecteur demandé automatiquement selon les chemins touchés. |
| `labels.yml` | État attendu des 32 labels. Décrit l'existant : un sync ne modifie rien. |
| `workflows/labels.yml` | `workflow_dispatch` seul, `dry_run: true` par défaut. |

## Flux principal

1. **Ouverture d'une issue** → `/issues/new/choose` propose trois formulaires.
   L'issue vierge est désactivée. Le label `type:*` est posé automatiquement.
2. **Ouverture d'une PR** → `PULL_REQUEST_TEMPLATE.md` pré-remplit le corps.
   L'auteur coche les 8 points de la DoD en mettant la preuve à côté.
3. **CODEOWNERS** lit les chemins du diff et **demande la revue** de la
   personne prévue pour cette area. L'auteur ne peut pas se désigner lui-même.
4. **La protection de `main`** refuse le merge tant qu'il manque l'approbation
   ou un check requis. *(Étape non active aujourd'hui — voir plus bas.)*

## Décisions d'implémentation

- **Formulaires YAML (*issue forms*), pas templates Markdown.** Un template
  Markdown se soumet vide en une seconde ; un formulaire refuse la soumission
  si un champ `required` est vide. C'est toute la différence entre une DoR
  affichée et une DoR appliquée.
- **Les 8 cases de la DoR sont, elles, non bloquantes.** Une issue doit pouvoir
  naître incomplète — c'est l'**entrée en sprint** qui exige les 8 cochées
  (§6). Les rendre obligatoires à l'ouverture pousserait à cocher sans lire.
- **`CODEOWNERS` liste deux comptes par chemin, jamais un.** Un seul nom
  bloquerait le dépôt dès que cette personne est absente, et deux noms
  n'affaiblissent rien puisque **l'auteur ne peut pas s'auto-approuver** : sur
  `/crates/`, une PR de Paul ne peut être validée que par Tanguy. La paire
  suit la règle de doublure de §13 — le relecteur est **celui qui consommera
  l'artefact au sprint suivant**, pour que la revue lui serve.
- **La ligne `*` est en tête du fichier, pas en fin.** Dans `CODEOWNERS`, la
  **dernière** ligne qui correspond gagne : placée en bas, elle écraserait
  toutes les règles par chemin.
- **`labels.yml` recopie l'existant, il ne le corrige pas.** Les couleurs et
  descriptions viennent de `gh label list`. Objectif : que le premier sync
  affiche zéro changement, donc que le fichier soit une photo vérifiable et
  non une intention.
- **Synchro des labels manuelle et en simulation par défaut.** Les labels sont
  posés sur 55 issues et servent de filtres au board. Un
  `delete-other-labels` déclenché automatiquement par un push les effacerait
  sans revue possible du résultat. Il faut trois gestes délibérés pour
  détruire : lancer le workflow, décocher `dry_run`, cocher `supprimer`.
- **Actions tierces épinglées sur un SHA de commit complet**, jamais sur un
  tag — un tag est mutable et son propriétaire peut le repointer vers
  n'importe quel code, qui s'exécuterait dans notre CI. Même règle que
  `core.yml`.
- **Le filtrage par chemin est fait DANS le job, jamais via
  `on.pull_request.paths`.** C'est la règle transverse à tous les workflows du
  dépôt, et la plus contre-intuitive. Avec un filtre au niveau du déclencheur,
  GitHub ne démarre pas le workflow sur une PR qui ne touche que `docs/` :
  **aucun check run n'est créé**, et la PR reste indéfiniment sur « Expected —
  Waiting for status to be reported » dès que le check est requis. Or la
  plupart des PR de ce projet ne touchent que `docs/`. Le patron, identique
  partout : une étape `dorny/paths-filter` avec `id: filtre`, une étape
  « Aucun changement X » qui écrit dans `$GITHUB_STEP_SUMMARY`, et
  `if: steps.filtre.outputs.X == 'true'` sur chaque étape réelle. Le check
  est **toujours rapporté**, vert, même quand il n'a rien fait.
- **Le nom du check requis est le nom du JOB, pas du workflow.** D'où : pas de
  `strategy.matrix` sur un job requis (le check deviendrait
  « core (ubuntu-latest) »), pas de workflow réutilisable (« appelant /
  appelé »), et on ne renomme pas ces quatre jobs.
- **`audit` est bloquant dès le premier jour**, sans `continue-on-error`. Un
  check requis qui ne rougit jamais n'est pas un check, c'est un rapport que
  personne ne lit. La soupape est `[advisories].ignore` de `deny.toml`, qui
  demande un identifiant RUSTSEC nommé, daté et justifié — deux entrées
  aujourd'hui, toutes deux tenues par l'épinglage `uniffi =0.28.3`.
- **`audit` tourne en entier sur le cron**, filtre de chemins court-circuité :
  une vulnérabilité paraît sans que le dépôt bouge, et personne n'ouvre de PR
  ce jour-là. `dorny/paths-filter` n'a d'ailleurs aucune base de comparaison
  sur `schedule`, donc l'étape elle-même est sautée.
- **Une seule liste d'exceptions RUSTSEC.** `cargo audit` ne lit pas
  `deny.toml` (son fichier serait `.cargo/audit.toml`) : le workflow dérive
  ses `--ignore` de `deny.toml` par un `grep`. Deux listes tenues à la main
  dériveraient l'une de l'autre au premier oubli.

## Protection de `main` — commandes à lancer (droit admin requis)

`gh api repos/G1TS23/dengon/collaborators` : **seul `G1TS23` est admin**. Ces
commandes ne peuvent donc pas être lancées depuis un poste authentifié en
`POWLAIR` ou `OswinFreyr`, quel que soit le scope du jeton.

```bash
gh api -X PUT repos/G1TS23/dengon/branches/main/protection --input - <<'JSON'
{
  "required_status_checks": { "strict": true, "contexts": ["core"] },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "required_approving_review_count": 1,
    "require_code_owner_reviews": true,
    "dismiss_stale_reviews": true
  },
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false
}
JSON

# Squash-only + suppression automatique des branches : réglages du DÉPÔT,
# pas de la branche — ils ne sont pas dans le JSON ci-dessus.
gh api -X PATCH repos/G1TS23/dengon \
  -F allow_squash_merge=true \
  -F allow_merge_commit=false \
  -F allow_rebase_merge=false \
  -F delete_branch_on_merge=true
```

Quatre points à ne pas survoler :

1. **N'exécuter qu'après le merge de la PR #57.** C'est elle qui crée le
   workflow `core`. Exiger un check qui n'a jamais tourné met toutes les PR en
   « Expected — Waiting for status to be reported », indéfiniment.
   **C'est arrivé** : la protection a été posée le 09/09 à 15:18, avant le
   merge de #57. Effet immédiat — #56 et #58 sont `BLOCKED` sur un `core` que
   rien ne peut rapporter, tandis que #57 est verte, son *head* portant
   `core.yml`. Sortie : merger #57 d'abord, puis fusionner `main` dans les
   autres branches pour que leur *merge ref* contienne le workflow.
2. **Un seul check requis aujourd'hui — à élargir à quatre après le merge de
   l'US-222.** `sim` (US-221), `audit` et `cross-vectors` (US-222) existent
   désormais ; la protection, elle, n'exige toujours que `core`.

   **À jouer APRÈS le merge, pas avant.** Tant que les trois workflows ne sont
   pas sur `main`, les exiger reproduirait exactement l'incident du 09/09 :
   toutes les PR bloquées sur un check que rien ne rapporte. Et **répéter la
   liste COMPLÈTE** — l'API `PATCH` *remplace* le tableau `contexts`, elle ne
   l'enrichit pas :

   ```bash
   gh api -X PATCH repos/G1TS23/dengon/branches/main/protection/required_status_checks \
     -f strict=true \
     -f 'contexts[]=core' -f 'contexts[]=sim' \
     -f 'contexts[]=audit' -f 'contexts[]=cross-vectors'
   ```

   Puis vérifier, plutôt que de supposer :

   ```bash
   gh api repos/G1TS23/dengon/branches/main/protection/required_status_checks --jq .contexts
   ```

   `firmware`, `contracts` et `dashboard` ne sont **pas** dans la liste : la
   DoD §7.1 point 3 nomme quatre checks, ces trois-là rapportent un statut
   sans bloquer. À rediscuter en équipe, pas à ajouter en passant.

   **Date d'exécution :** _(à compléter par la personne qui la joue —
   `G1TS23` est le seul compte admin)_

3. **`require_code_owner_reviews: true` est la seule ligne qui compte** pour la
   revue croisée. Sans elle, `CODEOWNERS` ne fait que suggérer un relecteur, et
   la DoD §7.1 point 4 reste déclarative.
4. **`enforce_admins: false` est délibéré.** À trois, si le réglage se révèle
   trop strict, `true` empêcherait même l'admin de réparer `main` — il faut
   alors désactiver la protection pour la corriger. À repasser à `true` quand
   la CI est stabilisée.

Vérifier ensuite l'état réel, plutôt que de le supposer :

```bash
gh api repos/G1TS23/dengon/branches/main/protection --jq \
  '{checks: .required_status_checks.contexts,
    approbations: .required_pull_request_reviews.required_approving_review_count,
    code_owners: .required_pull_request_reviews.require_code_owner_reviews,
    lineaire: .required_linear_history.enabled}'
```

## `cross-vectors` — ce que le job prouve réellement

C'est le job qui justifie le monorepo (`proposition-organisation-github.md`
§4.1 : « en multi-dépôts, il faut publier/pinner des artefacts à chaque
changement : intenable en 3 semaines »). Les autres jobs prouvent que chaque
composant marche ; celui-ci prouve qu'ils **lisent le format de la même
façon**.

Sa valeur tient à une condition : les implémentations doivent être écrites
**séparément** et lire le **même fichier**. Si l'une était générée depuis
l'autre, les comparer ne dirait rien.

| Vecteurs | Lecteur | Langage / configuration |
| --- | --- | --- |
| `contracts/packet/vectors_v0.json` | `crates/dengon-core/tests/protocol_vectors.rs` | Rust, `std` |
| `contracts/packet/vectors_v0.json` | `crates/dengon-conformance/tests/packet_vectors_nostd.rs` | Rust, **sans `std`** — proxy firmware |
| `contracts/packet/vectors_v0.json` | `contracts/tools/validate_packets.py` | Python, décodeur indépendant |
| `contracts/packet/{crypto,identity}_v0.json` | `crates/dengon-core/tests/{crypto,identity}_vectors.rs` | Rust, `std` |
| `contracts/events/fixtures/*.json` | `crates/dengon-core/tests/event_fixtures.rs` | Rust — canonique octet à octet, `batch_id`, signatures |
| `contracts/events/fixtures/*.json` | `contracts/tools/validate.py` | Python, côté contrat |
| `contracts/events/fixtures/*.json` | `dashboard/api/tests/test_cross_vectors.py` | Python, pipeline dashboard réel |

Trois points valent d'être connus.

- **Le décodeur Python est piloté par les données, pas par une table
  recopiée.** `vectors_v0.json` expose `header_layout_be`, `flag_bits` et
  `type_names` ; `validate_packets.py` ne connaît que ça. Un test Rust
  (`type_names_des_vecteurs_correspond_a_packet_type`) vérifie dans les deux
  sens que cette table correspond à l'enum `PacketType` — sans lui, renommer
  un type côté Rust laisserait le Python lire l'ancien nom, et les deux
  resteraient verts chacun de son côté.
- **`event_fixtures.rs` lit les vrais fichiers.** Les trois tests golden de
  `src/observability/mod.rs` comparent à des octets **écrits en dur** : ils
  documentent le format mais ne relient rien, modifier une fixture ne les
  fait pas broncher. Le nouveau test lit les 20 fichiers depuis le disque,
  et vérifie en plus que le **Rust accepte les signatures Ed25519 produites
  par le Python**.
- **Chaque patte a son test négatif.** Sans eux, rien ne prouve que le job
  détecte quoi que ce soit — voir le tableau de vérification ci-dessous.

## Vérification

| Quoi | Commande | Résultat |
| --- | --- | --- |
| YAML valide (6 fichiers) | `python3 -c "import yaml,glob;[yaml.safe_load(open(f)) for f in glob.glob('.github/**/*.yml',recursive=True)]"` | OK |
| Schéma des formulaires | script jetable : `id` unique, `options` présentes sur chaque `dropdown`/`checkboxes`, `required` bien sous `validations` | OK |
| `labels.yml` ≡ GitHub | diff avec `gh label list --limit 100 --json name,color,description` | **0 écart** sur les 32 labels déclarés |
| `CODEOWNERS` | `gh api repos/G1TS23/dengon/codeowners/errors?ref=chore/US-113-github-setup` | **`{"errors":[]}`** — les 6 règles sont valides et les 3 comptes reconnus avec droit d'écriture |

### US-222 — ce qui a été réellement exécuté (28/09)

| Quoi | Commande | Résultat |
| --- | --- | --- |
| Workspace Rust | `cargo fmt --all -- --check` puis `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | OK, 0 avertissement |
| Tests | `cargo test --workspace --all-features --locked` | **385 passed**, 2 ignored |
| Patte `no_std` | `cargo test -p dengon-conformance` | 3 passed |
| Isolation `no_std` | `cargo tree -p dengon-conformance -e features \| grep -c rusqlite` | **0** (contre **6** pour `dengon-ble`) |
| Licences / sources / avis | `cargo deny --all-features check` | advisories ok, bans ok, licenses ok, sources ok |
| Avis RUSTSEC | `cargo audit --deny warnings --file Cargo.lock --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2025-0141` | exit 0 |
| … et sans les `--ignore` | `cargo audit --deny warnings --file Cargo.lock` | **exit 1** — prouve que les exceptions servent |
| Contrat, trame | `cd contracts && uv run python tools/validate_packets.py` | 8 accept + 5 reject |
| Contrat, événements | `cd contracts && uv run python tools/validate.py` | 20 fixtures, 28 noms couverts |
| Dashboard | `cd dashboard/api && uv run pytest` | **85 passed** |

**Tests négatifs**, joués à la main puis annulés — c'est la seule preuve que
les jobs mordent :

| Ce qu'on casse | Ce qui doit rougir | Constaté |
| --- | --- | --- |
| 1 octet dans `contracts/packet/vectors_v0.json` | Rust `std`, Rust `no_std` **et** Python | les trois échouent — ils lisent bien le même fichier |
| `type_names["2"]` renommé | le test Rust de correspondance | échec |
| 1 champ dans `contracts/events/fixtures/01-pkt-seen.json` | `event_fixtures.rs`, pytest dashboard, `validate.py` | les trois échouent |
| `"Unicode-3.0"` retiré de `deny.toml` | `cargo deny check licenses` | exit 4 |

**Pas vérifié :** que les workflows tournent réellement sur GitHub — la preuve
demandée par la DoR n°7 de l'issue (« les workflows sont eux-mêmes la preuve :
verts sur une PR de test ») se fait sur la PR. Et l'état réel de la protection
de `main` : l'appel renvoie 404 depuis un compte non admin, ce qui ne prouve
rien (voir juste en dessous).

> **Le 404 de `branches/main/protection` ne prouve RIEN.** Cet endpoint renvoie
> 404 aux comptes **non admin**, que la protection existe ou non. Plusieurs
> affirmations « → aucune protection » ont été écrites sur cette base : le
> raisonnement était creux, même si le fait était vrai à ce moment-là. Le
> contrôle fiable depuis un compte sans droit admin est le `mergeStateStatus`
> d'une PR — `CLEAN` contre `BLOCKED` :
> `gh pr view <n> --json mergeStateStatus,statusCheckRollup`.

> **CODEOWNERS se lit depuis la branche de BASE, pas depuis celle de la PR.**
> Constaté sur la PR #58 : `gh pr view 58 --json reviewRequests` renvoyait `[]`
> alors que le fichier existait sur la branche. Tant qu'il n'est pas sur `main`,
> il ne gouverne rien. La PR qui met en place la revue croisée est donc la seule
> qui n'en bénéficie pas — le relecteur a dû être demandé à la main.

### Checks déjà présents sur les PR

Deux applications GitHub sont installées sur le dépôt et rapportent un statut
sur chaque PR, indépendamment de nos workflows : **GitGuardian Security Checks**
(fuite de secrets) et **SonarCloud Code Analysis**. Les deux étaient vertes sur
la PR #58. Elles sont donc candidates à devenir des checks requis, en plus de
`core` — décision à prendre en équipe, elles ne figurent pas dans la DoD §7.1.

## Limites connues / TODO

- **La protection de `main` est active depuis le 09/09 15:18**, posée par
  `G1TS23` : « Review required » + `core` en check requis + invalidation des
  approbations à chaque push. Le 5ᵉ critère d'acceptation est donc atteint —
  mais elle a été posée **avant** le merge de #57, donc `core` n'a aucun
  workflow pour le rapporter et **#56 et #58 sont bloquées**. Ce n'est pas un
  mauvais réglage, c'est un problème d'ordre : merger #57 en premier le résout.
- **La preuve par l'usage exigée par la DoR n°7 de l'issue** — « une PR de test
  est bloquée tant que les checks ne sont pas verts » — n'a pas pu être faite,
  pour la même raison. Mode opératoire : après activation, ouvrir une PR
  triviale, constater le bouton de merge grisé avec « Review required », puis
  la fermer.
- **Les formulaires ne sont pas encore visibles.** GitHub ne lit
  `ISSUE_TEMPLATE/` que depuis la branche par défaut : rien ne change sur
  `/issues/new/choose` avant le merge.
- **Le workflow `labels` n'est pas encore lançable**, pour la même raison —
  `workflow_dispatch` n'apparaît que si le fichier est sur `main`.
- Les 9 labels par défaut de GitHub non utilisés (`enhancement`, `wontfix`…)
  ne sont pas dans `labels.yml` ; ils survivent tant que `supprimer` reste à
  `false`.
- ~~`sim.yml`, `audit.yml`, `cross-vectors.yml`, `firmware.yml`,
  `dashboard.yml` restent à écrire~~ — faits (US-221, US-222, US-114,
  US-110). **`android.yml` et `deploy-vps.yml` (§4.3) restent à écrire.**
- **Les 4 checks requis ne sont pas encore posés sur `main`** : c'est le
  dernier critère d'acceptation de l'US-222, et il ne peut être rempli
  qu'après le merge, par un compte admin. Voir la commande plus haut.
- **La patte « firmware » de `cross-vectors` est un proxy** : le firmware ne
  lit aucun vecteur (le pont `dengon_core_ffi` est l'US-307). Ce qui tourne,
  c'est `crates/dengon-conformance/`, le même décodeur compilé **sans
  `std`** — la configuration que l'ESP32 embarquera. Écart consigné dans
  [`03-ecarts-conception.md`](../03-ecarts-conception.md).
- **Pas de SBOM** dans le job `audit`, alors que `synthese/10` §4.7 en nomme
  un. Il n'est dans aucun critère d'acceptation de l'US-222 ; à ouvrir en
  issue de suite plutôt qu'à livrer un artefact que personne ne consomme.

## Pour l'oral

Le point intéressant n'est pas le contenu des fichiers, c'est **la bascule du
déclaratif au mécanique**. On avait écrit « toute PR est relue par quelqu'un
d'une autre area » dans un document d'organisation ; personne n'était obligé de
le faire. Trois fichiers plus tard, GitHub refuse le merge — et comme il
interdit à l'auteur d'approuver sa propre PR, la règle s'applique toute seule,
sans que personne ait à la rappeler.

Le piège qu'on a évité mérite d'être raconté : la DoD nomme quatre checks
obligatoires (`core`, `sim`, `audit`, `cross-vectors`), dont trois n'existaient
pas encore en septembre. Les déclarer requis « pour être conforme » aurait
bloqué **toutes** les PR du dépôt sur un check que rien n'aurait jamais
rapporté. Être conforme à la lettre d'un document peut geler un projet ; on a
préféré consigner l'écart, puis écrire les workflows manquants (US-221,
US-222) et n'élargir la protection qu'une fois qu'ils étaient sur `main`.

Le deuxième point, plus technique, c'est **`cross-vectors` et ce qu'un job de
conformité peut honnêtement prouver**. L'intitulé dit « core ↔ firmware ↔
dashboard ». Or le firmware ne lit aucun vecteur : il ne contient que du BLE,
le pont vers le cœur du protocole est une US du sprint 3. On aurait pu écrire
un job qui coche la case sans rien comparer. On a préféré une troisième
lecture réelle — le **même** décodeur compilé sans `std`, la configuration que
l'ESP32 embarquera — et écrire noir sur blanc que c'est un proxy, avec la date
à laquelle il tombera.

Le détour instructif est là : le réflexe évident,
`cargo test -p dengon-core --no-default-features`, **ne compile pas en
`no_std`**. Cargo unifie les features d'un même graphe de build, et une
dépendance *de test* (`dengon-ble`) réactive `std` sans rien dire. Le test
serait passé vert en n'ayant rien prouvé — le pire mode d'échec possible pour
un test de conformité. C'est `cargo tree -e features` qui l'a montré, et c'est
ce qui a dicté la forme finale : une crate séparée, une dépendance écrite en
chemin (l'héritage de workspace ignore `default-features`), et le garde-fou
placé **hors** du code, dans une étape de CI qui inspecte la résolution isolée.
