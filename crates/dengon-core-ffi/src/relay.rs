//! Relais dengon exposé au C (US-308) : un handle opaque [`Relay`]
//! autour de [`dengon_core::relay::Relay`] (préfixé `Dengon` côté C par cbindgen), piloté par les tâches FreeRTOS du
//! firmware.
//!
//! # Modèle d'appel
//!
//! - Entrées : [`dengon_relay_link_up`] / [`dengon_relay_link_down`],
//!   [`dengon_relay_on_frame`], [`dengon_relay_poll`] (échéances).
//! - Sorties, **une à la fois** pour rester simple côté C (tampon fixe, pas
//!   d'allocation à libérer) : [`dengon_relay_pop_outgoing`] (trames à
//!   émettre) et [`dengon_relay_pop_ledger`] (entrées de journal à écrire
//!   dans `ledger.bin`, format `Entry::to_bytes`, celui que lit
//!   `dengon-verify`). Un tampon trop petit rend
//!   [`Status::BufferTooSmall`] avec la taille requise et **ne perd rien**.
//! - Persistance : après avoir écrit les entrées, le firmware enregistre le
//!   curseur ([`dengon_relay_ledger_anchor`]) ; au démarrage suivant il le
//!   repasse à [`dengon_relay_new`].
//!
//! Le handle n'est **pas** thread-safe : le firmware le protège par un
//! mutex. Aucune fonction ne panique sur une entrée hostile (trames
//! arbitraires, chaînes non UTF-8) : `on_frame` rejette, `record_event`
//! refuse.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::ffi::{c_char, CStr};

use dengon_core::ledger::Anchor;
use dengon_core::relay::{Now as CoreNow, Relay as CoreRelay, RelayConfig, RelaySecrets};

use crate::Status;

/// Horloges passées à chaque appel. `wall_ms` : ms UTC, ou toute valeur
/// antérieure à 2024 si l'heure est inconnue (le relais l'apprend alors d'un
/// `ANNOUNCE`) ; `mono_ms` : uptime, ne recule jamais.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Now {
    /// Horloge murale, ms UTC.
    pub wall_ms: u64,
    /// Horloge monotone, ms.
    pub mono_ms: u64,
}

impl From<Now> for CoreNow {
    fn from(n: Now) -> Self {
        CoreNow::new(n.wall_ms, n.mono_ms)
    }
}

/// Compteurs du relais (pour `relay.health` et les journaux série).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RelayStats {
    /// Trames illisibles.
    pub malformed: u64,
    /// Paquets signés refusés (usurpation, signature).
    pub unauthentic: u64,
    /// Paquets relayés.
    pub relayed: u64,
    /// Enveloppes déposées.
    pub envelopes_stored: u64,
    /// Enveloppes remises.
    pub envelopes_handed_off: u64,
    /// Paquets ignorés faute d'heure murale.
    pub clock_unknown: u64,
    /// Enveloppes détenues maintenant.
    pub envelopes_held: u32,
    /// Paquets au cache de réconciliation maintenant.
    pub cache_len: u32,
}

/// Handle opaque du relais.
#[derive(Debug)]
pub struct Relay {
    relay: CoreRelay<u64>,
    outgoing: VecDeque<(u64, Vec<u8>)>,
    ledger: VecDeque<Vec<u8>>,
}

impl Relay {
    fn drain(&mut self) {
        self.outgoing.extend(self.relay.take_outgoing());
        self.ledger.extend(
            self.relay
                .take_ledger_entries()
                .iter()
                .map(dengon_core::ledger::Entry::to_bytes),
        );
    }
}

/// Lit `N` octets à `ptr`, ou `None` si `ptr` est nul.
///
/// # Safety
///
/// `ptr` nul, ou valide pour `N` octets lus.
unsafe fn array<const N: usize>(ptr: *const u8) -> Option<[u8; N]> {
    if ptr.is_null() {
        return None;
    }
    let mut out = [0u8; N];
    // SAFETY : contrat de la fonction.
    unsafe { core::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), N) };
    Some(out)
}

