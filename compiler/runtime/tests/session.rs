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
    let mut s = ViewSession::new(&modules, &ui_refs, &files, &dirs, &PathBuf::new(), "Page");

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
    let mut s = ViewSession::new(&modules, &ui_refs, &files, &dirs, &PathBuf::new(), "Carte");

    assert_eq!(s.view().unwrap().children.len(), 1);
    assert!(s.dispatch(&[], Event::Enter, None).unwrap());
    assert_eq!(s.view().unwrap().children.len(), 2);
    assert!(s.dispatch(&[], Event::Leave, None).unwrap());
    assert_eq!(s.view().unwrap().children.len(), 1);
    // Le clic n'est pas écouté par cet élément.
    assert!(!s.dispatch(&[], Event::Click, None).unwrap());
}
