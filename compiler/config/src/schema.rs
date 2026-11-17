//! Validation d'une valeur JSON par un type MCF.
//!
//! Correspondances (`docs/grammaire/configuration.md`) :
//! `Bool` ← booléen ; `Int` ← nombre entier (sans partie décimale ni exposant) ; `Float` ← tout
//! nombre ; `String` ← chaîne ; énumération ← chaîne qui nomme une variante ; structure ← objet
//! avec exactement ses champs ; `Array<T>` ← tableau ; `Map<String, T>` ← objet ; DualType ← la
//! valeur choisit le type par sa forme JSON. `null` n'est jamais accepté.

use std::sync::Arc;

use mcf_diagnostics::{codes, Diagnostic, Span};

use crate::json::{Json, JsonKind};
use crate::{closest, error, SchemaRef};

/// Type MCF attendu, réduit à ce qui peut s'écrire en JSON. `id` désigne la structure ou
/// l'énumération pour celui qui a construit le schéma (analyseur ou interpréteur).
#[derive(Clone, Debug, PartialEq)]
pub enum Schema {
    Bool,
    Int,
    Float,
    String,
    Enum {
        id: usize,
        name: String,
        variants: Vec<String>,
    },
    Struct {
        id: usize,
        name: String,
        fields: Vec<(String, SchemaRef)>,
    },
    Array(SchemaRef),
    /// Clés `String`.
    Map(SchemaRef),
    Dual(SchemaRef, SchemaRef),
}

/// Forme JSON d'une valeur ou d'un type : deux types d'un DualType doivent en avoir des
/// différentes (sauf `Int` et `Float`, départagés par l'écriture du nombre).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Bool,
    Number,
    String,
    Array,
    Object,
}

impl Schema {
    /// Écriture MCF du type, pour les messages.
    pub fn describe(&self) -> String {
        match self {
            Schema::Bool => "Bool".into(),
            Schema::Int => "Int".into(),
            Schema::Float => "Float".into(),
            Schema::String => "String".into(),
            Schema::Enum { name, .. } | Schema::Struct { name, .. } => name.clone(),
            Schema::Array(e) => format!("Array<{}>", e.describe()),
            Schema::Map(v) => format!("Map<String, {}>", v.describe()),
            Schema::Dual(a, b) => format!("{}, {}", a.describe(), b.describe()),
        }
    }

    pub fn shape(&self) -> Option<Shape> {
        Some(match self {
            Schema::Bool => Shape::Bool,
            Schema::Int | Schema::Float => Shape::Number,
            Schema::String | Schema::Enum { .. } => Shape::String,
            Schema::Array(_) => Shape::Array,
            Schema::Struct { .. } | Schema::Map(_) => Shape::Object,
            Schema::Dual(..) => return None,
        })
    }

    /// Ce qu'on attend en JSON, pour les messages.
    fn expected(&self) -> String {
        match self {
            Schema::Bool => "un booléen (`true` ou `false`)".into(),
            Schema::Int => "un nombre entier".into(),
            Schema::Float => "un nombre".into(),
            Schema::String => "une chaîne".into(),
            Schema::Enum { name, .. } => format!("une chaîne (variante de `{name}`)"),
            Schema::Struct { name, .. } => format!("un objet (structure `{name}`)"),
            Schema::Array(_) => format!("un tableau (`{}`)", self.describe()),
            Schema::Map(_) => format!("un objet (`{}`)", self.describe()),
            Schema::Dual(a, b) => format!("{} ou {}", a.expected(), b.expected()),
        }
    }
}

/// Deux types d'un DualType qui s'écrivent pareil en JSON : la valeur ne pourrait pas dire
/// lequel elle est.
pub fn ambiguous_dual(a: &Schema, b: &Schema) -> bool {
    let numbers = matches!(
        (a, b),
        (Schema::Int, Schema::Float) | (Schema::Float, Schema::Int)
    );
    !numbers && a.shape() == b.shape()
}

fn shape_of(v: &Json) -> Option<Shape> {
    Some(match &v.kind {
        JsonKind::Null => return None,
        JsonKind::Bool(_) => Shape::Bool,
        JsonKind::Number { .. } => Shape::Number,
        JsonKind::Str(_) => Shape::String,
        JsonKind::Array(_) => Shape::Array,
        JsonKind::Object(_) => Shape::Object,
    })
}

/// Type d'un DualType que prend la valeur : celui qui a sa forme JSON ; pour `Int, Float`, un
/// nombre entier est un `Int`, un nombre avec partie décimale ou exposant un `Float`.
pub fn pick_member<'s>(v: &Json, a: &'s Schema, b: &'s Schema) -> Option<&'s Schema> {
    if let JsonKind::Number { integer, .. } = v.kind {
        let int = [a, b].into_iter().find(|s| matches!(s, Schema::Int));
        let float = [a, b].into_iter().find(|s| matches!(s, Schema::Float));
        return match (int, float) {
            (Some(i), Some(f)) => Some(if integer { i } else { f }),
            (Some(i), None) => Some(i),
            (None, f) => f,
        };
    }
    let shape = shape_of(v)?;
    [a, b].into_iter().find(|s| s.shape() == Some(shape))
}

