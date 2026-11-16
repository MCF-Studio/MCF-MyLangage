//! Analyse sémantique MCF — phase 3 : résolution des noms, scopes et imports.
//!
//! Règles de visibilité (cahier des charges 1.2, §7 et §8) :
//! - un enfant voit les variables de ses parents, jamais l'inverse ;
//! - les globales et fonctions d'un fichier sont visibles dans tout ce fichier, quel que soit l'ordre ;
//! - les locales ne sont visibles qu'après leur déclaration, dans leur bloc et ses sous-blocs ;
//! - un élément d'un autre fichier n'est visible qu'après `import` ;
//! - les événements sont visibles dans tout le projet (voir DECISIONS D19).
//!
//! Ordre de recherche d'un nom : blocs locaux (du plus proche au plus lointain) →
//! scopes englobants → racine du fichier → imports → fonctions natives.

use std::collections::HashMap;

use mcf_diagnostics::{codes, Diagnostic, Span};
use mcf_syntax::ast::*;

/// Types natifs connus en phase 3 (cahier des charges 1.2, §5 ; `Bytes` vient de §11.1).
pub const BUILTIN_TYPES: &[&str] = &[
    "Bool", "Int", "Float", "String", "Char", "Byte", "Bytes", "Array", "Map", "DateTime", "Void",
];

/// Fonctions natives et leur nombre d'arguments.
pub const BUILTIN_FUNCTIONS: &[(&str, usize)] = &[("print", 1), ("sleep", 1)];

pub struct Source {
    pub name: String,
    pub text: String,
}

/// Analyse complète d'un projet : syntaxe puis sémantique.
/// Retourne les diagnostics de chaque fichier, dans l'ordre des sources, triés par position.
pub fn check_project(sources: &[Source]) -> Vec<Vec<Diagnostic>> {
    let parsed: Vec<_> = sources.iter().map(|s| mcf_syntax::parse(&s.text)).collect();
    let modules: Vec<&Module> = parsed.iter().map(|p| &p.module).collect();
    let mut semantic = analyze(&modules);
    parsed
        .into_iter()
        .zip(semantic.iter_mut())
        .map(|(p, sem)| {
            let mut diags = p.diagnostics;
            // Un fichier syntaxiquement invalide produirait des erreurs sémantiques en cascade.
            if !diags.iter().any(|d| d.is_error()) {
                diags.append(sem);
            }
            diags.sort_by_key(|d| d.span.start);
            diags
        })
        .collect()
}

#[derive(Clone, Debug)]
enum Sym {
    Global,
    Func { params: usize },
    Scope { table: usize },
    Builtin { params: usize },
}

impl Sym {
    fn describe(&self) -> &'static str {
        match self {
            Sym::Global => "une variable",
            Sym::Func { .. } => "une fonction",
            Sym::Scope { .. } => "un scope",
            Sym::Builtin { .. } => "une fonction native",
        }
    }
}

struct Table {
    parent: Option<usize>,
    names: HashMap<String, Sym>,
}

struct EventInfo {
    params: usize,
}

/// Résultat de la résolution d'une expression-chemin (`a`, `A.b.c`).
enum Resolved {
    Local,
    Sym(Sym),
    /// Inconnu ou non résolvable statiquement (ex. membre d'une valeur) : rien à vérifier de plus.
    Opaque,
}

pub fn analyze(modules: &[&Module]) -> Vec<Vec<Diagnostic>> {
    let mut a = Analyzer {
        tables: Vec::new(),
        roots: Vec::new(),
        imports: vec![HashMap::new(); modules.len()],
        events: HashMap::new(),
        diags: vec![Vec::new(); modules.len()],
        file: 0,
        locals: Vec::new(),
        loop_depth: 0,
    };
    for (file, module) in modules.iter().enumerate() {
        a.file = file;
        let root = a.new_table(None);
        a.roots.push(root);
        a.declare_items(&module.items, root);
    }
    for (file, module) in modules.iter().enumerate() {
        a.file = file;
        a.resolve_imports(&module.items);
    }
    for (file, module) in modules.iter().enumerate() {
        a.file = file;
        a.check_items(&module.items, a.roots[file]);
    }
    a.diags
}

