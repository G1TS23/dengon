# Spécification du protocole — dengon (brouillon v0.3)

> Premier jet le 2026-09-08.
> v0.2 : alignement sur Meshtastic (limite de sauts à 7, « écouter avant de
> rediffuser ») suite à `etude-stack.md`.
> v0.3 : arbitrage des points ouverts avec Olivier (ordre d'affichage, taille
> de file, échange d'inventaire, anti-inondation, délais par défaut).
> À relire et compléter avec Paul et Tanguy.
> S'appuie sur `CONTEXT.md`, `analyse-besoins.md`, `decisions-v1.md` et
> `etude-stack.md`. Les choix marqués « (ouvert) » ne sont pas tranchés ;
> les valeurs chiffrées « (à calibrer) » sont des points de départ.

---

## 1. But et périmètre

Ce document décrit **comment les appareils s'échangent les messages** dans
dengon, sans internet, en se relayant de proche en proche par Bluetooth.

Périmètre de la v1 (voir `decisions-v1.md`) :

- Messages **1-à-1** uniquement, **texte court** (~140 à 500 caractères).
- Appareils : **téléphones Android** et **cartes ESP32**.
- Liaison radio : **Bluetooth basse consommation (BLE)**.
- Contenu **chiffré de bout en bout** : un appareil qui relaie ne peut pas lire.
- Statuts gérés : *En attente → Parti → Distribué* (le *Lu* est reporté en v2).

Hors périmètre v1 : groupes, diffusion générale, pièces jointes, iPhone,
secret persistant (*forward secrecy*).

## 2. Vocabulaire

| Terme | Sens |
|-------|------|
| **Nœud** | Un appareil qui fait tourner dengon (téléphone ou ESP32). |
| **Voisin** | Un nœud actuellement à portée Bluetooth directe. |
| **Saut** (*hop*) | Un passage d'un nœud à un voisin. |
| **Message** | Le texte qu'un utilisateur envoie à un autre. |
| **Trame** | L'unité que s'échangent deux nœuds sur la radio (un message tient en une ou plusieurs trames). |
| **Relais** | Un nœud qui transmet une trame qui ne lui est pas destinée. |
| **File de retransmission** | La réserve de messages qu'un nœud garde pour les repasser à de futurs voisins. |
| **Table « déjà vu »** | La liste des identifiants de messages qu'un nœud a déjà traités. |
| **Empreinte de clé** | Un court identifiant dérivé de la clé publique d'un utilisateur. |

## 3. Identités et clés

- Chaque utilisateur possède une **paire de clés** (une privée, gardée secrète
  sur l'appareil ; une publique, partageable).
- L'ajout d'un contact se fait **en personne** : on scanne le QR code de l'autre
  (il contient sa clé publique + son pseudo), puis les deux téléphones affichent
  un **code court identique** que l'on compare de visu pour être sûr qu'il n'y a
  pas eu de falsification.
- L'**empreinte de clé** (par ex. les 16 premiers octets d'un hachage de la clé
  publique) sert d'adresse dans les trames : « pour l'empreinte X », « de la part
  de l'empreinte Y ».
- Le **pseudo** est purement cosmétique et n'est jamais utilisé pour l'adressage.

## 4. Transport Bluetooth (BLE)

- Chaque nœud est **à la fois visible et chercheur** : il s'annonce (pour être
  trouvé) et scanne (pour trouver les autres).
- Quand deux nœuds se trouvent, ils ouvrent une **connexion BLE courte**,
  s'échangent les trames en attente, puis se déconnectent.
- Le BLE transporte peu de données par envoi (souvent ~20 à ~240 octets utiles
  selon la négociation). Les messages plus gros sont donc **découpés en
  fragments** (section 8).
- Service et caractéristiques BLE précis : **(ouvert)** — à définir côté
  implémentation (une caractéristique pour écrire une trame, une pour en
  recevoir).

## 5. Format d'une trame

> **La disposition exacte des octets est spécifiée dans `format-trame.md`**
> (document qui fait foi). Cette section n'en donne que le principe.

Trois natures de PDU en v1 :

- **DONNÉES** : transporte un message (ou, après découpage, un fragment).
- **ACCUSÉ** : remonte une confirmation vers l'expéditeur.
- **INVENTAIRE** : liste d'identifiants échangée entre voisins qui se
  rencontrent (section 9).

### En-tête commun (en clair, lisible par les relais)

| Champ | Rôle |
|-------|------|
| Version du protocole | Compatibilité. |
| Type de PDU | DONNÉES / ACCUSÉ / INVENTAIRE. |
| Identifiant de message (16 octets) | Unique et **stable sur tout le réseau** ; déduplication et corrélation de l'accusé. |
| Empreinte expéditeur / destinataire (16 octets chacune) | Adressage (inversé pour un ACCUSÉ — voir `format-trame.md` §3). |
| Sauts restants (*TTL*) | Départ **7** ; −1 par relais ; à 0 on ne relaie plus. |
| Horodatage d'envoi | *Best-effort* : affichage et expiration (~24 h). |
| Index / nombre de fragments | Réassemblage (section 8). |

