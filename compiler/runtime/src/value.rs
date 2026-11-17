//! Valeurs manipulées à l'exécution.
//!
//! Les nombres, booléens, chaînes et variantes d'énumération sont copiés ; les tableaux, maps
//! et structures sont **partagés** (DECISIONS D30) : `_b = _a;` désigne la même structure.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

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
    Str(Arc<str>),
    Enum {
        id: usize,
        variant: usize,
    },
    Array(Arc<ValueCell<Vec<Value>>>),
    /// Paires clé → valeur dans l'ordre d'insertion.
    Map(Arc<ValueCell<Vec<(Value, Value)>>>),
    Struct(Arc<ValueCell<StructValue>>),
    /// Tâche lancée par `start` : numéro de la tâche dans l'ordonnanceur (une poignée, pas
    /// un contenu : la copier ou la comparer porte sur la même tâche).
    Task(usize),
    /// Abonnement créé par `on Evenement => fonction` : numéro dans la liste des abonnements.
    Listener(usize),
    /// Résultat d'un appel de fonction `Void`.
    Void,
}

/// Contenu partagé d'une structure, d'un tableau ou d'une map. S'emprunte comme un `RefCell`,
/// mais peut passer d'une tâche à l'autre (chaque tâche a son fil, voir `lib.rs`).
///
/// Une seule tâche s'exécute à la fois et aucun emprunt n'est gardé pendant l'exécution de code
/// MCF : un emprunt déjà pris est donc forcément un emprunt imbriqué, un bug de l'interpréteur.
/// Il provoque une panique explicite, jamais un blocage silencieux.
///
/// Un contenu `frozen` vient d'une déclaration `config` : il se lit seulement, chaque
/// modification est refusée à l'exécution (MCF0414). `duplicate` en fait une copie modifiable.
#[derive(Debug)]
pub struct ValueCell<T> {
    value: Mutex<T>,
    frozen: bool,
}

impl<T> ValueCell<T> {
    pub fn new(value: T) -> Self {
        ValueCell {
            value: Mutex::new(value),
            frozen: false,
        }
    }

    pub fn new_frozen(value: T) -> Self {
        ValueCell {
            value: Mutex::new(value),
            frozen: true,
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn borrow(&self) -> MutexGuard<'_, T> {
        match self.value.try_lock() {
            Ok(guard) => guard,
            // Une panique a déjà interrompu l'exécution : on laisse l'arrêt se poursuivre.
            Err(TryLockError::Poisoned(p)) => p.into_inner(),
            Err(TryLockError::WouldBlock) => {
                panic!("valeur MCF empruntée deux fois en même temps (bug de l'interpréteur)")
            }
        }
    }

    pub fn borrow_mut(&self) -> MutexGuard<'_, T> {
        self.borrow()
    }
}

#[derive(Debug)]
pub struct StructValue {
    pub id: usize,
    pub fields: Vec<Value>,
}

impl Value {
    pub fn new_array(items: Vec<Value>) -> Value {
        Value::Array(Arc::new(ValueCell::new(items)))
    }

    pub fn new_map() -> Value {
        Value::Map(Arc::new(ValueCell::new(Vec::new())))
    }

    /// Adresse du contenu partagé, pour détecter les cycles.
    fn shared_ptr(&self) -> Option<usize> {
        match self {
            Value::Array(a) => Some(Arc::as_ptr(a) as *const u8 as usize),
            Value::Map(m) => Some(Arc::as_ptr(m) as *const u8 as usize),
            Value::Struct(s) => Some(Arc::as_ptr(s) as *const u8 as usize),
            _ => None,
        }
    }
}

/// `duplicate(v)` : copie profonde. Les structures, tableaux et maps sont recopiés à tous les
/// niveaux, la copie ne partage rien avec l'original. Les partages internes et les cycles
/// sont reproduits à l'identique dans la copie (DECISIONS D37).
pub fn deep_copy(v: &Value) -> Value {
    copy_in(v, &mut HashMap::new())
}

