//! Tests du relais (US-308) : deux relais et des « téléphones » simulés
//! (identités qui forgent leurs paquets à la main, faute de client qui émet
//! `ANNOUNCE` / `ENVELOPE_REQUEST` aujourd'hui — voir la doc de module).

use alloc::string::ToString;
use alloc::vec;

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

use super::*;
use crate::crypto::tag::{epoch_day, recipient_tag};
use crate::ledger::{verify_entries, verify_signatures, Verdict};
use crate::protocol::codec::announce::Announce;
use crate::protocol::codec::{decode, encode, signing_input};
use crate::protocol::consts::TTL_DEFAULT;

/// Position de `sender_id` dans l'en-tête L3 (`codec`, octets 12..20).
const OCTET_SENDER: core::ops::Range<usize> = 12..20;

/// Heure murale plausible (2025-09-01).
const T0: u64 = 1_756_684_800_000;

fn now(ms: u64) -> Now {
    Now::new(T0 + ms, ms)
}

/// Heure murale inconnue (ESP32 sans SNTP).
fn sans_heure(ms: u64) -> Now {
    Now::new(ms, ms)
}

fn secrets(n: u8) -> RelaySecrets {
    RelaySecrets {
        dh_secret: [n; 32],
        sign_seed: [n.wrapping_add(100); 32],
    }
}

fn relais(n: u8) -> Relay<u64> {
    Relay::new(
        &secrets(n),
        Anchor::GENESIS,
        RelayConfig::new(&alloc::format!("relais-{n}"), u64::from(n)),
    )
}

fn telephone(seed: u64) -> Identity {
    match Identity::generate("tel", ChaCha20Rng::seed_from_u64(seed)) {
        Ok(i) => i,
        Err(e) => panic!("{e}"),
    }
}

/// Paquet forgé par `id` ; signé si le type l'exige ou si `signe`.
#[allow(clippy::too_many_arguments)]
fn paquet(
    id: &Identity,
    packet_type: PacketType,
    recipient: Option<PeerId>,
    ttl: u8,
    relay_ok: bool,
    ts: u64,
    payload: Vec<u8>,
    signe: bool,
) -> Vec<u8> {
    let mut flags = Flags::empty();
    if recipient.is_some() {
        flags = flags | Flags::ADDRESSED;
    }
    if relay_ok {
        flags = flags | Flags::RELAY_OK;
    }
    if signe {
        flags = flags | Flags::SIGNED;
    }
    let mut p = Packet {
        header: Header {
            version: PROTO_VERSION,
            packet_type,
            ttl,
            flags,
            timestamp_ms: ts,
            sender_id: id.peer_id(),
            recipient_id: recipient,
            payload_len: u16::try_from(payload.len()).unwrap(),
        },
        payload,
        signature: None,
    };
    if signe {
        p.signature = Some(id.signing_key().sign(&signing_input(&p).unwrap()));
    }
    encode(&p).unwrap()
}

fn announce(id: &Identity, ts: u64) -> Vec<u8> {
    let a = Announce {
        peer_id: id.peer_id(),
        pub_static: id.static_keypair().public(),
        pub_sign: id.signing_key().verifying_key().to_bytes(),
        pseudo: "tel".to_string(),
        ledger_height: 0,
        caps: 0,
    };
    paquet(
        id,
        PacketType::Announce,
        None,
        1,
        false,
        ts,
        a.encode().unwrap(),
        true,
    )
}

fn enveloppe_pour(dest: &Identity, exp: &Identity, ts: u64) -> Vec<u8> {
    let mut payload = recipient_tag(&dest.static_keypair().public(), epoch_day(ts)).to_vec();
    payload.extend_from_slice(&epoch_day(ts).to_be_bytes());
    payload.extend_from_slice(b"ciphertext opaque");
    paquet(
        exp,
        PacketType::SealedEnvelope,
        None,
        TTL_DEFAULT,
        false,
        ts,
        payload,
        true,
    )
}

