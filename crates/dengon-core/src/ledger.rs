//! Journal chaîné append-only (US-206).
//!
//! Référence : `docs/synthese/04-architecture.md` §2 (« `ledger` : journal
//! append-only chaîné : `append(event) -> Entry`, `verify_chain()`,
//! `export(range)` — dépend de `crypto`, `sha2` »), `docs/synthese/03-etat-de-lart.md`
//! (« `hash_n = H(entry_n ‖ prev_hash)` »), `docs/powl/09-data-model.md` §1
//! (table `ledger` : `seq`, `ts_ms`, `event_name`, `payload_json`,
//! `prev_hash`, `entry_hash`, `sig`).
//!
//! # Signature : différée
//!
//! `crypto` (US-203, Ed25519) n'est pas encore livré à l'heure où ce module
//! est écrit — les deux US sont dans le même sprint, et la règle du projet
//! est qu'une US ne dépend jamais d'une autre US du même sprint. La
//! signature est donc **injectée** via le trait [`Signer`], pas câblée en
//! dur sur une implémentation Ed25519 : quand `crypto` arrivera, un
//! `Signer` réel le branchera ici sans changer la forme de `Ledger`. En
//! attendant, [`NullSigner`] produit une signature nulle, utilisée par les
//! tests. Écart consigné dans `docs/suivi/03-ecarts-conception.md`.
//!
//! # Persistance : hors périmètre de ce module
//!
//! `ledger` ne fait pas d'I/O (ni SQLite, ni littlefs) : il manipule une
//! séquence d'[`Entry`] en mémoire. Le critère d'acceptation « reprise après
//! redémarrage » est démontré ici par un aller-retour
//! [`Entry::to_bytes`]/[`Entry::from_bytes`] (sérialiser, tout détruire,
//! désérialiser, revérifier la chaîne) : la vraie écriture sur disque
//! (SQLite via `store`, US-207, ou littlefs côté firmware, US-308) est le
//! travail de la couche appelante, pas de `ledger` lui-même.

use alloc::string::String;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

/// Taille d'un hash SHA-256, en octets.
pub const HASH_LEN: usize = 32;
/// Taille d'une signature Ed25519, en octets (`docs/powl/09-data-model.md`).
pub const SIG_LEN: usize = 64;

/// Hash d'entrée de journal (SHA-256).
pub type Hash = [u8; HASH_LEN];
/// Signature d'une entrée de journal (Ed25519, ou nulle tant que `crypto`
/// n'est pas câblé — voir [`NullSigner`]).
pub type Signature = [u8; SIG_LEN];

/// `prev_hash` de la toute première entrée : pas d'entrée précédente, donc
/// tout à zéro plutôt qu'un `Option` — simplifie `Entry::compute_hash`, qui
/// n'a pas besoin de distinguer « première entrée » du reste.
pub const GENESIS_HASH: Hash = [0u8; HASH_LEN];

/// Signe une entrée de journal.
///
/// Injecté par l'appelant plutôt que câblé en dur (voir la note de module
/// sur `crypto`) : `ledger` ne connaît que la *forme* d'une signature
/// (64 octets), jamais l'algorithme qui la produit.
pub trait Signer {
    /// Signe `message` (toujours `entry_hash`, 32 octets) et renvoie la
    /// signature à stocker dans l'entrée.
    fn sign(&mut self, message: &[u8]) -> Signature;
}

/// Signeur nul : renvoie toujours 64 octets à zéro.
///
/// Utilisé tant qu'aucune vraie clé Ed25519 n'existe (voir la note de
/// module). Ne JAMAIS utiliser hors tests / bouchon — une entrée signée par
/// `NullSigner` ne prouve rien.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSigner;

impl Signer for NullSigner {
    fn sign(&mut self, _message: &[u8]) -> Signature {
        [0u8; SIG_LEN]
    }
}

/// Une entrée du journal chaîné.
///
/// `entry_hash = SHA-256(seq ‖ ts_ms ‖ len(event_name) ‖ event_name ‖
/// len(payload_json) ‖ payload_json ‖ prev_hash)` — les deux champs de
/// longueur variable sont préfixés par leur taille (`u32` big-endian) pour
/// qu'une concaténation ne soit jamais ambiguë entre deux découpages
/// différents (ex. `event_name="ab", payload="c"` ne doit pas hacher pareil
/// que `event_name="a", payload="bc"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub seq: u64,
    pub ts_ms: u64,
    pub event_name: String,
    pub payload_json: String,
    pub prev_hash: Hash,
    pub entry_hash: Hash,
    pub sig: Signature,
}

