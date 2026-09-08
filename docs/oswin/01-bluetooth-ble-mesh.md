# 01 — Bluetooth, BLE & réseau mesh

## 1. Bluetooth Classic vs Bluetooth Low Energy (BLE)

Il existe deux « Bluetooth » qu'il ne faut pas confondre :

| | **Bluetooth Classic (BR/EDR)** | **Bluetooth Low Energy (BLE)** |
|---|---|---|
| Usage typique | audio, transfert de flux (SPP série) | capteurs, objets connectés, messages courts |
| Débit | élevé (~1–3 Mbit/s) | faible (~0,1–1 Mbit/s utile) |
| Consommation | forte | très faible |
| Modèle | connexion point-à-point | connexion **et** diffusion (*advertising*) |
| Module Arduino courant | **HC-05 / HC-06** (profil série SPP) | **ESP32**, nRF52, HM-10 |
| Adapté au mesh ? | mal | **oui** (advertising + GATT) |

**Recommandation pour dengon :** viser le **BLE**. C'est ce qu'utilisent toutes les apps de
messagerie mesh (Bitchat, Bridgefy), c'est plus économe, et c'est là que se trouve la
norme *Bluetooth Mesh*. Le HC-05 (Classic/SPP) reste possible pour un **prototype
Arduino simple** point-à-point, mais il ne « maille » pas nativement (voir fiche 05).

## 2. Les couches BLE utiles à connaître

- **GAP (Generic Access Profile)** : gère la **découverte** et les rôles
  (*advertiser*/*scanner*, puis *central*/*peripheral*). C'est par l'*advertising* qu'un
  appareil signale sa présence.
- **GATT (Generic Attribute Profile)** : structure les **données échangées** en
  *services* et *caractéristiques* (des « boîtes » lisibles/écrivables/notifiables). Un
  message dengon transiterait typiquement par l'écriture d'une caractéristique dédiée.
- **MTU** : taille max d'un paquet applicatif (souvent **~20 à 512 octets** selon les
  appareils). Les messages plus longs doivent être **fragmentés** puis réassemblés — point
  d'attention concret pour le format de message (fiche 03).

## 3. Deux façons de faire du « multi-sauts » en BLE

### Option A — La norme *Bluetooth Mesh* (SIG, depuis 2017)

Bluetooth Mesh est une **spécification officielle** du Bluetooth SIG conçue pour faire
communiquer des centaines d'objets. Son principe de routage est le **managed flooding**
(« inondation maîtrisée ») :

- un message est **diffusé** à tous les voisins ;
- chaque nœud **relais** le retransmet à son tour ;
- pour éviter que ça tourne en boucle, chaque message porte un **TTL** (décrémenté à chaque
  saut) et chaque nœud garde un **cache des messages déjà vus** (*network message cache*)
  qu'il ne rediffuse pas une seconde fois ;
- les rôles clés : **Relay** (retransmet), **Friend/Low-Power** (un nœud « ami » **stocke
  les messages** destinés à un nœud basse consommation endormi — un vrai *store-and-forward*
  déjà prévu par la norme !), **Proxy** (permet à un smartphone GATT de parler au mesh).

La norme prévoit aussi une évolution, le **Directed Forwarding**, qui crée de vraies
**routes** au lieu de tout inonder (plus efficace sur les grands réseaux).

