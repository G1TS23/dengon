//! Réseau en mémoire et [`SimTransport`], l'implémentation simulée de
//! [`Transport`].
//!
//! Tous les nœuds d'une simulation partagent un même [`Reseau`] : une horloge
//! **virtuelle**, un générateur [`Alea`] à graine fixe, la topologie (qui est à
//! portée de qui, avec quelle latence et quel taux de perte), une éventuelle
//! partition, et une file d'événements datés par nœud. Rien ne dépend de
//! l'horloge système ni de l'ordonnancement des fils : c'est ce qui rend une
//! exécution rejouable à l'identique.
//!
//! # Modèle
//!
//! - Une **arête** `a — b` dit que deux nœuds sont à portée radio, avec des
//!   [`ParametresLien`]. Dès que les deux transports sont démarrés, dans le même
//!   groupe de partition, et sous leur quota `max_connections`, une
//!   **connexion** s'établit : chaque côté reçoit son propre [`LinkId`] (jamais
//!   réutilisé) et un `PeerConnected`.
//! - `send` tire la perte puis la gigue dans l'[`Alea`] partagé, et dépose un
//!   `FrameReceived` chez le destinataire à `maintenant + latence + gigue`.
//!   L'ordre **par lien** est préservé : une trame n'arrive jamais avant la
//!   précédente sur le même sens du même lien, comme l'exige le contrat.
//! - `poll` rend les événements dont la date est échue, dans l'ordre
//!   `(date, numéro d'ordre)`.
//! - Une coupure (partition, arête retirée, [`ReseauPartage::couper_lien`])
//!   livre d'abord ce qui est **déjà arrivé**, jette ce qui est **encore en
//!   vol**, puis émet `PeerDisconnected` des deux côtés.
//!
//! Chaque action notable est ajoutée à la [trace](Horodate) : c'est elle que
//! les tests de déterminisme comparent.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use dengon_ble::{
    DisconnectReason, LinkId, Result, Transport, TransportConfig, TransportError, TransportEvent,
};

use crate::alea::Alea;

/// Indice d'un nœud dans la simulation (`0..N`).
pub type NoeudId = usize;

/// Qualité d'une arête radio, dans les deux sens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParametresLien {
    /// Délai fixe avant arrivée d'une trame, en ms virtuelles.
    pub latence_ms: u64,
    /// Délai aléatoire ajouté, tiré uniformément dans `0..=gigue_ms`.
    pub gigue_ms: u64,
    /// Probabilité de perte d'une trame, en pour mille (`0..=1000`).
    pub perte_pour_mille: u16,
}

/// Une entrée de la trace d'exécution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntreeTrace {
    /// Connexion établie entre `a` et `b`.
    Connexion {
        a: NoeudId,
        b: NoeudId,
        lien_a: LinkId,
        lien_b: LinkId,
    },
    /// Connexion fermée ; chaque côté voit son propre motif.
    Coupure {
        a: NoeudId,
        b: NoeudId,
        motif_a: DisconnectReason,
        motif_b: DisconnectReason,
    },
    /// Trame acceptée par le réseau, arrivée prévue à `arrivee_ms`.
    Emission {
        de: NoeudId,
        vers: NoeudId,
        octets: Vec<u8>,
        arrivee_ms: u64,
    },
    /// Trame perdue (tirage de perte).
    Perte {
        de: NoeudId,
        vers: NoeudId,
        octets: Vec<u8>,
    },
    /// Trame en vol jetée par une coupure.
    PerteEnVol {
        de: NoeudId,
        vers: NoeudId,
        octets: Vec<u8>,
    },
    /// Trame remise au nœud par `poll`.
    Reception {
        noeud: NoeudId,
        de: NoeudId,
        octets: Vec<u8>,
    },
    /// Charge remise à l'application par le comportement du nœud.
    Livraison { noeud: NoeudId, octets: Vec<u8> },
}

/// Une entrée de trace et sa date virtuelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Horodate {
    pub t_ms: u64,
    pub entree: EntreeTrace,
}

