# Spike B (US-102) — `btleplug` peut-il tenir le rôle *peripheral* (GATT server) sous Linux ?

**Issue :** [#2](https://github.com/G1TS23/dengon/issues/2) · **Sprint** S1 ·
**Area** `core-rust` · 2 pts · **Jalon J0** · Timebox 0,5 jour (tenue : ~30 min
de recherche documentaire — voir §3, aucune manipulation réelle possible).
**Date :** 2026-09-25 · **Auteur :** Claude (Sonnet 5).
**Tranche :** [B-6](../../synthese/01-sujets-a-trancher.md) — backend BLE du
transport desktop/CLI (`dengon-ble`, US-303).

---

## 1. Réponse

> # NON.
>
> `btleplug` est une bibliothèque **central-only**, sur **toutes** les
> plateformes qu'elle supporte (Linux/BlueZ, macOS, Windows/WinRT) — pas
> seulement sous Linux. Ce n'est pas une limite de BlueZ, c'est un choix de
> conception assumé par la bibliothèque elle-même, sans plan annoncé pour
> l'étendre : elle ne permet ni d'annoncer (*advertise*) un service BLE, ni de
> servir des caractéristiques GATT localement (rôle *peripheral*/serveur).
>
> **Piège de nommage à connaître** : le trait `btleplug::api::Peripheral`
> **ne désigne pas** « notre appareil en rôle peripheral ». Il représente
> l'appareil **distant** découvert pendant un scan (« *a remote device you
> would like to communicate with (the "server" of BLE)* », doc officielle) —
> autrement dit le *serveur GATT d'en face*, vu depuis notre rôle *central*.
> Zéro rapport avec la capacité d'annoncer/servir localement.

**Conséquence pour le projet — plus large que prévu par l'issue.** Le tableau
de `docs/synthese/10-benchmarks-mvp-tests.md:49` (repris de
`docs/powl/01-benchmarks.md:96`) coche `btleplug` ✅ sur la colonne
« Peripheral + Central » pour Linux/BlueZ **et** macOS **et** Windows — c'est
une erreur qui préexistait à ce spike, pas seulement une inconnue Linux. La
case correcte est ❌ sur toute la ligne pour le rôle peripheral :
`btleplug` ne le fournit nulle part. Voir §4 « Ce que ça change dans les
docs ».

Ça ne remet pas en cause le contrat `Transport` de `dengon-ble` (US-105) :
`docs/suivi/modules/dengon-ble.md` l'anticipait déjà — *« Si la réponse est
non, c'est l'implémentation US-303 qui change, pas ce contrat »* —
`TransportError::Backend(String)` a justement été gardé assez vague pour
absorber un changement de backend sans toucher au trait.

---

## 2. Question posée

L'issue #2 : `btleplug` sait-il tenir le rôle *peripheral* (annoncer le
service `dengon`, servir du GATT) via son backend Linux/BlueZ ? Si non,
documenter un repli parmi : rester **central seul**, passer par **`bluer`**,
ou parler **BlueZ en direct via D-Bus**.

Contexte : `docs/synthese/10-benchmarks-mvp-tests.md:41-42` pose la
contrainte déterminante du projet — *« un vrai nœud mesh doit être
**simultanément** GATT peripheral + GATT central »* — et
`docs/synthese/04-architecture.md:95` généralise : *« chaque nœud est serveur
ET client »*, sans exception documentée pour `dengon-node`. Le spike vérifie
si le backend retenu pour `dengon-node` (« Desktop / CLI ») tient réellement
cette promesse.

---

## 3. Méthode suivie — et pourquoi ce n'est *pas* une exécution réelle

**Aucune machine Linux avec BlueZ n'est disponible dans cet environnement**
(poste Windows 10 ; ni WSL avec une distribution Linux active, ni service
`bluetoothd`, ni adaptateur BLE exposé à un userland Linux — voir la
commande `wsl --list --verbose`, qui ne montre que l'utilitaire
`docker-desktop` arrêté, pas une distro utilisable). C'est la même
contrainte d'environnement que le Spike C (US-103, PR #67, aucun téléphone
Android disponible) et que la réponse à la revue de la PR #69 (aucun
toolchain Rust local) : à consigner, pas à cacher (règle n°7 de
[`../README.md`](../README.md)).

