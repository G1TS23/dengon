// Fichier de test intégré : un `expect` qui panique = échec de test, c'est le
// comportement voulu. (`allow-*-in-tests` du clippy.toml ne couvre que les
// modules `#[cfg(test)]`, pas les crates de `tests/`.)
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! La suite de conformité `Transport` (US-105) exécutée contre
//! [`SimTransport`] — les **mêmes assertions** que pour `MockTransport`.
//!
//! Le banc crée un réseau neuf par cas, et fait apparaître chaque pair comme
//! un vrai nœud simulé relié au nœud sous test : la connexion, la coupure et
//! la réception passent donc par le chemin normal du réseau, pas par un
//! raccourci.

use std::collections::BTreeMap;

use dengon_ble::conformance::{suite_complete, BancDEssai};
use dengon_ble::{DisconnectReason, LinkId, Transport, TransportConfig};
use dengon_sim::{NoeudId, ParametresLien, ReseauPartage, SimTransport};

#[derive(Default)]
struct BancSim {
    reseau: Option<ReseauPartage>,
    pairs: BTreeMap<NoeudId, SimTransport>,
}

impl BancSim {
    fn reseau(&self) -> &ReseauPartage {
        self.reseau.as_ref().expect("nouveau() appelé avant")
    }
}

impl BancDEssai for BancSim {
    type T = SimTransport;

    fn nouveau(&mut self) -> Self::T {
        let reseau = ReseauPartage::new(dengon_sim::SEED_PAR_DEFAUT);
        let t = reseau.ajouter_noeud();
        self.reseau = Some(reseau);
        self.pairs.clear();
        t
    }

    fn connecter_un_pair(&mut self, t: &mut Self::T) -> LinkId {
        let mut pair = self.reseau().ajouter_noeud();
        pair.start(TransportConfig::default())
            .expect("start du pair");
        let (moi, lui) = (t.noeud(), pair.noeud());
        self.reseau().relier(moi, lui, ParametresLien::default());
        self.pairs.insert(lui, pair);
        self.reseau()
            .lien_entre(moi, lui)
            .expect("connexion établie")
    }

    fn couper(&mut self, _t: &mut Self::T, lien: LinkId, motif: DisconnectReason) {
        self.reseau().couper_lien(lien, motif);
    }

    fn faire_recevoir(&mut self, _t: &mut Self::T, lien: LinkId, bytes: &[u8]) {
        // Lien fermé : plus d'extrémité, rien n'est envoyé (c'est le cas
        // « fantôme » de la suite).
        if let Some((noeud, son_lien)) = self.reseau().lien_oppose(lien) {
            if let Some(pair) = self.pairs.get_mut(&noeud) {
                pair.send(son_lien, bytes).expect("envoi depuis le pair");
            }
        }
    }
}

#[test]
fn sim_transport_passe_la_suite_de_conformite() {
    suite_complete(&mut BancSim::default());
}
