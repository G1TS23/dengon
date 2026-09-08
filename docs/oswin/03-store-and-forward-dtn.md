# 03 — Stockage & relais : store-and-forward / DTN

> C'est le cœur original du projet (points 4 et 5 du cahier des charges) : le message est
> **stocké** sur chaque point de transmission **tant que le destinataire n'a pas reçu**, et
> il **circule** de nœud en nœud. Bonne nouvelle : c'est un domaine de recherche mûr, avec
> un nom, des RFC et des algorithmes connus.

## 1. Le bon terme : *store-carry-forward* / DTN

Un réseau où il n'existe pas forcément de chemin **continu** entre A et B à un instant
donné s'appelle un **réseau tolérant aux délais et aux ruptures — DTN (Delay/Disruption
Tolerant Networking)**. Son principe fondamental est le **store-carry-forward** :

> un nœud **reçoit** un message, le **garde** (store) même s'il n'a personne à qui le
> passer, le **transporte** physiquement (carry, ex. dans une poche qui se déplace), puis
> le **retransmet** (forward) dès qu'un nouveau voisin utile apparaît.

C'est **exactement** « stocker sur les points de transmission tant que le destinataire n'a
pas reçu ». À citer tel quel dans le rapport.

*Concept issu des travaux sur les réseaux interplanétaires puis généralisé ; voir
[EmergentMind – DTN protocols](https://www.emergentmind.com/topics/delay-disruption-tolerant-network-dtn-protocols).*

## 2. La norme : Bundle Protocol v7 (RFC 9171)

L'IETF a standardisé le DTN avec le **Bundle Protocol version 7 (RFC 9171, 2022)**. Idées
réutilisables pour dengon :

- l'unité transmise est un **bundle** (paquet auto-suffisant, comme notre « message ») ;
- chaque bundle a une **durée de vie** (*lifetime*) → au-delà, il est **détruit** (= notre
  TTL temporel) ;
- fonctionnement en **store-and-forward** avec **conservation en mémoire** entre deux
  contacts ;
- notion de **custody transfer** : un nœud peut prendre la **responsabilité** d'un bundle
  (« je m'engage à essayer de le délivrer »).

On ne va pas implémenter tout le RFC 9171, mais **s'en inspirer** donne de la crédibilité au
rapport, et les concepts (lifetime, custody, bundle) sont directement transposables.

*Sources : [RFC 9171 (texte)](https://www.rfc-editor.org/rfc/rfc9171.html),
[RFC 9171 (RFC Editor info)](https://www.rfc-editor.org/info/rfc9171/),
implémentation open source : [DTN7](https://dtn7.github.io/).*

## 3. Comment décider « à qui retransmettre » ? Les algorithmes de routage DTN

Puisqu'on ne connaît pas le chemin à l'avance, plusieurs stratégies existent :

| Algorithme | Principe | Avantage | Inconvénient |
|---|---|---|---|
| **Epidemic routing** | on donne une copie à **tous** les voisins rencontrés | livraison quasi maximale, très simple | **inonde** le réseau, gros coût mémoire/énergie |
| **Spray-and-Wait** | on ne diffuse qu'un **nombre limité L de copies**, puis on attend | bon compromis coût/livraison | règle L à fixer |
| **PRoPHET** | on relaie selon une **probabilité de rencontre** (historique) | efficace si les contacts sont réguliers | plus complexe |
| **Managed flooding + TTL** (Bitchat) | inondation **bornée par le TTL** et le cache | simple, éprouvé sur BLE | pas « intelligent » |

**Pour dengon (projet d'école), commencer par l'*epidemic routing* borné par TTL** (=
managed flooding, déjà décrit fiche 01). C'est le plus simple à coder et à expliquer.
Mentionner **Spray-and-Wait** comme optimisation « pour aller plus loin » (limiter la
consommation mémoire des relais).

## 4. Cycle de vie d'un message stocké

```text
                 message reçu par un relais
                          │
                          ▼
                 [ FILE DE STOCKAGE LOCALE ]
                 (clé = id_message, valeur = paquet + métadonnées)
                          │
          ┌───────────────┼─────────────────────────────┐
          ▼               ▼                             ▼
  un nouveau voisin   TTL/lifetime          ACK reçu pour ce message
  apparaît            atteint                (le destinataire l'a eu)
          │               │                             │
   retransmettre     SUPPRIMER               SUPPRIMER de la file
   (forward)         (expiration)            (purge → libère la mémoire)
```

Trois déclencheurs de **suppression** (essentiels pour ne pas saturer les relais) :

1. **Expiration** : TTL en sauts épuisé **ou** *lifetime* temporel dépassé.
2. **Accusé de réception (ACK)** : le destinataire renvoie un petit message signé
   `ACK(id_message)` qui **repart dans le mesh**. Chaque relais qui le reçoit **purge** sa
   copie du message correspondant. → C'est ce qui réalise proprement « tant que le
   destinataire n'a pas reçu ».
3. **Quota mémoire** : si la file est pleine, appliquer une politique (supprimer le plus
   vieux / le plus diffusé — *drop-oldest* / *drop-most-forwarded*).

## 5. Détails d'implémentation à ne pas oublier

- **Déduplication** : indexer par `id_message` et garder un cache des IDs déjà vus (anti-
  boucle, fiche 01).
- **Persistance** : stocker la file sur disque/flash pour survivre à un redémarrage du nœud
  (surtout l'Arduino).
- **Fragmentation** : un message > MTU BLE doit être découpé/réassemblé (fiche 01, §2) ;
  chaque fragment porte l'`id_message` + un index.
- **Anti-tempête** : temporiser aléatoirement les retransmissions (*jitter*) pour éviter que
  tous les nœuds réémettent en même temps.
- **Sécurité de l'ACK** : l'ACK doit être **signé** par le destinataire, sinon un relais
  malveillant pourrait effacer des messages en forgeant de faux ACK (lien avec fiche 02).

## 6. Ce qu'il faut mesurer (et montrer sur le dashboard)

Le store-carry-forward se **prouve** par des métriques — parfaites pour le dashboard
(fiche 05) :

- **taux de livraison** (messages délivrés / envoyés) ;
- **délai de livraison** (temps A→B, potentiellement long : c'est *voulu*) ;
- **nombre de sauts** effectivement parcourus ;
- **nombre de copies** en circulation (coût du routage épidémique) ;
- **occupation mémoire** des files de stockage.

---

### À retenir
« Stocker sur les points de transmission tant que le destinataire n'a pas reçu » = **store-
carry-forward** en **DTN**, formalisé par le **Bundle Protocol (RFC 9171)** avec sa notion
de *lifetime* et de *custody*. Routage conseillé : **épidémique borné par TTL**, purge par
**ACK signé** + **expiration**. Ces choix se **mesurent** et alimentent le dashboard.