### Charge utile

- **DONNÉES** : `nonce` + `étiquette d'authenticité` + `contenu chiffré`
  (illisible **et** non modifiable par un relais).
- **ACCUSÉ** : voir section 7 (chiffré pour l'auteur d'origine).
- **INVENTAIRE** : liste d'identifiants en clair (aucun contenu).

## 6. Circulation des messages (cœur du protocole)

Principe : **diffusion contrôlée** (chaque nœud rediffuse à tous ses voisins),
bornée par le TTL et la déduplication. Pas de table de routage en v1.

### À la réception d'une trame DONNÉES

1. **Déjà vu ?** Si l'identifiant de message est dans la table « déjà vu » →
   on ignore la trame.
2. **Message expiré ?** Si `maintenant − horodatage` dépasse **~24 h** → on
   ignore et on purge d'éventuels fragments gardés.
2 bis. **Limite anti-inondation** : si ce voisin nous a déjà envoyé **plus de
   ~20 nouveaux messages sur la dernière minute** (**valeur à calibrer**), on
   ignore les suivants venant de lui jusqu'à ce que le débit retombe. Protège
   contre un nœud qui tente de saturer le réseau (voir section 10).
3. **Réassemblage** : si le message est fragmenté, on met le fragment de côté ;
   tant que tous les fragments ne sont pas là, on s'arrête ici.
4. **Suis-je le destinataire ?**
   - **Oui** → on déchiffre, on affiche le message, on **émet un ACCUSÉ**
     (section 7), on note l'identifiant en « déjà vu ».
   - **Non** → on passe au relayage.
5. **Relayage** (si je ne suis pas le destinataire) :
   - si `sauts restants = 0` → on **ne relaie pas** ;
   - sinon → on décrémente `sauts restants`, on **place le message dans la file
     de retransmission**, et on le **réémet vers tous les voisins**, sauf celui
     qui vient de nous l'envoyer.
   - **Écouter avant de rediffuser** : avant de réémettre, attendre un court
     délai aléatoire (**~50 à ~500 ms, à calibrer**) ; si on entend un voisin
     rediffuser **déjà** ce même message, **s'abstenir**. Réduit les tempêtes de
     rediffusion quand beaucoup de nœuds sont à portée (idée reprise de
     Meshtastic — `etude-stack.md` §5).
6. On note l'identifiant dans la table « déjà vu » (avec l'heure, pour pouvoir
   l'oublier après ~24 h).

### Conditions d'arrêt (empêchent un message de tourner sans fin)

- **TTL épuisé** : `sauts restants = 0`.
- **Déjà vu** : l'identifiant est connu → on ne rediffuse pas une 2ᵉ fois.
- **Expiration** : message plus vieux que ~24 h.
- **Accusé passé par là** : si un ACCUSÉ pour ce message transite par le nœud,
  le message est retiré de la file de retransmission (section 7).

## 7. Accusés et remontée vers l'expéditeur

- Quand le **destinataire** reçoit et déchiffre le message, il crée une trame
  **ACCUSÉ** contenant :
  - l'identifiant du message concerné,
  - le statut : `DISTRIBUÉ`,
  - une **signature du destinataire** sur (identifiant + statut + heure), pour que
    l'expéditeur puisse y faire confiance,
  - le tout **chiffré à destination de l'expéditeur d'origine** (les relais ne
    voient donc pas le contenu de l'accusé, juste qu'un accusé circule).
- L'ACCUSÉ **se diffuse dans le réseau de la même façon** qu'un message
  (son propre TTL, sa propre déduplication).
- **Optimisation utile** : tout relais qui voit passer un ACCUSÉ pour le message X
  peut **retirer X de sa file de retransmission** — ça réduit le trafic inutile.
- À réception de l'ACCUSÉ, l'expéditeur passe le message à l'état **Distribué**.

### Correspondance avec les statuts affichés

| Statut | Déclencheur |
|--------|-------------|
| **En attente** | Message créé, pas encore transmis à un voisin. |
| **Parti** | Transmis à au moins un voisin (il a quitté l'appareil). |
| **Distribué** | ACCUSÉ reçu de la part du destinataire. |
| **Échec** | Plus de ~24 h sans ACCUSÉ. |
| **Lu** | *v2* : un 2ᵉ type d'accusé, émis quand l'utilisateur ouvre le message. |

### Ordre d'affichage dans une conversation

Les messages d'une même conversation peuvent arriver **dans le désordre** (voies
différentes, store-and-forward). En v1, on les affiche **dans l'ordre où le
téléphone les reçoit** (pas de retri par horodatage). Simple et sans surprise
sur les horloges ; un message très en retard apparaît donc en bas de la
conversation. *(Un tri par horodatage d'envoi avec tolérance est noté comme
évolution possible.)*