fn types_emis(out: &[(u64, Vec<u8>)], lien: u64) -> Vec<PacketType> {
    out.iter()
        .filter(|(l, _)| *l == lien)
        .map(|(_, b)| decode(b).unwrap().header.packet_type)
        .collect()
}

fn noms(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.event_name.as_str()).collect()
}

/// Relie deux relais par un lien (`1` chez chacun) et fait circuler les
/// trames jusqu'à épuisement.
fn relier(a: &mut Relay<u64>, b: &mut Relay<u64>, t: u64) {
    a.link_up(1, now(t));
    b.link_up(1, now(t));
    for _ in 0..4 {
        for (l, f) in a.take_outgoing() {
            assert_eq!(l, 1);
            b.on_frame(1, &f, now(t));
        }
        for (l, f) in b.take_outgoing() {
            assert_eq!(l, 1);
            a.on_frame(1, &f, now(t));
        }
    }
}

#[test]
fn le_relais_s_annonce_a_l_ouverture_d_un_lien() {
    let mut r = relais(1);
    r.link_up(7, now(0));
    let out = r.take_outgoing();
    assert_eq!(out.len(), 1);
    let p = decode(&out[0].1).unwrap();
    let a = Announce::verify(&p, &out[0].1).unwrap();
    assert_eq!(a.peer_id, r.peer_id());
    assert_eq!(a.caps & CAP_RELAY, CAP_RELAY);
    assert_eq!(p.header.ttl, 1);
    assert!(!p.header.flags.contains(Flags::RELAY_OK));
}

#[test]
fn deux_relais_se_lient_et_echangent_leur_inventaire() {
    let (mut a, mut b) = (relais(1), relais(2));
    relier(&mut a, &mut b, 0);
    for r in [&mut a, &mut b] {
        let entrees = r.take_ledger_entries();
        assert_eq!(noms(&entrees), vec!["peer.announce_seen"]);
    }
    assert_eq!(a.stats().unauthentic, 0);
}

#[test]
fn un_message_traverse_deux_relais() {
    // P1 — A — B — P2 : P1 écrit à P2, hors de portée l'un de l'autre.
    let (mut a, mut b) = (relais(1), relais(2));
    relier(&mut a, &mut b, 0);
    let (p1, p2) = (telephone(1), telephone(2));
    a.link_up(10, now(0));
    b.link_up(20, now(0));
    let _ = (a.take_outgoing(), b.take_outgoing());

    let msg = paquet(
        &p1,
        PacketType::NoiseMsg,
        Some(p2.peer_id()),
        TTL_DEFAULT,
        true,
        T0,
        b"chiffre".to_vec(),
        false,
    );
    a.on_frame(10, &msg, now(10));
    a.poll(now(1_000));
    let de_a = a.take_outgoing();
    assert_eq!(de_a.len(), 1, "relayé vers B seulement, pas vers P1");
    assert_eq!(de_a[0].0, 1);
    assert_eq!(de_a[0].1[TTL_OFFSET], TTL_DEFAULT - 1);

    b.on_frame(1, &de_a[0].1, now(1_010));
    b.poll(now(2_000));
    let de_b = b.take_outgoing();
    assert_eq!(de_b.len(), 1);
    assert_eq!(de_b[0].0, 20, "remis sur le lien de P2");
    assert_eq!(de_b[0].1[TTL_OFFSET], TTL_DEFAULT - 2);
    assert_eq!(decode(&de_b[0].1).unwrap().payload, b"chiffre");

    let journal = b.take_ledger_entries();
    assert!(noms(&journal).contains(&"pkt.relayed"));
    assert_eq!(b.stats().relayed, 1);

    // Le même paquet revenu : doublon, pas de second relais.
    b.on_frame(20, &de_b[0].1, now(2_100));
    b.poll(now(3_000));
    assert!(b.take_outgoing().is_empty());
}

