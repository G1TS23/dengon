# dengon — Concepts & état de l'art

> Fait partie du **contexte global** de `docs/synthese/` — point d'entrée :
> [`00-contexte-global.md`](00-contexte-global.md). Questions encore ouvertes :
> [`01-sujets-a-trancher.md`](01-sujets-a-trancher.md).
> Cette section est de la **recherche documentaire** (majoritairement issue de
> `docs/oswin/`) : vocabulaire académique, références à citer, contre-exemples.
> Elle sert de socle « rapport / soutenance ».

---

## 1. Store-carry-forward / DTN

Un réseau où il n'existe pas forcément de chemin **continu** entre A et B à un
instant donné = **réseau tolérant aux délais et aux ruptures (DTN — Delay/
Disruption Tolerant Networking)**. Principe fondamental = **store-carry-forward** :
un nœud **reçoit** un message, le **garde** (store) même sans personne à qui le
passer, le **transporte** physiquement (carry), le **retransmet** (forward) dès
qu'un voisin utile apparaît. C'est exactement « stocker sur les points de
transmission tant que le destinataire n'a pas reçu ». Concept issu des travaux
sur les réseaux interplanétaires puis généralisé.

## 2. Bundle Protocol v7 — RFC 9171 (2022, IETF)

Standardisation IETF du DTN. Idées réutilisables :

- unité transmise = un **bundle** (paquet auto-suffisant) ;
- chaque bundle a une **durée de vie** (*lifetime*) → au-delà, détruit (= TTL
  temporel) ;
- store-and-forward avec **conservation en mémoire** entre deux contacts ;
- **custody transfer** : un nœud peut prendre la **responsabilité** d'un bundle.

On n'implémente pas tout le RFC 9171, mais s'en inspirer donne de la crédibilité
au rapport ; `lifetime`, `custody`, `bundle` sont directement transposables.
Implémentation open source de référence : **DTN7**.

## 3. Algorithmes de routage DTN

| Algorithme | Principe | Avantage | Inconvénient |
| --- | --- | --- | --- |
| **Epidemic routing** | une copie à **tous** les voisins rencontrés | livraison quasi maximale, très simple | **inonde** le réseau, coût mémoire/énergie |
| **Spray-and-Wait** | ne diffuser qu'un **nombre limité L** de copies, puis attendre | bon compromis coût/livraison | règle L à fixer |
| **PRoPHET** | relayer selon une **probabilité de rencontre** (historique) | efficace si contacts réguliers | plus complexe |
| **Managed flooding + TTL** (Bitchat) | inondation **bornée par TTL** et cache | simple, éprouvé sur BLE | pas « intelligent » |

Recommandation `oswin` : commencer par l'**epidemic routing borné par TTL** (=
managed flooding) ; mentionner **Spray-and-Wait** comme optimisation.

> **Retenu pour dengon** : managed flooding + TTL 7 au MVP ; **Spray-and-Wait
> appliqué au budget de copies des enveloppes scellées** comme cible v2 (voir
> A-13 et [`05-protocole-et-trame.md`](05-protocole-et-trame.md)).

## 4. Bitchat — la référence à copier

Publié en **juillet 2025** par Jack Dorsey. Fait *presque exactement* ce que
décrit le projet :

- réseau **mesh Bluetooth LE**, chaque téléphone client **et** serveur ;
- routage multi-sauts avec **TTL limité à 7 sauts** ;
- **mise en cache** des messages pour les pairs hors-ligne, **livraison
  automatique** dès réapparition (= le « stockage aux points de transmission ») ;
- **chiffrement de bout en bout** : X25519, AES-256-GCM, Ed25519, cadre **Noise**
  (motif XX) ;
- **identifiants éphémères** par session pour la vie privée.

Conséquence : on ne réinvente rien, on **s'inspire** de l'architecture Bitchat
(open source) + on ajoute la spécificité passerelle Arduino → dashboard. dengon
reprend explicitement Noise et le TTL 7. On **ne reprend pas de code** (Bitchat
est en Swift, dengon en Rust — voir C-10). Whitepaper :
`github.com/permissionlesstech/bitchat`.

## 5. Bridgefy (2020) — le contre-exemple à NE PAS reproduire

Bridgefy, app de messagerie mesh populaire pendant des manifestations, **cassée
par des chercheurs** (Albrecht, Blasco, Jensen, Mareková ; CT-RSA 2021 ; IACR
ePrint 2021/214).

