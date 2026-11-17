//! Valeurs manipulées à l'exécution.
//!
//! Les nombres, booléens, chaînes et variantes d'énumération sont copiés ; les tableaux, maps
//! et structures sont **partagés** (DECISIONS D30) : `_b = _a;` désigne la même structure.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

/// Structure déclarée : son nom et ses champs dans l'ordre de déclaration.
pub(crate) struct StructInfo {
    pub name: String,
    pub fields: Vec<String>,
}

pub(crate) struct EnumInfo {
    pub name: String,
    pub variants: Vec<String>,
}

/// Noms nécessaires pour afficher une valeur.
pub(crate) struct TypeNames {
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
}

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Rc<str>),
    Enum {
        id: usize,
        variant: usize,
    },
    Array(Rc<RefCell<Vec<Value>>>),
    /// Paires clé → valeur dans l'ordre d'insertion.
    Map(Rc<RefCell<Vec<(Value, Value)>>>),
    Struct(Rc<RefCell<StructValue>>),
    /// Résultat d'un appel de fonction `Void`.
    Void,
}

#[derive(Debug)]
pub struct StructValue {
    pub id: usize,
    pub fields: Vec<Value>,
}

impl Value {
    pub fn new_array(items: Vec<Value>) -> Value {
        Value::Array(Rc::new(RefCell::new(items)))
    }

    pub fn new_map() -> Value {
        Value::Map(Rc::new(RefCell::new(Vec::new())))
    }

    /// Adresse du contenu partagé, pour détecter les cycles.
    fn shared_ptr(&self) -> Option<usize> {
        match self {
            Value::Array(a) => Some(Rc::as_ptr(a) as *const u8 as usize),
            Value::Map(m) => Some(Rc::as_ptr(m) as *const u8 as usize),
            Value::Struct(s) => Some(Rc::as_ptr(s) as *const u8 as usize),
            _ => None,
        }
    }
}

/// Égalité `==` : structurelle, avec promotion `Int`/`Float` (DECISIONS D23).
/// Une paire déjà en cours de comparaison est supposée égale : les cycles terminent.
pub fn equal(a: &Value, b: &Value) -> bool {
    equal_in(a, b, &mut Vec::new())
}

fn equal_in(a: &Value, b: &Value, assumed: &mut Vec<(usize, usize)>) -> bool {
    if let (Some(pa), Some(pb)) = (a.shared_ptr(), b.shared_ptr()) {
        if pa == pb || assumed.contains(&(pa, pb)) {
            return true;
        }
        assumed.push((pa, pb));
    }
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => (*x as f64) == *y,
        (Value::Float(x), Value::Int(y)) => *x == (*y as f64),
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (
            Value::Enum { id, variant },
            Value::Enum {
                id: id2,
                variant: v2,
            },
        ) => id == id2 && variant == v2,
        (Value::Array(x), Value::Array(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(i, j)| equal_in(i, j, assumed))
        }
        (Value::Map(x), Value::Map(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.iter()
                        .find(|(k2, _)| equal_in(k, k2, assumed))
                        .is_some_and(|(_, v2)| equal_in(v, v2, assumed))
                })
        }
        (Value::Struct(x), Value::Struct(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.id == y.id
                && x.fields
                    .iter()
                    .zip(y.fields.iter())
                    .all(|(i, j)| equal_in(i, j, assumed))
        }
        (Value::Void, Value::Void) => true,
        _ => false,
    }
}

/// Texte affiché par `print` et produit par la concaténation : une chaîne seule s'affiche
/// telle quelle, une chaîne dans une collection entre guillemets.
pub(crate) fn display(v: &Value, names: &TypeNames) -> String {
    let mut out = String::new();
    write_value(v, names, false, &mut out, &mut Vec::new());
    out
}

fn write_value(
    v: &Value,
    names: &TypeNames,
    quoted: bool,
    out: &mut String,
    open: &mut Vec<usize>,
) {
    if let Some(p) = v.shared_ptr() {
        if open.contains(&p) {
            out.push('…');
            return;
        }
        open.push(p);
    }
    match v {
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::Float(f) => out.push_str(&format_float(*f)),
        Value::Bool(b) => {
            let _ = write!(out, "{b}");
        }
        Value::Str(s) if quoted => {
            let _ = write!(out, "{:?}", &**s);
        }
        Value::Str(s) => out.push_str(s),
        Value::Enum { id, variant } => {
            let e = &names.enums[*id];
            let _ = write!(out, "{}.{}", e.name, e.variants[*variant]);
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.borrow().iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(item, names, true, out, open);
            }
            out.push(']');
        }
        Value::Map(entries) => {
            out.push('{');
            for (i, (k, val)) in entries.borrow().iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(k, names, true, out, open);
                out.push_str(": ");
                write_value(val, names, true, out, open);
            }
            out.push('}');
        }
        Value::Struct(s) => {
            let s = s.borrow();
            let info = &names.structs[s.id];
            let _ = write!(out, "{} {{ ", info.name);
            for (i, (name, val)) in info.fields.iter().zip(&s.fields).enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let _ = write!(out, "{name}: ");
                write_value(val, names, true, out, open);
            }
            out.push_str(" }");
        }
        Value::Void => out.push_str("Void"),
    }
    if v.shared_ptr().is_some() {
        open.pop();
    }
}

/// Un `Float` s'affiche toujours avec sa partie décimale : `3.0`, pas `3`.
fn format_float(f: f64) -> String {
    format!("{f:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> TypeNames {
        TypeNames {
            structs: vec![],
            enums: vec![],
        }
    }

    #[test]
    fn int_and_float_compare_with_promotion() {
        assert!(equal(&Value::Int(2), &Value::Float(2.0)));
        assert!(!equal(&Value::Int(2), &Value::Str("2".into())));
    }

    #[test]
    fn arrays_compare_by_content() {
        let a = Value::new_array(vec![Value::Int(1), Value::Int(2)]);
        let b = Value::new_array(vec![Value::Int(1), Value::Int(2)]);
        assert!(equal(&a, &b));
    }

    #[test]
    fn floats_keep_their_decimal_part() {
        assert_eq!(display(&Value::Float(3.0), &names()), "3.0");
        assert_eq!(display(&Value::Float(0.5), &names()), "0.5");
    }

    #[test]
    fn strings_are_quoted_only_inside_collections() {
        let s = Value::Str("Alex".into());
        assert_eq!(display(&s, &names()), "Alex");
        assert_eq!(display(&Value::new_array(vec![s]), &names()), "[\"Alex\"]");
    }

    #[test]
    fn cycles_are_displayed_without_looping() {
        let a = Value::new_array(vec![]);
        if let Value::Array(inner) = &a {
            inner.borrow_mut().push(a.clone());
        }
        assert_eq!(display(&a, &names()), "[…]");
        assert!(equal(&a, &a.clone()));
    }
}
