//! Suite de conformité `Transport` — un jeu de tests réutilisable tel quel.
//!
//! Un contrat qui n'est vérifié que sur son bouchon n'est pas un contrat : les
//! implémentations dérivent chacune de leur côté, et l'écart se découvre en
//! intégration. Cette suite existe pour que `btleplug` (US-303),
//! `AndroidTransport` (US-213) et NimBLE (US-220) soient jugés sur **les mêmes
//! assertions** que [`MockTransport`](crate::MockTransport).
//!
//! Elle est `pub`, pas `#[cfg(test)]` : sous `cfg(test)`, elle ne serait
//! compilée que pour les tests de *cette* crate et resterait inaccessible aux
//! autres. C'est la raison d'être du module.
//!
//! # Portée réelle
//!
//! La suite est écrite en Rust et pilote tout ce qui se présente comme un
//! [`Transport`] Rust. Les trois implémentations à venir y arrivent, chacune
//! par son pont :
//!
//! | Implémentation | Comment la suite l'atteint | Quand |
//! |---|---|---|
//! | `btleplug` (desktop) | directement, c'est du Rust | US-303 |
//! | `AndroidTransport` (Kotlin) | `Transport` est une **callback interface** UniFFI (`docs/plan-mvp.md:172`) : l'objet Kotlin est passé *dans* Rust et y est vu comme un `Transport` | US-302 puis US-213 |
//! | NimBLE (C) | adaptateur Rust mince au-dessus du *shim* `extern "C"` que le firmware câble déjà | US-307 puis US-220 |
//!
//! Ce sont donc bien **les mêmes assertions, le même code**, et non un portage
//! par plateforme — ce que demande le critère d'acceptation d'US-105.
//!
//! Ce qui reste hors de portée d'US-105, et qui appartient à US-213 et US-220 :
//! écrire le [`BancDEssai`] de chaque plateforme, et le faire tourner sur du
//! **matériel réel** (deux téléphones, deux cartes). Provoquer une vraie
//! coupure brutale demande de couper l'alimentation d'une carte : aucun
//! bouchon ne le simule honnêtement.
//!
//! # Comment l'utiliser
//!
//! Une implémentation fournit un [`BancDEssai`] : la suite ne sait pas
//! connecter un vrai téléphone, seul le banc sait comment provoquer une
//! connexion, une coupure ou l'arrivée d'une trame sur son transport.
//!
//! ```
//! use dengon_ble::conformance::{suite_complete, BancDEssai};
//! use dengon_ble::{DisconnectReason, LinkId, MockTransport, Transport};
//!
//! struct BancMock;
//!
//! impl BancDEssai for BancMock {
//!     type T = MockTransport;
//!
//!     fn nouveau(&mut self) -> Self::T {
//!         MockTransport::new()
//!     }
//!     fn connecter_un_pair(&mut self, t: &mut Self::T) -> LinkId {
//!         t.connecter_pair(Some(-60))
//!     }
//!     fn couper(&mut self, t: &mut Self::T, lien: LinkId, motif: DisconnectReason) {
//!         t.couper_lien(lien, motif);
//!     }
//!     fn faire_recevoir(&mut self, t: &mut Self::T, lien: LinkId, bytes: &[u8]) {
//!         t.injecter_trame(lien, bytes);
//!     }
//! }
//!
//! suite_complete(&mut BancMock);
//! ```

use crate::transport::{
    DisconnectReason, LinkId, Transport, TransportConfig, TransportError, TransportEvent,
};

/// Ce qu'une implémentation doit fournir pour être passée à la suite.
///
/// La suite pilote le transport **par l'extérieur** : elle a besoin de
/// provoquer des événements qu'un `Transport` seul ne sait pas déclencher (on
/// ne « se connecte » pas soi-même à un pair). Chaque implémentation sait le
/// faire à sa façon : le bouchon manipule ses structures, `btleplug` utilise un
/// second adaptateur ou une carte de test.
pub trait BancDEssai {
    /// Le transport que ce banc sait piloter.
    type T: Transport;

    /// Un transport neuf, **non démarré**.
    fn nouveau(&mut self) -> Self::T;

    /// Fait apparaître un pair connecté et rend son lien.
    ///
    /// Le transport est déjà démarré quand la suite appelle cette méthode.
    fn connecter_un_pair(&mut self, t: &mut Self::T) -> LinkId;

