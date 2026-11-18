//! Calcul d'un instantané de vue (phase 11) : un composant `ui` devient un arbre [`Node`].
//!
//! Cette première étape évalue la vue **une fois**, avec les valeurs initiales des globales, des
//! `prop` (valeur par défaut) et des `state` (valeur initiale). Elle ne garde aucune session
//! ouverte et ne traite aucun événement : le modèle d'exécution d'une interface vivante (D69)
//! reste à trancher. Les actions (`onClick: f`) ne sont pas exécutées : l'arbre dit seulement
//! quels événements sont écoutés.
//!
//! Une vue n'a pas d'effet : rien ici n'écrit dans les globales ni ne lance de tâche. Les
//! appels de fonctions `.mcf` dans une expression restent permis (D55), la vérification des
//! effets de bord viendra avec D56.

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Mutex;

use mcf_diagnostics::{codes, Diagnostic, SourceFile, Span};
use mcf_syntax::ast::*;
use mcf_view::{Color, Event, Kind, Length, Node, Prop, Style, StyleValue, MAX_NODES};

use super::*;

/// Calcule la vue du composant `component` (valeurs initiales, sans événement).
///
/// `files` : les fichiers de `modules` d'abord, puis ceux de `ui` (même ordre).
pub fn render_view(
    modules: &[&Module],
    ui: &[&UiModule],
    files: &[SourceFile],
    dirs: &[PathBuf],
    root: &FsPath,
    component: &str,
) -> Result<Node, Box<RuntimeError>> {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .name("mcf-view".into())
            .stack_size(STACK_SIZE)
            .spawn_scoped(s, || render_here(modules, ui, files, dirs, root, component))
            .map_err(|e| failure(&format!("impossible de démarrer le calcul de la vue : {e}")))?
            .join()
            .unwrap_or_else(|_| {
                Err(failure(
                    "le calcul de la vue s'est arrêté de façon inattendue",
                ))
            })
    })
}

#[derive(Clone, Copy)]
struct Component<'v> {
    decl: &'v UiDecl,
    /// Indice du fichier dans la liste complète (code puis interfaces).
    file: usize,
}

/// Ce que le calcul de la vue a besoin de savoir, hors de l'interpréteur.
struct ViewCx<'v> {
    components: HashMap<&'v str, Component<'v>>,
    ui: &'v [&'v UiModule],
    /// Nombre de fichiers `.mcf` : les interfaces viennent après.
    offset: usize,
    /// Composants en cours de calcul (le dernier est le courant).
    current: Vec<&'v UiDecl>,
    count: usize,
}

fn render_here(
    modules: &[&Module],
    ui: &[&UiModule],
    files: &[SourceFile],
    dirs: &[PathBuf],
    root: &FsPath,
    component: &str,
) -> Result<Node, Box<RuntimeError>> {
    // Un fichier `.uimcf` est vu comme un fichier sans code qui n'a que ses imports : ils se
    // résolvent comme ceux d'un `.mcf`.
    let pseudo: Vec<Module> = ui
        .iter()
        .map(|u| Module {
            items: u.imports.iter().cloned().map(Item::Import).collect(),
        })
        .collect();
    let all: Vec<&Module> = modules.iter().copied().chain(pseudo.iter()).collect();
    let mut program = Program::build(&all, files, dirs);
    program.root = root.to_path_buf();
    let mut sink = std::io::sink();
    let shared = Shared {
        world: Mutex::new(World {
            out: &mut sink,
            globals: vec![None; program.globals.len()],
            tasks: vec![TaskSlot::new()],
            current: 0,
            ready: VecDeque::new(),
            timers: BTreeMap::new(),
            seq: 0,
            clock: 0,
            failure: None,
            aborted: false,
            listeners: Vec::new(),
        }),
    };
    let offset = modules.len();
    let mut cx = ViewCx {
        components: HashMap::new(),
        ui,
        offset,
        current: Vec::new(),
        count: 0,
    };
    for (i, u) in ui.iter().enumerate() {
        for decl in &u.components {
            cx.components.insert(
                decl.name.name.as_str(),
                Component {
                    decl,
                    file: offset + i,
                },
            );
        }
    }
    let outcome = std::thread::scope(|scope| {
        let mut it = Interp {
            calls: Vec::new(),
            guard: Some(shared.lock()),
            p: &program,
            shared: &shared,
            scope,
            id: 0,
        };
        let result = it.snapshot(&mut cx, component);
        drop(it.guard.take());
        result
    });
    match outcome {
        Ok(node) => Ok(node),
        Err(Stop::Error(e)) => Err(e),
        Err(_) => Err(failure("le calcul de la vue a été interrompu")),
    }
}