/// Empreinte FNV-1a 64 bits d'une trace, sur un encodage canonique.
///
/// Deux exécutions à même graine doivent donner la même empreinte ; c'est ce
/// que compare le job CI `sim`.
#[must_use]
pub fn empreinte(trace: &[Horodate]) -> u64 {
    let mut h = Fnv::default();
    for e in trace {
        h.u64(e.t_ms);
        match &e.entree {
            EntreeTrace::Connexion {
                a,
                b,
                lien_a,
                lien_b,
            } => {
                h.octet(1);
                h.noeud(*a);
                h.noeud(*b);
                h.u64(lien_a.get());
                h.u64(lien_b.get());
            }
            EntreeTrace::Coupure {
                a,
                b,
                motif_a,
                motif_b,
            } => {
                h.octet(2);
                h.noeud(*a);
                h.noeud(*b);
                h.octet(code_motif(*motif_a));
                h.octet(code_motif(*motif_b));
            }
            EntreeTrace::Emission {
                de,
                vers,
                octets,
                arrivee_ms,
            } => {
                h.octet(3);
                h.noeud(*de);
                h.noeud(*vers);
                h.octets(octets);
                h.u64(*arrivee_ms);
            }
            EntreeTrace::Perte { de, vers, octets } => {
                h.octet(4);
                h.noeud(*de);
                h.noeud(*vers);
                h.octets(octets);
            }
            EntreeTrace::PerteEnVol { de, vers, octets } => {
                h.octet(5);
                h.noeud(*de);
                h.noeud(*vers);
                h.octets(octets);
            }
            EntreeTrace::Reception { noeud, de, octets } => {
                h.octet(6);
                h.noeud(*noeud);
                h.noeud(*de);
                h.octets(octets);
            }
            EntreeTrace::Livraison { noeud, octets } => {
                h.octet(7);
                h.noeud(*noeud);
                h.octets(octets);
            }
        }
    }
    h.0
}

const fn code_motif(m: DisconnectReason) -> u8 {
    match m {
        DisconnectReason::Propre => 1,
        DisconnectReason::Brutale => 2,
        DisconnectReason::Locale => 3,
    }
}

struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn octet(&mut self, o: u8) {
        self.0 ^= u64::from(o);
        self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
    }
    fn u64(&mut self, v: u64) {
        for o in v.to_le_bytes() {
            self.octet(o);
        }
    }
    fn noeud(&mut self, n: NoeudId) {
        // usize → u64 sans perte sur toutes les cibles hôtes (≤ 64 bits).
        self.u64(n as u64);
    }
    fn octets(&mut self, o: &[u8]) {
        self.noeud(o.len());
        for &x in o {
            self.octet(x);
        }
    }
}

/// Événement en file, avec le nœud émetteur pour les trames (trace).
type EnFile = (TransportEvent, Option<NoeudId>);

#[derive(Debug, Default)]
struct EtatNoeud {
    cfg: Option<TransportConfig>,
    /// Événements datés : clé `(date d'arrivée, numéro d'ordre global)`.
    file: BTreeMap<(u64, u64), EnFile>,
}

#[derive(Debug)]
struct Connexion {
    a: NoeudId,
    b: NoeudId,
    lien_a: LinkId,
    lien_b: LinkId,
    /// Dernière date d'arrivée programmée a→b, pour garder l'ordre par lien.
    derniere_ab: u64,
    /// Idem b→a.
    derniere_ba: u64,
}

/// L'état partagé du réseau simulé. On y accède par [`ReseauPartage`].
#[derive(Debug)]
pub struct Reseau {
    maintenant_ms: u64,
    alea: Alea,
    seq: u64,
    prochain_lien: u64,
    taille_max_trame: usize,
    noeuds: Vec<EtatNoeud>,
    aretes: BTreeMap<(NoeudId, NoeudId), ParametresLien>,
    /// Groupe de partition de chaque nœud ; `None` = réseau non partitionné.
    groupes: Option<Vec<usize>>,
    connexions: BTreeMap<(NoeudId, NoeudId), Connexion>,
    /// Liens ouverts : propriétaire et clé de connexion.
    liens: BTreeMap<LinkId, (NoeudId, (NoeudId, NoeudId))>,
    trace: Vec<Horodate>,
}

