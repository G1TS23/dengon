//! Constantes du protocole dengon.
//!
//! Transcription de [`docs/synthese/05-protocole-et-trame.md`] §2 (lui-même
//! résumé de `docs/powl/03` §2). Une valeur qui change ici est un changement de
//! contrat : revue à trois.

// --- Version ------------------------------------------------------------

/// Version courante du protocole (octet 0 de chaque paquet L3).
pub const PROTO_VERSION: u8 = 1;

// --- BLE GATT (C-2) ---------------------------------------------------

/// UUID du service GATT `dengon` : `6d656e67-2d64-656e-676f-6e2d76310000`.
pub const SERVICE_UUID: [u8; 16] = [
    0x6d, 0x65, 0x6e, 0x67, 0x2d, 0x64, 0x65, 0x6e, 0x67, 0x6f, 0x6e, 0x2d, 0x76, 0x31, 0x00, 0x00,
];
/// Caractéristique `RX` (write-without-response, pair → nœud). Dernier mot `0001`.
pub const CHAR_RX_UUID: [u8; 16] = [
    0x6d, 0x65, 0x6e, 0x67, 0x2d, 0x64, 0x65, 0x6e, 0x67, 0x6f, 0x6e, 0x2d, 0x76, 0x31, 0x00, 0x01,
];
/// Caractéristique `TX` (notify, nœud → pair). Dernier mot `0002`.
pub const CHAR_TX_UUID: [u8; 16] = [
    0x6d, 0x65, 0x6e, 0x67, 0x2d, 0x64, 0x65, 0x6e, 0x67, 0x6f, 0x6e, 0x2d, 0x76, 0x31, 0x00, 0x02,
];
/// MTU ATT à négocier à la connexion ; repli sur [`ATT_MTU_MIN`] si refus.
pub const ATT_MTU_PREFERRED: u16 = 517;
/// MTU ATT minimal garanti par BLE (→ 20 octets utiles).
pub const ATT_MTU_MIN: u16 = 23;

// --- Routage (synthese/05 §2, §6) -----------------------------------

/// Sauts au départ d'un paquet.
pub const TTL_DEFAULT: u8 = 7;
/// Plafond de TTL appliqué si le nœud a au moins [`DENSE_LINKS`] voisins.
pub const TTL_CLAMP_DENSE: u8 = 5;
/// Seuil de densité (nb de voisins) au-delà duquel on applique le clamp.
pub const DENSE_LINKS: u8 = 6;
/// Seuil « chaîne fine » : en dessous, un relais rediffuse à profondeur pleine.
pub const THIN_LINKS: u8 = 2;
/// Fenêtre du délai aléatoire avant relais (« écouter avant de rediffuser »), en ms.
pub const RELAY_JITTER_MS: core::ops::RangeInclusive<u16> = 10..=220;
/// Capacité du seen-set (déduplication), en entrées LRU.
pub const SEEN_SET_CAP: usize = 1024;
/// Expiration d'une entrée du seen-set, en secondes.
pub const SEEN_TTL_S: u32 = 300;
/// Anti-inondation : nouveaux `msgID` acceptés par minute et par voisin (A-13).
pub const FLOOD_MAX_PER_MIN_PEER: u16 = 20;

// --- Fragmentation L2 (synthese/05 §5) ----------------------------

/// Octets de payload par fragment.
pub const FRAG_SIZE: usize = 440;
/// Délai d'inactivité avant abandon d'un réassemblage, en secondes.
pub const FRAG_TIMEOUT_S: u32 = 30;
/// Nombre maximal de réassemblages simultanés (éviction du plus ancien au-delà).
pub const FRAG_MAX_CONCURRENT: usize = 64;

// --- Cycle de vie (synthese/05 §2, §6.5) --------------------------

/// Durée de vie applicative d'un message, en secondes (24 h).
pub const MSG_TTL_S: u32 = 86_400;
/// Taille maximale d'une enveloppe scellée (texte court), en octets.
pub const ENVELOPE_MAX_BYTES: usize = 4096;
/// Nombre maximal d'enveloppes stockées — cap ESP32-WROOM sans PSRAM (A-4 / C-1 ;
/// 512 sur WROVER, cf. `docs/synthese/08` §5).
pub const ENVELOPE_STORE_MAX: usize = 64;

