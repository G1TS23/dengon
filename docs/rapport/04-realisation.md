# Réalisation

> Section du [rapport](README.md) — voir le [plan détaillé](00-plan.md).
> Source : [`docs/suivi/`](../suivi/README.md) (journal, avancement par
> composant, fiches modules), l'état réel du code sur `main` à la date de
> rédaction — pas la cible de conception décrite dans
> [`docs/synthese/`](../synthese/00-contexte-global.md), à laquelle cette
> section se réfère uniquement pour mesurer l'écart.
>
> **Statut : brouillon, écrit en anticipation de la fin du sprint 3** — voir
> [`README.md`](README.md). Le développement se poursuit au moment de la
> rédaction ; les chiffres cités ci-dessous sont ceux mesurés à cette date,
> pas une projection.

## Ce qui tourne réellement

Le cœur commun `dengon-core` est le module le plus abouti du projet : les 28
noms d'événements du catalogue d'observabilité sont posés, le format de
trame binaire (en-tête, TTL, signature, fragmentation) est stable et
conforme à ses vecteurs de test, et surtout les briques qui font tourner le
protocole sont **toutes livrées** : chiffrement Ed25519/Noise, identité et QR
d'appairage, persistance locale chiffrée, routage sans-IO (déduplication,
TTL, anti-inondation, clamp de densité), réconciliation d'inventaire entre
voisins, dépôt et remise d'enveloppes scellées, journal chaîné signé, et la
façade `api` qui câble l'ensemble pour un client (téléphone, nœud de test) ou
un relais dédié. Sur ce seul module, `cargo test --workspace` fait tourner
**524 tests** (property tests compris) qui passent tous, avec une couverture
mesurée à **93 % de lignes** sur l'ensemble du workspace Rust (`cargo
llvm-cov`) — au-delà du seuil de 85 % visé par la conception pour le cœur du
protocole, chiffre vérifié directement (voir
[Difficultés](06-difficultes.md)), pas déclaré sur parole.

Les trois plateformes prévues par l'architecture existent et partagent ce
même cœur, exactement comme la conception le voulait :

- **L'application Android** possède un transport Bluetooth réel (serveur et
  client GATT, via `AndroidTransport`), une messagerie Compose complète
  (liste de conversations, fil, saisie, statuts), l'appairage par QR code, et
  est branchée sur le **vrai** FFI généré par UniFFI — plus sur un bouchon.
  Le scénario « un message part d'un téléphone et arrive sur l'autre » a été
  démontré sur deux appareils réels. 48 tests JVM couvrent la conformité du
  transport et la messagerie ; les builds `assembleDebug` et `assembleRelease`
  (avec réduction de code R8 active) passent.
- **Le firmware du relais ESP32** tourne sur la pile NimBLE de l'ESP-IDF, avec
  quatre tâches FreeRTOS concurrentes (routage, réconciliation d'inventaire,
  courrier des enveloppes, journal), un stockage NVS + LittleFS qui survit à
  un redémarrage sans rompre la chaîne du journal, et la même logique de
  protocole que le reste du projet, compilée en bibliothèque statique
  `no_std` et liée au C. 32 tests Unity (transport) plus 9 (stockage) passent
  sur cible, et un essai réel sur une carte a démontré trois redémarrages
  consécutifs suivis d'une vérification d'intégrité positive par
  `dengon-verify`. L'essai à **deux** cartes simultanées, lui, n'a pas encore
  eu lieu — voir [Recette](05-recette.md).
- **Le tableau de bord** (FastAPI + SQLite côté serveur, page statique
  côté client) ingère des lots d'événements signés, les vérifie et les
  restitue en cinq vues : parcours d'un message, carte du réseau, flotte des
  relais, intégrité des journaux chaînés (déléguée à `dengon-verify`, le même
  binaire de vérification que celui utilisé par chaque appareil du réseau),
  et recherche libre. 99 tests `pytest` côté API, rendu vérifié à 360 px avec
  un Chromium sans tête. Il est déployé sur un VPS réel derrière un
  reverse-proxy TLS.

## Le squelette qui a permis d'aller vite : le simulateur

Avant même que `dengon-core` existe, l'équipe a construit `dengon-sim`, un
harnais capable de faire tourner N nœuds sur un réseau simulé
(latence, gigue, perte, partition, scriptables) avec quatre scénarios
déterministes rejoués à graine fixe dans la CI. Ce choix — simuler avant de
coder le vrai protocole — a permis de valider tôt le comportement attendu
d'un maillage (convergence, absence de boucle, tenue sous inondation) sans
attendre que le Bluetooth réel, le firmware ou l'application soient prêts, et
sert toujours de garde-fou de non-régression à chaque changement du cœur du
protocole.

## Ce qui reste en dehors du périmètre livré

Deux catégories de manques, de nature différente. La première regroupe ce qui
est **explicitement hors MVP** dès la conception (voir
[Problème et besoins](01-probleme.md)) : l'application iOS, le statut
« Lu », la réconciliation par filtre compact (Gossip GCS) qui remplacerait
l'échange d'inventaire brut, les groupes et pièces jointes, le ratchet façon
Signal, un firmware relais entièrement réécrit en Rust plutôt qu'en C lié à
une bibliothèque Rust. Aucune de ces absences n'est une surprise : elles
étaient posées comme telles avant que la première ligne de code ne soit
écrite.

La seconde catégorie regroupe des **écarts découverts en cours de route**,
consignés au fil de l'eau dans
[`docs/suivi/03-ecarts-conception.md`](../suivi/03-ecarts-conception.md) plutôt
que corrigés en silence. Les plus significatifs :

- La façade côté client (`api`) accepte bien de **porter** une enveloppe
  scellée croisée sur son chemin (déposée dans `sync::courier` dès qu'elle ne
  s'ouvre pas avec sa propre clé), mais ne câble pas la négociation en deux
  paquets qui la **remettrait** ensuite à son vrai destinataire
  (`ENVELOPE_OFFER`/`ENVELOPE_REQUEST`) : un téléphone porte, sans jamais
  offrir ce qu'il porte — ce rôle actif reste celui du relais dédié.
- Un téléphone ne pose jamais le drapeau `RELAY_OK` et n'émet ni `ANNOUNCE`
  ni `INVENTORY` : il ne peut pas encore se faire relayer par un relais ESP32
  réel dans le sens client → relais, seulement relais → client. La
  démonstration à deux cartes et un téléphone (recette, point 3) en dépend
  directement.
- Le tableau de bord n'a, à ce jour, reçu de flux réel que d'un firmware de
  test et non d'un relais complet en conditions de terrain prolongées — la
  robustesse du client HTTPS face à des coupures Wi-Fi répétées reste
  vérifiée sur des scénarios courts, pas sur une session de plusieurs heures.

Aucun de ces trois points ne remet en cause l'architecture retenue : ce sont
des combinaisons de fonctionnalités individuellement livrées, pas encore
câblées bout en bout entre elles, et documentées comme telles plutôt que
passées sous silence — cohérent avec l'exigence du projet de « consigner les
tests qui échouent, les étapes bâclées », pas de déclarer un résultat qui
n'existe pas.
