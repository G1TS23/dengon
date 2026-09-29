# Difficultés rencontrées et apprentissages

> Section du [rapport](README.md) — voir le [plan détaillé](00-plan.md).
> Source : [`docs/suivi/04-apprentissages.md`](../suivi/04-apprentissages.md)
> (plus de soixante entrées détaillées, techniques) et
> [`docs/suivi/03-ecarts-conception.md`](../suivi/03-ecarts-conception.md).
> Cette section n'en reprend qu'une sélection représentative ; le détail
> complet, avec les fichiers et lignes exacts, vit dans ces deux documents de
> suivi.

Beaucoup des difficultés rencontrées partagent un même trait : elles étaient
**invisibles à la relecture du code**, et ne se sont révélées qu'à l'exécution
— sur du matériel réel, sous un test généré aléatoirement, ou dans un scénario
à plusieurs nœuds. Ce constat, répété tout au long du projet, a fini par
devenir une méthode : chaque brique du protocole qui pouvait raisonnablement
être testée sans radio l'a été de cette manière, avant tout essai sur
matériel — précisément pour débusquer ce genre de bug avant qu'il ne coûte
une session de test avec deux téléphones et une carte ESP32.

## Un bug de handshake que seul un test à deux nœuds révèle

Le protocole d'échange de clés Noise `XX` se termine différemment selon le
rôle : le répondeur termine en **lisant** le dernier message de la poignée de
main, l'initiateur termine en **l'écrivant** — il n'a rien à lire après. Le
code de dengon vérifiait la fin du handshake juste après une lecture, ce qui
est correct pour un répondeur mais faux pour un initiateur, qui doit encore
écrire avant que ce soit vraiment terminé. Le bug ne cassait rien de visible
côté initiateur, qui envoyait bien son dernier message sans erreur : c'est le
**répondeur**, plus tard, qui rejetait silencieusement toute la session,
parce que l'état de l'initiateur restait bloqué en « poignée de main en
cours ». Chaque relecture isolée du code semblait correcte ; seul un test de
bout en bout entre deux nœuds réels l'a fait apparaître.

## Un property test trouve le cas limite que personne n'a pensé à écrire

La fragmentation d'un paquet trop grand pour une trame Bluetooth doit
garantir qu'un paquet ressort **exactement une fois**, quel que soit l'ordre
d'arrivée des fragments ou la présence de doublons. Les tests écrits à la
main, avec des paquets de 300 à 1000 octets, passaient tous. Un test à base
de génération aléatoire (`proptest`) a réduit l'échec à son plus petit
contre-exemple : un paquet tenant sur un **seul** fragment, envoyé en
double. Un paquet à fragment unique n'était jamais mis en attente de
réassemblage, donc rien ne se souvenait qu'il était déjà sorti — le doublon
ressortait une seconde fois. Personne n'avait pensé à ce cas limite en
écrivant les tests à la main ; la correction a, en même temps, supprimé une
fuite mémoire liée aux réassemblages jamais terminés qu'un doublon tardif
pouvait laisser ouverts.

## Deux règles saines isolément, qui se contredisent une fois combinées

La réconciliation d'inventaire (« à la rencontre, pousse à ton voisin tout ce
qui lui manque ») et l'anti-inondation (« n'accepte pas plus de N nouveaux
messages par minute d'un même voisin ») sont chacune une bonne idée prise
séparément. Combinées sans précaution, elles se marchent dessus : un relais
qui a accumulé 120 paquets rencontre un téléphone et lui pousse tout d'un
bloc — le routeur du récepteur, qui applique son propre quota, en rejette la
majorité. La perte est silencieuse : la rencontre « réussit » quand même,
juste avec une fraction de ce qui aurait dû être transmis. Seul un test
témoin, comparant la même scène avec et sans cadencement du push, l'a rendue
mesurable. La correction cadence l'émission du manquant **sous** le quota du
récepteur plutôt qu'à son maximum, en tenant compte du fait que d'autres
trafics (l'annonce d'inventaire elle-même, les messages directs) consomment
aussi ce même quota.

## Un seuil « littéral » de la conception qui affame un losange

