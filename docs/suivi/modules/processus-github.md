# Module : `process` — outillage GitHub (`.github/`)

**Rôle en une phrase :** rendre la Definition of Ready et la revue croisée
**mécaniques** — imposées par l'outil — au lieu de déclaratives.
**Correspond à la conception :** [`docs/olivier/proposition-organisation-github.md`](../../olivier/proposition-organisation-github.md)
§4.3 (contenu de `.github/`), §5.3 (labels), §6 (DoR), §7 (DoD), §10.3 (revue croisée).
**Dernière mise à jour :** 2026-09-09
**État :** partiel — les quatre livrables versionnés existent ; **la protection
de `main` n'est pas activée** (droit admin manquant, voir *Limites connues*).

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
    core.yml         qualité du workspace Rust (US-104, PR #57)
    labels.yml       synchro des labels, manuelle et en simulation par défaut
```

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
2. **Un seul check requis pour l'instant.** `sim`, `audit` et `cross-vectors`
   sont nommés par la DoD §7.1 point 3 mais **n'existent pas encore** : les
   déclarer ici figerait le dépôt. Écart consigné dans
   [`03-ecarts-conception.md`](../03-ecarts-conception.md). Élargir ensuite,
   un check à la fois, à mesure que chaque workflow arrive — en **répétant la
   liste complète**, l'API remplace le tableau, elle ne l'ajoute pas :

   ```bash
   gh api -X PATCH repos/G1TS23/dengon/branches/main/protection/required_status_checks \
     -F 'contexts[]=core' -F 'contexts[]=sim'
   ```

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

## Vérification

| Quoi | Commande | Résultat |
| --- | --- | --- |
| YAML valide (6 fichiers) | `python3 -c "import yaml,glob;[yaml.safe_load(open(f)) for f in glob.glob('.github/**/*.yml',recursive=True)]"` | OK |
| Schéma des formulaires | script jetable : `id` unique, `options` présentes sur chaque `dropdown`/`checkboxes`, `required` bien sous `validations` | OK |
| `labels.yml` ≡ GitHub | diff avec `gh label list --limit 100 --json name,color,description` | **0 écart** sur les 32 labels déclarés |
| `CODEOWNERS` | `gh api repos/G1TS23/dengon/codeowners/errors?ref=chore/US-113-github-setup` | **`{"errors":[]}`** — les 6 règles sont valides et les 3 comptes reconnus avec droit d'écriture |

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

- **La protection de `main` n'est pas appliquée.** C'est le 5ᵉ critère
  d'acceptation de l'issue #13 et il manque le droit admin. L'issue reste
  ouverte tant que les commandes ci-dessus n'ont pas été lancées par `G1TS23`.
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
- `sim.yml`, `audit.yml`, `cross-vectors.yml`, `android.yml`, `firmware.yml`,
  `dashboard.yml` et `deploy-vps.yml` (§4.3) restent à écrire — hors périmètre
  de l'US-113.

## Pour l'oral

Le point intéressant n'est pas le contenu des fichiers, c'est **la bascule du
déclaratif au mécanique**. On avait écrit « toute PR est relue par quelqu'un
d'une autre area » dans un document d'organisation ; personne n'était obligé de
le faire. Trois fichiers plus tard, GitHub refuse le merge — et comme il
interdit à l'auteur d'approuver sa propre PR, la règle s'applique toute seule,
sans que personne ait à la rappeler.

Le piège qu'on a évité mérite d'être raconté : la DoD nomme quatre checks
obligatoires (`core`, `sim`, `audit`, `cross-vectors`), dont trois n'existent
pas encore. Les déclarer requis « pour être conforme » aurait bloqué **toutes**
les PR du dépôt sur un check que rien n'aurait jamais rapporté. Être conforme à
la lettre d'un document peut geler un projet ; on a préféré consigner l'écart.
