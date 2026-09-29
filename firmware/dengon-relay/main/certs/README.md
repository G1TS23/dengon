# Racine de l'autorité du dashboard (US-309)

Le firmware embarque `dashboard_root.pem` (s'il existe) et l'épingle pour
vérifier le serveur HTTPS du dashboard. Le VPS de démo est servi par Caddy en
`tls internal` : son certificat feuille dure **12 h** et l'intermédiaire
**7 jours**, seule la **racine** est stable (volume `dengon_caddy_data`,
conservé par `purge-demo.sh`). Le serveur ne l'envoie pas pendant la
poignée de main : il faut la récupérer sur le VPS.

```sh
# sur le VPS, dans dashboard/deploy/
docker compose exec caddy cat /data/caddy/pki/authorities/local/root.crt \
  > dashboard_root.pem
# puis la copier ici : firmware/dengon-relay/main/certs/dashboard_root.pem
```

Vérifier avant de flasher :

```sh
openssl s_client -connect 51.255.38.214:8443 \
  -CAfile firmware/dengon-relay/main/certs/dashboard_root.pem </dev/null \
  | grep 'Verify return code'   # attendu : 0 (ok)
```

Le fichier est ignoré par git (règle `*.pem` du `.gitignore` racine).
Absent, le build réussit quand même (CI) mais l'export HTTPS est coupé :
`dash status` affiche `racine CA : absente`. Une racine régénérée (volume
Caddy supprimé) impose de reflasher.
