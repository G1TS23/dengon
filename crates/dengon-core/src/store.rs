//! Persistance locale du nœud, SQLite (US-207).
//!
//! Schéma conforme à `docs/synthese/09-dashboard-et-donnees.md` §11.1.
//! Colonnes sensibles chiffrées **XChaCha20-Poly1305 champ par champ**
//! (décision B-3) : `identity.priv_static`/`priv_sign`, `messages.body`,
//! `noise_sessions.state`.
//!
//! # `std` seulement
//!
//! Contrairement à `protocol`/`sync`/`ledger`, ce module n'a pas vocation à
//! compiler en `no_std` : `rusqlite` vendorise sqlite3 en C, indisponible
//! sur cible ESP32. `docs/synthese/04-architecture.md` §2 est explicite :
//! « `store` est derrière un trait `Store` (impl `rusqlite` natif, impl
//! NVS/flash sur ESP32) » — l'implémentation ESP32 sera un module séparé,
//! pas celui-ci. D'où `#[cfg(feature = "std")]` sur `pub mod store;` dans
//! `lib.rs`, symétrique à la bascule `no_std` du reste de la crate.
//!
//! # Clé de chiffrement : différée derrière un trait `KeySource`
//!
//! `identity` (US-205) — qui génère et garde la vraie clé de chiffrement des
//! champs sensibles, idéalement dans le coffre de la plateforme
//! (Keystore/Keychain) — est dans le **même sprint** que `store` (US-207).
//! Même choix que `ledger::Signer` (US-206, voir
//! `docs/suivi/03-ecarts-conception.md`) : la clé est **injectée** via le
//! trait [`KeySource`], pas dérivée en dur ici. [`FixedKeySource`] est un
//! bouchon à clé fixe pour les tests — **jamais** à utiliser hors tests, une
//! clé fixe et connue de tous ne protège rien en pratique.

use std::path::Path;

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rusqlite::{params, Connection};

/// Taille de la clé de chiffrement des champs sensibles (256 bits).
pub const FIELD_KEY_LEN: usize = 32;
/// Taille du nonce XChaCha20 (192 bits — assez grand pour un nonce aléatoire
/// à chaque chiffrement sans risque de collision pratique).
const NONCE_LEN: usize = 24;

/// Fournit la clé de chiffrement des colonnes sensibles.
///
/// Voir la note de module : différé tant qu'`identity` (US-205) n'existe
/// pas, même sprint que `store`.
pub trait KeySource {
    fn field_key(&self) -> [u8; FIELD_KEY_LEN];
}

/// Clé fixe, pour les tests uniquement — voir la note de module.
#[derive(Debug, Clone, Copy)]
pub struct FixedKeySource(pub [u8; FIELD_KEY_LEN]);

impl KeySource for FixedKeySource {
    fn field_key(&self) -> [u8; FIELD_KEY_LEN] {
        self.0
    }
}

/// Erreurs de `store`.
#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    /// Le chiffrement d'un champ a échoué (ne devrait arriver que si la clé
    /// est mal formée — l'AEAD elle-même ne peut pas échouer sur un
    /// chiffrement).
    Encryption,
    /// Le déchiffrement a échoué : mauvaise clé, donnée tronquée, ou donnée
    /// corrompue/modifiée (l'AEAD rejette explicitement une donnée altérée,
    /// c'est la garantie d'authenticité de Poly1305).
    Decryption,
    /// Texte non-UTF8 après déchiffrement (ne devrait pas arriver pour un
    /// champ qu'on a nous-mêmes chiffré, mais un champ lu depuis une base
    /// corrompue ne doit pas faire paniquer l'appelant).
    InvalidUtf8,
}

