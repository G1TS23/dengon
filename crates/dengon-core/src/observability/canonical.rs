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
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
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
}
