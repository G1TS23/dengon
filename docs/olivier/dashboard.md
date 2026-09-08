# Sous-système de suivi (dashboard) — dengon (brouillon v0.2)

> Premier jet le 2026-09-08.
> v0.2 : arbitrage des points ouverts avec Olivier (code anonyme par message,
> rétention = durée de la démo, compteurs globaux inclus, mode démo si le temps
> le permet).
> À relire et compléter avec Paul et Tanguy.
> S'appuie sur `decisions-v1.md`, `architecture.md`, `protocole.md`.
> Les choix marqués « (ouvert) » ne sont pas tranchés.

---

## 1. But

Spécifier le **dashboard** : ce qu'il montre, ce qu'il ne montre **jamais**,
et comment les données y parviennent.

## 2. Principe et contraintes

- **Bonus non bloquant** : la messagerie fonctionne entièrement sans le dashboard.
- **Alimenté opportunément** : seuls les nœuds qui ont du **Wi-Fi** à un moment
  donné envoient des informations au serveur.
- **Anonymisé** : jamais le contenu, jamais les identités réelles. Un nœud se
  désigne par un **code anonyme différent pour chaque message** qu'il traite
  (voir section 6) — on peut donc reconstituer le parcours d'**un** message,
  mais pas relier entre eux les messages passés par un même appareil.
- **Mise à jour automatique** : la page se rafraîchit toute seule (choix v1).
- **Hébergement** : VPS Debian de l'équipe.

## 3. Ce que le dashboard affiche

- **Liste des messages suivis** : identifiant raccourci, statut courant, heure de
  création, heure de dernière activité.
- **Détail d'un message** : son **parcours** (suite de nœuds désignés par leurs
  codes), l'horodatage de chaque étape, le statut.
- **Compteurs globaux** (inclus en v1) : nombre de messages en circulation,
  nombre distribués / expirés, taux de distribution.
- **Nombre d'appareils actifs récemment** : souhaité, mais **impossible à
  déduire** des événements puisque le code d'un nœud change à chaque message
  (section 6). Deux options **(ouvert)** : soit on ajoute un **battement
  anonyme** séparé (un nœud connecté signale « je suis actif » avec un code qui
  tourne, ex. chaque jour), soit on **retire ce compteur** de la v1.
- Pas de carte géographique, pas de **liste** nominative de nœuds en v1.

## 4. Ce que le dashboard ne montre JAMAIS

| Donnée | Affichée ? |
|--------|-----------|
| Contenu d'un message | Non |
| Clé publique / empreinte / pseudo réel d'un utilisateur | Non |
| Qui a écrit à qui | Non |
| Position physique d'un appareil | Non |
| Identifiant de message (raccourci) | Oui |
| Statut d'un message | Oui |
| Parcours en **codes de nœuds** | Oui |
| Horodatages des étapes | Oui |

## 5. Les événements envoyés par les nœuds

Quand un nœud a du Wi-Fi, il envoie au serveur des **événements**. Aucun
événement ne contient d'empreinte d'expéditeur ou de destinataire.

| Événement | Émis par | Contenu |
|-----------|----------|---------|
| `message-créé` | l'expéditeur | id raccourci, code **pour ce message**, horodatage |
| `message-relayé` | un relais | id raccourci, code **pour ce message**, sauts restants, horodatage |
| `message-distribué` | le destinataire | id raccourci, code **pour ce message**, horodatage |
| `message-expiré` | tout nœud qui purge le message | id raccourci, code **pour ce message**, horodatage |

Le serveur doit être **tolérant** : les événements peuvent arriver en retard,
dans le désordre, en double, ou jamais (un nœud sans Wi-Fi ne rapporte rien).

## 6. Le code anonyme d'un nœud

- Un nœud génère un **code anonyme distinct pour chaque message** qu'il traite
  (par ex. dérivé de l'identifiant du message + un secret local, de façon à être
  stable pour *ce* message mais imprévisible d'un message à l'autre).
