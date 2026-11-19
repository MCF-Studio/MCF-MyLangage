//! Interface vivante : un événement exécute l'action du `.uimcf`, puis la vue est recalculée.

use std::path::PathBuf;

use mcf_diagnostics::SourceFile;
use mcf_runtime::{ui_stub_modules, ViewSession};
use mcf_view::{Event, Kind, Node, Prop};

const CODE: &str = "
clics: Int = 0;
func ajouter(): Void { clics = clics + 1; }
";

const UI: &str = "
import clics;
import ajouter;

ui Page {
    state ouvert: Bool = false;
    state saisie: String = \"\";
    view {
        Box {
            Text(\"Clics : \" + clics);
            Button(\"+1\", onClick: ajouter);
            Button(\"Détails\", onClick: ouvert = !ouvert);
            Input(bind value: saisie);
            Text(\"Saisie : \" + saisie);
            if (ouvert) then { Text(\"Détails visibles\"); }
        }
    }
}
";

fn text_at(tree: &Node, path: &[usize]) -> String {
    let mut n = tree;
    for &i in path {
        n = &n.children[i];
    }
    n.props.get(&Prop::Text).cloned().unwrap_or_default()
}

#[test]
fn click_updates_globals_and_state() {
    let code = mcf_syntax::parse(CODE);
    let ui = mcf_syntax::parse_ui(UI);
    let ui_refs = [&ui.module];
    let stubs = ui_stub_modules(&ui_refs);
    let modules: Vec<_> = std::iter::once(&code.module).chain(stubs.iter()).collect();
    let files = [
        SourceFile::new("a.mcf", CODE),
        SourceFile::new("b.uimcf", UI),
    ];
    let dirs = [PathBuf::new(), PathBuf::new()];
    ViewSession::with(
        &modules,
        &ui_refs,
        &files,
        &dirs,
        &PathBuf::new(),
        "Page",
        |s| {
            let tree = s.view().unwrap();
            assert_eq!(tree.kind, Kind::Box);
            assert_eq!(text_at(&tree, &[0]), "Clics : 0");
            assert_eq!(tree.children.len(), 5);

            // Bouton « +1 » : fonction `.mcf` qui modifie une globale.
            assert!(s.dispatch(&[1], Event::Click, None).unwrap());
            assert!(s.dispatch(&[1], Event::Click, None).unwrap());
            let tree = s.view().unwrap();
            assert_eq!(text_at(&tree, &[0]), "Clics : 2");

            // Bouton « Détails » : affectation d'un `state`, qui fait apparaître un élément.
            assert!(s.dispatch(&[2], Event::Click, None).unwrap());
            let tree = s.view().unwrap();
            assert_eq!(tree.children.len(), 6);
            assert_eq!(text_at(&tree, &[5]), "Détails visibles");

            // Saisie liée à un `state`.
            assert!(s.dispatch(&[3], Event::Change, Some("abc")).unwrap());
            let tree = s.view().unwrap();
            assert_eq!(text_at(&tree, &[4]), "Saisie : abc");

            // Un élément qui n'écoute pas l'événement, ou un chemin inconnu : rien ne se passe.
            assert!(!s.dispatch(&[0], Event::Click, None).unwrap());
            assert!(!s.dispatch(&[9], Event::Click, None).unwrap());
        },
    );
}

const HOVER_UI: &str = "
ui Carte {
    state survol: Bool = false;
    view {
        Box(onEnter: survol = true, onLeave: survol = false) {
            Text(\"Carte\");
            if (survol) then { Text(\"Survolée\"); }
        }
    }
}
";

#[test]
fn hover_events_toggle_state() {
    let ui = mcf_syntax::parse_ui(HOVER_UI);
    let ui_refs = [&ui.module];
    let stubs = ui_stub_modules(&ui_refs);
    let modules: Vec<_> = stubs.iter().collect();
    let files = [SourceFile::new("c.uimcf", HOVER_UI)];
    let dirs = [PathBuf::new()];
    ViewSession::with(
        &modules,
        &ui_refs,
        &files,
        &dirs,
        &PathBuf::new(),
        "Carte",
        |s| {
            assert_eq!(s.view().unwrap().children.len(), 1);
            assert!(s.dispatch(&[], Event::Enter, None).unwrap());
            assert_eq!(s.view().unwrap().children.len(), 2);
            assert!(s.dispatch(&[], Event::Leave, None).unwrap());
            assert_eq!(s.view().unwrap().children.len(), 1);
            // Le clic n'est pas écouté par cet élément.
            assert!(!s.dispatch(&[], Event::Click, None).unwrap());
        },
    );
}

const SLEEP_CODE: &str = "
etat: String = \"avant\";
func lent(): Void { sleep(60); etat = \"après\"; }
";

const SLEEP_UI: &str = "
import etat;
import lent;

ui Page {
    view {
        Box {
            Text(etat);
            Button(\"Go\", onClick: lent);
        }
    }
}
";

#[test]
fn sleeping_action_does_not_freeze_the_session() {
    let code = mcf_syntax::parse(SLEEP_CODE);
    let ui = mcf_syntax::parse_ui(SLEEP_UI);
    let ui_refs = [&ui.module];
    let stubs = ui_stub_modules(&ui_refs);
    let modules: Vec<_> = std::iter::once(&code.module).chain(stubs.iter()).collect();
    let files = [
        SourceFile::new("a.mcf", SLEEP_CODE),
        SourceFile::new("b.uimcf", SLEEP_UI),
    ];
    let dirs = [PathBuf::new(), PathBuf::new()];
    ViewSession::with(
        &modules,
        &ui_refs,
        &files,
        &dirs,
        &PathBuf::new(),
        "Page",
        |s| {
            assert_eq!(text_at(&s.view().unwrap(), &[0]), "avant");

            // Le clic rend la main tout de suite : l'action dort, son effet n'a pas encore eu lieu.
            let started = std::time::Instant::now();
            assert!(s.dispatch(&[1], Event::Click, None).unwrap());
            assert!(started.elapsed() < std::time::Duration::from_millis(40));
            assert_eq!(text_at(&s.view().unwrap(), &[0]), "avant");
            assert!(!s.tick().unwrap(), "l'échéance n'est pas passée");

            // Une fois l'échéance passée, `tick` reprend l'action là où elle dormait.
            std::thread::sleep(std::time::Duration::from_millis(90));
            assert!(s.tick().unwrap());
            assert_eq!(text_at(&s.view().unwrap(), &[0]), "après");
            assert!(!s.tick().unwrap(), "plus rien en attente");
        },
    );
}

#[test]
fn error_in_a_resumed_action_is_reported_by_tick_and_session_survives() {
    let code = "
etat: String = \"avant\";
func casse(): Void { sleep(10); let x: Int = 1 / 0; etat = \"jamais\"; }
func repare(): Void { etat = \"réparé\"; }
";
    let ui_src = "
import etat;
import casse;
import repare;

ui Page {
    view {
        Box {
            Text(etat);
            Button(\"Casse\", onClick: casse);
            Button(\"Répare\", onClick: repare);
        }
    }
}
";
    let code_ast = mcf_syntax::parse(code);
    let ui = mcf_syntax::parse_ui(ui_src);
    let ui_refs = [&ui.module];
    let stubs = ui_stub_modules(&ui_refs);
    let modules: Vec<_> = std::iter::once(&code_ast.module)
        .chain(stubs.iter())
        .collect();
    let files = [
        SourceFile::new("a.mcf", code),
        SourceFile::new("b.uimcf", ui_src),
    ];
    let dirs = [PathBuf::new(), PathBuf::new()];
    ViewSession::with(
        &modules,
        &ui_refs,
        &files,
        &dirs,
        &PathBuf::new(),
        "Page",
        |s| {
            s.view().unwrap();
            assert!(s.dispatch(&[1], Event::Click, None).unwrap());
            std::thread::sleep(std::time::Duration::from_millis(40));
            assert!(s.tick().is_err(), "la division par zéro est rapportée");
            // La session repart : les autres actions fonctionnent.
            assert!(s.dispatch(&[2], Event::Click, None).unwrap());
            assert_eq!(text_at(&s.view().unwrap(), &[0]), "réparé");
        },
    );
}
