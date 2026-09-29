# US-312 — Scénarios 2 et 3 du DoD sur vrai matériel : procédure rejouable

> **État (2026-09-29) :** procédure écrite, code prêt (branche
> `feat/US-312-e2e-relais`), **essai sur matériel pas encore fait**. Les
> champs « à relever » sont remplis pendant l'essai, puis l'essai est
> consigné dans [`00-journal.md`](../00-journal.md).

Référence : `docs/synthese/10-benchmarks-mvp-tests.md` §3.1 (DoD n°2 et n°3),
`docs/synthese/07-cycle-de-vie-et-statuts.md` §5 (parcours d'une enveloppe
par un relais).

## 1. Ce que l'on démontre

| Scénario | Énoncé du DoD | Critère de réussite observable |
|---|---|---|
| 2 | Un 3ᵉ appareil hors de portée est joint via au moins un relais ESP32. | B reçoit le message de A sans lien direct A ↔ B ; A passe à « Distribué ». |
| 3 | Un destinataire **éteint** reçoit son message à son retour (enveloppe scellée) ; l'expéditeur, **déconnecté** entre-temps, voit ses statuts se mettre à jour à sa reconnexion. | B reçoit le message après son retour ; A, revenu après B, passe de « Parti » à « Distribué ». |
| — | Les événements remontent jusqu'au dashboard déployé. | `envelope.stored`, `envelope.offered`, `envelope.handoff`, `peer.announce_seen` du relais visibles sur `https://51.255.38.214:8443`. |

## 2. Configuration matérielle

| Élément | Valeur |
|---|---|
| Téléphone A (expéditeur) | Pixel 8 Pro, Android 17 (SDK 37) — *à confirmer* |
| Téléphone B (destinataire) | OnePlus 7 Pro, Android 12 (SDK 31) — *à confirmer* |
| Relais | ESP32-D0WD-V3 (WROOM), `node_id` `relay-9309e5`, CH340, via `usbipd` |
| Firmware | commit *à relever* (`git rev-parse --short HEAD`), `idf.py build` ESP-IDF v5.5.5 (image Docker épinglée) |
| APK | commit *à relever*, `assembleDebug` |
| Point d'accès Wi-Fi du relais | partage de connexion du PC (SSID *à relever*) |
| Distances | A ↔ R ≈ *à relever* m ; R ↔ B ≈ *à relever* m ; A ↔ B ≈ *à relever* m |
| « Hors de portée » obtenu par | ☐ séparation physique (pièces / étages) ☐ option debug « Relais seulement » |

Avec un seul relais et deux téléphones, **l'ESP32 est le 3ᵉ appareil** du
scénario 2 : écart consigné dans `03-ecarts-conception.md` (US-312).

### Garantir que A et B ne se voient pas

1. **Préféré :** séparation physique. Vérifier dans l'écran *Transport BLE
   (debug)* de chaque téléphone que la liste « Pairs identifiés » ne contient
   **que** le relais (`… (relais)` dans le journal).
2. **Sinon :** activer l'interrupteur **« Relais seulement »** sur les deux
   téléphones (écran debug), **avant** d'ouvrir les liens (redémarrer le
   service si un lien direct existe déjà). Un lien vers un téléphone est
   alors journalisé `lien direct ignoré (option debug)` et n'atteint jamais
   le nœud. L'option est perdue au redémarrage de l'app.

## 3. Préparation (une fois)

```bash
# 1. Firmware : cœur Rust puis application, flash
. ~/export-esp.sh && firmware/dengon-relay/tools/build_core.sh
docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD:/repo" \
  -w /repo/firmware/dengon-relay espressif/idf@sha256:a9231d06… \
  idf.py -B build-us312 -D SDKCONFIG=build-us312/sdkconfig build
# flash depuis Windows (esptool) ou idf.py -p <port> flash

# 2. App : bibliothèques natives + APK, installation sur A et B
android/scripts/build-ffi.sh
(cd android && ./gradlew assembleDebug)
adb.exe -s <série A> install -r android/app/build/outputs/apk/debug/app-debug.apk
adb.exe -s <série B> install -r android/app/build/outputs/apk/debug/app-debug.apk

# 3. Dashboard : base propre avant l'essai (sur le VPS)
dashboard/deploy/purge-demo.sh
```