fn copy_in(v: &Value, done: &mut HashMap<usize, Value>) -> Value {
    let Some(ptr) = v.shared_ptr() else {
        return v.clone();
    };
    if let Some(copy) = done.get(&ptr) {
        return copy.clone();
    }
    match v {
        Value::Array(items) => {
            let copy = Value::new_array(Vec::new());
            done.insert(ptr, copy.clone());
            let values: Vec<Value> = items.borrow().iter().map(|i| copy_in(i, done)).collect();
            if let Value::Array(c) = &copy {
                *c.borrow_mut() = values;
            }
            copy
        }
        Value::Map(entries) => {
            let copy = Value::new_map();
            done.insert(ptr, copy.clone());
            let values: Vec<(Value, Value)> = entries
                .borrow()
                .iter()
                .map(|(k, val)| (copy_in(k, done), copy_in(val, done)))
                .collect();
            if let Value::Map(c) = &copy {
                *c.borrow_mut() = values;
            }
            copy
        }
        Value::Struct(s) => {
            let id = s.borrow().id;
            let copy = Value::Struct(Arc::new(ValueCell::new(StructValue {
                id,
                fields: Vec::new(),
            })));
            done.insert(ptr, copy.clone());
            let fields: Vec<Value> = s.borrow().fields.iter().map(|f| copy_in(f, done)).collect();
            if let Value::Struct(c) = &copy {
                c.borrow_mut().fields = fields;
            }
            copy
        }
        other => other.clone(),
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
        // Copies superficielles : aucun emprunt n'est gardé pendant la récursion (un tableau
        // peut se contenir lui-même).
        (Value::Array(x), Value::Array(y)) => {
            let (x, y) = (x.borrow().clone(), y.borrow().clone());
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(i, j)| equal_in(i, j, assumed))
        }
        (Value::Map(x), Value::Map(y)) => {
            let (x, y) = (x.borrow().clone(), y.borrow().clone());
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.iter()
                        .find(|(k2, _)| equal_in(k, k2, assumed))
                        .is_some_and(|(_, v2)| equal_in(v, v2, assumed))
                })
        }
        (Value::Struct(x), Value::Struct(y)) => {
            let (xid, xf) = {
                let x = x.borrow();
                (x.id, x.fields.clone())
            };
            let (yid, yf) = {
                let y = y.borrow();
                (y.id, y.fields.clone())
            };
            xid == yid
                && xf
                    .iter()
                    .zip(yf.iter())
                    .all(|(i, j)| equal_in(i, j, assumed))
        }
        (Value::Void, Value::Void) => true,
        (Value::Task(x), Value::Task(y)) | (Value::Listener(x), Value::Listener(y)) => x == y,
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
        Value::Task(id) => {
            let _ = write!(out, "Task#{id}");
        }
        Value::Listener(id) => {
            let _ = write!(out, "Listener#{id}");
        }
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
    fn deep_copy_shares_nothing_with_the_original() {
        let inner = Value::new_array(vec![Value::Int(1)]);
        let outer = Value::new_array(vec![inner.clone(), inner]);
        let copy = deep_copy(&outer);
        let (Value::Array(o), Value::Array(c)) = (&outer, &copy) else {
            unreachable!()
        };
        assert!(!Arc::ptr_eq(o, c));
        let items = c.borrow().clone();
        let (Value::Array(c0), Value::Array(c1)) = (&items[0], &items[1]) else {
            unreachable!()
        };
        // Le partage interne est reproduit, mais pas avec l'original.
        assert!(Arc::ptr_eq(c0, c1));
        let first = o.borrow()[0].clone();
        let Value::Array(o0) = &first else {
            unreachable!()
        };
        assert!(!Arc::ptr_eq(o0, c0));
        assert!(equal(&outer, &copy));
    }

    #[test]
    fn deep_copy_of_a_cycle_terminates() {
        let a = Value::new_array(vec![]);
        if let Value::Array(inner) = &a {
            inner.borrow_mut().push(a.clone());
        }
        let copy = deep_copy(&a);
        assert_eq!(display(&copy, &names()), "[…]");
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
