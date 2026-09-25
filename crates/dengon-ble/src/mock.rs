//! [`MockTransport`] — un transport en mémoire, sans radio.
//!
//! Il existe pour une raison de planning autant que de test : `sync::routing`
//! (US-209) et `dengon-sim` (US-221) doivent pouvoir démarrer **avant** que le
//! BLE existe. Sans bouchon, toute la moitié « cœur » du backlog attendrait la
//! moitié « radio ».
//!
//! Ce n'est pas un simulateur réseau : ni latence, ni perte, ni partition.
//! `dengon-sim` construira ça par-dessus (`10-benchmarks-mvp-tests.md` §4.3).
//! Ici, tout est immédiat et fiable, et c'est le **test** qui décide de ce qui
//! arrive, en appelant les méthodes de pilotage.

use std::collections::BTreeMap;

use crate::transport::{
    DisconnectReason, LinkId, Result, Transport, TransportConfig, TransportError, TransportEvent,
};

/// Ce qu'un pair simulé a reçu.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Pair {
    /// Trames que le nœud sous test lui a envoyées, dans l'ordre.
    recues: Vec<Vec<u8>>,
    /// Lien encore ouvert ?
    connecte: bool,
}

/// Transport en mémoire, piloté par le test.
///
/// Deux familles de méthodes, à ne pas confondre :
///
/// - l'implémentation de [`Transport`], celle que le code sous test utilise ;
/// - les **méthodes de pilotage** (`connecter_pair`, `couper_lien`,
///   `injecter_trame`, `trames_envoyees_a`…), que seul le test appelle, pour
///   jouer le rôle du monde extérieur.
///
/// # Exemple
///
/// ```
/// use dengon_ble::{MockTransport, Transport, TransportConfig, TransportEvent};
///
/// let mut t = MockTransport::new();
/// t.start(TransportConfig::default())?;
///
/// let lien = t.connecter_pair(Some(-60));
/// let evenements = t.poll();
/// assert!(matches!(
///     evenements.first(),
///     Some(TransportEvent::PeerConnected { .. })
/// ));
///
/// t.send(lien, b"bonjour")?;
/// assert_eq!(t.trames_envoyees_a(lien), [b"bonjour".to_vec()]);
/// # Ok::<(), dengon_ble::TransportError>(())
/// ```
#[derive(Debug, Default)]
pub struct MockTransport {
    demarre: bool,
    cfg: Option<TransportConfig>,
    /// Compteur monotone : un `LinkId` n'est jamais réutilisé, comme l'exige
    /// le contrat de [`LinkId`].
    prochain_lien: u64,
    pairs: BTreeMap<LinkId, Pair>,
    file: Vec<TransportEvent>,
    diffusions: Vec<Vec<u8>>,
    /// Plafond de taille de trame, pour pouvoir provoquer
    /// [`TransportError::FrameTooLarge`] dans un test.
    taille_max: usize,
}

impl MockTransport {
    /// Un transport neuf, arrêté, sans pair.
    #[must_use]
    pub fn new() -> Self {
        Self {
            taille_max: usize::MAX,
            ..Self::default()
        }
    }

    /// Fixe la taille de trame au-delà de laquelle `send` et `broadcast`
    /// échouent.
    ///
    /// Sert à exercer le chemin d'erreur sans avoir besoin d'une vraie radio.
    #[must_use]
    pub fn avec_taille_max(mut self, max: usize) -> Self {
        self.taille_max = max;
        self
    }

    // --- Pilotage : ce que le test fait « arriver » ------------------------

    /// Simule l'arrivée d'un pair et rend son lien.
    ///
    /// Émet un [`TransportEvent::PeerConnected`]. Si le quota
    /// [`TransportConfig::max_connections`] est atteint, le pair est refusé :
    /// aucun événement, et le lien rendu est déjà fermé — ce que le test
    /// constate avec [`Self::est_connecte`].
    pub fn connecter_pair(&mut self, rssi: Option<i16>) -> LinkId {
        let lien = LinkId::new(self.prochain_lien);
        self.prochain_lien += 1;

        let plafond = self.cfg.as_ref().map_or(usize::MAX, |c| c.max_connections);
        if self.pairs.values().filter(|p| p.connecte).count() >= plafond {
            self.pairs.insert(
                lien,
                Pair {
                    recues: Vec::new(),
                    connecte: false,
                },
            );
            return lien;
        }

        self.pairs.insert(
            lien,
            Pair {
                recues: Vec::new(),
                connecte: true,
            },
        );
        self.file.push(TransportEvent::PeerConnected {
            peer_link_id: lien,
            rssi,
        });
        lien
    }

