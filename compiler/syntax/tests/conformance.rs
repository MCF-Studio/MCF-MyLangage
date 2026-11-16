//! Tests de conformité : `tests/conformance/valid/*.mcf` doit être accepté sans aucun
//! diagnostic ; chaque `tests/conformance/invalid/*.mcf` commence par
//! `// attendu: MCFxxxx[, MCFyyyy]` et doit produire exactement ces identifiants, dans l'ordre.

use std::fs;
use std::path::{Path, PathBuf};

fn files(dir: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance")
        .join(dir);
    let mut out: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "mcf"))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "aucun fichier dans {}", root.display());
    out
}

fn ids(src: &str) -> Vec<&'static str> {
    mcf_syntax::parse(src)
        .diagnostics
        .iter()
        .map(|d| d.code.id)
        .collect()
}

#[test]
fn valid_programs_are_accepted() {
    let mut failures = Vec::new();
    for path in files("valid") {
        let src = fs::read_to_string(&path).unwrap();
        let got = ids(&src);
        if !got.is_empty() {
            failures.push(format!("{} : {got:?}", path.display()));
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
    for path in files("invalid") {
        let src = fs::read_to_string(&path).unwrap();
        let header = src.lines().next().unwrap_or_default();
        let expected: Vec<&str> = header
            .strip_prefix("// attendu:")
            .unwrap_or_else(|| panic!("{} : en-tête `// attendu:` manquant", path.display()))
            .split(',')
            .map(str::trim)
            .collect();
        let got = ids(&src);
        if got != expected {
            failures.push(format!(
                "{} : attendu {expected:?}, obtenu {got:?}",
                path.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
