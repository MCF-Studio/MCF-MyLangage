//! Fichiers de configuration `.cfg` (cahier des charges 1.2, §2 ; phase 9).
//!
//! Un `.cfg` est du JSON standard (RFC 8259) auquel on ajoute les commentaires `//` et `/* */`
//! (DECISIONS D48). Ce module le lit en gardant la position de chaque valeur, puis le valide
//! contre un [`Schema`] construit depuis un type MCF. La même validation sert à `mcf check`
//! (diagnostics dans le `.cfg`) et à l'exécution (relecture au démarrage et `loadConfig`).
//!
//! Règles détaillées : `docs/grammaire/configuration.md`.

use std::sync::Arc;

use mcf_diagnostics::{Diagnostic, Span};

mod json;
mod schema;

pub use json::{parse, Json, JsonKind, Member};
pub use schema::{ambiguous_dual, pick_member, validate, Schema, Shape};

/// Taille maximale d'un fichier `.cfg` (octets). Une configuration plus grosse est presque
/// toujours une erreur (mauvais fichier) ; la limite protège aussi l'outillage.
pub const MAX_SIZE: usize = 4 * 1024 * 1024;

/// Profondeur maximale d'imbrication (objets et tableaux).
pub const MAX_DEPTH: usize = 64;

/// Lit puis valide un fichier : la valeur JSON si elle est conforme, sinon tous les problèmes.
pub fn load(text: &str, schema: &Schema) -> Result<Json, Vec<Diagnostic>> {
    let value = parse(text).map_err(|d| vec![*d])?;
    let issues = validate(&value, schema);
    if issues.is_empty() {
        Ok(value)
    } else {
        Err(issues)
    }
}

/// Vérifie la forme d'un chemin de configuration : relatif, séparateur `/`, extension `.cfg`.
/// Retourne le message d'erreur sinon.
pub fn check_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("chemin vide".into());
    }
    if path.contains('\\') {
        return Err("les dossiers se séparent par `/`, pas `\\`".into());
    }
    if path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        return Err("chemin absolu : il doit être relatif au fichier `.mcf`".into());
    }
    if !path.ends_with(".cfg") {
        return Err("un fichier de configuration se termine par `.cfg`".into());
    }
    Ok(())
}

/// Joint `relative` au dossier de `mcf_file` puis supprime les `.` et `..`, avec `/` comme
/// séparateur. `None` si le chemin remonte au-dessus de la racine des noms.
pub fn resolve_path(mcf_file: &str, relative: &str) -> Option<String> {
    let base = mcf_file.replace('\\', "/");
    let mut parts: Vec<&str> = base.split('/').collect();
    parts.pop(); // nom du fichier `.mcf`
    let absolute = base.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for part in parts.into_iter().chain(relative.split('/')) {
        match part {
            "" | "." => {}
            ".." => match out.last() {
                Some(&last) if last != ".." => {
                    out.pop();
                }
                _ if absolute => return None,
                _ => out.push(".."),
            },
            p => out.push(p),
        }
    }
    let joined = out.join("/");
    Some(if absolute {
        format!("/{joined}")
    } else {
        joined
    })
}

/// Forme normalisée d'un nom de fichier, pour comparer à [`resolve_path`].
pub fn normalize_name(name: &str) -> String {
    let n = name.replace('\\', "/");
    let file = n.rsplit('/').next().unwrap_or_default().to_string();
    resolve_path(&n, &file).unwrap_or(n)
}

/// Nom le plus proche dans `candidates` (distance d'édition ≤ 2, ou même nom sans la casse).
pub fn closest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        let d = if c.eq_ignore_ascii_case(name) {
            0
        } else {
            distance(name, c)
        };
        if d <= 2 && best.map_or(true, |(b, _)| d < b) {
            best = Some((d, c));
        }
    }
    best.map(|(_, c)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

pub(crate) fn error(
    code: &'static mcf_diagnostics::Code,
    span: Span,
    message: String,
) -> Diagnostic {
    Diagnostic::new(code, span, message)
}

/// Schéma partagé (les structures réutilisées ne sont construites qu'une fois).
pub type SchemaRef = Arc<Schema>;

#[cfg(test)]
mod tests;
