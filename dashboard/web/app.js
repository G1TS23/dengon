// Squelette du dashboard (US-111) : page statique, données bidon (data.js),
// aucun appel réseau. Routage par hash (#/message/<id>) — fonctionne aussi
// bien en `file://` que servi par un vrai serveur plus tard (US-217/US-219
// brancheront ces mêmes écrans sur l'API réelle, cf. `docs/synthese/09-
// dashboard-et-donnees.md` §5/§6 — pas de fetch/XHR ici, seulement les
// scripts <script src> classiques, qui eux fonctionnent en `file://`).

(function () {
  "use strict";

  var DATA = window.DENGON_DASHBOARD_DATA;

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

  function messagesTriesParActivite() {
    // Les plus récemment actifs en premier ; un message "unknown" sans
    // `last_event_ms` retombe sur sa création (toujours défini).
    return DATA.messages.slice().sort(function (a, b) {
      var ta = a.last_event_ms !== null ? a.last_event_ms : a.first_seen_ms;
      var tb = b.last_event_ms !== null ? b.last_event_ms : b.first_seen_ms;
      return tb - ta;
    });
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

  function renderListe() {
    document.title = "dengon · suivi — messages";
    var messages = messagesTriesParActivite();
    var liste = el(
      "div",
      { class: "liste-messages" },
      messages.map(carteMessage),
    );

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

  function renderDetail(msgLogId) {
    var message = DATA.messages.find(function (m) {
      return m.msg_log_id === msgLogId;
    });

    var retour = el("a", { class: "retour", href: "#/", text: "← Retour à la liste" });

    if (!message) {
      document.title = "dengon · suivi — message introuvable";
      return el("section", { class: "ecran" }, [
        retour,
        el("p", { class: "vide", text: "Message introuvable (id inconnu de cette démo) : " + msgLogId }),
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

    var hops = DATA.hops[msgLogId] || [];
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

  function route() {
    var hash = window.location.hash;
    var correspondance = /^#\/message\/(.+)$/.exec(hash);
    var racine = document.getElementById("app");
    racine.innerHTML = "";
    racine.appendChild(correspondance ? renderDetail(decodeURIComponent(correspondance[1])) : renderListe());
  }

  window.addEventListener("hashchange", route);
  route();
})();