## 8. Découpage et réassemblage

- Un message chiffré peut dépasser ce qu'une trame BLE transporte → on le
  **découpe en fragments** de la taille négociée avec le voisin.
- Chaque fragment porte : l'identifiant du message, son **index** et le **nombre
  total** de fragments.
- Le récepteur **rassemble** les fragments par identifiant de message, avec un
  **délai maximum d'attente de ~30 s (à calibrer)** ; passé ce délai, fragments
  incomplets jetés.
- En v1, un nœud **rassemble le message complet avant de le relayer** (plus
  simple pour vérifier le destinataire et dédupliquer). Coût : il faut avoir reçu
  tous les fragments pour retransmettre. Acceptable pour du texte court.
  *Alternative (relayer les fragments à l'aveugle) : notée comme optimisation
  possible.*

Ordre de grandeur : un message de 500 caractères + en-têtes + chiffrement tient
en **2 à 6 fragments** selon la liaison.

## 9. Store-and-forward (tolérance à la déconnexion)

- La **file de retransmission** garde les messages pas encore connus comme
  distribués.
- **Quand un nouveau voisin apparaît**, les deux nœuds **échangent d'abord la
  liste des identifiants de messages qu'ils détiennent** (« inventaire »), puis
  chacun n'envoie que ce qui **manque** à l'autre. Évite de renvoyer en boucle
  des messages déjà connus. *(choix v1 — l'alternative « pousser toute la file »
  reste le repli si l'inventaire pose problème à l'implémentation.)*
- **Taille maximale de la file** : **~50 messages sur ESP32**, **~300 sur
  téléphone** (**à calibrer** selon la mémoire réelle). Quand la file est
  pleine, on **retire le message le plus ancien** (le plus proche de son
  expiration).
- Un message **sort de la file** quand : un ACCUSÉ le concernant passe par le
  nœud, **ou** il expire (~24 h), **ou** il est évincé (file pleine).

## 10. Sécurité — ce que le protocole garantit et ne garantit pas

**Garanti en v1 :**

- Un relais **ne peut pas lire** le contenu d'un message (chiffrement de bout en
  bout).
- Un relais **ne peut pas modifier** un message sans que ça se voie (étiquette
  d'authenticité / signature).
- L'expéditeur affiché est bien celui qui détient la clé privée correspondante.
- Un même message **rejoué** plus tard est reconnu (identifiant + « déjà vu »).

**Non garanti en v1 (limites à assumer et à écrire dans la doc sécurité) :**

- **Métadonnées** : les empreintes expéditeur/destinataire et les horaires sont
  en clair dans l'en-tête → un relais voit *qui parle à qui* et *quand*.
- **Pas de secret persistant** : si une clé privée est volée, les anciens
  messages capturés deviennent lisibles.
- **Horodatages non vérifiables** par les relais, et horloges parfois fausses.
- **Inondation malveillante** : atténuée en v1 par la **limite anti-inondation**
  (section 6, étape 2 bis) — un voisin trop bavard est temporairement ignoré.
  Cela ne bloque pas un attaquant qui change d'identité ; protection plus forte
  = évolution.
- **Analyse de trafic** (observer les flux radio) reste possible.

## 11. Exemple (scénario campus)

```
Alice veut écrire à Bob. Bob n'est pas à portée directe. Carole est entre les deux.

1. Alice  : crée le message, le chiffre pour Bob, statut « En attente ».
2. Alice→Carole : Carole reçoit la trame. Elle n'est pas destinataire,
                  sauts restants 7→6, elle met en file et rediffuse.
                  Alice a transmis à un voisin → statut « Parti ».
3. Carole→Bob   : Bob reçoit, déchiffre, affiche. Bob émet un ACCUSÉ
                  (signé, chiffré pour Alice).
4. Bob→Carole→Alice : l'ACCUSÉ remonte. En le voyant passer, Carole retire
                      le message de sa file.
5. Alice : ACCUSÉ reçu → statut « Distribué ».
```

## 12. Points ouverts (à trancher)

**Tranchés en v0.3 :** ordre d'affichage (= ordre d'arrivée) · taille de file
(~50 ESP32 / ~300 téléphone, éviction du plus ancien) · échange d'inventaire
entre voisins (= v1) · limite anti-inondation (= v1, ~20 msg/min/voisin).

**Encore ouverts :**

- Valeurs à calibrer sur le terrain : délai « écouter avant de rediffuser »
  (~50-500 ms), délai de réassemblage (~30 s), seuil anti-inondation
  (~20 msg/min/voisin), tailles de file.
- Service / caractéristiques BLE exacts (voir `etude-stack.md` §1) — décision
  d'implémentation.
- Format de l'échange d'inventaire (comment lister les identifiants de façon
  compacte).
- Choix précis des primitives cryptographiques (à détailler dans la doc
  sécurité) — piste : libsodium `crypto_box`, voir `etude-stack.md` §4.
