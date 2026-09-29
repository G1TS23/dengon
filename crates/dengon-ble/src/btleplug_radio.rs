//! [`BtleplugRadio`] — la radio BLE desktop, en rôle **central** (US-303).
//!
//! # Ce que ce backend fait — et ne fait pas
//!
//! `btleplug` est *central-only* (Spike B, US-102) : ce nœud **scanne** le
//! service `dengon`, **se connecte** aux pairs qui l'annoncent, s'abonne à leur
//! `CHAR_TX` et écrit sur leur `CHAR_RX`. Il **n'annonce rien** et ne sert
//! aucun GATT : un autre nœud desktop ne peut donc pas le découvrir. Il joue
//! avec un téléphone Android ou un relais ESP32, qui, eux, annoncent.
//!
//! Écarts assumés, consignés dans `docs/suivi/03-ecarts-conception.md` :
//!
//! - **Règle anti-boucle non appliquée.** La règle « le plus petit `peerID`
//!   initie » suppose que les deux côtés peuvent initier. Un nœud qui ne sait
//!   pas être découvert doit initier vers **tous** les pairs, sinon ceux dont
//!   le `peerID` est plus petit ne le joindraient jamais. Au pire, un lien en
//!   double si le pair nous initie aussi (impossible ici : on n'annonce pas).
//! - **Pas de MTU négocié explicitement** : `btleplug` ne l'expose pas, la pile
//!   de l'OS le négocie seule. On plafonne donc à la valeur d'attribut GATT
//!   maximale (512) et on laisse la pile refuser ce qui ne passe pas.
//! - **Motif de coupure** : `btleplug` ne dit pas si le pair a fermé
//!   proprement ; toute perte de lien est rapportée [`DisconnectReason::Brutale`],
//!   sauf la fermeture décidée localement ([`DisconnectReason::Locale`]).
//!
//! # Architecture
//!
//! `btleplug` est asynchrone, `Transport` ne l'est pas. La radio lance donc un
//! fil qui porte un exécuteur `tokio` ; `poll`/`write` du côté synchrone ne
//! font que lire/écrire des files (jamais d'attente sur la radio).

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use btleplug::api::{
    Central as _, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::StreamExt as _;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::central::{CentralRadio, CentralTransport, RadioEvent, RadioHandle};
use crate::transport::{DisconnectReason, Result, TransportConfig, TransportError};

/// Le transport desktop : la logique de liens de [`CentralTransport`] sur la
/// radio `btleplug`.
pub type BtleplugTransport = CentralTransport<BtleplugRadio>;

const SERVICE: Uuid = Uuid::from_bytes(dengon_core::protocol::consts::SERVICE_UUID);
const CHAR_RX: Uuid = Uuid::from_bytes(dengon_core::protocol::consts::CHAR_RX_UUID);
const CHAR_TX: Uuid = Uuid::from_bytes(dengon_core::protocol::consts::CHAR_TX_UUID);

/// Longueur maximale d'une valeur d'attribut GATT (spec Bluetooth, vol. 3 F).
const VALEUR_MAX: usize = 512;

/// Délai d'attente du démarrage de l'adaptateur.
const DELAI_DEMARRAGE: Duration = Duration::from_secs(10);

/// Ordres du côté synchrone vers la tâche d'une connexion.
#[derive(Debug)]
enum OrdreLien {
    Ecrire(Vec<u8>),
    Fermer,
    /// L'adaptateur a signalé la déconnexion.
    Perdu,
}

type Liens = Arc<Mutex<HashMap<RadioHandle, UnboundedSender<OrdreLien>>>>;

/// Radio BLE desktop en rôle central.
#[derive(Debug)]
pub struct BtleplugRadio {
    evenements: Option<Receiver<RadioEvent>>,
    liens: Liens,
    fil: Option<JoinHandle<()>>,
    arret: Option<tokio::sync::oneshot::Sender<()>>,
}

impl BtleplugRadio {
    /// Radio non démarrée.
    #[must_use]
    pub fn new() -> Self {
        Self {
            evenements: None,
            liens: Arc::new(Mutex::new(HashMap::new())),
            fil: None,
            arret: None,
        }
    }

    /// Transport prêt à l'emploi : `BtleplugTransport::new(BtleplugRadio::new())`.
    #[must_use]
    pub fn transport() -> BtleplugTransport {
        CentralTransport::new(Self::new())
    }
}

impl Default for BtleplugRadio {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BtleplugRadio {
    fn drop(&mut self) {
        if let Some(arret) = self.arret.take() {
            let _ = arret.send(());
        }
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
    }
}

fn backend<E: std::fmt::Display>(e: E) -> TransportError {
    TransportError::Backend(e.to_string())
}

impl CentralRadio for BtleplugRadio {
    fn start(&mut self, cfg: &TransportConfig) -> Result<()> {
        let (tx_evt, rx_evt) = mpsc::channel();
        let (tx_pret, rx_pret) = mpsc::channel::<std::result::Result<(), String>>();
        let (tx_arret, rx_arret) = tokio::sync::oneshot::channel();
        let liens = Arc::clone(&self.liens);
        let scan = cfg.scan;

        let fil = std::thread::Builder::new()
            .name("dengon-btleplug".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx_pret.send(Err(e.to_string()));
                        return;
                    }
                };
                runtime.block_on(boucle(scan, tx_evt, liens, tx_pret, rx_arret));
            })
            .map_err(backend)?;

        match rx_pret.recv_timeout(DELAI_DEMARRAGE) {
            Ok(Ok(())) => {
                self.evenements = Some(rx_evt);
                self.fil = Some(fil);
                self.arret = Some(tx_arret);
                Ok(())
            }
            Ok(Err(e)) => Err(TransportError::Backend(e)),
            Err(_) => Err(TransportError::Backend(
                "démarrage de l'adaptateur BLE trop long".into(),
            )),
        }
    }

    fn poll(&mut self) -> Vec<RadioEvent> {
        self.evenements
            .as_ref()
            .map_or_else(Vec::new, |rx| rx.try_iter().collect())
    }

    fn write(&mut self, handle: RadioHandle, bytes: &[u8]) -> Result<()> {
        let liens = self.liens.lock().map_err(backend)?;
        let lien = liens
            .get(&handle)
            .ok_or_else(|| TransportError::Backend(format!("connexion {handle} fermée")))?;
        lien.send(OrdreLien::Ecrire(bytes.to_vec()))
            .map_err(|_| TransportError::Backend(format!("connexion {handle} fermée")))
    }

    fn disconnect(&mut self, handle: RadioHandle) {
        if let Ok(liens) = self.liens.lock() {
            if let Some(lien) = liens.get(&handle) {
                let _ = lien.send(OrdreLien::Fermer);
            }
        }
    }

    fn max_frame(&self) -> usize {
        VALEUR_MAX
    }
}

