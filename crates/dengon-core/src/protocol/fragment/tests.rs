//! Tests de la fragmentation L2 : découpe, format, réassemblage dans le
//! désordre / avec doublons / avec manques, bornes mémoire.

use alloc::string::ToString;
use alloc::vec;

use proptest::prelude::*;

use super::*;
use crate::protocol::consts::{ATT_MTU_MIN, ATT_MTU_PREFERRED};

const T0: u64 = 1_700_000_000_000;
const TIMEOUT_MS: u64 = FRAG_TIMEOUT_S as u64 * 1000;

fn packet(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(i % 251).unwrap() ^ seed)
        .collect()
}

// --- MTU et découpe ---------------------------------------------------------

#[test]
fn capacite_selon_le_mtu() {
    // MTU mesuré au Spike C (US-103) : 517 → plafonné à FRAG_SIZE.
    assert_eq!(chunk_capacity(ATT_MTU_PREFERRED), Some(FRAG_SIZE));
    // 185 - 3 - 30 - 12 = 140.
    assert_eq!(chunk_capacity(185), Some(140));
    // Le minimum BLE ne porte pas un fragment dengon.
    assert_eq!(chunk_capacity(ATT_MTU_MIN), None);
    assert_eq!(MIN_USABLE_ATT_MTU, 46);
    let min = u16::try_from(MIN_USABLE_ATT_MTU).unwrap();
    assert_eq!(chunk_capacity(min), Some(1));
    assert_eq!(chunk_capacity(min - 1), None);
}

#[test]
fn besoin_de_fragmenter() {
    assert!(!needs_fragmentation(514, 517));
    assert!(needs_fragmentation(515, 517));
    assert!(needs_fragmentation(1, 0));
}

#[test]
fn decoupe_nominale() {
    let p = packet(1000, 7);
    let frags = split(&p, 440).unwrap();
    assert_eq!(frags.len(), 3);
    assert_eq!(
        frags.iter().map(|f| f.chunk.len()).collect::<Vec<_>>(),
        vec![440, 440, 120]
    );
    for (i, f) in frags.iter().enumerate() {
        assert_eq!(usize::from(f.index), i);
        assert_eq!(f.total, 3);
        assert_eq!(f.frag_id, frag_id(&p));
    }
}

#[test]
fn decoupe_refuse_les_entrees_invalides() {
    assert_eq!(split(&[], 10), Err(FragmentError::EmptyPacket));
    assert_eq!(split(&[1], 0), Err(FragmentError::InvalidChunkLen));
    assert_eq!(
        split(&[1], FRAG_SIZE + 1),
        Err(FragmentError::InvalidChunkLen)
    );
    let big = vec![0; PACKET_MAX_LEN + 1];
    assert_eq!(split(&big, FRAG_SIZE), Err(FragmentError::PacketTooLarge));
    // Paquet maximal en chunks d'un octet : plus de u16::MAX fragments.
    let max = vec![0; PACKET_MAX_LEN];
    assert_eq!(split(&max, 1), Err(FragmentError::TooManyFragments));
    assert_eq!(
        split_for_mtu(&[1], ATT_MTU_MIN),
        Err(FragmentError::MtuTooSmall)
    );
    assert_eq!(split_for_mtu(&[1, 2, 3], 517).unwrap().len(), 1);
}

// --- Format ---------------------------------------------------------------

#[test]
fn format_du_payload() {
    let f = Fragment {
        frag_id: [0xAA; 8],
        index: 0x0102,
        total: 0x0304,
        chunk: vec![9, 8, 7],
    };
    let bytes = f.encode();
    assert_eq!(bytes.len(), FRAG_HEADER_LEN + 3);
    assert_eq!(&bytes[8..12], &[0x01, 0x02, 0x03, 0x04]);
    assert_eq!(Fragment::decode(&bytes).unwrap(), f);
}

#[test]
fn decodage_rejette_les_payloads_invalides() {
    let ok = Fragment {
        frag_id: [1; 8],
        index: 0,
        total: 2,
        chunk: vec![1],
    }
    .encode();
    assert_eq!(Fragment::decode(&ok[..11]), Err(FragmentError::Truncated));
    assert_eq!(Fragment::decode(&ok[..12]), Err(FragmentError::EmptyChunk));
    let mut b = ok.clone();
    b[10..12].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(Fragment::decode(&b), Err(FragmentError::InvalidIndex));
    let mut b = ok.clone();
    b[8..10].copy_from_slice(&2u16.to_be_bytes());
    assert_eq!(Fragment::decode(&b), Err(FragmentError::InvalidIndex));
    let mut b = ok[..12].to_vec();
    b.extend_from_slice(&[0; FRAG_SIZE + 1]);
    assert_eq!(Fragment::decode(&b), Err(FragmentError::ChunkTooLarge));
}

