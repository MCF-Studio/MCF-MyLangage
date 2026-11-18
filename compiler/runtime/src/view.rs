//! Interfaces `.uimcf` vivantes (phase 11) : un composant `ui` devient un arbre [`Node`], et les
//! événements de l'affichage (clic, saisie) déclenchent les actions du `.uimcf`.
//!
//! [`ViewSession`] garde entre deux événements les globales, les `state` des composants et les
//! abonnements `on X => f`. Chaque événement s'exécute **jusqu'au bout** (tâches lancées
//! comprises), puis la vue est recalculée en entier ; [`mcf_view::Renderer`] en déduit les
//! changements d'affichage. C'est un modèle **provisoire** : le choix définitif (D69) reste à
//! faire, et un événement qui dort (`sleep`) fige l'interface le temps qu'il dure.
//!
//! Une vue n'a pas d'effet : rien ici n'écrit dans les globales pendant le calcul de la vue. Les
//! appels de fonctions `.mcf` dans une expression restent permis (D55), la vérification des
//! effets de bord viendra avec D56.
//!
//! Identité (D61) : un composant est identifié par sa position (parent, nom, rang d'utilisation) ;
//! son `state` suit cette identité d'un calcul à l'autre.

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};

use mcf_diagnostics::{codes, Diagnostic, SourceFile, Span};
use mcf_syntax::ast::*;
use mcf_view::{Color, Event, Kind, Length, Node, Prop, Style, StyleValue, MAX_NODES};

use super::*;

/// Fichiers « sans code » qui portent les imports des `.uimcf` : à ajouter après les modules
/// `.mcf` pour que les imports d'une interface se résolvent comme ceux d'un `.mcf`.
pub fn ui_stub_modules(ui: &[&UiModule]) -> Vec<Module> {
    ui.iter()
        .map(|u| Module {
            items: u.imports.iter().cloned().map(Item::Import).collect(),
        })
        .collect()
}

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
    let stubs = ui_stub_modules(ui);
    let all: Vec<&Module> = modules.iter().copied().chain(stubs.iter()).collect();
    let mut session = ViewSession::new(&all, ui, files, dirs, root, component);
    session.view()
}

#[derive(Clone, Copy)]
struct Component<'v> {
    decl: &'v UiDecl,
    /// Indice du fichier dans la liste complète (code puis interfaces).
    file: usize,
}

#[derive(Clone)]
enum ActionKind<'v> {
    /// `bind value: x` : la saisie devient la valeur du `state` `x`.
    Bind(&'v str),
    /// Valeur d'un `onClick`… : appel, affectation d'un `state` ou `emit`.
    Run(&'v UiValue),
}

/// Action d'un élément, avec de quoi la rejouer plus tard : variables visibles au moment du
/// calcul de la vue (`prop`, `state`, variable de `foreach`) et composant d'origine.
#[derive(Clone)]
struct Action<'v> {
    /// Rang de l'élément dans l'arbre (parcours parent avant enfants).
    index: usize,
    event: Event,
    kind: ActionKind<'v>,
    blocks: Vec<Locals>,
    file: usize,
    table: usize,
    instance: String,
}

/// Ce que le calcul de la vue garde d'un calcul à l'autre.
struct ViewCx<'v> {
    components: HashMap<&'v str, Component<'v>>,
    ui: &'v [&'v UiModule],
    /// Nombre de fichiers `.mcf` : les interfaces viennent après.
    offset: usize,
    /// Composants en cours de calcul (le dernier est le courant).
    current: Vec<&'v UiDecl>,
    /// Instances en cours : identité par position et compteur d'utilisations de composants.
    instances: Vec<(String, usize)>,
    /// `state` de tous les composants affichés, par « instance/nom ».
    states: HashMap<String, Value>,
    actions: Vec<Action<'v>>,
    count: usize,
    /// Plusieurs éléments à la racine : un `Box` a été ajouté devant.
    wrapped: bool,
}

impl ViewCx<'_> {
    fn instance(&self) -> &str {
        self.instances.last().map_or("", |(k, _)| k.as_str())
    }

    fn state_key(&self, name: &str) -> String {
        format!("{}/{name}", self.instance())
    }
}

#[derive(Clone, Copy)]
enum Op<'x> {
    Render,
    Event {
        index: usize,
        event: Event,
        text: Option<&'x str>,
    },
}

