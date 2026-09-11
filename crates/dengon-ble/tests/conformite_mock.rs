//! La suite de conformité exécutée contre [`MockTransport`].
//!
//! Ce fichier est le **modèle** que `btleplug` (US-303), `AndroidTransport`
//! (US-213) et NimBLE (US-220) recopieront : écrire un banc d'essai, puis
//! appeler la suite. Rien d'autre.
//!
//! Il est en `tests/` et non dans la crate pour une raison précise : un test
//! d'intégration ne voit que l'API **publique**. Si la suite cessait d'être
//! utilisable de l'extérieur, ce fichier ne compilerait plus — ce qui est
//! exactement le garde-fou qu'on veut, puisque son intérêt est d'être appelée
//! depuis d'autres crates.

use dengon_ble::conformance::{
    cas_deconnexion_brutale, cas_link_id_jamais_reutilise, suite_complete, BancDEssai,
};
use dengon_ble::{DisconnectReason, LinkId, MockTransport};

/// Banc d'essai du bouchon : il pilote ses propres structures internes.
struct BancMock;

impl BancDEssai for BancMock {
    type T = MockTransport;

    fn nouveau(&mut self) -> Self::T {
        MockTransport::new()
    }

    fn connecter_un_pair(&mut self, t: &mut Self::T) -> LinkId {
        t.connecter_pair(Some(-60))
    }

    fn couper(&mut self, t: &mut Self::T, lien: LinkId, motif: DisconnectReason) {
        t.couper_lien(lien, motif);
    }

    fn faire_recevoir(&mut self, t: &mut Self::T, lien: LinkId, bytes: &[u8]) {
        t.injecter_trame(lien, bytes);
    }
}

#[test]
fn mock_transport_passe_toute_la_suite() {
    suite_complete(&mut BancMock);
}

// Les deux cas les plus structurants sont aussi appelés seuls : quand la suite
// complète échoue, on veut voir lequel casse sans lire la trace.

#[test]
fn mock_transport_respecte_le_contrat_de_deconnexion_brutale() {
    cas_deconnexion_brutale(&mut BancMock);
}

#[test]
fn mock_transport_ne_reutilise_jamais_un_link_id() {
    cas_link_id_jamais_reutilise(&mut BancMock);
}