- **Ce que ça permet** : reconstituer le parcours d'**un** message (les
  événements portant le même id se recollent).
- **Ce que ça empêche** : relier entre eux les messages passés par le même
  appareil → on ne peut pas suivre un appareil dans la durée depuis le
  dashboard. C'est le choix retenu (confidentialité).
- Conséquence : pas de « nombre d'appareils actifs » directement calculable
  (voir section 3).

## 7. Côté serveur (VPS Debian)

- **API de réception** des événements, en HTTPS.
- **Authentification des envois** — **(ouvert)** : jeton partagé entre les nœuds
  et le serveur ? Le but est d'empêcher un tiers d'injecter de faux événements
  qui pollueraient l'affichage.
- **Stockage** : les événements bruts + un **état reconstruit par message**.
- **Reconstruction du parcours** : pour un message donné, ordonner ses événements
  par horodatage, retirer les doublons, en déduire le **statut courant**.
- **Canal temps réel** vers la page (pour la mise à jour automatique) — piste :
  SSE (voir `etude-stack.md` §6).
- **Page web** servie par le même serveur.
- **Rétention** : les données sont **effacées après chaque session de démo**
  (pas d'historique long conservé). Une remise à zéro manuelle suffit en v1.

## 8. Statut courant d'un message (déduit par le serveur)

| Statut affiché | Règle |
|----------------|-------|
| **En circulation** | Au moins un `message-créé` ou `message-relayé`, ni `distribué` ni `expiré`. |
| **Distribué** | Un `message-distribué` reçu. |
| **Expiré** | Un `message-expiré` reçu, et pas de `message-distribué`. |
| **Inconnu / partiel** | Aucun événement reçu pour ce message (il peut exister quand même). |

Le dashboard ne connaît que ce que les **nœuds connectés** lui rapportent : c'est
une **vue partielle** du réseau, à assumer et à indiquer sur la page.

## 9. Écrans (esquisse)

```
┌───────────────────────────── dengon · suivi ─────────────────────────────┐
│ Messages suivis                              nœuds actifs : 6            │
│ ─────────────────────────────────────────────────────────────────────── │
│ id        statut           créé        dernière activité                │
│ 7f3a…     En circulation   10:02       10:04   (3 étapes)   ▶           │
│ b1c8…     Distribué        09:51       09:58   (4 étapes)   ▶           │
│ 42de…     Expiré           hier 22:10  hier 22:41            ▶          │
└─────────────────────────────────────────────────────────────────────────┘

Détail du message 7f3a… :
   [ node-K ] créé 10:02
        │
        ▼ relayé 10:03 (sauts restants 6)
   [ node-M ]
        │
        ▼ relayé 10:04 (sauts restants 5)
   [ node-P ]        … en attente de la suite
```

## 10. Repli si le temps manque

1. Page **rafraîchie à la main** (on abandonne le canal temps réel).
2. **Maquette statique** avec des données d'exemple, juste pour illustrer le
   principe à la soutenance.
3. **Mode démo** qui rejoue un scénario enregistré (utile si le réseau réel est
   capricieux le jour J) — **à faire seulement s'il reste du temps** en fin de
   projet.

## 11. Points ouverts

**Tranchés en v0.2 :** code anonyme = un par message · rétention = effacée
après chaque session de démo · compteurs globaux inclus · mode démo = si le
temps le permet.

**Encore ouverts :**

- **Authentification des nœuds auprès de l'API** (jeton partagé ?) — pour
  empêcher l'injection de faux événements.
- **Nombre d'appareils actifs** : ajouter un battement anonyme séparé, ou
  retirer ce compteur (voir section 3).
- Technologies précises du serveur et de la page (décision d'Olivier —
  pistes dans `etude-stack.md` §6).
- Format exact des événements et de l'identifiant raccourci de message.