Pour limiter le nombre de copies inutiles d'un même message en circulation
(*tempête de diffusion*), un nœud qui entend un voisin rediffuser un paquet
identique pendant son propre délai d'attente renonce à le relayer à son tour.
La conception, lue littéralement, fixe ce seuil à un seul doublon entendu.
Sur une topologie en losange (deux chemins vers un même nœud, qui mène
lui-même à un cinquième), ce seuil affame complètement le dernier maillon :
le nœud du milieu entend le paquet passer par les deux branches, renonce, et
le nœud final ne reçoit jamais rien — vérifié par un test dédié à cette
topologie précise, pas une intuition. Porter le seuil à deux doublons
résout le problème sans réintroduire de tempête de diffusion, un compromis
directement documenté comme un écart assumé face à la conception initiale.

## Le matériel a des limites qu'aucun compilateur ne signale

Le firmware du relais a fait remonter plusieurs contraintes propres à un
environnement embarqué contraint, aucune détectable en dehors d'un test sur
cible ou une lecture attentive de la documentation matérielle :

- Une annonce Bluetooth 4.x ne transporte que **31 octets**. Ce que la
  conception exige dans ce paquet (identifiants de service, données
  constructeur) en consomme déjà 30 ; il a fallu déplacer le nom de
  l'appareil dans le paquet de réponse au scan pour tenir. Le dépassement
  n'est pas détecté à la compilation : sans vérifier le code retour de la
  fonction d'annonce NimBLE, la carte se contente, silencieusement, de ne
  jamais annoncer sa présence.
- Le générateur d'aléa matériel de l'ESP32 (`esp_fill_random`) n'est un vrai
  générateur matériel que si le Wi-Fi, le Bluetooth, ou un mode explicite du
  bootloader sont actifs — sinon c'est un simple générateur pseudo-aléatoire.
  L'ordre d'initialisation du firmware en tient compte : les secrets
  cryptographiques du premier démarrage sont tirés avant toute activation de
  la radio, sous ce mode explicite.
- NimBLE réutilise l'identifiant numérique d'une connexion (`conn_handle`)
  dès que la précédente est fermée, alors que l'abstraction de transport du
  projet exige un identifiant **jamais** réutilisé au cours d'une exécution
  — sans cette garantie, une trame en retard sur un lien mort ou un
  événement traité après coup pourrait être attribué au pair suivant. Un
  test naïf qui ouvre deux connexions successives passerait même avec ce bug
  si le banc de test attribue des identifiants différents par hasard ; il a
  fallu écrire un banc qui imite fidèlement le comportement de réutilisation
  de NimBLE pour que le test soit réellement probant.

## L'intégration Kotlin ↔ Rust a son lot de pièges spécifiques

Les liaisons Kotlin générées par UniFFI ne passent pas par JNI mais par JNA,
qui charge la bibliothèque native par son nom à l'exécution — un mécanisme
qui rend possible de faire tourner les tests d'intégration Kotlin ↔ Rust
directement sur la machine de développement, sans téléphone ni émulateur,
mais qui a demandé trois ajustements non documentés d'emblée : nommer
explicitement la bibliothèque générée (sinon `uniffi-bindgen` suppose un nom
par défaut qui ne correspond à rien), restreindre les architectures
processeur embarquées dans l'APK (sinon l'installation réussit sur une
architecture pour laquelle la bibliothèque native n'existe pas, et
l'application plante au premier appel plutôt qu'à l'installation), et
protéger explicitement les classes que JNA retrouve par réflexion de la
réduction de code en version release (sinon l'erreur n'apparaît qu'à
l'exécution, jamais à la compilation).

## Ce que ces épisodes ont en commun

Aucune de ces difficultés n'aurait été détectée par une relecture, aussi
attentive soit-elle : il a fallu, à chaque fois, une exécution — un test à
deux nœuds réels, un test généré aléatoirement, un essai sur cible physique,
un scénario à cinq nœuds simulés. C'est directement ce qui a motivé
l'investissement, tôt dans le projet, dans `dengon-sim` (le simulateur
multi-nœuds) et dans les property tests plutôt que des exemples écrits à la
main : ces deux outils sont ceux qui ont concrètement débusqué la majorité
des bugs cités ici, avant qu'ils n'atteignent un essai coûteux sur matériel
réel.
