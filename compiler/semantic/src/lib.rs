//! Analyse sémantique MCF : résolution des noms (phase 3), types (phase 4),
//! structures et énumérations (phase 5).
//!
//! Visibilité (cahier des charges 1.2, §7 et §8) — détail dans `docs/grammaire/resolution-des-noms.md` :
//! blocs locaux → scopes englobants → racine du fichier → imports → fonctions natives.
//!
//! Types (§5 et §6) — détail dans `docs/grammaire/types.md` : aucune conversion implicite,
//! DualType limité à deux types, narrowing par `is` suivi de façon conservatrice.
//!
//! Passes : 1) déclaration des noms ; 2) imports ; 3) signatures (types des globales, fonctions,
//! champs, événements — un type peut venir d'un autre fichier) ; 4) structures récursives ;
//! 5) point d'entrée `func main(): Void` ; 6) corps.

use std::collections::{HashMap, HashSet};

use mcf_diagnostics::{codes, Diagnostic, Span};
use mcf_syntax::ast::*;

pub mod types;
pub use types::Ty;

/// Types natifs connus (cahier des charges 1.2, §5 ; `Bytes` vient de §11.1).
pub const BUILTIN_TYPES: &[&str] = &[
    "Bool", "Int", "Float", "String", "Char", "Byte", "Bytes", "Array", "Map", "Task", "DateTime",
    "Void",
];

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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Builtin {
    Print,
    Sleep,
    Duplicate,
}

#[derive(Clone, Debug)]
enum Sym {
    Global {
        ty: Ty,
    },
    Func {
        params: Vec<Ty>,
        ret: Ty,
        origin: FuncId,
    },
    Scope {
        table: usize,
    },
    Struct {
        id: usize,
    },
    Enum {
        id: usize,
    },
    Builtin(Builtin),
}

impl Sym {
    fn describe(&self) -> &'static str {
        match self {
            Sym::Global { .. } => "une variable",
            Sym::Func { .. } => "une fonction",
            Sym::Scope { .. } => "un scope",
            Sym::Struct { .. } => "une structure",
            Sym::Enum { .. } => "une énumération",
            Sym::Builtin(_) => "une fonction native",
        }
    }
}

struct Table {
    parent: Option<usize>,
    names: HashMap<String, Sym>,
}

struct EventInfo {
    params: Vec<Ty>,
}

struct StructInfo {
    name: String,
    name_span: Span,
    file: usize,
    fields: Vec<(String, Ty)>,
    methods: Vec<MethodInfo>,
}

/// Méthode d'une structure : appelée sur une valeur, `self` désigne cette valeur.
#[derive(Clone)]
struct MethodInfo {
    name: String,
    params: Vec<Ty>,
    ret: Ty,
    origin: FuncId,
}

/// Identité d'une fonction ou méthode MCF : (fichier, début de son nom).
type FuncId = (usize, usize);

/// Nom interne de la locale qui porte le receveur d'une méthode. `self` est un mot-clé :
/// aucun identifiant écrit par l'utilisateur ne peut le masquer.
const SELF_LOCAL: &str = "self";

struct EnumInfo {
    name: String,
    variants: Vec<String>,
}

/// Un bloc local : ses variables et les types actifs connus (narrowing) dans ce bloc.
#[derive(Default)]
struct Frame {
    locals: HashMap<String, (Span, Ty)>,
    narrow: HashMap<String, Ty>,
}

/// Ce que désigne une expression, quand c'est un chemin nommé.
#[derive(Clone)]
enum Resolved {
    Local,
    Sym(Sym),
    /// Méthode d'une valeur (`_xs.push`, `_user.rename`) : signature déjà adaptée au
    /// type du receveur. `user` : méthode écrite en MCF, qui peut modifier des globales.
    Method {
        params: Vec<Ty>,
        ret: Ty,
        /// Méthode écrite en MCF (`None` : méthode native).
        origin: Option<FuncId>,
    },
    Opaque,
}

/// Résultat de la vérification d'une expression.
struct Typed {
    res: Resolved,
    /// Type actif (après narrowing).
    ty: Ty,
    /// Type déclaré (pour une affectation, c'est lui qui compte).
    declared: Ty,
}

impl Typed {
    fn value(ty: Ty) -> Self {
        Typed {
            res: Resolved::Opaque,
            declared: ty.clone(),
            ty,
        }
    }
}

type Narrowings = Vec<(String, Ty)>;

pub fn analyze(modules: &[&Module]) -> Vec<Vec<Diagnostic>> {
    let mut a = Analyzer {
        tables: Vec::new(),
        roots: Vec::new(),
        imports: vec![HashMap::new(); modules.len()],
        events: HashMap::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        rejected: HashSet::new(),
        diags: vec![Vec::new(); modules.len()],
        file: 0,
        frames: Vec::new(),
        loop_depth: 0,
        ret: None,
        current_func: None,
        last_call: None,
        started: HashSet::new(),
        loops_without_pause: Vec::new(),
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
        a.resolve_signatures(&module.items, a.roots[file]);
    }
    a.check_recursive_structs();
    a.check_entry_points(modules);
    for (file, module) in modules.iter().enumerate() {
        a.file = file;
        a.check_items(&module.items, a.roots[file]);
    }
    a.advise_loops_in_tasks();
    a.diags
}

struct Analyzer {
    tables: Vec<Table>,
    roots: Vec<usize>,
    /// Par fichier : nom importé → (table d'origine, nom). Résolu à la lecture pour voir les types à jour.
    imports: Vec<HashMap<String, (usize, String)>>,
    events: HashMap<String, EventInfo>,
    structs: Vec<StructInfo>,
    enums: Vec<EnumInfo>,
    /// Déclarations en doublon (fichier, début du nom) : ignorées après le diagnostic.
    rejected: HashSet<(usize, usize)>,
    diags: Vec<Vec<Diagnostic>>,
    file: usize,
    frames: Vec<Frame>,
    loop_depth: usize,
    /// Type de retour de la fonction en cours (`None` hors fonction).
    ret: Option<Ty>,
    /// Fonction ou méthode dont on vérifie le corps.
    current_func: Option<FuncId>,
    /// Fonction MCF visée par le dernier appel vérifié (pour `start`).
    last_call: Option<FuncId>,
    /// Fonctions lancées quelque part avec `start`.
    started: HashSet<FuncId>,
    /// Boucles `while` sans aucun point de suspension possible : (fonction, fichier, endroit).
    loops_without_pause: Vec<(FuncId, usize, Span)>,
}

fn path_text(path: &Path) -> String {
    path.segments
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Or => "||",
        BinaryOp::And => "&&",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::Le => "<=",
        BinaryOp::Ge => ">=",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
    }
}

/// Un bloc se termine-t-il toujours par un `return` ?
fn always_returns(block: &Block) -> bool {
    block.stmts.iter().any(|s| match s {
        Stmt::Return { .. } => true,
        Stmt::If(i) => if_always_returns(i),
        _ => false,
    })
}