| Faille trouvée | Cause | Ce que dengon doit faire |
| --- | --- | --- |
| **Aucune authentification** | pas de vérification d'identité → usurpation | signer avec **Ed25519**, vérifier les clés |
| **Man-in-the-middle** | identités contradictoires acceptées | *handshake* authentifié (Noise) |
| **Chiffrement cassé** | RSA **PKCS#1 v1.5** obsolète → *padding oracle* (Bleichenbacher) | **AEAD moderne** (ChaCha20-Poly1305), jamais de crypto maison |
| **Rejeu / falsification** | ciphertexts réordonnables/rejouables | **numéro de séquence + AEAD** (`conv_seq`) |
| **Désanonymisation** | identifiants en clair diffusés en continu | tags tournants (`recipient_tag`) |
| **Déni de service** | *bombe de compression* (gzip) | limiter la taille, méfiance sur compression **avant** chiffrement |

**Les 3 leçons universelles :** (1) ne jamais inventer sa propre cryptographie
— utiliser des bibliothèques éprouvées (libsodium/NaCl, Noise, Web Crypto) ;
(2) authentifier avant de chiffrer les identités, sinon MITM ; (3) attention à
compression + chiffrement, les combiner crée des oracles.

## 6. Bluetooth Classic vs BLE

| | Bluetooth Classic (BR/EDR) | BLE |
| --- | --- | --- |
| Usage typique | audio, transfert de flux (SPP série) | capteurs, IoT, messages courts |
| Débit | élevé (~1–3 Mbit/s) | faible (~0,1–1 Mbit/s utile) |
| Consommation | forte | très faible |
| Modèle | connexion point-à-point | connexion **et** diffusion (*advertising*) |
| Module Arduino courant | HC-05 / HC-06 (profil série SPP) | ESP32, nRF52, HM-10 |
| Adapté au mesh ? | mal | **oui** (advertising + GATT) |

Recommandation : viser le **BLE** (ce qu'utilisent Bitchat, Bridgefy ; plus
économe ; là où vit la norme *Bluetooth Mesh*). Le HC-05 (Classic/SPP) reste
possible pour un prototype point-à-point simple mais ne maille pas nativement.
BLE est aussi le **dénominateur commun** avec iOS.

## 7. Couches BLE utiles

- **GAP** (Generic Access Profile) : **découverte** et rôles (advertiser/scanner,
  puis central/peripheral). L'*advertising* signale la présence.
- **GATT** (Generic Attribute Profile) : structure les données en **services** et
  **caractéristiques** (boîtes lisibles/écrivables/notifiables). Un message
  dengon transite par l'écriture d'une caractéristique dédiée.
- **MTU** : taille max d'un paquet applicatif (souvent **~20 à 512 octets**).
  Les messages plus longs doivent être **fragmentés** puis réassemblés.

## 8. Norme *Bluetooth Mesh* (SIG, depuis 2017)

Spécification officielle du Bluetooth SIG pour des centaines d'objets. Routage =
**managed flooding** : message diffusé à tous les voisins ; chaque nœud relais
le retransmet ; anti-boucle par **TTL** décrémenté + **cache des messages déjà
vus** (*network message cache*). Rôles clés :

- **Relay** : retransmet ;
- **Friend / Low-Power** : un nœud « ami » **stocke** les messages destinés à un
  nœud basse consommation endormi — **un vrai store-and-forward déjà prévu par la
  norme** (= point 4 du cahier des charges, à citer) ;
- **Proxy** : permet à un smartphone GATT de parler au mesh.

Évolution : **Directed Forwarding** — crée de vraies **routes** au lieu de tout
inonder (plus efficace sur grands réseaux).

**Pourquoi dengon ne l'adopte pas** : c'est un modèle *publish/subscribe* avec
sécurité par clés « réseau »/« application » partagées — **pas** le modèle dengon
(1-à-1, E2E, accusés qui remontent vers l'expéditeur). L'adopter = adopter tout
son modèle. Mieux vaut partir de **NimBLE brut** (GATT + annonce) et
**s'inspirer** des idées.

## 9. Analyse critique « faut-il une blockchain ? »

Une blockchain « complète » combine : (1) hachages cryptographiques ; (2)
**chaînage** (chaque bloc contient le hash du précédent → immuabilité + ordre) ;
(3) **arbres de Merkle** (résumer plein de données par une racine) ; (4)
**réplication** (tout le monde a une copie) ; (5) **consensus** (PoW/PoS…) pour
que des acteurs qui ne se font pas confiance s'accordent sur **un** historique
sans autorité centrale. Le point (5) est ce qui rend la blockchain *lourde,
lente, coûteuse* — et ce dont dengon **n'a pas besoin**.

**Ce que la blockchain résout :** accord sur un historique unique, ordonné,
infalsifiable, entre parties sans confiance mutuelle et sans tiers de confiance.
dengon n'a **pas** ce problème (messages privés → confidentialité, pas registre
public ; relais qui transportent sans lire → E2EE + intégrité, pas consensus ;
livrer « au mieux » malgré les ruptures → store-carry-forward, pas registre
répliqué global).