// --- Réassemblage ---------------------------------------------------------

#[test]
fn reassemblage_dans_le_desordre_avec_doublons() {
    let p = packet(1000, 3);
    let mut frags = split(&p, 100).unwrap();
    frags.reverse();
    let dup = frags[4].clone();
    frags.insert(2, dup);

    let mut r = Reassembler::default();
    let mut out = Vec::new();
    for f in frags {
        if let Some(done) = r.push(f, T0).unwrap() {
            out.push(done);
        }
    }
    assert_eq!(out, vec![p]);
    assert!(r.is_empty());
    assert_eq!(r.buffered_bytes(), 0);
}

#[test]
fn paquet_en_un_seul_fragment() {
    let p = packet(50, 1);
    let mut frags = split(&p, 440).unwrap();
    let mut r = Reassembler::default();
    assert_eq!(r.push(frags.remove(0), T0).unwrap(), Some(p));
    assert!(r.is_empty());
}

#[test]
fn doublon_tardif_ignore_sans_reassemblage_zombie() {
    let p = packet(300, 8);
    let frags = split(&p, 100).unwrap();
    let mut r = Reassembler::default();
    for f in &frags {
        r.push(f.clone(), T0).unwrap();
    }
    // Fragment rejoué après la fin : ni seconde sortie, ni réassemblage ouvert.
    assert_eq!(r.push(frags[0].clone(), T0 + 10).unwrap(), None);
    assert!(r.is_empty());
    assert_eq!(r.buffered_bytes(), 0);

    // Idem pour un paquet d'un seul fragment (autre paquet, donc autre frag_id).
    let q = packet(50, 9);
    let single = split(&q, FRAG_SIZE).unwrap().remove(0);
    assert_eq!(r.push(single.clone(), T0).unwrap(), Some(q.clone()));
    assert_eq!(r.push(single.clone(), T0 + 1).unwrap(), None);
    // Après `timeout_ms`, le frag_id est oublié : le paquet peut ressortir.
    assert_eq!(r.push(single, T0 + TIMEOUT_MS + 2).unwrap(), Some(q));
}

#[test]
fn memoire_des_termines_bornee() {
    let mut r = Reassembler::default();
    let singles: Vec<Fragment> = (0..=COMPLETED_MEMORY)
        .map(|i| {
            let p = packet(10 + i, 0);
            split(&p, FRAG_SIZE).unwrap().remove(0)
        })
        .collect();
    for (i, f) in singles.iter().enumerate() {
        let t = T0 + u64::try_from(i).unwrap();
        assert!(r.push(f.clone(), t).unwrap().is_some());
    }
    assert_eq!(r.completed.len(), COMPLETED_MEMORY);
    // Le plus ancien a été oublié : il ressort ; le plus récent, non.
    let t = T0 + 1000;
    assert!(r.push(singles[0].clone(), t).unwrap().is_some());
    assert_eq!(r.push(singles[COMPLETED_MEMORY].clone(), t).unwrap(), None);
}

#[test]
fn fragment_manquant_puis_abandon_au_timeout() {
    let p = packet(500, 2);
    let frags = split(&p, 100).unwrap();
    let mut r = Reassembler::default();
    for f in frags.iter().skip(1).cloned() {
        assert_eq!(r.push(f, T0).unwrap(), None);
    }
    assert_eq!(r.len(), 1);
    // À l'échéance exacte : toujours là.
    assert_eq!(r.expire(T0 + TIMEOUT_MS), 0);
    assert_eq!(r.expire(T0 + TIMEOUT_MS + 1), 1);
    assert!(r.is_empty());
    assert_eq!(r.buffered_bytes(), 0);
    // Le fragment manquant arrive trop tard : nouveau réassemblage incomplet.
    assert_eq!(r.push(frags[0].clone(), T0 + TIMEOUT_MS + 2).unwrap(), None);
    assert_eq!(r.len(), 1);
}

#[test]
fn l_activite_repousse_le_timeout() {
    let frags = split(&packet(300, 4), 100).unwrap();
    let mut r = Reassembler::default();
    r.push(frags[0].clone(), T0).unwrap();
    r.push(frags[1].clone(), T0 + TIMEOUT_MS).unwrap();
    assert_eq!(r.expire(T0 + TIMEOUT_MS + 5), 0);
}