const fn cle(a: NoeudId, b: NoeudId) -> (NoeudId, NoeudId) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

impl Reseau {
    fn new(graine: u64) -> Self {
        Self {
            maintenant_ms: 0,
            alea: Alea::new(graine),
            seq: 0,
            prochain_lien: 0,
            taille_max_trame: usize::MAX,
            noeuds: Vec::new(),
            aretes: BTreeMap::new(),
            groupes: None,
            connexions: BTreeMap::new(),
            liens: BTreeMap::new(),
            trace: Vec::new(),
        }
    }

    fn tracer(&mut self, entree: EntreeTrace) {
        self.trace.push(Horodate {
            t_ms: self.maintenant_ms,
            entree,
        });
    }

    fn deposer(&mut self, noeud: NoeudId, arrivee: u64, ev: TransportEvent, de: Option<NoeudId>) {
        self.seq += 1;
        let seq = self.seq;
        if let Some(n) = self.noeuds.get_mut(noeud) {
            n.file.insert((arrivee, seq), (ev, de));
        }
    }

    fn nouveau_lien(&mut self) -> LinkId {
        let l = LinkId::new(self.prochain_lien);
        self.prochain_lien += 1;
        l
    }

    fn demarre(&self, n: NoeudId) -> bool {
        self.noeuds.get(n).is_some_and(|e| e.cfg.is_some())
    }

    fn nb_connexions(&self, n: NoeudId) -> usize {
        self.liens.values().filter(|(p, _)| *p == n).count()
    }

    fn meme_groupe(&self, a: NoeudId, b: NoeudId) -> bool {
        self.groupes
            .as_ref()
            .is_none_or(|g| g.get(a).is_some() && g.get(a) == g.get(b))
    }

    /// Peut-on établir `a — b` maintenant ?
    fn connectable(&self, a: NoeudId, b: NoeudId) -> bool {
        let (Some(ca), Some(cb)) = (
            self.noeuds.get(a).and_then(|e| e.cfg.as_ref()),
            self.noeuds.get(b).and_then(|e| e.cfg.as_ref()),
        ) else {
            return false;
        };
        let se_voient = (ca.scan && cb.advertise) || (cb.scan && ca.advertise);
        se_voient
            && self.meme_groupe(a, b)
            && !self.connexions.contains_key(&cle(a, b))
            && self.nb_connexions(a) < ca.max_connections
            && self.nb_connexions(b) < cb.max_connections
    }

    /// Établit toutes les connexions possibles, dans l'ordre des arêtes.
    fn rafraichir(&mut self) {
        let aretes: Vec<_> = self.aretes.keys().copied().collect();
        for (a, b) in aretes {
            if self.connectable(a, b) {
                self.connecter(a, b);
            }
        }
    }

    fn connecter(&mut self, a: NoeudId, b: NoeudId) {
        let lien_a = self.nouveau_lien();
        let lien_b = self.nouveau_lien();
        let k = cle(a, b);
        self.liens.insert(lien_a, (a, k));
        self.liens.insert(lien_b, (b, k));
        self.connexions.insert(
            k,
            Connexion {
                a,
                b,
                lien_a,
                lien_b,
                derniere_ab: 0,
                derniere_ba: 0,
            },
        );
        let t = self.maintenant_ms;
        self.deposer(
            a,
            t,
            TransportEvent::PeerConnected {
                peer_link_id: lien_a,
                rssi: None,
            },
            None,
        );
        self.deposer(
            b,
            t,
            TransportEvent::PeerConnected {
                peer_link_id: lien_b,
                rssi: None,
            },
            None,
        );
        self.tracer(EntreeTrace::Connexion {
            a,
            b,
            lien_a,
            lien_b,
        });
    }