Console série du relais (`idf.py monitor`, capture avec `| tee capture-us312.log`) :

```
dash id                       # node_id + pub_sign
# sur le PC : python3 firmware/dengon-relay/tools/register_relay.py --node-id relay-9309e5 --pub-sign <pub_sign>
dash token <jwt>
wifi <ssid> <mot de passe>
dash status                   # attendu : jeton présent, Wi-Fi connecté
relay                         # compteurs à zéro (ou valeurs de départ à relever)
```

Appairage : A et B s'ajoutent en contact **par QR** (écran QR, code à 60
chiffres identique des deux côtés), comme au scénario 1 (US-306). C'est la
seule étape où A et B peuvent être proches.

## 4. Scénario 2 — joindre B hors de portée via le relais

| # | Action | Attendu | Relevé (heure, valeur) |
|---|---|---|---|
| 2.1 | A et B près du relais, pas l'un de l'autre. Démarrer le service sur les deux. | Journal debug A et B : `link#… ↔ <relais> (relais)`. Console `relay` : `non_authentiques=0` (aucun handshake vers le relais). | |
| 2.2 | A écrit à B : « scénario 2 ». | A : « Parti » puis « Distribué ». | |
| 2.3 | — | B : message reçu. | |
| 2.4 | Console `relay`. | `déposées` +2 (message + accusé), `remises` +2, `non_authentiques=0`, `illisibles=0`. | |

`adb.exe -s <série> logcat -s dengon-transport` sur chaque téléphone pendant
l'essai (à joindre au journal).

## 5. Scénario 3 — destinataire éteint, expéditeur parti

| # | Action | Attendu | Relevé |
|---|---|---|---|
| 3.1 | B : arrêter le service (ou mode avion). A près du relais. | Journal A : lien relais seul. | |
| 3.2 | A écrit à B : « scénario 3 ». | A : « Parti » (enveloppe confiée au relais). Console `relay` : `déposées` +1. | |
| 3.3 | A : arrêter le service (A parti). | Relais : lien A fermé. | |
| 3.4 | B : redémarrer le service près du relais. | B reçoit « scénario 3 ». Console : `remises` +1 puis `déposées` +1 (accusé de B). | |
| 3.5 | B : arrêter le service. A : redémarrer près du relais. | A : « Distribué » (accusé récupéré sur le relais). | |

## 6. Dashboard et journal du relais

1. `https://51.255.38.214:8443` → écran « parcours d'un message » et carte
   réseau : événements `envelope.stored` / `envelope.offered` /
   `envelope.handoff` du relais `relay-9309e5`, horodatés dans l'ordre des
   étapes ci-dessus. `dash status` : `perdus/refusés 0`.
2. Journal chaîné du relais, après l'essai :
   ```
   ledger                         # console série, capturée
   python3 firmware/dengon-relay/tools/dump_ledger.py capture-us312.log -o ledger-us312.bin
   cargo run -p dengon-verify -- --pubkey <pub_sign> ledger-us312.bin   # attendu : "verdict":"ok"
   ```
3. Les téléphones **n'exportent pas** leurs événements vers le dashboard
   (aucun client HTTP dans l'app) : seuls ceux du relais y figurent — écart
   consigné.

## 7. Limites connues à garder en tête pendant l'essai

- **Messages courts seulement** : un texte qui fait passer l'enveloppe au
  palier de padding 512 (au-delà d'environ 220 octets UTF-8) ne tient pas
  dans une trame du relais (514 o) ; il n'est pas confié au relais et reste
  « En attente » (test `texte_trop_long_pour_une_trame_du_relais_reste_en_attente`).
- Le relais tient **3 liens** au plus (`CONFIG_BT_NIMBLE_MAX_CONNECTIONS`).
- La remise d'une enveloppe est comptée dès sa mise en file (US-308) : si le
  lien tombe à cet instant, l'enveloppe est perdue côté relais ; l'expéditeur
  la rejoue à sa prochaine liaison (outbox, au plus `RESEND_MAX` fois par pair).
- Heure : le relais apprend l'heure du premier `ANNOUNCE` de téléphone (ou du
  SNTP, US-309) ; les tags J-1/J/J+1 tolèrent un jour d'écart.