/// Propriété principale d'un élément (`Text("…")`).
fn main_prop(kind: Kind) -> Option<Prop> {
    match kind {
        Kind::Text => Some(Prop::Text),
        Kind::Image => Some(Prop::Source),
        Kind::Button => Some(Prop::Label),
        Kind::Input => Some(Prop::Value),
        Kind::Window | Kind::Box => None,
    }
}

impl<W: Write + Send> Interp<'_, '_, '_, '_, W> {
    fn snapshot<'v>(&mut self, cx: &mut ViewCx<'v>, name: &str) -> Res<Node> {
        self.init_globals()?;
        let Some(&Component { decl, file }) = cx.components.get(name) else {
            let mut names: Vec<&str> = cx.components.keys().copied().collect();
            names.sort_unstable();
            return Err(self.error_with(
                Diagnostic::new(
                    &codes::UNKNOWN_VIEW_COMPONENT,
                    Span::new(0, 0),
                    format!("aucun composant `ui {name}` dans le projet"),
                )
                .suggestion(if names.is_empty() {
                    "le projet n'a aucun fichier `.uimcf`".to_string()
                } else {
                    format!("composants disponibles : {}", names.join(", "))
                }),
            ));
        };
        let mut nodes = self.component_nodes(cx, decl, file, Vec::new())?;
        if nodes.len() == 1 {
            Ok(nodes.remove(0))
        } else {
            // Plusieurs éléments à la racine : un `Box` les réunit pour l'affichage.
            let mut root = Node::new(Kind::Box);
            root.children = nodes;
            Ok(root)
        }
    }

    /// Calcule la vue d'un composant avec les valeurs de `prop` données (les autres prennent
    /// leur valeur par défaut) et l'état initial.
    fn component_nodes<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        decl: &'v UiDecl,
        file: usize,
        args: Vec<(String, Value)>,
    ) -> Res<Vec<Node>> {
        if self.calls.len() >= MAX_CALL_DEPTH {
            return Err(self.error(
                &codes::STACK_OVERFLOW,
                decl.span,
                "composants imbriqués trop profondément",
            ));
        }
        self.calls.push(CallFrame {
            name: format!("vue `{}`", decl.name.name),
            file,
            table: self.p.roots[file],
            called_at: None,
            blocks: vec![Locals::new()],
        });
        let result = self.component_body(cx, decl, args);
        self.calls.pop();
        result
    }

    fn component_body<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        decl: &'v UiDecl,
        args: Vec<(String, Value)>,
    ) -> Res<Vec<Node>> {
        for m in &decl.members {
            let given = args
                .iter()
                .find(|(n, _)| *n == m.name.name)
                .map(|(_, v)| v.clone());
            let value = match (given, &m.init) {
                (Some(v), _) => v,
                (None, Some(init)) => self.eval(init)?,
                (None, None) => {
                    return Err(self.error_with(
                        Diagnostic::new(
                            &codes::UNKNOWN_VIEW_COMPONENT,
                            m.span,
                            format!("la `prop` `{}` n'a pas de valeur", m.name.name),
                        )
                        .suggestion(
                            "lui donner une valeur par défaut pour afficher le composant seul",
                        ),
                    ));
                }
            };
            self.declare_local(&m.name.name, Some(value));
        }
        let Some(view) = &decl.view else {
            return Ok(Vec::new());
        };
        cx.current.push(decl);
        let nodes = self.view_nodes(cx, view);
        cx.current.pop();
        nodes
    }

    fn view_nodes<'v>(&mut self, cx: &mut ViewCx<'v>, nodes: &[ViewNode]) -> Res<Vec<Node>> {
        let mut out = Vec::new();
        for n in nodes {
            match n {
                ViewNode::Element(el) => out.extend(self.element(cx, el)?),
                ViewNode::If(i) => {
                    if self.eval_bool(&i.cond)? {
                        out.extend(self.scoped(cx, &i.then_nodes)?);
                    } else if let Some(e) = &i.else_nodes {
                        out.extend(self.scoped(cx, e)?);
                    }
                }
                ViewNode::Foreach(f) => {
                    let items: Vec<Value> = match self.eval(&f.iter)? {
                        Value::Array(a) => a.borrow().clone(),
                        Value::Map(m) => m.borrow().iter().map(|(k, _)| k.clone()).collect(),
                        _ => {
                            return Err(self
                                .internal(f.iter.span, "`foreach` sur une valeur non parcourable"))
                        }
                    };
                    for item in items {
                        self.push_block();
                        self.declare_local(&f.binding.name, Some(item));
                        let r = self.view_nodes(cx, &f.body);
                        self.pop_block();
                        out.extend(r?);
                    }
                }
            }
        }
        Ok(out)
    }

    fn scoped<'v>(&mut self, cx: &mut ViewCx<'v>, nodes: &[ViewNode]) -> Res<Vec<Node>> {
        self.push_block();
        let r = self.view_nodes(cx, nodes);
        self.pop_block();
        r
    }

    fn push_block(&mut self) {
        self.calls
            .last_mut()
            .expect("appel en cours")
            .blocks
            .push(Locals::new());
    }

    fn pop_block(&mut self) {
        self.calls.last_mut().expect("appel en cours").blocks.pop();
    }

    fn text_of(&mut self, expr: &Expr) -> Res<String> {
        let v = self.eval(expr)?;
        Ok(display(&v, &self.p.types))
    }

    fn element<'v>(&mut self, cx: &mut ViewCx<'v>, el: &Element) -> Res<Vec<Node>> {
        let Some(kind) = Kind::from_name(&el.name.name) else {
            return self.use_component(cx, el);
        };
        cx.count += 1;
        if cx.count > MAX_NODES {
            return Err(self.error(
                &codes::VIEW_TOO_LARGE,
                el.span,
                format!("la vue dépasse {MAX_NODES} éléments"),
            ));
        }
        let mut node = Node::new(kind);
        if let (Some(prop), Some(expr)) = (main_prop(kind), el.positional.first()) {
            node.props.insert(prop, self.text_of(expr)?);
        }
        // Style nommé d'abord, propriétés de l'élément ensuite (D54).
        for p in el.props.iter().filter(|p| p.name.name == "style") {
            if let UiValue::Expr(Expr {
                kind: ExprKind::Name { ident },
                ..
            }) = &p.value
            {
                self.apply_named_style(cx, &ident.name, &mut node)?;
            }
        }
        for p in &el.props {
            let name = p.name.name.as_str();
            if name == "style" {
                continue;
            }
            if let Some(event) = Event::from_name(name) {
                node.events.insert(event);
            } else if let Some(prop) = Prop::from_name(name) {
                if let UiValue::Expr(e) = &p.value {
                    node.props.insert(prop, self.text_of(e)?);
                }
                if p.bind {
                    node.events.insert(Event::Change);
                }
            } else if let Some(style) = Style::from_name(name) {
                let value = self.style_value(style, &p.value)?;
                node.style.insert(style, value);
            }
        }
        if let Some(children) = &el.children {
            node.children = self.view_nodes(cx, children)?;
        }
        Ok(vec![node])
    }

    fn use_component<'v>(&mut self, cx: &mut ViewCx<'v>, el: &Element) -> Res<Vec<Node>> {
        let Some(&Component { decl, file }) = cx.components.get(el.name.name.as_str()) else {
            return Err(self.internal(el.span, "composant inconnu dans une vue vérifiée"));
        };
        let mut args = Vec::new();
        for p in &el.props {
            if let UiValue::Expr(e) = &p.value {
                args.push((p.name.name.clone(), self.eval(e)?));
            }
        }
        self.component_nodes(cx, decl, file, args)
    }

    fn apply_named_style<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        name: &str,
        node: &mut Node,
    ) -> Res<()> {
        let module = cx.ui[self.frame().file - cx.offset];
        let found = cx
            .current
            .last()
            .and_then(|d| d.styles.iter().find(|s| s.name.name == name))
            .or_else(|| module.styles.iter().find(|s| s.name.name == name));
        let Some(decl) = found else {
            return Err(self.internal(Span::new(0, 0), "style inconnu dans une vue vérifiée"));
        };
        for p in &decl.props {
            if let Some(style) = Style::from_name(&p.name.name) {
                let value = self.style_value(style, &p.value)?;
                node.style.insert(style, value);
            }
        }
        Ok(())
    }

    fn style_value(&mut self, style: Style, value: &UiValue) -> Res<StyleValue> {
        let span = value.span();
        match style {
            Style::Background | Style::Color | Style::BorderColor => {
                let text = match value {
                    UiValue::Color { hex, .. } => format!("#{hex}"),
                    UiValue::Expr(e) => self.text_of(e)?,
                    _ => return Err(self.internal(span, "couleur attendue")),
                };
                Color::parse(&text).map(StyleValue::Color).ok_or_else(|| {
                    self.error_with(
                        Diagnostic::new(
                            &codes::INVALID_VIEW_COLOR,
                            span,
                            format!("`{text}` n'est pas une couleur"),
                        )
                        .suggestion("écrire `#rgb`, `#rgba`, `#rrggbb` ou `#rrggbbaa`"),
                    )
                })
            }
            Style::FontWeight => {
                let n = self.int_of(value)?;
                if (100..=900).contains(&n) {
                    Ok(StyleValue::Weight(n as u16))
                } else {
                    Err(self.out_of_range(span, "`fontWeight`", "100 à 900", n))
                }
            }
            Style::Opacity => {
                let x = match value {
                    UiValue::Expr(e) => match self.eval(e)? {
                        Value::Int(n) => n as f64,
                        Value::Float(f) => f,
                        _ => return Err(self.internal(span, "nombre attendu")),
                    },
                    _ => return Err(self.internal(span, "nombre attendu")),
                };
                if (0.0..=1.0).contains(&x) {
                    Ok(StyleValue::Opacity(x))
                } else {
                    Err(self.out_of_range(span, "`opacity`", "0 à 1", x))
                }
            }
            Style::Grow => {
                let n = self.int_of(value)?;
                if n >= 0 {
                    Ok(StyleValue::Grow(n))
                } else {
                    Err(self.out_of_range(span, "`grow`", "0 ou plus", n))
                }
            }
            Style::Visible => match value {
                UiValue::Expr(e) => match self.eval(e)? {
                    Value::Bool(b) => Ok(StyleValue::Bool(b)),
                    _ => Err(self.internal(span, "booléen attendu")),
                },
                _ => Err(self.internal(span, "booléen attendu")),
            },
            Style::Layout | Style::Align | Style::Justify | Style::TextAlign | Style::Overflow => {
                match value {
                    UiValue::Expr(Expr {
                        kind: ExprKind::Name { ident },
                        ..
                    }) => Ok(StyleValue::Word(ident.name.clone())),
                    _ => Err(self.internal(span, "mot attendu")),
                }
            }
            // Toutes les autres propriétés sont des longueurs.
            _ => match value {
                UiValue::Percent { value, .. } => Ok(StyleValue::Length(Length::Percent(*value))),
                _ => Ok(StyleValue::Length(Length::Px(self.int_of(value)?))),
            },
        }
    }

    fn int_of(&mut self, value: &UiValue) -> Res<i64> {
        match value {
            UiValue::Expr(e) => match self.eval(e)? {
                Value::Int(n) => Ok(n),
                _ => Err(self.internal(e.span, "entier attendu")),
            },
            other => Err(self.internal(other.span(), "entier attendu")),
        }
    }

    fn out_of_range(
        &self,
        span: Span,
        what: &str,
        range: &str,
        got: impl std::fmt::Display,
    ) -> Stop {
        self.error_with(
            Diagnostic::new(
                &codes::VIEW_VALUE_OUT_OF_RANGE,
                span,
                format!("{what} vaut {got} : {range} attendu"),
            )
            .suggestion("borner la valeur calculée avant de l'utiliser"),
        )
    }
}
