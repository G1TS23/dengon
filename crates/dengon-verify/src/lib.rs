//! `dengon-verify` — vérificateur de journal chaîné exporté (US-305).
//!
//! Binaire appelé **en sous-processus** par le dashboard Python (décisions
//! A-5 et B-5, `docs/synthese/09-dashboard-et-donnees.md` §2-3) : la règle de
//! vérification reste **une seule**, celle de `dengon_core::ledger`
//! ([`verify_entries`], [`verify_signatures`]), au lieu d'être réécrite en
//! Python.
//!
//! # Usage
//!
//! ```text
//! dengon-verify [--from-seq N --prev-hash HEX64] [--pubkey HEX64] [FICHIER | -]
//! ```
//!
//! - `FICHIER` : export binaire du journal, suite d'entrées au format
//!   `Entry::to_bytes` mises bout à bout. `-` ou absent : entrée standard.
//! - `--from-seq` / `--prev-hash` : ancre d'une **tranche** qui ne commence
//!   pas à 0 (la dernière entrée déjà vérifiée : `seq + 1`, `entry_hash`).
//!   Les deux ensemble ou aucun ; absents = début de journal.
//! - `--pubkey` : clé publique Ed25519 de signature du nœud ; si fournie, la
//!   signature de chaque entrée est vérifiée (une signature invalide rend
//!   `broken`). Sans elle, les signatures ne sont **pas** vérifiées, et la
//!   sortie le dit (`"signatures":"unchecked"`).
//!
//! # Sortie
//!
//! Une ligne JSON sur la sortie standard, par exemple :
//!
//! ```text
//! {"verdict":"ok","entries":12,"first_seq":0,"last_seq":11,"signatures":"verified"}
//! ```
//!
//! et un **code de sortie** propre à chaque verdict ([`exit_code`]) : `0` ok,
//! `1` broken, `2` fork, `3` gap. Les erreurs d'appel ou d'entrée utilisent
//! les codes `sysexits` `64`, `65`, `66`, avec un message sur la sortie
//! d'erreur et rien sur la sortie standard.

use std::fmt;
use std::io::Read;

pub use dengon_core::ledger::Verdict;
use dengon_core::ledger::{verify_entries, verify_signatures, Anchor, Entry, Hash, HASH_LEN};

use dengon_core::crypto::{VerifyingKey, PUBLIC_KEY_LEN};

/// Code de sortie : journal intègre.
pub const EXIT_OK: u8 = 0;
/// Code de sortie : entrée modifiée ou signature invalide.
pub const EXIT_BROKEN: u8 = 1;
/// Code de sortie : deux entrées pour la même position.
pub const EXIT_FORK: u8 = 2;
/// Code de sortie : position(s) manquante(s).
pub const EXIT_GAP: u8 = 3;
/// Code de sortie : appel incorrect (`EX_USAGE`).
pub const EXIT_USAGE: u8 = 64;
/// Code de sortie : export illisible (`EX_DATAERR`).
pub const EXIT_DATAERR: u8 = 65;
/// Code de sortie : fichier introuvable ou illisible (`EX_NOINPUT`).
pub const EXIT_NOINPUT: u8 = 66;

/// Code de sortie associé à un verdict.
#[must_use]
pub const fn exit_code(verdict: Verdict) -> u8 {
    match verdict {
        Verdict::Ok => EXIT_OK,
        Verdict::Broken => EXIT_BROKEN,
        Verdict::Fork => EXIT_FORK,
        Verdict::Gap => EXIT_GAP,
    }
}

/// Nom du verdict dans la sortie JSON.
#[must_use]
pub const fn verdict_name(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Ok => "ok",
        Verdict::Broken => "broken",
        Verdict::Fork => "fork",
        Verdict::Gap => "gap",
    }
}

/// Source de l'export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Entrée standard.
    Stdin,
    /// Chemin d'un fichier.
    File(String),
}

/// Arguments de la ligne de commande, validés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Ancre de la vérification.
    pub anchor: Anchor,
    /// Clé publique de signature, si les signatures doivent être vérifiées.
    pub pubkey: Option<[u8; PUBLIC_KEY_LEN]>,
    /// Source de l'export.
    pub input: Input,
}

/// Erreur d'appel ou de lecture ; porte son code de sortie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Arguments invalides.
    Usage(String),
    /// Fichier introuvable ou illisible.
    NoInput(String),
    /// Export illisible (entrée tronquée ou incohérente).
    Data(String),
}

impl Error {
    /// Code de sortie correspondant.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => EXIT_USAGE,
            Self::NoInput(_) => EXIT_NOINPUT,
            Self::Data(_) => EXIT_DATAERR,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(m) => write!(f, "usage : {m}\n{USAGE}"),
            Self::NoInput(m) | Self::Data(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

/// Aide affichée avec une erreur d'usage.
pub const USAGE: &str =
    "dengon-verify [--from-seq N --prev-hash HEX64] [--pubkey HEX64] [FICHIER | -]";

/// Analyse les arguments (sans le nom du programme).
///
/// # Errors
///
/// [`Error::Usage`] pour une option inconnue, une valeur manquante ou
/// invalide, `--from-seq` sans `--prev-hash` (ou l'inverse), ou plus d'un
/// fichier.
pub fn parse_args<I, S>(args: I) -> Result<Options, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut from_seq = None;
    let mut prev_hash = None;
    let mut pubkey = None;
    let mut input = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        let mut value = |name: &str| {
            args.next()
                .map(|v| v.as_ref().to_owned())
                .ok_or_else(|| Error::Usage(format!("valeur manquante pour {name}")))
        };
        match arg {
            "--from-seq" => {
                let v = value(arg)?;
                from_seq = Some(
                    v.parse::<u64>()
                        .map_err(|_| Error::Usage(format!("--from-seq invalide : {v}")))?,
                );
            }
            "--prev-hash" => prev_hash = Some(hex32(&value(arg)?, "--prev-hash")?),
            "--pubkey" => pubkey = Some(hex32(&value(arg)?, "--pubkey")?),
            "-" => set_input(&mut input, Input::Stdin)?,
            other if other.starts_with("--") => {
                return Err(Error::Usage(format!("option inconnue : {other}")));
            }
            path => set_input(&mut input, Input::File(path.to_owned()))?,
        }
    }
    let anchor = match (from_seq, prev_hash) {
        (None, None) => Anchor::GENESIS,
        (Some(first_seq), Some(prev_hash)) => Anchor {
            first_seq,
            prev_hash,
        },
        _ => {
            return Err(Error::Usage(
                "--from-seq et --prev-hash vont ensemble".to_owned(),
            ))
        }
    };
    Ok(Options {
        anchor,
        pubkey,
        input: input.unwrap_or(Input::Stdin),
    })
}