impl Entry {
    fn signing_bytes(
        seq: u64,
        ts_ms: u64,
        event_name: &str,
        payload_json: &str,
        prev_hash: &Hash,
    ) -> Vec<u8> {
        let mut buf =
            Vec::with_capacity(8 + 8 + 4 + event_name.len() + 4 + payload_json.len() + HASH_LEN);
        buf.extend_from_slice(&seq.to_be_bytes());
        buf.extend_from_slice(&ts_ms.to_be_bytes());
        // `try_from` + saturation plutôt qu'un cast brut (retour de revue
        // #63, `cast_possible_truncation` activé workspace-wide avec
        // `protocol`) : un `event_name`/`payload_json` de plus de 4 Go n'a
        // aucun sens pratique ici, saturer à `u32::MAX` évite un panic sans
        // prétendre gérer un cas qui ne se produira jamais.
        let event_name_len = u32::try_from(event_name.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&event_name_len.to_be_bytes());
        buf.extend_from_slice(event_name.as_bytes());
        let payload_json_len = u32::try_from(payload_json.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&payload_json_len.to_be_bytes());
        buf.extend_from_slice(payload_json.as_bytes());
        buf.extend_from_slice(prev_hash);
        buf
    }

    fn compute_hash(
        seq: u64,
        ts_ms: u64,
        event_name: &str,
        payload_json: &str,
        prev_hash: &Hash,
    ) -> Hash {
        let bytes = Self::signing_bytes(seq, ts_ms, event_name, payload_json, prev_hash);
        let digest = Sha256::digest(&bytes);
        let mut out = [0u8; HASH_LEN];
        out.copy_from_slice(&digest);
        out
    }

    /// Sérialise l'entrée en un format binaire simple (longueurs préfixées
    /// en `u32` big-endian pour les deux champs texte, champs fixes bruts
    /// pour le reste). Utilisé par le test de reprise après redémarrage —
    /// voir la note de module sur la persistance.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            8 + 8
                + 4
                + self.event_name.len()
                + 4
                + self.payload_json.len()
                + HASH_LEN * 2
                + SIG_LEN,
        );
        buf.extend_from_slice(&self.seq.to_be_bytes());
        buf.extend_from_slice(&self.ts_ms.to_be_bytes());
        let event_name_len = u32::try_from(self.event_name.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&event_name_len.to_be_bytes());
        buf.extend_from_slice(self.event_name.as_bytes());
        let payload_json_len = u32::try_from(self.payload_json.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&payload_json_len.to_be_bytes());
        buf.extend_from_slice(self.payload_json.as_bytes());
        buf.extend_from_slice(&self.prev_hash);
        buf.extend_from_slice(&self.entry_hash);
        buf.extend_from_slice(&self.sig);
        buf
    }

    /// Désérialise une entrée depuis le format produit par [`Entry::to_bytes`].
    ///
    /// Renvoie `None` sur un buffer tronqué ou incohérent (longueur annoncée
    /// dépassant ce qui reste) plutôt que de paniquer : un journal relu
    /// depuis un stockage physique peut être partiellement corrompu, ce
    /// n'est pas un bug appelant.
    pub fn from_bytes(buf: &[u8]) -> Option<(Self, &[u8])> {
        let mut cursor = buf;
        let seq = u64::from_be_bytes(take(&mut cursor, 8)?.try_into().ok()?);
        let ts_ms = u64::from_be_bytes(take(&mut cursor, 8)?.try_into().ok()?);
        let event_name = take_string(&mut cursor)?;
        let payload_json = take_string(&mut cursor)?;
        let prev_hash: Hash = take(&mut cursor, HASH_LEN)?.try_into().ok()?;
        let entry_hash: Hash = take(&mut cursor, HASH_LEN)?.try_into().ok()?;
        let sig: Signature = take(&mut cursor, SIG_LEN)?.try_into().ok()?;
        Some((
            Entry {
                seq,
                ts_ms,
                event_name,
                payload_json,
                prev_hash,
                entry_hash,
                sig,
            },
            cursor,
        ))
    }
}

fn take<'a>(cursor: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    if cursor.len() < n {
        return None;
    }
    let (head, tail) = cursor.split_at(n);
    *cursor = tail;
    Some(head)
}