/// Crée un relais. `dh_secret` / `sign_seed` : 32 octets chacun, générés une
/// fois puis relus de NVS. `anchor_seq` / `anchor_hash` (32 octets) : curseur
/// du journal persisté, ou `0` / `NULL` au tout premier démarrage (genèse).
/// `pseudo` : chaîne C UTF-8, ou `NULL` pour `"relais"`. Rend `NULL` si un
/// secret manque.
///
/// # Safety
///
/// `dh_secret`, `sign_seed` : valides pour 32 octets. `anchor_hash` : nul ou
/// valide pour 32 octets. `pseudo` : nul ou chaîne C terminée par `\0`.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_new(
    dh_secret: *const u8,
    sign_seed: *const u8,
    anchor_seq: u64,
    anchor_hash: *const u8,
    pseudo: *const c_char,
    routing_seed: u64,
) -> *mut Relay {
    // SAFETY : contrat de la fonction.
    let (Some(dh_secret), Some(sign_seed)) =
        (unsafe { array(dh_secret) }, unsafe { array(sign_seed) })
    else {
        return core::ptr::null_mut();
    };
    let anchor = Anchor {
        first_seq: anchor_seq,
        // SAFETY : contrat de la fonction.
        prev_hash: unsafe { array(anchor_hash) }.unwrap_or([0; 32]),
    };
    let pseudo = if pseudo.is_null() {
        "relais"
    } else {
        // SAFETY : contrat de la fonction.
        unsafe { CStr::from_ptr(pseudo) }
            .to_str()
            .unwrap_or("relais")
    };
    let secrets = RelaySecrets {
        dh_secret,
        sign_seed,
    };
    let relay = CoreRelay::new(&secrets, anchor, RelayConfig::new(pseudo, routing_seed));
    Box::into_raw(Box::new(Relay {
        relay,
        outgoing: VecDeque::new(),
        ledger: VecDeque::new(),
    }))
}

/// Libère un relais créé par [`dengon_relay_new`] (sans effet sur `NULL`).
///
/// # Safety
///
/// `relay` : nul, ou rendu par [`dengon_relay_new`] et pas encore libéré.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_free(relay: *mut Relay) {
    if !relay.is_null() {
        // SAFETY : contrat de la fonction.
        drop(unsafe { Box::from_raw(relay) });
    }
}

/// Écrit le `peerID` du relais (8 octets) dans `out`.
///
/// # Safety
///
/// `relay` valide ; `out` valide pour 8 octets écrits.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_peer_id(relay: *const Relay, out: *mut u8) -> Status {
    // SAFETY : contrat de la fonction.
    let Some(r) = (unsafe { relay.as_ref() }) else {
        return Status::NullPointer;
    };
    // SAFETY : contrat de la fonction.
    unsafe { write(out, &r.relay.peer_id()) }
}

/// Écrit la clé publique Ed25519 du relais (32 octets) dans `out` : celle de
/// `dengon-verify --pubkey`.
///
/// # Safety
///
/// `relay` valide ; `out` valide pour 32 octets écrits.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_verifying_key(relay: *const Relay, out: *mut u8) -> Status {
    // SAFETY : contrat de la fonction.
    let Some(r) = (unsafe { relay.as_ref() }) else {
        return Status::NullPointer;
    };
    // SAFETY : contrat de la fonction.
    unsafe { write(out, &r.relay.verifying_key()) }
}

/// Un lien vient de s'ouvrir.
///
/// # Safety
///
/// `relay` nul ou valide.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_link_up(relay: *mut Relay, link: u64, now: Now) {
    // SAFETY : contrat de la fonction.
    if let Some(r) = unsafe { relay.as_mut() } {
        r.relay.link_up(link, now.into());
        r.drain();
    }
}

/// Le lien est tombé.
///
/// # Safety
///
/// `relay` nul ou valide.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_link_down(relay: *mut Relay, link: u64) {
    // SAFETY : contrat de la fonction.
    if let Some(r) = unsafe { relay.as_mut() } {
        r.relay.link_down(link);
    }
}

