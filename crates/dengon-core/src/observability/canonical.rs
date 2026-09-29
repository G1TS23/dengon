//! Sérialisation JSON canonique (`contracts/events/CANONICAL.md`).
//!
//! Pas de dépendance à `serde_json` dans la crate (seulement en
//! dev-dependency, pour les vecteurs de `protocol`) : un `BTreeMap<String,
//! Value>` trie déjà les clés par ordre de point de code (l'ordre exigé,
//! ASCII étant le seul alphabet réellement exercé par les fixtures — voir
//! CANONICAL.md), ce qui évite la config `preserve_order` d'un `serde_json`
//! générique. Le sous-ensemble représenté (`Bool`/`Int`/`Str`/`Array`/
//! `Object`) élimine aussi par construction le piège `f64` du contrat :
//! aucun flottant n'est même représentable.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

/// Valeur JSON restreinte au sous-ensemble utilisé par le contrat
/// d'événements — voir la note de module.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// Sérialise en JSON canonique : clés triées (`BTreeMap`, déjà en
    /// ordre), séparateurs `,`/`:` sans espace, pas d'échappement
    /// non-ASCII, pas de retour à la ligne final — `CANONICAL.md` §1.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        self.write_canonical(&mut out);
        out.into_bytes()
    }

    fn write_canonical(&self, out: &mut String) {
        match self {
            Value::Bool(b) => {
                out.push_str(if *b { "true" } else { "false" });
            }
            Value::Int(i) => {
                // `write!` sur un `String` (`core::fmt::Write`) ne peut
                // échouer que si l'allocateur est épuisé — pas un cas
                // qu'on rattrape utilement ici (voir aussi `ledger`/
                // `store`, qui font le même choix pour des écritures
                // infaillibles en pratique).
                let _ = write!(out, "{i}");
            }
            Value::Str(s) => write_json_string(s, out),
            Value::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_canonical(out);
                }
                out.push(']');
            }
            Value::Object(map) => {
                out.push('{');
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json_string(k, out);
                    out.push(':');
                    v.write_canonical(out);
                }
                out.push('}');
            }
        }
    }
}

/// Échappe une chaîne selon JSON, sans passer les caractères non-ASCII en
/// `\uXXXX` (`CANONICAL.md` : « pas d'échappement non-ASCII »).
fn write_json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // `\b` et `\f` : mêmes échappements courts que `json.dumps`
            // (Python), sinon un payload qui en contient ne hacherait pas
            // pareil des deux côtés (US-309, signature du batch).
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Profondeur d'imbrication maximale acceptée par [`parse`] : le parseur est
/// récursif et tourne aussi sur l'ESP32, où la pile d'une tâche est petite.
/// Aucun payload du catalogue ne dépasse 3 niveaux.
pub const PARSE_MAX_DEPTH: usize = 16;

/// Erreur de [`parse`] : le texte n'est pas du JSON du sous-ensemble
/// [`Value`] (flottant, `null`, syntaxe invalide, trop profond…).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError;

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("JSON hors du sous-ensemble canonique")
    }
}

impl core::error::Error for ParseError {}