fn take_string(cursor: &mut &[u8]) -> Option<String> {
    let len_bytes = take(cursor, 4)?;
    let len = u32::from_be_bytes(len_bytes.try_into().ok()?) as usize;
    let bytes = take(cursor, len)?;
    String::from_utf8(bytes.to_vec()).ok()
}

/// Verdict rendu par [`Ledger::verify_chain`].
///
/// Mêmes variantes que celles déjà déclarées dans `dengon-verify` (US-104) :
/// ce type les remplace, pour que `dengon-verify::main` réutilise
/// littéralement `ledger::verify_chain` comme le prévoit
/// `docs/synthese/04-architecture.md` §2, plutôt que de dupliquer l'énumération.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Chaîne cohérente et hashes valides (la vérification de signature
    /// suit le même sort que `Signer` : différée tant que `crypto` n'existe
    /// pas — voir la note de module).
    Ok,
    /// Une entrée a été modifiée (le hash recalculé ne correspond plus).
    Broken,
    /// Deux entrées revendiquent la même position (`seq`).
    Fork,
    /// Une ou plusieurs positions manquent dans la chaîne.
    Gap,
}

/// Journal chaîné append-only.
///
/// Paramétré par `S: Signer` plutôt que par une clé concrète : voir la note
/// de module sur la dépendance différée à `crypto`.
#[derive(Debug, Clone)]
pub struct Ledger<S: Signer> {
    entries: Vec<Entry>,
    signer: S,
}

impl<S: Signer> Ledger<S> {
    /// Journal vide, prêt à recevoir sa première entrée (`seq = 0`).
    pub fn new(signer: S) -> Self {
        Self {
            entries: Vec::new(),
            signer,
        }
    }

    /// Reconstruit un journal à partir d'entrées déjà persistées (relecture
    /// après redémarrage). N'appelle PAS `verify_chain` toute seule :
    /// l'appelant décide quand vérifier (voir le test de reprise).
    pub fn from_entries(entries: Vec<Entry>, signer: S) -> Self {
        Self { entries, signer }
    }

    /// Nombre d'entrées dans le journal.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` si le journal est vide.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Toutes les entrées, dans l'ordre d'ajout.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn last_hash(&self) -> Hash {
        self.entries.last().map_or(GENESIS_HASH, |e| e.entry_hash)
    }

    /// Ajoute une entrée au journal et renvoie une référence dessus.
    ///
    /// `seq` est calculé automatiquement (dernière `seq` + 1, ou 0 pour la
    /// première entrée) : l'appelant ne peut pas se tromper de numéro.
    pub fn append(&mut self, event_name: &str, payload_json: &str, ts_ms: u64) -> &Entry {
        // `saturating_add` plutôt que `+ 1` (retour de revue #75,
        // OswinFreyr) : `e.seq + 1` panique en debug/test et reboucle
        // silencieusement à 0 en release quand `seq == u64::MAX` — un cas
        // physiquement inatteignable par un usage légitime (2⁶⁴ appends),
        // mais `append()` est infaillible par signature (retourne `&Entry`,
        // pas `Result`), donc saturer est le choix le moins mauvais : au
        // pire, deux entrées consécutives partageraient `seq = u64::MAX`,
        // ce que `verify_chain()` détecte déjà comme `Fork`.
        let seq = self.entries.last().map_or(0, |e| e.seq.saturating_add(1));
        let prev_hash = self.last_hash();
        let entry_hash = Entry::compute_hash(seq, ts_ms, event_name, payload_json, &prev_hash);
        let sig = self.signer.sign(&entry_hash);
        self.entries.push(Entry {
            seq,
            ts_ms,
            event_name: String::from(event_name),
            payload_json: String::from(payload_json),
            prev_hash,
            entry_hash,
            sig,
        });
        // Indexation plutôt que `.last().expect(...)` : on vient de pousser,
        // `len() - 1` ne peut pas être hors bornes, et `unwrap`/`expect` sont
        // interdits hors tests par `[workspace.lints.clippy]`.
        let idx = self.entries.len() - 1;
        &self.entries[idx]
    }

    /// Vérifie la cohérence de la chaîne : hashes, absence de trou, absence
    /// de position dupliquée. Équivaut à [`verify_entries`] ancré sur
    /// [`Anchor::GENESIS`].
    ///
    /// Ne vérifie PAS la signature : voir [`verify_signatures`] (US-305).
    pub fn verify_chain(&self) -> Verdict {
        verify_entries(&self.entries, Anchor::GENESIS)
    }

    /// Les entrées dont `seq` tombe dans `range`, dans l'ordre du journal.
    ///
    /// # Re-vérifier un export qui ne commence pas à 0
    ///
    /// `verify_chain()` suppose une chaîne démarrant à `seq = 0` avec
    /// `prev_hash == GENESIS_HASH` (retour de revue #75, OswinFreyr). Pour une
    /// tranche (`export(3..6)`), utiliser [`verify_entries`] avec
    /// [`Anchor::after`] la dernière entrée vérifiée auparavant (US-305).
    pub fn export(&self, range: core::ops::Range<u64>) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| range.contains(&e.seq))
            .collect()
    }
}