/// Une trame (`len` octets à `bytes`) est arrivée sur `link`.
///
/// # Safety
///
/// `relay` nul ou valide ; `bytes` valide pour `len` octets (ou nul si
/// `len == 0`).
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_on_frame(
    relay: *mut Relay,
    link: u64,
    bytes: *const u8,
    len: usize,
    now: Now,
) {
    // SAFETY : contrat de la fonction.
    let Some(r) = (unsafe { relay.as_mut() }) else {
        return;
    };
    let frame = if len == 0 || bytes.is_null() {
        &[][..]
    } else {
        // SAFETY : contrat de la fonction.
        unsafe { core::slice::from_raw_parts(bytes, len) }
    };
    r.relay.on_frame(link, frame, now.into());
    r.drain();
}

/// Fait avancer toutes les échéances (relais jitterés, pushs, expirations).
///
/// # Safety
///
/// `relay` nul ou valide.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_poll(relay: *mut Relay, now: Now) {
    // SAFETY : contrat de la fonction.
    if let Some(r) = unsafe { relay.as_mut() } {
        r.relay.poll(now.into());
        r.drain();
    }
}

/// Relais jitterés arrivés à échéance seulement (tâche `route`).
///
/// # Safety
///
/// `relay` nul ou valide.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_poll_routing(relay: *mut Relay, now: Now) {
    // SAFETY : contrat de la fonction.
    if let Some(r) = unsafe { relay.as_mut() } {
        r.relay.poll_routing(now.into());
        r.drain();
    }
}

/// Pushs d'inventaire cadencés seulement (tâche `inventory`).
///
/// # Safety
///
/// `relay` nul ou valide.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_poll_inventory(relay: *mut Relay, now: Now) {
    // SAFETY : contrat de la fonction.
    if let Some(r) = unsafe { relay.as_mut() } {
        r.relay.poll_inventory(now.into());
        r.drain();
    }
}

/// Expiration des enveloppes seulement (tâche `courier`).
///
/// # Safety
///
/// `relay` nul ou valide.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_poll_courier(relay: *mut Relay, now: Now) {
    // SAFETY : contrat de la fonction.
    if let Some(r) = unsafe { relay.as_mut() } {
        r.relay.poll_courier(now.into());
        r.drain();
    }
}

/// Prochaine échéance (ms **monotones**) dans `*out` ; `false` s'il n'y en a
/// pas (le firmware peut alors attendre la prochaine trame).
///
/// # Safety
///
/// `relay` nul ou valide ; `out` nul ou valide.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_next_deadline(
    relay: *const Relay,
    now: Now,
    out: *mut u64,
) -> bool {
    // SAFETY : contrat de la fonction.
    let (Some(r), Some(out)) = (unsafe { relay.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    match r.relay.next_deadline(now.into()) {
        Some(t) => {
            *out = t;
            true
        }
        None => false,
    }
}

/// Retire la prochaine trame à émettre : lien dans `*link`, octets dans
/// `buf`, longueur dans `*len`. [`Status::Empty`] si rien à émettre ;
/// [`Status::BufferTooSmall`] si `cap` ne suffit pas (`*len` = taille
/// requise, trame conservée).
///
/// # Safety
///
/// `relay` valide ; `link`, `len` valides ; `buf` valide pour `cap` octets.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_pop_outgoing(
    relay: *mut Relay,
    link: *mut u64,
    buf: *mut u8,
    cap: usize,
    len: *mut usize,
) -> Status {
    // SAFETY : contrat de la fonction.
    let (Some(r), Some(link), Some(len)) = (
        unsafe { relay.as_mut() },
        unsafe { link.as_mut() },
        unsafe { len.as_mut() },
    ) else {
        return Status::NullPointer;
    };
    let Some((l, frame)) = r.outgoing.front() else {
        return Status::Empty;
    };
    *len = frame.len();
    if frame.len() > cap {
        return Status::BufferTooSmall;
    }
    *link = *l;
    // SAFETY : contrat de la fonction, `frame.len() <= cap`.
    let status = unsafe { write(buf, frame) };
    if status == Status::Ok {
        r.outgoing.pop_front();
    }
    status
}

/// Retire la prochaine entrée de journal à persister (`Entry::to_bytes`,
/// à ajouter telle quelle à `ledger.bin`). Mêmes conventions que
/// [`dengon_relay_pop_outgoing`].
///
/// # Safety
///
/// `relay` valide ; `len` valide ; `buf` valide pour `cap` octets.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_pop_ledger(
    relay: *mut Relay,
    buf: *mut u8,
    cap: usize,
    len: *mut usize,
) -> Status {
    // SAFETY : contrat de la fonction.
    let (Some(r), Some(len)) = (unsafe { relay.as_mut() }, unsafe { len.as_mut() }) else {
        return Status::NullPointer;
    };
    let Some(entry) = r.ledger.front() else {
        return Status::Empty;
    };
    *len = entry.len();
    if entry.len() > cap {
        return Status::BufferTooSmall;
    }
    // SAFETY : contrat de la fonction, `entry.len() <= cap`.
    let status = unsafe { write(buf, entry) };
    if status == Status::Ok {
        r.ledger.pop_front();
    }
    status
}