/// Relit un texte JSON en [`Value`] (US-309).
///
/// Sert à **re-canonicaliser** les payloads du journal avant de les signer
/// dans un batch : le journal stocke le payload en texte, et une partie est
/// écrite à la main côté C (`relay.boot`), sans garantie d'ordre des clés ni
/// d'espacement. Le dashboard, lui, recalcule `canonical_json` sur le JSON
/// qu'il reçoit : on doit signer exactement ces octets-là.
///
/// Même comportement que `json.loads` (Python) sur les points qui touchent
/// la signature : clé en double → la dernière gagne. Refuse ce que [`Value`]
/// ne sait pas représenter (flottants, exposants, `null`) plutôt que de
/// l'approcher.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let v = p.value(0)?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(ParseError);
    }
    Ok(v)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn next(&mut self) -> Result<u8, ParseError> {
        let b = self.peek().ok_or(ParseError)?;
        self.pos += 1;
        Ok(b)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, lit: &[u8]) -> Result<(), ParseError> {
        if self.bytes.get(self.pos..self.pos + lit.len()) == Some(lit) {
            self.pos += lit.len();
            Ok(())
        } else {
            Err(ParseError)
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > PARSE_MAX_DEPTH {
            return Err(ParseError);
        }
        self.skip_ws();
        match self.peek().ok_or(ParseError)? {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => self.string().map(Value::Str),
            b't' => self.expect(b"true").map(|()| Value::Bool(true)),
            b'f' => self.expect(b"false").map(|()| Value::Bool(false)),
            b'-' | b'0'..=b'9' => self.int(),
            _ => Err(ParseError),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.pos += 1; // '{'
        let mut map = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(ParseError);
            }
            let key = self.string()?;
            self.skip_ws();
            self.expect(b":")?;
            let v = self.value(depth + 1)?;
            map.insert(key, v);
            self.skip_ws();
            match self.next()? {
                b',' => {}
                b'}' => return Ok(Value::Object(map)),
                _ => return Err(ParseError),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.next()? {
                b',' => {}
                b']' => return Ok(Value::Array(items)),
                _ => return Err(ParseError),
            }
        }
    }

    /// Entier JSON (`-?(0|[1-9][0-9]*)`) tenant dans un `i64`. Une partie
    /// fractionnaire ou un exposant est refusé (pas de flottant au contrat).
    fn int(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let digits = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        let n = self.pos - digits;
        if n == 0 || (n > 1 && self.bytes[digits] == b'0') {
            return Err(ParseError);
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(ParseError);
        }
        let text = core::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| ParseError)?;
        text.parse::<i64>().map(Value::Int).map_err(|_| ParseError)
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let mut v = 0u32;
        for _ in 0..4 {
            let d = char::from(self.next()?).to_digit(16).ok_or(ParseError)?;
            v = (v << 4) | d;
        }
        Ok(v)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // '"'
        let mut out = String::new();
        loop {
            // Recopie d'un trait tout ce qui n'est ni fin ni échappement :
            // le texte d'entrée est déjà de l'UTF-8 valide (`&str`), et on ne
            // coupe qu'aux octets ASCII '"' / '\\', jamais au milieu d'un
            // caractère multi-octets.
            let run = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            let chunk = core::str::from_utf8(&self.bytes[run..self.pos]).map_err(|_| ParseError)?;
            out.push_str(chunk);
            match self.next()? {
                b'"' => return Ok(out),
                b'\\' => {}
                _ => return Err(ParseError), // caractère de contrôle brut
            }
            let c = match self.next()? {
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'b' => '\u{8}',
                b'f' => '\u{c}',
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'u' => {
                    let hi = self.hex4()?;
                    let code = if (0xD800..0xDC00).contains(&hi) {
                        self.expect(b"\\u")?;
                        let lo = self.hex4()?;
                        if !(0xDC00..0xE000).contains(&lo) {
                            return Err(ParseError);
                        }
                        0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                    } else {
                        hi
                    };
                    char::from_u32(code).ok_or(ParseError)?
                }
                _ => return Err(ParseError),
            };
            out.push(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn obj(pairs: Vec<(&str, Value)>) -> Value {
        let mut map = BTreeMap::new();
        for (k, v) in pairs {
            map.insert(String::from(k), v);
        }
        Value::Object(map)
    }

    #[test]
    fn les_cles_sortent_triees_meme_inserees_dans_le_desordre() {
        let v = obj(vec![
            ("z", Value::Int(1)),
            ("a", Value::Int(2)),
            ("m", Value::Int(3)),
        ]);
        assert_eq!(v.to_canonical_bytes(), b"{\"a\":2,\"m\":3,\"z\":1}");
    }

    #[test]
    fn pas_despace_apres_les_separateurs() {
        let v = obj(vec![(
            "x",
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
        )]);
        assert_eq!(v.to_canonical_bytes(), b"{\"x\":[1,2]}");
    }

    #[test]
    fn un_entier_negatif_ne_prend_pas_de_notation_exposant() {
        let v = obj(vec![("rssi", Value::Int(-63))]);
        assert_eq!(v.to_canonical_bytes(), b"{\"rssi\":-63}");
    }

    #[test]
    fn les_guillemets_et_antislash_sont_echappes() {
        let v = Value::Str(String::from("a\"b\\c"));
        assert_eq!(v.to_canonical_bytes(), b"\"a\\\"b\\\\c\"");
    }

    #[test]
    fn un_caractere_non_ascii_sort_en_utf8_litteral_pas_en_uxxxx() {
        // CANONICAL.md : « les caractères ≥ U+0080 sortent en UTF-8
        // littéral, pas en \uXXXX » — même si les fixtures actuelles ne
        // l'exercent pas, la règle vaut aussi pour du texte futur.
        let v = Value::Str(String::from("café"));
        assert_eq!(v.to_canonical_bytes(), "\"café\"".as_bytes());
    }

    #[test]
    fn b_et_f_sortent_comme_json_dumps() {
        // `json.dumps("\b\f\x01")` == `"\"\\b\\f\\u0001\""`.
        let v = Value::Str(String::from("\u{8}\u{c}\u{1}"));
        assert_eq!(v.to_canonical_bytes(), b"\"\\b\\f\\u0001\"");
    }

    // ----- parse (US-309) --------------------------------------------------

    fn recanon(text: &str) -> Vec<u8> {
        parse(text).expect("JSON valide").to_canonical_bytes()
    }

    #[test]
    fn un_payload_ecrit_a_la_main_est_recanonicalise() {
        // Ce que produit `dengon_relay_app.c` pour `relay.boot` : clés dans
        // l'ordre du `snprintf`, pas triées.
        let brut = r#"{ "fw_version": "0.1.0", "reset_reason": "power_on",
                        "secure_boot": false, "flash_enc": false }"#;
        assert_eq!(
            recanon(brut),
            br#"{"flash_enc":false,"fw_version":"0.1.0","reset_reason":"power_on","secure_boot":false}"#
        );
    }

    #[test]
    fn imbrication_entiers_negatifs_et_echappements() {
        assert_eq!(
            recanon(r#"{"b":[1,-2,{"z":0,"a":"x\"y\\\n\u00e9\ud83d\ude00"}],"a":true}"#),
            "{\"a\":true,\"b\":[1,-2,{\"a\":\"x\\\"y\\\\\\né😀\",\"z\":0}]}".as_bytes()
        );
    }

    #[test]
    fn cle_en_double_la_derniere_gagne_comme_json_loads() {
        assert_eq!(recanon(r#"{"a":1,"a":2}"#), br#"{"a":2}"#);
    }

    #[test]
    fn ce_que_value_ne_represente_pas_est_refuse() {
        for texte in [
            "1.5",
            "1e3",
            "null",
            "{\"a\":null}",
            "01",
            "-",
            "99999999999999999999",
            "\"\\ud800\"",
            "\"a\u{1}\"",
            "{\"a\":1,}",
            "[1 2]",
            "{} x",
            "",
        ] {
            assert_eq!(parse(texte), Err(ParseError), "accepté : {texte:?}");
        }
    }

    #[test]
    fn la_profondeur_est_bornee() {
        let ok = "[".repeat(PARSE_MAX_DEPTH + 1) + &"]".repeat(PARSE_MAX_DEPTH + 1);
        assert!(parse(&ok).is_ok());
        let trop = "[".repeat(PARSE_MAX_DEPTH + 2) + &"]".repeat(PARSE_MAX_DEPTH + 2);
        assert_eq!(parse(&trop), Err(ParseError));
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn value() -> impl Strategy<Value = Value> {
            let feuille = prop_oneof![
                any::<bool>().prop_map(Value::Bool),
                any::<i64>().prop_map(Value::Int),
                any::<String>().prop_map(Value::Str),
            ];
            feuille.prop_recursive(4, 32, 6, |inner| {
                prop_oneof![
                    prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                    prop::collection::btree_map(any::<String>(), inner, 0..6)
                        .prop_map(Value::Object),
                ]
            })
        }

        proptest! {
            /// parse ∘ sérialisation = identité : ce qu'on relit du journal
            /// redonne exactement la valeur, donc les mêmes octets signés.
            #[test]
            fn parse_relit_toute_sortie_canonique(v in value()) {
                let octets = v.to_canonical_bytes();
                let texte = core::str::from_utf8(&octets).unwrap();
                prop_assert_eq!(parse(texte), Ok(v));
            }

            /// Aucune entrée arbitraire ne fait paniquer le parseur.
            #[test]
            fn parse_ne_panique_jamais(s in any::<String>()) {
                let _ = parse(&s);
            }
        }
    }
}