enum OpOut {
    Tree(Node),
    Handled(bool),
}

/// Interface vivante : garde les globales, les `state` et les abonnements entre deux
/// événements (voir l'en-tête du module pour le modèle d'exécution).
pub struct ViewSession<'a> {
    program: Program<'a>,
    cx: ViewCx<'a>,
    globals: Vec<Option<Value>>,
    listeners: Vec<ListenerSlot>,
    initialized: bool,
    component: String,
    out: Vec<u8>,
    tree: Option<Node>,
}

impl<'a> ViewSession<'a> {
    /// `modules` : les `.mcf` puis les modules de [`ui_stub_modules`] (un par `.uimcf`) ;
    /// `files` et `dirs` dans le même ordre.
    pub fn new(
        modules: &[&'a Module],
        ui: &'a [&'a UiModule],
        files: &'a [SourceFile],
        dirs: &'a [PathBuf],
        root: &FsPath,
        component: &str,
    ) -> Self {
        let mut program = Program::build(modules, files, dirs);
        program.root = root.to_path_buf();
        let offset = modules.len() - ui.len();
        let mut cx = ViewCx {
            components: HashMap::new(),
            ui,
            offset,
            current: Vec::new(),
            instances: Vec::new(),
            states: HashMap::new(),
            actions: Vec::new(),
            count: 0,
            wrapped: false,
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
        ViewSession {
            globals: vec![None; program.globals.len()],
            program,
            cx,
            listeners: Vec::new(),
            initialized: false,
            component: component.to_string(),
            out: Vec::new(),
            tree: None,
        }
    }

    /// Calcule la vue avec l'état actuel.
    pub fn view(&mut self) -> Result<Node, Box<RuntimeError>> {
        match self.run_op(Op::Render)? {
            OpOut::Tree(node) => {
                self.tree = Some(node.clone());
                Ok(node)
            }
            OpOut::Handled(_) => Err(failure("calcul de la vue : résultat inattendu")),
        }
    }

    /// Traite un événement de l'élément désigné par `path` (indices d'enfants depuis la racine
    /// de la dernière vue calculée). `text` : saisie d'un `Input`. Rend `false` si l'élément
    /// n'écoute pas cet événement. Rappeler [`ViewSession::view`] ensuite.
    pub fn dispatch(
        &mut self,
        path: &[usize],
        event: Event,
        text: Option<&str>,
    ) -> Result<bool, Box<RuntimeError>> {
        let Some(tree) = &self.tree else {
            return Ok(false);
        };
        let Some(rank) = tree.preorder_index(path) else {
            return Ok(false);
        };
        // L'éventuel `Box` de la racine n'a pas été créé par une vue : il n'a pas d'action.
        let Some(index) = rank.checked_sub(usize::from(self.cx.wrapped)) else {
            return Ok(false);
        };
        match self.run_op(Op::Event { index, event, text })? {
            OpOut::Handled(h) => Ok(h),
            OpOut::Tree(_) => Err(failure("événement : résultat inattendu")),
        }
    }

    /// Ce que `print` a écrit depuis le dernier appel.
    pub fn take_output(&mut self) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut self.out)).into_owned()
    }

    fn run_op(&mut self, op: Op<'_>) -> Result<OpOut, Box<RuntimeError>> {
        let ViewSession {
            program,
            cx,
            globals,
            listeners,
            initialized,
            component,
            out,
            ..
        } = self;
        let program = &*program;
        // Un tour d'ordonnanceur sur un fil à grande pile (récursion des appels MCF) ; aucun
        // autre fil n'est créé : les tâches de l'événement sont des futures du même fil.
        std::thread::scope(|s| {
            std::thread::Builder::new()
                .name("mcf-view".into())
                .stack_size(STACK_SIZE)
                .spawn_scoped(s, || {
                    let shared = Shared {
                        world: RefCell::new(World {
                            out: &mut *out,
                            globals: std::mem::take(globals),
                            tasks: vec![TaskSlot::new()],
                            current: 0,
                            ready: VecDeque::new(),
                            timers: BTreeMap::new(),
                            seq: 0,
                            clock: 0,
                            failure: None,
                            aborted: false,
                            listeners: std::mem::take(listeners),
                        }),
                    };
                    let queue: Rc<Queue<'_>> = Rc::new(RefCell::new(Vec::new()));
                    let result: Rc<RefCell<Option<Res<OpOut>>>> = Rc::new(RefCell::new(None));
                    let (shared_ref, main_queue, main_result) =
                        (&shared, Rc::clone(&queue), Rc::clone(&result));
                    let main: TaskFuture<'_> = Box::pin(async move {
                        let mut it = Interp {
                            calls: Vec::new(),
                            p: program,
                            shared: shared_ref,
                            queue: main_queue,
                            id: 0,
                        };
                        let mut r = it.step(cx, initialized, component, op).await;
                        if r.is_ok() && matches!(op, Op::Event { .. }) {
                            // Les tâches lancées par l'événement vont jusqu'au bout.
                            let mut t = 1;
                            while t < it.w().tasks.len() {
                                if let Err(e) = it.wait_for(t, None).await {
                                    r = Err(e);
                                    break;
                                }
                                t += 1;
                            }
                        }
                        it.end_task(match &r {
                            Ok(_) => Ok(Value::Void),
                            Err(_) => Err(Stop::Abort),
                        });
                        *main_result.borrow_mut() = Some(r);
                    });
                    drive(&shared, &queue, main);
                    let mut w = shared.world.borrow_mut();
                    *globals = std::mem::take(&mut w.globals);
                    *listeners = std::mem::take(&mut w.listeners);
                    let failed = w.failure.take();
                    drop(w);
                    let outcome = result.borrow_mut().take();
                    match (outcome, failed) {
                        (Some(Err(Stop::Error(e))), _)
                        | (Some(Ok(_)), Some(e))
                        | (Some(Err(_)), Some(e)) => Err(e),
                        (Some(Err(_)), None) | (None, None) => {
                            Err(failure("le traitement a été interrompu"))
                        }
                        (None, Some(e)) => Err(e),
                        (Some(Ok(v)), None) => Ok(v),
                    }
                })
                .map_err(|e| failure(&format!("impossible de démarrer le calcul : {e}")))?
                .join()
                .unwrap_or_else(|_| Err(failure("le calcul s'est arrêté de façon inattendue")))
        })
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

