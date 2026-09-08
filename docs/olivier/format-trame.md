# Format binaire des trames — dengon (brouillon v0.1)

> Rédigé le 2026-09-08. **Ce document fait foi** sur la structure des données
> échangées : les deux implémentations (téléphone et ESP32) doivent produire et
> lire exactement ces octets. Voir `etude-stack.md` §3.
> Complète `protocole.md` (v0.3), qui décrit le comportement ; ici, on décrit la
> forme.
> Valeurs « (à calibrer) » = points de départ, ajustables après essais.

---

## 0. Conventions

- **Ordre des octets** : *big-endian* (ordre réseau) pour tout entier de plus
  d'un octet.
- **Entiers** non signés sauf mention contraire.
- Les tailles sont en **octets**.
- Un champ « réservé » est mis à 0 à l'émission et ignoré à la réception (marge
  pour les versions futures).

## 1. Les deux niveaux

1. **PDU dengon** = l'unité complète du protocole (en-tête + corps), avant
   découpage. C'est ce qui est chiffré, relayé, dédupliqué.
2. **Fragment BLE** = un morceau de PDU envoyé en **un seul envoi Bluetooth**.
   Un PDU est découpé en 1 à N fragments (voir `protocole.md` §8).

```
   PDU dengon  ──découpage──▶  [Frag 0][Frag 1]…[Frag N-1]  ──BLE──▶ voisin
   voisin  ──BLE──▶  [Frag 0][Frag 1]…[Frag N-1]  ──recollage──▶  PDU dengon
```

## 2. Fragment BLE

| Décalage | Taille | Champ | Description |
|---:|---:|---|---|
| 0 | 1 | `version_proto` | Version du protocole. **= 1**. |
| 1 | 1 | `drapeaux` | bit 0 = corps chiffré ; bits 1-7 réservés. |
| 2 | 16 | `id_message` | Identifiant **unique et stable** du message sur tout le réseau (128 bits aléatoires). Sert à la déduplication et à corréler l'accusé. **N'est jamais modifié par un relais.** |
| 18 | 2 | `index_fragment` | Numéro du fragment, à partir de 0. |
| 20 | 2 | `nombre_fragments` | Total de fragments du PDU (≥ 1). |
| 22 | 2 | `longueur_charge` | Nombre d'octets utiles dans ce fragment. |
| 24 | `longueur_charge` | `charge_fragment` | Tranche du PDU dengon. |

**En-tête fragment = 24 octets.** Avec une taille utile BLE négociée d'environ
**180 octets**, il reste ~**156 octets** de charge par fragment (**à calibrer**
selon le MTU réel).

Règle de recollage : concaténer les `charge_fragment` dans l'ordre des
`index_fragment`, de 0 à `nombre_fragments − 1`. Délai maximum d'attente des
fragments manquants : **~30 s (à calibrer)** ; au-delà, on jette.

## 3. PDU dengon — en-tête commun

Obtenu après recollage des fragments.

| Décalage | Taille | Champ | Description |
|---:|---:|---|---|
| 0 | 1 | `type_pdu` | **1** = DONNÉES, **2** = ACCUSÉ, **3** = INVENTAIRE. |
| 1 | 1 | `sauts_restants` | TTL. **7** à l'émission (voir `protocole.md` §5). Décrémenté de 1 par chaque relais. À 0, on ne relaie plus. |
| 2 | 16 | `empreinte_expediteur` | Empreinte de clé publique de l'émetteur du PDU. |
| 18 | 16 | `empreinte_destinataire` | Empreinte de clé publique du destinataire du PDU. |
| 34 | 8 | `horodatage_envoi` | Millisecondes depuis le 1ᵉʳ janvier 1970 (UTC). *Best-effort* : sert à l'affichage et à l'expiration (~24 h), pas de garantie. |
| 42 | 1 | `version_contenu` | Version du format du corps. **= 1**. |
| 43 | 1 | réservé | 0. |
| 44 | … | `corps` | Dépend de `type_pdu` (sections 4 à 6). |

**En-tête PDU = 44 octets.**

> Adressage : pour un PDU **DONNÉES**, `empreinte_expediteur` = l'auteur du
> message et `empreinte_destinataire` = son lecteur. Pour un PDU **ACCUSÉ**,
> c'est **l'inverse** : l'émetteur de l'accusé est le lecteur du message, et le
> destinataire de l'accusé est l'auteur d'origine.

## 4. Corps d'un PDU DONNÉES (`type_pdu` = 1)

Le bit « chiffré » des `drapeaux` est à 1 (cas normal en v1).

| Décalage (dans le corps) | Taille | Champ | Description |
|---:|---:|---|---|
| 0 | 24 | `nonce` | Aléa unique par PDU pour le chiffrement authentifié. |
| 24 | 16 | `etiquette_auth` | Code d'authentification (empêche la lecture *et* la modification par un relais). |
| 40 | M | `contenu_chiffre` | Le texte du message, chiffré. |