// --- Budget de copies d'une enveloppe — **cible v2** (A-13) -------

/// Budget de copies initial d'une enveloppe (Spray-and-Wait, v2).
pub const COPY_BUDGET_INIT: u8 = 4;
/// Budget de copies maximal d'une enveloppe (v2).
pub const COPY_BUDGET_MAX: u8 = 8;

// --- Chiffrement / anti-analyse de trafic ------------------------

/// Tailles cibles d'un paquet chiffré (padding PKCS#7 vers le bucket supérieur).
pub const PAD_BUCKETS: [usize; 4] = [256, 512, 1024, 2048];

// --- ANNOUNCE (synthese/05 §2) ---------------------------------

/// Période d'ANNOUNCE quand le nœud est isolé, en secondes.
pub const ANNOUNCE_ISOLATED_S: u32 = 4;
/// Période d'ANNOUNCE quand le nœud est connecté (avec jitter), en secondes.
pub const ANNOUNCE_CONNECTED_S: core::ops::RangeInclusive<u32> = 15..=30;

// --- Anti-rejeu (synthese/05 §6.4, C-7) -----------------------

/// Tolérance sur le `timestamp_ms` d'un paquet, en millisecondes (±2 h) ;
/// au-delà, le paquet est rejeté.
pub const TIMESTAMP_TOLERANCE_MS: u64 = 2 * 60 * 60 * 1000;

// --- Tailles de champ de l'en-tête L3 (synthese/05 §3) -------

/// Longueur d'un `peerID` = `SHA-256(pub_static)[0..8]` (A-8).
pub const PEER_ID_LEN: usize = 8;
/// Longueur d'un `msgID` = `SHA-256(sender_id ‖ ts ‖ type ‖ payload)` (A-9).
pub const MSG_ID_LEN: usize = 32;
/// Longueur d'une signature Ed25519 en fin de paquet.
pub const SIGNATURE_LEN: usize = 64;
/// Longueur d'un `recipient_tag` (enveloppes scellées, `docs/synthese/06`).
pub const RECIPIENT_TAG_LEN: usize = 16;
/// Longueur de l'en-tête L3 d'un paquet **broadcast** (sans `recipient_id`).
pub const HEADER_LEN_BROADCAST: usize = 22;
/// Longueur de l'en-tête L3 d'un paquet **adressé** (avec `recipient_id`).
pub const HEADER_LEN_ADDRESSED: usize = 30;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valeurs_de_reference() {
        assert_eq!(PROTO_VERSION, 1);
        assert_eq!(TTL_DEFAULT, 7);
        assert_eq!(TTL_CLAMP_DENSE, 5);
        assert_eq!(MSG_TTL_S, 24 * 60 * 60);
        assert_eq!(FLOOD_MAX_PER_MIN_PEER, 20);
        assert_eq!(PAD_BUCKETS, [256, 512, 1024, 2048]);
        assert_eq!(TIMESTAMP_TOLERANCE_MS, 7_200_000);
    }

    #[test]
    fn tailles_den_tete_coherentes() {
        // broadcast = version+type+ttl+flags(4) + timestamp(8) + sender(8) + payload_len(2)
        assert_eq!(HEADER_LEN_BROADCAST, 4 + 8 + PEER_ID_LEN + 2);
        // adressé = broadcast + recipient_id(8)
        assert_eq!(HEADER_LEN_ADDRESSED, HEADER_LEN_BROADCAST + PEER_ID_LEN);
    }

    #[test]
    fn uuids_ble() {
        // RX / TX ne diffèrent du service que par le dernier octet.
        assert_eq!(&CHAR_RX_UUID[..15], &SERVICE_UUID[..15]);
        assert_eq!(&CHAR_TX_UUID[..15], &SERVICE_UUID[..15]);
        assert_eq!(CHAR_RX_UUID[15], 0x01);
        assert_eq!(CHAR_TX_UUID[15], 0x02);
        assert_eq!(SERVICE_UUID[15], 0x00);
    }

    #[test]
    fn les_bornes_de_plage_sont_dans_le_bon_sens() {
        assert!(RELAY_JITTER_MS.start() < RELAY_JITTER_MS.end());
        assert!(ANNOUNCE_CONNECTED_S.start() < ANNOUNCE_CONNECTED_S.end());
    }
}