    /// Ferme le lien avec le motif demandé.
    ///
    /// Sur matériel réel, [`DisconnectReason::Brutale`] veut dire « couper
    /// l'alimentation de la carte d'en face », pas « appeler `disconnect` ».
    /// C'est toute la différence que ces tests cherchent à établir.
    fn couper(&mut self, t: &mut Self::T, lien: LinkId, motif: DisconnectReason);

    /// Fait arriver une trame **en provenance** du pair.
    fn faire_recevoir(&mut self, t: &mut Self::T, lien: LinkId, bytes: &[u8]);

    /// Configuration utilisée par la suite. Surchargeable si une plateforme a
    /// besoin de réglages particuliers.
    fn config(&self) -> TransportConfig {
        TransportConfig::default()
    }
}

/// Prépare un transport démarré, ou échoue bruyamment.
fn demarre<B: BancDEssai>(banc: &mut B) -> B::T {
    let mut t = banc.nouveau();
    let cfg = banc.config();
    assert!(
        t.start(cfg).is_ok(),
        "conformité : start() sur un transport neuf doit réussir"
    );
    t
}

/// `poll` avant `start` ne panique pas et ne rend rien.
pub fn cas_poll_avant_start_est_vide<B: BancDEssai>(banc: &mut B) {
    let mut t = banc.nouveau();
    assert!(
        t.poll().is_empty(),
        "conformité : poll() avant start() doit rendre un Vec vide, pas paniquer"
    );
}

/// `send` et `broadcast` avant `start` signalent `NotStarted`.
pub fn cas_envoi_avant_start_est_refuse<B: BancDEssai>(banc: &mut B) {
    let mut t = banc.nouveau();
    assert_eq!(
        t.send(LinkId::new(0), b"x"),
        Err(TransportError::NotStarted),
        "conformité : send() avant start() doit rendre NotStarted"
    );
    assert_eq!(
        t.broadcast(b"x"),
        Err(TransportError::NotStarted),
        "conformité : broadcast() avant start() doit rendre NotStarted"
    );
}

/// Un second `start` est refusé — ce n'est pas une reconfiguration.
pub fn cas_double_start_est_refuse<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    let cfg = banc.config();
    assert_eq!(
        t.start(cfg),
        Err(TransportError::AlreadyStarted),
        "conformité : un second start() doit rendre AlreadyStarted"
    );
}

/// Une connexion remonte un `PeerConnected` portant le bon lien.
pub fn cas_connexion_remonte_un_evenement<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    let lien = banc.connecter_un_pair(&mut t);

    let vu = t.poll().into_iter().any(|e| {
        matches!(
            e,
            TransportEvent::PeerConnected { peer_link_id, .. } if peer_link_id == lien
        )
    });
    assert!(
        vu,
        "conformité : une connexion doit produire un PeerConnected portant son LinkId"
    );
}

/// `poll` consomme : un événement ne sort qu'une fois.
pub fn cas_poll_consomme_les_evenements<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    banc.connecter_un_pair(&mut t);

    assert!(
        !t.poll().is_empty(),
        "conformité : le premier poll() doit livrer l'événement de connexion"
    );
    assert!(
        t.poll().is_empty(),
        "conformité : poll() consomme — le même événement ne doit pas ressortir"
    );
}

/// Une trame reçue remonte à l'octet près, sur le bon lien.
pub fn cas_trame_recue_remonte_intacte<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    let lien = banc.connecter_un_pair(&mut t);
    let _ = t.poll();

    let charge = b"dengon-conformite".to_vec();
    banc.faire_recevoir(&mut t, lien, &charge);

    let vu = t.poll().into_iter().any(|e| {
        matches!(
            e,
            TransportEvent::FrameReceived { peer_link_id, ref bytes }
                if peer_link_id == lien && *bytes == charge
        )
    });
    assert!(
        vu,
        "conformité : une trame reçue doit remonter intacte, sur le lien qui l'a livrée"
    );
}

/// `send` vers un pair connecté réussit.
pub fn cas_envoi_vers_un_pair_connecte_reussit<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    let lien = banc.connecter_un_pair(&mut t);
    assert!(
        t.send(lien, b"charge").is_ok(),
        "conformité : send() vers un lien ouvert doit réussir"
    );
}

