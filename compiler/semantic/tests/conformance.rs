//! Tests de conformité sur le pipeline complet (syntaxe + sémantique).
//!
//! - `tests/conformance/valid/` : chaque entrée doit être acceptée sans erreur ni
//!   avertissement (les conseils sont ignorés).
//! - `tests/conformance/invalid/` : chaque fichier commence par `// attendu: MCFxxxx[, ...]`
//!   et doit produire exactement ces identifiants, dans l'ordre (conseils ignorés).
//! - `tests/conformance/conseils/` : même en-tête, comparé aux seuls conseils (MCF05xx) ;
//!   `// attendu: aucun` vérifie qu'aucun conseil n'est donné à tort.
//!
//! Une entrée est soit un fichier `.mcf` (projet d'un seul fichier), soit un dossier
//! (projet multi-fichiers ; les en-têtes `// attendu:` de ses fichiers sont concaténés
//! dans l'ordre alphabétique des fichiers). Une entrée peut aussi être un `.uimcf` seul.
//! Les `.uimcf` et `.cfg` d'un dossier font partie du projet ;
//! comme ils acceptent les commentaires, ils peuvent aussi porter un en-tête `// attendu:`.

use mcf_semantic::{check_project, Source};
use std::fs;
use std::path::{Path, PathBuf};

fn entries(dir: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance")
        .join(dir);
    let mut out: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir() || p.extension().is_some_and(|x| x == "mcf" || x == "uimcf"))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "aucune entrée dans {}", root.display());
    out
}

/// Fichiers `.mcf`, `.uimcf` et `.cfg` d'une entrée, sous-dossiers compris.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            collect(&p, out);
        } else if p
            .extension()
            .is_some_and(|x| x == "mcf" || x == "uimcf" || x == "cfg")
        {
            out.push(p);
        }
    }
}

fn sources(entry: &Path) -> Vec<Source> {
    let mut files = Vec::new();
    if entry.is_dir() {
        collect(entry, &mut files);
    } else {
        files.push(entry.to_path_buf());
    }
    files.sort();
    files
        .into_iter()
        .map(|p| Source {
            name: p.display().to_string(),
            text: fs::read_to_string(&p).unwrap(),
        })
        .collect()
}

/// (codes attendus, codes obtenus) ; `hints` : ne garder que les conseils, sinon les ignorer.
fn run(entry: &Path, hints: bool) -> (Vec<String>, Vec<&'static str>) {
    let srcs = sources(entry);
    let expected = srcs
        .iter()
        .filter_map(|s| s.text.lines().next()?.strip_prefix("// attendu:"))
        .flat_map(|h| h.split(',').map(|c| c.trim().to_string()))
        .filter(|c| c != "aucun")
        .collect();
    let got = check_project(&srcs)
        .into_iter()
        .flatten()
        .filter(|d| d.is_hint() == hints)
        .map(|d| d.code.id)
        .collect();
    (expected, got)
}

#[test]
fn valid_programs_are_accepted() {
    let mut failures = Vec::new();
    for entry in entries("valid") {
        let (_, got) = run(&entry, false);
        if !got.is_empty() {
            failures.push(format!("{} : {got:?}", entry.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "programmes valides refusés :\n{}",
        failures.join("\n")
    );
}

#[test]
fn invalid_programs_report_expected_codes() {
    let mut failures = Vec::new();
    for entry in entries("invalid") {
        let (expected, got) = run(&entry, false);
        assert!(
            !expected.is_empty(),
            "{} : en-tête `// attendu:` manquant",
            entry.display()
        );
        if got != expected {
            failures.push(format!(
                "{} : attendu {expected:?}, obtenu {got:?}",
                entry.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn hints_are_given_where_expected() {
    let mut failures = Vec::new();
    for entry in entries("conseils") {
        let header = sources(&entry)
            .iter()
            .any(|s| s.text.starts_with("// attendu:"));
        assert!(
            header,
            "{} : en-tête `// attendu:` manquant",
            entry.display()
        );
        let (expected, got) = run(&entry, true);
        if got != expected {
            failures.push(format!(
                "{} : attendu {expected:?}, obtenu {got:?}",
                entry.display()
            ));
        }
        let (_, errors) = run(&entry, false);
        if !errors.is_empty() {
            failures.push(format!(
                "{} : erreurs inattendues {errors:?}",
                entry.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