/// Tous les écarts entre la valeur et le type attendu (vide si conforme).
pub fn validate(value: &Json, schema: &Schema) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    check(value, schema, "", &mut out);
    out
}

/// `where_` : chemin de la valeur dans le fichier (`server.hosts[2]`), vide pour la racine.
fn check(v: &Json, schema: &Schema, where_: &str, out: &mut Vec<Diagnostic>) {
    let at = if where_.is_empty() {
        "la racine du fichier".to_string()
    } else {
        format!("`{where_}`")
    };
    let wrong = |out: &mut Vec<Diagnostic>, extra: Option<&str>| {
        let mut d = error(
            &codes::CONFIG_WRONG_TYPE,
            v.span,
            format!(
                "{at} doit être {}, trouvé {}",
                schema.expected(),
                v.kind.describe()
            ),
        )
        .expected(schema.describe())
        .received(v.kind.describe());
        if let Some(s) = extra {
            d = d.suggestion(s);
        } else if v.kind == JsonKind::Null {
            d = d.suggestion("`null` n'existe pas en MCF : écrire une valeur du type attendu");
        }
        out.push(d);
    };
    match (schema, &v.kind) {
        (Schema::Dual(a, b), _) => match pick_member(v, a, b) {
            Some(member) => check(v, member, where_, out),
            None => wrong(out, None),
        },
        (Schema::Bool, JsonKind::Bool(_)) | (Schema::String, JsonKind::Str(_)) => {}
        (Schema::Int, JsonKind::Number { text, integer }) => {
            if !integer {
                wrong(
                    out,
                    Some("un `Int` s'écrit sans partie décimale ni exposant (`8080`) ; pour un décimal, déclarer le champ en `Float`"),
                );
            } else if text.parse::<i64>().is_err() {
                out.push(
                    error(
                        &codes::CONFIG_NUMBER_OUT_OF_RANGE,
                        v.span,
                        format!("{at} : {text} dépasse la plage de `Int` (64 bits signé)"),
                    )
                    .expected(format!("entre {} et {}", i64::MIN, i64::MAX)),
                );
            }
        }
        (Schema::Float, JsonKind::Number { text, .. }) => {
            if !text.parse::<f64>().is_ok_and(f64::is_finite) {
                out.push(error(
                    &codes::CONFIG_NUMBER_OUT_OF_RANGE,
                    v.span,
                    format!("{at} : {text} dépasse la plage de `Float` (64 bits)"),
                ));
            }
        }
        (Schema::Enum { name, variants, .. }, JsonKind::Str(s)) => {
            if !variants.contains(s) {
                let mut d = error(
                    &codes::CONFIG_UNKNOWN_VARIANT,
                    v.span,
                    format!("{at} : \"{s}\" n'est pas une variante de `{name}`"),
                )
                .expected(variants.join(", "))
                .received(format!("\"{s}\""));
                if let Some(c) = closest(s, variants.iter().map(String::as_str)) {
                    d = d.suggestion(format!("vouliez-vous dire \"{c}\" ?"));
                }
                out.push(d);
            }
        }
        (Schema::Struct { name, fields, .. }, JsonKind::Object(members)) => {
            for m in members {
                let inner = join(where_, &m.key);
                match fields.iter().find(|(f, _)| *f == m.key) {
                    Some((_, ty)) => check(&m.value, ty, &inner, out),
                    None => {
                        let mut d = error(
                            &codes::CONFIG_UNKNOWN_KEY,
                            m.key_span,
                            format!("la structure `{name}` n'a pas de champ \"{}\"", m.key),
                        )
                        .expected(field_list(fields));
                        d = match closest(&m.key, fields.iter().map(|(f, _)| f.as_str())) {
                            Some(c) => d.suggestion(format!("vouliez-vous dire \"{c}\" ?")),
                            None => d.suggestion(
                                "retirer cette clé, ou ajouter le champ à la structure",
                            ),
                        };
                        out.push(d);
                    }
                }
            }
            for (f, ty) in fields {
                if !members.iter().any(|m| m.key == *f) {
                    out.push(
                        error(
                            &codes::CONFIG_MISSING_KEY,
                            Span::new(v.span.start, v.span.start + 1),
                            format!(
                                "{at} : le champ \"{f}\" de `{name}` est absent (tous les champs sont obligatoires)"
                            ),
                        )
                        .suggestion(format!("ajouter \"{f}\": <{}>", ty.describe())),
                    );
                }
            }
        }
        (Schema::Map(value_ty), JsonKind::Object(members)) => {
            for m in members {
                let inner = format!("{where_}[\"{}\"]", m.key);
                check(&m.value, value_ty, &inner, out);
            }
        }
        (Schema::Array(item_ty), JsonKind::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                check(item, item_ty, &format!("{where_}[{i}]"), out);
            }
        }
        _ => wrong(out, None),
    }
}

fn join(where_: &str, key: &str) -> String {
    if where_.is_empty() {
        key.to_string()
    } else {
        format!("{where_}.{key}")
    }
}

fn field_list(fields: &[(String, Arc<Schema>)]) -> String {
    fields
        .iter()
        .map(|(f, _)| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ")
}