#[test]
fn total_incoherent_rejete_sans_corrompre() {
    let p = packet(300, 5);
    let frags = split(&p, 100).unwrap();
    let mut r = Reassembler::default();
    r.push(frags[0].clone(), T0).unwrap();
    let mut bad = frags[1].clone();
    bad.total = 4;
    assert_eq!(r.push(bad, T0), Err(FragmentError::InconsistentTotal));
    r.push(frags[1].clone(), T0).unwrap();
    assert_eq!(r.push(frags[2].clone(), T0).unwrap(), Some(p));
}

#[test]
fn fragment_altere_detecte_par_frag_id() {
    let p = packet(300, 6);
    let mut frags = split(&p, 100).unwrap();
    frags[1].chunk[0] ^= 0x01;
    let mut r = Reassembler::default();
    r.push(frags[0].clone(), T0).unwrap();
    r.push(frags[1].clone(), T0).unwrap();
    assert_eq!(
        r.push(frags[2].clone(), T0),
        Err(FragmentError::IntegrityMismatch)
    );
    assert!(r.is_empty());

    // Idem pour un paquet d'un seul fragment.
    let mut single = split(&p, FRAG_SIZE).unwrap().remove(0);
    single.chunk[0] ^= 0x01;
    assert_eq!(r.push(single, T0), Err(FragmentError::IntegrityMismatch));
}

#[test]
fn fragment_invalide_rejete() {
    let mut r = Reassembler::default();
    let f = Fragment {
        frag_id: [0; 8],
        index: 3,
        total: 3,
        chunk: vec![1],
    };
    assert_eq!(r.push(f, T0), Err(FragmentError::InvalidIndex));
    assert!(r.is_empty());
}

#[test]
fn eviction_du_plus_ancien_au_dela_de_max_concurrent() {
    let config = ReassemblerConfig {
        max_concurrent: 2,
        ..ReassemblerConfig::default()
    };
    let mut r = Reassembler::new(config);
    assert_eq!(r.config(), config);
    let a = split(&packet(200, 10), 100).unwrap();
    let b = split(&packet(200, 11), 100).unwrap();
    let c = split(&packet(200, 12), 100).unwrap();
    r.push(a[0].clone(), T0).unwrap();
    r.push(b[0].clone(), T0).unwrap();
    r.push(c[0].clone(), T0).unwrap(); // évince `a`
    assert_eq!(r.len(), 2);
    // `a` repart de zéro : son 2e fragment seul ne suffit pas.
    assert_eq!(r.push(a[1].clone(), T0).unwrap(), None);
    // `b` a été évincé à son tour (plus ancien) ; `c` se termine.
    assert!(r.push(c[1].clone(), T0).unwrap().is_some());
}

#[test]
fn budget_memoire_evince_les_autres_puis_abandonne() {
    let config = ReassemblerConfig {
        max_bytes: 2 * (100 + CHUNK_OVERHEAD),
        ..ReassemblerConfig::default()
    };
    let mut r = Reassembler::new(config);
    let a = split(&packet(300, 20), 100).unwrap();
    let b = split(&packet(400, 21), 100).unwrap();
    r.push(a[0].clone(), T0).unwrap();
    r.push(a[1].clone(), T0).unwrap();
    // `b` n'a pas la place : `a` (plus ancien) est évincé.
    r.push(b[0].clone(), T0).unwrap();
    assert_eq!(r.len(), 1);
    r.push(b[1].clone(), T0).unwrap();
    assert_eq!(r.buffered_bytes(), config.max_bytes);
    // Plus rien à évincer : `b` lui-même est abandonné.
    assert_eq!(r.push(b[2].clone(), T0), Err(FragmentError::OverBudget));
    assert!(r.is_empty());
    assert_eq!(r.buffered_bytes(), 0);

    // Un chunk seul plus gros que le budget.
    let mut tiny = Reassembler::new(ReassemblerConfig {
        max_bytes: 10,
        ..ReassemblerConfig::default()
    });
    assert_eq!(tiny.push(a[0].clone(), T0), Err(FragmentError::OverBudget));
}

#[test]
fn reassemblage_borne_a_packet_max_len() {
    // Un pair annonce 65535 fragments de FRAG_SIZE : au-delà de
    // PACKET_MAX_LEN octets cumulés, le réassemblage est abandonné.
    let mut r = Reassembler::new(ReassemblerConfig {
        max_bytes: usize::MAX,
        ..ReassemblerConfig::default()
    });
    let mut result = Ok(None);
    for index in 0..u16::MAX {
        result = r.push(
            Fragment {
                frag_id: [0x42; 8],
                index,
                total: u16::MAX,
                chunk: vec![0; FRAG_SIZE],
            },
            T0,
        );
        if result.is_err() {
            break;
        }
    }
    assert_eq!(result, Err(FragmentError::PacketTooLarge));
    assert!(r.is_empty());
    assert_eq!(r.buffered_bytes(), 0);
}

