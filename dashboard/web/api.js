// Accès à l'API réelle du dashboard (US-219) — remplace `data.js` (US-111,
// données bidon, aucun appel réseau).
//
// `window.DENGON_API_BASE` : URL de base de l'API. Vide par défaut (même
// origine que la page — le cas visé une fois `dashboard/web` servi derrière
// le même reverse-proxy que `dashboard/api`, US-224). Une page ouverte en
// `file://` ou servie depuis un autre port doit la définir avant de charger
// ce script (voir `index.html`) — sans ça, l'API n'a pas de sens à
// atteindre en relatif.

(function () {
  "use strict";

  function baseUrl() {
    return window.DENGON_API_BASE || "";
  }

  // `msg_log_id` est un hash tronqué à 8 octets / 16 hex (contrat gelé,
  // voir `docs/suivi/03-ecarts-conception.md`) — jamais autre chose. Rejeter
  // ici tout ce qui ne correspond pas évite de construire une URL d'API à
  // partir d'une valeur non validée issue du hash de navigation
  // (`window.location.hash`, donc modifiable par quiconque tape/partage un
  // lien), avant même d'atteindre `encodeURIComponent` — pas seulement une
  // histoire d'encodage, mais de forme attendue.
  const MSG_LOG_ID_VALIDE = /^[0-9a-f]{16}$/;

  async function fetchMessages() {
    const reponse = await fetch(baseUrl() + "/api/messages");
    if (!reponse.ok) {
      throw new Error("GET /api/messages → " + reponse.status);
    }
    return reponse.json();
  }

  // `null` pour un message inconnu (id mal formé ou 404) — distingué d'une
  // panne réseau (qui lève), pour que l'appelant affiche « introuvable »
  // plutôt que « erreur de connexion » dans ce cas précis.
  async function fetchMessage(msgLogId) {
    if (!MSG_LOG_ID_VALIDE.test(msgLogId)) return null;
    const reponse = await fetch(baseUrl() + "/api/messages/" + encodeURIComponent(msgLogId));
    if (reponse.status === 404) return null;
    if (!reponse.ok) {
      throw new Error("GET /api/messages/" + msgLogId + " → " + reponse.status);
    }
    return reponse.json();
  }

  // Abonnement au flux SSE (US-218) : `onEvenement` est rappelé pour CHAQUE
  // événement reçu (rattrapage inclus, à la connexion). L'appelant décide
  // quoi en faire (ici : redemander l'écran courant, voir app.js) — ce
  // module ne maintient aucun état de projection côté client, le serveur
  // reste la seule source de vérité pour `status`/`hop_count`/etc.
  //
  // Reconnexion : gérée nativement par `EventSource` (ré-essaie seule après
  // une coupure, en renvoyant `Last-Event-ID` — voir `app/main.py`), rien à
  // coder ici pour ce critère d'acceptation de l'US-218.
  function abonnerFlux(onEvenement) {
    const source = new EventSource(baseUrl() + "/api/stream");
    source.onmessage = function (msg) {
      try {
        onEvenement(JSON.parse(msg.data));
      } catch (error_) {
        // Un message SSE mal formé ne doit pas casser l'abonnement pour les
        // suivants — le flux garantit des événements valides côté serveur
        // (voir app/stream.py), donc ce cas n'est là qu'en dernier recours.
        console.error("dengon: événement SSE illisible", error_);
      }
    };
    return source;
  }

  window.DengonApi = {
    fetchMessages: fetchMessages,
    fetchMessage: fetchMessage,
    abonnerFlux: abonnerFlux,
  };
})();
