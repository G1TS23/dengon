# Mise en commun — proposition (brouillon v0.1)

> Rédigé le 2026-09-08. À présenter à Paul et Tanguy pour la fusion prévue
> **après la phase de conception** (`decisions-v1.md`, planning).
> Objectif : structure du dépôt commun + liste de tout ce qui est à **valider
> ou trancher en réunion d'équipe**.

---

## 1. Structure de dépôt proposée

```
dengon/
├── README.md              présentation générale + état d'avancement
├── docs/
│   ├── concept.md         le concept et les besoins (ex-CONTEXT.md, partagé)
│   ├── decisions.md       décisions d'équipe VALIDÉES (source de vérité)
│   ├── planning.md        phases et jalons jusqu'au 29/09
│   ├── protocole/
│   │   ├── comportement.md   (base : docs/olivier/protocole.md)
│   │   ├── format-trame.md   (base : docs/olivier/format-trame.md) — fait foi
│   │   └── securite.md       (à écrire — Paul ou Tanguy ?)
│   ├── architecture.md   (base : docs/olivier/architecture.md)
│   ├── dashboard.md      (base : docs/olivier/dashboard.md)
│   ├── stack.md          (base : docs/olivier/etude-stack.md)
│   ├── olivier/  paul/  tanguy/     brouillons personnels, gardés en archive
├── app/                  application mobile
├── firmware/             code des cartes ESP32
├── dashboard/
│   ├── serveur/          API + reconstruction + temps réel
│   └── page/             page web de suivi
├── outils/               scripts de test, simulateur léger
└── .githooks/            hook commit-msg (déjà en place)
```

Principe : les brouillons de `docs/olivier/` deviennent la **base** des
documents partagés `docs/…` ; chacun garde son sous-dossier perso comme
historique.

## 2. Ce qu'Olivier apporte à la fusion

| Document (dans `docs/olivier/`) | État | Devient |
|---|---|---|
| `analyse-besoins.md` | v0.1 | `docs/` (annexe) |
| `decisions-v1.md` | v0.1 | base de `docs/decisions.md` + `docs/planning.md` |
| `protocole.md` | v0.3 | `docs/protocole/comportement.md` |
| `format-trame.md` | v0.1 | `docs/protocole/format-trame.md` |
| `architecture.md` | v0.1 | `docs/architecture.md` |
| `dashboard.md` | v0.2 | `docs/dashboard.md` |
| `etude-stack.md` | v0.1 | `docs/stack.md` |

## 3. À valider en réunion (décisions prises comme « avis d'Olivier »)

Tout est détaillé dans `decisions-v1.md`. En synthèse, à confirmer ou corriger :

**Cadrage v1** — objectif étude + démo à parts égales ; Android + ESP32 (iOS
reporté) ; messages 1-à-1 texte court ; protocole maison ; chiffrement dès la
v1 ; ajout de contact en présentiel ; statut « Lu » reporté ; dashboard inclus
mais minimal.

**Produit / UX** — pseudo libre ; service de fond avec notification ;
historique jusqu'à suppression manuelle ; indicateur réseau simple ; interface
en français ; relais toujours actif + mode éco ; vérification de clé par code à
comparer ; message expiré → statut « Échec » ; bouton « Renvoyer » ; blocage de
contact reporté ; dashboard live et anonymisé ; nom « dengon » à rediscuter.

**Organisation** — livrables : 3 docs techniques + rapport + slides ;
répartition **par composant** (mobile / ESP32 / dashboard), conception
commune ; Olivier sur protocole + dashboard ; simulateur léger ; fusion après
la phase conception ; découpage ~1 sem / ~1,5 sem / ~0,5 sem.

**Protocole (tranché v0.3)** — TTL 7 ; « écouter avant de rediffuser » ; ordre
d'affichage = ordre d'arrivée ; file ~50 (ESP32) / ~300 (téléphone), éviction
du plus ancien ; échange d'inventaire entre voisins en v1 ; anti-inondation
simple en v1.

**Dashboard (tranché v0.2)** — code anonyme distinct par message ; données
effacées après chaque session de démo ; compteurs globaux inclus ; mode démo
seulement si le temps le permet.

## 4. À trancher en réunion (décisions techniques Paul / Tanguy)

- **Techno de l'app mobile** : Flutter + `bluetooth_low_energy` vs natif
  Kotlin — à décider **après le prototype « hello mesh »** (voir §5).
- **Techno du serveur dashboard** et de la page (piste : FastAPI/Express + SSE
  + SQLite).
- **Un seul « moteur dengon » ou deux ?** (reco `etude-stack.md` §3 : deux
  implémentations + spec de trame stricte).
- **Service et caractéristiques BLE** exacts.
- **Primitives cryptographiques** définitives (→ doc sécurité).
- **Authentification des nœuds** auprès de l'API du dashboard.
- **Nombre d'appareils actifs** sur le dashboard : battement anonyme séparé, ou
  on retire ce compteur.
- Valeurs numériques à **calibrer** (délais, seuils, tailles de file).
- Réutilise-t-on du code de Meshtastic / Bridgefy, ou seulement leurs idées ?

## 5. À se répartir tout de suite (semaine 1)

| Tâche | Pourquoi maintenant | Proposition |
|---|---|---|
| **Prototype « hello mesh »** : 2 téléphones + 1 relais, 1 message qui passe | C'est le **go / no-go** de la techno mobile | Paul ou Tanguy |
| **Doc sécurité** (`docs/protocole/securite.md`) | Référencée par `protocole.md` et `format-trame.md` ; bloque la crypto | Paul ou Tanguy |
| **Monter le dépôt commun** (structure §1, migration des docs) | Fin de la phase conception | Olivier |
| **Attribution des composants** (mobile / ESP32 / dashboard) | Conditionne toute la phase démo | Réunion |
| **Plan de la démo réduite** + scénario campus concret | Cadre le développement | Réunion, puis Olivier rédige |

## 6. Ordre du jour proposé pour la réunion

1. Confirmer / corriger le cadrage v1 et les points « avis d'Olivier » (§3).
2. Trancher les décisions techniques (§4).
3. Attribuer les composants et les tâches de la semaine 1 (§5).
4. Valider la structure du dépôt commun (§1) et la date de fusion.
5. Fixer un point d'avancement récurrent (2-3 fois par semaine vu le délai).