/// Point de départ d'une vérification : la `seq` que doit porter la première
/// entrée, et le `prev_hash` qu'elle doit citer (US-305).
///
/// [`Anchor::GENESIS`] pour un journal complet. Pour une **tranche** exportée
/// (`export(3..6)`, ou un batch reçu par le dashboard), l'ancre est la
/// dernière entrée déjà vérifiée : `first_seq = seq + 1`, `prev_hash =
/// entry_hash`. C'est l'API qui manquait pour re-vérifier un export ne
/// commençant pas à 0 (limite notée sur [`Ledger::export`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    /// `seq` attendue de la première entrée.
    pub first_seq: u64,
    /// `prev_hash` attendu de la première entrée.
    pub prev_hash: Hash,
}

impl Anchor {
    /// Début d'un journal : `seq = 0`, `prev_hash` à zéro.
    pub const GENESIS: Self = Self {
        first_seq: 0,
        prev_hash: GENESIS_HASH,
    };

    /// Ancre qui prolonge `entry` : la tranche suivante commence juste après.
    #[must_use]
    pub fn after(entry: &Entry) -> Self {
        Self {
            first_seq: entry.seq.saturating_add(1),
            prev_hash: entry.entry_hash,
        }
    }
}

/// Vérifie une suite d'entrées à partir de `anchor` : positions (`Fork`,
/// `Gap`) puis chaîne de hash (`Broken`). Une suite vide est `Ok`.
///
/// Ne vérifie PAS les signatures : voir [`verify_signatures`].
#[must_use]
pub fn verify_entries(entries: &[Entry], anchor: Anchor) -> Verdict {
    // Deux passes, volontairement séparées :
    //
    // 1. Positions (`seq`) : un doublon est un fork, un trou dans 0..=max
    //    en est un. Vérifié sur l'ENSEMBLE des `seq`, pas seulement
    //    contre l'entrée immédiatement précédente — un premier essai qui
    //    ne comparait qu'à la précédente classait à tort `[0, 1, 2, 1]`
    //    (rejeu d'une ancienne entrée en fin de chaîne — un vrai cas de
    //    fork) comme `Gap`, parce que la comparaison ne portait que sur
    //    l'entrée d'avant (`2`), jamais revue par rapport aux `seq` déjà
    //    vues plus tôt (`1`, trouvé en relecture de revue). Rien n'était
    //    accepté à tort dans ce cas (le chaînage de hash ci-dessous
    //    aurait fini par détecter une incohérence de toute façon), mais
    //    le verdict précis était faux.
    //
    // 2. Chaîne de hash, dans l'ORDRE DE STOCKAGE (`self.entries`), qui
    //    doit correspondre à l'ordre de production (`append` empile
    //    dans cet ordre) : une entrée déplacée ou rejouée à la mauvaise
    //    position casse ce chaînage même si sa `seq` est par ailleurs
    //    valide.
    let mut seen = alloc::collections::BTreeSet::new();
    let mut max_seq: Option<u64> = None;
    for entry in entries {
        // Une `seq` antérieure à l'ancre revendique une position déjà
        // couverte par la partie vérifiée auparavant : historique
        // concurrent, donc fork (US-305).
        if entry.seq < anchor.first_seq || !seen.insert(entry.seq) {
            return Verdict::Fork;
        }
        max_seq = Some(max_seq.map_or(entry.seq, |m| core::cmp::max(m, entry.seq)));
    }
    if let Some(max) = max_seq {
        // Arithmétique en `u128` plutôt que `max + 1` en `u64` (retour
        // de revue #75, OswinFreyr) : un export corrompu ou hostile
        // portant `seq = u64::MAX` faisait paniquer ce calcul en
        // debug/test et le faisait reboucler silencieusement à 0 en
        // release — un vérificateur qui panique ou ment sur une entrée
        // hostile est exactement ce que `verify_chain()` doit éviter.
        if u128::from(seen.len() as u64) != u128::from(max) - u128::from(anchor.first_seq) + 1 {
            return Verdict::Gap;
        }
    }

    let mut expected_prev = anchor.prev_hash;
    for entry in entries {
        let recomputed = Entry::compute_hash(
            entry.seq,
            entry.ts_ms,
            &entry.event_name,
            &entry.payload_json,
            &entry.prev_hash,
        );
        if recomputed != entry.entry_hash || entry.prev_hash != expected_prev {
            return Verdict::Broken;
        }
        expected_prev = entry.entry_hash;
    }

    Verdict::Ok
}