fn if_always_returns(i: &IfStmt) -> bool {
    always_returns(&i.then_block)
        && match &i.else_branch {
            Some(Else::Block(b)) => always_returns(b),
            Some(Else::If(nested)) => if_always_returns(nested),
            None => false,
        }
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

    fn is_rejected(&self, ident: &Ident) -> bool {
        self.rejected.contains(&(self.file, ident.span.start))
    }

    fn set_sym(&mut self, table: usize, ident: &Ident, sym: Sym) {
        if !self.is_rejected(ident) {
            self.tables[table].names.insert(ident.name.clone(), sym);
        }
    }

    // ================================================================ types déclarés

    /// Résout un chemin `A.B.C` : premier segment selon les règles de visibilité, puis scopes.
    fn resolve_path(&self, path: &Path, table: usize) -> Option<Sym> {
        let first = &path.segments[0];
        let mut sym = match self.lookup(&first.name, table)? {
            Resolved::Sym(s) => s,
            _ => return None,
        };
        for seg in &path.segments[1..] {
            let Sym::Scope { table: t } = sym else {
                return None;
            };
            sym = self.tables[t].names.get(&seg.name)?.clone();
        }
        Some(sym)
    }

    fn type_ref(&mut self, r: &TypeRef, table: usize, report: bool) -> Ty {
        let name = path_text(&r.path);
        let base = match Ty::from_builtin(&name) {
            Some(b) => b,
            None => match self.resolve_path(&r.path, table) {
                Some(Sym::Struct { id }) => Ty::Struct {
                    id,
                    name: self.structs[id].name.clone(),
                },
                Some(Sym::Enum { id }) => Ty::Enum {
                    id,
                    name: self.enums[id].name.clone(),
                },
                Some(other) => {
                    if report {
                        self.report(Diagnostic::new(
                            &codes::NOT_A_TYPE,
                            r.path.span,
                            format!("`{name}` est {}, pas un type", other.describe()),
                        ));
                    }
                    return Ty::Unknown;
                }
                None => {
                    if report {
                        self.report(
                            Diagnostic::new(
                                &codes::UNKNOWN_TYPE,
                                r.path.span,
                                format!("type inconnu `{name}`"),
                            )
                            .expected(format!(
                                "{} ou une structure / énumération déclarée",
                                BUILTIN_TYPES.join(", ")
                            )),
                        );
                    }
                    return Ty::Unknown;
                }
            },
        };
        let expected_args = match base {
            Ty::Array(_) | Ty::Task(_) => &[0usize, 1][..],
            Ty::Map(..) => &[0, 2][..],
            _ => &[0][..],
        };
        if !expected_args.contains(&r.args.len()) {
            if report {
                let expected = expected_args
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(" ou ");
                self.report(
                    Diagnostic::new(
                        &codes::WRONG_TYPE_ARGUMENTS,
                        r.span,
                        format!(
                            "`{name}` attend {expected} argument(s) de type, {} fourni(s)",
                            r.args.len()
                        ),
                    )
                    .suggestion("exemples valides : `Array<Int>`, `Map<String, Int>`"),
                );
            }
            return Ty::Unknown;
        }
        let mut args: Vec<Ty> = r
            .args
            .iter()
            .map(|a| self.type_ref(a, table, report))
            .collect();
        if report {
            for (arg, ty) in r.args.iter().zip(&args) {
                // `Task<Void>` : tâche qui ne produit pas de valeur.
                if *ty == Ty::Void && !matches!(base, Ty::Task(_)) {
                    self.report(Diagnostic::new(
                        &codes::VOID_NOT_ALLOWED,
                        arg.span,
                        "`Void` n'est pas autorisé comme argument de type",
                    ));
                }
            }
        }
        match base {
            Ty::Array(_) if args.len() == 1 => Ty::Array(Box::new(args.remove(0))),
            Ty::Task(_) if args.len() == 1 => Ty::Task(Box::new(args.remove(0))),
            Ty::Map(..) if args.len() == 2 => {
                let v = args.remove(1);
                Ty::Map(Box::new(args.remove(0)), Box::new(v))
            }
            other => other,
        }
    }

    /// Type d'une déclaration `T` ou `T1, T2`. `Void` n'est accepté que seul et si `allow_void`.
    fn type_decl(&mut self, decl: &TypeDecl, table: usize, allow_void: bool, report: bool) -> Ty {
        let tys: Vec<Ty> = decl
            .types
            .iter()
            .map(|t| self.type_ref(t, table, report))
            .collect();
        if report {
            for (r, ty) in decl.types.iter().zip(&tys) {
                if *ty == Ty::Void && !(allow_void && tys.len() == 1) {
                    self.report(
                        Diagnostic::new(
                            &codes::VOID_NOT_ALLOWED,
                            r.span,
                            "`Void` n'est pas un type de valeur",
                        )
                        .suggestion("`Void` s'utilise seul, comme type de retour d'une fonction"),
                    );
                }
            }
        }
        match tys.as_slice() {
            [single] => single.clone(),
            [a, b] => {
                if a.same(b) && !a.is_unknown() {
                    if report {
                        self.report(
                            Diagnostic::new(
                                &codes::DUPLICATE_DUAL_TYPE,
                                decl.span,
                                format!("DualType `{a}, {b}` : les deux types sont identiques"),
                            )
                            .suggestion(format!("écrire simplement `{a}`")),
                        );
                    }
                    a.clone()
                } else if matches!(
                    (a, b),
                    (Ty::Array(_), Ty::Array(_))
                        | (Ty::Map(..), Ty::Map(..))
                        | (Ty::Task(_), Ty::Task(_))
                ) {
                    if report {
                        self.report(
                            Diagnostic::new(
                                &codes::DUPLICATE_DUAL_TYPE,
                                decl.span,
                                format!("DualType `{a}, {b}` : deux collections ou tâches de même sorte ne se distinguent pas avec `is`"),
                            )
                            .suggestion("garder une seule collection, ou combiner une collection et un type simple"),
                        );
                    }
                    Ty::Unknown
                } else if a.is_unknown() || b.is_unknown() {
                    Ty::Unknown
                } else {
                    Ty::Dual(Box::new(a.clone()), Box::new(b.clone()))
                }
            }
            // Plus de deux types : MCF0106 déjà signalé par le parser.
            _ => Ty::Unknown,
        }
    }

    // ================================================================ passe 1 : noms

    fn declare(&mut self, table: usize, ident: &Ident, sym: Sym) {
        let clash_builtin = matches!(sym, Sym::Struct { .. } | Sym::Enum { .. })
            && BUILTIN_TYPES.contains(&ident.name.as_str());
        if clash_builtin || self.tables[table].names.contains_key(&ident.name) {
            let message = if clash_builtin {
                format!("`{}` est le nom d'un type natif", ident.name)
            } else {
                format!("`{}` est déjà déclaré dans ce scope", ident.name)
            };
            self.report(
                Diagnostic::new(&codes::DUPLICATE_DECLARATION, ident.span, message)
                    .suggestion("renommer l'un des deux, ou les ranger dans des scopes différents"),
            );
            self.rejected.insert((self.file, ident.span.start));
            return;
        }
        self.tables[table].names.insert(ident.name.clone(), sym);
    }

    fn declare_items(&mut self, items: &[Item], table: usize) {
        for item in items {
            match item {
                Item::Global(v) => self.declare(table, &v.name, Sym::Global { ty: Ty::Unknown }),
                Item::Func(f) => self.declare(
                    table,
                    &f.name,
                    Sym::Func {
                        params: vec![Ty::Unknown; f.params.len()],
                        ret: Ty::Unknown,
                        origin: (self.file, f.name.span.start),
                    },
                ),
                Item::Scope(s) => {
                    let child = self.new_table(Some(table));
                    self.declare(table, &s.name, Sym::Scope { table: child });
                    self.declare_items(&s.items, child);
                }
                Item::Struct(s) => {
                    let id = self.structs.len();
                    self.structs.push(StructInfo {
                        name: s.name.name.clone(),
                        name_span: s.name.span,
                        file: self.file,
                        fields: Vec::new(),
                        methods: Vec::new(),
                    });
                    self.declare(table, &s.name, Sym::Struct { id });
                }
                Item::Enum(e) => {
                    let mut variants: Vec<String> = Vec::new();
                    for v in &e.variants {
                        if variants.contains(&v.name) {
                            self.report(Diagnostic::new(
                                &codes::DUPLICATE_FIELD,
                                v.span,
                                format!("la variante `{}` est déclarée deux fois", v.name),
                            ));
                        } else {
                            variants.push(v.name.clone());
                        }
                    }
                    let id = self.enums.len();
                    self.enums.push(EnumInfo {
                        name: e.name.name.clone(),
                        variants,
                    });
                    self.declare(table, &e.name, Sym::Enum { id });
                }
                Item::Event(e) => {
                    let name = path_text(&e.name);
                    if let std::collections::hash_map::Entry::Vacant(slot) =
                        self.events.entry(name.clone())
                    {
                        slot.insert(EventInfo {
                            params: vec![Ty::Unknown; e.params.len()],
                        });
                    } else {
                        self.report(Diagnostic::new(
                            &codes::DUPLICATE_DECLARATION,
                            e.name.span,
                            format!("l'événement `{name}` est déjà déclaré dans le projet"),
                        ));
                        let last = &e.name.segments[e.name.segments.len() - 1];
                        self.rejected.insert((self.file, last.span.start));
                    }
                }
                Item::Import(_) | Item::On(_) => {}
            }
        }
    }

    // ================================================================ passe 2 : imports

    /// Cherche `segments` à partir de la racine de chaque autre fichier : (table, nom).
    fn find_exported(&self, segments: &[Ident]) -> Vec<(usize, String)> {
        let mut found = Vec::new();
        for (file, &root) in self.roots.iter().enumerate() {
            if file == self.file {
                continue;
            }
            let mut table = root;
            for (i, seg) in segments.iter().enumerate() {
                match self.tables[table].names.get(&seg.name) {
                    Some(Sym::Scope { table: t }) if i + 1 < segments.len() => table = *t,
                    Some(_) if i + 1 == segments.len() => found.push((table, seg.name.clone())),
                    _ => break,
                }
            }
        }
        found
    }

    fn resolve_imports(&mut self, items: &[Item]) {
        let root = self.roots[self.file];
        for item in items {
            let Item::Import(imp) = item else { continue };
            let full = path_text(&imp.path);
            let local = imp.local_name();
            let mut found = self.find_exported(&imp.path.segments);
            if found.len() > 1 {
                let n = found.len();
                self.report(
                    Diagnostic::new(
                        &codes::IMPORT_CONFLICT,
                        imp.path.span,
                        format!("`{full}` est déclaré dans {n} fichiers : import ambigu"),
                    )
                    .suggestion(
                        "renommer l'une des déclarations ou importer un chemin plus précis",
                    ),
                );
                continue;
            }
            let Some(target) = found.pop() else {
                self.report(
                    Diagnostic::new(
                        &codes::UNRESOLVED_IMPORT,
                        imp.path.span,
                        format!("`{full}` n'existe dans aucun autre fichier du projet"),
                    )
                    .suggestion("vérifier l'orthographe, ou que le fichier qui le déclare fait partie du projet vérifié"),
                );
                continue;
            };
            // Un même élément sous deux noms rendrait le fichier plus difficile à lire.
            if let Some(other) = self.imports[self.file]
                .iter()
                .find(|(_, t)| **t == target)
                .map(|(name, _)| name.clone())
            {
                self.report(
                    Diagnostic::new(
                        &codes::IMPORT_CONFLICT,
                        imp.span,
                        format!("`{full}` est déjà importé dans ce fichier sous le nom `{other}`"),
                    )
                    .suggestion("supprimer cet import"),
                );
                continue;
            }
            if local.name == "main" {
                self.report(
                    Diagnostic::new(
                        &codes::INVALID_ENTRY_POINT,
                        local.span,
                        "`main` est réservé au point d'entrée : un import ne peut pas porter ce nom",
                    )
                    .suggestion(format!("importer sous un autre nom : `import {full} as autreNom;`")),
                );
                continue;
            }
            if self.tables[root].names.contains_key(&local.name)
                || self.imports[self.file].contains_key(&local.name)
            {
                let what = if imp.alias.is_some() {
                    "l'alias"
                } else {
                    "le nom"
                };
                self.report(
                    Diagnostic::new(
                        &codes::IMPORT_CONFLICT,
                        local.span,
                        format!("{what} `{}` est déjà déclaré ou importé dans ce fichier", local.name),
                    )
                    .suggestion(format!(
                        "importer sous un autre nom (`import {full} as autreNom;`) ou utiliser l'accès qualifié (`import {};` puis `{full}`)",
                        imp.path.segments[0].name
                    )),
                );
                continue;
            }
            self.imports[self.file].insert(local.name.clone(), target);
        }
    }

    // ================================================================ passe 3 : signatures

    fn resolve_signatures(&mut self, items: &[Item], table: usize) {
        for item in items {
            match item {
                Item::Global(v) => {
                    let ty = self.type_decl(&v.ty, table, false, true);
                    self.set_sym(table, &v.name, Sym::Global { ty });
                }
                Item::Func(f) => {
                    let params = f
                        .params
                        .iter()
                        .map(|p| self.type_decl(&p.ty, table, false, true))
                        .collect();
                    let ret = match &f.ret {
                        Some(r) => self.type_decl(r, table, true, true),
                        None => Ty::Unknown,
                    };
                    let origin = (self.file, f.name.span.start);
                    self.set_sym(
                        table,
                        &f.name,
                        Sym::Func {
                            params,
                            ret,
                            origin,
                        },
                    );
                }
                Item::Scope(s) => {
                    if let Some(Sym::Scope { table: child }) =
                        self.tables[table].names.get(&s.name.name).cloned()
                    {
                        if !self.is_rejected(&s.name) {
                            self.resolve_signatures(&s.items, child);
                        }
                    }
                }
                Item::Struct(s) => {
                    let mut fields: Vec<(String, Ty)> = Vec::new();
                    for f in &s.fields {
                        let ty = self.type_decl(&f.ty, table, false, true);
                        if fields.iter().any(|(n, _)| *n == f.name.name) {
                            self.report(Diagnostic::new(
                                &codes::DUPLICATE_FIELD,
                                f.name.span,
                                format!("le champ `{}` est déclaré deux fois", f.name.name),
                            ));
                        } else {
                            fields.push((f.name.name.clone(), ty));
                        }
                    }
                    let mut methods: Vec<MethodInfo> = Vec::new();
                    for m in &s.methods {
                        let params = m
                            .params
                            .iter()
                            .map(|p| self.type_decl(&p.ty, table, false, true))
                            .collect();
                        let ret = match &m.ret {
                            Some(r) => self.type_decl(r, table, true, true),
                            None => Ty::Unknown,
                        };
                        let clash = if fields.iter().any(|(n, _)| *n == m.name.name) {
                            Some("un champ porte déjà ce nom")
                        } else if methods.iter().any(|x| x.name == m.name.name) {
                            Some("la méthode est déclarée deux fois")
                        } else {
                            None
                        };
                        if let Some(why) = clash {
                            self.report(
                                Diagnostic::new(
                                    &codes::DUPLICATE_FIELD,
                                    m.name.span,
                                    format!("`{}` : {why}", m.name.name),
                                )
                                .suggestion(
                                    "les champs et les méthodes d'une structure ont des noms distincts",
                                ),
                            );
                            self.rejected.insert((self.file, m.name.span.start));
                        } else {
                            methods.push(MethodInfo {
                                name: m.name.name.clone(),
                                origin: (self.file, m.name.span.start),
                                params,
                                ret,
                            });
                        }
                    }
                    if !self.is_rejected(&s.name) {
                        if let Some(Sym::Struct { id }) = self.tables[table].names.get(&s.name.name)
                        {
                            let id = *id;
                            self.structs[id].fields = fields;
                            self.structs[id].methods = methods;
                        }
                    }
                }
                Item::Event(e) => {
                    let params: Vec<Ty> = e
                        .params
                        .iter()
                        .map(|p| self.type_decl(&p.ty, table, false, true))
                        .collect();
                    let last = &e.name.segments[e.name.segments.len() - 1];
                    if !self.is_rejected(last) {
                        if let Some(info) = self.events.get_mut(&path_text(&e.name)) {
                            info.params = params;
                        }
                    }
                }
                Item::Enum(_) | Item::Import(_) | Item::On(_) => {}
            }
        }
    }

    // ================================================================ passe 4 : récursion

    /// Une structure qui se contient elle-même (directement, via d'autres structures ou un
    /// DualType) aurait une taille infinie. Passer par `Array<T>` est autorisé.
    fn check_recursive_structs(&mut self) {
        fn reaches(
            structs: &[StructInfo],
            from: usize,
            target: usize,
            seen: &mut HashSet<usize>,
        ) -> bool {
            if !seen.insert(from) {
                return false;
            }
            structs[from].fields.iter().any(|(_, ty)| {
                ty.members().iter().any(|m| match m {
                    Ty::Struct { id, .. } => *id == target || reaches(structs, *id, target, seen),
                    _ => false,
                })
            })
        }
        for id in 0..self.structs.len() {
            if reaches(&self.structs, id, id, &mut HashSet::new()) {
                let (file, span, name) = {
                    let s = &self.structs[id];
                    (s.file, s.name_span, s.name.clone())
                };
                self.diags[file].push(
                    Diagnostic::new(
                        &codes::RECURSIVE_STRUCT,
                        span,
                        format!("la structure `{name}` se contient elle-même : sa taille serait infinie"),
                    )
                    .suggestion("passer par une collection, ex. `children: Array<Node>;`"),
                );
            }
        }
    }

    // ================================================================ passe 5 : point d'entrée

    /// `main` à la racine d'un fichier est réservé au point d'entrée du projet :
    /// une fonction `func main(): Void` sans paramètre, une seule dans tout le projet.
    /// Un projet sans `main` reste vérifiable (bibliothèque) ; c'est `mcf run` qui l'exigera.
    fn check_entry_points(&mut self, modules: &[&Module]) {
        let mut found = false;
        for (file, module) in modules.iter().enumerate() {
            self.file = file;
            for item in &module.items {
                let (ident, what) = match item {
                    Item::Func(f) => (&f.name, None),
                    Item::Global(v) => (&v.name, Some("une variable")),
                    Item::Scope(s) => (&s.name, Some("un scope")),
                    Item::Struct(s) => (&s.name, Some("une structure")),
                    Item::Enum(e) => (&e.name, Some("une énumération")),
                    Item::Import(_) | Item::Event(_) | Item::On(_) => continue,
                };
                if ident.name != "main" || self.is_rejected(ident) {
                    continue;
                }
                let Item::Func(f) = item else {
                    self.report(
                        Diagnostic::new(
                            &codes::INVALID_ENTRY_POINT,
                            ident.span,
                            format!(
                                "`main` est réservé au point d'entrée, pas à {}",
                                what.unwrap_or_default()
                            ),
                        )
                        .expected("func main(): Void { ... }")
                        .suggestion("renommer cette déclaration"),
                    );
                    continue;
                };
                if found {
                    self.report(
                        Diagnostic::new(
                            &codes::DUPLICATE_ENTRY_POINT,
                            ident.span,
                            "le projet a déjà un point d'entrée `main` dans un autre fichier",
                        )
                        .suggestion("garder un seul `func main(): Void` par projet"),
                    );
                    continue;
                }
                found = true;
                if let (Some(first), Some(last)) = (f.params.first(), f.params.last()) {
                    self.report(
                        Diagnostic::new(
                            &codes::INVALID_ENTRY_POINT,
                            first.span.to(last.span),
                            "le point d'entrée `main` ne prend aucun paramètre",
                        )
                        .expected("func main(): Void")
                        .received(format!("{} paramètre(s)", f.params.len())),
                    );
                }
                let ret = match self.tables[self.roots[file]].names.get("main") {
                    Some(Sym::Func { ret, .. }) => ret.clone(),
                    _ => Ty::Unknown,
                };
                // `None` ou `Unknown` : erreur déjà signalée (MCF0111, type inconnu).
                if let (Some(r), false) = (&f.ret, matches!(ret, Ty::Void | Ty::Unknown)) {
                    self.report(
                        Diagnostic::new(
                            &codes::INVALID_ENTRY_POINT,
                            r.span,
                            "le point d'entrée `main` ne retourne rien",
                        )
                        .expected("Void")
                        .received(ret.to_string()),
                    );
                }
            }
        }
    }

    // ================================================================ passe 6 : corps

    fn check_items(&mut self, items: &[Item], table: usize) {
        for item in items {
            match item {
                Item::Global(v) => {
                    if self.is_rejected(&v.name) {
                        continue;
                    }
                    if let (Some(init), Some(Sym::Global { ty })) =
                        (&v.init, self.tables[table].names.get(&v.name.name).cloned())
                    {
                        let value = self.check_expr(init, table);
                        self.expect_assignable(&value.ty, &ty, init.span);
                    }
                }
                Item::Func(f) => {
                    if self.is_rejected(&f.name) {
                        continue;
                    }
                    if let Some(Sym::Func { params, ret, .. }) =
                        self.tables[table].names.get(&f.name.name).cloned()
                    {
                        self.check_func_body(f, &params, ret, None, table);
                    }
                }
                Item::Scope(s) => {
                    if self.is_rejected(&s.name) {
                        continue;
                    }
                    if let Some(Sym::Scope { table: child }) =
                        self.tables[table].names.get(&s.name.name).cloned()
                    {
                        self.check_items(&s.items, child);
                    }
                }
                Item::Event(e) => {
                    self.enter_body(Ty::Void);
                    for p in &e.params {
                        self.declare_local(&p.name, Ty::Unknown);
                    }
                    self.leave_body();
                }
                Item::On(h) => {
                    let params =
                        self.check_event_use(&h.event, h.bindings.len(), h.event.span, "données");
                    self.enter_body(Ty::Void);
                    for (i, b) in h.bindings.iter().enumerate() {
                        let ty = params.get(i).cloned().unwrap_or(Ty::Unknown);
                        self.declare_local(b, ty);
                    }
                    self.check_block_in_current_frame(&h.body, table);
                    self.leave_body();
                }
                Item::Struct(s) => {
                    if self.is_rejected(&s.name) {
                        continue;
                    }
                    let Some(Sym::Struct { id }) = self.tables[table].names.get(&s.name.name)
                    else {
                        continue;
                    };
                    let id = *id;
                    let receiver = Ty::Struct {
                        id,
                        name: s.name.name.clone(),
                    };
                    for m in &s.methods {
                        if self.is_rejected(&m.name) {
                            continue;
                        }
                        let Some(info) = self.structs[id]
                            .methods
                            .iter()
                            .find(|x| x.name == m.name.name)
                        else {
                            continue;
                        };
                        let (params, ret) = (info.params.clone(), info.ret.clone());
                        self.check_func_body(m, &params, ret, Some(receiver.clone()), table);
                    }
                }
                Item::Enum(_) | Item::Import(_) => {}
            }
        }
    }

    /// Corps d'une fonction ou d'une méthode (`receiver` : type de `self`).
    fn check_func_body(
        &mut self,
        f: &FuncDecl,
        params: &[Ty],
        ret: Ty,
        receiver: Option<Ty>,
        table: usize,
    ) {
        self.enter_body(ret.clone());
        self.current_func = Some((self.file, f.name.span.start));
        if let (Some(ty), Some(frame)) = (receiver, self.frames.last_mut()) {
            frame
                .locals
                .insert(SELF_LOCAL.to_string(), (f.name.span, ty));
        }
        for (p, ty) in f.params.iter().zip(params) {
            self.declare_local(&p.name, ty.clone());
        }
        self.check_block_in_current_frame(&f.body, table);
        if !matches!(ret, Ty::Void | Ty::Unknown) && !always_returns(&f.body) {
            let end = Span::new(f.body.span.end.saturating_sub(1), f.body.span.end);
            self.report(
                Diagnostic::new(
                    &codes::MISSING_RETURN,
                    end,
                    format!(
                        "la fonction `{}` doit retourner `{ret}` sur tous les chemins",
                        f.name.name
                    ),
                )
                .suggestion("ajouter un `return` à la fin, ou une branche `else` qui retourne"),
            );
        }
        self.current_func = None;
        self.leave_body();
    }

    fn enter_body(&mut self, ret: Ty) {
        self.frames.push(Frame::default());
        self.ret = Some(ret);
    }

    fn leave_body(&mut self) {
        self.frames.pop();
        self.ret = None;
    }

    /// Retourne les types des données de l'événement (vide s'il est inconnu).
    fn check_event_use(&mut self, path: &Path, count: usize, span: Span, what: &str) -> Vec<Ty> {
        let name = path_text(path);
        match self.events.get(&name) {
            None => {
                self.report(
                    Diagnostic::new(
                        &codes::UNDECLARED_EVENT,
                        path.span,
                        format!("l'événement `{name}` n'est pas déclaré"),
                    )
                    .suggestion(format!("déclarer `event {name}(...);`")),
                );
                Vec::new()
            }
            Some(info) => {
                let params = info.params.clone();
                if params.len() != count {
                    self.report(
                        Diagnostic::new(
                            &codes::EVENT_ARITY_MISMATCH,
                            span,
                            format!(
                                "l'événement `{name}` transporte {} donnée(s), {count} {what} fournie(s)",
                                params.len()
                            ),
                        )
                        .expected(params.len().to_string())
                        .received(count.to_string()),
                    );
                }
                params
            }
        }
    }

    fn expect_assignable(&mut self, value: &Ty, target: &Ty, span: Span) {
        if value.assignable_to(target) {
            return;
        }
        let mut d = Diagnostic::new(
            &codes::TYPE_MISMATCH,
            span,
            format!("une valeur `{value}` ne peut pas être rangée dans `{target}`"),
        )
        .expected(target.to_string())
        .received(value.to_string());
        if value.is_dual() {
            d = d.suggestion("vérifier d'abord le type actif : `if (valeur is Type) then { ... }`");
        } else if value.is_numeric() && target.is_numeric() {
            d = d.suggestion(
                "pas de conversion implicite entre Int et Float : écrire `2.0` plutôt que `2`, par exemple",
            );
        }
        self.report(d);
    }

    // ---------------------------------------------------------------- locales et narrowing

    fn declare_local(&mut self, ident: &Ident, ty: Ty) {
        let frame = self.frames.len() - 1;
        if self.frames[frame].locals.contains_key(&ident.name) {
            self.report(Diagnostic::new(
                &codes::DUPLICATE_DECLARATION,
                ident.span,
                format!("`{}` est déjà déclarée dans ce bloc", ident.name),
            ));
            return;
        }
        let shadows = self.frames[..frame]
            .iter()
            .any(|f| f.locals.contains_key(&ident.name));
        if shadows {
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
        let f = &mut self.frames[frame];
        f.narrow.remove(&ident.name);
        f.locals.insert(ident.name.clone(), (ident.span, ty));
    }

    /// Type actif d'un nom : le narrowing le plus proche, sans traverser sa propre déclaration.
    fn active_ty(&self, name: &str, declared: &Ty) -> Ty {
        for f in self.frames.iter().rev() {
            if let Some(t) = f.narrow.get(name) {
                return t.clone();
            }
            if f.locals.contains_key(name) {
                break;
            }
        }
        declared.clone()
    }

    fn forget_narrowing(&mut self, name: &str) {
        for f in &mut self.frames {
            f.narrow.remove(name);
        }
    }

    /// Un appel de fonction ou un `emit` peut modifier n'importe quelle globale.
    fn forget_global_narrowings(&mut self) {
        for f in &mut self.frames {
            f.narrow.retain(|name, _| name.starts_with('_'));
        }
    }

    fn with_narrowings<R>(&mut self, n: Narrowings, run: impl FnOnce(&mut Self) -> R) -> R {
        let mut frame = Frame::default();
        frame.narrow.extend(n);
        self.frames.push(frame);
        let r = run(self);
        self.frames.pop();
        r
    }

    /// Narrowings induits par une condition : (si vraie, si fausse).
    fn narrowings(&mut self, cond: &Expr, table: usize) -> (Narrowings, Narrowings) {
        match &cond.kind {
            ExprKind::Is { expr, ty } => {
                let ExprKind::Name { ident } = &expr.kind else {
                    return (vec![], vec![]);
                };
                let Some((_, declared)) = self.lookup_typed(&ident.name, table) else {
                    return (vec![], vec![]);
                };
                let current = self.active_ty(&ident.name, &declared);
                let tested = self.type_ref(ty, table, false);
                let members = current.members();
                if !current.is_dual() || !members.iter().any(|m| m.same(&tested)) {
                    return (vec![], vec![]);
                }
                let other = members
                    .iter()
                    .find(|m| !m.same(&tested))
                    .map(|m| (*m).clone());
                let then = vec![(ident.name.clone(), tested)];
                let otherwise = other
                    .map(|o| vec![(ident.name.clone(), o)])
                    .unwrap_or_default();
                (then, otherwise)
            }
            ExprKind::Binary {
                op: BinaryOp::And,
                lhs,
                rhs,
            } => {
                let (mut t, _) = self.narrowings(lhs, table);
                let (t2, _) = self.with_narrowings(t.clone(), |a| a.narrowings(rhs, table));
                t.extend(t2);
                (t, vec![])
            }
            ExprKind::Binary {
                op: BinaryOp::Or,
                lhs,
                rhs,
            } => {
                let (_, mut f) = self.narrowings(lhs, table);
                let (_, f2) = self.with_narrowings(f.clone(), |a| a.narrowings(rhs, table));
                f.extend(f2);
                (vec![], f)
            }
            ExprKind::Unary {
                op: UnaryOp::Not,
                expr,
            } => {
                let (t, f) = self.narrowings(expr, table);
                (f, t)
            }
            ExprKind::Paren { expr } => self.narrowings(expr, table),
            _ => (vec![], vec![]),
        }
    }

    // ---------------------------------------------------------------- recherche

    fn lookup(&self, name: &str, table: usize) -> Option<Resolved> {
        if self.frames.iter().any(|f| f.locals.contains_key(name)) {
            return Some(Resolved::Local);
        }
        let mut t = Some(table);
        while let Some(id) = t {
            if let Some(sym) = self.tables[id].names.get(name) {
                return Some(Resolved::Sym(sym.clone()));
            }
            t = self.tables[id].parent;
        }
        if let Some((t, n)) = self.imports[self.file].get(name) {
            return self.tables[*t].names.get(n).cloned().map(Resolved::Sym);
        }
        match name {
            "print" => Some(Resolved::Sym(Sym::Builtin(Builtin::Print))),
            "sleep" => Some(Resolved::Sym(Sym::Builtin(Builtin::Sleep))),
            "duplicate" => Some(Resolved::Sym(Sym::Builtin(Builtin::Duplicate))),
            _ => None,
        }
    }

    /// Résolution d'un nom avec son type déclaré (variables uniquement).
    fn lookup_typed(&self, name: &str, table: usize) -> Option<(Resolved, Ty)> {
        if let Some((_, ty)) = self.frames.iter().rev().find_map(|f| f.locals.get(name)) {
            return Some((Resolved::Local, ty.clone()));
        }
        match self.lookup(name, table)? {
            Resolved::Sym(Sym::Global { ty }) => {
                Some((Resolved::Sym(Sym::Global { ty: ty.clone() }), ty))
            }
            other => Some((other, Ty::Unknown)),
        }
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
        let mut aliases: Vec<_> = self.imports[self.file]
            .iter()
            .filter(|(alias, (_, original))| *original == ident.name && **alias != ident.name)
            .map(|(alias, _)| format!("`{alias}`"))
            .collect();
        aliases.sort();
        if !aliases.is_empty() {
            d = d.suggestion(format!(
                "`{}` est importé dans ce fichier sous le nom {}",
                ident.name,
                aliases.join(" ou ")
            ));
        } else if elsewhere {
            d = d.suggestion(format!(
                "`{0}` existe dans un autre fichier : ajouter `import {0};`",
                ident.name
            ));
        } else if ident.is_local() {
            d = d.suggestion(
                "une variable locale doit être déclarée avant usage, dans ce bloc ou un bloc parent",
            );
        }
        self.report(d);
    }

    // ---------------------------------------------------------------- instructions

    fn check_block_in_current_frame(&mut self, block: &Block, table: usize) {
        for stmt in &block.stmts {
            self.check_stmt(stmt, table);
        }
    }

    fn check_block(&mut self, block: &Block, table: usize, narrow: Narrowings) {
        self.with_narrowings(narrow, |a| a.check_block_in_current_frame(block, table));
    }

    fn check_condition(&mut self, cond: &Expr, table: usize) {
        let t = self.check_expr(cond, table);
        if !t.ty.is_unknown() && t.ty != Ty::Bool {
            self.report(
                Diagnostic::new(
                    &codes::TYPE_MISMATCH,
                    cond.span,
                    "une condition doit être un `Bool`",
                )
                .expected("Bool")
                .received(t.ty.to_string()),
            );
        }
    }

    /// Avant une boucle : tout narrowing que le corps pourrait invalider est oublié,
    /// car à la deuxième itération il ne serait plus vrai.
    fn prepare_loop(&mut self, parts: &[&Block], extra: &[&Stmt], table: usize) {
        let mut assigned = HashSet::new();
        let mut calls = false;
        for b in parts {
            self.scan_block(b, table, &mut assigned, &mut calls);
        }
        for s in extra {
            self.scan_stmt(s, table, &mut assigned, &mut calls);
        }
        for name in assigned {
            self.forget_narrowing(&name);
        }
        if calls {
            self.forget_global_narrowings();
        }
    }

    fn scan_block(
        &self,
        b: &Block,
        table: usize,
        assigned: &mut HashSet<String>,
        calls: &mut bool,
    ) {
        for s in &b.stmts {
            self.scan_stmt(s, table, assigned, calls);
        }
    }

    fn scan_stmt(&self, s: &Stmt, table: usize, assigned: &mut HashSet<String>, calls: &mut bool) {
        let mut exprs: Vec<&Expr> = Vec::new();
        match s {
            Stmt::Local(v) => exprs.extend(v.init.iter()),
            Stmt::Assign { target, value, .. } => {
                if let ExprKind::Name { ident } = &target.kind {
                    assigned.insert(ident.name.clone());
                }
                exprs.push(target);
                exprs.push(value);
            }
            Stmt::If(i) => {
                let mut cur = Some(i);
                while let Some(i) = cur {
                    exprs.push(&i.cond);
                    self.scan_block(&i.then_block, table, assigned, calls);
                    cur = match &i.else_branch {
                        Some(Else::Block(b)) => {
                            self.scan_block(b, table, assigned, calls);
                            None
                        }
                        Some(Else::If(n)) => Some(n),
                        None => None,
                    };
                }
            }
            Stmt::While { cond, body, .. } => {
                exprs.push(cond);
                self.scan_block(body, table, assigned, calls);
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                exprs.extend(init.init.iter());
                exprs.push(cond);
                self.scan_stmt(step, table, assigned, calls);
                self.scan_block(body, table, assigned, calls);
            }
            Stmt::Foreach { iter, body, .. } => {
                exprs.push(iter);
                self.scan_block(body, table, assigned, calls);
            }
            Stmt::Return { value, .. } => exprs.extend(value.iter()),
            Stmt::Emit {
                args, background, ..
            } => {
                *calls |= !background;
                exprs.extend(args.iter());
            }
            Stmt::Expr { expr, .. } => exprs.push(expr),
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
        for e in exprs {
            self.scan_expr(e, table, calls);
        }
    }

    fn scan_expr(&self, e: &Expr, table: usize, calls: &mut bool) {
        match &e.kind {
            ExprKind::Call { callee, args } => {
                // `sleep` laisse tourner les autres tâches, qui peuvent modifier les globales.
                let builtin = matches!(&callee.kind, ExprKind::Name { ident }
                    if matches!(self.lookup(&ident.name, table),
                        Some(Resolved::Sym(Sym::Builtin(b))) if b != Builtin::Sleep));
                if !builtin {
                    *calls = true;
                }
                self.scan_expr(callee, table, calls);
                for a in args {
                    self.scan_expr(a, table, calls);
                }
            }
            ExprKind::Member { object, .. } => self.scan_expr(object, table, calls),
            ExprKind::Start { call } => self.scan_expr(call, table, calls),
            ExprKind::Await { expr } => {
                *calls = true;
                self.scan_expr(expr, table, calls);
            }
            ExprKind::Index { object, index } => {
                self.scan_expr(object, table, calls);
                self.scan_expr(index, table, calls);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Paren { expr } | ExprKind::Is { expr, .. } => {
                self.scan_expr(expr, table, calls)
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.scan_expr(lhs, table, calls);
                self.scan_expr(rhs, table, calls);
            }
            ExprKind::Array { items } => {
                for i in items {
                    self.scan_expr(i, table, calls);
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    self.scan_expr(&f.value, table, calls);
                }
            }
            _ => {}
        }
    }

    fn loop_body(&mut self, body: &Block, table: usize) {
        self.loop_depth += 1;
        self.check_block(body, table, vec![]);
        self.loop_depth -= 1;
    }

    fn check_stmt(&mut self, stmt: &Stmt, table: usize) {
        match stmt {
            Stmt::Local(v) => {
                let ty = self.type_decl(&v.ty, table, false, true);
                // L'initialisation est vérifiée avant la déclaration : `_x: Int = _x;` est refusé.
                let init_ty = v.init.as_ref().map(|init| {
                    let value = self.check_expr(init, table);
                    self.expect_assignable(&value.ty, &ty, init.span);
                    value.ty
                });
                self.declare_local(&v.name, ty.clone());
                // Une locale DualType initialisée a un type actif connu.
                if let Some(t) = init_ty {
                    if ty.is_dual() && !t.is_dual() && t.assignable_to(&ty) {
                        if let Some(f) = self.frames.last_mut() {
                            f.narrow.insert(v.name.name.clone(), t);
                        }
                    }
                }
            }
            Stmt::Assign { target, value, .. } => {
                let t = self.check_any(target, table);
                if let Resolved::Sym(
                    sym @ (Sym::Func { .. }
                    | Sym::Scope { .. }
                    | Sym::Builtin(_)
                    | Sym::Struct { .. }
                    | Sym::Enum { .. }),
                ) = &t.res
                {
                    self.report(Diagnostic::new(
                        &codes::NOT_ASSIGNABLE,
                        target.span,
                        format!("impossible d'affecter une valeur à {}", sym.describe()),
                    ));
                    self.check_expr(value, table);
                    return;
                }
                if let Resolved::Method { .. } = t.res {
                    self.report(Diagnostic::new(
                        &codes::NOT_ASSIGNABLE,
                        target.span,
                        "impossible d'affecter une valeur à une méthode",
                    ));
                    self.check_expr(value, table);
                    return;
                }
                let v = self.check_expr(value, table);
                self.expect_assignable(&v.ty, &t.declared, value.span);
                // Après `x = valeur`, le type actif de x est celui de la valeur.
                if let ExprKind::Name { ident } = &target.kind {
                    self.forget_narrowing(&ident.name);
                    if t.declared.is_dual() && !v.ty.is_dual() && v.ty.assignable_to(&t.declared) {
                        if let Some(f) = self.frames.last_mut() {
                            f.narrow.insert(ident.name.clone(), v.ty.clone());
                        }
                    }
                }
            }
            Stmt::If(i) => self.check_if(i, table),
            Stmt::While { cond, body, span } => {
                self.prepare_loop(&[body], &[], table);
                self.check_condition(cond, table);
                self.loop_body(body, table);
                self.advise_while(cond, body, *span, table);
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                self.frames.push(Frame::default());
                self.check_stmt(&Stmt::Local(init.clone()), table);
                self.prepare_loop(&[body], &[step], table);
                self.check_condition(cond, table);
                self.check_stmt(step, table);
                self.loop_body(body, table);
                self.frames.pop();
            }
            Stmt::Foreach {
                binding,
                ty,
                iter,
                body,
                ..
            } => {
                let it = self.check_expr(iter, table);
                let element = match &it.ty {
                    Ty::Array(e) => (**e).clone(),
                    Ty::Unknown | Ty::Map(..) => Ty::Unknown,
                    other => {
                        self.report(
                            Diagnostic::new(
                                &codes::TYPE_MISMATCH,
                                iter.span,
                                "`foreach` parcourt un tableau",
                            )
                            .expected("Array")
                            .received(other.to_string()),
                        );
                        Ty::Unknown
                    }
                };
                let binding_ty = match ty {
                    Some(decl) => {
                        let declared = self.type_decl(decl, table, false, true);
                        self.expect_assignable(&element, &declared, binding.span);
                        declared
                    }
                    None => element,
                };
                self.prepare_loop(&[body], &[], table);
                self.frames.push(Frame::default());
                self.declare_local(binding, binding_ty);
                self.loop_body(body, table);
                self.frames.pop();
            }
            Stmt::Return { value, span } => self.check_return(value.as_ref(), *span, table),
            Stmt::Break { span } | Stmt::Continue { span } => {
                if self.loop_depth == 0 {
                    self.report(Diagnostic::new(
                        &codes::LOOP_CONTROL_OUTSIDE_LOOP,
                        *span,
                        "`break` / `continue` utilisé hors d'une boucle",
                    ));
                }
            }
            Stmt::Emit {
                event,
                args,
                background,
                span,
            } => {
                let params = self.check_event_use(event, args.len(), *span, "valeurs");
                for (i, a) in args.iter().enumerate() {
                    let t = self.check_expr(a, table);
                    if let Some(p) = params.get(i) {
                        self.expect_assignable(&t.ty, p, a.span);
                    }
                }
                // `start emit` n'exécute rien tout de suite.
                if !background {
                    self.forget_global_narrowings();
                }
            }
            Stmt::Expr { expr, .. } => {
                self.check_expr(expr, table);
            }
        }
    }

    fn check_return(&mut self, value: Option<&Expr>, span: Span, table: usize) {
        let ret = self.ret.clone().unwrap_or(Ty::Unknown);
        match value {
            Some(v) => {
                let t = self.check_expr(v, table);
                if ret == Ty::Void {
                    self.report(
                        Diagnostic::new(
                            &codes::UNEXPECTED_RETURN_VALUE,
                            v.span,
                            "cette fonction est `Void` : elle ne retourne pas de valeur",
                        )
                        .suggestion("écrire `return;` ou changer le type de retour"),
                    );
                } else {
                    self.expect_assignable(&t.ty, &ret, v.span);
                }
            }
            None if !matches!(ret, Ty::Void | Ty::Unknown) => self.report(
                Diagnostic::new(
                    &codes::MISSING_RETURN_VALUE,
                    span,
                    format!("une valeur `{ret}` doit être retournée"),
                )
                .expected(ret.to_string()),
            ),
            None => {}
        }
    }

    fn check_if(&mut self, i: &IfStmt, table: usize) {
        self.check_condition(&i.cond, table);
        let (then_n, else_n) = self.narrowings(&i.cond, table);
        self.check_block(&i.then_block, table, then_n);
        match &i.else_branch {
            Some(Else::Block(b)) => self.check_block(b, table, else_n),
            Some(Else::If(nested)) => self.with_narrowings(else_n, |a| a.check_if(nested, table)),
            None => {}
        }
    }

    // ---------------------------------------------------------------- expressions

    /// Expression utilisée comme valeur : une fonction, un scope ou un type n'en est pas une.
    fn check_expr(&mut self, expr: &Expr, table: usize) -> Typed {
        let t = self.check_any(expr, table);
        match &t.res {
            Resolved::Sym(Sym::Global { .. }) | Resolved::Local | Resolved::Opaque => t,
            Resolved::Method { .. } => {
                self.report(
                    Diagnostic::new(
                        &codes::NOT_A_VALUE,
                        expr.span,
                        "une méthode n'est pas une valeur",
                    )
                    .suggestion("l'appeler : `valeur.methode(...)`"),
                );
                Typed::value(Ty::Unknown)
            }
            Resolved::Sym(sym) => {
                let suggestion = match sym {
                    Sym::Func { .. } | Sym::Builtin(_) => "l'appeler : `nom(...)`",
                    Sym::Enum { .. } => "choisir une variante : `Nom.Variante`",
                    Sym::Struct { .. } => "créer une valeur : `Nom { champ: valeur, ... }`",
                    _ => "utiliser l'un de ses membres : `Scope.membre`",
                };
                self.report(
                    Diagnostic::new(
                        &codes::NOT_A_VALUE,
                        expr.span,
                        format!("{} n'est pas une valeur", sym.describe()),
                    )
                    .suggestion(suggestion),
                );
                Typed::value(Ty::Unknown)
            }
        }
    }

    /// Expression quelconque, y compris un nom qui désigne une fonction, un scope ou un type
    /// (appelé, parcouru avec `.` ou cible d'affectation).
    fn check_any(&mut self, expr: &Expr, table: usize) -> Typed {
        match &expr.kind {
            ExprKind::Int { text } => {
                if text.parse::<i64>().is_err() {
                    self.report(Diagnostic::new(
                        &codes::INTEGER_OVERFLOW,
                        expr.span,
                        format!(
                            "`{text}` dépasse la valeur maximale de `Int` ({})",
                            i64::MAX
                        ),
                    ));
                }
                Typed::value(Ty::Int)
            }
            ExprKind::Float { .. } => Typed::value(Ty::Float),
            ExprKind::Str { .. } => Typed::value(Ty::String),
            ExprKind::Bool { .. } => Typed::value(Ty::Bool),
            ExprKind::Error => Typed::value(Ty::Unknown),
            ExprKind::Name { ident } => match self.lookup_typed(&ident.name, table) {
                Some((res, declared)) => {
                    let ty = match res {
                        Resolved::Local | Resolved::Sym(Sym::Global { .. }) => {
                            self.active_ty(&ident.name, &declared)
                        }
                        _ => Ty::Unknown,
                    };
                    Typed { res, ty, declared }
                }
                None => {
                    self.undeclared(ident);
                    Typed::value(Ty::Unknown)
                }
            },
            ExprKind::SelfRef => {
                let found = self
                    .frames
                    .iter()
                    .rev()
                    .find_map(|f| f.locals.get(SELF_LOCAL))
                    .map(|(_, ty)| ty.clone());
                match found {
                    Some(ty) => Typed {
                        res: Resolved::Opaque,
                        declared: ty.clone(),
                        ty,
                    },
                    None => {
                        self.report(
                            Diagnostic::new(
                                &codes::SELF_OUTSIDE_METHOD,
                                expr.span,
                                "`self` n'existe que dans une méthode de structure",
                            )
                            .suggestion(
                                "déclarer la fonction dans le corps d'un `struct`, ou passer la valeur en paramètre",
                            ),
                        );
                        Typed::value(Ty::Unknown)
                    }
                }
            }
            ExprKind::Member { object, field } => self.check_member(object, field, table),
            ExprKind::StructLit { path, fields } => {
                self.check_struct_lit(path, fields, expr.span, table)
            }
            ExprKind::Call { callee, args } => self.check_call(expr, callee, args, table),
            ExprKind::Start { call } => {
                let ty = match &call.kind {
                    ExprKind::Call { callee, args } => {
                        let t = self.check_call(call, callee, args, table).ty;
                        if let Some(f) = self.last_call {
                            self.started.insert(f);
                        }
                        Ty::Task(Box::new(t))
                    }
                    // Déjà signalé par le parser (MCF0121).
                    _ => {
                        self.check_any(call, table);
                        Ty::Unknown
                    }
                };
                Typed::value(ty)
            }
            ExprKind::Await { expr: inner } => {
                let t = self.check_expr(inner, table).ty;
                let ty = match t {
                    Ty::Task(r) => *r,
                    Ty::Unknown => Ty::Unknown,
                    ref d if d.is_dual() => {
                        self.not_narrowed_or_invalid(d, inner.span, "await");
                        Ty::Unknown
                    }
                    other => {
                        self.report(
                            Diagnostic::new(
                                &codes::TYPE_MISMATCH,
                                inner.span,
                                "`await` attend une tâche lancée par `start`",
                            )
                            .expected("Task<...>")
                            .received(other.to_string()),
                        );
                        Ty::Unknown
                    }
                };
                // Pendant l'attente, les autres tâches peuvent modifier les globales.
                self.forget_global_narrowings();
                Typed::value(ty)
            }
            ExprKind::Index { object, index } => {
                let obj = self.check_expr(object, table);
                let idx = self.check_expr(index, table);
                let ty = match &obj.ty {
                    Ty::Array(e) => {
                        self.expect_assignable(&idx.ty, &Ty::Int, index.span);
                        (**e).clone()
                    }
                    Ty::Map(k, v) => {
                        self.expect_assignable(&idx.ty, k, index.span);
                        (**v).clone()
                    }
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.not_narrowed_or_invalid(other, object.span, "[]");
                        Ty::Unknown
                    }
                };
                Typed {
                    res: Resolved::Opaque,
                    declared: ty.clone(),
                    ty,
                }
            }
            ExprKind::Paren { expr } => {
                let inner = self.check_expr(expr, table);
                Typed::value(inner.ty)
            }
            ExprKind::Unary { op, expr: inner } => {
                let t = self.check_expr(inner, table).ty;
                let ty = match (op, &t) {
                    (_, Ty::Unknown) => Ty::Unknown,
                    (UnaryOp::Not, Ty::Bool) => Ty::Bool,
                    (UnaryOp::Neg, Ty::Int | Ty::Float) => t.clone(),
                    (op, other) => {
                        let sym = if *op == UnaryOp::Not { "!" } else { "-" };
                        self.not_narrowed_or_invalid(other, inner.span, sym);
                        Ty::Unknown
                    }
                };
                Typed::value(ty)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.check_expr(lhs, table).ty;
                // `a && b` : b est évalué seulement si a est vrai, il profite donc du narrowing de a.
                let r = match op {
                    BinaryOp::And => {
                        let (n, _) = self.narrowings(lhs, table);
                        self.with_narrowings(n, |a| a.check_expr(rhs, table).ty)
                    }
                    BinaryOp::Or => {
                        let (_, n) = self.narrowings(lhs, table);
                        self.with_narrowings(n, |a| a.check_expr(rhs, table).ty)
                    }
                    _ => self.check_expr(rhs, table).ty,
                };
                self.advise_equality(*op, lhs, rhs, &l, &r, expr.span);
                Typed::value(self.binary_ty(*op, &l, &r, lhs.span, rhs.span, expr.span))
            }
            ExprKind::Is { expr: inner, ty } => {
                let t = self.check_expr(inner, table).ty;
                let tested = self.type_ref(ty, table, true);
                if !t.is_unknown() && !tested.is_unknown() {
                    let possible = t.members().iter().any(|m| m.same(&tested));
                    if !possible {
                        self.report(
                            Diagnostic::new(
                                &codes::IMPOSSIBLE_TYPE_TEST,
                                ty.span,
                                format!("une valeur `{t}` ne peut jamais être `{tested}`"),
                            )
                            .expected(t.to_string()),
                        );
                    } else if !t.is_dual() {
                        self.report(Diagnostic::new(
                            &codes::TYPE_TEST_ALWAYS_TRUE,
                            expr.span,
                            format!("le type actif est déjà `{t}` : ce test est toujours vrai"),
                        ));
                    }
                }
                Typed::value(Ty::Bool)
            }
            ExprKind::Array { items } => {
                let mut element = Ty::Unknown;
                for item in items {
                    let t = self.check_expr(item, table).ty;
                    if element.is_unknown() {
                        element = t;
                    } else if !t.same(&element) {
                        self.report(
                            Diagnostic::new(
                                &codes::TYPE_MISMATCH,
                                item.span,
                                "tous les éléments d'un tableau doivent avoir le même type",
                            )
                            .expected(element.to_string())
                            .received(t.to_string()),
                        );
                    }
                }
                Typed::value(Ty::Array(Box::new(element)))
            }
        }
    }

    fn check_member(&mut self, object: &Expr, field: &Ident, table: usize) -> Typed {
        let obj = self.check_any(object, table);
        match (&obj.res, &obj.ty) {
            // `Scope.membre`
            (Resolved::Sym(Sym::Scope { table: scope }), _) => {
                match self.tables[*scope].names.get(&field.name).cloned() {
                    Some(sym) => {
                        let declared = match &sym {
                            Sym::Global { ty } => ty.clone(),
                            _ => Ty::Unknown,
                        };
                        Typed {
                            res: Resolved::Sym(sym),
                            ty: declared.clone(),
                            declared,
                        }
                    }
                    None => {
                        self.report(Diagnostic::new(
                            &codes::UNKNOWN_MEMBER,
                            field.span,
                            format!("le scope ne contient pas `{}`", field.name),
                        ));
                        Typed::value(Ty::Unknown)
                    }
                }
            }
            // `Enum.Variante`
            (Resolved::Sym(Sym::Enum { id }), _) => {
                let info = &self.enums[*id];
                let ty = Ty::Enum {
                    id: *id,
                    name: info.name.clone(),
                };
                if !info.variants.contains(&field.name) {
                    let list = info.variants.join(", ");
                    let name = info.name.clone();
                    self.report(
                        Diagnostic::new(
                            &codes::UNKNOWN_VARIANT,
                            field.span,
                            format!(
                                "l'énumération `{name}` n'a pas de variante `{}`",
                                field.name
                            ),
                        )
                        .expected(list),
                    );
                }
                Typed::value(ty)
            }
            // `valeur.champ`
            (_, Ty::Struct { id, name }) => {
                match self.structs[*id]
                    .fields
                    .iter()
                    .find(|(n, _)| *n == field.name)
                {
                    Some((_, ty)) => Typed {
                        res: Resolved::Opaque,
                        ty: ty.clone(),
                        declared: ty.clone(),
                    },
                    None => {
                        let info = &self.structs[*id];
                        if let Some(m) = info.methods.iter().find(|m| m.name == field.name) {
                            return Typed {
                                res: Resolved::Method {
                                    params: m.params.clone(),
                                    ret: m.ret.clone(),
                                    origin: Some(m.origin),
                                },
                                ty: Ty::Unknown,
                                declared: Ty::Unknown,
                            };
                        }
                        let list = info
                            .fields
                            .iter()
                            .map(|(n, _)| n.clone())
                            .chain(info.methods.iter().map(|m| format!("{}()", m.name)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let name = name.clone();
                        self.report(
                            Diagnostic::new(
                                &codes::UNKNOWN_FIELD,
                                field.span,
                                format!("la structure `{name}` n'a pas de champ `{}`", field.name),
                            )
                            .expected(list),
                        );
                        Typed::value(Ty::Unknown)
                    }
                }
            }
            (_, Ty::Unknown) => Typed::value(Ty::Unknown),
            (_, other) => {
                let other = other.clone();
                if let Some((params, ret)) = types::native_method(&other, &field.name) {
                    return Typed {
                        res: Resolved::Method {
                            params,
                            ret,
                            origin: None,
                        },
                        ty: Ty::Unknown,
                        declared: Ty::Unknown,
                    };
                }
                if other.is_dual() {
                    self.not_narrowed_or_invalid(&other, object.span, ".");
                } else {
                    let methods = types::native_method_names(&other);
                    let mut d = Diagnostic::new(
                        &codes::UNKNOWN_FIELD,
                        field.span,
                        format!("une valeur `{other}` n'a pas de membre `{}`", field.name),
                    );
                    if !methods.is_empty() {
                        d = d.expected(methods.join(", "));
                    }
                    self.report(d);
                }
                Typed::value(Ty::Unknown)
            }
        }
    }

    fn check_struct_lit(
        &mut self,
        path: &Path,
        fields: &[FieldInit],
        span: Span,
        table: usize,
    ) -> Typed {
        let name = path_text(path);
        let id = match self.resolve_path(path, table) {
            Some(Sym::Struct { id }) => id,
            Some(other) => {
                self.report(Diagnostic::new(
                    &codes::NOT_A_STRUCT,
                    path.span,
                    format!("`{name}` est {}, pas une structure", other.describe()),
                ));
                for f in fields {
                    self.check_expr(&f.value, table);
                }
                return Typed::value(Ty::Unknown);
            }
            None => {
                self.undeclared(&path.segments[0]);
                for f in fields {
                    self.check_expr(&f.value, table);
                }
                return Typed::value(Ty::Unknown);
            }
        };
        let declared = self.structs[id].fields.clone();
        let mut seen: Vec<&str> = Vec::new();
        for f in fields {
            let value = self.check_expr(&f.value, table);
            if seen.contains(&f.name.name.as_str()) {
                self.report(Diagnostic::new(
                    &codes::DUPLICATE_FIELD,
                    f.name.span,
                    format!("le champ `{}` est initialisé deux fois", f.name.name),
                ));
                continue;
            }
            seen.push(&f.name.name);
            match declared.iter().find(|(n, _)| *n == f.name.name) {
                Some((_, ty)) => self.expect_assignable(&value.ty, ty, f.value.span),
                None => self.report(
                    Diagnostic::new(
                        &codes::UNKNOWN_FIELD,
                        f.name.span,
                        format!("la structure `{name}` n'a pas de champ `{}`", f.name.name),
                    )
                    .expected(
                        declared
                            .iter()
                            .map(|(n, _)| n.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ),
            }
        }
        let missing: Vec<&str> = declared
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| !seen.contains(n))
            .collect();
        if !missing.is_empty() {
            self.report(
                Diagnostic::new(
                    &codes::MISSING_FIELD,
                    span,
                    format!(
                        "champ(s) manquant(s) pour `{name}` : {}",
                        missing.join(", ")
                    ),
                )
                .suggestion(
                    "chaque champ doit recevoir une valeur : pas de valeur par défaut implicite",
                ),
            );
        }
        Typed::value(Ty::Struct {
            id,
            name: self.structs[id].name.clone(),
        })
    }

    fn check_call(&mut self, expr: &Expr, callee: &Expr, args: &[Expr], table: usize) -> Typed {
        let c = self.check_any(callee, table);
        let arg_tys: Vec<(Ty, Span)> = args
            .iter()
            .map(|a| (self.check_expr(a, table).ty, a.span))
            .collect();
        let count_error = |a: &mut Self, expected: usize| {
            if expected != args.len() {
                a.report(
                    Diagnostic::new(
                        &codes::ARGUMENT_COUNT_MISMATCH,
                        expr.span,
                        format!(
                            "{expected} argument(s) attendu(s), {} fourni(s)",
                            args.len()
                        ),
                    )
                    .expected(expected.to_string())
                    .received(args.len().to_string()),
                );
            }
        };
        self.last_call = match &c.res {
            Resolved::Sym(Sym::Func { origin, .. }) => Some(*origin),
            Resolved::Method { origin, .. } => *origin,
            _ => None,
        };
        let ret = match c.res {
            Resolved::Sym(Sym::Func { params, ret, .. }) => {
                count_error(self, params.len());
                for ((t, span), p) in arg_tys.iter().zip(&params) {
                    self.expect_assignable(t, p, *span);
                }
                // La fonction appelée peut modifier des globales.
                self.forget_global_narrowings();
                ret
            }
            Resolved::Method {
                params,
                ret,
                origin,
            } => {
                count_error(self, params.len());
                for ((t, span), p) in arg_tys.iter().zip(&params) {
                    self.expect_assignable(t, p, *span);
                }
                if origin.is_some() {
                    // Une méthode MCF peut modifier des globales, comme une fonction.
                    self.forget_global_narrowings();
                }
                ret
            }
            Resolved::Sym(Sym::Builtin(Builtin::Duplicate)) => {
                count_error(self, 1);
                match arg_tys.first() {
                    Some((Ty::Void, span)) => {
                        self.report(
                            Diagnostic::new(
                                &codes::TYPE_MISMATCH,
                                *span,
                                "`duplicate` attend une valeur, pas `Void`",
                            )
                            .received("Void"),
                        );
                        Ty::Unknown
                    }
                    // La copie a le type (actif) de l'original.
                    Some((t, _)) if args.len() == 1 => t.clone(),
                    _ => Ty::Unknown,
                }
            }
            Resolved::Sym(Sym::Builtin(b)) => {
                count_error(self, 1);
                if let Some((t, span)) = arg_tys.first() {
                    match b {
                        Builtin::Print if *t == Ty::Void => self.report(
                            Diagnostic::new(
                                &codes::TYPE_MISMATCH,
                                *span,
                                "`print` attend une valeur, pas `Void`",
                            )
                            .received("Void"),
                        ),
                        Builtin::Sleep => {
                            self.expect_assignable(t, &Ty::Int, *span);
                            // Pendant la pause, les autres tâches peuvent modifier les globales.
                            self.forget_global_narrowings();
                        }
                        _ => {}
                    }
                }
                Ty::Void
            }
            Resolved::Opaque if c.ty.is_unknown() => Ty::Unknown,
            _ => {
                let mut d = Diagnostic::new(
                    &codes::NOT_CALLABLE,
                    callee.span,
                    "cette expression n'est pas une fonction",
                );
                if let Resolved::Sym(Sym::Struct { .. }) = c.res {
                    d = d.suggestion("une structure se crée avec `Nom { champ: valeur, ... }`");
                }
                self.report(d);
                Ty::Unknown
            }
        };
        Typed::value(ret)
    }

    // ---------------------------------------------------------------- conseils (05xx)

    /// Conseils sur une boucle `while` qui ne contient aucun point de suspension possible :
    /// ni `sleep`, ni `await`, ni appel de fonction MCF (qui pourrait en contenir), ni `emit`.
    fn advise_while(&mut self, cond: &Expr, body: &Block, span: Span, table: usize) {
        let mut assigned = HashSet::new();
        let mut may_pause = false;
        self.scan_block(body, table, &mut assigned, &mut may_pause);
        self.scan_expr(cond, table, &mut may_pause);
        if may_pause {
            return;
        }
        let head = Span::new(span.start, cond.span.end + 1);
        if is_true_literal(cond) && !leaves_loop(body) {
            self.report(
                Diagnostic::new(
                    &codes::ENDLESS_LOOP_WITHOUT_PAUSE,
                    head,
                    "cette boucle ne s'arrête jamais (`while (true)` sans `break` ni `return`) et ne fait aucune pause",
                )
                .suggestion(
                    "le programme va tourner à fond sans jamais finir. Pour une boucle de jeu ou d'attente, ajoute `sleep(16);` dans la boucle (environ 60 tours par seconde) ; sinon prévois un `break` pour en sortir",
                ),
            );
            return;
        }
        if let Some(f) = self.current_func {
            self.loops_without_pause.push((f, self.file, head));
        }
    }

    /// Une boucle sans pause dans une fonction lancée avec `start` monopolise l'exécution :
    /// une seule tâche tourne à la fois (DECISIONS D42).
    fn advise_loops_in_tasks(&mut self) {
        for (f, file, span) in std::mem::take(&mut self.loops_without_pause) {
            if !self.started.contains(&f) {
                continue;
            }
            self.file = file;
            self.report(
                Diagnostic::new(
                    &codes::LOOP_WITHOUT_PAUSE_IN_TASK,
                    span,
                    "cette boucle ne fait aucune pause, alors que sa fonction est lancée avec `start` : tant qu'elle tourne, les autres tâches (et `main`) attendent",
                )
                .suggestion(
                    "si la boucle est courte, ce n'est pas grave. Si elle peut durer (beaucoup de tours, attente d'un état), ajoute `sleep(0);` dans la boucle pour laisser les autres tâches avancer à chaque tour",
                ),
            );
        }
    }

    /// Conseils sur `==` / `!=` (types déjà connus).
    fn advise_equality(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        l: &Ty,
        r: &Ty,
        whole: Span,
    ) {
        if !matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            return;
        }
        let sym = op_symbol(op);
        let literal = |e: &Expr| match &e.kind {
            ExprKind::Bool { value } => Some(*value),
            _ => None,
        };
        if let Some((value, other)) = literal(rhs)
            .map(|v| (v, lhs))
            .or(literal(lhs).map(|v| (v, rhs)))
        {
            if *l == Ty::Bool && *r == Ty::Bool {
                let positive = value == (op == BinaryOp::Eq);
                let text = other_text(other);
                self.report(
                    Diagnostic::new(
                        &codes::COMPARISON_WITH_BOOL_LITERAL,
                        whole,
                        format!("comparer un booléen avec `{value}` n'est pas nécessaire"),
                    )
                    .suggestion(if positive {
                        format!("écrire simplement `{text}`")
                    } else {
                        format!("écrire `!{text}` (« pas {text} »)")
                    }),
                );
            }
            return;
        }
        // Deux littéraux (`3 == 3.0`) : aucun calcul, donc aucun arrondi possible.
        if (*l == Ty::Float || *r == Ty::Float)
            && !(is_number_literal(lhs) && is_number_literal(rhs))
        {
            self.report(
                Diagnostic::new(
                    &codes::FLOAT_EQUALITY,
                    whole,
                    format!("`{sym}` entre décimaux : un calcul peut donner 0.30000000000000004 au lieu de 0.3, et l'égalité échoue"),
                )
                .suggestion(
                    "comparer avec une petite marge, ex. `_d: Float = a - b;` puis `_d < 0.0001 && _d > -0.0001`, ou travailler en entiers (centimes plutôt qu'euros)",
                ),
            );
        }
    }

    fn not_narrowed_or_invalid(&mut self, ty: &Ty, span: Span, op: &str) {
        if ty.is_dual() {
            self.report(
                Diagnostic::new(
                    &codes::DUAL_TYPE_NOT_NARROWED,
                    span,
                    format!("valeur `{ty}` utilisée avec `{op}` sans connaître son type actif"),
                )
                .suggestion("vérifier d'abord le type : `if (valeur is Int) then { ... }`"),
            );
        } else {
            self.report(Diagnostic::new(
                &codes::INVALID_OPERANDS,
                span,
                format!("`{op}` ne s'applique pas à une valeur `{ty}`"),
            ));
        }
    }

    fn binary_ty(&mut self, op: BinaryOp, l: &Ty, r: &Ty, ls: Span, rs: Span, whole: Span) -> Ty {
        use BinaryOp::*;
        let result_if_unknown = match op {
            Or | And | Eq | NotEq | Lt | Gt | Le | Ge => Ty::Bool,
            _ => Ty::Unknown,
        };
        for (t, span) in [(l, ls), (r, rs)] {
            if t.is_dual() {
                self.not_narrowed_or_invalid(t, span, op_symbol(op));
                return result_if_unknown;
            }
        }
        if l.is_unknown() || r.is_unknown() {
            return result_if_unknown;
        }
        let numeric = |a: &Ty, b: &Ty| -> Option<Ty> {
            match (a, b) {
                (Ty::Int, Ty::Int) => Some(Ty::Int),
                // Seule promotion admise : Int avec Float dans un calcul donne Float (DECISIONS D23).
                (Ty::Int | Ty::Float, Ty::Int | Ty::Float) => Some(Ty::Float),
                _ => None,
            }
        };
        let concat_part =
            |t: &Ty| matches!(t, Ty::String | Ty::Int | Ty::Float | Ty::Bool | Ty::Char);
        let ty = match op {
            Or | And => (l == &Ty::Bool && r == &Ty::Bool).then_some(Ty::Bool),
            Eq | NotEq => (l.same(r) || numeric(l, r).is_some()).then_some(Ty::Bool),
            Lt | Gt | Le | Ge => numeric(l, r).map(|_| Ty::Bool),
            // Concaténation : une chaîne avec une valeur simple donne une chaîne (DECISIONS D22).
            Add if (l == &Ty::String && concat_part(r)) || (r == &Ty::String && concat_part(l)) => {
                Some(Ty::String)
            }
            Add | Sub | Mul | Div | Rem => numeric(l, r),
        };
        match ty {
            Some(t) => t,
            None => {
                self.report(
                    Diagnostic::new(
                        &codes::INVALID_OPERANDS,
                        whole,
                        format!("`{}` impossible entre `{l}` et `{r}`", op_symbol(op)),
                    )
                    .received(format!("{l} {} {r}", op_symbol(op))),
                );
                result_if_unknown
            }
        }
    }
}

// ==================================================================== aides des conseils

fn is_true_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Bool { value } => *value,
        ExprKind::Paren { expr } => is_true_literal(expr),
        _ => false,
    }
}

fn is_number_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int { .. } | ExprKind::Float { .. } => true,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        }
        | ExprKind::Paren { expr } => is_number_literal(expr),
        _ => false,
    }
}

