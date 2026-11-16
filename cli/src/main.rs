//! `mcf` — interface en ligne de commande.
//!
//! Codes de sortie : 0 = aucun problème, 1 = erreurs détectées, 2 = usage ou E/S.

use mcf_diagnostics::{codes, SourceFile};
use mcf_semantic::{check_project, Source};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
MCF – MyLangage

Usage :
  mcf check <fichier|dossier> [--format text|json] [--strict]
  mcf describe errors|keywords [--format text|json]
  mcf version

Commandes prévues (pas encore implémentées) : new, run, build, watch, clean, info, fmt, doc";

#[derive(Clone, Copy, PartialEq)]
enum Format {
    Text,
    Json,
}

struct Options {
    positional: Vec<String>,
    format: Format,
    strict: bool,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        positional: Vec::new(),
        format: Format::Text,
        strict: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--strict" => opts.strict = true,
            "--format" => {
                opts.format = match it.next().map(String::as_str) {
                    Some("text") => Format::Text,
                    Some("json") => Format::Json,
                    other => return Err(format!("format inconnu : {other:?} (text ou json)")),
                }
            }
            flag if flag.starts_with("--") => return Err(format!("option inconnue : {flag}")),
            value => opts.positional.push(value.to_string()),
        }
    }
    Ok(opts)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let opts = match parse_options(&args[1..]) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("mcf : {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match command.as_str() {
        "check" => check(&opts),
        "describe" => describe(&opts),
        "version" | "--version" | "-V" => {
            println!("mcf {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        "new" | "run" | "build" | "watch" | "clean" | "info" | "fmt" | "doc" => {
            eprintln!("mcf {command} : pas encore implémenté (voir ROADMAP.md)");
            ExitCode::from(2)
        }
        other => {
            eprintln!("mcf : commande inconnue `{other}`\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn collect_sources(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let p = entry?.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if p.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "dist" {
                collect_sources(&p, out)?;
            }
        } else if p.extension().is_some_and(|e| e == "mcf") {
            out.push(p);
        }
    }
    Ok(())
}

fn check(opts: &Options) -> ExitCode {
    let [target] = opts.positional.as_slice() else {
        eprintln!("mcf check : un chemin est attendu\n\n{USAGE}");
        return ExitCode::from(2);
    };
    let mut paths = Vec::new();
    if let Err(e) = collect_sources(Path::new(target), &mut paths) {
        eprintln!("mcf check : impossible de lire `{target}` : {e}");
        return ExitCode::from(2);
    }
    paths.sort();

    let (mut errors, mut warnings) = (0usize, 0usize);
    let mut json_diags = Vec::new();
    let mut sources = Vec::new();
    for path in &paths {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("mcf check : impossible de lire `{}` : {e}", path.display());
                return ExitCode::from(2);
            }
        };
        sources.push(Source {
            name: path.display().to_string().replace('\\', "/"),
            text: text.strip_prefix('\u{feff}').unwrap_or(&text).to_string(),
        });
    }
    // Tout le dossier forme un seul projet : les imports sont résolus entre ses fichiers.
    let reports = check_project(&sources);
    for (source, mut diags) in sources.into_iter().zip(reports) {
        let file = SourceFile::new(source.name, source.text);
        for diag in &mut diags {
            if opts.strict {
                diag.promote_to_error();
            }
            if diag.is_error() {
                errors += 1;
            } else {
                warnings += 1;
            }
            match opts.format {
                Format::Text => eprintln!("{}", file.render(diag)),
                Format::Json => json_diags.push(file.to_json(diag)),
            }
        }
    }

    match opts.format {
        Format::Text => {
            let status = if errors == 0 { "ok" } else { "échec" };
            eprintln!(
                "{status} : {} fichier(s), {errors} erreur(s), {warnings} avertissement(s){}",
                paths.len(),
                if opts.strict { " [strict]" } else { "" }
            );
        }
        Format::Json => {
            let out = json!({
                "format_version": 1,
                "strict": opts.strict,
                "files_checked": paths.len(),
                "errors": errors,
                "warnings": warnings,
                "diagnostics": json_diags,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&out).expect("JSON sérialisable")
            );
        }
    }
    if errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn describe(opts: &Options) -> ExitCode {
    match opts.positional.first().map(String::as_str) {
        Some("errors") => {
            if opts.format == Format::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(codes::ALL).expect("JSON sérialisable")
                );
            } else {
                for c in codes::ALL {
                    println!("{}  {:<36} {}", c.id, c.name, c.summary);
                }
            }
        }
        Some("keywords") => {
            let keywords: Vec<_> = mcf_syntax::lexer::KEYWORDS
                .iter()
                .map(|(k, _)| *k)
                .collect();
            let reserved = mcf_syntax::lexer::RESERVED;
            if opts.format == Format::Json {
                let out = json!({ "keywords": keywords, "reserved": reserved });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out).expect("JSON sérialisable")
                );
            } else {
                println!("Mots-clés : {}", keywords.join(", "));
                println!("Réservés  : {}", reserved.join(", "));
            }
        }
        _ => {
            eprintln!("mcf describe : `errors` ou `keywords` attendu");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
