# Git hooks du projet

Hooks versionnés, partagés via `core.hooksPath`.

## Activation (à faire une fois après le clone)

```sh
git config core.hooksPath .githooks
```

## Hooks disponibles

| Hook         | Rôle                                                              |
|--------------|-----------------------------------------------------------------|
| `commit-msg` | Vérifie que le message suit la convention [Conventional Commits](https://www.conventionalcommits.org/) |

### Format d'un message de commit

```
<type>(<scope facultatif>)<! si breaking change>: <description>
```

Types autorisés : `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.

Exemples :

```
feat: ajoute la page de connexion
fix(auth): gère le token expiré
docs: complète le README
refactor(core)!: renomme la configuration
```

Pour contourner ponctuellement (déconseillé) : `git commit --no-verify`.