    /// Ferme un lien avec le motif donné et émet
    /// [`TransportEvent::PeerDisconnected`].
    ///
    /// Sur un lien déjà fermé ou inconnu, ne fait rien — un transport réel ne
    /// panique pas non plus dans ce cas.
    pub fn couper_lien(&mut self, lien: LinkId, motif: DisconnectReason) {
        let Some(pair) = self.pairs.get_mut(&lien) else {
            return;
        };
        if !pair.connecte {
            return;
        }
        pair.connecte = false;
        self.file.push(TransportEvent::PeerDisconnected {
            peer_link_id: lien,
            reason: motif,
        });
    }

    /// Coupe un lien **brutalement** : le pair est parti sans prévenir.
    ///
    /// Raccourci sur [`Self::couper_lien`] avec
    /// [`DisconnectReason::Brutale`]. C'est le cas que la suite de conformité
    /// exerce en priorité, parce que c'est le cas courant sur le terrain.
    pub fn couper_brutalement(&mut self, lien: LinkId) {
        self.couper_lien(lien, DisconnectReason::Brutale);
    }

    /// Simule une trame reçue **du** pair.
    ///
    /// Ignorée si le lien est fermé : c'est exactement ce qu'exige le contrat
    /// de déconnexion (« ne plus jamais émettre d'événement portant ce
    /// `LinkId` »).
    pub fn injecter_trame(&mut self, lien: LinkId, bytes: &[u8]) {
        if self.pairs.get(&lien).is_some_and(|p| p.connecte) {
            self.file.push(TransportEvent::FrameReceived {
                peer_link_id: lien,
                bytes: bytes.to_vec(),
            });
        }
    }

    // --- Observation : ce que le test vérifie -----------------------------

    /// Les trames envoyées à ce pair, dans l'ordre. Vide si le lien est
    /// inconnu.
    #[must_use]
    pub fn trames_envoyees_a(&self, lien: LinkId) -> &[Vec<u8>] {
        self.pairs.get(&lien).map_or(&[], |p| &p.recues)
    }

    /// Les trames passées à [`Transport::broadcast`], dans l'ordre.
    #[must_use]
    pub fn diffusions(&self) -> &[Vec<u8>] {
        &self.diffusions
    }

    /// Le lien est-il encore ouvert ?
    #[must_use]
    pub fn est_connecte(&self, lien: LinkId) -> bool {
        self.pairs.get(&lien).is_some_and(|p| p.connecte)
    }

    /// Nombre de liens ouverts.
    #[must_use]
    pub fn nb_pairs_connectes(&self) -> usize {
        self.pairs.values().filter(|p| p.connecte).count()
    }

    /// La configuration passée à [`Transport::start`], si le transport tourne.
    #[must_use]
    pub fn config(&self) -> Option<&TransportConfig> {
        self.cfg.as_ref()
    }

    fn verifie_taille(&self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.taille_max {
            return Err(TransportError::FrameTooLarge {
                size: bytes.len(),
                max: self.taille_max,
            });
        }
        Ok(())
    }
}

impl Transport for MockTransport {
    fn start(&mut self, cfg: TransportConfig) -> Result<()> {
        if self.demarre {
            return Err(TransportError::AlreadyStarted);
        }
        self.demarre = true;
        self.cfg = Some(cfg);
        Ok(())
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        if !self.demarre {
            return Vec::new();
        }
        core::mem::take(&mut self.file)
    }

    fn send(&mut self, peer_link_id: LinkId, bytes: &[u8]) -> Result<()> {
        if !self.demarre {
            return Err(TransportError::NotStarted);
        }
        self.verifie_taille(bytes)?;

        match self.pairs.get_mut(&peer_link_id) {
            Some(pair) if pair.connecte => {
                pair.recues.push(bytes.to_vec());
                Ok(())
            }
            _ => Err(TransportError::UnknownPeer(peer_link_id)),
        }
    }

