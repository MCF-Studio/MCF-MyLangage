//! Vérification des fichiers `.uimcf` (phase 10). Règles : `docs/grammaire/interfaces.md`.
//!
//! Les expressions d'une interface sont vérifiées par l'analyseur MCF : les `prop` et `state`
//! d'un composant y sont déclarés comme des variables du bloc, les variables de `foreach`
//! comme des locales, et les imports du fichier comme ceux d'un `.mcf`. Les actions
//! d'événement sont vérifiées comme des instructions (`emit`, affectation) ou des appels.

use std::collections::{HashMap, HashSet};

use mcf_diagnostics::{codes, Diagnostic, Span};
use mcf_syntax::ast::*;

use crate::{root_name, Analyzer, Resolved, Sym, Ty};

/// Élément de base (D53).
struct Primitive {
    name: &'static str,
    /// Propriété principale, qui peut s'écrire sans nom en premier.
    main: Option<&'static str>,
    main_required: bool,
    /// Autres propriétés de type `String`.
    extra: &'static [&'static str],
    /// Événements propres à l'élément (en plus de [`COMMON_EVENTS`]).
    events: &'static [&'static str],
    children: bool,
}

const PRIMITIVES: &[Primitive] = &[
    Primitive {
        name: "Window",
        main: None,
        main_required: false,
        extra: &["title"],
        events: &[],
        children: true,
    },
    Primitive {
        name: "Box",
        main: None,
        main_required: false,
        extra: &[],
        events: &[],
        children: true,
    },
    Primitive {
        name: "Text",
        main: Some("text"),
        main_required: true,
        extra: &[],
        events: &[],
        children: false,
    },
    Primitive {
        name: "Image",
        main: Some("source"),
        main_required: true,
        extra: &["alt"],
        events: &[],
        children: false,
    },
    Primitive {
        name: "Button",
        main: Some("label"),
        main_required: true,
        extra: &[],
        events: &[],
        children: false,
    },
    Primitive {
        name: "Input",
        main: Some("value"),
        main_required: false,
        extra: &["placeholder"],
        events: &["onChange", "onSubmit"],
        children: false,
    },
];

const COMMON_EVENTS: &[&str] = &["onClick", "onEnter", "onLeave"];

fn primitive(name: &str) -> Option<&'static Primitive> {
    PRIMITIVES.iter().find(|p| p.name == name)
}

/// Sorte de valeur attendue par une propriété de style (D54).
#[derive(Clone, Copy)]
enum StyleKind {
    /// `Int` (pixels) ou `50%`.
    Length,
    /// `#rrggbb` ou `String`.
    Color,
    /// `Int` de 100 à 900.
    Weight,
    /// Nombre de 0 à 1.
    Opacity,
    /// `Int` positif.
    Grow,
    Bool,
    Words(&'static [&'static str]),
}

const STYLE_PROPERTIES: &[(&str, StyleKind)] = &[
    ("width", StyleKind::Length),
    ("height", StyleKind::Length),
    ("minWidth", StyleKind::Length),
    ("maxWidth", StyleKind::Length),
    ("minHeight", StyleKind::Length),
    ("maxHeight", StyleKind::Length),
    ("padding", StyleKind::Length),
    ("paddingTop", StyleKind::Length),
    ("paddingRight", StyleKind::Length),
    ("paddingBottom", StyleKind::Length),
    ("paddingLeft", StyleKind::Length),
    ("margin", StyleKind::Length),
    ("marginTop", StyleKind::Length),
    ("marginRight", StyleKind::Length),
    ("marginBottom", StyleKind::Length),
    ("marginLeft", StyleKind::Length),
    ("gap", StyleKind::Length),
    ("borderWidth", StyleKind::Length),
    ("radius", StyleKind::Length),
    ("fontSize", StyleKind::Length),
    ("x", StyleKind::Length),
    ("y", StyleKind::Length),
    ("background", StyleKind::Color),
    ("color", StyleKind::Color),
    ("borderColor", StyleKind::Color),
    ("fontWeight", StyleKind::Weight),
    ("opacity", StyleKind::Opacity),
    ("grow", StyleKind::Grow),
    ("visible", StyleKind::Bool),
    ("layout", StyleKind::Words(&["row", "column", "stack"])),
    (
        "align",
        StyleKind::Words(&["start", "center", "end", "stretch"]),
    ),
    (
        "justify",
        StyleKind::Words(&["start", "center", "end", "between", "around"]),
    ),
    ("textAlign", StyleKind::Words(&["left", "center", "right"])),
    (
        "overflow",
        StyleKind::Words(&["visible", "hidden", "scroll"]),
    ),
];

fn style_kind(name: &str) -> Option<StyleKind> {
    STYLE_PROPERTIES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
}

