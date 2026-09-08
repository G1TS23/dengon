# Analyse des besoins — compléments à `CONTEXT.md`

> Document de réflexion. Il ne remplace pas `CONTEXT.md`, il liste les
> besoins, questions et risques qui me semblent à cadrer « dans un premier
> temps » avant de choisir une techno ou d'écrire une ligne de code.

---

## 1. Identité et adressage

- Comment désigne-t-on un destinataire ? (clé publique, pseudo, identifiant court ?)
- Un message a-t-il **un** destinataire, **plusieurs** (groupe) ou est-il **diffusé à tous** ?
- Comment ajoute-t-on un contact ? Échange de clés en présentiel (QR code / NFC) ?
- Comment empêche-t-on l'usurpation d'un expéditeur ?
- Un même utilisateur peut-il avoir plusieurs appareils ? (probablement hors périmètre v1)

## 2. Format du message et du protocole

- **Le livrable central du projet est la spécification du protocole** (format binaire des trames)
  commun aux 3 familles d'appareils (Android, iOS, ESP32).
- Taille maximale d'un message ? BLE a un MTU faible (~20 à ~500 octets selon négociation)
  → besoin de **fragmentation / réassemblage** dès qu'on ajoute chiffrement + entêtes.
- Contenu : texte seul ? accusés de lecture ? pièces jointes (a priori non en v1) ?
- Entêtes nécessaires : ID unique de message, expéditeur, destinataire, TTL/sauts,
  horodatage, numéro de séquence / nonce.

## 3. Propagation dans le réseau (routage mesh)

- Stratégie : **flooding** (chaque nœud rediffuse) — simple mais coûteux — vs routage.
  Recommandation v1 : flooding contrôlé.