#[test]
fn sans_heure_le_relais_attend_un_announce_pour_router() {
    let mut r = relais(1);
    let (p1, p2) = (telephone(1), telephone(2));
    r.link_up(10, sans_heure(0));
    r.link_up(11, sans_heure(0));
    let msg = |ts| {
        paquet(
            &p1,
            PacketType::NoiseMsg,
            Some(p2.peer_id()),
            TTL_DEFAULT,
            true,
            ts,
            b"x".to_vec(),
            false,
        )
    };
    r.on_frame(10, &msg(T0), sans_heure(5));
    assert_eq!(r.stats().clock_unknown, 1);
    assert!(!r.clock_known(sans_heure(5)));

    // P1 s'annonce, avec l'heure du téléphone : le relais l'adopte.
    r.on_frame(10, &announce(&p1, T0 + 10), sans_heure(10));
    assert!(r.clock_known(sans_heure(10)));
    r.on_frame(10, &msg(T0 + 20), sans_heure(20));
    r.poll(sans_heure(1_000));
    let relayes: Vec<_> = r
        .take_outgoing()
        .into_iter()
        .filter(|(l, _)| *l == 11)
        .collect();
    assert!(relayes
        .iter()
        .any(|(_, b)| decode(b).unwrap().header.packet_type == PacketType::NoiseMsg));
}

#[test]
fn enveloppe_deposee_puis_remise_sur_requete() {
    let mut r = relais(1);
    let (p1, p2) = (telephone(1), telephone(2));
    r.link_up(10, now(0));
    r.on_frame(10, &announce(&p1, T0), now(0));
    let _ = r.take_outgoing();

    r.on_frame(10, &enveloppe_pour(&p2, &p1, T0 + 5), now(5));
    assert_eq!(r.envelopes_held(), 1);

    // P2 arrive et s'annonce : inventaire, puis offre de l'enveloppe.
    r.link_up(20, now(100));
    r.on_frame(20, &announce(&p2, T0 + 100), now(100));
    let out = r.take_outgoing();
    let emis = types_emis(&out, 20);
    assert_eq!(
        emis,
        vec![
            PacketType::Announce,
            PacketType::Inventory,
            PacketType::EnvelopeOffer
        ]
    );
    let offre = out
        .iter()
        .find(|(l, b)| {
            *l == 20 && decode(b).unwrap().header.packet_type == PacketType::EnvelopeOffer
        })
        .map(|(_, b)| decode(b).unwrap())
        .unwrap();
    let tags = courier::decode_tag_list(&offre.payload).unwrap();
    assert_eq!(tags.len(), 1);

    // P2 reconnaît son tag et demande l'enveloppe.
    let requete = paquet(
        &p2,
        PacketType::EnvelopeRequest,
        Some(r.peer_id()),
        1,
        false,
        T0 + 200,
        courier::encode_tag_list(&tags),
        true,
    );
    r.on_frame(20, &requete, now(200));
    let remise = r.take_outgoing();
    assert_eq!(types_emis(&remise, 20), vec![PacketType::SealedEnvelope]);
    assert_eq!(r.envelopes_held(), 0);

    let journal = r.take_ledger_entries();
    let n = noms(&journal);
    for attendu in [
        "peer.announce_seen",
        "envelope.stored",
        "envelope.offered",
        "envelope.handoff",
    ] {
        assert!(n.contains(&attendu), "{attendu} absent de {n:?}");
    }
}

#[test]
fn une_nouvelle_enveloppe_est_offerte_aux_voisins_deja_lies() {
    let mut r = relais(1);
    let (p1, p2) = (telephone(1), telephone(2));
    for (lien, p) in [(10, &p1), (20, &p2)] {
        r.link_up(lien, now(0));
        r.on_frame(lien, &announce(p, T0), now(0));
    }
    let _ = r.take_outgoing();
    r.on_frame(10, &enveloppe_pour(&p2, &p1, T0 + 5), now(5));
    let out = r.take_outgoing();
    assert_eq!(types_emis(&out, 20), vec![PacketType::EnvelopeOffer]);
    assert!(
        types_emis(&out, 10).is_empty(),
        "pas renvoyée à l'expéditeur"
    );
}

