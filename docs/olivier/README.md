# Dossier de travail — Olivier

Documentation du projet **dengon** (messagerie hors-ligne par maillage Bluetooth).
Espace de travail personnel en attendant la mise en commun avec Paul et Tanguy.

| Document | Contenu | État |
|----------|---------|------|
| [`CONTEXT.md`](CONTEXT.md) | Concept, besoins métier, statuts des messages (réflexion d'équipe) | Référence |
| [`analyse-besoins.md`](analyse-besoins.md) | Besoins, questions ouvertes et risques ; compléments à `CONTEXT.md` | v0.1 |
| [`decisions-v1.md`](decisions-v1.md) | Décisions de cadrage v1, avis d'Olivier sur les points d'équipe, planning jusqu'au 29/09/2026 | v0.1 |
| [`protocole.md`](protocole.md) | Spécification du protocole (format de trame, circulation des messages, accusés, store-and-forward, sécurité) | v0.2 |
| [`architecture.md`](architecture.md) | Éléments du système (mobile / ESP32 / dashboard), couches, chemin d'un message | v0.1 |
| [`dashboard.md`](dashboard.md) | Sous-système de suivi : événements anonymisés, serveur, écrans, replis | v0.1 |
| [`etude-stack.md`](etude-stack.md) | Étude comparative de la stack technique (BLE mobile, ESP32, moteur partagé, crypto, serveur) + recommandations | v0.1 |

## Rappels

- **Échéance : soutenance le 29/09/2026.** Voir la section « Impact du délai » de
  `decisions-v1.md`.
- Zones d'Olivier : **protocole** + **dashboard**.
- Priorité de conception : **la circulation des messages**.
- Tous les brouillons sont en v0.1 : ils contiennent des points explicitement
  marqués « (ouvert) » à trancher en équipe.
