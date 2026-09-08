# 04 — Faut-il une blockchain ? (analyse critique)

> Votre première intuition est d'utiliser « un système similaire à de la blockchain ».
> Cette fiche fait le tri **honnêtement** : ce que la blockchain apporterait vraiment, ce
> qu'elle coûterait, et **quelles briques issues de la blockchain sont réellement utiles**
> pour dengon (spoiler : le **chaînage par hash** et les **arbres de Merkle**, oui ; une
> blockchain complète avec consensus, probablement non).

## 1. De quoi parle-t-on exactement ?

Une blockchain « complète » (type Bitcoin/Ethereum) combine plusieurs ingrédients :

1. des **hachages cryptographiques** (SHA-256…) ;
2. un **chaînage** : chaque bloc contient le hash du précédent → **immuabilité** et
   **ordre** ;
3. des **arbres de Merkle** : résumer plein de données par un seul hash (« racine ») ;
4. la **réplication** : tout le monde a une copie du registre ;
5. un **mécanisme de consensus** (preuve de travail, preuve d'enjeu…) pour que des acteurs
   **qui ne se font pas confiance** se mettent d'accord sur **un** historique commun, sans
   autorité centrale.

Le point (5) — le **consensus** — est ce qui rend une blockchain *lourde, lente et
coûteuse*. C'est aussi ce dont dengon **n'a probablement pas besoin**.

*Sources : [Medium – Blockchain, Hash & Merkle tree](https://medium.com/@zlhk100/blockchain-hash-and-merkle-tree-data-immutability-and-integrity-append-only-database-eff7b621b9c3),
[GeeksforGeeks – Blockchain Merkle Trees](https://www.geeksforgeeks.org/blockchain-merkle-trees/).*

## 2. À quel besoin la blockchain répond-elle vraiment ?

La blockchain résout **un** problème très précis : obtenir un **accord sur un historique
unique, ordonné et infalsifiable, entre des parties qui ne se font pas confiance et sans
tiers de confiance**.

Question à se poser pour dengon : **avons-nous ce problème ?**

- A veut envoyer un message à B, de façon **privée**. → besoin de **confidentialité**, pas
  d'un registre public.
- Les relais doivent transporter sans lire/modifier. → besoin d'**E2EE + intégrité**
  (fiche 02), pas de consensus.
- On veut délivrer « au mieux » malgré les ruptures. → besoin de **store-carry-forward**
  (fiche 03), pas d'un registre répliqué global.

**Conclusion :** le problème central de dengon n'est **pas** un problème de consensus. Une
blockchain complète serait une **réponse à une question qu'on ne se pose pas**.

## 3. Pourquoi une blockchain « complète » est mal adaptée ici

| Contrainte dengon | Ce qu'impose une blockchain | Verdict |
|---|---|---|
| Messages **privés** entre 2 personnes | registre **partagé/visible** par les nœuds | ❌ contradictoire avec la confidentialité |
| Nœuds **contraints** (téléphone, **Arduino**, batterie) | consensus **coûteux** (calcul, mémoire, énergie) | ❌ trop lourd pour un ESP32 |
| Réseau **intermittent** (souvent déconnecté) | consensus a besoin d'une **large connectivité** simultanée | ❌ incompatible avec le DTN |
| On veut **oublier** les messages livrés (purge) | blockchain = registre **append-only immuable**, on n'efface pas | ❌ à l'opposé du besoin de purge |
| Passer à l'échelle sur peu de nœuds | registre qui **grossit indéfiniment** | ❌ gaspillage de stockage |

Autrement dit, plusieurs propriétés « vendeuses » de la blockchain (registre public,
immuable, répliqué partout) sont ici des **inconvénients**.

## 4. Ce qui, dans la blockchain, est VRAIMENT utile pour dengon

Bonne nouvelle : on peut prendre les **briques cryptographiques** de la blockchain **sans**
le consensus. Ce sont elles qui portent la « magie » d'intégrité.

### a) Le chaînage par hash (*hash chain*)

Faire en sorte que chaque message d'une conversation contienne le **hash du message
précédent** :

```
msg1 : {texte, prev = 0000}          h1 = hash(msg1)
msg2 : {texte, prev = h1  }          h2 = hash(msg2)
msg3 : {texte, prev = h2  }          h3 = hash(msg3)
```

Bénéfices concrets, **très pertinents** :
- **intégrité + ordre** : impossible d'insérer, retirer ou réordonner un message sans casser
  la chaîne (le récepteur détecte le trou) ;
- **détection de censure** par un relais qui « avalerait » un message ;
- c'est **léger** : juste des hachages, aucun consensus. Un ESP32 le fait sans problème.

### b) Les arbres de Merkle

Résumer un **lot de messages** (ou de fragments) par une seule **racine de Merkle**. Cela
permet une **preuve d'inclusion compacte** : prouver qu'un message donné fait partie d'un
lot en ne fournissant que **log₂(n)** hachages, pas tout le lot.

Usages pour dengon :
- **accusé de réception groupé** : B prouve « j'ai bien reçu ces 8 messages » avec une seule
  racine + une petite preuve, économe en Bluetooth ;
- **vérification d'intégrité** d'un message fragmenté (chaque fragment = une feuille).

*Sources : [HackerNoon – Merkle trees, backbone of integrity](https://hackernoon.com/merkle-trees-and-cryptographic-accumulators-the-mathematical-backbone-of-blockchain-integrity),
[DEV – Merkle tree root for data integrity](https://dev.to/bloxbytes/understanding-the-concept-of-merkle-tree-root-in-blockchain-for-data-integrity-2hp0).*

### c) Signatures & horodatage

Déjà couverts fiche 02 (Ed25519). Combinés au chaînage, ils donnent une **preuve d'origine
+ d'ordre** — c'est 90 % de ce qu'on attend intuitivement d'une « blockchain » pour de la
messagerie, **sans** la blockchain.

## 5. Recommandation

> **Ne pas construire une blockchain avec consensus.** Construire un **DTN store-carry-
> forward chiffré de bout en bout** (fiches 01–03), et y **incorporer deux idées de la
> blockchain** : le **chaînage par hash** (intégrité + ordre + anti-censure) et,
> optionnellement, les **arbres de Merkle** (accusés de réception / preuves d'intégrité
> compacts).

Formulation défendable pour le rapport :

> « Nous nous inspirons des **structures de données de la blockchain** (chaînage
> cryptographique et arbres de Merkle) pour garantir l'intégrité et l'ordre des messages,
> **sans** recourir à un mécanisme de consensus distribué, inadapté à un réseau Bluetooth
> intermittent composé de nœuds contraints. »

C'est **plus juste techniquement** et **plus impressionnant** qu'un « on a fait une
blockchain » : ça montre que vous avez compris *à quoi sert* chaque brique.

## 6. Si l'énoncé impose vraiment « une blockchain »

Si le sujet exige explicitement une blockchain, l'option la moins absurde est une
**blockchain locale, légère, à autorité de confiance** (pas de preuve de travail) :

- un **registre chaîné** des **événements** (message émis / relayé / livré), pas du contenu
  des messages (qui reste chiffré) ;
- **hébergé/agrégé par la passerelle Arduino → dashboard** : le dashboard devient le
  « registre » qui montre la chaîne d'événements horodatés et vérifiables par hash ;
- consensus remplacé par la **signature** de chaque nœud (preuve d'autorité simplifiée).

Cela coche la case « blockchain » pour l'énoncé, tout en restant réaliste sur Arduino, et se
**visualise très bien** sur le dashboard (fiche 05).

---

### À retenir
La blockchain **complète** (avec consensus) est **surdimensionnée** et même **contre-
productive** ici (privé vs registre public, nœuds contraints, réseau intermittent, besoin de
purge). Mais deux de ses briques — **chaînage par hash** et **arbres de Merkle** — sont
**excellentes** pour l'intégrité et les preuves de réception, et **légères**. C'est **ça**
qu'il faut garder.
