//! Représentation des types et règles de compatibilité.
//! Règles documentées dans `docs/grammaire/types.md`.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Bool,
    Int,
    Float,
    String,
    Char,
    Byte,
    Bytes,
    DateTime,
    Void,
    /// `Array` seul = `Array<Unknown>` (élément non précisé).
    Array(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    /// DualType : exactement deux types distincts, jamais imbriqués.
    Dual(Box<Ty>, Box<Ty>),
    /// Structure déclarée par `struct` (identifiant interne + nom pour les messages).
    Struct {
        id: usize,
        name: String,
    },
    /// Énumération déclarée par `enum`.
    Enum {
        id: usize,
        name: String,
    },
    /// Type non déterminable (erreur déjà signalée, ou valeur pas encore typée comme
    /// le membre d'une valeur). Compatible avec tout pour éviter les cascades d'erreurs.
    Unknown,
}

impl Ty {
    pub fn from_builtin(name: &str) -> Option<Ty> {
        Some(match name {
            "Bool" => Ty::Bool,
            "Int" => Ty::Int,
            "Float" => Ty::Float,
            "String" => Ty::String,
            "Char" => Ty::Char,
            "Byte" => Ty::Byte,
            "Bytes" => Ty::Bytes,
            "DateTime" => Ty::DateTime,
            "Void" => Ty::Void,
            "Array" => Ty::Array(Box::new(Ty::Unknown)),
            "Map" => Ty::Map(Box::new(Ty::Unknown), Box::new(Ty::Unknown)),
            _ => return None,
        })
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float)
    }

    pub fn is_dual(&self) -> bool {
        matches!(self, Ty::Dual(..))
    }

    /// Types possibles d'une valeur : deux pour un DualType, un sinon.
    pub fn members(&self) -> Vec<&Ty> {
        match self {
            Ty::Dual(a, b) => vec![a, b],
            other => vec![other],
        }
    }

    /// Égalité où `Unknown` joue le rôle de joker.
    pub fn same(&self, other: &Ty) -> bool {
        match (self, other) {
            (Ty::Unknown, _) | (_, Ty::Unknown) => true,
            (Ty::Array(a), Ty::Array(b)) => a.same(b),
            (Ty::Map(k1, v1), Ty::Map(k2, v2)) => k1.same(k2) && v1.same(v2),
            (Ty::Dual(a1, b1), Ty::Dual(a2, b2)) => {
                (a1.same(a2) && b1.same(b2)) || (a1.same(b2) && b1.same(a2))
            }
            (a, b) => a == b,
        }
    }

    /// Une valeur de type `self` peut-elle être rangée dans `target` ?
    /// Aucune conversion implicite : seuls l'égalité et l'inclusion dans un DualType sont admises.
    pub fn assignable_to(&self, target: &Ty) -> bool {
        if self.is_unknown() || target.is_unknown() {
            return true;
        }
        match target {
            Ty::Dual(..) => self
                .members()
                .iter()
                .all(|m| target.members().iter().any(|t| m.same(t))),
            _ => !self.is_dual() && self.same(target),
        }
    }
}

/// Méthodes natives d'une valeur : (types des paramètres, type de retour).
/// Liste documentée dans `docs/grammaire/types.md` (DECISIONS D38).
pub fn native_method(recv: &Ty, name: &str) -> Option<(Vec<Ty>, Ty)> {
    Some(match (recv, name) {
        (Ty::Array(_) | Ty::Map(..) | Ty::String, "length") => (vec![], Ty::Int),
        (Ty::Array(e), "push") => (vec![(**e).clone()], Ty::Void),
        (Ty::Array(e), "pop") => (vec![], (**e).clone()),
        (Ty::Array(e), "removeAt") => (vec![Ty::Int], (**e).clone()),
        (Ty::Array(e), "contains") => (vec![(**e).clone()], Ty::Bool),
        (Ty::Map(k, _), "has") => (vec![(**k).clone()], Ty::Bool),
        (Ty::Map(k, _), "remove") => (vec![(**k).clone()], Ty::Bool),
        (Ty::Map(k, _), "keys") => (vec![], Ty::Array(k.clone())),
        (Ty::Map(_, v), "values") => (vec![], Ty::Array(v.clone())),
        _ => return None,
    })
}

/// Noms des méthodes natives d'un type (pour les messages d'erreur).
pub fn native_method_names(recv: &Ty) -> &'static [&'static str] {
    match recv {
        Ty::Array(_) => &["length", "push", "pop", "removeAt", "contains"],
        Ty::Map(..) => &["length", "has", "remove", "keys", "values"],
        Ty::String => &["length"],
        _ => &[],
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Bool => write!(f, "Bool"),
            Ty::Int => write!(f, "Int"),
            Ty::Float => write!(f, "Float"),
            Ty::String => write!(f, "String"),
            Ty::Char => write!(f, "Char"),
            Ty::Byte => write!(f, "Byte"),
            Ty::Bytes => write!(f, "Bytes"),
            Ty::DateTime => write!(f, "DateTime"),
            Ty::Void => write!(f, "Void"),
            Ty::Array(e) if e.is_unknown() => write!(f, "Array"),
            Ty::Array(e) => write!(f, "Array<{e}>"),
            Ty::Map(k, v) if k.is_unknown() && v.is_unknown() => write!(f, "Map"),
            Ty::Map(k, v) => write!(f, "Map<{k}, {v}>"),
            Ty::Dual(a, b) => write!(f, "{a}, {b}"),
            Ty::Struct { name, .. } | Ty::Enum { name, .. } => write!(f, "{name}"),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dual(a: Ty, b: Ty) -> Ty {
        Ty::Dual(Box::new(a), Box::new(b))
    }

    #[test]
    fn dual_accepts_its_members_only() {
        let d = dual(Ty::Bool, Ty::Int);
        assert!(Ty::Bool.assignable_to(&d));
        assert!(Ty::Int.assignable_to(&d));
        assert!(!Ty::String.assignable_to(&d));
        assert!(dual(Ty::Int, Ty::Bool).assignable_to(&d));
    }

    #[test]
    fn dual_cannot_flow_into_single_type() {
        assert!(!dual(Ty::Bool, Ty::Int).assignable_to(&Ty::Int));
    }

    #[test]
    fn no_implicit_numeric_conversion_on_assignment() {
        assert!(!Ty::Int.assignable_to(&Ty::Float));
        assert!(!Ty::Float.assignable_to(&Ty::Int));
    }

    #[test]
    fn untyped_array_accepts_any_array() {
        let untyped = Ty::Array(Box::new(Ty::Unknown));
        let strings = Ty::Array(Box::new(Ty::String));
        assert!(strings.assignable_to(&untyped));
        assert!(!strings.assignable_to(&Ty::Array(Box::new(Ty::Int))));
    }
}
