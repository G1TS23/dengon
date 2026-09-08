# Support de présentation orale

> Consolidé **en fin de projet** à partir du reste du dossier de suivi. Ébauché tôt
> pour ne rien oublier. Objectif : présenter le projet et **tout ce qui a été codé**
> sans relire le code, et tenir face aux questions du jury.

---

## 1. Pitch (30 s)

_dengon : une messagerie qui marche sans Internet. Les messages sautent de téléphone
en téléphone et de relais en relais par Bluetooth, chiffrés de bout en bout. Un
tableau de bord suit leur parcours sans jamais voir leur contenu._

_(à affiner)_

## 2. Plan proposé (10-15 min)

1. **Le problème** : communiquer sans réseau (1 min).
2. **L'idée** : réseau maillé BLE + tolérance aux déconnexions (2 min).
3. **« Type blockchain » — ce qu'on a retenu et rejeté** : gossip + journal chaîné
   signé, pas de consensus (2 min). → argument de maturité technique.
4. **Architecture** : cœur Rust partagé, app Android, relais ESP32, dashboard (2 min).
5. **Démo** (3-4 min) : voir §4.
6. **Ce qu'on a appris** : 2-3 notions marquantes de [`04-apprentissages.md`](04-apprentissages.md) (2 min).
7. **Limites & suite** (1 min).

## 3. Messages clés (ce que le jury doit retenir)

- On a **compris** le rapprochement avec la blockchain et fait un **choix motivé** de
  ne garder que ce qui sert (infalsifiabilité, décentralisation) sans le coût du
  consensus.
- **Une seule implémentation** du protocole et de la crypto (Rust), réutilisée
  partout → sécurité auditable une fois.
- **Le dashboard ne peut pas tricher ni espionner** : c'est une garantie *by design*,
  pas une promesse.
- La **déconnexion n'est jamais bloquante** : démonstrable en direct.

_(cocher/ajuster en fin de projet selon ce qui a vraiment été livré)_

## 4. Script de démo

_(à écrire quand la démo est stable — remplir avec les commandes/gestes exacts)_

| Étape | Geste | Ce qu'on montre | Résultat attendu |
|---|---|---|---|
| 1 | scanner le QR entre 2 téléphones | établissement de contact + code de vérification | contact « ✔ vérifié » |
| 2 | envoyer un message à portée | statuts qui défilent | en attente → parti → distribué → lu |
| 3 | éloigner le destinataire, relais ESP32 au milieu | multi-saut | message livré via le relais |
| 4 | éteindre le destinataire, envoyer, le rallumer | store-and-forward | message reçu au retour, ACK remonte |
| 5 | ouvrir le dashboard sur le VPS | parcours du message + carte réseau | chemin et statuts visibles |
| 6 | (optionnel) altérer un journal de relais | détection d'intégrité | alerte `chain_broken` |

Plan B si la radio fait des siennes : `dengon-sim` rejoue les mêmes scénarios en
local (déterministe).

## 5. Questions anticipées

| Question probable | Réponse (à étoffer) |
|---|---|
| « Pourquoi pas une vraie blockchain ? » | Consensus = latence + énergie + besoin d'un quorum joignable, incompatible avec l'offline-first ; aucun problème de double-dépense à résoudre. On garde la hash-chain pour la traçabilité. |
| « Le dashboard peut-il lire les messages ? » | Non : chiffrement E2E, il ne reçoit que des métadonnées signées, `msgID` haché, aucune clé. |
| « Que se passe-t-il si un relais est malveillant ? » | Il ne déchiffre rien. Il peut jeter des paquets (le réseau route autour) ou mentir dans ses logs (détecté par recoupement multi-relais + rupture de chaîne). |
| « Pourquoi Android seulement ? » | iOS bride le Bluetooth périphérique en arrière-plan ; l'architecture (cœur Rust + UniFFI) permet iOS ensuite. |
| « C'est quoi la portée réelle ? » | ~10-30 m par saut BLE ; les relais ESP32 densifient ; multi-saut jusqu'à TTL. |
| « Combien de nœuds ça tient ? » | _(à remplir avec les chiffres des tests de charge, cf. `11-testing-strategy.md`)_ |

## 6. Ce qui a été livré vs conçu

_(tableau final à copier depuis [`01-etat-du-code.md`](01-etat-du-code.md) en fin de
projet, en distinguant : livré et testé / livré partiel / non fait et pourquoi)_

## 7. Limites assumées & pistes

_(reprendre [`docs/powl/10-mvp-scope-roadmap.md`](../powl/10-mvp-scope-roadmap.md) §3
et §4, filtré sur ce qui reste vrai à la fin)_