/// Composant du projet : fichier et déclaration.
type Components<'m> = HashMap<String, (usize, &'m UiDecl)>;

/// Utilisation d'un composant par un autre : (utilisateur, utilisé, sous `if`/`foreach`).
struct Use {
    from: String,
    to: String,
    conditional: bool,
}

/// Composant en cours de vérification.
struct Ctx<'c, 'm> {
    table: usize,
    name: String,
    members: HashMap<String, UiMemberKind>,
    /// Styles visibles : ceux du composant et ceux du fichier.
    styles: HashSet<String>,
    components: &'c Components<'m>,
    uses: &'c mut Vec<Use>,
    /// Profondeur sous `if` / `foreach`.
    conditional: usize,
}

impl Analyzer {
    /// `files` : (indice du fichier pour l'analyseur, contenu) de chaque `.uimcf` sans erreur
    /// de syntaxe. Leurs imports ont déjà été résolus comme ceux d'un `.mcf`.
    pub(crate) fn check_ui(&mut self, files: &[(usize, &UiModule)]) {
        let mut components: Components = HashMap::new();
        for &(file, m) in files {
            self.file = file;
            for c in &m.components {
                let name = &c.name.name;
                if primitive(name).is_some() {
                    self.report(
                        Diagnostic::new(
                            &codes::UI_NAME_CONFLICT,
                            c.name.span,
                            format!("`{name}` est un élément de base"),
                        )
                        .suggestion("choisir un autre nom pour ce composant"),
                    );
                } else if components.contains_key(name) {
                    self.report(
                        Diagnostic::new(
                            &codes::UI_NAME_CONFLICT,
                            c.name.span,
                            format!("le composant `{name}` est déjà déclaré dans le projet"),
                        )
                        .suggestion("un nom de composant est unique dans tout le projet"),
                    );
                } else {
                    components.insert(name.clone(), (file, c));
                }
            }
        }

        let mut uses = Vec::new();
        for &(file, m) in files {
            self.file = file;
            let table = self.roots[file];
            let mut file_styles = HashSet::new();
            for s in &m.styles {
                if !file_styles.insert(s.name.name.clone()) {
                    self.duplicate_style(&s.name);
                }
                self.check_style_decl(s, table);
            }
            for c in &m.components {
                let declared = components
                    .get(&c.name.name)
                    .is_some_and(|(f, d)| *f == file && std::ptr::eq(*d, c));
                if declared {
                    self.check_component(c, table, &file_styles, &components, &mut uses);
                }
            }
        }
        self.check_component_cycles(&components, &uses);
    }

    fn duplicate_style(&mut self, name: &Ident) {
        self.report(Diagnostic::new(
            &codes::UI_NAME_CONFLICT,
            name.span,
            format!("le style `{}` est déjà déclaré ici", name.name),
        ));
    }