impl core::fmt::Display for StoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "erreur SQLite : {e}"),
            StoreError::Encryption => write!(f, "échec du chiffrement d'un champ"),
            StoreError::Decryption => write!(f, "échec du déchiffrement d'un champ"),
            StoreError::InvalidUtf8 => write!(f, "champ déchiffré non-UTF8"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

/// Chiffre `plaintext` avec un nonce aléatoire, préfixé au résultat.
///
/// Format stocké : `nonce(24 o) ‖ ciphertext_avec_tag_poly1305`. Le nonce
/// n'a pas besoin d'être secret, seulement unique par clé — le stocker en
/// clair à côté du texte chiffré est le fonctionnement normal d'un AEAD.
///
/// `aad` (« additional authenticated data ») lie le texte chiffré à sa
/// ligne/colonne d'origine (ex. `msg_uuid` pour `messages.body`, `peer_id`
/// pour `noise_sessions.state`) : sans ça, un attaquant qui peut écrire
/// directement dans le fichier `.db` (device compromis, sync malveillante)
/// pourrait copier le blob chiffré d'une ligne vers une autre — par exemple
/// remplacer le corps d'un message par celui, chiffré, d'un autre message —
/// et le déchiffrement réussirait quand même, puisque l'AEAD n'authentifie
/// alors que le texte chiffré lui-même, pas la ligne à laquelle il est
/// censé appartenir. Avec l'AAD, un tel remplacement change le contexte
/// authentifié et fait échouer le déchiffrement (voir
/// `un_champ_dechiffre_avec_un_mauvais_contexte_echoue`).
fn encrypt_field(
    key: &[u8; FIELD_KEY_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, StoreError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| StoreError::Encryption)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Déchiffre un champ produit par [`encrypt_field`]. `aad` doit être
/// exactement celui utilisé au chiffrement (même contexte de ligne/colonne).
fn decrypt_field(
    key: &[u8; FIELD_KEY_LEN],
    aad: &[u8],
    stored: &[u8],
) -> Result<Vec<u8>, StoreError> {
    if stored.len() < NONCE_LEN {
        return Err(StoreError::Decryption);
    }
    let (nonce_bytes, ciphertext) = stored.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = XNonce::from_slice(nonce_bytes);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| StoreError::Decryption)
}

type Migration = (i64, &'static str, &'static [&'static str]);

/// Migrations versionnées et rejouables (critère d'acceptation US-207).
///
/// Même discipline que `dashboard/api/app/migrations.py` (US-110, déjà
/// revue) : une liste `(version, nom, [instructions])`, `CREATE ... IF NOT
/// EXISTS` partout, une seule transaction par migration, jamais rejouée une
/// fois enregistrée dans `schema_migrations`. Schéma repris tel quel de
/// `docs/synthese/09-dashboard-et-donnees.md` §11.1.
const MIGRATIONS: &[Migration] = &[(
    1,
    "initial",
    &[
        "CREATE TABLE IF NOT EXISTS identity (
            id          INTEGER PRIMARY KEY CHECK (id = 1),
            peer_id     BLOB NOT NULL,
            priv_static BLOB NOT NULL,
            priv_sign   BLOB NOT NULL,
            pub_static  BLOB NOT NULL,
            pub_sign    BLOB NOT NULL,
            pseudo      TEXT NOT NULL,
            created_ms  INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS contacts (
            peer_id        BLOB PRIMARY KEY,
            pub_static     BLOB NOT NULL,
            pub_sign       BLOB NOT NULL,
            pseudo         TEXT,
            verified_at    INTEGER,
            first_seen_ms  INTEGER NOT NULL,
            last_seen_ms   INTEGER,
            key_changed_at INTEGER,
            blocked        INTEGER NOT NULL DEFAULT 0
        )",
        "CREATE TABLE IF NOT EXISTS conversations (
            conv_id      BLOB PRIMARY KEY,
            peer_id      BLOB NOT NULL REFERENCES contacts(peer_id),
            last_msg_ms  INTEGER,
            unread_count INTEGER NOT NULL DEFAULT 0
        )",
        "CREATE TABLE IF NOT EXISTS messages (
            msg_uuid       BLOB PRIMARY KEY,
            conv_id        BLOB NOT NULL REFERENCES conversations(conv_id),
            direction      TEXT NOT NULL CHECK (direction IN ('out','in')),
            author_peer_id BLOB NOT NULL,
            conv_seq       INTEGER NOT NULL,
            body           BLOB NOT NULL,
            sent_ms        INTEGER NOT NULL,
            received_ms    INTEGER,
            status         TEXT NOT NULL DEFAULT 'queued'
                           CHECK (status IN ('queued','in_flight','delivered','read','expired','cancelled')),
            status_ms      INTEGER NOT NULL,
            read_ms        INTEGER
        )",
        "CREATE INDEX IF NOT EXISTS idx_msg_conv ON messages(conv_id, sent_ms)",
        "CREATE TABLE IF NOT EXISTS outbox (
            msg_uuid      BLOB NOT NULL REFERENCES messages(msg_uuid),
            dest_peer_id  BLOB NOT NULL,
            packet        BLOB NOT NULL,
            kind          TEXT NOT NULL CHECK (kind IN ('session','envelope')),
            attempts      INTEGER NOT NULL DEFAULT 0,
            first_sent_ms INTEGER,
            last_sent_ms  INTEGER,
            expires_ms    INTEGER NOT NULL,
            PRIMARY KEY (msg_uuid, dest_peer_id)
        )",
        "CREATE TABLE IF NOT EXISTS held_envelopes (
            msg_log_id    BLOB PRIMARY KEY,
            recipient_tag BLOB NOT NULL,
            epoch_day     INTEGER NOT NULL,
            packet        BLOB NOT NULL,
            copy_budget   INTEGER NOT NULL,
            deposit_ms    INTEGER NOT NULL,
            expires_ms    INTEGER NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS idx_env_tag ON held_envelopes(recipient_tag)",
        "CREATE TABLE IF NOT EXISTS seen_set (
            msg_id  BLOB PRIMARY KEY,
            seen_ms INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS recon_cache (
            msg_id    BLOB PRIMARY KEY,
            packet    BLOB NOT NULL,
            cached_ms INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS noise_sessions (
            peer_id        BLOB PRIMARY KEY,
            state          BLOB NOT NULL,
            established_ms INTEGER NOT NULL,
            tx_count       INTEGER NOT NULL DEFAULT 0
        )",
        "CREATE TABLE IF NOT EXISTS ledger (
            seq          INTEGER PRIMARY KEY,
            ts_ms        INTEGER NOT NULL,
            event_name   TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            prev_hash    BLOB NOT NULL,
            entry_hash   BLOB NOT NULL,
            sig          BLOB NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS ship_cursor (
            id               INTEGER PRIMARY KEY CHECK (id = 1),
            last_shipped_seq INTEGER NOT NULL DEFAULT -1
        )",
    ],
)];

/// Connexion à la base locale du nœud, avec chiffrement champ par champ des
/// colonnes sensibles.
pub struct Store<K: KeySource> {
    conn: Connection,
    keys: K,
}

// `rusqlite::Connection` n'implémente pas `Debug` : implémentation manuelle
// qui ne montre ni la connexion (pas de représentation utile) ni `keys`
// (une clé de chiffrement ne doit jamais atterrir dans un log de debug).
impl<K: KeySource> core::fmt::Debug for Store<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl<K: KeySource> Store<K> {
    /// Ouvre (ou crée) la base au chemin donné et applique les migrations
    /// manquantes.
    pub fn open<P: AsRef<Path>>(path: P, keys: K) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        Self::from_connection(conn, keys)
    }

    /// Base en mémoire, pour les tests — pas de fichier, donc pas de test
    /// négatif possible dessus (voir `tests::le_corps_du_message_nest_jamais_en_clair_sur_disque`,
    /// qui utilise volontairement [`Store::open`] sur un vrai fichier).
    pub fn open_in_memory(keys: K) -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn, keys)
    }

    fn from_connection(conn: Connection, keys: K) -> Result<Self, StoreError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self { conn, keys };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&mut self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version    INTEGER PRIMARY KEY,
                name       TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )?;

        let applied: Vec<i64> = {
            let mut stmt = self.conn.prepare("SELECT version FROM schema_migrations")?;
            let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
            rows.collect::<Result<_, _>>()?
        };

        for (version, name, statements) in MIGRATIONS {
            if applied.contains(version) {
                continue;
            }
            let tx = self.conn.transaction()?;
            for statement in *statements {
                tx.execute_batch(statement)?;
            }
            tx.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                params![version, name],
            )?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Enregistre l'identité du nœud (une seule ligne, `id = 1`).
    ///
    /// `priv_static`/`priv_sign` sont chiffrées avant stockage — ce sont les
    /// clés privées X25519/Ed25519 du nœud (`docs/synthese/09` §11.1).
    #[allow(clippy::too_many_arguments)] // reflète 1:1 les colonnes de la table `identity`
    pub fn set_identity(
        &self,
        peer_id: &[u8],
        priv_static: &[u8],
        priv_sign: &[u8],
        pub_static: &[u8],
        pub_sign: &[u8],
        pseudo: &str,
        created_ms: i64,
    ) -> Result<(), StoreError> {
        let key = self.keys.field_key();
        let priv_static_enc = encrypt_field(&key, b"identity.priv_static", priv_static)?;
        let priv_sign_enc = encrypt_field(&key, b"identity.priv_sign", priv_sign)?;
        self.conn.execute(
            "INSERT INTO identity (id, peer_id, priv_static, priv_sign, pub_static, pub_sign, pseudo, created_ms)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                peer_id = excluded.peer_id, priv_static = excluded.priv_static,
                priv_sign = excluded.priv_sign, pub_static = excluded.pub_static,
                pub_sign = excluded.pub_sign, pseudo = excluded.pseudo,
                created_ms = excluded.created_ms",
            params![peer_id, priv_static_enc, priv_sign_enc, pub_static, pub_sign, pseudo, created_ms],
        )?;
        Ok(())
    }

    /// Relit l'identité et déchiffre les clés privées. `(priv_static, priv_sign)`.
    pub fn get_identity_private_keys(&self) -> Result<(Vec<u8>, Vec<u8>), StoreError> {
        let (priv_static_enc, priv_sign_enc): (Vec<u8>, Vec<u8>) = self.conn.query_row(
            "SELECT priv_static, priv_sign FROM identity WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let key = self.keys.field_key();
        let priv_static = decrypt_field(&key, b"identity.priv_static", &priv_static_enc)?;
        let priv_sign = decrypt_field(&key, b"identity.priv_sign", &priv_sign_enc)?;
        Ok((priv_static, priv_sign))
    }

    /// Ajoute (ou remplace) un contact. Champs publics, pas de chiffrement.
    pub fn upsert_contact(
        &self,
        peer_id: &[u8],
        pub_static: &[u8],
        pub_sign: &[u8],
        pseudo: Option<&str>,
        first_seen_ms: i64,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO contacts (peer_id, pub_static, pub_sign, pseudo, first_seen_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(peer_id) DO UPDATE SET
                pub_static = excluded.pub_static, pub_sign = excluded.pub_sign,
                pseudo = excluded.pseudo",
            params![peer_id, pub_static, pub_sign, pseudo, first_seen_ms],
        )?;
        Ok(())
    }

    /// Crée une conversation avec un contact.
    pub fn insert_conversation(&self, conv_id: &[u8], peer_id: &[u8]) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO conversations (conv_id, peer_id) VALUES (?1, ?2)",
            params![conv_id, peer_id],
        )?;
        Ok(())
    }

    /// Insère un message. `body` est le texte **en clair** : il est chiffré
    /// ici avant stockage (décision B-3, critère d'acceptation US-207).
    #[allow(clippy::too_many_arguments)]
    pub fn insert_message(
        &self,
        msg_uuid: &[u8],
        conv_id: &[u8],
        direction: &str,
        author_peer_id: &[u8],
        conv_seq: i64,
        body: &str,
        sent_ms: i64,
        status: &str,
        status_ms: i64,
    ) -> Result<(), StoreError> {
        let body_enc = encrypt_field(&self.keys.field_key(), msg_uuid, body.as_bytes())?;
        self.conn.execute(
            "INSERT INTO messages
                (msg_uuid, conv_id, direction, author_peer_id, conv_seq, body, sent_ms, status, status_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                msg_uuid, conv_id, direction, author_peer_id, conv_seq, body_enc, sent_ms, status,
                status_ms
            ],
        )?;
        Ok(())
    }

    /// Relit et déchiffre le corps d'un message.
    pub fn get_message_body(&self, msg_uuid: &[u8]) -> Result<String, StoreError> {
        let body_enc: Vec<u8> = self.conn.query_row(
            "SELECT body FROM messages WHERE msg_uuid = ?1",
            params![msg_uuid],
            |row| row.get(0),
        )?;
        let plain = decrypt_field(&self.keys.field_key(), msg_uuid, &body_enc)?;
        String::from_utf8(plain).map_err(|_| StoreError::InvalidUtf8)
    }

    /// Enregistre (ou remplace) l'état sérialisé d'une session Noise établie
    /// avec `peer_id`. `state` (le blob `snow`) est chiffré avant stockage.
    pub fn set_noise_session(
        &self,
        peer_id: &[u8],
        state: &[u8],
        established_ms: i64,
    ) -> Result<(), StoreError> {
        let state_enc = encrypt_field(&self.keys.field_key(), peer_id, state)?;
        self.conn.execute(
            "INSERT INTO noise_sessions (peer_id, state, established_ms)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(peer_id) DO UPDATE SET
                state = excluded.state, established_ms = excluded.established_ms",
            params![peer_id, state_enc, established_ms],
        )?;
        Ok(())
    }

    /// Relit et déchiffre l'état d'une session Noise.
    pub fn get_noise_session_state(&self, peer_id: &[u8]) -> Result<Vec<u8>, StoreError> {
        let state_enc: Vec<u8> = self.conn.query_row(
            "SELECT state FROM noise_sessions WHERE peer_id = ?1",
            params![peer_id],
            |row| row.get(0),
        )?;
        decrypt_field(&self.keys.field_key(), peer_id, &state_enc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> FixedKeySource {
        FixedKeySource([0x42; FIELD_KEY_LEN])
    }

    fn seed_conversation(store: &Store<FixedKeySource>, peer_id: &[u8], conv_id: &[u8]) {
        store
            .upsert_contact(peer_id, b"pub_static", b"pub_sign", Some("alice"), 1_000)
            .expect("upsert_contact");
        store
            .insert_conversation(conv_id, peer_id)
            .expect("insert_conversation");
    }

    #[test]
    fn les_migrations_sont_rejouables_sans_erreur() {
        let mut store = Store::open_in_memory(keys()).expect("premiere ouverture");
        // Rejoue explicitement : ne doit rien casser (IF NOT EXISTS partout,
        // et la version 1 est déjà dans schema_migrations donc re-sautée).
        store.run_migrations().expect("deuxieme passage");
    }

    #[test]
    fn round_trip_identite_dechiffre_les_bonnes_cles() {
        let store = Store::open_in_memory(keys()).expect("open");
        store
            .set_identity(
                b"peerid8o",
                b"cle privee X25519 secrete",
                b"cle privee Ed25519 secrete",
                b"pub_static",
                b"pub_sign",
                "alice",
                1_000,
            )
            .expect("set_identity");

        let (priv_static, priv_sign) = store
            .get_identity_private_keys()
            .expect("get_identity_private_keys");
        assert_eq!(priv_static, b"cle privee X25519 secrete");
        assert_eq!(priv_sign, b"cle privee Ed25519 secrete");
    }

    #[test]
    fn round_trip_message_dechiffre_le_bon_corps() {
        let store = Store::open_in_memory(keys()).expect("open");
        seed_conversation(&store, b"peerpeer", b"convconv");
        store
            .insert_message(
                b"msguuid16bytes!!",
                b"convconv",
                "out",
                b"peerpeer",
                0,
                "salut, ceci est un message secret",
                1_000,
                "queued",
                1_000,
            )
            .expect("insert_message");

        let body = store
            .get_message_body(b"msguuid16bytes!!")
            .expect("get_message_body");
        assert_eq!(body, "salut, ceci est un message secret");
    }

    #[test]
    fn round_trip_session_noise_dechiffre_le_bon_etat() {
        let store = Store::open_in_memory(keys()).expect("open");
        store
            .set_noise_session(b"peerpeer", b"etat serialise par snow", 1_000)
            .expect("set_noise_session");
        let state = store
            .get_noise_session_state(b"peerpeer")
            .expect("get_noise_session_state");
        assert_eq!(state, b"etat serialise par snow");
    }

    #[test]
    fn deux_chiffrements_du_meme_texte_produisent_des_octets_differents() {
        // Nonce aléatoire à chaque appel : même clé, même texte, mais deux
        // sorties différentes — condition nécessaire pour qu'un attaquant ne
        // puisse pas repérer un message répété rien qu'en comparant les
        // octets chiffrés.
        let key = [0x11; FIELD_KEY_LEN];
        let a = encrypt_field(&key, b"ctx", b"meme texte").expect("chiffrement 1");
        let b = encrypt_field(&key, b"ctx", b"meme texte").expect("chiffrement 2");
        assert_ne!(a, b);
        assert_eq!(
            decrypt_field(&key, b"ctx", &a).expect("dechiffrement 1"),
            b"meme texte"
        );
        assert_eq!(
            decrypt_field(&key, b"ctx", &b).expect("dechiffrement 2"),
            b"meme texte"
        );
    }

    #[test]
    fn dechiffrer_avec_la_mauvaise_cle_echoue() {
        let bonne_cle = [0x11; FIELD_KEY_LEN];
        let mauvaise_cle = [0x22; FIELD_KEY_LEN];
        let chiffre = encrypt_field(&bonne_cle, b"ctx", b"secret").expect("chiffrement");
        assert!(matches!(
            decrypt_field(&mauvaise_cle, b"ctx", &chiffre),
            Err(StoreError::Decryption)
        ));
    }

    #[test]
    fn dechiffrer_une_donnee_modifiee_echoue() {
        // Garantie d'authenticité de l'AEAD (le tag Poly1305) : modifier un
        // seul octet du texte chiffré doit faire échouer le déchiffrement,
        // pas renvoyer un texte corrompu silencieusement.
        let key = [0x11; FIELD_KEY_LEN];
        let mut chiffre = encrypt_field(&key, b"ctx", b"secret").expect("chiffrement");
        let last = chiffre.len() - 1;
        chiffre[last] ^= 0xFF;
        assert!(matches!(
            decrypt_field(&key, b"ctx", &chiffre),
            Err(StoreError::Decryption)
        ));
    }

    #[test]
    fn dechiffrer_un_buffer_tronque_echoue_sans_paniquer() {
        let key = [0x11; FIELD_KEY_LEN];
        assert!(matches!(
            decrypt_field(&key, b"ctx", &[0u8; 4]),
            Err(StoreError::Decryption)
        ));
    }

    #[test]
    fn un_champ_dechiffre_avec_un_mauvais_contexte_echoue() {
        // C'est la protection apportée par l'AAD : un texte chiffré produit
        // pour une ligne/colonne donnée (ex. le message A) ne se déchiffre
        // plus correctement si on le présente comme appartenant à un autre
        // contexte (ex. le message B) — même clé, même texte chiffré, seul
        // le contexte authentifié change. Empêche un attaquant à écriture
        // sur le fichier `.db` de copier le blob chiffré d'une ligne vers
        // une autre sans que ça se voie au déchiffrement.
        let key = [0x11; FIELD_KEY_LEN];
        let chiffre_pour_msg_a = encrypt_field(&key, b"msg-a", b"secret").expect("chiffrement");
        assert!(matches!(
            decrypt_field(&key, b"msg-b", &chiffre_pour_msg_a),
            Err(StoreError::Decryption)
        ));
        // Avec le bon contexte, ça déchiffre toujours normalement.
        assert_eq!(
            decrypt_field(&key, b"msg-a", &chiffre_pour_msg_a).expect("dechiffrement"),
            b"secret"
        );
    }

    /// Critère d'acceptation US-207 : « après avoir écrit un message connu,
    /// un `grep` binaire sur le fichier `.db` ne le retrouve pas en clair ».
    #[test]
    fn le_corps_du_message_nest_jamais_en_clair_sur_disque() {
        let path = std::env::temp_dir().join(format!(
            "dengon-store-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("horloge")
                .as_nanos()
        ));

        const SECRET: &str = "ceci-est-un-message-tres-identifiable-XYZ123";

        {
            let store = Store::open(&path, keys()).expect("open sur fichier");
            seed_conversation(&store, b"peerpeer", b"convconv");
            store
                .insert_message(
                    b"msguuid16bytes!!",
                    b"convconv",
                    "out",
                    b"peerpeer",
                    0,
                    SECRET,
                    1_000,
                    "queued",
                    1_000,
                )
                .expect("insert_message");
        } // `store` droppé : la connexion SQLite est fermée, tout doit être sur disque.

        let raw = std::fs::read(&path).expect("lecture du fichier .db");
        let needle = SECRET.as_bytes();
        let trouve_en_clair = raw.windows(needle.len()).any(|w| w == needle);

        std::fs::remove_file(&path).ok();

        assert!(
            !trouve_en_clair,
            "le message en clair a été retrouvé tel quel dans le fichier .db"
        );
    }
}