    fn broadcast(&mut self, bytes: &[u8]) -> Result<()> {
        if !self.demarre {
            return Err(TransportError::NotStarted);
        }
        self.verifie_taille(bytes)?;

        self.diffusions.push(bytes.to_vec());
        for pair in self.pairs.values_mut().filter(|p| p.connecte) {
            pair.recues.push(bytes.to_vec());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MockTransport;
    use crate::transport::{
        DisconnectReason, LinkId, Transport, TransportConfig, TransportError, TransportEvent,
    };

    fn demarre() -> MockTransport {
        let mut t = MockTransport::new();
        assert!(t.start(TransportConfig::default()).is_ok());
        t
    }

    #[test]
    fn poll_avant_start_ne_rend_rien_et_ne_panique_pas() {
        let mut t = MockTransport::new();
        assert!(t.poll().is_empty());
    }

    #[test]
    fn send_avant_start_signale_not_started() {
        let mut t = MockTransport::new();
        assert_eq!(
            t.send(LinkId::new(0), b"x"),
            Err(TransportError::NotStarted)
        );
        assert_eq!(t.broadcast(b"x"), Err(TransportError::NotStarted));
    }

    #[test]
    fn deux_start_successifs_sont_refuses() {
        let mut t = demarre();
        assert_eq!(
            t.start(TransportConfig::default()),
            Err(TransportError::AlreadyStarted)
        );
    }

    #[test]
    fn la_config_est_conservee() {
        let mut t = MockTransport::new();
        let cfg = TransportConfig {
            local_peer_id: [1, 2, 3, 4, 5, 6, 7, 8],
            ..TransportConfig::default()
        };
        assert!(t.start(cfg.clone()).is_ok());
        assert_eq!(t.config(), Some(&cfg));
    }

    #[test]
    fn une_connexion_produit_un_evenement_avec_son_rssi() {
        let mut t = demarre();
        let lien = t.connecter_pair(Some(-55));
        assert_eq!(
            t.poll(),
            vec![TransportEvent::PeerConnected {
                peer_link_id: lien,
                rssi: Some(-55),
            }]
        );
    }

    #[test]
    fn poll_consomme_les_evenements() {
        let mut t = demarre();
        t.connecter_pair(None);
        assert_eq!(t.poll().len(), 1);
        assert!(t.poll().is_empty(), "un événement ne sort qu'une fois");
    }

    #[test]
    fn une_trame_injectee_remonte_telle_quelle() {
        let mut t = demarre();
        let lien = t.connecter_pair(None);
        t.injecter_trame(lien, b"salut");
        let evenements = t.poll();
        assert_eq!(
            evenements.last(),
            Some(&TransportEvent::FrameReceived {
                peer_link_id: lien,
                bytes: b"salut".to_vec(),
            })
        );
    }

    #[test]
    fn send_atteint_le_bon_pair_et_pas_les_autres() {
        let mut t = demarre();
        let a = t.connecter_pair(None);
        let b = t.connecter_pair(None);
        assert!(t.send(a, b"pour-a").is_ok());
        assert_eq!(t.trames_envoyees_a(a), [b"pour-a".to_vec()]);
        assert!(t.trames_envoyees_a(b).is_empty());
    }

    #[test]
    fn broadcast_sert_tout_le_monde_et_reussit_sans_pair() {
        let mut t = demarre();
        assert!(
            t.broadcast(b"seul").is_ok(),
            "un nœud isolé n'est pas en erreur"
        );

        let a = t.connecter_pair(None);
        let b = t.connecter_pair(None);
        assert!(t.broadcast(b"a-tous").is_ok());
        assert_eq!(t.trames_envoyees_a(a), [b"a-tous".to_vec()]);
        assert_eq!(t.trames_envoyees_a(b), [b"a-tous".to_vec()]);
        assert_eq!(t.diffusions(), [b"seul".to_vec(), b"a-tous".to_vec()]);
    }

    #[test]
    fn une_trame_trop_grande_est_refusee() {
        let mut t = MockTransport::new().avec_taille_max(4);
        assert!(t.start(TransportConfig::default()).is_ok());
        let lien = t.connecter_pair(None);
        assert_eq!(
            t.send(lien, b"beaucoup-trop-long"),
            Err(TransportError::FrameTooLarge { size: 18, max: 4 })
        );
    }

    #[test]
    fn le_quota_de_connexions_est_respecte() {
        let mut t = MockTransport::new();
        let cfg = TransportConfig {
            max_connections: 1,
            ..TransportConfig::default()
        };
        assert!(t.start(cfg).is_ok());

        let premier = t.connecter_pair(None);
        let refuse = t.connecter_pair(None);
        assert!(t.est_connecte(premier));
        assert!(!t.est_connecte(refuse), "le second dépasse le quota");
        assert_eq!(t.nb_pairs_connectes(), 1);
    }

    #[test]
    fn un_link_id_n_est_jamais_reutilise() {
        let mut t = demarre();
        let a = t.connecter_pair(None);
        t.couper_brutalement(a);
        let b = t.connecter_pair(None);
        assert_ne!(
            a, b,
            "réutiliser un LinkId attribuerait des trames au mauvais pair"
        );
    }

    #[test]
    fn couper_un_lien_deja_ferme_ne_produit_rien() {
        let mut t = demarre();
        let lien = t.connecter_pair(None);
        t.couper_brutalement(lien);
        let _ = t.poll();
        t.couper_lien(lien, DisconnectReason::Propre);
        assert!(t.poll().is_empty(), "pas de second événement de fermeture");
    }
}
