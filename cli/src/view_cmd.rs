//! `mcf view` : calcule la vue d'un composant `.uimcf` (valeurs initiales, sans événement) et
//! l'écrit en page HTML (`--out`, par défaut `<composant>.html`) ou en JSON (`--format json`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mcf_diagnostics::SourceFile;
use mcf_semantic::check_project;

use crate::{html, load_sources, Format, Options};

pub fn view(opts: &Options) -> ExitCode {
    let sources = match load_sources("view", opts) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let files: Vec<SourceFile> = sources
        .iter()
        .map(|s| SourceFile::new(s.name.clone(), s.text.clone()))
        .collect();
    let mut errors = 0usize;
    for (file, diags) in files.iter().zip(check_project(&sources)) {
        for diag in diags {
            if diag.is_hint() {
                continue;
            }
            errors += usize::from(diag.is_error());
            eprintln!("{}", file.render(&diag));
        }
    }
    if errors > 0 {
        eprintln!("échec : {errors} erreur(s), la vue n'est pas calculée");
        return ExitCode::from(1);
    }
    let code: Vec<usize> = (0..sources.len())
        .filter(|&i| !sources[i].is_config() && !sources[i].is_ui())
        .collect();
    let uis: Vec<usize> = (0..sources.len()).filter(|&i| sources[i].is_ui()).collect();
    let code_parsed: Vec<_> = code
        .iter()
        .map(|&i| mcf_syntax::parse(&sources[i].text))
        .collect();
    let ui_parsed: Vec<_> = uis
        .iter()
        .map(|&i| mcf_syntax::parse_ui(&sources[i].text))
        .collect();
    let modules: Vec<_> = code_parsed.iter().map(|p| &p.module).collect();
    let ui_modules: Vec<_> = ui_parsed.iter().map(|p| &p.module).collect();
    // Fichiers dans l'ordre attendu par le runtime : code, puis interfaces.
    let ordered: Vec<SourceFile> = code
        .iter()
        .chain(&uis)
        .map(|&i| SourceFile::new(sources[i].name.clone(), sources[i].text.clone()))
        .collect();
    let component = match &opts.component {
        Some(c) => c.clone(),
        None => match ui_modules.iter().flat_map(|m| &m.components).next() {
            Some(c) => c.name.name.clone(),
            None => {
                eprintln!("mcf view : aucun composant `ui` dans le projet");
                return ExitCode::from(2);
            }
        },
    };
    let target = Path::new(&opts.positional[0]);
    let root = if target.is_file() {
        target.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        target.to_path_buf()
    };
    let dirs: Vec<PathBuf> = ordered
        .iter()
        .map(|f| {
            Path::new(&f.name)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        })
        .collect();
    let node =
        match mcf_runtime::render_view(&modules, &ui_modules, &ordered, &dirs, &root, &component) {
            Ok(n) => n,
            Err(err) => {
                let d = &err.diagnostic;
                match err.file {
                    Some(i) => eprint!("{}", ordered[i].render(d)),
                    None => {
                        eprintln!("erreur[{}] {}: {}", d.code.id, d.code.name, d.message);
                        if let Some(s) = &d.suggestion {
                            eprintln!("  = suggestion : {s}");
                        }
                    }
                }
                return ExitCode::from(3);
            }
        };
    if opts.format == Format::Json {
        println!(
            "{}",
            serde_json::to_string_pretty(&node).expect("JSON sérialisable")
        );
        return ExitCode::SUCCESS;
    }
    let out = opts
        .out
        .clone()
        .unwrap_or_else(|| format!("{component}.html"));
    if let Err(e) = std::fs::write(&out, html::page(&component, &node)) {
        eprintln!("mcf view : impossible d'écrire `{out}` : {e}");
        return ExitCode::from(2);
    }
    eprintln!(
        "vue `{component}` écrite dans {out} ({} éléments)",
        node.count()
    );
    ExitCode::SUCCESS
}
