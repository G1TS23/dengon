// Données bidon (US-111) — aucune n'est réelle, aucun appel réseau ne les
// produit. Forme alignée sur le schéma SQLite du dashboard
// (docs/synthese/09-dashboard-et-donnees.md §11.2, tables `messages` et
// `message_hops`) : c'est le même modèle que consommera la vraie API
// (US-217/US-219), pour que brancher l'un sur l'autre plus tard ne change
// que la source des données, pas la forme.
//
// Complet à dessein : au moins un message sans aucun saut ("unknown" — le
// cas "vue partielle" documenté dans docs/olivier/dashboard.md §8), et un
// saut avec des champs radio manquants (rssi/fanout absents), pour vérifier
// que l'écran ne casse pas sur des données incomplètes (critère d'
// acceptation de l'US-111).

window.DENGON_DASHBOARD_DATA = {
  messages: [
    {
      msg_log_id: "7f3a1c9d2e4b5a60",
      status: "in_flight",
      first_seen_ms: Date.UTC(2026, 8, 20, 10, 2, 0),
      last_event_ms: Date.UTC(2026, 8, 20, 10, 4, 12),
      hop_count: 3,
      delivery_latency_ms: null,
    },
    {
      msg_log_id: "b1c8e2f309a7d4c1",
      status: "delivered",
      first_seen_ms: Date.UTC(2026, 8, 20, 9, 51, 0),
      last_event_ms: Date.UTC(2026, 8, 20, 9, 58, 30),
      hop_count: 4,
      delivery_latency_ms: 450000,
    },
    {
      msg_log_id: "42de99a1b3c05e78",
      status: "expired",
      first_seen_ms: Date.UTC(2026, 8, 19, 22, 10, 0),
      last_event_ms: Date.UTC(2026, 8, 19, 22, 41, 5),
      hop_count: 2,
      delivery_latency_ms: null,
    },
    {
      // Cas « Inconnu / partiel » (docs/olivier/dashboard.md §8) : le
      // dashboard sait qu'un message de ce nom a été créé (on lui a donné
      // un id à suivre) mais n'a reçu aucun événement de relais — aucun nœud
      // rapporteur n'a eu de Wi-Fi depuis. hop_count et delivery_latency_ms
      // sont donc absents, pas juste à zéro.
      msg_log_id: "9a0f11223344aabb",
      status: "unknown",
      first_seen_ms: Date.UTC(2026, 8, 20, 10, 30, 0),
      last_event_ms: null,
      hop_count: null,
      delivery_latency_ms: null,
    },
    {
      msg_log_id: "c4d5e6f708192a3b",
      status: "queued",
      first_seen_ms: Date.UTC(2026, 8, 20, 10, 40, 0),
      last_event_ms: Date.UTC(2026, 8, 20, 10, 40, 0),
      hop_count: 1,
      delivery_latency_ms: null,
    },
  ],

  // Clé = msg_log_id. Un message sans entrée ici (ex. "9a0f11223344aabb")
  // est un cas valide : aucun saut connu, pas une erreur de données.
  hops: {
    "7f3a1c9d2e4b5a60": [
      { node_id: "client-9c1d40", ts_ms: Date.UTC(2026, 8, 20, 10, 2, 0), ttl_in: null, ttl_out: 8, fanout: 2, rssi: null, kind: "relay" },
      { node_id: "relay-3f2a9c", ts_ms: Date.UTC(2026, 8, 20, 10, 3, 4), ttl_in: 8, ttl_out: 7, fanout: 3, rssi: -62, kind: "relay" },
      { node_id: "relay-88d1b2", ts_ms: Date.UTC(2026, 8, 20, 10, 4, 12), ttl_in: 7, ttl_out: 6, fanout: 2, rssi: -71, kind: "relay" },
    ],
    "b1c8e2f309a7d4c1": [
      { node_id: "client-77aa02", ts_ms: Date.UTC(2026, 8, 20, 9, 51, 0), ttl_in: null, ttl_out: 8, fanout: 1, rssi: null, kind: "relay" },
      { node_id: "relay-cc44ee", ts_ms: Date.UTC(2026, 8, 20, 9, 52, 40), ttl_in: 8, ttl_out: 7, fanout: 2, rssi: -58, kind: "relay" },
      { node_id: "relay-3f2a9c", ts_ms: Date.UTC(2026, 8, 20, 9, 55, 10), ttl_in: 7, ttl_out: 6, fanout: 1, rssi: -66, kind: "envelope_handoff" },
      { node_id: "client-9c1d40", ts_ms: Date.UTC(2026, 8, 20, 9, 58, 30), ttl_in: 6, ttl_out: null, fanout: null, rssi: -49, kind: "delivered" },
    ],
    "42de99a1b3c05e78": [
      { node_id: "client-9c1d40", ts_ms: Date.UTC(2026, 8, 19, 22, 10, 0), ttl_in: null, ttl_out: 8, fanout: 1, rssi: null, kind: "relay" },
      // Saut volontairement incomplet : un relais réel ne remonte pas
      // toujours le RSSI (dépend du chipset) ni le fanout (dépôt en
      // enveloppe scellée plutôt que diffusion). L'écran doit afficher
      // « — » à la place, pas planter ni afficher "undefined"/"null".
      { node_id: "relay-88d1b2", ts_ms: Date.UTC(2026, 8, 19, 22, 41, 5), ttl_in: 8, ttl_out: 7, fanout: null, rssi: null, kind: "envelope_store" },
    ],
    "c4d5e6f708192a3b": [
      { node_id: "client-77aa02", ts_ms: Date.UTC(2026, 8, 20, 10, 40, 0), ttl_in: null, ttl_out: 8, fanout: 0, rssi: null, kind: "relay" },
    ],
  },
};