- **TTL / limite de sauts** pour éviter que les messages circulent indéfiniment.
- **Déduplication** : un nœud reçoit le même message par plusieurs voisins → table des ID déjà vus.
- Détection / cassure de boucles.
- **Accusé de livraison** : il doit *remonter* dans le mesh jusqu'à l'expéditeur
  (routage retour, plus difficile que l'aller) — nécessaire pour les statuts « Distribué » / « Lu ».
- Que devient un message dont le destinataire n'est jamais à portée ?
  → **durée de vie / expiration**, politique d'abandon.
- Priorité entre messages ?

## 4. Store-and-forward (réseau à connectivité intermittente)

- « Être déconnecté ne doit pas être bloquant » ⇒ chaque nœud **stocke** les messages
  non délivrés et les **rejoue** quand il croise de nouveaux voisins.
- Combien de temps garde-t-on un message ? Quelle taille de file ?
- **Contrainte forte sur l'ESP32** : RAM ~520 Ko, flash limitée → capacité de tampon réduite,
  politique d'éviction à définir (FIFO, par TTL, par priorité).
- Persistance locale sur mobile (base chiffrée) pour survivre à un redémarrage.

## 5. Sécurité (à détailler — `CONTEXT.md` reste très haut niveau)

- **Chiffrement de bout en bout du contenu** : les relais ne doivent jamais lire le message.
  Choisir une primitive éprouvée (ex. X25519 + AES-GCM / libsodium) plutôt que du fait-maison.
- **Intégrité / authenticité** : signature ou MAC pour qu'un relais ne puisse pas altérer.
- **Anti-rejeu** : nonce / compteur / horodatage.
- **Métadonnées** : même chiffré, « qui parle à qui » transparaît dans les entêtes de routage.
  Jusqu'où veut-on masquer l'expéditeur / le destinataire ?
- **Provisioning des clés** : appairage initial hors-bande (QR code en présentiel ?).
- **Stockage des clés sur ESP32** (NVS chiffrée ?) et sur mobile (Keystore / Keychain).
- Révocation / que faire si un appareil est compromis ou perdu ?
- Anti-spam / anti-DoS : un nœud malveillant qui inonde le réseau.
- Forward secrecy : probablement trop pour une v1, à noter comme évolution.

## 6. Contraintes des plateformes (à valider **avant** de s'engager)

- **iOS — risque technique n°1.** Le BLE en arrière-plan est très bridé par CoreBluetooth :
  pas de scan permanent, throttling, restrictions sur le scan par UUID en background.
  Un relais qui tourne « en tâche de fond » sur iPhone est incertain
  → **faire une preuve de faisabilité iOS dès le départ.**
- **Android** : permissions BLE + localisation, services *foreground* avec notification obligatoire,
  Doze mode, comportements variables selon constructeurs.
- **ESP32 FREENOVE WROOM** : BLE + Bluetooth Classic + Wi-Fi ; ressources limitées, pas d'OS.
  Évaluer le **Bluetooth Mesh natif d'ESP-IDF** vs un protocole maison.
- **BLE vs Bluetooth Classic** : le Classic offre plus de débit/portée mais n'est pas
  exploitable librement sur iOS → **BLE est le dénominateur commun**.
- Chaque téléphone doit être **central et périphérique en même temps** (scanner *et* émettre)
  pour relayer — faisable mais non trivial.
- Portée BLE réaliste en intérieur : ~10–30 m → la densité de nœuds conditionne tout.

## 7. Dashboard / observabilité

- Qui pousse les données vers le dashboard ? Les nœuds qui ont du Wi-Fi à cet instant.
- Où est hébergé le backend d'agrégation ? (le dashboard suppose une connexion,
  c'est un bonus *non bloquant* — à assumer clairement).
- Quelles données remontent ? **ID de message, statut, horodatage, sauts, nœuds traversés —
  jamais le contenu.** Attention : ces métadonnées sont sensibles (qui/quand/où).
- Reconstituer le trajet d'un message à partir de rapports **partiels** de plusieurs nœuds.
- **Horloge** : pas de temps global fiable dans un mesh hors-ligne → horloges désynchronisées.
  Prévoir une horloge logique (type Lamport) ou des horodatages best-effort assumés.
- Temps réel vs remontée par lots.

## 8. Expérience utilisateur

- Notification à la réception d'un message (encore la contrainte du fonctionnement en arrière-plan).
- Comment l'utilisateur voit-il qu'un message est « En attente » depuis longtemps ?
- Retour visuel sur l'état du réseau : nombre de voisins, « à portée » / « isolé ».
- Historique des conversations, persistance locale.
- Onboarding : ajout de contact et échange de clés en présentiel.
- Mode dégradé quand l'appareil est seul.

## 9. Cadrage projet / non-fonctionnel

- **Écrire 2–3 scénarios d'usage concrets** (festival, zone blanche, manifestation,
  catastrophe naturelle, campus…) : ils cadreront la plupart des choix techniques.
- Définir un **MVP** resserré — proposition :
  - 2 téléphones Android + 1 ESP32 en relais ;
  - message texte 1-à-1, chiffré de bout en bout ;
  - flooding + TTL + déduplication ;
  - statuts « En attente / Parti / Distribué » (le « Lu » et le dashboard en second temps).
- **Métriques de succès** : taux de livraison, latence de bout en bout, nombre de sauts,
  impact sur la batterie.
- **Batterie** : la consommation du scan/advertising BLE continu est un critère de viabilité,
  pas un détail.
- Stratégie de test : simulateur de mesh ? bancs d'essai multi-appareils ? tests terrain ?
- Aspects légaux : usage de la radio, chiffrement, RGPD sur les métadonnées du dashboard.
- Licence / ouverture du code ?

## 10. Décisions à trancher en premier

1. **Protocole maison** ou réutilisation d'un existant (Bluetooth Mesh SIG, Meshtastic,
   Briar, Bridgefy…) ?
2. **BLE seul** ou BLE + Bluetooth Classic ?
3. Messagerie **1-à-1 uniquement** ou aussi des **groupes** en v1 ?
4. Modèle de confiance : échange de clés en présentiel (TOFU) ? autorité ? 
5. Le relais iOS en arrière-plan est-il un objectif **v1** ou une évolution
   (selon le résultat de la preuve de faisabilité) ?
6. Périmètre exact du MVP et répartition du travail entre Paul, Tanguy et Olivier.
