// Dashboard (US-219) : mêmes écrans que le squelette US-111, branchés sur
// l'API réelle (`api.js`) au lieu des données bidon de `data.js` (retirées).
// Routage par hash (#/message/<id>), inchangé depuis l'US-111.
//
// Régression assumée par rapport à l'US-111 : la page ne fonctionne plus en
// `file://` (un navigateur bloque `fetch`/`EventSource` depuis cette
// origine vers une API HTTP) — attendu, l'US-111 disait déjà que US-219
// « branchera ces mêmes écrans sur l'API réelle », ce qui suppose de facto
// un vrai serveur. Voir `docs/suivi/modules/dashboard-web.md`.
//
// Écran #/integrite (US-310) : verdict d'intégrité par nœud (`GET
// /api/integrity`, calculé par `dengon-verify` côté API).

(function () {
  "use strict";

  var VERDICT_LABEL = {
    ok: "Intègre",
    broken: "Altéré",
    fork: "Fourche détectée",
    gap: "Trou dans le journal",
    unverified: "Non vérifiable",
  };

  // "Vire au rouge" (critère d'acceptation US-310) : les trois verdicts
  // d'anomalie partagent la même couleur d'alerte — la distinction entre eux
  // est dans le libellé/l'étiquette, pas dans une nuance de rouge en plus.
  var VERDICT_CLASSE = {
    ok: "statut--delivered",
    broken: "statut--expired",
    fork: "statut--expired",
    gap: "statut--expired",
    unverified: "statut--inconnu",
  };

  var SIGNATURES_LABEL = {
    verified: "vérifiées",
    invalid: "invalides",
    unchecked: "non vérifiées",
  };

  var MOIS = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];

  // Correspondance avec docs/synthese/07-cycle-de-vie-et-statuts.md §1
  // (mêmes codes que le cœur) + "unknown", propre à la projection dashboard
  // (docs/synthese/09-dashboard-et-donnees.md §11.2) : aucun événement reçu
  // pour ce message — vue partielle, pas une erreur.
  var STATUT_LABEL = {
    queued: "En attente",
    in_flight: "Parti",
    delivered: "Distribué",
    read: "Lu",
    expired: "Expiré",
    unknown: "Inconnu",
  };

  var HOP_KIND_LABEL = {
    relay: "Relayé",
    envelope_store: "Déposé en enveloppe scellée",
    envelope_handoff: "Transmis à un autre porteur",
    delivered: "Distribué",
  };

  function texteOuTiret(valeur) {
    return valeur === null || valeur === undefined || valeur === "" ? "—" : String(valeur);
  }

  function formatHorodatage(ms) {
    if (ms === null || ms === undefined) return "—";
    var d = new Date(ms);
    var jour = String(d.getUTCDate()).padStart(2, "0");
    var mois = MOIS[d.getUTCMonth()];
    var heures = String(d.getUTCHours()).padStart(2, "0");
    var minutes = String(d.getUTCMinutes()).padStart(2, "0");
    return jour + " " + mois + " " + heures + ":" + minutes;
  }

  function formatDuree(ms) {
    if (ms === null || ms === undefined) return "—";
    var totalSec = Math.round(ms / 1000);
    var min = Math.floor(totalSec / 60);
    var sec = totalSec % 60;
    if (min === 0) return sec + " s";
    return min + " min " + sec + " s";
  }

  function formatSauts(hopCount) {
    if (hopCount === null || hopCount === undefined) return "—";
    return hopCount + (hopCount === 1 ? " saut" : " sauts");
  }

  function idCourt(msgLogId) {
    return msgLogId.length > 8 ? msgLogId.slice(0, 8) + "…" : msgLogId;
  }

  function statutLabel(statut) {
    return STATUT_LABEL[statut] || texteOuTiret(statut);
  }

  function classeStatut(statut) {
    return "statut statut--" + (STATUT_LABEL[statut] ? statut.replaceAll("_", "-") : "inconnu");
  }

  function el(tag, attrs, enfants) {
    var node = document.createElement(tag);
    attrs = attrs || {};
    Object.keys(attrs).forEach(function (nom) {
      if (nom === "class") node.className = attrs[nom];
      else if (nom === "text") node.textContent = attrs[nom];
      else node.setAttribute(nom, attrs[nom]);
    });
    (enfants || []).forEach(function (enfant) {
      if (enfant) node.appendChild(enfant);
    });
    return node;
  }

  function ecranChargement() {
    return el("section", { class: "ecran" }, [el("p", { class: "vide", text: "Chargement…" })]);
  }

  function ecranErreur(erreur) {
    return el("section", { class: "ecran" }, [
      el("p", {
        class: "vide",
        text: "Impossible de joindre l'API du dashboard (" + erreur.message + "). Nouvelle tentative automatique à la prochaine mise à jour.",
      }),
    ]);
  }

  function carteMessage(message) {
    var lien = el("a", { class: "carte-message", href: "#/message/" + encodeURIComponent(message.msg_log_id) });

    lien.appendChild(
      el("div", { class: "carte-message__entete" }, [
        el("code", { class: "id-message", title: message.msg_log_id, text: idCourt(message.msg_log_id) }),
        el("span", { class: classeStatut(message.status), text: statutLabel(message.status) }),
      ]),
    );

    lien.appendChild(
      el("dl", { class: "carte-message__meta" }, [
        el("dt", { text: "Créé" }),
        el("dd", { text: formatHorodatage(message.first_seen_ms) }),
        el("dt", { text: "Dernière activité" }),
        el("dd", { text: formatHorodatage(message.last_event_ms) }),
        el("dt", { text: "Sauts" }),
        el("dd", { text: formatSauts(message.hop_count) }),
      ]),
    );

    return lien;
  }

  async function renderListe() {
    document.title = "dengon · suivi — messages";
    var messages = await window.DengonApi.fetchMessages();
    // Tri par activité la plus récente déjà fait côté API (ORDER BY
    // last_event_ms DESC, voir app/messages_api.py) — pas refait ici.
    var liste = el("div", { class: "liste-messages" }, messages.map(carteMessage));

    if (messages.length === 0) {
      return el("section", { class: "ecran" }, [
        el("p", { class: "vide", text: "Aucun message suivi pour l'instant." }),
      ]);
    }

    return el("section", { class: "ecran" }, [
      el("p", { class: "compteur", text: messages.length + " message(s) suivi(s)" }),
      liste,
    ]);
  }

  function ligneHop(hop, estDernier) {
    var detail =
      "TTL " +
      texteOuTiret(hop.ttl_in) +
      " → " +
      texteOuTiret(hop.ttl_out) +
      " · fanout " +
      texteOuTiret(hop.fanout) +
      " · RSSI " +
      (hop.rssi !== null && hop.rssi !== undefined ? hop.rssi + " dBm" : "—");

    return el("li", { class: "sauts__item" + (estDernier ? " sauts__item--dernier" : "") }, [
      el("div", { class: "sauts__point" }),
      el("div", { class: "sauts__contenu" }, [
        el("div", { class: "sauts__ligne1" }, [
          el("code", { class: "id-noeud", text: hop.node_id }),
          el("time", { class: "sauts__heure", text: formatHorodatage(hop.ts_ms) }),
        ]),
        el("div", { class: "sauts__kind", text: HOP_KIND_LABEL[hop.kind] || texteOuTiret(hop.kind) }),
        el("div", { class: "sauts__detail", text: detail }),
      ]),
    ]);
  }

  async function renderDetail(msgLogId) {
    var retour = el("a", { class: "retour", href: "#/", text: "← Retour à la liste" });
    var message = await window.DengonApi.fetchMessage(msgLogId);

    if (!message) {
      document.title = "dengon · suivi — message introuvable";
      return el("section", { class: "ecran" }, [
        retour,
        el("p", { class: "vide", text: "Message introuvable : " + msgLogId }),
      ]);
    }

    document.title = "dengon · suivi — " + idCourt(message.msg_log_id);

    var entete = el("div", { class: "detail__entete" }, [
      el("code", { class: "id-message id-message--grand", text: message.msg_log_id }),
      el("span", { class: classeStatut(message.status), text: statutLabel(message.status) }),
    ]);

    var meta = el("dl", { class: "detail__meta" }, [
      el("dt", { text: "Créé" }),
      el("dd", { text: formatHorodatage(message.first_seen_ms) }),
      el("dt", { text: "Dernière activité" }),
      el("dd", { text: formatHorodatage(message.last_event_ms) }),
      el("dt", { text: "Sauts connus" }),
      el("dd", { text: formatSauts(message.hop_count) }),
      el("dt", { text: "Latence de distribution" }),
      el("dd", { text: formatDuree(message.delivery_latency_ms) }),
    ]);

    var hops = message.hops || [];
    var corpsSauts;
    if (hops.length === 0) {
      corpsSauts = el("p", {
        class: "vide",
        text: "Aucun saut remonté pour l'instant — vue partielle du réseau (voir le bandeau en haut de page).",
      });
    } else {
      corpsSauts = el(
        "ol",
        { class: "sauts" },
        hops.map(function (hop, i) {
          return ligneHop(hop, i === hops.length - 1);
        }),
      );
    }

    return el("section", { class: "ecran" }, [retour, entete, meta, el("h2", { class: "titre-section", text: "Parcours" }), corpsSauts]);
  }

  function ligneIntegrite(verdict) {
    var plage = verdict.first_seq === null ? "—" : "seq " + verdict.first_seq + "–" + verdict.last_seq;
    var signatures = SIGNATURES_LABEL[verdict.signatures] || texteOuTiret(verdict.signatures);

    return el("li", { class: "carte-integrite" }, [
      el("div", { class: "carte-integrite__entete" }, [
        el("code", { class: "id-noeud", text: verdict.node_id }),
        el("span", {
          class: "statut " + (VERDICT_CLASSE[verdict.verdict] || "statut--inconnu"),
          text: VERDICT_LABEL[verdict.verdict] || texteOuTiret(verdict.verdict),
        }),
      ]),
      el("dl", { class: "carte-integrite__meta" }, [
        el("dt", { text: "Entrées vérifiées" }),
        el("dd", { text: String(verdict.entries) }),
        el("dt", { text: "Plage" }),
        el("dd", { text: plage }),
        el("dt", { text: "Signatures" }),
        el("dd", { text: signatures }),
      ]),
    ]);
  }

  async function renderIntegrite() {
    document.title = "dengon · suivi — intégrité";
    var retour = el("a", { class: "retour", href: "#/", text: "← Retour à la liste" });
    var titre = el("h2", { class: "titre-section", text: "Intégrité des journaux, par nœud" });
    // Même garde contre une réponse périmée que les autres écrans : le jeton
    // de génération de `route()` (plus de vérification `isConnected` ici).
    var verdicts = await window.DengonApi.fetchIntegrity();

    if (verdicts.length === 0) {
      return el("section", { class: "ecran" }, [
        retour,
        titre,
        el("p", { class: "vide", text: "Aucun nœud enregistré pour l'instant." }),
      ]);
    }

    return el("section", { class: "ecran" }, [
      retour,
      titre,
      el("ul", { class: "liste-integrite" }, verdicts.map(ligneIntegrite)),
    ]);
  }

  // Jeton de génération : une requête `fetch` en vol dont la réponse arrive
  // APRÈS qu'une navigation ou un rafraîchissement SSE plus récent a déjà
  // affiché autre chose ne doit pas écraser cet affichage plus récent avec
  // un résultat périmé (ex. l'utilisateur clique vite sur deux messages
  // différents, ou un événement SSE arrive pendant le chargement de la
  // liste).
  var generationCourante = 0;

  function ecranCourant(hash) {
    if (hash === "#/integrite") return renderIntegrite();
    var correspondanceMessage = /^#\/message\/(.+)$/.exec(hash);
    if (correspondanceMessage) return renderDetail(decodeURIComponent(correspondanceMessage[1]));
    return renderListe();
  }

  async function route() {
    var generation = ++generationCourante;
    var racine = document.getElementById("app");

    racine.innerHTML = "";
    racine.appendChild(ecranChargement());

    var ecran;
    try {
      ecran = await ecranCourant(window.location.hash);
    } catch (error_) {
      ecran = ecranErreur(error_);
    }

    if (generation !== generationCourante) return; // une navigation plus récente a déjà pris la main
    racine.innerHTML = "";
    racine.appendChild(ecran);
  }

  // Rafraîchissement live (US-218/US-219) : tout événement reçu sur le flux
  // SSE redemande l'écran courant à l'API plutôt que de maintenir une
  // projection côté client — le serveur reste la seule source de vérité
  // pour `status`/`hop_count`/etc. (voir app/projections.py). Débit borné à
  // un rafraîchissement toutes les 500 ms au plus : un batch de plusieurs
  // événements ne doit pas déclencher autant de requêtes `fetch`.
  var rafraichissementProgramme = null;
  function planifierRafraichissement() {
    if (rafraichissementProgramme) return;
    rafraichissementProgramme = window.setTimeout(function () {
      rafraichissementProgramme = null;
      route();
    }, 500);
  }

  window.addEventListener("hashchange", route);
  route();
  window.DengonApi.abonnerFlux(planifierRafraichissement);
})();