Une fois déchiffré, `contenu_chiffre` donne le **texte en clair, en UTF-8**
(≤ ~500 caractères, soit ≤ ~1000 octets — voir `decisions-v1.md`).

Primitive envisagée : **libsodium `crypto_box`** (X25519 + chiffrement
authentifié) entre la clé privée de l'expéditeur et la clé publique du
destinataire, échangées en personne (`etude-stack.md` §4, `protocole.md` §3).
La disposition exacte `nonce` / `etiquette_auth` / `contenu_chiffre` suit la
convention de la primitive retenue — **à figer dans la doc sécurité**.

## 5. Corps d'un PDU ACCUSÉ (`type_pdu` = 2)

Un ACCUSÉ a **son propre** `id_message` (pour sa propre déduplication) et
référence le message acquitté dans son corps. Il est **chiffré pour l'auteur
d'origine**, de la même façon qu'un PDU DONNÉES :

| Décalage (corps) | Taille | Champ | Description |
|---:|---:|---|---|
| 0 | 24 | `nonce` | Voir section 4. |
| 24 | 16 | `etiquette_auth` | Voir section 4. |
| 40 | P | `contenu_chiffre` | Contenu de l'accusé, chiffré (structure ci-dessous). |

Contenu de l'accusé **en clair** (après déchiffrement) :

| Décalage | Taille | Champ | Description |
|---:|---:|---|---|
| 0 | 16 | `id_message_confirme` | L'`id_message` du PDU DONNÉES acquitté. |
| 16 | 1 | `statut` | **3** = DISTRIBUÉ (**4** = LU, réservé v2). |
| 17 | 8 | `horodatage_reception` | Millisecondes UTC de la réception par le destinataire. |
| 25 | 64 | `signature_destinataire` | Signature de (`id_message_confirme` ‖ `statut` ‖ `horodatage_reception`) par la clé du destinataire, pour que l'auteur puisse faire confiance à l'accusé. |

## 6. Corps d'un PDU INVENTAIRE (`type_pdu` = 3)

Échangé quand deux voisins se rencontrent (`protocole.md` §9). Il **n'est pas
chiffré** (il ne contient que des identifiants, aucun contenu) ; le bit
« chiffré » des `drapeaux` est à 0.

| Décalage (corps) | Taille | Champ | Description |
|---:|---:|---|---|
| 0 | 2 | `nombre_entrees` | Nombre d'identifiants listés. |
| 2 | 16 × `nombre_entrees` | `ids` | Les `id_message` que ce nœud détient actuellement dans sa file de retransmission. |

À réception, le nœud envoie à ce voisin les messages dont l'`id_message`
**n'apparaît pas** dans la liste reçue.

> Ordre de grandeur : une file pleine côté téléphone (~300 messages) = ~4,8 Ko
> d'identifiants, soit ~30 fragments. Acceptable en v1. Une représentation plus
> compacte (filtre de Bloom) est notée comme optimisation possible.

## 7. Constantes du protocole (v0.3)

| Constante | Valeur | Note |
|---|---|---|
| `version_proto` | 1 | |
| `version_contenu` | 1 | |
| TTL initial (`sauts_restants`) | 7 | aligné Meshtastic |
| Taille de `id_message` | 16 octets (128 bits) | aléatoire |
| Taille max du texte en clair | ~500 caractères (~1000 octets) | `decisions-v1.md` |
| Charge utile par fragment | ~156 octets | à calibrer selon MTU |
| Délai de réassemblage | ~30 s | à calibrer |
| Expiration d'un message | ~24 h | `protocole.md` §6 |
| Délai « écouter avant de rediffuser » | ~50-500 ms | à calibrer |
| Fenêtre anti-inondation | ~20 nouveaux `id_message` / min / voisin | à calibrer |
| Taille de la file de retransmission | ~50 (ESP32) / ~300 (téléphone) | à calibrer |

## 8. Exemple de tailles — message texte de 500 caractères

```
texte en clair              ~700 octets (UTF-8 courant, jusqu'à ~1000 au pire)
+ nonce (24) + étiquette (16)   40 octets
= contenu chiffré           ~740 octets
+ en-tête PDU                 44 octets
= PDU DONNÉES               ~784 octets
découpage en fragments de ~156 octets utiles :  ⌈784 / 156⌉ = 6 fragments
                                        (jusqu'à ~7 dans le pire cas)
```

## 9. Points ouverts

- Taille utile réelle d'un fragment (dépend du MTU BLE négocié — mesurer sur
  les appareils cibles).
- Disposition exacte `nonce` / `étiquette` / `contenu` selon la primitive
  cryptographique définitive (**doc sécurité**).
- Faut-il un champ de **somme de contrôle** par fragment, ou se repose-t-on sur
  le CRC intégré au BLE ?
- Représentation compacte de l'inventaire (filtre de Bloom) : v1 ou évolution ?
- Faut-il numéroter les PDU d'un même expéditeur (compteur anti-rejeu explicite)
  en plus de l'`id_message` ? — à trancher dans la doc sécurité.
