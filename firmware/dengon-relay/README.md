# `dengon-relay` — firmware du relais ESP32

Squelette ESP-IDF + NimBLE du relais dengon (US-114). Il annonce le service BLE
`dengon` et déclare sa table GATT ; il ne relaie encore aucun message.

**La documentation d'usage — chaîne de compilation, build, flash, moniteur et
les pièges — est dans la fiche de suivi :
[`docs/suivi/modules/firmware-relay.md`](../../docs/suivi/modules/firmware-relay.md).**
Elle n'est pas dupliquée ici pour qu'il n'y ait qu'un seul endroit à tenir à jour.

En résumé, depuis la racine du dépôt :

```bash
IDF=espressif/idf:v5.5.5@sha256:a9231d0697ab8f7517cc072e93b7c83e04907bfbfba80b6440d7dbbf90665cf2

docker run --rm -it -u "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD:/repo" -w /repo/firmware/dengon-relay "$IDF" idf.py build
```

- Cible : ESP32-WROOM-32E (`docs/synthese/08-relais-esp32.md` §2).
- UUID du service et des caractéristiques : `docs/powl/03-network-protocol.md`
  §2, décision C-2. **Ne pas les redéfinir ailleurs.**
- Toute modification de `sdkconfig.defaults` exige un `idf.py fullclean` —
  sinon elle est ignorée en silence.