*Sources : [Bluetooth Mesh Networking primer](https://www.bluetooth.com/bluetooth-mesh-networking-primer/),
[Novel Bits – guide mesh](https://novelbits.io/bluetooth-mesh-networking-the-ultimate-guide/),
[Bluetooth Mesh Directed Forwarding](https://www.bluetooth.com/mesh-directed-forwarding/),
[MokoSmart – What is Bluetooth Mesh](https://www.mokosmart.com/what-is-bluetooth-mesh-how-it-works/).*

**Le concept « Friend node » de la norme est exactement le point 4 du cahier des charges**
(« stocker le message tant que le destinataire ne l'a pas reçu »). À citer dans le rapport.

### Option B — Un mesh « maison » par-dessus BLE (comme Bitchat)

Plutôt que d'implémenter toute la norme Bluetooth Mesh (lourde), les apps de messagerie
définissent **leur propre protocole binaire simple** au-dessus du BLE brut :

- chaque appareil est **central + peripheral** en même temps et scanne en continu ;
- chaque message a un **ID unique** + un **TTL** (Bitchat = **7 sauts max**) ;
- à la réception d'un message *jamais vu*, le nœud le **stocke, décrémente le TTL, et le
  retransmet** à ses voisins ; s'il l'a déjà vu, il l'ignore (anti-boucle) ;
- les messages destinés à un pair absent sont **mis en cache** et **rejoués** à sa
  reconnexion.

C'est **exactement** l'algorithme de *managed flooding*, mais réécrit à la main. **C'est
l'option recommandée pour un projet d'école** : plus simple à comprendre, à coder et à
expliquer que la pile Bluetooth Mesh complète.

*Source : [dev.to – Bitchat](https://dev.to/grenishrai/offline-messaging-reinvented-with-bitchat-5011),
[TechRadar – how Bitchat works](https://www.techradar.com/phones/bitchat-is-a-new-private-bluetooth-messaging-app-that-doesnt-need-the-internet-heres-how-it-works).*

## 4. L'algorithme de relais, en pseudo-code

```text
à la réception d'un paquet P (venant d'un voisin) :
    si P.id est dans mon cache "déjà vu" :
        ignorer            # anti-boucle / anti-tempête de diffusion
    sinon :
        ajouter P.id au cache "déjà vu"
        si P.destinataire == moi :
            déchiffrer, vérifier la signature, afficher
            émettre un ACK(P.id) dans le mesh
        sinon :
            stocker P dans ma file locale       # store
            P.ttl = P.ttl - 1
            si P.ttl > 0 et P non expiré :
                pour chaque voisin BLE actuel :
                    envoyer P                    # forward
```

Deux garde-fous indispensables (leçons de la théorie du *broadcast storm*) :

- **cache des messages vus** → évite les boucles infinies et l'effondrement du réseau ;
- **TTL** (en sauts **et** en temps) → garantit qu'un message finit par disparaître.

## 5. Portée & topologie réalistes

- Portée BLE **par saut** : **~10 m** en intérieur encombré, jusqu'à **~30 m** en champ
  ouvert (chiffres cohérents avec les 30 m annoncés pour Bitchat). Le mesh sert à
  **cumuler** les portées : 3 sauts ≈ potentiellement ~100 m de bout en bout.
- Le débit et la latence se dégradent avec le nombre de sauts et de nœuds : viser des
  **messages courts** (texte), pas du transfert de fichiers.
- Pour la démo : disposer les nœuds de façon à ce que **A ne « voie » pas B directement**,
  pour forcer au moins **un relais** au milieu (idéalement l'Arduino) → ça prouve le mesh.

## 6. Choix technique conseillé pour dengon

- **Appareils utilisateurs** : smartphones ou PC en **BLE**, protocole binaire maison à la
  Bitchat (option B).
- **Nœud Arduino** : **ESP32** (BLE natif + Wi-Fi pour la passerelle) plutôt que HC-05.
  Voir fiche 05 pour la justification.
- **Réutiliser** le code open source de Bitchat comme référence d'architecture (format de
  paquet, TTL, cache), plutôt que partir d'une page blanche.

---

### À retenir
Le multi-sauts BLE, c'est du **managed flooding** : diffuser + relayer avec **TTL** et
**cache anti-doublon**. La norme *Bluetooth Mesh* le formalise (et son rôle **Friend node**
fait déjà du stockage), mais pour un projet d'école un **protocole maison à la Bitchat**
est plus pédagogique et suffisant.