    fn check_component<'m>(
        &mut self,
        c: &'m UiDecl,
        table: usize,
        file_styles: &HashSet<String>,
        components: &Components<'m>,
        uses: &mut Vec<Use>,
    ) {
        self.enter_body(Ty::Void);
        // Valeurs initiales vérifiées avant de déclarer les membres : elles ne lisent que
        // le programme.
        let mut types = Vec::new();
        for m in &c.members {
            let ty = self.type_decl(&m.ty, table, false, true);
            if let Some(init) = &m.init {
                self.forbid_logic(init);
                let v = self.check_expr(init, table);
                self.expect_assignable(&v.ty, &ty, init.span);
            }
            types.push(ty);
        }
        let mut members = HashMap::new();
        for (m, ty) in c.members.iter().zip(types) {
            let name = &m.name.name;
            if members.contains_key(name) {
                self.report(Diagnostic::new(
                    &codes::UI_NAME_CONFLICT,
                    m.name.span,
                    format!("`{name}` est déjà un membre de `{}`", c.name.name),
                ));
                continue;
            }
            if let Some(found) = self.lookup(name, table) {
                let what = match found {
                    Resolved::Sym(s) => s.describe(),
                    _ => "un autre nom",
                };
                self.report(
                    Diagnostic::new(
                        &codes::UI_NAME_CONFLICT,
                        m.name.span,
                        format!("`{name}` désigne déjà {what} visible dans ce fichier"),
                    )
                    .suggestion(
                        "choisir un autre nom : on doit toujours savoir ce que désigne un nom",
                    ),
                );
            }
            members.insert(name.clone(), m.kind);
            self.declare_local(&m.name, ty);
        }

        let mut styles = file_styles.clone();
        let mut local = HashSet::new();
        for s in &c.styles {
            if !local.insert(s.name.name.clone()) || file_styles.contains(&s.name.name) {
                self.duplicate_style(&s.name);
            }
            styles.insert(s.name.name.clone());
            self.check_style_decl(s, table);
        }

        let mut ctx = Ctx {
            table,
            name: c.name.name.clone(),
            members,
            styles,
            components,
            uses,
            conditional: 0,
        };
        match &c.view {
            Some(nodes) => self.check_nodes(nodes, &mut ctx, true),
            None => self.report(
                Diagnostic::new(
                    &codes::UI_MISSING_VIEW,
                    c.name.span,
                    format!("le composant `{}` n'a pas de `view`", c.name.name),
                )
                .suggestion("ajouter `view { … }` avec les éléments à afficher"),
            ),
        }
        self.leave_body();
    }

    fn check_style_decl(&mut self, s: &StyleDecl, table: usize) {
        let mut seen = HashSet::new();
        for p in &s.props {
            if !seen.insert(p.name.name.as_str()) {
                self.duplicate_property(p);
                continue;
            }
            match style_kind(&p.name.name) {
                Some(kind) => self.check_style_value(p, kind, table),
                None => {
                    let hint = if p.name.name.starts_with("on") {
                        Some("un événement s'écrit sur l'élément, pas dans un style")
                    } else {
                        None
                    };
                    let candidates: Vec<&str> = STYLE_PROPERTIES.iter().map(|(n, _)| *n).collect();
                    self.unknown_property(p, "un style", &candidates, hint);
                }
            }
        }
    }

    fn duplicate_property(&mut self, p: &UiProp) {
        self.report(
            Diagnostic::new(
                &codes::UI_DUPLICATE_PROPERTY,
                p.name.span,
                format!("`{}` est déjà donnée plus haut", p.name.name),
            )
            .suggestion("garder une seule valeur"),
        );
    }

    fn unknown_property(
        &mut self,
        p: &UiProp,
        owner: &str,
        candidates: &[&str],
        hint: Option<&str>,
    ) {
        let mut d = Diagnostic::new(
            &codes::UI_UNKNOWN_PROPERTY,
            p.name.span,
            format!("`{}` n'est pas une propriété de {owner}", p.name.name),
        );
        if let Some(h) = hint {
            d = d.suggestion(h.to_string());
        } else if let Some(c) = mcf_config::closest(&p.name.name, candidates.iter().copied()) {
            d = d.suggestion(format!("vouliez-vous dire `{c}` ?"));
        }
        // La valeur n'est pas vérifiée : sans propriété connue, ses erreurs ne seraient que du bruit.
        self.report(d);
    }

    // ---------------------------------------------------------------- vue

    fn check_nodes(&mut self, nodes: &[ViewNode], ctx: &mut Ctx, root: bool) {
        for n in nodes {
            match n {
                ViewNode::Element(e) => self.check_element(e, ctx, root && nodes.len() == 1),
                ViewNode::If(i) => {
                    self.forbid_logic(&i.cond);
                    self.check_condition(&i.cond, ctx.table);
                    let (yes, no) = self.narrowings(&i.cond, ctx.table);
                    ctx.conditional += 1;
                    self.with_narrowings(yes, |a| a.check_nodes(&i.then_nodes, ctx, false));
                    if let Some(e) = &i.else_nodes {
                        self.with_narrowings(no, |a| a.check_nodes(e, ctx, false));
                    }
                    ctx.conditional -= 1;
                }
                ViewNode::Foreach(f) => {
                    self.forbid_logic(&f.iter);
                    let it = self.check_expr(&f.iter, ctx.table);
                    let element = match &it.ty {
                        Ty::Array(e) => (**e).clone(),
                        Ty::Unknown | Ty::Map(..) => Ty::Unknown,
                        other => {
                            self.report(
                                Diagnostic::new(
                                    &codes::TYPE_MISMATCH,
                                    f.iter.span,
                                    "`foreach` parcourt un tableau",
                                )
                                .expected("Array<T>")
                                .received(other.to_string()),
                            );
                            Ty::Unknown
                        }
                    };
                    let ty = match &f.ty {
                        Some(t) => {
                            let declared = self.type_decl(t, ctx.table, false, true);
                            self.expect_assignable(&element, &declared, f.iter.span);
                            declared
                        }
                        None => element,
                    };
                    ctx.conditional += 1;
                    self.with_narrowings(Vec::new(), |a| {
                        a.declare_local(&f.binding, ty);
                        a.check_nodes(&f.body, ctx, false);
                    });
                    ctx.conditional -= 1;
                }
            }
        }
    }

    fn check_element(&mut self, e: &Element, ctx: &mut Ctx, root: bool) {
        let name = e.name.name.as_str();
        let children_allowed = if let Some(p) = primitive(name) {
            if p.name == "Window" && !root {
                self.report(
                    Diagnostic::new(
                        &codes::UI_WINDOW_NOT_ROOT,
                        e.name.span,
                        "`Window` doit être le seul élément à la racine de la `view`",
                    )
                    .suggestion("pour un cadre dans une fenêtre, utiliser `Box`"),
                );
            }
            self.check_primitive(e, p, ctx);
            p.children
        } else if let Some(&(file, decl)) = ctx.components.get(name) {
            ctx.uses.push(Use {
                from: ctx.name.clone(),
                to: name.to_string(),
                conditional: ctx.conditional > 0,
            });
            self.check_component_use(e, decl, file, ctx);
            false
        } else {
            let mut d = Diagnostic::new(
                &codes::UI_UNKNOWN_ELEMENT,
                e.name.span,
                format!("`{name}` n'est ni un élément de base ni un composant `ui` du projet"),
            );
            let candidates: Vec<&str> = PRIMITIVES
                .iter()
                .map(|p| p.name)
                .chain(ctx.components.keys().map(String::as_str))
                .collect();
            if let Some(c) = mcf_config::closest(name, candidates.into_iter()) {
                d = d.suggestion(format!("vouliez-vous dire `{c}` ?"));
            } else if matches!(
                self.lookup(name, ctx.table),
                Some(Resolved::Sym(Sym::Struct { .. }))
            ) {
                d = d.suggestion("une structure n'est pas un élément : déclarer un composant `ui`");
            } else {
                d = d.suggestion("éléments de base : Window, Box, Text, Image, Button, Input");
            }
            self.report(d);
            true
        };
        if let Some(children) = &e.children {
            if !children_allowed {
                self.report(
                    Diagnostic::new(
                        &codes::UI_CHILDREN_NOT_ALLOWED,
                        e.name.span,
                        format!("`{name}` ne contient pas d'autres éléments"),
                    )
                    .suggestion(format!(
                        "terminer par `;`, ou regrouper dans un `Box` : `Box {{ {name}(…); … }}`"
                    )),
                );
            }
            self.check_nodes(children, ctx, false);
        }
    }

    fn check_primitive(&mut self, e: &Element, p: &Primitive, ctx: &mut Ctx) {
        let mut given: HashSet<&str> = HashSet::new();
        for (i, value) in e.positional.iter().enumerate() {
            self.forbid_logic(value);
            match p.main {
                Some(main) if i == 0 => {
                    given.insert(main);
                    self.check_typed(value, &Ty::String, main, ctx.table);
                }
                _ => {
                    self.positional_refused(value, p.main, ctx.table);
                }
            }
        }
        for prop in &e.props {
            let n = prop.name.name.as_str();
            if !given.insert(n) {
                self.duplicate_property(prop);
                continue;
            }
            if prop.bind {
                self.check_bind(prop, p, ctx);
            } else if n == "style" {
                self.check_style_ref(prop, ctx);
            } else if COMMON_EVENTS.contains(&n) || p.events.contains(&n) {
                self.check_action(&prop.value, ctx);
            } else if p.main == Some(n) || p.extra.contains(&n) {
                self.check_value(&prop.value, &Ty::String, n, ctx.table);
            } else if let Some(kind) = style_kind(n) {
                self.check_style_value(prop, kind, ctx.table);
            } else {
                let candidates: Vec<&str> = p
                    .main
                    .iter()
                    .chain(p.extra)
                    .chain(COMMON_EVENTS)
                    .chain(p.events)
                    .copied()
                    .chain(["style"])
                    .chain(STYLE_PROPERTIES.iter().map(|(n, _)| *n))
                    .collect();
                let hint = (n == "onChange" || n == "onSubmit")
                    .then_some("seul `Input` a `onChange` et `onSubmit`");
                self.unknown_property(prop, &format!("`{}`", p.name), &candidates, hint);
            }
        }
        if let Some(main) = p.main {
            if p.main_required && !given.contains(main) {
                self.report(
                    Diagnostic::new(
                        &codes::UI_MISSING_PROPERTY,
                        e.name.span,
                        format!("`{}` a besoin de `{main}`", p.name),
                    )
                    .suggestion(format!("`{}(\"…\")` ou `{}({main}: …)`", p.name, p.name)),
                );
            }
        }
    }

    fn positional_refused(&mut self, value: &Expr, main: Option<&str>, table: usize) {
        let why = match main {
            Some(m) => format!("seule la propriété principale `{m}` s'écrit sans nom, en premier"),
            None => "cet élément n'a pas de propriété principale : chaque valeur se nomme".into(),
        };
        self.report(
            Diagnostic::new(&codes::UI_POSITIONAL_ARGUMENT, value.span, why)
                .suggestion("nommer la valeur : `propriété: valeur`"),
        );
        self.check_expr(value, table);
    }

    fn check_component_use(&mut self, e: &Element, decl: &UiDecl, decl_file: usize, ctx: &mut Ctx) {
        for value in &e.positional {
            self.forbid_logic(value);
            self.positional_refused(value, None, ctx.table);
        }
        let props: Vec<&UiMember> = decl
            .members
            .iter()
            .filter(|m| m.kind == UiMemberKind::Prop)
            .collect();
        let mut given: HashSet<&str> = HashSet::new();
        for prop in &e.props {
            let n = prop.name.name.as_str();
            if !given.insert(n) {
                self.duplicate_property(prop);
                continue;
            }
            if prop.bind {
                self.report(
                    Diagnostic::new(
                        &codes::UI_INVALID_BIND,
                        prop.span,
                        "seule la propriété `value` d'un `Input` se lie avec `bind`",
                    )
                    .suggestion(format!("`{n}: valeur`")),
                );
                continue;
            }
            match decl.members.iter().find(|m| m.name.name == n) {
                Some(m) if m.kind == UiMemberKind::Prop => {
                    let ty = self.type_decl(&m.ty, self.roots[decl_file], false, false);
                    self.check_value(&prop.value, &ty, n, ctx.table);
                }
                Some(_) => {
                    self.report(
                        Diagnostic::new(
                            &codes::UI_UNKNOWN_PROPERTY,
                            prop.name.span,
                            format!(
                                "`{n}` est un `state` interne de `{}` : il ne se donne pas de l'extérieur",
                                decl.name.name
                            ),
                        )
                        .suggestion("le déclarer avec `prop` pour qu'il soit reçu"),
                    );
                }
                None => {
                    let candidates: Vec<&str> =
                        props.iter().map(|m| m.name.name.as_str()).collect();
                    let owner = format!("`{}`", decl.name.name);
                    let hint = (style_kind(n).is_some() || n == "style" || n.starts_with("on"))
                        .then_some("un composant reçoit seulement ses `prop` : le style et les événements se mettent sur les éléments de sa `view`");
                    self.unknown_property(prop, &owner, &candidates, hint);
                }
            }
        }
        for m in props {
            if m.init.is_none() && !given.contains(m.name.name.as_str()) {
                self.report(
                    Diagnostic::new(
                        &codes::UI_MISSING_PROPERTY,
                        e.name.span,
                        format!("`{}` a besoin de `{}`", decl.name.name, m.name.name),
                    )
                    .suggestion(format!("`{}({}: …)`", decl.name.name, m.name.name)),
                );
            }
        }
    }

    // ---------------------------------------------------------------- valeurs

    /// Expression du type `ty`.
    fn check_typed(&mut self, e: &Expr, ty: &Ty, what: &str, table: usize) {
        let t = self.check_expr(e, table).ty;
        if !(t.is_unknown() || ty.is_unknown() || t.assignable_to(ty)) {
            self.report(
                Diagnostic::new(
                    &codes::UI_WRONG_VALUE,
                    e.span,
                    format!("`{what}` attend une valeur `{ty}`"),
                )
                .expected(ty.to_string())
                .received(t.to_string()),
            );
        }
    }

    fn check_value(&mut self, v: &UiValue, ty: &Ty, what: &str, table: usize) {
        match v {
            UiValue::Expr(e) => {
                self.forbid_logic(e);
                self.check_typed(e, ty, what, table);
            }
            other => self.wrong_value(other, &format!("`{what}` attend une valeur `{ty}`")),
        }
    }

    fn wrong_value(&mut self, v: &UiValue, message: &str) {
        let received = match v {
            UiValue::Expr(_) => "une expression",
            UiValue::Percent { .. } => "un pourcentage",
            UiValue::Color { .. } => "une couleur",
            UiValue::Emit { .. } | UiValue::Assign { .. } => "une action d'événement",
        };
        self.report(
            Diagnostic::new(&codes::UI_WRONG_VALUE, v.span(), message.to_string())
                .received(received),
        );
    }

    fn check_style_value(&mut self, p: &UiProp, kind: StyleKind, table: usize) {
        let name = p.name.name.as_str();
        let expr = match (&p.value, kind) {
            (UiValue::Percent { .. }, StyleKind::Length)
            | (UiValue::Color { .. }, StyleKind::Color) => {
                return;
            }
            (UiValue::Expr(e), _) => e,
            (other, _) => {
                let message = match kind {
                    StyleKind::Length => {
                        format!("`{name}` attend une longueur : `16` (pixels) ou `50%`")
                    }
                    StyleKind::Color => {
                        format!("`{name}` attend une couleur : `#1e1e2e` ou une `String`")
                    }
                    _ => format!("`{name}` n'accepte pas cette valeur"),
                };
                self.wrong_value(other, &message);
                return;
            }
        };
        if let StyleKind::Words(words) = kind {
            let ok = matches!(&expr.kind, ExprKind::Name { ident } if words.contains(&ident.name.as_str()));
            if !ok {
                let mut d = Diagnostic::new(
                    &codes::UI_WRONG_VALUE,
                    expr.span,
                    format!("`{name}` accepte : {}", words.join(", ")),
                )
                .expected(words.join(" | "));
                if let ExprKind::Name { ident } = &expr.kind {
                    if let Some(c) = mcf_config::closest(&ident.name, words.iter().copied()) {
                        d = d.suggestion(format!("vouliez-vous dire `{c}` ?"));
                    }
                }
                self.report(d);
            }
            return;
        }
        self.forbid_logic(expr);
        let (ty, range): (Ty, Option<(f64, f64)>) = match kind {
            StyleKind::Length => (Ty::Int, None),
            StyleKind::Color => (Ty::String, None),
            StyleKind::Weight => (Ty::Int, Some((100.0, 900.0))),
            StyleKind::Opacity => (
                Ty::Dual(Box::new(Ty::Int), Box::new(Ty::Float)),
                Some((0.0, 1.0)),
            ),
            StyleKind::Grow => (Ty::Int, Some((0.0, f64::INFINITY))),
            StyleKind::Bool => (Ty::Bool, None),
            StyleKind::Words(_) => unreachable!("traité plus haut"),
        };
        let t = self.check_expr(expr, table).ty;
        let fits = t.is_unknown()
            || match kind {
                StyleKind::Opacity => t.is_numeric() || t == ty,
                _ => t.assignable_to(&ty),
            };
        if !fits {
            let expected = match kind {
                StyleKind::Length => "Int (pixels) ou un pourcentage `50%`".to_string(),
                StyleKind::Color => "une couleur `#rrggbb` ou une `String`".to_string(),
                StyleKind::Opacity => "Int ou Float".to_string(),
                _ => ty.to_string(),
            };
            self.report(
                Diagnostic::new(
                    &codes::UI_WRONG_VALUE,
                    expr.span,
                    format!("`{name}` attend {expected}"),
                )
                .expected(expected)
                .received(t.to_string()),
            );
            return;
        }
        if let (Some((min, max)), Some(n)) = (range, literal_number(expr)) {
            if n < min || n > max {
                let plage = if max.is_infinite() {
                    format!("au moins {min}")
                } else {
                    format!("de {min} à {max}")
                };
                self.report(
                    Diagnostic::new(
                        &codes::UI_VALUE_OUT_OF_RANGE,
                        expr.span,
                        format!("`{name}` va {plage}"),
                    )
                    .expected(plage)
                    .received(n.to_string()),
                );
            }
        }
        if let (StyleKind::Color, ExprKind::Str { value }) = (kind, &expr.kind) {
            if !is_color(value) {
                self.report(
                    Diagnostic::new(
                        &codes::UI_WRONG_VALUE,
                        expr.span,
                        format!("`\"{value}\"` n'est pas une couleur"),
                    )
                    .suggestion("écrire la couleur directement : `#1e1e2e` (`#rgb`, `#rgba`, `#rrggbb` ou `#rrggbbaa`)"),
                );
            }
        }
    }

    fn check_style_ref(&mut self, p: &UiProp, ctx: &Ctx) {
        let UiValue::Expr(Expr {
            kind: ExprKind::Name { ident },
            ..
        }) = &p.value
        else {
            self.wrong_value(
                &p.value,
                "`style` attend le nom d'un style (`style: carte`)",
            );
            return;
        };
        if !ctx.styles.contains(&ident.name) {
            let mut d = Diagnostic::new(
                &codes::UI_UNKNOWN_STYLE,
                ident.span,
                format!(
                    "aucun style `{}` dans ce composant ni dans ce fichier",
                    ident.name
                ),
            );
            d = match mcf_config::closest(&ident.name, ctx.styles.iter().map(String::as_str)) {
                Some(c) => d.suggestion(format!("vouliez-vous dire `{c}` ?")),
                None => d.suggestion(format!("déclarer `style {} {{ … }}`", ident.name)),
            };
            self.report(d);
        }
    }

    // ---------------------------------------------------------------- liaison avec le code

    /// Action d'un événement (D55) : appel d'une fonction MCF sans résultat, `emit`, ou
    /// affectation d'un `state` du composant.
    fn check_action(&mut self, v: &UiValue, ctx: &mut Ctx) {
        let table = ctx.table;
        match v {
            UiValue::Expr(e) => {
                self.forbid_logic(e);
                if matches!(
                    e.kind,
                    ExprKind::Start { .. } | ExprKind::Await { .. } | ExprKind::Listen { .. }
                ) {
                    return; // déjà refusé : rien de plus à dire
                }
                let call = match &e.kind {
                    ExprKind::Call { .. } => e.clone(),
                    ExprKind::Name { .. } | ExprKind::Member { .. } => Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(e.clone()),
                            args: Vec::new(),
                        },
                        span: e.span,
                    },
                    _ => {
                        self.invalid_action(e.span, "un événement déclenche une action, pas une valeur");
                        self.check_expr(e, table);
                        return;
                    }
                };
                let ExprKind::Call { callee, .. } = &call.kind else {
                    return;
                };
                let target = expr_path(callee).and_then(|p| self.resolve_path(&p, table));
                let declared = expr_path(callee).is_some_and(|p| {
                    self.lookup(&p.segments[0].name, table).is_some()
                });
                if declared && !matches!(target, Some(Sym::Func { .. })) {
                    self.invalid_action(
                        callee.span,
                        "seule une fonction du programme (`.mcf`) peut être appelée par un événement",
                    );
                    return;
                }
                let t = self.check_expr(&call, table).ty;
                if !matches!(t, Ty::Void | Ty::Unknown) {
                    self.invalid_action(
                        call.span,
                        &format!("la fonction retourne une valeur (`{t}`) qui serait perdue"),
                    );
                }
            }
            UiValue::Emit { event, args, span } => {
                for a in args {
                    self.forbid_logic(a);
                }
                self.check_stmt(
                    &Stmt::Emit {
                        event: event.clone(),
                        args: args.clone(),
                        meta: Vec::new(),
                        background: false,
                        span: *span,
                    },
                    table,
                );
            }
            UiValue::Assign {
                target,
                value,
                span,
            } => {
                self.forbid_logic(target);
                self.forbid_logic(value);
                let kind = root_name(target).and_then(|n| ctx.members.get(n)).copied();
                match kind {
                    Some(UiMemberKind::State) => self.check_stmt(
                        &Stmt::Assign {
                            target: target.clone(),
                            value: value.clone(),
                            span: *span,
                        },
                        table,
                    ),
                    Some(UiMemberKind::Prop) => self.invalid_action(
                        target.span,
                        "une `prop` se lit seulement : elle appartient à celui qui utilise le composant",
                    ),
                    None => self.invalid_action(
                        target.span,
                        "seul un `state` du composant se modifie dans un événement ; une variable du programme se modifie dans une fonction `.mcf`",
                    ),
                }
            }
            other => self.invalid_action(other.span(), "un événement attend une action : `fonction`, `emit Evenement(…)` ou `etat = valeur`"),
        }
    }

    fn invalid_action(&mut self, span: Span, message: &str) {
        self.report(
            Diagnostic::new(&codes::UI_INVALID_ACTION, span, message.to_string()).suggestion(
                "`onClick: fonction`, `onClick: fonction(x)`, `onClick: emit Evenement(x)` ou `onClick: etat = valeur`",
            ),
        );
    }

    /// `bind value: cible` sur un `Input` : la cible est un `state` `String` du composant (D55).
    fn check_bind(&mut self, p: &UiProp, prim: &Primitive, ctx: &Ctx) {
        let bad = |a: &mut Self, span: Span, message: &str| {
            a.report(
                Diagnostic::new(&codes::UI_INVALID_BIND, span, message.to_string()).suggestion(
                    "`Input(bind value: recherche)` avec `state recherche: String = \"\";`",
                ),
            );
        };
        if prim.name != "Input" || p.name.name != "value" {
            bad(
                self,
                p.span,
                "seule la propriété `value` d'un `Input` se lie avec `bind`",
            );
            return;
        }
        let UiValue::Expr(Expr {
            kind: ExprKind::Name { ident },
            ..
        }) = &p.value
        else {
            bad(
                self,
                p.value.span(),
                "la cible d'un `bind` est un nom de variable",
            );
            return;
        };
        let name = ident.name.as_str();
        let ty = match ctx.members.get(name) {
            Some(UiMemberKind::Prop) => {
                bad(
                    self,
                    ident.span,
                    "une `prop` se lit seulement : la lier la modifierait",
                );
                return;
            }
            Some(UiMemberKind::State) => self.lookup_typed(name, ctx.table).map(|(_, t)| t),
            // D55 : l'interface n'écrit jamais directement dans l'état du programme ; une
            // variable du programme change dans une fonction `.mcf` (`onChange: f(saisie)`).
            None => {
                if self.lookup_typed(name, ctx.table).is_none() {
                    self.undeclared(ident);
                } else if self.is_config_name(name, ctx.table) {
                    bad(
                        self,
                        ident.span,
                        &format!(
                            "`{name}` vient d'un fichier de configuration : elle se lit seulement"
                        ),
                    );
                } else {
                    bad(
                        self,
                        ident.span,
                        &format!("`{name}` n'est pas un `state` du composant : seul un `state` se lie avec `bind`, une variable du programme se modifie dans une fonction `.mcf` (`onChange: fonction(saisie)`)"),
                    );
                }
                return;
            }
        };
        if let Some(t) = ty {
            if !(t.is_unknown() || t == Ty::String) {
                self.report(
                    Diagnostic::new(
                        &codes::UI_INVALID_BIND,
                        ident.span,
                        format!("`value` d'un `Input` est une `String`, `{name}` est `{t}`"),
                    )
                    .expected("String")
                    .received(t.to_string()),
                );
            }
        }
    }

    /// `start`, `await` et `on X => f` sont de la logique : refusés dans un `.uimcf`.
    fn forbid_logic(&mut self, e: &Expr) {
        let refused = match &e.kind {
            ExprKind::Start { .. } => Some("`start`"),
            ExprKind::Await { .. } => Some("`await`"),
            ExprKind::Listen { .. } => Some("un abonnement `on … =>`"),
            _ => None,
        };
        if let Some(what) = refused {
            self.report(
                Diagnostic::new(
                    &codes::UI_LOGIC_NOT_ALLOWED,
                    e.span,
                    format!("{what} est de la logique : pas dans un `.uimcf`"),
                )
                .suggestion("le faire dans une fonction `.mcf` appelée par un événement"),
            );
            return;
        }
        match &e.kind {
            ExprKind::Unary { expr, .. } | ExprKind::Paren { expr } | ExprKind::Is { expr, .. } => {
                self.forbid_logic(expr)
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.forbid_logic(lhs);
                self.forbid_logic(rhs);
            }
            ExprKind::Call { callee, args } => {
                self.forbid_logic(callee);
                for a in args {
                    self.forbid_logic(a);
                }
            }
            ExprKind::Member { object, .. } => self.forbid_logic(object),
            ExprKind::Index { object, index } => {
                self.forbid_logic(object);
                self.forbid_logic(index);
            }
            ExprKind::Array { items } => {
                for i in items {
                    self.forbid_logic(i);
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    self.forbid_logic(&f.value);
                }
            }
            _ => {}
        }
    }

    /// Un composant qui s'inclut lui-même sans `if` ni `foreach` sur le chemin ne
    /// finirait jamais de s'afficher.
    fn check_component_cycles(&mut self, components: &Components, uses: &[Use]) {
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
        for u in uses.iter().filter(|u| !u.conditional) {
            graph.entry(&u.from).or_default().push(&u.to);
        }
        let mut names: Vec<&String> = components.keys().collect();
        names.sort();
        for name in names {
            let mut stack: Vec<&str> = graph.get(name.as_str()).cloned().unwrap_or_default();
            let mut seen = HashSet::new();
            let mut cyclic = false;
            while let Some(n) = stack.pop() {
                if n == name {
                    cyclic = true;
                    break;
                }
                if seen.insert(n) {
                    stack.extend(graph.get(n).into_iter().flatten());
                }
            }
            if cyclic {
                let (file, decl) = components[name];
                self.file = file;
                self.report(
                    Diagnostic::new(
                        &codes::UI_RECURSIVE_COMPONENT,
                        decl.name.span,
                        format!("`{name}` s'inclut lui-même sans condition : il ne finirait jamais de s'afficher"),
                    )
                    .suggestion("placer l'inclusion sous un `if` ou un `foreach` (arbre, liste imbriquée)"),
                );
            }
        }
    }
}

/// Chemin `a`, `A.b` d'une expression faite seulement de noms.
fn expr_path(e: &Expr) -> Option<Path> {
    match &e.kind {
        ExprKind::Name { ident } => Some(Path {
            segments: vec![ident.clone()],
            span: ident.span,
        }),
        ExprKind::Member { object, field } => {
            let mut p = expr_path(object)?;
            p.segments.push(field.clone());
            p.span = p.span.to(field.span);
            Some(p)
        }
        _ => None,
    }
}

/// Valeur d'un nombre littéral (`3`, `-1`, `0.5`, `(2)`).
fn literal_number(e: &Expr) -> Option<f64> {
    match &e.kind {
        ExprKind::Int { text } | ExprKind::Float { text } => text.parse().ok(),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => literal_number(expr).map(|n| -n),
        ExprKind::Paren { expr } => literal_number(expr),
        _ => None,
    }
}

fn is_color(s: &str) -> bool {
    s.strip_prefix('#').is_some_and(|hex| {
        matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
    })
}