    /// Ferme la connexion `k`. Les trames déjà arrivées restent livrables,
    /// celles encore en vol sont jetées.
    fn deconnecter(
        &mut self,
        k: (NoeudId, NoeudId),
        motif_a: DisconnectReason,
        motif_b: DisconnectReason,
    ) {
        let Some(c) = self.connexions.remove(&k) else {
            return;
        };
        self.liens.remove(&c.lien_a);
        self.liens.remove(&c.lien_b);
        let t = self.maintenant_ms;
        for (noeud, lien, de) in [(c.a, c.lien_a, c.b), (c.b, c.lien_b, c.a)] {
            let en_vol: Vec<(u64, u64)> = self
                .noeuds
                .get(noeud)
                .map(|e| {
                    e.file
                        .iter()
                        .filter(|(&(arrivee, _), (ev, _))| {
                            arrivee > t
                                && matches!(ev, TransportEvent::FrameReceived { peer_link_id, .. } if *peer_link_id == lien)
                        })
                        .map(|(k, _)| *k)
                        .collect()
                })
                .unwrap_or_default();
            for cle_file in en_vol {
                let retire = self
                    .noeuds
                    .get_mut(noeud)
                    .and_then(|e| e.file.remove(&cle_file));
                if let Some((TransportEvent::FrameReceived { bytes, .. }, _)) = retire {
                    self.tracer(EntreeTrace::PerteEnVol {
                        de,
                        vers: noeud,
                        octets: bytes,
                    });
                }
            }
        }
        self.deposer(
            c.a,
            t,
            TransportEvent::PeerDisconnected {
                peer_link_id: c.lien_a,
                reason: motif_a,
            },
            None,
        );
        self.deposer(
            c.b,
            t,
            TransportEvent::PeerDisconnected {
                peer_link_id: c.lien_b,
                reason: motif_b,
            },
            None,
        );
        self.tracer(EntreeTrace::Coupure {
            a: c.a,
            b: c.b,
            motif_a,
            motif_b,
        });
    }

    fn start(&mut self, n: NoeudId, cfg: TransportConfig) -> Result<()> {
        let etat = self
            .noeuds
            .get_mut(n)
            .ok_or_else(|| TransportError::Backend(format!("nœud simulé {n} inconnu")))?;
        if etat.cfg.is_some() {
            return Err(TransportError::AlreadyStarted);
        }
        etat.cfg = Some(cfg);
        self.rafraichir();
        Ok(())
    }

    fn poll(&mut self, n: NoeudId) -> Vec<TransportEvent> {
        if !self.demarre(n) {
            return Vec::new();
        }
        let t = self.maintenant_ms;
        let Some(etat) = self.noeuds.get_mut(n) else {
            return Vec::new();
        };
        let a_garder = etat.file.split_off(&(t + 1, 0));
        let echus = core::mem::replace(&mut etat.file, a_garder);
        let mut sortie = Vec::with_capacity(echus.len());
        for (_, (ev, de)) in echus {
            if let (TransportEvent::FrameReceived { bytes, .. }, Some(de)) = (&ev, de) {
                self.tracer(EntreeTrace::Reception {
                    noeud: n,
                    de,
                    octets: bytes.clone(),
                });
            }
            sortie.push(ev);
        }
        sortie
    }

    fn verifier_envoi(&self, n: NoeudId, bytes: &[u8]) -> Result<()> {
        if !self.demarre(n) {
            return Err(TransportError::NotStarted);
        }
        if bytes.len() > self.taille_max_trame {
            return Err(TransportError::FrameTooLarge {
                size: bytes.len(),
                max: self.taille_max_trame,
            });
        }
        Ok(())
    }

    fn send(&mut self, n: NoeudId, lien: LinkId, bytes: &[u8]) -> Result<()> {
        self.verifier_envoi(n, bytes)?;
        let Some(&(proprietaire, k)) = self.liens.get(&lien) else {
            return Err(TransportError::UnknownPeer(lien));
        };
        if proprietaire != n {
            return Err(TransportError::UnknownPeer(lien));
        }
        let params = self.aretes.get(&k).copied().unwrap_or_default();
        let Some(c) = self.connexions.get(&k) else {
            return Err(TransportError::UnknownPeer(lien));
        };
        let (vers, lien_dest, dernier) = if c.a == n {
            (c.b, c.lien_b, c.derniere_ab)
        } else {
            (c.a, c.lien_a, c.derniere_ba)
        };

        if params.perte_pour_mille > 0 && self.alea.sous(1000) < u64::from(params.perte_pour_mille)
        {
            self.tracer(EntreeTrace::Perte {
                de: n,
                vers,
                octets: bytes.to_vec(),
            });
            return Ok(());
        }
        let gigue = if params.gigue_ms > 0 {
            self.alea.sous(params.gigue_ms + 1)
        } else {
            0
        };
        let arrivee = (self.maintenant_ms + params.latence_ms + gigue).max(dernier);
        if let Some(c) = self.connexions.get_mut(&k) {
            if c.a == n {
                c.derniere_ab = arrivee;
            } else {
                c.derniere_ba = arrivee;
            }
        }
        self.deposer(
            vers,
            arrivee,
            TransportEvent::FrameReceived {
                peer_link_id: lien_dest,
                bytes: bytes.to_vec(),
            },
            Some(n),
        );
        self.tracer(EntreeTrace::Emission {
            de: n,
            vers,
            octets: bytes.to_vec(),
            arrivee_ms: arrivee,
        });
        Ok(())
    }