fn set_input(slot: &mut Option<Input>, input: Input) -> Result<(), Error> {
    if slot.is_some() {
        return Err(Error::Usage("un seul export à la fois".to_owned()));
    }
    *slot = Some(input);
    Ok(())
}

fn hex32(s: &str, name: &str) -> Result<Hash, Error> {
    let bad = || Error::Usage(format!("{name} : 64 caractères hexadécimaux attendus"));
    if s.len() != HASH_LEN * 2 || !s.is_ascii() {
        return Err(bad());
    }
    let mut out = [0; HASH_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|_| bad())?;
    }
    Ok(out)
}

/// Découpe un export en entrées.
///
/// # Errors
///
/// [`Error::Data`] si l'export est tronqué ou incohérent, avec la position
/// (octet) et le rang de l'entrée fautive.
pub fn parse_export(bytes: &[u8]) -> Result<Vec<Entry>, Error> {
    let mut entries = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let offset = bytes.len() - rest.len();
        let (entry, tail) = Entry::from_bytes(rest).ok_or_else(|| {
            Error::Data(format!(
                "export illisible : entrée n°{} à l'octet {offset}",
                entries.len()
            ))
        })?;
        entries.push(entry);
        rest = tail;
    }
    Ok(entries)
}

/// Résultat d'une vérification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Verdict.
    pub verdict: Verdict,
    /// Nombre d'entrées lues.
    pub entries: usize,
    /// `seq` de la première et de la dernière entrée lues.
    pub seq_range: Option<(u64, u64)>,
    /// `Some(true)` signatures vérifiées et valides, `Some(false)` au moins
    /// une invalide, `None` non vérifiées (pas de `--pubkey`, ou chaîne déjà
    /// rejetée).
    pub signatures: Option<bool>,
}

impl Report {
    /// La ligne JSON écrite sur la sortie standard.
    #[must_use]
    pub fn to_json(&self) -> String {
        let seqs = match self.seq_range {
            Some((first, last)) => format!("\"first_seq\":{first},\"last_seq\":{last}"),
            None => "\"first_seq\":null,\"last_seq\":null".to_owned(),
        };
        let signatures = match self.signatures {
            Some(true) => "verified",
            Some(false) => "invalid",
            None => "unchecked",
        };
        format!(
            "{{\"verdict\":\"{}\",\"entries\":{},{seqs},\"signatures\":\"{signatures}\"}}",
            verdict_name(self.verdict),
            self.entries,
        )
    }
}

/// Vérifie des entrées : structure et chaîne de hash d'abord, puis les
/// signatures si une clé est fournie et que la chaîne est intègre.
///
/// # Errors
///
/// [`Error::Usage`] si `pubkey` n'est pas une clé Ed25519 valide.
pub fn verify(
    entries: &[Entry],
    anchor: Anchor,
    pubkey: Option<&[u8; PUBLIC_KEY_LEN]>,
) -> Result<Report, Error> {
    let key = pubkey
        .map(|k| {
            VerifyingKey::from_bytes(k)
                .map_err(|_| Error::Usage("--pubkey : clé Ed25519 invalide".to_owned()))
        })
        .transpose()?;
    let mut verdict = verify_entries(entries, anchor);
    let mut signatures = None;
    if let (Verdict::Ok, Some(key)) = (verdict, key) {
        let valid = verify_signatures(entries, &key).is_none();
        if !valid {
            verdict = Verdict::Broken;
        }
        signatures = Some(valid);
    }
    Ok(Report {
        verdict,
        entries: entries.len(),
        seq_range: entries
            .first()
            .zip(entries.last())
            .map(|(a, b)| (a.seq, b.seq)),
        signatures,
    })
}

/// Exécute l'outil : lit l'export désigné par `args` (ou `stdin`) et rend
/// le rapport.
///
/// # Errors
///
/// Voir [`Error`] ; son [`Error::exit_code`] est le code de sortie à rendre.
pub fn run<I, S>(args: I, stdin: impl Read) -> Result<Report, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let options = parse_args(args)?;
    let bytes = match &options.input {
        Input::Stdin => {
            let mut buf = Vec::new();
            let mut stdin = stdin;
            stdin
                .read_to_end(&mut buf)
                .map_err(|e| Error::NoInput(format!("lecture de l'entrée standard : {e}")))?;
            buf
        }
        Input::File(path) => {
            std::fs::read(path).map_err(|e| Error::NoInput(format!("{path} : {e}")))?
        }
    };
    let entries = parse_export(&bytes)?;
    verify(&entries, options.anchor, options.pubkey.as_ref())
}

#[cfg(test)]
mod tests;
