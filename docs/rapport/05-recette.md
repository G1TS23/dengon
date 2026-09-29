# Recette et tests

> Section du [rapport](README.md) — voir le [plan détaillé](00-plan.md).
> Source : [`docs/synthese/10-benchmarks-mvp-tests.md`](../synthese/10-benchmarks-mvp-tests.md)
> §4.6 (checklist de recette) ; issue **US-314**
> ([#52](https://github.com/G1TS23/dengon/issues/52)).
>
> **Statut : la recette n'a pas encore eu lieu au moment de la rédaction de
> cette section.** Contrairement aux sections précédentes, celle-ci ne peut
> pas être rédigée à l'avance : `docs/rapport/README.md` le dit explicitement
> — « pas d'affirmation sur un résultat qui n'existe pas encore ». Ce qui suit
> décrit le protocole de recette prévu et l'état de préparation vérifié, pas
> un résultat.

## Ce que la recette doit prouver

La conception distingue explicitement ce qui est **testé** (les 524 tests
automatisés de `dengon-core`, les tests JVM d'Android, les tests Unity du
firmware, les tests `pytest` du tableau de bord — voir
[Réalisation](04-realisation.md)) de ce qui est **recetté** : neuf points
exécutés dans l'ordre, sur du matériel réel, qui vérifient que l'assemblage
complet des pièces se comporte comme prévu, pas seulement chacune isolément.

1. **Contact** — appairage par QR code, comparaison orale du code de
   vérification à soixante chiffres.
2. **Direct** — un message A→B à 5 m, statuts observés jusqu'à « Distribué ».
3. **Multi-saut** — B s'éloigne à 40 m avec un relais ESP32 entre les deux,
   le message doit tout de même arriver.
4. **Destinataire absent** — Charlie éteint son Bluetooth, reçoit un message,
   le récupère en le rallumant près d'un relais.
5. **Expéditeur absent** — A envoie puis coupe son Bluetooth, retrouve le
   statut « Distribué » en revenant.
6. **Tableau de bord** — parcours de chaque message, carte du réseau, état
   des relais, observés en direct.
7. **Intégrité** — une entrée de journal altérée sur un relais de test doit
   être détectée par le tableau de bord (`chain_broken`).
8. **Sécurité** — une capture du trafic Bluetooth ne doit révéler aucun texte
   en clair ; le VPS ne doit recevoir aucun identifiant de message en clair.
9. **Densité** — huit à dix appareils dans une même salle, messages croisés,
   sans effondrement du réseau.

## État de préparation, à ce jour

Plusieurs des neuf points s'appuient sur des mécanismes qui ne sont, à ce
jour, pas tous câblés bout en bout — voir la section « Ce qui reste en dehors
du périmètre livré » de [Réalisation](04-realisation.md). En particulier, le
point 3 (multi-saut avec un relais ESP32) suppose qu'un téléphone puisse se
faire relayer par un relais réel, ce qui n'est pas encore le cas côté
application : c'est un exemple concret de pourquoi la recette ne peut pas
être anticipée par une simple relecture du code — seule une exécution réelle,
sur le matériel, dira si l'assemblage tient.

Ce qui est en revanche déjà vérifié séparément, pièce par pièce, sur du
matériel réel (pas en simulation) :

- **Point 2 (direct)**, en partie : un message a été échangé de bout en bout
  entre deux téléphones Android réels, via le vrai transport Bluetooth
  (US-306).
- **Point 7 (intégrité)**, en partie : trois redémarrages consécutifs d'un
  relais réel, suivis d'une vérification par `dengon-verify` confirmant une
  chaîne de journal intacte (US-308).
- **Point 8 (sécurité)**, en partie : le format binaire des paquets, la
  redaction structurelle des événements d'observabilité (aucun identifiant de
  message en clair ne peut être construit, garanti par le typage plutôt que
  par convention — voir [Difficultés](06-difficultes.md)) et le chiffrement
  Noise sont vérifiés par des tests automatisés et des vecteurs de
  conformité, mais pas encore par une capture radio réelle avec un
  analyseur Bluetooth.

## Ce que la recette produira

Conformément au critère d'acceptation de l'US-314, chaque point exécuté sera
consigné dans un rapport **daté**, dans `docs/suivi/`, avec un verdict passé
ou échoué et sa raison — un échec sera rapporté tel quel, pas dissimulé, et
ouvrira une issue de suivi. Cette section sera alors remplacée par les
résultats réels, avec un renvoi vers ce rapport, dans le cadre de l'US-315 qui
finalise le rapport écrit.