    fn broadcast(&mut self, n: NoeudId, bytes: &[u8]) -> Result<()> {
        self.verifier_envoi(n, bytes)?;
        let miens: Vec<LinkId> = self
            .liens
            .iter()
            .filter(|(_, (p, _))| *p == n)
            .map(|(l, _)| *l)
            .collect();
        for l in miens {
            // « Au mieux » : un échec vers un pair n'empêche pas les autres.
            let _ = self.send(n, l, bytes);
        }
        Ok(())
    }
}

/// Poignée partagée sur le [`Reseau`] : c'est par elle que le harness et les
/// tests pilotent le monde extérieur (topologie, partitions, horloge).
#[derive(Debug, Clone)]
pub struct ReseauPartage(Arc<Mutex<Reseau>>);

impl ReseauPartage {
    /// Un réseau vide, horloge à 0, aléa initialisé par `graine`.
    #[must_use]
    pub fn new(graine: u64) -> Self {
        Self(Arc::new(Mutex::new(Reseau::new(graine))))
    }

    fn verrou(&self) -> MutexGuard<'_, Reseau> {
        // Un panic pendant un test ne doit pas en masquer la cause par un
        // second panic « mutex empoisonné » : on récupère l'état tel quel.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Ajoute un nœud (transport non démarré) et rend son transport.
    #[must_use]
    pub fn ajouter_noeud(&self) -> SimTransport {
        let mut r = self.verrou();
        r.noeuds.push(EtatNoeud::default());
        SimTransport {
            reseau: self.clone(),
            noeud: r.noeuds.len() - 1,
        }
    }

    /// Nombre de nœuds.
    #[must_use]
    pub fn nb_noeuds(&self) -> usize {
        self.verrou().noeuds.len()
    }

    /// Met `a` et `b` à portée l'un de l'autre (ou change les paramètres de
    /// l'arête existante), puis établit les connexions possibles.
    pub fn relier(&self, a: NoeudId, b: NoeudId, params: ParametresLien) {
        if a == b {
            return;
        }
        let mut r = self.verrou();
        r.aretes.insert(cle(a, b), params);
        r.rafraichir();
    }

    /// Retire l'arête `a — b` : la connexion éventuelle tombe brutalement.
    pub fn delier(&self, a: NoeudId, b: NoeudId) {
        let mut r = self.verrou();
        let k = cle(a, b);
        r.aretes.remove(&k);
        r.deconnecter(k, DisconnectReason::Brutale, DisconnectReason::Brutale);
    }

    /// Partitionne le réseau : seules les connexions **intra**-groupe
    /// survivent. Un nœud absent de tous les groupes est isolé.
    pub fn partitionner(&self, groupes: &[Vec<NoeudId>]) {
        let mut r = self.verrou();
        let n = r.noeuds.len();
        // Groupe « personne » distinct pour chaque nœud non listé.
        let mut g: Vec<usize> = (0..n).map(|i| groupes.len() + i).collect();
        for (i, groupe) in groupes.iter().enumerate() {
            for &noeud in groupe {
                if let Some(slot) = g.get_mut(noeud) {
                    *slot = i;
                }
            }
        }
        r.groupes = Some(g);
        let a_couper: Vec<_> = r
            .connexions
            .keys()
            .copied()
            .filter(|&(a, b)| !r.meme_groupe(a, b))
            .collect();
        for k in a_couper {
            r.deconnecter(k, DisconnectReason::Brutale, DisconnectReason::Brutale);
        }
    }

