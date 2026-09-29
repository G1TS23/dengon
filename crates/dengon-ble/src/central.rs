//! [`CentralTransport`] — l'implémentation de [`Transport`] pour une radio qui
//! ne sait tenir que le rôle **central** (US-303).
//!
//! # Pourquoi ce module existe à part de `btleplug`
//!
//! Le Spike B (US-102) a établi que `btleplug` ne sait ni annoncer ni servir du
//! GATT : il ne fait que scanner, se connecter, écrire et s'abonner. Tout le
//! reste du contrat [`Transport`] — attribution des [`LinkId`], quota de
//! connexions, ordre des événements, comportement en coupure brutale — est de
//! la **logique pure**, indépendante de la pile BLE. La placer ici, derrière le
//! petit trait [`CentralRadio`], permet de la **passer à la suite de
//! conformité sans matériel** : les tests branchent une fausse radio, et le
//! backend `btleplug` (module `btleplug_radio`, feature `btleplug`) n'a plus
//! qu'à traduire les appels de la pile.
//!
//! # Ce que la radio doit garantir
//!
//! - un [`RadioHandle`] n'est **jamais réutilisé** pour une autre connexion ;
//! - pour un handle donné, les événements sortent **dans l'ordre** où ils se
//!   sont produits (donc les trames reçues avant la coupure avant
//!   [`RadioEvent::Disconnected`]) ;
//! - les fragments BLE incomplets sont jetés par la radio, jamais remontés.
//!
//! Le [`CentralTransport`] traduit les handles en [`LinkId`] à lui, filtre tout
//! ce qui arrive après la fermeture d'un lien, et applique le quota.

use std::collections::BTreeMap;

use crate::transport::{
    DisconnectReason, LinkId, Result, Transport, TransportConfig, TransportError, TransportEvent,
};

/// Identifiant d'une connexion, attribué par la radio. Opaque pour le transport.
pub type RadioHandle = u64;

/// Ce que la radio remonte au [`CentralTransport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadioEvent {
    /// Connexion GATT établie, service découvert, abonnement `TX` actif.
    Connected {
        /// Handle de la nouvelle connexion.
        handle: RadioHandle,
        /// RSSI vu au scan, si connu.
        rssi: Option<i16>,
    },
    /// Connexion fermée.
    Disconnected {
        /// Handle fermé.
        handle: RadioHandle,
        /// Cause de la fermeture.
        reason: DisconnectReason,
    },
    /// Une notification `TX` complète est arrivée.
    Frame {
        /// Handle qui l'a livrée.
        handle: RadioHandle,
        /// Octets reçus.
        bytes: Vec<u8>,
    },
}

/// Une radio BLE en rôle central, non bloquante.
pub trait CentralRadio: Send {
    /// Lance le scan (et les connexions aux pairs `dengon` découverts).
    ///
    /// # Errors
    ///
    /// [`TransportError::Backend`] si la pile refuse (adaptateur absent…).
    fn start(&mut self, cfg: &TransportConfig) -> Result<()>;

    /// Retire les événements accumulés. Ne bloque jamais.
    fn poll(&mut self) -> Vec<RadioEvent>;

    /// Écrit une trame sur `CHAR_RX` du pair (write-without-response).
    ///
    /// # Errors
    ///
    /// [`TransportError::Backend`] si la pile échoue.
    fn write(&mut self, handle: RadioHandle, bytes: &[u8]) -> Result<()>;

    /// Ferme la connexion (quota atteint, arrêt). Sans effet si elle est morte.
    fn disconnect(&mut self, handle: RadioHandle);

    /// Taille maximale d'une trame acceptée par [`write`](Self::write).
    fn max_frame(&self) -> usize;
}

/// [`Transport`] bâti sur une [`CentralRadio`].
#[derive(Debug)]
pub struct CentralTransport<R: CentralRadio> {
    radio: R,
    demarre: bool,
    plafond: usize,
    prochain_lien: u64,
    /// Liens vivants : handle radio → lien exposé au cœur.
    vivants: BTreeMap<RadioHandle, LinkId>,
    file: Vec<TransportEvent>,
}

