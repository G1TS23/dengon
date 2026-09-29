//! Suite de conformité `Transport` contre [`CentralTransport`] (US-303).
//!
//! La radio est **fausse** (aucun matériel) : ce qui est jugé ici est la
//! logique de liens du transport desktop — attribution des `LinkId`, quota,
//! ordre des événements, comportement en coupure brutale. Le backend
//! `btleplug` lui-même ne se teste que sur matériel : voir
//! `docs/suivi/modules/dengon-ble.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use dengon_ble::conformance::{suite_complete, BancDEssai};
use dengon_ble::{
    CentralRadio, CentralTransport, DisconnectReason, LinkId, RadioEvent, RadioHandle, Result,
    TransportConfig,
};

#[derive(Debug, Default)]
struct Etat {
    file: Vec<RadioEvent>,
    ecrites: Vec<(RadioHandle, Vec<u8>)>,
    fermes: Vec<RadioHandle>,
}

/// Radio simulée : le test pousse des événements, lit ce qui a été écrit.
#[derive(Debug, Clone, Default)]
struct FausseRadio(Arc<Mutex<Etat>>);

impl FausseRadio {
    fn pousser(&self, e: RadioEvent) {
        self.0.lock().unwrap().file.push(e);
    }
}

impl CentralRadio for FausseRadio {
    fn start(&mut self, _cfg: &TransportConfig) -> Result<()> {
        Ok(())
    }
    fn poll(&mut self) -> Vec<RadioEvent> {
        std::mem::take(&mut self.0.lock().unwrap().file)
    }
    fn write(&mut self, handle: RadioHandle, bytes: &[u8]) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .ecrites
            .push((handle, bytes.to_vec()));
        Ok(())
    }
    fn disconnect(&mut self, handle: RadioHandle) {
        self.0.lock().unwrap().fermes.push(handle);
    }
    fn max_frame(&self) -> usize {
        512
    }
}

/// Banc : garde la radio pour piloter le « monde extérieur ».
///
/// Le transport attribue ses `LinkId` par un compteur qui part de 0 : le
/// n-ième pair connecté porte `LinkId::new(n)`. Le banc s'appuie sur cette
/// propriété (documentée sur `LinkId`) pour rendre le lien **sans consommer**
/// l'événement `PeerConnected`, que la suite relit ensuite avec `poll`.
#[derive(Default)]
struct BancCentral {
    radio: FausseRadio,
    /// Nombre de pairs connectés depuis `nouveau` = handle radio du suivant.
    connectes: u64,
}

impl BancDEssai for BancCentral {
    type T = CentralTransport<FausseRadio>;

    fn nouveau(&mut self) -> Self::T {
        self.radio = FausseRadio::default();
        self.connectes = 0;
        CentralTransport::new(self.radio.clone())
    }

    fn connecter_un_pair(&mut self, _t: &mut Self::T) -> LinkId {
        let handle = self.connectes;
        self.connectes += 1;
        self.radio.pousser(RadioEvent::Connected {
            handle,
            rssi: Some(-60),
        });
        LinkId::new(handle)
    }

    fn couper(&mut self, _t: &mut Self::T, lien: LinkId, motif: DisconnectReason) {
        self.radio.pousser(RadioEvent::Disconnected {
            handle: lien.get(),
            reason: motif,
        });
    }

    fn faire_recevoir(&mut self, _t: &mut Self::T, lien: LinkId, bytes: &[u8]) {
        self.radio.pousser(RadioEvent::Frame {
            handle: lien.get(),
            bytes: bytes.to_vec(),
        });
    }
}

#[test]
fn central_transport_passe_toute_la_suite() {
    suite_complete(&mut BancCentral::default());
}

// --- Au-delà de la suite : ce que seul CentralTransport ajoute ------------

use dengon_ble::{Transport, TransportError, TransportEvent};

fn demarre() -> (CentralTransport<FausseRadio>, FausseRadio) {
    let radio = FausseRadio::default();
    let mut t = CentralTransport::new(radio.clone());
    t.start(TransportConfig::default()).unwrap();
    (t, radio)
}

#[test]
fn le_quota_de_connexions_refuse_le_pair_en_trop() {
    let radio = FausseRadio::default();
    let mut t = CentralTransport::new(radio.clone());
    t.start(TransportConfig {
        max_connections: 1,
        ..TransportConfig::default()
    })
    .unwrap();

    radio.pousser(RadioEvent::Connected {
        handle: 0,
        rssi: None,
    });
    radio.pousser(RadioEvent::Connected {
        handle: 1,
        rssi: None,
    });
    let evenements = t.poll();

    assert_eq!(
        evenements.len(),
        1,
        "un seul PeerConnected : le second est refusé"
    );
    assert_eq!(t.nb_liens(), 1);
    assert_eq!(
        radio.0.lock().unwrap().fermes,
        [1],
        "la radio ferme le pair refusé"
    );
}

#[test]
fn une_trame_trop_grande_est_refusee_sans_toucher_la_radio() {
    let (mut t, radio) = demarre();
    radio.pousser(RadioEvent::Connected {
        handle: 0,
        rssi: None,
    });
    let _ = t.poll();

    assert_eq!(
        t.send(LinkId::new(0), &[0; 513]),
        Err(TransportError::FrameTooLarge {
            size: 513,
            max: 512
        })
    );
    assert!(radio.0.lock().unwrap().ecrites.is_empty());
}

#[test]
fn broadcast_atteint_tous_les_pairs_et_send_le_bon_pair() {
    let (mut t, radio) = demarre();
    radio.pousser(RadioEvent::Connected {
        handle: 10,
        rssi: None,
    });
    radio.pousser(RadioEvent::Connected {
        handle: 11,
        rssi: None,
    });
    let _ = t.poll();

    t.broadcast(b"tous").unwrap();
    t.send(LinkId::new(1), b"un").unwrap();

    let ecrites = radio.0.lock().unwrap().ecrites.clone();
    assert_eq!(
        ecrites,
        [
            (10, b"tous".to_vec()),
            (11, b"tous".to_vec()),
            (11, b"un".to_vec())
        ]
    );
}

#[test]
fn un_send_apres_coupure_non_encore_pollee_rend_unknown_peer() {
    let (mut t, radio) = demarre();
    radio.pousser(RadioEvent::Connected {
        handle: 0,
        rssi: None,
    });
    let _ = t.poll();
    radio.pousser(RadioEvent::Disconnected {
        handle: 0,
        reason: DisconnectReason::Brutale,
    });

    // Pas de `poll` entre la coupure et l'envoi.
    assert_eq!(
        t.send(LinkId::new(0), b"x"),
        Err(TransportError::UnknownPeer(LinkId::new(0)))
    );
    // L'événement de fermeture n'est pas perdu pour autant.
    assert!(t
        .poll()
        .iter()
        .any(|e| matches!(e, TransportEvent::PeerDisconnected { .. })));
}