    /// Lève la partition et rétablit les connexions possibles.
    pub fn reunir(&self) {
        let mut r = self.verrou();
        r.groupes = None;
        r.rafraichir();
    }

    /// Ferme le lien `lien` ; son propriétaire voit `motif`, l'autre côté le
    /// motif symétrique (`Propre` ↔ `Locale`, `Brutale` des deux côtés).
    /// L'arête est retirée : les deux nœuds ne se reconnectent pas d'eux-mêmes.
    pub fn couper_lien(&self, lien: LinkId, motif: DisconnectReason) {
        let mut r = self.verrou();
        let Some(&(proprietaire, k)) = r.liens.get(&lien) else {
            return;
        };
        let oppose = match motif {
            DisconnectReason::Propre => DisconnectReason::Locale,
            DisconnectReason::Locale => DisconnectReason::Propre,
            DisconnectReason::Brutale => DisconnectReason::Brutale,
        };
        let (motif_a, motif_b) = if k.0 == proprietaire {
            (motif, oppose)
        } else {
            (oppose, motif)
        };
        r.aretes.remove(&k);
        r.deconnecter(k, motif_a, motif_b);
    }

    /// Le lien que `a` utilise pour parler à `b`, s'ils sont connectés.
    #[must_use]
    pub fn lien_entre(&self, a: NoeudId, b: NoeudId) -> Option<LinkId> {
        let r = self.verrou();
        r.connexions
            .get(&cle(a, b))
            .map(|c| if c.a == a { c.lien_a } else { c.lien_b })
    }

    /// L'autre extrémité d'un lien ouvert : son nœud et le lien qu'il voit.
    #[must_use]
    pub fn lien_oppose(&self, lien: LinkId) -> Option<(NoeudId, LinkId)> {
        let r = self.verrou();
        let &(proprietaire, k) = r.liens.get(&lien)?;
        let c = r.connexions.get(&k)?;
        Some(if c.a == proprietaire {
            (c.b, c.lien_b)
        } else {
            (c.a, c.lien_a)
        })
    }

    /// Plafonne la taille des trames (défaut : illimitée).
    pub fn regler_taille_max_trame(&self, max: usize) {
        self.verrou().taille_max_trame = max;
    }

    /// Avance l'horloge virtuelle.
    pub fn avancer(&self, ms: u64) {
        let mut r = self.verrou();
        r.maintenant_ms = r.maintenant_ms.saturating_add(ms);
    }

    /// Date virtuelle courante, en ms.
    #[must_use]
    pub fn maintenant_ms(&self) -> u64 {
        self.verrou().maintenant_ms
    }

    /// Ajoute une livraison applicative à la trace (appelé par le harness).
    pub fn tracer_livraison(&self, noeud: NoeudId, octets: &[u8]) {
        self.verrou().tracer(EntreeTrace::Livraison {
            noeud,
            octets: octets.to_vec(),
        });
    }

    /// Copie de la trace complète.
    #[must_use]
    pub fn trace(&self) -> Vec<Horodate> {
        self.verrou().trace.clone()
    }
}

/// Transport d'un nœud simulé. Implémente le contrat gelé [`Transport`]
/// (US-105) et passe sa suite de conformité (`tests/conformite_sim.rs`).
#[derive(Debug, Clone)]
pub struct SimTransport {
    reseau: ReseauPartage,
    noeud: NoeudId,
}

impl SimTransport {
    /// L'indice du nœud qui possède ce transport.
    #[must_use]
    pub const fn noeud(&self) -> NoeudId {
        self.noeud
    }
}

impl Transport for SimTransport {
    fn start(&mut self, cfg: TransportConfig) -> Result<()> {
        self.reseau.verrou().start(self.noeud, cfg)
    }

    fn poll(&mut self) -> Vec<TransportEvent> {
        self.reseau.verrou().poll(self.noeud)
    }