/// Vérifie la signature de chaque entrée (`Ed25519(entry_hash)`) avec la clé
/// publique de signature du nœud (US-305). `Some(seq)` = première entrée dont
/// la signature est invalide, `None` si toutes sont valides.
///
/// À appeler **après** [`verify_entries`] : une signature valide sur un
/// `entry_hash` qui ne correspond pas au contenu ne prouve rien.
#[must_use]
pub fn verify_signatures(entries: &[Entry], key: &crate::crypto::VerifyingKey) -> Option<u64> {
    entries
        .iter()
        .find(|e| key.verify(&e.entry_hash, &e.sig).is_err())
        .map(|e| e.seq)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use proptest::prelude::*;

    fn ledger() -> Ledger<NullSigner> {
        Ledger::new(NullSigner)
    }

    #[test]
    fn une_tranche_ancree_se_verifie() {
        let mut l = ledger();
        for i in 0..6 {
            l.append("pkt.relayed", "{}", i);
        }
        let tranche: Vec<Entry> = l.export(3..6).into_iter().cloned().collect();
        // Sans ancre : la tranche paraît trouée / cassée.
        assert_ne!(verify_entries(&tranche, Anchor::GENESIS), Verdict::Ok);
        // Ancrée sur l'entrée 2 : intègre.
        let ancre = Anchor::after(&l.entries()[2]);
        assert_eq!(ancre.first_seq, 3);
        assert_eq!(verify_entries(&tranche, ancre), Verdict::Ok);
        // Ancre trop tôt (après l'entrée 1) : la position 2 manque.
        assert_eq!(
            verify_entries(&tranche, Anchor::after(&l.entries()[1])),
            Verdict::Gap
        );
        // Ancre trop tard (après l'entrée 3) : la tranche réécrit la position 3.
        assert_eq!(
            verify_entries(&tranche, Anchor::after(&l.entries()[3])),
            Verdict::Fork
        );
        // Bonne position, mauvais `prev_hash` : chaîne cassée.
        let fausse = Anchor {
            first_seq: 3,
            prev_hash: l.entries()[1].entry_hash,
        };
        assert_eq!(verify_entries(&tranche, fausse), Verdict::Broken);
        // Tranche commençant après l'ancre : trou.
        assert_eq!(verify_entries(&tranche[1..], ancre), Verdict::Gap);
        assert_eq!(verify_entries(&[], ancre), Verdict::Ok);
    }

    #[test]
    fn signatures_verifiees_avec_la_cle_du_noeud() {
        use crate::crypto::SigningKey;

        struct Ed25519(SigningKey);
        impl Signer for Ed25519 {
            fn sign(&mut self, message: &[u8]) -> Signature {
                self.0.sign(message)
            }
        }

        let cle = SigningKey::from_seed(&[7; 32]);
        let publique = cle.verifying_key();
        let mut l = Ledger::new(Ed25519(cle));
        for i in 0..3 {
            l.append("msg.queued", "{}", i);
        }
        assert_eq!(verify_signatures(l.entries(), &publique), None);

        // Clé d'un autre nœud : première entrée rejetée.
        let autre = SigningKey::from_seed(&[8; 32]).verifying_key();
        assert_eq!(verify_signatures(l.entries(), &autre), Some(0));

        // Signature altérée sur l'entrée 1.
        let mut entries = l.entries().to_vec();
        entries[1].sig[0] ^= 1;
        assert_eq!(verify_signatures(&entries, &publique), Some(1));
    }

    #[test]
    fn un_journal_vide_est_valide() {
        assert_eq!(ledger().verify_chain(), Verdict::Ok);
    }

    #[test]
    fn append_puis_verify_chain_est_toujours_ok() {
        let mut l = ledger();
        for i in 0..5 {
            l.append("msg.queued", &format!("{{\"i\":{i}}}"), 1_000 + i);
        }
        assert_eq!(l.len(), 5);
        assert_eq!(l.verify_chain(), Verdict::Ok);
    }

    #[test]
    fn les_seq_sont_bien_consecutives_depuis_zero() {
        let mut l = ledger();
        l.append("a", "{}", 1);
        l.append("b", "{}", 2);
        l.append("c", "{}", 3);
        let seqs: Vec<u64> = l.entries().iter().map(|e| e.seq).collect();
        assert_eq!(seqs, alloc::vec![0, 1, 2]);
    }

    #[test]
    fn un_trou_est_detecte() {
        let mut l = ledger();
        l.append("a", "{}", 1);
        l.append("b", "{}", 2);
        l.append("c", "{}", 3);
        // Retire l'entrée du milieu : seq 0, 2 — un trou en position 1.
        let mut entries = l.entries().to_vec();
        entries.remove(1);
        let l2 = Ledger::from_entries(entries, NullSigner);
        assert_eq!(l2.verify_chain(), Verdict::Gap);
    }

    #[test]
    fn une_entree_dupliquee_est_un_fork() {
        let mut l = ledger();
        l.append("a", "{}", 1);
        l.append("b", "{}", 2);
        let mut entries = l.entries().to_vec();
        let doublon = entries[0].clone(); // même seq (0) que la première
        entries.insert(1, doublon);
        let l2 = Ledger::from_entries(entries, NullSigner);
        assert_eq!(l2.verify_chain(), Verdict::Fork);
    }

    #[test]
    fn un_doublon_non_adjacent_est_bien_un_fork() {
        // Rejoue d'une entrée déjà vue, mais pas juste après l'original :
        // seq = [0, 1, 2, 1]. Avant la réécriture en deux passes de
        // `verify_chain`, seule la comparaison au seq immédiatement
        // précédent était faite, donc ce cas était classé à tort `Gap`
        // (le seq max était 2 sans jamais avoir vu de doublon adjacent).
        let mut l = ledger();
        l.append("a", "{}", 1);
        l.append("b", "{}", 2);
        l.append("c", "{}", 3);
        let mut entries = l.entries().to_vec();
        let doublon = entries[1].clone(); // seq = 1, déjà présent
        entries.push(doublon);
        let l2 = Ledger::from_entries(entries, NullSigner);
        assert_eq!(l2.verify_chain(), Verdict::Fork);
    }

    #[test]
    fn seq_u64_max_ne_panique_pas_et_est_detecte() {
        // Retour de revue #75 (OswinFreyr) : un export corrompu ou hostile
        // portant `seq = u64::MAX` faisait paniquer `verify_chain()` en
        // debug/test (`max + 1` déborde) et rebouclait silencieusement à 0
        // en release. Une seule entrée à `seq = u64::MAX` doit être
        // détectée comme `Gap` (il manque les positions 0..u64::MAX-1),
        // sans jamais paniquer.
        let entry_hash = Entry::compute_hash(u64::MAX, 1, "a", "{}", &GENESIS_HASH);
        let entry = Entry {
            seq: u64::MAX,
            ts_ms: 1,
            event_name: String::from("a"),
            payload_json: String::from("{}"),
            prev_hash: GENESIS_HASH,
            entry_hash,
            sig: [0u8; SIG_LEN],
        };
        let l = Ledger::from_entries(alloc::vec![entry], NullSigner);
        assert_eq!(l.verify_chain(), Verdict::Gap);
    }

    #[test]
    fn une_entree_modifiee_casse_la_chaine() {
        let mut l = ledger();
        l.append("a", "{}", 1);
        l.append("b", "{}", 2);
        let mut entries = l.entries().to_vec();
        entries[0].payload_json = String::from("{\"modifie\":true}"); // hash plus recalculé
        let l2 = Ledger::from_entries(entries, NullSigner);
        assert_eq!(l2.verify_chain(), Verdict::Broken);
    }

    #[test]
    fn prev_hash_incoherent_casse_la_chaine() {
        let mut l = ledger();
        l.append("a", "{}", 1);
        l.append("b", "{}", 2);
        let mut entries = l.entries().to_vec();
        // entry_hash recalculé à partir d'un prev_hash différent de celui
        // vraiment attendu (mais cohérent avec lui-même) : détecté par la
        // comparaison à `expected_prev`, pas par le recalcul de hash.
        entries[1].prev_hash = [0xAA; HASH_LEN];
        entries[1].entry_hash = Entry::compute_hash(
            entries[1].seq,
            entries[1].ts_ms,
            &entries[1].event_name,
            &entries[1].payload_json,
            &entries[1].prev_hash,
        );
        let l2 = Ledger::from_entries(entries, NullSigner);
        assert_eq!(l2.verify_chain(), Verdict::Broken);
    }

    #[test]
    fn export_filtre_par_plage_de_seq() {
        let mut l = ledger();
        for i in 0..10 {
            l.append("e", "{}", i);
        }
        let sub = l.export(3..6);
        let seqs: Vec<u64> = sub.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, alloc::vec![3, 4, 5]);
    }

    #[test]
    fn reprise_apres_redemarrage_par_serialisation() {
        // Simule un arrêt brutal puis une relance : les entrées sont
        // sérialisées (ce que ferait un vrai stockage), le journal en
        // mémoire est détruit, puis reconstruit depuis les octets — la
        // chaîne doit rester vérifiable (critère d'acceptation US-206).
        let mut l = ledger();
        for i in 0..7 {
            l.append("msg.queued", &format!("{{\"i\":{i}}}"), 2_000 + i);
        }
        assert_eq!(l.verify_chain(), Verdict::Ok);

        let mut bytes = Vec::new();
        for entry in l.entries() {
            bytes.extend_from_slice(&entry.to_bytes());
        }
        drop(l); // « arrêt brutal » : le journal en mémoire disparaît

        let mut restored = Vec::new();
        let mut cursor: &[u8] = &bytes;
        while !cursor.is_empty() {
            let (entry, rest) = Entry::from_bytes(cursor).expect("entrée bien formée");
            restored.push(entry);
            cursor = rest;
        }

        let l2 = Ledger::from_entries(restored, NullSigner);
        assert_eq!(l2.len(), 7);
        assert_eq!(l2.verify_chain(), Verdict::Ok);
    }

    #[test]
    fn from_bytes_sur_un_buffer_tronque_renvoie_none() {
        let mut l = ledger();
        l.append("a", "{}", 1);
        let bytes = l.entries()[0].to_bytes();
        // Tronque à mi-chemin : ne doit jamais paniquer.
        assert!(Entry::from_bytes(&bytes[..bytes.len() / 2]).is_none());
    }

    proptest! {
        /// Toute séquence d'ajouts (noms/payloads/timestamps arbitraires)
        /// produit un journal dont la chaîne est vérifiable — c'est
        /// `append` qui construit la chaîne, il ne peut pas se tromper sur
        /// ses propres entrées.
        #[test]
        fn toute_sequence_d_appends_reste_verifiable(
            noms in prop::collection::vec("[a-z]{1,10}\\.[a-z_]{1,10}", 0..30),
            ts_base in 0u64..1_000_000,
        ) {
            let mut l = ledger();
            for (i, nom) in noms.iter().enumerate() {
                l.append(nom, "{}", ts_base + i as u64);
            }
            prop_assert_eq!(l.verify_chain(), Verdict::Ok);
            prop_assert_eq!(l.len(), noms.len());
        }

        /// Corrompre l'octet du payload d'une entrée quelconque (parmi
        /// celles qui existent) doit systématiquement être détecté comme
        /// `Broken` — jamais silencieusement accepté comme `Ok`.
        #[test]
        fn corrompre_une_entree_est_toujours_detecte(
            n in 1usize..10,
            index in 0usize..9,
        ) {
            let index = index % n;
            let mut l = ledger();
            for i in 0..n {
                l.append("msg.queued", &format!("{{\"i\":{i}}}"), i as u64);
            }
            let mut entries = l.entries().to_vec();
            entries[index].payload_json.push('!');
            let l2 = Ledger::from_entries(entries, NullSigner);
            prop_assert_eq!(l2.verify_chain(), Verdict::Broken);
        }
    }
}
