//! Catalogue des noms d'événements du périmètre MVP (US-208).
//!
//! Source de vérité : `docs/powl/08-observability-events.md` (le format
//! complet, tous domaines confondus) et `contracts/tools/catalogue.py`
//! (`CATALOGUE`, la version Python déjà utilisée par `dashboard/api` et
//! `contracts/tools/validate.py` pour valider les 20 fixtures golden de
//! l'US-107 — restreinte au périmètre MVP, les événements `integrity.*`/
//! `node.clock_skew` dérivés côté dashboard et `msg.read`/`read.observed`
//! (statut « Lu », v2) en sont explicitement exclus).
//!
//! Cette liste **doit rester synchronisée** avec `catalogue.py` — vérifié
//! manuellement à l'écriture de ce module (28 noms des deux côtés, mêmes
//! noms). Aucun outillage cross-langage ne le garantit automatiquement
//! aujourd'hui (écart consigné dans `03-ecarts-conception.md`) : le job CI
//! `cross-vectors` (US-222) est le bon endroit pour l'automatiser un jour,
//! sur le même principe que `protocol_vectors.rs`.

/// Les 28 noms d'événements du périmètre MVP, triés — ordre alphabétique
/// par domaine puis par nom, pas l'ordre du document `docs/powl/08`.
pub const EVENT_NAMES: &[&str] = &[
    "ack.observed",
    "attest.emitted",
    "attest.observed",
    "client.summary",
    "envelope.delivered",
    "envelope.expired",
    "envelope.handoff",
    "envelope.offered",
    "envelope.stored",
    "msg.delivered",
    "msg.expired",
    "msg.handed_off",
    "msg.queued",
    "msg.received",
    "peer.announce_seen",
    "peer.connected",
    "peer.disconnected",
    "pkt.delivered_local",
    "pkt.dropped",
    "pkt.duplicate",
    "pkt.rejected",
    "pkt.relayed",
    "pkt.seen",
    "relay.boot",
    "relay.health",
    "relay.overloaded",
    "relay.wifi_down",
    "relay.wifi_up",
];

/// `true` si `name` fait partie du catalogue MVP.
pub fn is_known(name: &str) -> bool {
    EVENT_NAMES.binary_search(&name).is_ok()
}

/// Le nom tel qu'il est rangé au catalogue (`&'static str`, ce qu'attend
/// [`super::Envelope`]), ou `None` s'il n'y figure pas. Sert à relire les
/// noms du journal, stockés en `String` (US-309).
pub fn lookup(name: &str) -> Option<&'static str> {
    EVENT_NAMES
        .binary_search(&name)
        .ok()
        .map(|i| EVENT_NAMES[i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    #[test]
    fn exactement_28_noms_comme_cote_python() {
        // Recompté manuellement le 2026-09-28 dans contracts/tools/
        // catalogue.py::CATALOGUE : 28 clés. Pas de vérification
        // programmatique cross-langage aujourd'hui (voir la note de
        // module) — ce test protège au moins contre une régression du
        // côté Rust seul (ex. un nom supprimé par erreur).
        assert_eq!(EVENT_NAMES.len(), 28);
    }

    #[test]
    fn la_liste_est_triee_et_sans_doublon() {
        // `is_known` fait un `binary_search` : la liste doit rester triée.
        let mut sorted = EVENT_NAMES.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            EVENT_NAMES,
            sorted.as_slice(),
            "EVENT_NAMES doit rester trié"
        );

        let unique: BTreeSet<&str> = EVENT_NAMES.iter().copied().collect();
        assert_eq!(unique.len(), EVENT_NAMES.len(), "pas de doublon attendu");
    }

    #[test]
    fn is_known_reconnait_un_nom_du_catalogue_et_rejette_le_reste() {
        assert!(is_known("pkt.relayed"));
        assert!(is_known("msg.queued"));
        assert!(!is_known("pkt.inconnu"));
        assert!(!is_known("msg.read")); // hors périmètre MVP (statut « Lu », v2)
        assert!(!is_known("integrity.chain_broken")); // dérivé côté dashboard, jamais émis par un nœud
    }
}