    fn send(&mut self, peer_link_id: LinkId, bytes: &[u8]) -> Result<()> {
        self.reseau.verrou().send(self.noeud, peer_link_id, bytes)
    }

    fn broadcast(&mut self, bytes: &[u8]) -> Result<()> {
        self.reseau.verrou().broadcast(self.noeud, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deux_noeuds(params: ParametresLien) -> (ReseauPartage, SimTransport, SimTransport) {
        let r = ReseauPartage::new(1);
        let mut a = r.ajouter_noeud();
        let mut b = r.ajouter_noeud();
        a.start(TransportConfig::default()).unwrap();
        b.start(TransportConfig::default()).unwrap();
        r.relier(0, 1, params);
        (r, a, b)
    }

    #[test]
    fn la_latence_retarde_la_livraison() {
        let (r, mut a, mut b) = deux_noeuds(ParametresLien {
            latence_ms: 50,
            ..ParametresLien::default()
        });
        let _ = (a.poll(), b.poll());
        let lien = r.lien_entre(0, 1).unwrap();
        a.send(lien, b"x").unwrap();
        r.avancer(49);
        assert!(b.poll().is_empty(), "pas encore arrivée à 49 ms");
        r.avancer(1);
        assert_eq!(b.poll().len(), 1, "arrivée à 50 ms");
    }

    #[test]
    fn l_ordre_par_lien_survit_a_la_gigue() {
        let (r, mut a, mut b) = deux_noeuds(ParametresLien {
            latence_ms: 10,
            gigue_ms: 100,
            perte_pour_mille: 0,
        });
        let _ = (a.poll(), b.poll());
        let lien = r.lien_entre(0, 1).unwrap();
        for i in 0u8..50 {
            a.send(lien, &[i]).unwrap();
        }
        r.avancer(1000);
        let recu: Vec<u8> = b
            .poll()
            .into_iter()
            .filter_map(|e| match e {
                TransportEvent::FrameReceived { bytes, .. } => bytes.first().copied(),
                _ => None,
            })
            .collect();
        assert_eq!(recu, (0u8..50).collect::<Vec<_>>());
    }

    #[test]
    fn perte_totale_et_perte_nulle() {
        let (r, mut a, mut b) = deux_noeuds(ParametresLien {
            perte_pour_mille: 1000,
            ..ParametresLien::default()
        });
        let _ = (a.poll(), b.poll());
        let lien = r.lien_entre(0, 1).unwrap();
        a.send(lien, b"perdu").unwrap();
        assert!(b.poll().is_empty());
        assert!(r
            .trace()
            .iter()
            .any(|h| matches!(h.entree, EntreeTrace::Perte { .. })));
    }

    #[test]
    fn la_partition_coupe_et_la_reunion_reconnecte_avec_de_nouveaux_liens() {
        let (r, mut a, mut b) = deux_noeuds(ParametresLien::default());
        let _ = (a.poll(), b.poll());
        let avant = r.lien_entre(0, 1).unwrap();

        r.partitionner(&[vec![0], vec![1]]);
        assert!(r.lien_entre(0, 1).is_none());
        assert!(a.poll().iter().any(|e| matches!(
            e,
            TransportEvent::PeerDisconnected {
                reason: DisconnectReason::Brutale,
                ..
            }
        )));
        assert_eq!(a.send(avant, b"x"), Err(TransportError::UnknownPeer(avant)));

        r.reunir();
        let apres = r.lien_entre(0, 1).unwrap();
        assert_ne!(avant, apres, "un LinkId n'est jamais réutilisé");
        assert!(matches!(
            a.poll().first(),
            Some(TransportEvent::PeerConnected { .. })
        ));
    }

    #[test]
    fn une_trame_en_vol_est_perdue_a_la_coupure() {
        let (r, mut a, mut b) = deux_noeuds(ParametresLien {
            latence_ms: 100,
            ..ParametresLien::default()
        });
        let _ = (a.poll(), b.poll());
        a.send(r.lien_entre(0, 1).unwrap(), b"en-vol").unwrap();
        r.avancer(10);
        r.delier(0, 1);
        r.avancer(200);
        assert!(!b
            .poll()
            .iter()
            .any(|e| matches!(e, TransportEvent::FrameReceived { .. })));
        assert!(r
            .trace()
            .iter()
            .any(|h| matches!(h.entree, EntreeTrace::PerteEnVol { .. })));
    }

    #[test]
    fn le_quota_et_les_roles_limitent_les_connexions() {
        let r = ReseauPartage::new(1);
        let mut centre = r.ajouter_noeud();
        centre
            .start(TransportConfig {
                max_connections: 1,
                ..TransportConfig::default()
            })
            .unwrap();
        for i in 1..=2 {
            let mut t = r.ajouter_noeud();
            t.start(TransportConfig::default()).unwrap();
            r.relier(0, i, ParametresLien::default());
        }
        assert!(r.lien_entre(0, 1).is_some());
        assert!(r.lien_entre(0, 2).is_none(), "quota de 1 atteint");

        // Deux nœuds qui annoncent sans scanner ne se voient pas.
        let r = ReseauPartage::new(1);
        let muets = TransportConfig {
            scan: false,
            ..TransportConfig::default()
        };
        r.ajouter_noeud().start(muets.clone()).unwrap();
        r.ajouter_noeud().start(muets).unwrap();
        r.relier(0, 1, ParametresLien::default());
        assert!(r.lien_entre(0, 1).is_none());
    }

    #[test]
    fn broadcast_taille_max_et_liens_etrangers() {
        let (r, mut a, mut b) = deux_noeuds(ParametresLien::default());
        let _ = (a.poll(), b.poll());
        r.regler_taille_max_trame(4);
        assert_eq!(
            a.broadcast(b"trop-long"),
            Err(TransportError::FrameTooLarge { size: 9, max: 4 })
        );
        a.broadcast(b"ok").unwrap();
        assert_eq!(b.poll().len(), 1);

        // Le lien de b n'appartient pas à a.
        let lien_b = r.lien_entre(1, 0).unwrap();
        assert_eq!(
            a.send(lien_b, b"x"),
            Err(TransportError::UnknownPeer(lien_b))
        );
        assert_eq!(r.lien_oppose(lien_b).map(|(n, _)| n), Some(0));
        assert_eq!(a.noeud(), 0);
        assert_eq!(r.nb_noeuds(), 2);
    }

    #[test]
    fn couper_lien_donne_des_motifs_symetriques() {
        let (r, mut a, mut b) = deux_noeuds(ParametresLien::default());
        let _ = (a.poll(), b.poll());
        r.couper_lien(r.lien_entre(1, 0).unwrap(), DisconnectReason::Locale);
        let motif = |evs: Vec<TransportEvent>| {
            evs.into_iter().find_map(|e| match e {
                TransportEvent::PeerDisconnected { reason, .. } => Some(reason),
                _ => None,
            })
        };
        assert_eq!(motif(b.poll()), Some(DisconnectReason::Locale));
        assert_eq!(motif(a.poll()), Some(DisconnectReason::Propre));
        // Lien déjà fermé : sans effet, sans panique.
        r.couper_lien(LinkId::new(999), DisconnectReason::Brutale);
    }

    #[test]
    fn un_noeud_inconnu_ou_non_demarre_est_gere() {
        let r = ReseauPartage::new(1);
        let mut t = r.ajouter_noeud();
        r.relier(0, 0, ParametresLien::default()); // boucle : ignorée
        assert!(t.poll().is_empty());
        let fantome = SimTransport {
            reseau: r.clone(),
            noeud: 9,
        };
        let mut fantome = fantome;
        assert!(matches!(
            fantome.start(TransportConfig::default()),
            Err(TransportError::Backend(_))
        ));
    }

    #[test]
    fn l_empreinte_depend_de_chaque_champ() {
        let base = vec![Horodate {
            t_ms: 1,
            entree: EntreeTrace::Livraison {
                noeud: 0,
                octets: vec![1],
            },
        }];
        let mut autre = base.clone();
        autre[0].t_ms = 2;
        assert_ne!(empreinte(&base), empreinte(&autre));
        assert_eq!(empreinte(&base), empreinte(&base.clone()));
        assert_ne!(empreinte(&[]), empreinte(&base));
    }
}
