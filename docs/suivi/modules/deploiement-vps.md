# Module : `deploiement-vps` (`dashboard/deploy/`, `.github/workflows/deploy-vps.yml`)

**Rôle en une phrase :** faire tourner `dashboard/api` derrière un reverse-proxy
TLS sur un VPS réel, joignable en HTTPS depuis l'extérieur, avec un moyen de
repartir d'une base de démo propre en une commande.
**Correspond à la conception :** [`docs/synthese/09-dashboard-et-donnees.md`](../../synthese/09-dashboard-et-donnees.md)
§7 (déploiement), [`docs/olivier/dashboard.md`](../../olivier/dashboard.md).
**Dernière mise à jour :** 2026-09-28
**État :** fait (US-224) — déployé et vérifié sur le VPS réel du groupe.

## À quoi ça sert

Sans ce module, `dashboard/api` ne tourne qu'en local (`uvicorn` direct,
`TestClient` dans les tests). US-224 le met derrière un reverse-proxy TLS sur
un serveur réellement accessible depuis Internet, avec un script pour
réinitialiser la base entre deux sessions de démo — le strict nécessaire pour
qu'une démo en direct (soutenance) fonctionne sans dépendre d'un poste
particulier.

**Le VPS est PARTAGÉ avec d'autres groupes du cours** (pas dédié à dengon) :
`docker ps` y montre des conteneurs d'autres projets, et les ports 80/443 sont
déjà occupés par un processus **root** que nous ne contrôlons pas (pas
d'accès `sudo`). Ce constat, fait en investiguant le VPS au moment de l'US-224,
a déterminé presque toutes les décisions ci-dessous — voir
`03-ecarts-conception.md` pour le détail de chaque écart face à la conception
« reverse-proxy TLS standard » implicite de l'US.

## Structure

```
dashboard/
  api/
    Dockerfile              — build l'image API ; CONTEXTE DE BUILD = racine du
                               dépôt (pas dashboard/api/), voir Dépendances
  deploy/
    docker-compose.yml      — services api + caddy + web, volumes nommés fixes
    Caddyfile                — reverse-proxy TLS devant api (voir Décisions)
    Caddyfile.web             — sert dashboard/web/ en statique, TLS séparé
                                 (issue #125, voir Décisions)
    .env.example             — modèle non secret, copié en .env sur le VPS
    purge-demo.sh             — reset de la base de démo en une commande
.github/workflows/
  deploy-vps.yml             — déploiement manuel (workflow_dispatch),
                               environment vps-prod
.dockerignore (racine)       — exclut .venv/target/android/crates/docs/
                               firmware/dashboard-web/… du contexte de build
.uv-version (racine)         — source unique de la version uv (Dockerfile +
                               .github/workflows/dashboard.yml)
```

## Concepts / types importants

| Élément | Fichier | Ce que ça fait |
|---|---|---|
| `dashboard/api/Dockerfile` | | image `dengon-dashboard-api` : `uv sync --frozen` puis `uvicorn app.main:app` sur `:8000` (jamais publié directement sur l'hôte — seul Caddy l'est) |
| `docker-compose.yml` | `dashboard/deploy/` | service `api` (build local) + service `caddy` (image officielle) ; volumes **nommés fixes** (`dengon_api_db`, `dengon_caddy_data`, `dengon_caddy_config`) pour que `purge-demo.sh` puisse les cibler sans deviner le préfixe du projet Compose |
| `Caddyfile` | `dashboard/deploy/` | reverse-proxy TLS ; `tls internal` (CA auto-signée, pas de nom de domaine) ; `default_sni` (voir Décisions) ; redirection HTTP→HTTPS |
| `CADDY_SITE_ADDRESS` | `.env` (non commité) | IP publique du VPS — CN/SAN du certificat auto-signé et nom par défaut utilisé par `default_sni` |
| `purge-demo.sh` | `dashboard/deploy/` | `docker compose down` → supprime **seulement** `dengon_api_db` → `docker compose up -d --build` ; garde les volumes Caddy (pas de re-génération du certificat à chaque purge) |
| `deploy-vps.yml` | `.github/workflows/` | `workflow_dispatch` seul déclencheur ; transfert par `tar`+`ssh` (pas de `rsync` sur le VPS, voir Décisions) ; smoke tests `/healthz` HTTPS + un batch ingéré, exécutés depuis le runner GitHub (donc depuis l'extérieur, comme un vrai client) |

## Flux principal (exemple)

```
déclenchement manuel de `deploy-vps` (Actions → Run workflow)
  checkout du dépôt
  clé SSH écrite dans ~/.ssh (secrets de l'environment vps-prod)
  tar czf dashboard/api contracts dashboard/deploy | ssh vps "tar xzf - -C ~/dengon"
  ssh vps "cd ~/dengon/dashboard/deploy && docker compose up -d --build"
    → image api reconstruite si le code a changé, sinon cache Docker réutilisé
    → .env du VPS PAS touché (créé une fois à la main, voir Procédure d'accès)
  smoke test 1 : curl -fsSk https://<IP>:8443/healthz                    → doit répondre 200
  smoke test 2 : curl -X POST https://<IP>:8443/ingest/batch (sans JWT)  → doit répondre 401
```

En session de démo, avant de commencer : `ssh vps "cd ~/dengon/dashboard/deploy && ./purge-demo.sh"`.

## Dépendances

- **Internes :** `dashboard/api/` (l'image construite) et `contracts/events/`
  (copié dans l'image — l'ingestion de l'US-216, pas encore mergée au moment
  de l'US-224, lit `contracts/events/*.schema.json` par chemin filesystem
  relatif à la racine du dépôt, pas par import Python : le `Dockerfile` doit
  donc avoir la racine du dépôt comme **contexte de build**
  (`docker build -f dashboard/api/Dockerfile .`, pas `dashboard/api/`), et
  `docker-compose.yml` le fait déjà (`build.context: ../..`). Anticipé pour
  ne pas avoir à retoucher ce module quand #91 mergera.
- **Externes :** `caddy:2-alpine` (reverse-proxy), Docker + Docker Compose
  (déjà présents sur le VPS, contrairement à `rsync`).

## Décisions d'implémentation

- **Ports 8080/8443, pas 80/443** : les ports standard sont déjà occupés par
  un processus root sur ce VPS partagé (confirmé via `/proc/net/tcp`, uid 0,
  sans qu'aucun conteneur Docker visible ne les publie — donc un service
  système, hors de notre contrôle, `sudo` demandant un mot de passe que nous
  n'avons pas). Écart consigné dans `03-ecarts-conception.md`.
- **`tls internal` (CA auto-signée), pas Let's Encrypt** : aucun nom de
  domaine disponible pour ce VPS (identifié seulement par IP) — Let's
  Encrypt (HTTP-01/TLS-ALPN-01) exige un nom d'hôte. Écart consigné.
- **`default_sni` dans le Caddyfile — piège rencontré en déployant pour de
  vrai** : un client qui se connecte à une IP littérale (`curl`, un
  navigateur) **n'envoie pas d'indication SNI** — comportement TLS normal
  pour une IP. Sans `default_sni`, Caddy n'a alors aucun moyen de choisir un
  certificat pour la connexion et répond `tlsv1 alert internal error` — testé
  en local d'abord avec `localhost` (fonctionnait, un nom de domaine envoie
  bien du SNI), puis en re-testant sur le VPS réel avec l'IP publique
  (échouait). Le diagnostic clé : `openssl s_client -servername <IP>`
  fonctionnait (SNI forcé manuellement) alors que `curl` échouait toujours
  sur la MÊME configuration — la preuve que ce n'était pas le certificat en
  lui-même mais l'absence de SNI d'un client réel. `default_sni
  {$CADDY_SITE_ADDRESS}` donne à Caddy un nom de repli à utiliser quand
  aucune SNI n'arrive.
- **Pas de `rsync`** : absent du VPS, pas d'accès `sudo` pour l'installer.
  `tar` sur `ssh` (déjà utilisé pour la préparation manuelle, repris tel
  quel dans le workflow) fait le même travail sans dépendance supplémentaire
  côté serveur.
- **`.env` du VPS créé une fois à la main, jamais par le workflow** : le
  régénérer à chaque déploiement changerait le secret JWT et invaliderait
  tous les jetons déjà émis aux nœuds — un déploiement doit pouvoir se
  répéter sans casser les nœuds déjà enregistrés.
- **`workflow_dispatch` seul déclencheur, pas `push` sur `main`** : le VPS
  étant partagé, redémarrer le service à chaque merge déciderait à la place
  des autres groupes du moment où leur "bruit de fond" (redémarrage de
  conteneur, pas d'indisponibilité pour eux vu que les services sont
  indépendants, mais un principe de prudence sur une ressource commune).
- **Volumes Compose nommés explicitement** (`name:`) plutôt que laissés au
  préfixe du projet (nom du dossier) : `purge-demo.sh` doit pouvoir cibler
  `dengon_api_db` sans dépendre de l'endroit d'où `docker compose` est
  invoqué.
- **`purge-demo.sh` ne touche pas aux volumes Caddy** : les supprimer
  régénérerait le certificat auto-signé à chaque purge, ce qui obligerait à
  re-valider le certificat dans le navigateur à chaque session de démo —
  coût sans bénéfice, seule la base applicative doit repartir de zéro.
- **Corrections suite à la revue de la PR #97** (constats POWLAIR/OswinFreyr,
  détail dans l'entrée de journal du 2026-09-28 « corrections post-revue ») :
  - Smoke test 2 vérifie désormais un **401** sur `POST /ingest/batch` sans
    JWT (US-216, PR #91, était déjà mergée sur `main` — l'ancien test qui
    attendait un `202` aurait échoué au premier run réel du workflow).
  - `VPS_KNOWN_HOSTS` (nouveau secret, voir Limites/Procédure d'accès)
    remplace `StrictHostKeyChecking accept-new`, qui acceptait la clé d'hôte
    sans vérification à **chaque** run (runner éphémère) plutôt qu'au seul
    premier contact.
  - `CADDY_HTTPS_PORT`/`CADDY_HTTP_PORT` (`.env`, optionnels, défaut
    8443/8080) : une seule variable pour le port, lue par
    `docker-compose.yml`, `Caddyfile` (redirection HTTP→HTTPS) et
    `deploy-vps.yml` (smoke tests) — remplace `8443` dupliqué en dur à 4
    endroits.
  - `.uv-version` (racine du dépôt) : source unique pour la version de `uv`,
    lue par `dashboard/api/Dockerfile` et `.github/workflows/dashboard.yml`.
  - `HEALTHCHECK` sur l'image `api` + `depends_on: condition: service_healthy`
    côté `caddy` : ferme la fenêtre de 502 transitoires au déploiement.
  - `encode gzip` exclu de `/api/stream` dans le Caddyfile (SSE, US-218) :
    `gzip` bufferise sa sortie, ce qui aurait retenu les événements live.
  - `.dockerignore` exclut maintenant `android/`, `crates/`, `docs/`,
    `firmware/`, `dashboard/web/` : seuls `contracts/` et `dashboard/api/`
    sont copiés dans l'image.
  - `purge-demo.sh` : `docker volume inspect` avant `rm` (distingue « absent »
    d'une autre erreur, ex. volume encore utilisé) ; `sleep 2` remplacé par
    un poll sur `/healthz`.
- **Second conteneur Caddy dédié pour `dashboard/web/`** (issue #125) :
  ajouté plutôt que d'ajouter une route au `Caddyfile` de l'API, pour qu'une
  purge/un redéploiement de l'un ne touche jamais l'autre (le service `web`
  n'a pas de base de données à purger, l'API n'a pas de fichiers statiques
  à recharger). Deuxième instance Caddy plutôt que nginx : réutilise
  directement `tls internal` + `default_sni` déjà éprouvés ci-dessus,
  sans re-générer un certificat auto-signé à la main pour un serveur
  différent. **Volumes de certificats séparés** (`dengon_web_caddy_data`/
  `dengon_web_caddy_config`, distincts de `dengon_caddy_data`/`_config`) :
  deux instances Caddy indépendantes ne peuvent pas partager le même
  répertoire `/data`, chacune y gère son propre état de CA interne.
  `dashboard/web/index.html` ne code plus en dur l'URL de l'API :
  `config.js`, généré par l'entrypoint du conteneur `web` à partir de la
  variable `DENGON_WEB_API_BASE` (`.env`), n'est jamais commité — voir
  `dashboard/deploy/docker-compose.yml` (service `web`) et
  `Caddyfile.web`.
- **`--force-recreate web` après chaque redéploiement** (revue de PR #128,
  POWLAIR — **piège reproduit et vérifié en local**) : `web` monte
  `../web` et `./Caddyfile.web` en **bind mount**, qui suit l'inode, pas le
  chemin. `deploy-vps.yml` fait `rm -rf ~/dengon/dashboard/web` puis
  ré-extrait le `tar` : ça crée un **nouveau** répertoire. Un conteneur
  `web` déjà en marche garde son montage sur l'**ancien** inode, supprimé —
  ni l'image (`caddy:2-alpine`) ni la config Compose du service n'ayant
  changé, `docker compose up -d --build` seul ne le recrée pas. Reproduit
  en local : `rm -rf ../web && cp -r <sauvegarde> ../web` puis `up -d
  --build` seul → **404** (`Container deploy-web-1 Running`, pas recréé) ;
  `up -d --no-deps --force-recreate web` juste après → 200, `config.js`
  correct. Invisible sur un premier déploiement, seulement au **second**.
- **`DENGON_WEB_API_BASE` échappée avant injection dans `config.js`**
  (revue de PR #128, POWLAIR) : un guillemet ou un antislash dans la
  valeur casserait silencieusement la syntaxe JS générée. Pas une faille
  (la valeur vient du `.env` de l'opérateur, pas d'une entrée réseau), mais
  corrigé par un `sed 's/\\/\\\\/g; s/"/\\"/g'` avant le `printf` — vérifié
  en local avec une valeur contenant `"` : le guillemet ressort bien
  échappé (`\"`) dans `config.js`, pas casseur de syntaxe.
- **Utilisateur non-root dans l'image `api`** (`USER app`, retour de revue
  SonarCloud — 4 findings sur `dashboard/api/Dockerfile` : image `python`
  tournant root par défaut, deux `uv`/`pip install` sans forcer les wheels
  seules, un `cd` préféré à `WORKDIR`). Tous corrigés : `--only-binary
  :all:`/`--no-build` sur les deux installs, `WORKDIR` partout, utilisateur
  `app` dédié activé juste avant `CMD`. **Piège rencontré en le redéployant** :
  le volume `dengon_api_db` existant (créé par l'ancien conteneur, root)
  restait la propriété de `root` — `sqlite3.OperationalError: attempt to
  write a readonly database` au démarrage du conteneur non-root. Corrigé en
  relançant `purge-demo.sh` (le volume est recréé, donc hérite des
  permissions `app` posées dans l'image) — la bonne réaction est déjà le
  script existant, pas un nouveau outil.

## Tests

- **Vérifié manuellement sur le VPS réel** (le smoke test « DoR n°7 » de
  l'issue #38) le 2026-09-28 :
  - `curl -sk https://51.255.38.214:8443/healthz` → `{"status":"ok"}`, HTTP 200.
  - `curl -sI http://51.255.38.214:8080/healthz` → `301` vers `https://…:8443/…`.
  - `curl -X POST https://…/ingest/batch -d '[]'` → `202`, batch stocké.
  - `./purge-demo.sh` exécuté : conteneurs recréés, `/healthz` répond de
    nouveau 200 après coup.
  - Testé avec `openssl s_client` en plus de `curl`, ce qui a permis
    d'isoler le bug `default_sni` (voir Décisions) — `curl`/LibreSSL seul
    aurait laissé penser à un problème de certificat.
  - Re-déployé après les correctifs SonarCloud (utilisateur non-root) :
    `/healthz` (200) et `POST /ingest/batch` (202) re-vérifiés en HTTPS
    externe après `purge-demo.sh` (nécessaire pour le volume, voir
    Décisions).
- **SonarCloud** : Quality Gate passait au rouge sur la PR (« C Security
  Rating on New Code », 4 findings sur `dashboard/api/Dockerfile`) — les 4
  corrigés (voir Décisions), ré-analyse à vérifier sur la PR.
- **`deploy-vps.yml` pas encore exécuté depuis GitHub Actions** au moment de
  cette entrée (secrets de l'environment `vps-prod` à configurer — voir
  Limites) ; la procédure manuelle ci-dessus est celle que le workflow
  reproduit à l'identique (mêmes commandes `tar`/`ssh`/`docker compose`).

## Limites connues / TODO

- **Certificat auto-signé** : un vrai navigateur affiche un avertissement de
  sécurité sur `https://51.255.38.214:8443/` (API) **et** sur
  `https://51.255.38.214:8444/` (dashboard web, issue #125 — deux
  instances Caddy, donc deux CA internes distinctes, deux avertissements à
  accepter séparément). Pour l'oral, prévoir de cliquer « continuer » sur
  les deux ou d'importer les CA internes de Caddy à l'avance
  (`docker compose exec caddy cat /data/caddy/pki/authorities/local/root.crt`,
  puis `docker compose exec web cat /data/caddy/pki/authorities/local/root.crt`).
- **Second conteneur `web` (issue #125) pas encore vérifié sur le VPS réel**
  au moment de cette entrée : la configuration (`docker-compose.yml`,
  `Caddyfile.web`, workflow) est prête et vérifiable en local, mais le
  dernier critère d'acceptation de l'issue — les 4 écrans chargent de
  vraies données depuis un navigateur, sur le VPS — exige un accès SSH et
  un run réel de `deploy-vps.yml`, non fait ici.
- **Pas de pare-feu applicatif de notre côté** : sans `sudo`, impossible de
  configurer `ufw`/`iptables` sur ce VPS. On dépend entièrement du réseau
  déjà en place (et des autres groupes qui partagent la machine).
- **Secrets GitHub Actions (`vps-prod`) pas encore configurés** au moment de
  cette entrée : `VPS_HOST`, `VPS_PORT`, `VPS_USER`, `VPS_SSH_KEY`,
  `VPS_KNOWN_HOSTS` — à ajouter dans Settings → Environments → vps-prod avant
  le premier run du workflow. `VPS_KNOWN_HOSTS` (ajouté après revue de la PR
  #97) = sortie de `ssh-keyscan -p <VPS_PORT> <VPS_HOST>`, à générer et
  **vérifier une fois à la main** (voir Procédure d'accès) avant de la coller
  comme secret — sans lui, l'étape « Préparer la clé SSH » du workflow écrit
  un `known_hosts` vide et tout `ssh`/`tar` suivant échouera (`Host key
  verification failed`), c'est voulu : mieux vaut un run qui échoue vite
  qu'une vérification d'hôte silencieusement désactivée.
- **Le smoke test d'ingestion vérifie un 401, pas un aller-retour complet** :
  depuis que US-216 (PR #91) est mergée sur `main`, `/ingest/batch` exige un
  JWT + une signature Ed25519 valide — hors de portée d'un smoke test bash
  sans y enregistrer un nœud de test au préalable. Le smoke test 2 vérifie
  donc que l'auth est bien active en prod (401 sans jeton) plutôt qu'une
  ingestion réussie de bout en bout ; ce niveau de couverture reste un choix
  à revisiter si une ingestion réelle post-déploiement devient nécessaire.
- **Un seul utilisateur SSH (`group3`) partagé pour toute l'équipe** : voir
  Procédure d'accès ci-dessous pour comment Paul et Oswin s'y connectent
  sans partager de clé privée entre eux.

## Procédure d'accès (3 personnes)

Le VPS n'a qu'un compte SSH (`group3`), déjà protégé par mot de passe +
désormais une clé. Chaque personne doit avoir **sa propre paire de clés**
(pas de clé privée partagée par messagerie) :

1. Générer sa propre paire : `ssh-keygen -t ed25519 -f ~/.ssh/dengon_vps -C "<prénom>-dengon-deploy"`.
2. Demander à quelqu'un qui a déjà accès (mot de passe, ou déjà connecté)
   d'ajouter la clé **publique** (`~/.ssh/dengon_vps.pub`) au fichier
   `~/.ssh/authorized_keys` du compte `group3` sur le VPS — soit via
   `ssh-copy-id -i ~/.ssh/dengon_vps.pub -p 2221 group3@51.255.38.214`
   depuis un poste qui a déjà le mot de passe, soit en collant la ligne
   manuellement dans `authorized_keys` depuis une session déjà ouverte.
3. Ajouter l'alias dans son `~/.ssh/config` local :
   ```
   Host dengon-vps
       HostName 51.255.38.214
       Port 2221
       User group3
       IdentityFile ~/.ssh/dengon_vps
   ```
4. Vérifier : `ssh dengon-vps echo ok`.

Le mot de passe du compte `group3` ne doit être utilisé qu'une fois par
personne, pour l'étape 2 — jamais pour se connecter ensuite.

### Secret `VPS_KNOWN_HOSTS` (une seule fois, pour le workflow)

Le workflow `deploy-vps.yml` tourne sur un runner GitHub **éphémère** : son
`~/.ssh/known_hosts` est vide à chaque run, donc il ne peut pas vérifier la
clé d'hôte du VPS tout seul (voir Décisions — corrigé après revue de la PR
#97). Une personne qui a déjà vérifié l'empreinte de l'hôte (via l'étape 4
ci-dessus, ou en la comparant à une source de confiance) génère ce secret
une fois :

```
ssh-keyscan -p 2221 51.255.38.214
```

Coller la sortie telle quelle (plusieurs lignes, une par type de clé) dans
Settings → Environments → vps-prod → secrets → `VPS_KNOWN_HOSTS`. À refaire
seulement si le VPS change de clé d'hôte (réinstallation du système, par
exemple) — le workflow échouera alors explicitement (`Host key verification
failed`) plutôt que d'accepter silencieusement la nouvelle clé.

## Pour l'oral

Le VPS n'est pas un serveur dédié : c'est une machine **partagée** entre
plusieurs projets du cours, avec un accès root qui ne nous appartient pas.
Le point intéressant à raconter : découvrir cette contrainte a changé
concrètement l'implémentation (ports non standard, TLS auto-signé au lieu
de Let's Encrypt) — et un bug found en testant en conditions réelles
(`default_sni`, invisible en local avec `localhost`) montre pourquoi un
smoke test sur le vrai serveur, pas seulement en local, fait partie de la
définition de fini de cette US.