/// Curseur du journal **après** toutes les entrées produites jusqu'ici
/// (y compris celles pas encore retirées par [`dengon_relay_pop_ledger`]) :
/// `seq` de la prochaine entrée dans `*seq`, hash de la dernière (32
/// octets) dans `hash`. À persister **une fois la file vidée et écrite**.
///
/// # Safety
///
/// `relay`, `seq` valides ; `hash` valide pour 32 octets écrits.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_ledger_anchor(
    relay: *const Relay,
    seq: *mut u64,
    hash: *mut u8,
) -> Status {
    // SAFETY : contrat de la fonction.
    let (Some(r), Some(seq)) = (unsafe { relay.as_ref() }, unsafe { seq.as_mut() }) else {
        return Status::NullPointer;
    };
    let anchor = r.relay.ledger_anchor();
    *seq = anchor.first_seq;
    // SAFETY : contrat de la fonction.
    unsafe { write(hash, &anchor.prev_hash) }
}

/// Ajoute au journal un événement du firmware (`relay.boot`,
/// `peer.connected`…). `false` si le nom n'est pas au catalogue ou si une
/// chaîne n'est pas de l'UTF-8.
///
/// # Safety
///
/// `relay` nul ou valide ; `name`, `payload_json` nuls ou chaînes C
/// terminées par `\0`.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_record_event(
    relay: *mut Relay,
    name: *const c_char,
    payload_json: *const c_char,
    now: Now,
) -> bool {
    // SAFETY : contrat de la fonction.
    let Some(r) = (unsafe { relay.as_mut() }) else {
        return false;
    };
    if name.is_null() || payload_json.is_null() {
        return false;
    }
    // SAFETY : contrat de la fonction.
    let (Ok(name), Ok(payload)) = (
        unsafe { CStr::from_ptr(name) }.to_str(),
        unsafe { CStr::from_ptr(payload_json) }.to_str(),
    ) else {
        return false;
    };
    let ok = r.relay.record_event(name, payload, now.into());
    r.drain();
    ok
}

/// Compteurs du relais dans `*out`.
///
/// # Safety
///
/// `relay`, `out` valides.
#[no_mangle]
pub unsafe extern "C" fn dengon_relay_stats(relay: *const Relay, out: *mut RelayStats) -> Status {
    // SAFETY : contrat de la fonction.
    let (Some(r), Some(out)) = (unsafe { relay.as_ref() }, unsafe { out.as_mut() }) else {
        return Status::NullPointer;
    };
    let s = r.relay.stats();
    *out = RelayStats {
        malformed: s.malformed,
        unauthentic: s.unauthentic,
        relayed: s.relayed,
        envelopes_stored: s.envelopes_stored,
        envelopes_handed_off: s.envelopes_handed_off,
        clock_unknown: s.clock_unknown,
        envelopes_held: u32::try_from(r.relay.envelopes_held()).unwrap_or(u32::MAX),
        cache_len: u32::try_from(r.relay.cache_len()).unwrap_or(u32::MAX),
    };
    Status::Ok
}

/// Copie `src` à `dst`.
///
/// # Safety
///
/// `dst` nul, ou valide pour `src.len()` octets écrits.
unsafe fn write(dst: *mut u8, src: &[u8]) -> Status {
    if src.is_empty() {
        return Status::Ok;
    }
    if dst.is_null() {
        return Status::NullPointer;
    }
    // SAFETY : contrat de la fonction.
    unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len()) };
    Status::Ok
}
