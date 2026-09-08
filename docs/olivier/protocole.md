# Spécification du protocole — dengon (brouillon v0.1)

> Premier jet, rédigé le 2026-09-08. À relire et compléter avec Paul et Tanguy.
> S'appuie sur `CONTEXT.md`, `analyse-besoins.md` et `decisions-v1.md`.
> Les choix marqués « (ouvert) » ne sont pas tranchés.

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

Deux natures de trames en v1 :

- **DONNÉES** : transporte un message (ou un fragment de message).
- **ACCUSÉ** : remonte une confirmation vers l'expéditeur.

### En-tête commun (en clair, lisible par les relais)

| Champ | Taille indicative | Rôle |
|-------|-------------------|------|
| Version | 1 octet | Version du protocole. |
| Type | 1 octet | DONNÉES ou ACCUSÉ. |
| Identifiant de message | 16 octets | Unique par message ; sert à la déduplication et à corréler l'accusé. |
| Empreinte expéditeur | 16 octets | Qui a créé le message. |
| Empreinte destinataire | 16 octets | À qui il est destiné. |
| Sauts restants (*TTL*) | 1 octet | Décrémenté à chaque relais ; à 0, on ne relaie plus. Valeur de départ : **8 (ouvert)**. |
| Horodatage d'envoi | 8 octets | Heure indiquée par l'expéditeur. **Best-effort** : sert à l'affichage et à l'expiration, pas de garantie. |
| Index / nombre de fragments | 2 + 2 octets | Pour le réassemblage (section 8). |

### Charge utile

- Pour une trame **DONNÉES** : le **contenu chiffré** du message + une **étiquette
  d'authenticité** (prouve que ça vient bien de l'expéditeur et que rien n'a été
  modifié).
- Pour une trame **ACCUSÉ** : voir section 7.

## 6. Circulation des messages (cœur du protocole)

Principe : **diffusion contrôlée** (chaque nœud rediffuse à tous ses voisins),
bornée par le TTL et la déduplication. Pas de table de routage en v1.

### À la réception d'une trame DONNÉES

1. **Déjà vu ?** Si l'identifiant de message est dans la table « déjà vu » →
   on ignore la trame.
2. **Message expiré ?** Si `maintenant − horodatage` dépasse **~24 h** → on
   ignore et on purge d'éventuels fragments gardés.
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

## 8. Découpage et réassemblage

- Un message chiffré peut dépasser ce qu'une trame BLE transporte → on le
  **découpe en fragments** de la taille négociée avec le voisin.
- Chaque fragment porte : l'identifiant du message, son **index** et le **nombre
  total** de fragments.
- Le récepteur **rassemble** les fragments par identifiant de message, avec un
  **délai maximum d'attente (ouvert)** ; passé ce délai, fragments incomplets
  jetés.
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
- **Quand un nouveau voisin apparaît**, on lui repropose le contenu de la file.
  - v1 : on **pousse tout** (le plus simple).
  - Optimisation : d'abord échanger « quels identifiants as-tu déjà ? » pour ne
    renvoyer que le manquant. **(ouvert)**
- **Taille maximale de la file (ouvert)** : ordre d'idée ~50 sur ESP32,
  ~200-500 sur téléphone. Politique d'éviction quand c'est plein **(ouvert)** :
  le plus vieux ? le plus retransmis ? le plus proche de l'expiration ?
- Un message **sort de la file** quand : un ACCUSÉ le concernant passe par le
  nœud, **ou** il expire (~24 h).

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
- **Inondation malveillante** : un nœud hostile peut saturer le réseau —
  limitation de débit **(ouvert)**.
- **Analyse de trafic** (observer les flux radio) reste possible.

## 11. Exemple (scénario campus)

```
Alice veut écrire à Bob. Bob n'est pas à portée directe. Carole est entre les deux.

1. Alice  : crée le message, le chiffre pour Bob, statut « En attente ».
2. Alice→Carole : Carole reçoit la trame. Elle n'est pas destinataire,
                  sauts restants 8→7, elle met en file et rediffuse.
                  Alice a transmis à un voisin → statut « Parti ».
3. Carole→Bob   : Bob reçoit, déchiffre, affiche. Bob émet un ACCUSÉ
                  (signé, chiffré pour Alice).
4. Bob→Carole→Alice : l'ACCUSÉ remonte. En le voyant passer, Carole retire
                      le message de sa file.
5. Alice : ACCUSÉ reçu → statut « Distribué ».
```

## 12. Points ouverts (à trancher)

- Valeur de départ du TTL (proposé : 8).
- Service / caractéristiques BLE exacts.
- Délai maximum d'attente pour le réassemblage.
- Taille maximale et politique d'éviction de la file de retransmission
  (lié au budget mémoire ESP32 — voir `decisions-v1.md`).
- Échange « quels identifiants as-tu ? » entre voisins : v1 ou plus tard.
- Limitation de débit contre l'inondation malveillante.
- Ordre d'affichage des messages reçus dans le désordre (par horodatage
  d'envoi, ou par ordre d'arrivée).
- Choix précis des primitives cryptographiques (à détailler dans la doc
  sécurité).