#[test]
fn messages_d_erreur() {
    for e in [
        FragmentError::Truncated,
        FragmentError::EmptyChunk,
        FragmentError::ChunkTooLarge,
        FragmentError::InvalidIndex,
        FragmentError::EmptyPacket,
        FragmentError::PacketTooLarge,
        FragmentError::InvalidChunkLen,
        FragmentError::TooManyFragments,
        FragmentError::MtuTooSmall,
        FragmentError::InconsistentTotal,
        FragmentError::OverBudget,
        FragmentError::IntegrityMismatch,
    ] {
        assert!(!e.to_string().is_empty());
    }
}

// --- Property tests ---------------------------------------------------------

fn mtu() -> impl Strategy<Value = u16> {
    u16::try_from(MIN_USABLE_ATT_MTU).unwrap()..=ATT_MTU_PREFERRED
}

/// Fragment arbitraire, `frag_id` pris dans un petit ensemble pour provoquer
/// des collisions entre réassemblages.
fn any_fragment() -> impl Strategy<Value = Fragment> {
    (
        0..4u8,
        0..8u16,
        0..8u16,
        proptest::collection::vec(any::<u8>(), 0..=FRAG_SIZE + 1),
    )
        .prop_map(|(id, index, total, chunk)| Fragment {
            frag_id: [id; 8],
            index,
            total,
            chunk,
        })
}

proptest! {
    /// Réassemblage correct pour un MTU tiré au hasard, fragments mélangés,
    /// certains dupliqués : le paquet sort exactement une fois, identique.
    #[test]
    fn reassemblage_mtu_et_ordre_aleatoires(
        p in proptest::collection::vec(any::<u8>(), 1..3000),
        mtu in mtu(),
        seed in any::<u64>(),
        dups in proptest::collection::vec(any::<prop::sample::Index>(), 0..5),
    ) {
        let mut frags = split_for_mtu(&p, mtu).unwrap();
        let cap = chunk_capacity(mtu).unwrap();
        prop_assert!(frags.iter().all(|f| f.chunk.len() <= cap));
        for d in &dups {
            let extra = frags[d.index(frags.len())].clone();
            frags.push(extra);
        }
        // Mélange déterministe (Fisher-Yates sur un LCG) : reproductible par seed.
        let mut s = seed;
        for i in (1..frags.len()).rev() {
            s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let j = usize::try_from(s >> 33).unwrap() % (i + 1);
            frags.swap(i, j);
        }
        let mut r = Reassembler::default();
        let mut out = Vec::new();
        for f in frags {
            if let Some(done) = r.push(f, T0).unwrap() {
                out.push(done);
            }
        }
        prop_assert_eq!(out, vec![p]);
        prop_assert!(r.is_empty());
    }

    /// Fragments manquants : rien ne sort, et tout est purgé au timeout.
    #[test]
    fn manquant_jamais_complete(
        p in proptest::collection::vec(any::<u8>(), 2..2000),
        mtu in mtu(),
        missing in any::<prop::sample::Index>(),
    ) {
        let mut frags = split_for_mtu(&p, mtu).unwrap();
        prop_assume!(frags.len() > 1);
        frags.remove(missing.index(frags.len()));
        let mut r = Reassembler::default();
        for f in frags {
            prop_assert_eq!(r.push(f, T0).unwrap(), None);
        }
        r.expire(T0 + TIMEOUT_MS + 1);
        prop_assert!(r.is_empty());
        prop_assert_eq!(r.buffered_bytes(), 0);
    }

    /// Entrées hostiles : jamais de panic, mémoire toujours bornée.
    #[test]
    fn memoire_bornee_face_a_un_pair_malveillant(
        frags in proptest::collection::vec(any_fragment(), 0..200),
        max_concurrent in 1..6usize,
        max_bytes in 0..4000usize,
    ) {
        let mut r = Reassembler::new(ReassemblerConfig {
            max_concurrent,
            max_bytes,
            ..ReassemblerConfig::default()
        });
        for (i, f) in frags.into_iter().enumerate() {
            let _ = r.push(f, T0 + u64::try_from(i).unwrap());
            prop_assert!(r.len() <= max_concurrent);
            prop_assert!(r.buffered_bytes() <= max_bytes);
        }
    }

    #[test]
    fn decodage_sans_panic(bytes in proptest::collection::vec(any::<u8>(), 0..600)) {
        if let Ok(f) = Fragment::decode(&bytes) {
            prop_assert_eq!(f.encode(), bytes);
        }
    }
}