/// Le corps contient-il un `return`, ou un `break` qui sort de cette boucle-ci ?
fn leaves_loop(body: &Block) -> bool {
    fn stmt(s: &Stmt, nested: bool) -> bool {
        match s {
            Stmt::Return { .. } => true,
            Stmt::Break { .. } => !nested,
            Stmt::If(i) => {
                let mut cur = Some(i);
                while let Some(i) = cur {
                    if block(&i.then_block, nested) {
                        return true;
                    }
                    cur = match &i.else_branch {
                        Some(Else::Block(b)) => return block(b, nested),
                        Some(Else::If(n)) => Some(n),
                        None => None,
                    };
                }
                false
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } | Stmt::Foreach { body, .. } => {
                block(body, true)
            }
            _ => false,
        }
    }
    fn block(b: &Block, nested: bool) -> bool {
        b.stmts.iter().any(|s| stmt(s, nested))
    }
    block(body, false)
}

/// Texte court d'une expression simple pour un conseil (`ready`, `_done`, `p.alive`).
fn other_text(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Name { ident } => ident.name.clone(),
        ExprKind::SelfRef => "self".into(),
        ExprKind::Member { object, field } => format!("{}.{}", other_text(object), field.name),
        ExprKind::Paren { expr } => other_text(expr),
        _ => "la condition".into(),
    }
}
