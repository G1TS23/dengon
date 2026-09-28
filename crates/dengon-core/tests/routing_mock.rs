//! `sync::routing` de bout en bout contre `MockTransport` (US-209).
//!
//! Chaque nœud = un `MockTransport` + un `Router<LinkId>`. Le [`Reseau`] de
//! test recopie les trames envoyées par un nœud dans le transport de son
//! voisin : c'est le « fil » entre deux mocks, que `dengon-sim` (US-221)
//! remplacera par un vrai modèle réseau (latence, perte, partition).
//!
//! **Décodage provisoire.** `protocol::codec` (US-201) n'existe pas encore :
//! [`encoder`] / [`decoder`] lisent et écrivent l'en-tête L3 **réel**
//! (`synthese/05` §3, big-endian) à la main, sans signature. Le TTL est à
//! l'octet 2 : le relais le réécrit en place, sans réencoder. À remplacer par
//! `codec::{encode, decode}` quand US-201 sera sur `main`.

use std::collections::BTreeMap;

use dengon_ble::{LinkId, MockTransport, Transport, TransportConfig, TransportEvent};
use dengon_core::protocol::{
    Flags, Header, MsgId, PacketType, PeerId, DENSE_LINKS, FLOOD_MAX_PER_MIN_PEER, PROTO_VERSION,
    SEEN_SET_CAP, TTL_CLAMP_DENSE, TTL_DEFAULT,
};
use dengon_core::sync::routing::{Decision, Router, RoutingConfig, DUP_CANCEL_THRESHOLD};
use sha2::{Digest, Sha256};

const T0: u64 = 1_800_000_000_000;
/// Pas d'horloge de la simulation, en ms.
const PAS_MS: u64 = 5;
/// Octet du TTL dans l'en-tête L3 (`version, type, ttl, flags, …`).
const OCTET_TTL: usize = 2;

// --- Codec de test (provisoire, voir la doc du module) ------------------