impl<R: CentralRadio> CentralTransport<R> {
    /// Enveloppe une radio. Le transport n'est pas démarré.
    #[must_use]
    pub fn new(radio: R) -> Self {
        Self {
            radio,
            demarre: false,
            plafond: usize::MAX,
            prochain_lien: 0,
            vivants: BTreeMap::new(),
            file: Vec::new(),
        }
    }

    /// Accès à la radio (bancs d'essai, diagnostics).
    pub fn radio_mut(&mut self) -> &mut R {
        &mut self.radio
    }

    /// Nombre de liens ouverts.
    #[must_use]
    pub fn nb_liens(&self) -> usize {
        self.vivants.len()
    }

    /// Vide la radio vers `self.file`, en appliquant les règles du contrat.
    fn pomper(&mut self) {
        for evenement in self.radio.poll() {
            match evenement {
                RadioEvent::Connected { handle, rssi } => {
                    if self.vivants.contains_key(&handle) {
                        continue;
                    }
                    if self.vivants.len() >= self.plafond {
                        self.radio.disconnect(handle);
                        continue;
                    }
                    // Compteur monotone : un `LinkId` n'est jamais réutilisé.
                    let lien = LinkId::new(self.prochain_lien);
                    self.prochain_lien += 1;
                    self.vivants.insert(handle, lien);
                    self.file.push(TransportEvent::PeerConnected {
                        peer_link_id: lien,
                        rssi,
                    });
                }
                RadioEvent::Disconnected { handle, reason } => {
                    if let Some(lien) = self.vivants.remove(&handle) {
                        self.file.push(TransportEvent::PeerDisconnected {
                            peer_link_id: lien,
                            reason,
                        });
                    }
                }
                RadioEvent::Frame { handle, bytes } => {
                    // Après fermeture, le handle n'est plus dans `vivants` :
                    // la trame fantôme est jetée (règle 5 du contrat).
                    if let Some(&lien) = self.vivants.get(&handle) {
                        self.file.push(TransportEvent::FrameReceived {
                            peer_link_id: lien,
                            bytes,
                        });
                    }
                }
            }
        }
    }

    fn handle_de(&self, lien: LinkId) -> Option<RadioHandle> {
        self.vivants
            .iter()
            .find_map(|(h, l)| (*l == lien).then_some(*h))
    }

    fn verifie_taille(&self, bytes: &[u8]) -> Result<()> {
        let max = self.radio.max_frame();
        if bytes.len() > max {
            return Err(TransportError::FrameTooLarge {
                size: bytes.len(),
                max,
            });
        }
        Ok(())
    }
}

impl<R: CentralRadio> Transport for CentralTransport<R> {
    fn start(&mut self, cfg: TransportConfig) -> Result<()> {
        if self.demarre {
            return Err(TransportError::AlreadyStarted);
        }
        self.radio.start(&cfg)?;
        self.plafond = cfg.max_connections;
        self.demarre = true;
        Ok(())
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        if !self.demarre {
            return Vec::new();
        }
        self.pomper();
        core::mem::take(&mut self.file)
    }

    fn send(&mut self, peer_link_id: LinkId, bytes: &[u8]) -> Result<()> {
        if !self.demarre {
            return Err(TransportError::NotStarted);
        }
        self.verifie_taille(bytes)?;
        // Rafraîchit l'état : une coupure survenue depuis le dernier `poll`
        // doit déjà rendre `UnknownPeer`.
        self.pomper();
        let handle = self
            .handle_de(peer_link_id)
            .ok_or(TransportError::UnknownPeer(peer_link_id))?;
        self.radio.write(handle, bytes)
    }

    fn broadcast(&mut self, bytes: &[u8]) -> Result<()> {
        if !self.demarre {
            return Err(TransportError::NotStarted);
        }
        self.verifie_taille(bytes)?;
        self.pomper();
        let handles: Vec<RadioHandle> = self.vivants.keys().copied().collect();
        for handle in handles {
            // « Au mieux » : l'échec vers un pair ne prive pas les autres.
            let _ = self.radio.write(handle, bytes);
        }
        Ok(())
    }
}