struct Analyzer {
    tables: Vec<Table>,
    roots: Vec<usize>,
    imports: Vec<HashMap<String, Sym>>,
    events: HashMap<String, EventInfo>,
    diags: Vec<Vec<Diagnostic>>,
    file: usize,
    /// Pile des blocs locaux de la fonction en cours : nom → position de déclaration.
    locals: Vec<HashMap<String, Span>>,
    loop_depth: usize,
}

fn path_text(path: &Path) -> String {
    path.segments
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn type_text(ty: &TypeRef) -> String {
    let mut s = path_text(&ty.path);
    if !ty.args.is_empty() {
        let args: Vec<_> = ty.args.iter().map(type_text).collect();
        s.push('<');
        s.push_str(&args.join(", "));
        s.push('>');
    }
    s
}

impl Analyzer {
    fn report(&mut self, diag: Diagnostic) {
        self.diags[self.file].push(diag);
    }

    fn new_table(&mut self, parent: Option<usize>) -> usize {
        self.tables.push(Table {
            parent,
            names: HashMap::new(),
        });
        self.tables.len() - 1
    }

    // ------------------------------------------------------------ passe 1 : déclarations

    fn declare(&mut self, table: usize, ident: &Ident, sym: Sym) {
        if self.tables[table].names.contains_key(&ident.name) {
            self.report(
                Diagnostic::new(
                    &codes::DUPLICATE_DECLARATION,
                    ident.span,
                    format!("`{}` est déjà déclaré dans ce scope", ident.name),
                )
                .suggestion("renommer l'un des deux, ou les ranger dans des scopes différents"),
            );
            return;
        }
        self.tables[table].names.insert(ident.name.clone(), sym);
    }

    fn declare_items(&mut self, items: &[Item], table: usize) {
        for item in items {
            match item {
                Item::Global(v) => self.declare(table, &v.name, Sym::Global),
                Item::Func(f) => self.declare(
                    table,
                    &f.name,
                    Sym::Func {
                        params: f.params.len(),
                    },
                ),
                Item::Scope(s) => {
                    let child = self.new_table(Some(table));
                    self.declare(table, &s.name, Sym::Scope { table: child });
                    self.declare_items(&s.items, child);
                }
                Item::Event(e) => {
                    let name = path_text(&e.name);
                    if let std::collections::hash_map::Entry::Vacant(slot) =
                        self.events.entry(name.clone())
                    {
                        slot.insert(EventInfo {
                            params: e.params.len(),
                        });
                    } else {
                        self.report(Diagnostic::new(
                            &codes::DUPLICATE_DECLARATION,
                            e.name.span,
                            format!("l'événement `{name}` est déjà déclaré dans le projet"),
                        ));
                    }
                }
                Item::Import(_) | Item::On(_) => {}
            }
        }
    }

    // ------------------------------------------------------------ passe 2 : imports

    /// Cherche `segments` à partir de la racine de chaque autre fichier.
    fn find_exported(&self, segments: &[Ident]) -> Vec<Sym> {
        let mut found = Vec::new();
        for (file, &root) in self.roots.iter().enumerate() {
            if file == self.file {
                continue;
            }
            let mut table = root;
            let mut sym = None;
            for (i, seg) in segments.iter().enumerate() {
                match self.tables[table].names.get(&seg.name) {
                    Some(Sym::Scope { table: t }) if i + 1 < segments.len() => table = *t,
                    Some(s) if i + 1 == segments.len() => sym = Some(s.clone()),
                    _ => break,
                }
            }
            if let Some(s) = sym {
                found.push(s);
            }
        }
        found
    }

    fn resolve_imports(&mut self, items: &[Item]) {
        let root = self.roots[self.file];
        for item in items {
            let Item::Import(imp) = item else { continue };
            let full = path_text(&imp.path);
            let last = &imp.path.segments[imp.path.segments.len() - 1];
            let found = self.find_exported(&imp.path.segments);
            match found.len() {
                0 => self.report(
                    Diagnostic::new(
                        &codes::UNRESOLVED_IMPORT,
                        imp.path.span,
                        format!("`{full}` n'existe dans aucun autre fichier du projet"),
                    )
                    .suggestion("vérifier l'orthographe, ou que le fichier qui le déclare fait partie du projet vérifié"),
                ),
                1 => {
                    if self.tables[root].names.contains_key(&last.name)
                        || self.imports[self.file].contains_key(&last.name)
                    {
                        self.report(
                            Diagnostic::new(
                                &codes::IMPORT_CONFLICT,
                                imp.path.span,
                                format!("`{}` est déjà déclaré ou importé dans ce fichier", last.name),
                            )
                            .suggestion(format!(
                                "supprimer cet import et utiliser l'accès qualifié, ex. `import {};` puis `{full}`",
                                imp.path.segments[0].name
                            )),
                        );
                    } else {
                        let sym = found.into_iter().next().expect("un résultat");
                        self.imports[self.file].insert(last.name.clone(), sym);
                    }
                }
                n => self.report(
                    Diagnostic::new(
                        &codes::IMPORT_CONFLICT,
                        imp.path.span,
                        format!("`{full}` est déclaré dans {n} fichiers : import ambigu"),
                    )
                    .suggestion("renommer l'une des déclarations ou importer un chemin plus précis"),
                ),
            }
        }
    }

    // ------------------------------------------------------------ passe 3 : corps

    fn check_items(&mut self, items: &[Item], table: usize) {
        for item in items {
            match item {
                Item::Global(v) => {
                    self.check_type_decl(&v.ty);
                    if let Some(init) = &v.init {
                        self.check_expr(init, table);
                    }
                }
                Item::Func(f) => {
                    self.locals.push(HashMap::new());
                    for p in &f.params {
                        self.check_type_decl(&p.ty);
                        self.declare_local(&p.name);
                    }
                    if let Some(ret) = &f.ret {
                        self.check_type_decl(ret);
                    }
                    self.check_block(&f.body, table, false);
                    self.locals.pop();
                }
                Item::Scope(s) => {
                    let Some(Sym::Scope { table: child }) =
                        self.tables[table].names.get(&s.name.name).cloned()
                    else {
                        continue; // doublon déjà signalé
                    };
                    self.check_items(&s.items, child);
                }
                Item::Event(e) => {
                    self.locals.push(HashMap::new());
                    for p in &e.params {
                        self.check_type_decl(&p.ty);
                        self.declare_local(&p.name);
                    }
                    self.locals.pop();
                }
                Item::On(h) => {
                    self.check_event_use(&h.event, h.bindings.len(), h.event.span, "données");
                    self.locals.push(HashMap::new());
                    for b in &h.bindings {
                        self.declare_local(b);
                    }
                    self.check_block(&h.body, table, false);
                    self.locals.pop();
                }
                Item::Import(_) => {}
            }
        }
    }

    fn check_event_use(&mut self, path: &Path, count: usize, span: Span, what: &str) {
        let name = path_text(path);
        match self.events.get(&name) {
            None => self.report(
                Diagnostic::new(
                    &codes::UNDECLARED_EVENT,
                    path.span,
                    format!("l'événement `{name}` n'est pas déclaré"),
                )
                .suggestion(format!("déclarer `event {name}(...);`")),
            ),
            Some(info) if info.params != count => {
                let expected = info.params;
                self.report(
                    Diagnostic::new(
                        &codes::EVENT_ARITY_MISMATCH,
                        span,
                        format!("l'événement `{name}` transporte {expected} donnée(s), {count} {what} fournie(s)"),
                    )
                    .expected(expected.to_string())
                    .received(count.to_string()),
                );
            }
            Some(_) => {}
        }
    }

    fn check_type_decl(&mut self, decl: &TypeDecl) {
        for ty in &decl.types {
            self.check_type(ty);
        }
        if let [a, b] = decl.types.as_slice() {
            let (ta, tb) = (type_text(a), type_text(b));
            if ta == tb {
                self.report(
                    Diagnostic::new(
                        &codes::DUPLICATE_DUAL_TYPE,
                        decl.span,
                        format!("DualType `{ta}, {tb}` : les deux types sont identiques"),
                    )
                    .suggestion(format!("écrire simplement `{ta}`")),
                );
            }
        }
    }

    fn check_type(&mut self, ty: &TypeRef) {
        let name = path_text(&ty.path);
        if !BUILTIN_TYPES.contains(&name.as_str()) {
            self.report(
                Diagnostic::new(
                    &codes::UNKNOWN_TYPE,
                    ty.path.span,
                    format!("type inconnu `{name}`"),
                )
                .expected(BUILTIN_TYPES.join(", ")),
            );
        }
        for arg in &ty.args {
            self.check_type(arg);
        }
    }

    fn declare_local(&mut self, ident: &Ident) {
        let frame = self.locals.len() - 1;
        if self.locals[frame].contains_key(&ident.name) {
            self.report(Diagnostic::new(
                &codes::DUPLICATE_DECLARATION,
                ident.span,
                format!("`{}` est déjà déclarée dans ce bloc", ident.name),
            ));
            return;
        }
        let outer = self.locals[..frame]
            .iter()
            .rev()
            .find_map(|f| f.get(&ident.name).copied());
        if outer.is_some() {
            self.report(
                Diagnostic::new(
                    &codes::SHADOWED_LOCAL,
                    ident.span,
                    format!(
                        "`{}` masque une variable du même nom déclarée dans un bloc parent",
                        ident.name
                    ),
                )
                .suggestion("choisir un autre nom pour éviter toute confusion"),
            );
        }
        self.locals[frame].insert(ident.name.clone(), ident.span);
    }

    fn check_block(&mut self, block: &Block, table: usize, new_frame: bool) {
        if new_frame {
            self.locals.push(HashMap::new());
        }
        for stmt in &block.stmts {
            self.check_stmt(stmt, table);
        }
        if new_frame {
            self.locals.pop();
        }
    }

    fn check_loop_body(&mut self, block: &Block, table: usize) {
        self.loop_depth += 1;
        self.check_block(block, table, true);
        self.loop_depth -= 1;
    }

    fn check_stmt(&mut self, stmt: &Stmt, table: usize) {
        match stmt {
            Stmt::Local(v) => {
                self.check_type_decl(&v.ty);
                // L'initialisation est vérifiée avant la déclaration : `_x: Int = _x;` est refusé.
                if let Some(init) = &v.init {
                    self.check_expr(init, table);
                }
                self.declare_local(&v.name);
            }
            Stmt::Assign { target, value, .. } => {
                if let Resolved::Sym(
                    sym @ (Sym::Func { .. } | Sym::Scope { .. } | Sym::Builtin { .. }),
                ) = self.check_expr(target, table)
                {
                    self.report(Diagnostic::new(
                        &codes::NOT_ASSIGNABLE,
                        target.span,
                        format!("impossible d'affecter une valeur à {}", sym.describe()),
                    ));
                }
                self.check_expr(value, table);
            }
            Stmt::If(i) => self.check_if(i, table),
            Stmt::While { cond, body, .. } => {
                self.check_expr(cond, table);
                self.check_loop_body(body, table);
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                self.locals.push(HashMap::new());
                self.check_stmt(&Stmt::Local(init.clone()), table);
                self.check_expr(cond, table);
                self.check_stmt(step, table);
                self.check_loop_body(body, table);
                self.locals.pop();
            }
            Stmt::Foreach {
                binding,
                ty,
                iter,
                body,
                ..
            } => {
                if let Some(ty) = ty {
                    self.check_type_decl(ty);
                }
                self.check_expr(iter, table);
                self.locals.push(HashMap::new());
                self.declare_local(binding);
                self.check_loop_body(body, table);
                self.locals.pop();
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v, table);
                }
            }
            Stmt::Break { span } | Stmt::Continue { span } => {
                if self.loop_depth == 0 {
                    self.report(Diagnostic::new(
                        &codes::LOOP_CONTROL_OUTSIDE_LOOP,
                        *span,
                        "`break` / `continue` utilisé hors d'une boucle",
                    ));
                }
            }
            Stmt::Emit { event, args, span } => {
                self.check_event_use(event, args.len(), *span, "valeurs");
                for a in args {
                    self.check_expr(a, table);
                }
            }
            Stmt::Expr { expr, .. } => {
                self.check_expr(expr, table);
            }
        }
    }

    fn check_if(&mut self, i: &IfStmt, table: usize) {
        self.check_expr(&i.cond, table);
        self.check_block(&i.then_block, table, true);
        match &i.else_branch {
            Some(Else::Block(b)) => self.check_block(b, table, true),
            Some(Else::If(nested)) => self.check_if(nested, table),
            None => {}
        }
    }

    fn lookup(&self, name: &str, table: usize) -> Option<Resolved> {
        if self.locals.iter().rev().any(|f| f.contains_key(name)) {
            return Some(Resolved::Local);
        }
        let mut t = Some(table);
        while let Some(id) = t {
            if let Some(sym) = self.tables[id].names.get(name) {
                return Some(Resolved::Sym(sym.clone()));
            }
            t = self.tables[id].parent;
        }
        if let Some(sym) = self.imports[self.file].get(name) {
            return Some(Resolved::Sym(sym.clone()));
        }
        BUILTIN_FUNCTIONS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, params)| Resolved::Sym(Sym::Builtin { params: *params }))
    }

    fn undeclared(&mut self, ident: &Ident) {
        let mut d = Diagnostic::new(
            &codes::UNDECLARED_NAME,
            ident.span,
            format!("`{}` n'est pas déclaré ici", ident.name),
        );
        let elsewhere = self
            .roots
            .iter()
            .enumerate()
            .any(|(f, &r)| f != self.file && self.tables[r].names.contains_key(&ident.name));
        if elsewhere {
            d = d.suggestion(format!(
                "`{0}` existe dans un autre fichier : ajouter `import {0};`",
                ident.name
            ));
        } else if ident.is_local() {
            d = d.suggestion("une variable locale doit être déclarée avant usage, dans ce bloc ou un bloc parent");
        }
        self.report(d);
    }

    fn check_expr(&mut self, expr: &Expr, table: usize) -> Resolved {
        match &expr.kind {
            ExprKind::Name { ident } => match self.lookup(&ident.name, table) {
                Some(r) => r,
                None => {
                    self.undeclared(ident);
                    Resolved::Opaque
                }
            },
            ExprKind::Member { object, field } => match self.check_expr(object, table) {
                Resolved::Sym(Sym::Scope { table: scope }) => {
                    match self.tables[scope].names.get(&field.name).cloned() {
                        Some(sym) => Resolved::Sym(sym),
                        None => {
                            self.report(Diagnostic::new(
                                &codes::UNKNOWN_MEMBER,
                                field.span,
                                format!("le scope ne contient pas `{}`", field.name),
                            ));
                            Resolved::Opaque
                        }
                    }
                }
                // Membre d'une valeur : vérifié avec les types (phase 4).
                _ => Resolved::Opaque,
            },
            ExprKind::Call { callee, args } => {
                let callee_res = self.check_expr(callee, table);
                if let Resolved::Sym(Sym::Func { params } | Sym::Builtin { params }) = callee_res {
                    if params != args.len() {
                        self.report(
                            Diagnostic::new(
                                &codes::ARGUMENT_COUNT_MISMATCH,
                                expr.span,
                                format!(
                                    "{params} argument(s) attendu(s), {} fourni(s)",
                                    args.len()
                                ),
                            )
                            .expected(params.to_string())
                            .received(args.len().to_string()),
                        );
                    }
                }
                for a in args {
                    self.check_expr(a, table);
                }
                Resolved::Opaque
            }
            ExprKind::Index { object, index } => {
                self.check_expr(object, table);
                self.check_expr(index, table);
                Resolved::Opaque
            }
            ExprKind::Unary { expr, .. } | ExprKind::Paren { expr } => {
                self.check_expr(expr, table);
                Resolved::Opaque
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs, table);
                self.check_expr(rhs, table);
                Resolved::Opaque
            }
            ExprKind::Is { expr, ty } => {
                self.check_expr(expr, table);
                self.check_type(ty);
                Resolved::Opaque
            }
            ExprKind::Array { items } => {
                for i in items {
                    self.check_expr(i, table);
                }
                Resolved::Opaque
            }
            ExprKind::Int { .. }
            | ExprKind::Float { .. }
            | ExprKind::Str { .. }
            | ExprKind::Bool { .. }
            | ExprKind::Error => Resolved::Opaque,
        }
    }
}