/// Tâche principale : scan, puis une tâche par pair découvert.
async fn boucle(
    scan: bool,
    tx_evt: Sender<RadioEvent>,
    liens: Liens,
    pret: Sender<std::result::Result<(), String>>,
    mut arret: tokio::sync::oneshot::Receiver<()>,
) {
    let adaptateur = match ouvrir_adaptateur().await {
        Ok(a) => a,
        Err(e) => {
            let _ = pret.send(Err(e));
            return;
        }
    };
    let mut flux = match adaptateur.events().await {
        Ok(f) => f,
        Err(e) => {
            let _ = pret.send(Err(e.to_string()));
            return;
        }
    };
    if scan {
        let filtre = ScanFilter {
            services: vec![SERVICE],
        };
        if let Err(e) = adaptateur.start_scan(filtre).await {
            let _ = pret.send(Err(e.to_string()));
            return;
        }
    }
    let _ = pret.send(Ok(()));

    let mut prochain_handle: RadioHandle = 0;
    // Pairs déjà pris en charge : évite de se reconnecter à chaque annonce.
    let mut connus: HashMap<PeripheralId, RadioHandle> = HashMap::new();
    // Tâches de connexion terminées : le pair redevient éligible au scan
    // (sans quoi un échec de connexion ne serait jamais retenté).
    let (tx_fini, mut rx_fini) = unbounded_channel::<PeripheralId>();

    loop {
        tokio::select! {
            _ = &mut arret => break,
            Some(id) = rx_fini.recv() => {
                connus.remove(&id);
            }
            evt = flux.next() => {
                let Some(evt) = evt else { break };
                match evt {
                    CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => {
                        if connus.contains_key(&id) {
                            continue;
                        }
                        let Ok(pair) = adaptateur.peripheral(&id).await else { continue };
                        if !annonce_dengon(&pair).await {
                            continue;
                        }
                        let handle = prochain_handle;
                        prochain_handle += 1;
                        connus.insert(id.clone(), handle);
                        let (tx_ordre, rx_ordre) = unbounded_channel();
                        if let Ok(mut l) = liens.lock() {
                            l.insert(handle, tx_ordre);
                        }
                        tokio::spawn(tache_lien(
                            pair,
                            id,
                            tx_fini.clone(),
                            handle,
                            tx_evt.clone(),
                            rx_ordre,
                            Arc::clone(&liens),
                        ));
                    }
                    CentralEvent::DeviceDisconnected(id) => {
                        // Un pair qui revient recevra un NOUVEAU handle :
                        // un `LinkId` n'est jamais réutilisé.
                        if let Some(handle) = connus.remove(&id) {
                            if let Ok(l) = liens.lock() {
                                if let Some(lien) = l.get(&handle) {
                                    let _ = lien.send(OrdreLien::Perdu);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let _ = adaptateur.stop_scan().await;
}

async fn ouvrir_adaptateur() -> std::result::Result<Adapter, String> {
    let manager = Manager::new().await.map_err(|e| e.to_string())?;
    let adaptateurs = manager.adapters().await.map_err(|e| e.to_string())?;
    adaptateurs
        .into_iter()
        .next()
        .ok_or_else(|| "aucun adaptateur Bluetooth trouvé".to_string())
}

/// L'annonce contient-elle le service `dengon` ?
async fn annonce_dengon(pair: &Peripheral) -> bool {
    pair.properties()
        .await
        .ok()
        .flatten()
        .is_some_and(|p| p.services.contains(&SERVICE))
}

/// Vie d'une connexion : connexion, découverte, abonnement, puis boucle.
async fn tache_lien(
    pair: Peripheral,
    id: PeripheralId,
    fini: UnboundedSender<PeripheralId>,
    handle: RadioHandle,
    evt: Sender<RadioEvent>,
    mut ordres: UnboundedReceiver<OrdreLien>,
    liens: Liens,
) {
    let raison = vivre(&pair, handle, &evt, &mut ordres).await;
    // Retire d'abord le lien : plus aucun `write` ne doit l'atteindre.
    if let Ok(mut l) = liens.lock() {
        l.remove(&handle);
    }
    if let Some(raison) = raison {
        let _ = pair.disconnect().await;
        let _ = evt.send(RadioEvent::Disconnected {
            handle,
            reason: raison,
        });
    } else {
        let _ = pair.disconnect().await;
    }
    let _ = fini.send(id);
}

/// Rend la raison de fin si la connexion avait été annoncée au cœur ; `None`
/// si elle n'a jamais abouti (rien à fermer côté événements).
async fn vivre(
    pair: &Peripheral,
    handle: RadioHandle,
    evt: &Sender<RadioEvent>,
    ordres: &mut UnboundedReceiver<OrdreLien>,
) -> Option<DisconnectReason> {
    if pair.connect().await.is_err() || pair.discover_services().await.is_err() {
        return None;
    }
    let caracs = pair.characteristics();
    let rx: Characteristic = caracs.iter().find(|c| c.uuid == CHAR_RX)?.clone();
    let tx: Characteristic = caracs.iter().find(|c| c.uuid == CHAR_TX)?.clone();
    let mut notifications = pair.notifications().await.ok()?;
    pair.subscribe(&tx).await.ok()?;

    let rssi = pair.properties().await.ok().flatten().and_then(|p| p.rssi);
    let _ = evt.send(RadioEvent::Connected { handle, rssi });

    loop {
        tokio::select! {
            notif = notifications.next() => match notif {
                Some(n) if n.uuid == CHAR_TX => {
                    let _ = evt.send(RadioEvent::Frame { handle, bytes: n.value });
                }
                Some(_) => {}
                None => return Some(DisconnectReason::Brutale),
            },
            ordre = ordres.recv() => match ordre {
                Some(OrdreLien::Ecrire(octets)) => {
                    if pair.write(&rx, &octets, WriteType::WithoutResponse).await.is_err() {
                        return Some(DisconnectReason::Brutale);
                    }
                }
                Some(OrdreLien::Fermer) => return Some(DisconnectReason::Locale),
                Some(OrdreLien::Perdu) | None => return Some(DisconnectReason::Brutale),
            },
        }
    }
}
