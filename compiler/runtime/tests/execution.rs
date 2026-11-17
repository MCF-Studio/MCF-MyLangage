//! Tests d'exécution : `tests/execution/`.
//!
//! - Une entrée est un fichier `nom.mcf` ou un dossier `nom/` (projet multi-fichiers).
//! - `nom.attendu` contient exactement ce que le programme doit afficher (stdout).
//! - Si la première ligne du premier fichier est `// erreur: MCFxxxx`, l'exécution doit
//!   s'arrêter sur cette erreur (après avoir affiché `nom.attendu`, s'il existe).
//! - Chaque entrée doit d'abord passer la vérification sans erreur ni avertissement
//!   (les conseils sont ignorés).
//! - Les fichiers `.cfg` d'un dossier font partie du projet (phase 9).

use mcf_semantic::{check_project, Source};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/execution")
}

/// Fichiers `.mcf` et `.cfg` d'une entrée, sous-dossiers compris.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "mcf" || x == "cfg") {
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
            text: fs::read_to_string(&p).unwrap().replace("\r\n", "\n"),
        })
        .collect()
}

fn run_entry(entry: &Path) -> Result<(), String> {
    let srcs = sources(entry);
    let diags: Vec<_> = check_project(&srcs)
        .into_iter()
        .flatten()
        .filter(|d| !d.is_hint())
        .map(|d| format!("{} {}", d.code.id, d.message))
        .collect();
    if !diags.is_empty() {
        return Err(format!("refusé à la vérification : {diags:?}"));
    }
    let first_code = srcs
        .iter()
        .find(|s| !s.is_config())
        .expect("un fichier .mcf");
    let expected_error = first_code
        .text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("// erreur:"))
        .map(|c| c.trim().to_string());
    let expected_out = fs::read_to_string(entry.with_extension("attendu"))
        .map(|s| s.replace("\r\n", "\n"))
        .unwrap_or_default();

    let code: Vec<_> = srcs.iter().filter(|s| !s.is_config()).collect();
    let parsed: Vec<_> = code.iter().map(|s| mcf_syntax::parse(&s.text)).collect();
    let modules: Vec<_> = parsed.iter().map(|p| &p.module).collect();
    // Noms courts : `meta.source` affiche « fichier.mcf:ligne » quel que soit le dossier.
    let files: Vec<_> = code
        .iter()
        .map(|s| {
            let short = Path::new(&s.name).file_name().unwrap().to_string_lossy();
            mcf_diagnostics::SourceFile::new(short.to_string(), s.text.clone())
        })
        .collect();
    let mut out = Vec::new();
    let dirs: Vec<_> = code
        .iter()
        .map(|s| Path::new(&s.name).parent().unwrap().to_path_buf())
        .collect();
    let result = mcf_runtime::run_with_dirs(&modules, &files, &dirs, &mut out);
    let out = String::from_utf8(out).unwrap();

    if out != expected_out {
        return Err(format!(
            "sortie différente\n--- attendu\n{expected_out}--- obtenu\n{out}---"
        ));
    }
    match (result, expected_error) {
        (Ok(()), None) => Ok(()),
        (Err(e), Some(code)) if e.diagnostic.code.id == code => Ok(()),
        (Err(e), expected) => Err(format!(
            "erreur {} ({}) ; attendu : {expected:?}",
            e.diagnostic.code.id, e.diagnostic.message
        )),
        (Ok(()), Some(code)) => Err(format!("aucune erreur, attendu {code}")),
    }
}

#[test]
fn programs_produce_expected_output() {
    let mut entries: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir() || p.extension().is_some_and(|x| x == "mcf"))
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "aucun programme dans tests/execution");
    let failures: Vec<String> = entries
        .iter()
        .filter_map(|e| {
            run_entry(e)
                .err()
                .map(|why| format!("{} : {why}", e.display()))
        })
        .collect();
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}