#[test]
fn announce_usurpe_refuse_et_lien_reste_anonyme() {
    let mut r = relais(1);
    let (p1, p2) = (telephone(1), telephone(2));
    r.link_up(10, now(0));
    let _ = r.take_outgoing();
    // Clés de P1 annoncées, mais signées par P2.
    let a = Announce {
        peer_id: p1.peer_id(),
        pub_static: p1.static_keypair().public(),
        pub_sign: p1.signing_key().verifying_key().to_bytes(),
        pseudo: "tel".to_string(),
        ledger_height: 0,
        caps: 0,
    };
    let mut brut = paquet(
        &p2,
        PacketType::Announce,
        None,
        1,
        false,
        T0,
        a.encode().unwrap(),
        true,
    );
    // `sender_id` de P1 (sinon `PeerIdMismatch` avant la signature).
    brut[OCTET_SENDER].copy_from_slice(&p1.peer_id());
    r.on_frame(10, &brut, now(0));
    assert_eq!(r.stats().unauthentic, 1);
    assert!(r.take_outgoing().is_empty(), "ni inventaire ni offre");
    let journal = r.take_ledger_entries();
    assert_eq!(noms(&journal), vec!["pkt.rejected"]);
    assert!(journal[0].payload_json.contains("\"reason\":\"bad_sig\""));
}

#[test]
fn requete_mal_signee_ne_remet_rien() {
    let mut r = relais(1);
    let (p1, p2) = (telephone(1), telephone(2));
    r.link_up(10, now(0));
    r.on_frame(10, &announce(&p1, T0), now(0));
    r.on_frame(10, &enveloppe_pour(&p2, &p1, T0), now(1));
    r.link_up(20, now(2));
    r.on_frame(20, &announce(&p2, T0 + 2), now(2));
    let _ = r.take_outgoing();

    let tag = recipient_tag(&p2.static_keypair().public(), epoch_day(T0));
    // Requête « de P2 » signée par P1.
    let mut requete = paquet(
        &p1,
        PacketType::EnvelopeRequest,
        Some(r.peer_id()),
        1,
        false,
        T0 + 3,
        courier::encode_tag_list(&[tag]),
        true,
    );
    requete[OCTET_SENDER].copy_from_slice(&p2.peer_id());
    r.on_frame(20, &requete, now(3));
    assert!(r.take_outgoing().is_empty());
    assert_eq!(r.envelopes_held(), 1);
    assert_eq!(r.stats().unauthentic, 1);
}

#[test]
fn trame_illisible_journalisee_sans_panique() {
    let mut r = relais(1);
    r.link_up(10, now(0));
    for n in 0..40 {
        r.on_frame(10, &vec![0xff; n], now(0));
    }
    assert_eq!(r.stats().malformed, 40);
}

#[test]
fn le_journal_survit_a_un_redemarrage() {
    // Premier « boot » : quelques événements, persistés puis perdus en RAM.
    let (p1, p2) = (telephone(1), telephone(2));
    let mut r = relais(1);
    let cle = r.verifying_key();
    assert!(r.record_event("relay.boot", "{}", now(0)));
    r.link_up(10, now(0));
    r.on_frame(10, &announce(&p1, T0), now(0));
    r.on_frame(10, &enveloppe_pour(&p2, &p1, T0), now(1));
    let mut fichier = r.take_ledger_entries();
    let ancre = r.ledger_anchor();
    drop(r);

    // `esp_restart()` : mêmes secrets, seule l'ancre est relue.
    let mut r = Relay::<u64>::new(&secrets(1), ancre, RelayConfig::new("relais-1", 1));
    assert!(r.record_event("relay.boot", "{}", now(10_000)));
    r.link_up(10, now(10_000));
    r.on_frame(10, &announce(&p2, T0 + 10_000), now(10_000));
    fichier.extend(r.take_ledger_entries());

    assert!(fichier.len() >= 5);
    assert_eq!(verify_entries(&fichier, Anchor::GENESIS), Verdict::Ok);
    let cle = VerifyingKey::from_bytes(&cle).unwrap();
    assert_eq!(verify_signatures(&fichier, &cle), None);
    // Octets de fichier relisibles tels que `dengon-verify` les lit.
    let octets: Vec<u8> = fichier.iter().flat_map(Entry::to_bytes).collect();
    let mut reste = octets.as_slice();
    let mut relues = 0;
    while let Some((_, suite)) = Entry::from_bytes(reste) {
        relues += 1;
        reste = suite;
    }
    assert_eq!(relues, fichier.len());
}