fn encoder(h: &Header, payload: &[u8]) -> Vec<u8> {
    let mut b = vec![h.version, h.packet_type.to_u8(), h.ttl, h.flags.bits()];
    b.extend_from_slice(&h.timestamp_ms.to_be_bytes());
    b.extend_from_slice(&h.sender_id);
    if let Some(r) = h.recipient_id {
        b.extend_from_slice(&r);
    }
    b.extend_from_slice(
        &u16::try_from(payload.len())
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    b.extend_from_slice(payload);
    b
}

fn decoder(b: &[u8]) -> Option<(Header, MsgId)> {
    let flags = Flags::from_bits_truncate(*b.get(3)?);
    let mut pos = 12;
    let lire8 = |pos: usize| -> Option<[u8; 8]> { b.get(pos..pos + 8)?.try_into().ok() };
    let timestamp_ms = u64::from_be_bytes(lire8(4)?);
    let sender_id = lire8(pos)?;
    pos += 8;
    let recipient_id = if flags.contains(Flags::ADDRESSED) {
        let r = lire8(pos)?;
        pos += 8;
        Some(r)
    } else {
        None
    };
    let payload_len = u16::from_be_bytes(b.get(pos..pos + 2)?.try_into().ok()?);
    pos += 2;
    let payload = b.get(pos..pos + usize::from(payload_len))?;
    let h = Header {
        version: b[0],
        packet_type: PacketType::from_u8(b[1])?,
        ttl: b[2],
        flags,
        timestamp_ms,
        sender_id,
        recipient_id,
        payload_len,
    };
    // msgID = SHA-256(sender_id ‖ timestamp_ms ‖ type ‖ payload) (A-9) :
    // le TTL n'y entre pas, donc un relais garde le même msgID.
    let mut hasher = Sha256::new();
    hasher.update(sender_id);
    hasher.update(timestamp_ms.to_be_bytes());
    hasher.update([h.packet_type.to_u8()]);
    hasher.update(payload);
    Some((h, hasher.finalize().into()))
}

fn paquet(sender: PeerId, dest: Option<PeerId>, ttl: u8, ts: u64, payload: &[u8]) -> Vec<u8> {
    let mut flags = Flags::RELAY_OK;
    if dest.is_some() {
        flags = flags | Flags::ADDRESSED;
    }
    let h = Header {
        version: PROTO_VERSION,
        packet_type: PacketType::NoiseMsg,
        ttl,
        flags,
        timestamp_ms: ts,
        sender_id: sender,
        recipient_id: dest,
        payload_len: 0,
    };
    encoder(&h, payload)
}

fn peer(n: u8) -> PeerId {
    [n; 8]
}

// --- Un nœud : transport mock + routeur ------------------------------------

struct Noeud {
    t: MockTransport,
    r: Router<LinkId>,
    /// Octets des paquets en attente de relais, par msgID.
    a_relayer: BTreeMap<MsgId, Vec<u8>>,
    /// Paquets livrés localement, avec le TTL reçu.
    livres: Vec<(MsgId, u8)>,
}

impl Noeud {
    fn new(id: PeerId, seed: u64) -> Self {
        Self::avec(RoutingConfig::new(id), seed)
    }

    fn avec(rcfg: RoutingConfig, seed: u64) -> Self {
        let id = rcfg.local_id;
        let mut t = MockTransport::new();
        let cfg = TransportConfig {
            local_peer_id: id,
            max_connections: 16,
            ..TransportConfig::default()
        };
        assert!(t.start(cfg).is_ok());
        Self {
            t,
            r: Router::new(rcfg, seed),
            a_relayer: BTreeMap::new(),
            livres: Vec::new(),
        }
    }

    /// Une itération de la boucle d'événements du nœud.
    fn tick(&mut self, now: u64) {
        for ev in self.t.poll() {
            match ev {
                TransportEvent::PeerConnected { peer_link_id, .. } => self.r.link_up(peer_link_id),
                TransportEvent::PeerDisconnected { peer_link_id, .. } => {
                    self.r.link_down(peer_link_id);
                }
                TransportEvent::FrameReceived {
                    peer_link_id,
                    bytes,
                } => {
                    let Some((h, id)) = decoder(&bytes) else {
                        continue;
                    };
                    match self.r.on_packet(peer_link_id, &h, &id, now) {
                        Decision::RelayScheduled { .. } => {
                            self.a_relayer.insert(id, bytes);
                        }
                        Decision::Deliver => self.livres.push((id, h.ttl)),
                        _ => {}
                    }
                }
            }
        }
        for ordre in self.r.poll_due(now) {
            let Some(mut bytes) = self.a_relayer.remove(&ordre.msg_id) else {
                continue;
            };
            bytes[OCTET_TTL] = ordre.ttl;
            for cible in ordre.targets {
                assert!(self.t.send(cible, &bytes).is_ok());
            }
        }
        // Relais annulés (doublons pendant le jitter) : leurs octets ne
        // seront jamais réclamés.
        if self.r.pending_len() == 0 {
            self.a_relayer.clear();
        }
    }

    fn nb_trames_envoyees(&self, liens: &[LinkId]) -> usize {
        liens
            .iter()
            .map(|l| self.t.trames_envoyees_a(*l).len())
            .sum()
    }
}

// --- Réseau : recopie des trames entre mocks ----------------------------------

struct Reseau {
    noeuds: Vec<Noeud>,
    /// (nœud, lien local) → (nœud voisin, lien chez le voisin).
    fils: BTreeMap<(usize, LinkId), (usize, LinkId)>,
    /// Trames déjà recopiées, par (nœud, lien).
    curseurs: BTreeMap<(usize, LinkId), usize>,
    now: u64,
}

impl Reseau {
    fn new(n: u8) -> Self {
        Self::avec(n, |cfg| cfg)
    }

    fn avec(n: u8, regler: impl Fn(RoutingConfig) -> RoutingConfig) -> Self {
        Self {
            noeuds: (0..n)
                .map(|i| Noeud::avec(regler(RoutingConfig::new(peer(i + 1))), u64::from(i)))
                .collect(),
            fils: BTreeMap::new(),
            curseurs: BTreeMap::new(),
            now: T0,
        }
    }

    fn relier(&mut self, a: usize, b: usize) {
        let la = self.noeuds[a].t.connecter_pair(None);
        let lb = self.noeuds[b].t.connecter_pair(None);
        self.fils.insert((a, la), (b, lb));
        self.fils.insert((b, lb), (a, la));
    }

    /// Le nœud `a` émet un paquet qu'il a lui-même créé, vers tous ses voisins.
    fn emettre(&mut self, a: usize, bytes: &[u8]) {
        assert!(self.noeuds[a].t.broadcast(bytes).is_ok());
    }

    /// Fait tourner le réseau jusqu'à ce plus rien ne bouge.
    fn stabiliser(&mut self) {
        for _ in 0..10_000 {
            let mut actif = false;
            for n in &mut self.noeuds {
                n.tick(self.now);
                actif |= n.r.pending_len() > 0;
            }
            let fils: Vec<_> = self.fils.iter().map(|(k, v)| (*k, *v)).collect();
            for ((a, la), (b, lb)) in fils {
                let deja = self.curseurs.get(&(a, la)).copied().unwrap_or(0);
                let trames: Vec<Vec<u8>> = self.noeuds[a].t.trames_envoyees_a(la)[deja..].to_vec();
                actif |= !trames.is_empty();
                self.curseurs.insert((a, la), deja + trames.len());
                for tr in trames {
                    self.noeuds[b].t.injecter_trame(lb, &tr);
                }
            }
            self.now += PAS_MS;
            if !actif {
                return;
            }
        }
        panic!("le réseau ne s'est pas stabilisé");
    }
}

// --- Scénarios -----------------------------------------------------------

#[test]
fn relais_multi_sauts_sur_une_chaine() {
    // A — B — C — D ; A écrit à D.
    let mut net = Reseau::new(4);
    for i in 0..3 {
        net.relier(i, i + 1);
    }
    net.stabiliser();
    let p = paquet(peer(1), Some(peer(4)), TTL_DEFAULT, net.now, b"salut");
    net.emettre(0, &p);
    net.stabiliser();

    let d = &net.noeuds[3];
    assert_eq!(d.livres.len(), 1, "D doit recevoir le message une fois");
    // Émis à 7, relayé par B (6) puis C (5).
    assert_eq!(d.livres[0].1, TTL_DEFAULT - 2);
    for relais in [1, 2] {
        assert_eq!(net.noeuds[relais].r.stats().relays_emitted, 1);
    }
    // D ne relaie pas ce qui lui est adressé.
    assert_eq!(d.r.stats().relays_scheduled, 0);
}

/// Losange A→{B,C}→D, puis D→E ; A écrit à E. Rend le réseau stabilisé.
///
/// ```text
///     B
///   /   \
/// A       D — E
///   \   /
///     C
/// ```
fn losange(seuil_annulation: u8) -> Reseau {
    let mut net = Reseau::avec(5, |mut cfg| {
        cfg.dup_cancel_threshold = seuil_annulation;
        cfg
    });
    for (a, b) in [(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)] {
        net.relier(a, b);
    }
    net.stabiliser();
    let p = paquet(peer(1), Some(peer(5)), TTL_DEFAULT, net.now, b"losange");
    net.emettre(0, &p);
    net.stabiliser();
    net
}

#[test]
fn plusieurs_chemins_un_seul_relais_par_noeud() {
    let net = losange(DUP_CANCEL_THRESHOLD);

    assert_eq!(
        net.noeuds[4].livres.len(),
        1,
        "un seul exemplaire livré à E"
    );
    for n in &net.noeuds {
        assert!(n.r.stats().relays_emitted <= 1);
    }
    // D a reçu le message par B et par C : le second est un doublon.
    assert!(net.noeuds[3].r.stats().duplicates >= 1);
}

/// Régression documentée : avec le seuil littéral de `synthese/05` §6.1
/// (« un doublon pendant l'attente → abandonner »), E n'est **jamais**
/// servi — c'est ce qui a motivé [`DUP_CANCEL_THRESHOLD`] = 2.
#[test]
fn seuil_litteral_affame_le_losange() {
    let net = losange(1);
    assert!(net.noeuds[4].livres.is_empty());
    assert_eq!(net.noeuds[3].r.stats().relays_cancelled, 1);
}

#[test]
fn le_ttl_borne_la_portee() {
    // Chaîne de 8 nœuds, message diffusé avec TTL 3 : 3 sauts au plus.
    let mut net = Reseau::new(8);
    for i in 0..7 {
        net.relier(i, i + 1);
    }
    net.stabiliser();
    let p = paquet(peer(1), None, 3, net.now, b"proche");
    net.emettre(0, &p);
    net.stabiliser();

    let vus: Vec<u64> = net.noeuds.iter().map(|n| n.r.stats().received).collect();
    // n1 (ttl 3), n2 (ttl 2), n3 (ttl 1, pas relayé) ; au-delà, rien.
    assert!(vus[1] >= 1 && vus[2] >= 1 && vus[3] >= 1);
    assert!(vus[4..].iter().all(|v| *v == 0), "vus = {vus:?}");
    assert_eq!(net.noeuds[3].r.stats().not_relayed, 1);
}

#[test]
fn clamp_de_densite_et_exclusion_de_la_source() {
    // Un nœud avec DENSE_LINKS voisins simulés directement sur son mock.
    let mut n = Noeud::new(peer(1), 3);
    let liens: Vec<LinkId> = (0..DENSE_LINKS).map(|_| n.t.connecter_pair(None)).collect();
    n.tick(T0);
    let source = liens[0];
    n.t.injecter_trame(source, &paquet(peer(9), None, 7, T0, b"dense"));

    let mut now = T0;
    while n.nb_trames_envoyees(&liens) == 0 && now < T0 + 1_000 {
        n.tick(now);
        now += PAS_MS;
    }
    assert!(
        n.t.trames_envoyees_a(source).is_empty(),
        "jamais vers la source"
    );
    for l in &liens[1..] {
        let trames = n.t.trames_envoyees_a(*l);
        assert_eq!(trames.len(), 1);
        assert_eq!(trames[0][OCTET_TTL], TTL_CLAMP_DENSE, "ttl' = min(7-1, 5)");
    }
}

/// Injecte `par_seconde` msgID distincts par seconde sur chacun des liens
/// `attaquants`, pendant `secondes`. Rend le nombre de relais émis.
fn inonder(n: &mut Noeud, attaquants: &[LinkId], par_seconde: u64, secondes: u64) -> u64 {
    let pas = 1_000 / par_seconde;
    let mut compteur: u64 = 0;
    let fin = T0 + secondes * 1_000;
    let mut now = T0;
    while now < fin {
        for l in attaquants {
            compteur += 1;
            let payload = compteur.to_be_bytes();
            n.t.injecter_trame(*l, &paquet(peer(0xEE), None, 7, now, &payload));
        }
        n.tick(now);
        now += pas;
    }
    // Laisser partir les derniers relais programmés.
    n.tick(now + 1_000);
    n.r.stats().relays_emitted
}

#[test]
fn inondation_un_voisin_500_msg_par_seconde_pendant_60_s() {
    let mut n = Noeud::new(peer(1), 11);
    let liens: Vec<LinkId> = (0..3).map(|_| n.t.connecter_pair(None)).collect();
    n.tick(T0);

    let relais = inonder(&mut n, &liens[..1], 500, 60);
    let trames = n.nb_trames_envoyees(&liens);
    let s = n.r.stats();
    println!(
        "inondation 1 voisin : {} reçus, {} relais, {} trames émises, \
         {} anti-inondation, {} quota lien, seen-set = {}",
        s.received,
        relais,
        trames,
        s.flood_limited,
        s.link_quota,
        n.r.seen_len()
    );

    assert_eq!(s.received, 30_000);
    // Borne explicite du critère d'acceptation : sur 60 s, au plus
    // FLOOD_MAX_PER_MIN_PEER = 20 relais venant de ce voisin, chacun vers les
    // 2 voisins hors source → au plus 40 trames émises.
    assert!(
        relais <= u64::from(FLOOD_MAX_PER_MIN_PEER),
        "relais = {relais}"
    );
    assert!(trames <= 2 * usize::from(FLOOD_MAX_PER_MIN_PEER));
    assert!(n.t.trames_envoyees_a(liens[0]).is_empty());
    assert!(n.r.seen_len() <= SEEN_SET_CAP);

    // Le trafic honnête d'un autre voisin passe toujours.
    let now = T0 + 61_000;
    n.t.injecter_trame(liens[1], &paquet(peer(2), None, 7, now, b"honnete"));
    n.tick(now);
    n.tick(now + 1_000);
    assert_eq!(n.r.stats().relays_emitted, relais + 1);
}

#[test]
fn inondation_trois_voisins_bornee_a_soixante_relais() {
    let mut n = Noeud::new(peer(1), 12);
    let liens: Vec<LinkId> = (0..3).map(|_| n.t.connecter_pair(None)).collect();
    n.tick(T0);

    let relais = inonder(&mut n, &liens, 500, 60);
    let trames = n.nb_trames_envoyees(&liens);
    println!("inondation 3 voisins : {relais} relais, {trames} trames émises");

    assert!(
        relais <= 3 * u64::from(FLOOD_MAX_PER_MIN_PEER),
        "relais = {relais}"
    );
    assert!(trames <= 3 * 2 * usize::from(FLOOD_MAX_PER_MIN_PEER));
    assert!(n.r.seen_len() <= SEEN_SET_CAP);
}

#[test]
fn meme_graine_meme_trace() {
    fn trace(seed: u64) -> Vec<Vec<u8>> {
        let mut n = Noeud::new(peer(1), seed);
        let liens: Vec<LinkId> = (0..4).map(|_| n.t.connecter_pair(None)).collect();
        n.tick(T0);
        for k in 0..10u8 {
            n.t.injecter_trame(
                liens[usize::from(k % 4)],
                &paquet(peer(9), None, 7, T0, &[k]),
            );
        }
        // Trace = (instant, lien, trame) de chaque émission, dans l'ordre.
        let mut trace = Vec::new();
        let mut deja = vec![0; liens.len()];
        for now in T0..T0 + 500 {
            n.tick(now);
            for (i, l) in liens.iter().enumerate() {
                for tr in &n.t.trames_envoyees_a(*l)[deja[i]..] {
                    trace.push([&now.to_be_bytes()[..], &l.get().to_be_bytes(), tr].concat());
                }
                deja[i] = n.t.trames_envoyees_a(*l).len();
            }
        }
        trace
    }
    assert_eq!(trace(99), trace(99));
    assert_ne!(trace(99), trace(100), "la graine doit influencer le jitter");
}