| Contrainte dengon | Ce qu'impose une blockchain | Verdict |
| --- | --- | --- |
| Messages **privés** entre 2 personnes | registre **partagé/visible** par les nœuds | ❌ contredit la confidentialité |
| Nœuds **contraints** (téléphone, Arduino, batterie) | consensus **coûteux** | ❌ trop lourd pour un ESP32 |
| Réseau **intermittent** | consensus a besoin d'une **large connectivité** simultanée | ❌ incompatible DTN |
| On veut **oublier** les messages livrés (purge) | registre **append-only immuable** | ❌ à l'opposé du besoin |
| Passer à l'échelle sur peu de nœuds | registre qui **grossit indéfiniment** | ❌ gaspillage de stockage |

**Ce qui est vraiment utile** — les briques cryptographiques **sans** le
consensus :

- **Chaînage par hash** : chaque entrée contient le hash de la précédente →
  intégrité + ordre (impossible d'insérer/retirer/réordonner sans casser la
  chaîne, le récepteur détecte le trou) ; détection de censure ; **léger** (juste
  des hachages, un ESP32 le fait sans problème).

  ```text
  msg1 : {texte, prev = 0000}   h1 = hash(msg1)
  msg2 : {texte, prev = h1  }   h2 = hash(msg2)
  msg3 : {texte, prev = h2  }   h3 = hash(msg3)
  ```

- **Arbres de Merkle** : résumer un lot par une **racine** → **preuve
  d'inclusion compacte** (prouver qu'un message est dans un lot avec seulement
  **log₂(n)** hachages). Usages : accusé de réception groupé ; vérification
  d'intégrité d'un message fragmenté.
- **Signatures & horodatage** (Ed25519) : combinés au chaînage → preuve
  d'origine + d'ordre = 90 % de ce qu'on attend d'une « blockchain » pour de la
  messagerie, **sans** la blockchain.

**Formulation retenue** :

> « Nous nous inspirons des **structures de données de la blockchain** (chaînage
> cryptographique et arbres de Merkle) pour garantir l'intégrité et l'ordre des
> messages, **sans** recourir à un mécanisme de consensus distribué, inadapté à
> un réseau Bluetooth intermittent composé de nœuds contraints. »

**Repli si l'énoncé impose vraiment « une blockchain »** : une **blockchain
locale, légère, à autorité de confiance** (pas de PoW) — un registre chaîné des
**événements** (message émis/relayé/livré), pas du contenu ; hébergé/agrégé par
la passerelle → dashboard ; consensus remplacé par la **signature** de chaque
nœud (preuve d'autorité simplifiée). C'est le même code que le **journal chaîné
signé** retenu.

> **Décision A-15 / A-7 (tranché)** : dengon implémente un **journal chaîné signé
> par appareil, agrégé et audité par le dashboard, sans consensus** (arbre de
> Merkle optionnel pour les accusés groupés / l'intégrité des fragments). Détail
> dans [`06-securite.md`](06-securite.md) §4. Le repli lexical ci-dessus s'emploie
> **sans changer le code** si un enseignant exige littéralement le mot
> « blockchain ».

## 10. Ce que le projet retient de tout ça

Le réseau est un **DTN à routage épidémique / gossip** : store-carry-forward,
échange épidémique (deux nœuds se rencontrent → échangent tout ce qu'ils n'ont
pas en commun), flooding contrôlé borné par **TTL** + **fenêtre de
déduplication** + (cible v2) **budget de copies** pour les envois ciblés. Lignée
de recherche : Vahdat & Becker *Epidemic Routing* (2000), PRoPHET,
Spray-and-Wait. Messageries réelles : Briar, Bridgefy, bitchat.

Traits « blockchain-like » **présents** : pas de serveur central (identité = clé
publique, `peerID = SHA-256(pub_static)[0..8]`) ; propagation gossip P2P ;
messages signés (Ed25519 par paquet) ; anti-rejeu/anti-doublon par hash de
contenu (`msgID` + seen-set) ; registre append-only chaîné par hash par appareil
(`hash_n = H(entry_n ‖ prev_hash)`) ; réconciliation d'état par inventaire d'IDs
(MVP) puis filtres compacts GCS (cible v2).

Traits **non empruntés** : consensus global (PoW/PoS/BFT) — incompatible
offline-first, coût prohibitif ; chaîne globale unique / ordre total — un ordre
**causal par conversation** suffit ; résistance Sybil / PoW à l'entrée — pas de
double-dépense, messages idempotents ; réplication totale du registre — chaque
nœud n'a besoin que de ses conversations + un cache court ; smart contracts / VM
— hors sujet.