/// `broadcast` sans aucun pair est un **succès**.
///
/// C'est l'état normal d'un nœud isolé, pas une anomalie : le transformer en
/// erreur ferait remonter du bruit permanent au cœur.
pub fn cas_broadcast_sans_pair_reussit<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    assert!(
        t.broadcast(b"personne").is_ok(),
        "conformité : broadcast() sans pair connecté est un succès, pas une erreur"
    );
}

/// **Le cas central** : coupure brutale.
///
/// Vérifie les points 1, 4 et 5 du contrat écrit sur [`Transport`] :
/// un `PeerDisconnected` portant [`DisconnectReason::Brutale`], puis un `send`
/// qui échoue proprement, puis plus aucun événement sur ce lien.
pub fn cas_deconnexion_brutale<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    let lien = banc.connecter_un_pair(&mut t);
    let _ = t.poll();

    banc.couper(&mut t, lien, DisconnectReason::Brutale);

    let evenements = t.poll();
    let ferme = evenements.iter().any(|e| {
        matches!(
            e,
            TransportEvent::PeerDisconnected { peer_link_id, reason }
                if *peer_link_id == lien && *reason == DisconnectReason::Brutale
        )
    });
    assert!(
        ferme,
        "conformité : une coupure brutale doit produire un PeerDisconnected \
         portant DisconnectReason::Brutale"
    );

    assert_eq!(
        t.send(lien, b"trop tard"),
        Err(TransportError::UnknownPeer(lien)),
        "conformité : send() sur un lien mort doit rendre UnknownPeer, sans paniquer"
    );

    banc.faire_recevoir(&mut t, lien, b"fantome");
    let fantome = t.poll().into_iter().any(
        |e| matches!(e, TransportEvent::FrameReceived { peer_link_id, .. } if peer_link_id == lien),
    );
    assert!(
        !fantome,
        "conformité : plus aucun événement ne doit porter un LinkId fermé"
    );
}

/// Une déconnexion **propre** se distingue d'une coupure brutale.
///
/// Sans cette distinction, la couche du dessus ne peut pas décider s'il faut
/// retenter tout de suite.
pub fn cas_deconnexion_propre_est_distinguee<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    let lien = banc.connecter_un_pair(&mut t);
    let _ = t.poll();

    banc.couper(&mut t, lien, DisconnectReason::Propre);

    let propre = t.poll().into_iter().any(|e| {
        matches!(
            e,
            TransportEvent::PeerDisconnected { peer_link_id, reason }
                if peer_link_id == lien && reason == DisconnectReason::Propre
        )
    });
    assert!(
        propre,
        "conformité : une déconnexion propre doit être signalée comme telle"
    );
}

/// Un `LinkId` n'est jamais réattribué après fermeture.
pub fn cas_link_id_jamais_reutilise<B: BancDEssai>(banc: &mut B) {
    let mut t = demarre(banc);
    let premier = banc.connecter_un_pair(&mut t);
    banc.couper(&mut t, premier, DisconnectReason::Brutale);
    let _ = t.poll();

    let second = banc.connecter_un_pair(&mut t);
    assert_ne!(
        premier, second,
        "conformité : réutiliser un LinkId attribuerait des trames au mauvais pair"
    );
}

/// Lance tous les cas, dans l'ordre.
///
/// Chaque cas repart d'un transport neuf via [`BancDEssai::nouveau`] : ils sont
/// indépendants et peuvent être appelés isolément pour déboguer.
///
/// # Panics
///
/// Panique au premier cas non conforme, avec un message qui dit **quelle règle
/// du contrat** est violée.
pub fn suite_complete<B: BancDEssai>(banc: &mut B) {
    cas_poll_avant_start_est_vide(banc);
    cas_envoi_avant_start_est_refuse(banc);
    cas_double_start_est_refuse(banc);
    cas_connexion_remonte_un_evenement(banc);
    cas_poll_consomme_les_evenements(banc);
    cas_trame_recue_remonte_intacte(banc);
    cas_envoi_vers_un_pair_connecte_reussit(banc);
    cas_broadcast_sans_pair_reussit(banc);
    cas_deconnexion_brutale(banc);
    cas_deconnexion_propre_est_distinguee(banc);
    cas_link_id_jamais_reutilise(banc);
}