impl<W: Write> Interp<'_, '_, '_, W> {
    /// Une opération de la session, avec le verrou de l'état partagé déjà pris.
    async fn step<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        initialized: &mut bool,
        component: &str,
        op: Op<'_>,
    ) -> Res<OpOut> {
        if !*initialized {
            self.init_globals().await?;
            *initialized = true;
        }
        match op {
            Op::Render => self.render(cx, component).await.map(OpOut::Tree),
            Op::Event { index, event, text } => {
                let mut found: Vec<Action<'v>> = cx
                    .actions
                    .iter()
                    .filter(|a| a.index == index && a.event == event)
                    .cloned()
                    .collect();
                // La liaison met à jour le `state` avant que `onChange` ne le lise.
                found.sort_by_key(|a| !matches!(a.kind, ActionKind::Bind(_)));
                for a in &found {
                    self.run_action(cx, a, text).await?;
                }
                Ok(OpOut::Handled(!found.is_empty()))
            }
        }
    }

    async fn render<'v>(&mut self, cx: &mut ViewCx<'v>, name: &str) -> Res<Node> {
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
        cx.count = 0;
        cx.actions.clear();
        cx.instances.clear();
        cx.instances.push(("racine".to_string(), 0));
        let nodes = self.component_nodes(cx, decl, file, Vec::new()).await;
        cx.instances.clear();
        let mut nodes = nodes?;
        cx.wrapped = nodes.len() != 1;
        if cx.wrapped {
            // Plusieurs éléments à la racine : un `Box` les réunit pour l'affichage.
            let mut root = Node::new(Kind::Box);
            root.children = nodes;
            Ok(root)
        } else {
            Ok(nodes.remove(0))
        }
    }

    /// Exécute l'action d'un élément avec les variables vues au moment du calcul de la vue.
    async fn run_action<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        a: &Action<'v>,
        text: Option<&str>,
    ) -> Res<()> {
        self.calls.push(CallFrame {
            name: "action d'interface".to_string(),
            file: a.file,
            table: a.table,
            called_at: None,
            blocks: a.blocks.clone(),
        });
        let r = self.action_body(cx, a, text).await;
        self.calls.pop();
        r
    }

    async fn action_body<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        a: &Action<'v>,
        text: Option<&str>,
    ) -> Res<()> {
        let key = |name: &str| format!("{}/{name}", a.instance);
        match &a.kind {
            ActionKind::Bind(name) => {
                let value = Value::Str(text.unwrap_or("").into());
                cx.states.insert(key(name), value);
            }
            ActionKind::Run(UiValue::Expr(e)) => {
                // `onClick: f` désigne une fonction sans argument ; `f(x)` est un appel.
                if let Some(Entity::Func(id)) = self.entity_of(e) {
                    self.call_func(id, None, Vec::new(), None).await?;
                } else {
                    self.eval(e).await?;
                }
            }
            ActionKind::Run(UiValue::Assign { target, value, .. }) => {
                let v = self.eval(value).await?;
                match &target.kind {
                    ExprKind::Name { ident } => {
                        cx.states.insert(key(&ident.name), v);
                    }
                    _ => return Err(self.internal(target.span, "cible d'affectation inconnue")),
                }
            }
            ActionKind::Run(UiValue::Emit { event, args, span }) => {
                let values = self.eval_all(args).await?;
                self.emit(event, values, &[], false, *span).await?;
            }
            ActionKind::Run(_) => {}
        }
        Ok(())
    }

    /// Calcule la vue d'un composant avec les valeurs de `prop` données (les autres prennent
    /// leur valeur par défaut) ; ses `state` gardent leur valeur d'un calcul à l'autre.
    async fn component_nodes<'v>(
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
        let result = self.component_body(cx, decl, args).await;
        self.calls.pop();
        result
    }

    async fn component_body<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        decl: &'v UiDecl,
        args: Vec<(String, Value)>,
    ) -> Res<Vec<Node>> {
        for m in &decl.members {
            let value = if m.kind == UiMemberKind::State {
                let key = cx.state_key(&m.name.name);
                match cx.states.get(&key) {
                    Some(v) => v.clone(),
                    None => {
                        let v = match &m.init {
                            Some(init) => self.eval(init).await?,
                            None => return Err(self.internal(m.span, "`state` sans valeur")),
                        };
                        cx.states.insert(key, v.clone());
                        v
                    }
                }
            } else {
                let given = args
                    .iter()
                    .find(|(n, _)| *n == m.name.name)
                    .map(|(_, v)| v.clone());
                match (given, &m.init) {
                    (Some(v), _) => v,
                    (None, Some(init)) => self.eval(init).await?,
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
                }
            };
            self.declare_local(&m.name.name, Some(value));
        }
        let Some(view) = &decl.view else {
            return Ok(Vec::new());
        };
        cx.current.push(decl);
        let nodes = self.view_nodes(cx, view).await;
        cx.current.pop();
        nodes
    }

    async fn view_nodes<'v>(
        &mut self,
        cx: &mut ViewCx<'v>,
        nodes: &'v [ViewNode],
    ) -> Res<Vec<Node>> {
        let mut out = Vec::new();
        for n in nodes {
            match n {
                ViewNode::Element(el) => out.extend(self.element(cx, el).await?),
                ViewNode::If(i) => {
                    if self.eval_bool(&i.cond).await? {
                        out.extend(Box::pin(self.scoped(cx, &i.then_nodes)).await?);
                    } else if let Some(e) = &i.else_nodes {
                        out.extend(Box::pin(self.scoped(cx, e)).await?);
                    }
                }
                ViewNode::Foreach(f) => {
                    let items: Vec<Value> = match self.eval(&f.iter).await? {
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
                        let r = Box::pin(self.view_nodes(cx, &f.body)).await;
                        self.pop_block();
                        out.extend(r?);
                    }
                }
            }
        }
        Ok(out)
    }

    async fn scoped<'v>(&mut self, cx: &mut ViewCx<'v>, nodes: &'v [ViewNode]) -> Res<Vec<Node>> {
        self.push_block();
        let r = self.view_nodes(cx, nodes).await;
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

    async fn text_of(&mut self, expr: &Expr) -> Res<String> {
        let v = self.eval(expr).await?;
        Ok(display(&v, &self.p.types))
    }

    fn record<'v>(&self, cx: &mut ViewCx<'v>, index: usize, event: Event, kind: ActionKind<'v>) {
        let frame = self.frame();
        cx.actions.push(Action {
            index,
            event,
            kind,
            blocks: frame.blocks.clone(),
            file: frame.file,
            table: frame.table,
            instance: cx.instance().to_string(),
        });
    }

    async fn element<'v>(&mut self, cx: &mut ViewCx<'v>, el: &'v Element) -> Res<Vec<Node>> {
        let Some(kind) = Kind::from_name(&el.name.name) else {
            return self.use_component(cx, el).await;
        };
        // Rang de l'élément : parent avant enfants, comme `Node::preorder_index`.
        let index = cx.count;
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
            node.props.insert(prop, self.text_of(expr).await?);
        }
        // Style nommé d'abord, propriétés de l'élément ensuite (D54).
        for p in el.props.iter().filter(|p| p.name.name == "style") {
            if let UiValue::Expr(Expr {
                kind: ExprKind::Name { ident },
                ..
            }) = &p.value
            {
                self.apply_named_style(cx, &ident.name, &mut node).await?;
            }
        }
        for p in &el.props {
            let name = p.name.name.as_str();
            if name == "style" {
                continue;
            }
            if let Some(event) = Event::from_name(name) {
                node.events.insert(event);
                self.record(cx, index, event, ActionKind::Run(&p.value));
            } else if let Some(prop) = Prop::from_name(name) {
                if let UiValue::Expr(e) = &p.value {
                    node.props.insert(prop, self.text_of(e).await?);
                    if p.bind {
                        if let ExprKind::Name { ident } = &e.kind {
                            node.events.insert(Event::Change);
                            self.record(cx, index, Event::Change, ActionKind::Bind(&ident.name));
                        }
                    }
                }
            } else if let Some(style) = Style::from_name(name) {
                let value = self.style_value(style, &p.value).await?;
                node.style.insert(style, value);
            }
        }
        if let Some(children) = &el.children {
            node.children = Box::pin(self.view_nodes(cx, children)).await?;
        }
        Ok(vec![node])
    }

    async fn use_component<'v>(&mut self, cx: &mut ViewCx<'v>, el: &'v Element) -> Res<Vec<Node>> {
        let Some(&Component { decl, file }) = cx.components.get(el.name.name.as_str()) else {
            return Err(self.internal(el.span, "composant inconnu dans une vue vérifiée"));
        };
        let mut args = Vec::new();
        for p in &el.props {
            if let UiValue::Expr(e) = &p.value {
                args.push((p.name.name.clone(), self.eval(e).await?));
            }
        }
        // Identité par position : parent, nom et rang d'utilisation dans ce parent.
        let child = {
            let (parent, counter) = cx.instances.last_mut().expect("instance en cours");
            *counter += 1;
            format!("{parent}/{}#{counter}", decl.name.name)
        };
        cx.instances.push((child, 0));
        let r = Box::pin(self.component_nodes(cx, decl, file, args)).await;
        cx.instances.pop();
        r
    }

    async fn apply_named_style<'v>(
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
                let value = self.style_value(style, &p.value).await?;
                node.style.insert(style, value);
            }
        }
        Ok(())
    }

    async fn style_value(&mut self, style: Style, value: &UiValue) -> Res<StyleValue> {
        let span = value.span();
        match style {
            Style::Background | Style::Color | Style::BorderColor => {
                let text = match value {
                    UiValue::Color { hex, .. } => format!("#{hex}"),
                    UiValue::Expr(e) => self.text_of(e).await?,
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
                let n = self.int_of(value).await?;
                if (100..=900).contains(&n) {
                    Ok(StyleValue::Weight(n as u16))
                } else {
                    Err(self.out_of_range(span, "`fontWeight`", "100 à 900", n))
                }
            }
            Style::Opacity => {
                let x = match value {
                    UiValue::Expr(e) => match self.eval(e).await? {
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
                let n = self.int_of(value).await?;
                if n >= 0 {
                    Ok(StyleValue::Grow(n))
                } else {
                    Err(self.out_of_range(span, "`grow`", "0 ou plus", n))
                }
            }
            Style::Visible => match value {
                UiValue::Expr(e) => match self.eval(e).await? {
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
                _ => Ok(StyleValue::Length(Length::Px(self.int_of(value).await?))),
            },
        }
    }

    async fn int_of(&mut self, value: &UiValue) -> Res<i64> {
        match value {
            UiValue::Expr(e) => match self.eval(e).await? {
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