#[test]
fn evenement_hors_catalogue_refuse() {
    let mut r = relais(1);
    assert!(!r.record_event("pkt.inconnu", "{}", now(0)));
    assert!(r.take_ledger_entries().is_empty());
}

#[test]
fn les_paquets_du_relais_tiennent_en_une_trame() {
    let mut r = relais(1);
    let p1 = telephone(1);
    r.link_up(10, now(0));
    r.on_frame(10, &announce(&p1, T0), now(0));
    // 40 enveloppes pour 40 destinataires distincts, puis 60 messages en cache.
    for i in 0..40u64 {
        let dest = telephone(100 + i);
        r.on_frame(10, &enveloppe_pour(&dest, &p1, T0 + i), now(i));
    }
    for i in 0..60u64 {
        let msg = paquet(
            &p1,
            PacketType::NoiseMsg,
            Some([9; 8]),
            TTL_DEFAULT,
            true,
            T0 + 100 + i,
            b"x".to_vec(),
            false,
        );
        r.on_frame(10, &msg, now(100 + i));
    }
    let _ = r.take_outgoing();
    let p2 = telephone(2);
    r.link_up(20, now(70_000));
    r.on_frame(20, &announce(&p2, T0 + 70_000), now(70_000));
    let out = r.take_outgoing();
    assert!(types_emis(&out, 20).contains(&PacketType::Inventory));
    assert!(types_emis(&out, 20).contains(&PacketType::EnvelopeOffer));
    for (_, trame) in &out {
        assert!(trame.len() <= FRAME_MAX, "trame de {} octets", trame.len());
    }
    assert_eq!(RELAY_INVENTORY_MAX_IDS, 13);
    assert_eq!(RELAY_OFFER_MAX_TAGS, 26);
}

// ----- Export vers le dashboard (US-309) -----------------------------------

#[test]
fn le_journal_du_relais_part_en_batch_signe_par_sa_cle() {
    let mut r = relais(1);
    assert!(r.record_event(
        "relay.boot",
        r#"{"fw_version":"0.1.0","reset_reason":"power_on","secure_boot":false,"flash_enc":false}"#,
        now(0),
    ));
    assert!(r.record_event("relay.wifi_up", r#"{"ssid":"ap","duration_s":0}"#, now(5)));
    let entries = r.take_ledger_entries();
    let corps = r.build_batch(&entries).unwrap();
    let texte = core::str::from_utf8(&corps).unwrap();

    let node_id = r.node_id();
    assert_eq!(node_id.len(), 12);
    assert!(node_id.starts_with("relay-"));
    assert!(texte.contains(&alloc::format!("\"node_id\":\"{node_id}\"")));
    assert!(texte.contains("\"seq\":0"));
    assert!(texte.contains("\"seq\":1"));

    // La signature se vérifie avec la clé publique annoncée par le relais.
    let Value::Object(mut obj) = crate::observability::canonical::parse(texte).unwrap() else {
        panic!("objet attendu")
    };
    let Some(Value::Str(sig)) = obj.remove("sig") else {
        panic!("sig absent")
    };
    let sig: [u8; 64] = data_encoding::BASE64
        .decode(sig.as_bytes())
        .unwrap()
        .try_into()
        .unwrap();
    let pk = VerifyingKey::from_bytes(&r.verifying_key()).unwrap();
    assert!(pk
        .verify(&Value::Object(obj).to_canonical_bytes(), &sig)
        .is_ok());
}

#[test]
fn un_batch_vide_n_est_pas_construit() {
    assert_eq!(relais(1).build_batch(&[]), Err(BatchError::Empty));
}