**Ce spike n'avait cependant pas besoin d'exécution pour répondre.** La
question posée — « l'API de `btleplug` expose-t-elle une capacité
d'annonce/serveur GATT ? » — est une question sur la **surface publique
d'une bibliothèque**, constatable en lisant sa documentation officielle et
son code, pas un comportement dépendant du runtime BlueZ d'une machine
précise. C'est une différence importante avec le Spike A (US-101), qui
vérifiait un résultat de *compilation* (donc exécutable en isolation) : ici,
l'information cherchée est déjà écrite noir sur blanc par les mainteneurs de
`btleplug`.

Sources consultées (voir annexe pour les citations exactes) :

| # | Source | Ce qu'elle confirme |
|---|---|---|
| 1 | [README GitHub `deviceplug/btleplug`](https://github.com/deviceplug/btleplug) | « btleplug is meant to be *host/central mode only* » + alternatives suggérées (`bluster`, `ble-peripheral-rust`) pour le rôle peripheral |
| 2 | [docs.rs — page racine du crate](https://docs.rs/btleplug/latest/btleplug/) | Tagline officielle : « a Bluetooth Low Energy (BLE) **central** module library for Rust » ; exemple d'usage 100 % central (scan, connect, discover, write) |
| 3 | [docs.rs — module `btleplug::api`](https://docs.rs/btleplug/latest/btleplug/api/index.html) | Liste des traits publics (`Central`, `Manager`, `Peripheral`) — aucun trait/struct d'annonce ou de serveur GATT local ; confirme le sens de `Peripheral` (§1) |
| 4 | Recherche croisée (issues/discussions tierces) | Aucune roadmap ni fork actif visant à ajouter le rôle peripheral à `btleplug` ; les projets qui en ont besoin citent systématiquement une bibliothèque **différente** |

**Limite assumée** : ceci reste une lecture de documentation, pas une preuve
par le code compilé/exécuté comme le Spike A. Le risque résiduel (doc
officielle inexacte ou obsolète) est faible — la phrase « central mode only »
est la description **du projet lui-même**, pas une déduction — mais ce n'est
pas le même niveau de certitude qu'un test qui tourne. À noter pour qui
lirait ce rapport en s'attendant au format du Spike A.

---

## 4. Ce que ça change dans les docs

Deux tableaux de conception affirmaient, avant ce spike, que `btleplug`
couvrait le rôle peripheral+central sur les trois OS desktop. Les deux sont
corrigés par ce spike (voir diffs dans les fichiers cités, annotation en
ligne plutôt que réécriture) :

- `docs/synthese/04-architecture.md:86` — colonne « Techno » de la ligne
  Desktop/CLI : annotée pour préciser que `btleplug` seul ne couvre que le
  rôle **central**.
- `docs/synthese/10-benchmarks-mvp-tests.md:49` — colonne « Peripheral +
  Central » de la ligne Desktop/CLI : `✅` remplacé par une annotation
  précisant que seul le rôle central est couvert, avec renvoi à ce rapport.

`docs/powl/01-benchmarks.md:96` (source originelle de l'erreur) n'est **pas**
modifié : `docs/powl/` est la matière première inchangée (voir
`CLAUDE.md`) ; seule la couche `docs/synthese/` (décisions retenues) est
mise à jour.

---

## 5. Repli recommandé

Les trois options de l'issue, évaluées :

| Option | Couvre central | Couvre peripheral | Portée OS | Verdict |
|---|---|---|---|---|
| **Central seul** (`btleplug`, sans rôle peripheral) | ✅ | ❌ | Linux/macOS/Windows | `dengon-node` ne serait jamais **découvrable** — contredit `docs/synthese/10-benchmarks-mvp-tests.md:217` (« `dengon-node` comme relais mobile d'appoint »), qui suppose qu'on peut s'y connecter |
| **`bluer`** (bindings officiels BlueZ, D-Bus) | ✅ | ✅ | **Linux uniquement** | Couvre les deux rôles **nativement** avec une seule dépendance ; maintenu par le projet BlueZ lui-même |
| **BlueZ en direct via D-Bus** (sans binding) | ✅ | ✅ | Linux uniquement | Fait la même chose que `bluer`, à la main — pas de raison de réinventer un binding déjà officiel |

**Recommandation, à ratifier en réunion (même statut que B-2/B-3) :**
remplacer `btleplug` par **`bluer`** comme unique backend de
`dengon-ble` pour `dengon-node`, et assumer que `dengon-node` est **Linux
uniquement** — ce qui n'abandonne rien : `docs/synthese/02-probleme-et-besoins.md:66`
et `docs/powl/00-overview.md:45` désignaient déjà `dengon-node` comme
« PC**/Linux** (Rust) », jamais comme un outil macOS/Windows. La promesse
« Linux/BlueZ, macOS, Windows » du tableau `10-benchmarks-mvp-tests.md`
concernait la portée de `btleplug` en tant que bibliothèque, pas un objectif
produit pour `dengon-node` — personne ne perd une plateforme réellement
visée.

Repli du repli, si l'équipe préfère ne pas ajouter `bluer` tout de suite :
**central seul**, en acceptant explicitement que `dengon-node` ne puisse
qu'initier des connexions (utile pour les tests `dengon-sim`/CI, pas pour le
rôle de relais mobile d'appoint) — décision alors à documenter dans
`03-ecarts-conception.md` au moment de l'US-303, pas ici.

---

## 6. Ce que le spike ne prouve pas

- **`bluer` n'a pas été compilé ni exécuté.** Sa documentation officielle
  affirme le support central+peripheral+GATT server via D-Bus, mais aucune
  ligne de code n'a tourné ici pour le vérifier — contrairement au Spike A,
  qui avait un `.a` compilé et un `nm` pour preuve. **L'US-303 devra
  reprendre ce spike sur une vraie machine Linux** (deux processus `bluer`
  qui s'annoncent et se scannent l'un l'autre) avant de considérer le choix
  définitivement validé.
- **Rien ne dit que BlueZ est installé/actif sur les postes de l'équipe** :
  `bluer` en dépend runtime (`bluetoothd`) en plus de `libdbus-1-dev` à la
  compilation. Pas vérifié ici.
- Le comportement d'Android/ESP32 (déjà dual-rôle par construction,
  `BluetoothGattServer`/NimBLE) n'est pas concerné par ce spike.

---

## 7. Suites à donner

| Quoi | Où |
|---|---|
| Ratifier (ou rejeter) le remplacement `btleplug` → `bluer` pour `dengon-ble` desktop | Réunion d'équipe, avant US-303 |
| Si ratifié : ajouter `bluer` (pas `btleplug`) aux `[workspace.dependencies]`, écrire `crates/dengon-ble/src/bluer_transport.rs` | US-303 |
| Reproduire ce spike avec du vrai code (deux process `bluer` locaux qui s'annoncent/se scannent) sur une machine Linux avec BlueZ, avant de fermer définitivement B-6 | US-303 |
| Corriger `docs/synthese/04-architecture.md:86` et `10-benchmarks-mvp-tests.md:49` si le nom du backend change (actuellement annotés, pas réécrits) | US-303 |

---

## Annexe — citations exactes

> « btleplug is meant to be *host/central mode only*. If you are interested
> in peripheral BTLE (i.e. acting like a Bluetooth LE device instead of
> connecting to one), check out `bluster` or `ble-peripheral-rust`. »
> — README, [`deviceplug/btleplug`](https://github.com/deviceplug/btleplug)

> « btleplug is a Bluetooth Low Energy (BLE) **central** module library for
> Rust. »
> — [docs.rs/btleplug](https://docs.rs/btleplug/latest/btleplug/), description du crate

> Le trait `Peripheral` représente « a remote device you would like to
> communicate with (the "server" of BLE) » — un appareil **distant**
> découvert au scan, pas notre propre rôle peripheral.
> — [docs.rs/btleplug/api](https://docs.rs/btleplug/latest/btleplug/api/index.html)

Pas de log de commande à annexer : recherche documentaire uniquement, aucun
code jetable écrit (rien à compiler sans Linux/BlueZ pour le vérifier).
