//! Arbre syntaxique abstrait. Chaque nœud porte sa position (`Span`).

use mcf_diagnostics::Span;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Module {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum Item {
    Global(VarDecl),
    Func(FuncDecl),
    Scope(ScopeDecl),
    Import(ImportDecl),
    Event(EventDecl),
    On(OnHandler),
    Struct(StructDecl),
    Enum(EnumDecl),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn is_local(&self) -> bool {
        self.name.starts_with('_')
    }
}

/// Nom qualifié : `Player.name`, `OS.File`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub span: Span,
}

/// Référence à un type, éventuellement générique : `Int`, `Map<String, Int>`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TypeRef {
    pub path: Path,
    pub args: Vec<TypeRef>,
    pub span: Span,
}

/// Un ou deux types (DualType).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TypeDecl {
    pub types: Vec<TypeRef>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VarDecl {
    pub name: Ident,
    pub ty: TypeDecl,
    pub init: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeDecl,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FuncDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    /// `None` uniquement après une erreur MCF0111.
    pub ret: Option<TypeDecl>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScopeDecl {
    pub name: Ident,
    pub items: Vec<Item>,
    pub span: Span,
}

/// Champ de structure : `name: Type;`
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Field {
    pub name: Ident,
    pub ty: TypeDecl,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StructDecl {
    pub name: Ident,
    pub fields: Vec<Field>,
    /// Méthodes déclarées dans le corps : `func nom(...): Type { ... }` (receveur `self`).
    pub methods: Vec<FuncDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<Ident>,
    pub span: Span,
}

/// `champ: valeur` dans un littéral de structure.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ImportDecl {
    pub path: Path,
    /// `import Player.name as playerName;` : seul l'alias est visible dans le fichier.
    pub alias: Option<Ident>,
    pub span: Span,
}

impl ImportDecl {
    /// Nom sous lequel l'élément importé est visible : l'alias, sinon le dernier segment.
    pub fn local_name(&self) -> &Ident {
        self.alias
            .as_ref()
            .unwrap_or_else(|| &self.path.segments[self.path.segments.len() - 1])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventDecl {
    pub name: Path,
    pub params: Vec<Param>,
    /// `meta { champ: Type = défaut; }` (DECISIONS D46).
    pub meta: Vec<MetaField>,
    pub span: Span,
}

/// Métadonnée déclarée par un événement : toujours avec une valeur par défaut.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MetaField {
    pub name: Ident,
    pub ty: TypeDecl,
    pub default: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OnHandler {
    pub event: Path,
    pub bindings: Vec<Ident>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum Stmt {
    Local(VarDecl),
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    If(IfStmt),
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    For {
        init: VarDecl,
        cond: Expr,
        step: Box<Stmt>,
        body: Block,
        span: Span,
    },
    Foreach {
        binding: Ident,
        ty: Option<TypeDecl>,
        iter: Expr,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    /// `emit X(v);` ; `start emit X(v);` (`background`) lance chaque gestionnaire dans sa
    /// propre tâche au lieu de l'exécuter tout de suite (DECISIONS D44).
    Emit {
        event: Path,
        args: Vec<Expr>,
        /// `emit X(v) meta { champ: valeur };`
        meta: Vec<FieldInit>,
        background: bool,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IfStmt {
    pub cond: Expr,
    pub then_block: Block,
    pub else_branch: Option<Else>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum Else {
    Block(Block),
    If(Box<IfStmt>),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum ExprKind {
    Int {
        text: String,
    },
    Float {
        text: String,
    },
    Str {
        value: String,
    },
    Bool {
        value: bool,
    },
    Name {
        ident: Ident,
    },
    /// `self` : la structure qui reçoit l'appel, dans une méthode.
    SelfRef,
    Array {
        items: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Is {
        expr: Box<Expr>,
        ty: TypeRef,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Member {
        object: Box<Expr>,
        field: Ident,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Paren {
        expr: Box<Expr>,
    },
    /// `start f(x)` : lance l'appel dans une nouvelle tâche, vaut `Task<T>` (DECISIONS D41).
    Start {
        call: Box<Expr>,
    },
    /// `on Evenement => fonction` : abonne la fonction, vaut un `Listener` (DECISIONS D45).
    Listen {
        event: Path,
        handler: Path,
    },
    /// `await t` : attend la fin de la tâche `t` et vaut son résultat.
    Await {
        expr: Box<Expr>,
    },
    /// `User { id: 1, name: "Alex" }`
    StructLit {
        path: Path,
        fields: Vec<FieldInit>,
    },
    /// Placeholder produit après une erreur pour poursuivre l'analyse.
    Error,
}

impl Expr {
    pub fn is_place(&self) -> bool {
        match &self.kind {
            ExprKind::Name { .. } | ExprKind::Index { .. } | ExprKind::Member { .. } => true,
            ExprKind::Paren { expr } => expr.is_place(),
            _ => false,
        }
    }
}
