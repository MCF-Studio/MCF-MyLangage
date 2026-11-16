//! Parser descendant récursif. Grammaire : `docs/grammaire/grammaire.md`.
//!
//! Le parser ne s'arrête jamais à la première erreur : il se resynchronise pour
//! produire le plus de diagnostics utiles possible, sans cascade.

use crate::ast::*;
use crate::lexer::{lex, Token, TokenKind};
use mcf_diagnostics::{codes, Code, Diagnostic, Span};

pub struct ParseResult {
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse(src: &str) -> ParseResult {
    let (tokens, mut diagnostics) = lex(src);
    let mut p = Parser {
        src,
        tokens,
        pos: 0,
        diags: Vec::new(),
        panic: false,
    };
    let items = p.items_until_eof();
    diagnostics.append(&mut p.diags);
    diagnostics.sort_by_key(|d| d.span.start);
    ParseResult {
        module: Module { items },
        diagnostics,
    }
}

type K = TokenKind;

struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    diags: Vec<Diagnostic>,
    /// Vrai après une erreur structurelle : l'appelant doit se resynchroniser.
    panic: bool,
}

impl Parser<'_> {
    // ---------------------------------------------------------------- outils

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_at(&self, n: usize) -> &TokenKind {
        let i = (self.pos + n).min(self.tokens.len() - 1);
        &self.tokens[i].kind
    }

    fn span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn prev_span(&self) -> Span {
        self.tokens[self.pos.saturating_sub(1)].span
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn bump(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if tok.kind != K::Eof {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn text(&self, span: Span) -> &str {
        &self.src[span.start..span.end]
    }

    fn error(&mut self, diag: Diagnostic) {
        self.panic = true;
        self.diags.push(diag);
    }

    fn unexpected(&mut self, code: &'static Code, expected: &str) {
        let found = self.peek().describe();
        let span = self.span();
        self.error(
            Diagnostic::new(code, span, format!("{expected} attendu, trouvé {found}"))
                .expected(expected.to_string())
                .received(found),
        );
    }

    fn expect(&mut self, kind: TokenKind) -> bool {
        if self.eat(&kind) {
            return true;
        }
        let what = format!("`{}`", kind.symbol());
        self.unexpected(&codes::UNEXPECTED_TOKEN, &what);
        false
    }

    /// `;` manquant : diagnostic précis, sans consommer, sans resynchronisation.
    fn expect_semi(&mut self) {
        if self.eat(&K::Semi) {
            return;
        }
        let end = self.prev_span().end;
        self.diags.push(
            Diagnostic::new(
                &codes::MISSING_SEMICOLON,
                Span::new(end, end),
                format!("`;` manquant, trouvé {}", self.peek().describe()),
            )
            .suggestion("ajouter `;` à la fin de l'instruction"),
        );
    }

    /// Saute jusqu'à une frontière d'instruction (`;` consommé, `}` ou fin non consommés).
    fn recover(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek() {
                K::Eof => break,
                K::LBrace => depth += 1,
                K::RBrace if depth == 0 => break,
                K::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        break;
                    }
                }
                K::Semi if depth == 0 => {
                    self.bump();
                    break;
                }
                K::Func | K::Scope | K::Import | K::Event | K::On | K::Struct | K::Enum
                    if depth == 0 =>
                {
                    break
                }
                _ => {}
            }
            self.bump();
        }
        self.panic = false;
    }

    fn ident(&mut self) -> Option<Ident> {
        if matches!(self.peek(), K::Ident | K::LocalIdent) {
            let span = self.bump().span;
            return Some(Ident {
                name: self.text(span).to_string(),
                span,
            });
        }
        self.unexpected(&codes::EXPECTED_IDENTIFIER, "un identifiant");
        None
    }

    fn global_ident(&mut self) -> Option<Ident> {
        if self.at(&K::Ident) {
            return self.ident();
        }
        self.unexpected(
            &codes::EXPECTED_IDENTIFIER,
            "un identifiant global (sans `_`)",
        );
        None
    }

    fn path(&mut self) -> Option<Path> {
        let first = self.global_ident()?;
        let mut span = first.span;
        let mut segments = vec![first];
        while self.at(&K::Dot) && matches!(self.peek_at(1), K::Ident) {
            self.bump();
            let seg = self.global_ident()?;
            span = span.to(seg.span);
            segments.push(seg);
        }
        Some(Path { segments, span })
    }

    // ---------------------------------------------------------------- items

    fn items_until_eof(&mut self) -> Vec<Item> {
        let mut items = Vec::new();
        while !self.at(&K::Eof) {
            if self.at(&K::RBrace) {
                let span = self.bump().span;
                self.diags.push(
                    Diagnostic::new(&codes::UNEXPECTED_TOKEN, span, "`}` sans `{` correspondant")
                        .received("`}`"),
                );
                continue;
            }
            self.item_into(&mut items);
        }
        items
    }

    fn item_into(&mut self, items: &mut Vec<Item>) {
        let start = self.pos;
        if let Some(item) = self.item() {
            items.push(item);
        }
        if self.panic {
            self.recover();
        }
        if self.pos == start {
            self.bump();
        }
    }

    fn item(&mut self) -> Option<Item> {
        match self.peek() {
            K::Func => self.func().map(Item::Func),
            K::Scope => self.scope().map(Item::Scope),
            K::Import => self.import().map(Item::Import),
            K::Event => self.event().map(Item::Event),
            K::On => self.on_handler().map(Item::On),
            K::Struct => self.struct_decl().map(Item::Struct),
            K::Enum => self.enum_decl().map(Item::Enum),
            K::Semi => {
                self.stray_semicolon();
                None
            }
            K::Ident if self.peek_at(1) == &K::Colon => self.var_decl().map(Item::Global),
            K::LocalIdent if self.peek_at(1) == &K::Colon => {
                let span = self.span();
                let name = self.text(span).to_string();
                self.diags.push(
                    Diagnostic::new(
                        &codes::LOCAL_DECLARATION_AT_MODULE_LEVEL,
                        span,
                        format!("`{name}` est déclarée au niveau du module avec le préfixe `_`"),
                    )
                    .suggestion(format!(
                        "une variable globale s'écrit sans `_` : `{}`",
                        name.trim_start_matches('_')
                    )),
                );
                self.var_decl().map(Item::Global)
            }
            _ if self.starts_statement() => {
                let span = self.span();
                self.diags.push(
                    Diagnostic::new(
                        &codes::STATEMENT_OUTSIDE_FUNCTION,
                        span,
                        "instruction exécutable au niveau du module",
                    )
                    .suggestion(
                        "placer cette instruction dans une fonction `func` ou un gestionnaire `on`",
                    ),
                );
                let _ = self.stmt();
                None
            }
            _ => {
                self.unexpected(
                    &codes::UNEXPECTED_TOKEN,
                    "une déclaration (func, scope, import, event, on ou `nom: Type`)",
                );
                None
            }
        }
    }

    fn stray_semicolon(&mut self) {
        let span = self.bump().span;
        self.diags.push(
            Diagnostic::new(&codes::UNNECESSARY_SEMICOLON, span, "point-virgule inutile")
                .suggestion("les blocs `}` de fonctions, scopes et structures de contrôle ne prennent pas de `;`"),
        );
    }

    fn func(&mut self) -> Option<FuncDecl> {
        let start = self.bump().span;
        let name = self.ident()?;
        if name.is_local() {
            self.diags.push(
                Diagnostic::new(
                    &codes::EXPECTED_IDENTIFIER,
                    name.span,
                    "un nom de fonction ne prend pas le préfixe `_`",
                )
                .suggestion(format!("`func {}(...)`", name.name.trim_start_matches('_'))),
            );
        }
        let params = self.params()?;
        let ret = if self.eat(&K::Colon) {
            Some(self.type_decl()?)
        } else {
            let at = self.prev_span().end;
            self.diags.push(
                Diagnostic::new(
                    &codes::MISSING_RETURN_TYPE,
                    Span::new(at, at),
                    format!("la fonction `{}` n'a pas de type de retour", name.name),
                )
                .suggestion("ajouter `: Void` si la fonction ne retourne rien"),
            );
            None
        };
        let body = self.block()?;
        Some(FuncDecl {
            span: start.to(body.span),
            name,
            params,
            ret,
            body,
        })
    }

    fn params(&mut self) -> Option<Vec<Param>> {
        self.expect(K::LParen).then_some(())?;
        let mut params = Vec::new();
        if !self.at(&K::RParen) {
            loop {
                params.push(self.param()?);
                if !self.eat(&K::Comma) {
                    break;
                }
            }
        }
        self.expect(K::RParen).then_some(params)
    }

    fn param(&mut self) -> Option<Param> {
        let name = self.ident()?;
        if !name.is_local() {
            self.diags.push(
                Diagnostic::new(
                    &codes::PARAMETER_MUST_BE_LOCAL,
                    name.span,
                    format!("le paramètre `{}` doit commencer par `_`", name.name),
                )
                .suggestion(format!("`_{}`", name.name)),
            );
        }
        self.expect(K::Colon).then_some(())?;
        let ty = self.type_decl()?;
        Some(Param {
            span: name.span.to(ty.span),
            name,
            ty,
        })
    }

    /// `struct Nom { champ: Type; ... }`
    fn struct_decl(&mut self) -> Option<StructDecl> {
        let start = self.bump().span;
        let name = self.global_ident()?;
        self.expect(K::LBrace).then_some(())?;
        let mut fields = Vec::new();
        while !matches!(self.peek(), K::RBrace | K::Eof) {
            if self.at(&K::LocalIdent) {
                let span = self.span();
                let text = self.text(span).to_string();
                self.diags.push(
                    Diagnostic::new(
                        &codes::INVALID_FIELD_NAME,
                        span,
                        format!("le champ `{text}` ne doit pas commencer par `_`"),
                    )
                    .suggestion(format!("`{}`", text.trim_start_matches('_'))),
                );
            } else if !self.at(&K::Ident) {
                self.unexpected(&codes::EXPECTED_IDENTIFIER, "un champ `nom: Type;`");
                return None;
            }
            let fname = self.ident()?;
            self.expect(K::Colon).then_some(())?;
            let ty = self.type_decl()?;
            let end = self.prev_span();
            self.expect_semi();
            fields.push(Field {
                span: fname.span.to(end),
                name: fname,
                ty,
            });
        }
        let end = self.span();
        self.expect(K::RBrace).then_some(())?;
        Some(StructDecl {
            span: start.to(end),
            name,
            fields,
        })
    }

    /// `enum Nom { A, B, C }`
    fn enum_decl(&mut self) -> Option<EnumDecl> {
        let start = self.bump().span;
        let name = self.global_ident()?;
        self.expect(K::LBrace).then_some(())?;
        let mut variants = Vec::new();
        while !self.at(&K::RBrace) {
            variants.push(self.global_ident()?);
            if !self.eat(&K::Comma) {
                break;
            }
        }
        let end = self.span();
        self.expect(K::RBrace).then_some(())?;
        Some(EnumDecl {
            span: start.to(end),
            name,
            variants,
        })
    }

    fn scope(&mut self) -> Option<ScopeDecl> {
        let start = self.bump().span;
        let name = self.global_ident()?;
        self.expect(K::LBrace).then_some(())?;
        let mut items = Vec::new();
        while !matches!(self.peek(), K::RBrace | K::Eof) {
            self.item_into(&mut items);
        }
        let end = self.span();
        if !self.eat(&K::RBrace) {
            self.diags.push(
                Diagnostic::new(
                    &codes::UNCLOSED_DELIMITER,
                    start.to(name.span),
                    format!("le scope `{}` n'est jamais fermé", name.name),
                )
                .suggestion("ajouter `}`"),
            );
        }
        Some(ScopeDecl {
            span: start.to(end),
            name,
            items,
        })
    }

    fn import(&mut self) -> Option<ImportDecl> {
        let start = self.bump().span;
        let path = self.path()?;
        self.expect_semi();
        Some(ImportDecl {
            span: start.to(path.span),
            path,
        })
    }

    fn event(&mut self) -> Option<EventDecl> {
        let start = self.bump().span;
        let name = self.path()?;
        let params = self.params()?;
        let end = self.prev_span();
        self.expect_semi();
        Some(EventDecl {
            span: start.to(end),
            name,
            params,
        })
    }

    fn on_handler(&mut self) -> Option<OnHandler> {
        let start = self.bump().span;
        let event = self.path()?;
        self.expect(K::LParen).then_some(())?;
        let mut bindings = Vec::new();
        if !self.at(&K::RParen) {
            loop {
                let b = self.ident()?;
                if !b.is_local() {
                    self.diags.push(
                        Diagnostic::new(
                            &codes::PARAMETER_MUST_BE_LOCAL,
                            b.span,
                            format!("la donnée d'événement `{}` doit commencer par `_`", b.name),
                        )
                        .suggestion(format!("`_{}`", b.name)),
                    );
                }
                bindings.push(b);
                if !self.eat(&K::Comma) {
                    break;
                }
            }
        }
        self.expect(K::RParen).then_some(())?;
        let body = self.block()?;
        Some(OnHandler {
            span: start.to(body.span),
            event,
            bindings,
            body,
        })
    }

    /// `nom: Type[, Type] [= expr];`
    fn var_decl(&mut self) -> Option<VarDecl> {
        let name = self.ident()?;
        self.expect(K::Colon).then_some(())?;
        let ty = self.type_decl()?;
        let init = if self.eat(&K::Assign) {
            Some(self.expr())
        } else {
            None
        };
        let end = self.prev_span();
        self.expect_semi();
        Some(VarDecl {
            span: name.span.to(end),
            name,
            ty,
            init,
        })
    }

    // ---------------------------------------------------------------- types

    fn type_decl(&mut self) -> Option<TypeDecl> {
        let first = self.type_ref()?;
        let mut span = first.span;
        let mut types = vec![first];
        // `,` suivi d'un nom de type (et non `nom:` d'un paramètre suivant).
        while self.at(&K::Comma) && self.peek_at(1) == &K::Ident && self.peek_at(2) != &K::Colon {
            self.bump();
            let ty = self.type_ref()?;
            span = span.to(ty.span);
            types.push(ty);
        }
        if types.len() > 2 {
            let extra = types[2].span.to(span);
            let received: Vec<_> = types
                .iter()
                .map(|t| self.text(t.span).to_string())
                .collect();
            self.diags.push(
                Diagnostic::new(
                    &codes::TOO_MANY_TYPES,
                    extra,
                    format!("DualType limité à deux types, {} déclarés", types.len()),
                )
                .expected("1 ou 2 types")
                .received(received.join(", "))
                .suggestion("garder au plus deux types ; pour plus de cas, utiliser une structure ou une énumération"),
            );
        }
        Some(TypeDecl { types, span })
    }

    fn type_ref(&mut self) -> Option<TypeRef> {
        if !self.at(&K::Ident) {
            self.unexpected(&codes::EXPECTED_TYPE, "un nom de type");
            return None;
        }
        let path = self.path()?;
        let mut span = path.span;
        let mut args = Vec::new();
        if self.eat(&K::Lt) {
            loop {
                args.push(self.type_ref()?);
                if !self.eat(&K::Comma) {
                    break;
                }
            }
            span = span.to(self.span());
            self.expect(K::Gt).then_some(())?;
        }
        Some(TypeRef { path, args, span })
    }

    // ---------------------------------------------------------------- blocs

    fn block(&mut self) -> Option<Block> {
        let start = self.span();
        self.expect(K::LBrace).then_some(())?;
        let mut stmts = Vec::new();
        while !matches!(self.peek(), K::RBrace | K::Eof) {
            let before = self.pos;
            if let Some(s) = self.stmt() {
                stmts.push(s);
            }
            if self.panic {
                self.recover();
            }
            if self.pos == before {
                self.bump();
            }
        }
        let end = self.span();
        if !self.eat(&K::RBrace) {
            self.diags.push(
                Diagnostic::new(&codes::UNCLOSED_DELIMITER, start, "bloc `{` jamais fermé")
                    .suggestion("ajouter `}`"),
            );
        }
        Some(Block {
            stmts,
            span: start.to(end),
        })
    }

    fn starts_statement(&self) -> bool {
        matches!(
            self.peek(),
            K::If
                | K::While
                | K::For
                | K::Foreach
                | K::Return
                | K::Break
                | K::Continue
                | K::Emit
                | K::Ident
                | K::LocalIdent
                | K::Int
                | K::Float
                | K::Str(_)
                | K::True
                | K::False
                | K::LParen
                | K::LBracket
                | K::Bang
                | K::Minus
        )
    }

    fn stmt(&mut self) -> Option<Stmt> {
        match self.peek() {
            K::LocalIdent if self.peek_at(1) == &K::Colon => self.var_decl().map(Stmt::Local),
            K::Ident if self.peek_at(1) == &K::Colon => {
                let span = self.span();
                let name = self.text(span).to_string();
                self.diags.push(
                    Diagnostic::new(
                        &codes::GLOBAL_DECLARATION_IN_BLOCK,
                        span,
                        format!("`{name}` est déclarée dans un bloc sans le préfixe `_`"),
                    )
                    .suggestion(format!("une variable locale s'écrit `_{name}`")),
                );
                self.var_decl().map(Stmt::Local)
            }
            K::If => self.if_stmt().map(Stmt::If),
            K::While => self.while_stmt(),
            K::For => self.for_stmt(),
            K::Foreach => self.foreach_stmt(),
            K::Return => {
                let start = self.bump().span;
                let value = if self.at(&K::Semi) {
                    None
                } else {
                    Some(self.expr())
                };
                let end = self.prev_span();
                self.expect_semi();
                Some(Stmt::Return {
                    value,
                    span: start.to(end),
                })
            }
            K::Break => {
                let span = self.bump().span;
                self.expect_semi();
                Some(Stmt::Break { span })
            }
            K::Continue => {
                let span = self.bump().span;
                self.expect_semi();
                Some(Stmt::Continue { span })
            }
            K::Emit => {
                let start = self.bump().span;
                let event = self.path()?;
                let args = self.call_args()?;
                let end = self.prev_span();
                self.expect_semi();
                Some(Stmt::Emit {
                    event,
                    args,
                    span: start.to(end),
                })
            }
            K::Semi => {
                self.stray_semicolon();
                None
            }
            K::Func | K::Scope | K::Import | K::Event | K::On | K::Struct | K::Enum => {
                let span = self.span();
                let kw = self.peek().symbol();
                self.diags.push(
                    Diagnostic::new(
                        &codes::DECLARATION_NOT_ALLOWED_HERE,
                        span,
                        format!("`{kw}` ne peut pas être déclaré dans un bloc"),
                    )
                    .suggestion("déplacer cette déclaration au niveau du module ou d'un scope"),
                );
                // Analysée pour la récupération puis ignorée.
                let _ = self.item();
                None
            }
            _ => {
                let stmt = self.simple_stmt()?;
                self.expect_semi();
                Some(stmt)
            }
        }
    }

    /// Affectation ou expression, sans le `;` final (réutilisé par `for`).
    fn simple_stmt(&mut self) -> Option<Stmt> {
        if !self.starts_statement() {
            self.unexpected(&codes::EXPECTED_EXPRESSION, "une instruction");
            return None;
        }
        let expr = self.expr();
        if self.eat(&K::Assign) {
            if !expr.is_place() {
                self.diags.push(
                    Diagnostic::new(
                        &codes::INVALID_ASSIGNMENT_TARGET,
                        expr.span,
                        "impossible d'affecter une valeur à cette expression",
                    )
                    .expected("une variable, un membre (`a.b`) ou un index (`a[i]`)"),
                );
            }
            let value = self.expr();
            return Some(Stmt::Assign {
                span: expr.span.to(value.span),
                target: expr,
                value,
            });
        }
        Some(Stmt::Expr {
            span: expr.span,
            expr,
        })
    }

    fn condition(&mut self) -> Option<Expr> {
        self.expect(K::LParen).then_some(())?;
        let cond = self.expr();
        self.expect(K::RParen).then_some(cond)
    }

    fn keyword_before_block(&mut self, kw: TokenKind, code: &'static Code, hint: &str) {
        if self.eat(&kw) {
            return;
        }
        let at = self.prev_span().end;
        // Non bloquant : si le bloc suit directement, on continue sans resynchroniser.
        self.diags.push(
            Diagnostic::new(
                code,
                Span::new(at, at),
                format!("`{}` manquant", kw.symbol()),
            )
            .suggestion(hint.to_string()),
        );
    }

    fn if_stmt(&mut self) -> Option<IfStmt> {
        let start = self.bump().span;
        let cond = self.condition()?;
        self.keyword_before_block(
            K::Then,
            &codes::MISSING_THEN,
            "écrire `if (condition) then { ... }`",
        );
        let then_block = self.block()?;
        let mut span = start.to(then_block.span);
        let else_branch = if self.eat(&K::Else) {
            if self.at(&K::If) {
                let nested = self.if_stmt()?;
                span = span.to(nested.span);
                Some(Else::If(Box::new(nested)))
            } else {
                let b = self.block()?;
                span = span.to(b.span);
                Some(Else::Block(b))
            }
        } else {
            None
        };
        Some(IfStmt {
            cond,
            then_block,
            else_branch,
            span,
        })
    }

    fn while_stmt(&mut self) -> Option<Stmt> {
        let start = self.bump().span;
        let cond = self.condition()?;
        self.keyword_before_block(
            K::Do,
            &codes::MISSING_DO,
            "écrire `while (condition) do { ... }`",
        );
        let body = self.block()?;
        Some(Stmt::While {
            span: start.to(body.span),
            cond,
            body,
        })
    }

    fn for_stmt(&mut self) -> Option<Stmt> {
        let start = self.bump().span;
        self.expect(K::LParen).then_some(())?;
        if !(self.at(&K::LocalIdent) && self.peek_at(1) == &K::Colon) {
            self.unexpected(
                &codes::UNEXPECTED_TOKEN,
                "une déclaration locale `_i: Int = 0;`",
            );
            return None;
        }
        let init = self.var_decl()?;
        let cond = self.expr();
        self.expect(K::Semi).then_some(())?;
        let step = self.simple_stmt()?;
        self.expect(K::RParen).then_some(())?;
        self.keyword_before_block(K::Do, &codes::MISSING_DO, "écrire `for (...) do { ... }`");
        let body = self.block()?;
        Some(Stmt::For {
            span: start.to(body.span),
            init,
            cond,
            step: Box::new(step),
            body,
        })
    }

    fn foreach_stmt(&mut self) -> Option<Stmt> {
        let start = self.bump().span;
        self.expect(K::LParen).then_some(())?;
        let binding = self.ident()?;
        if !binding.is_local() {
            self.diags.push(
                Diagnostic::new(
                    &codes::GLOBAL_DECLARATION_IN_BLOCK,
                    binding.span,
                    format!(
                        "la variable de boucle `{}` doit commencer par `_`",
                        binding.name
                    ),
                )
                .suggestion(format!("`_{}`", binding.name)),
            );
        }
        let ty = if self.eat(&K::Colon) {
            Some(self.type_decl()?)
        } else {
            None
        };
        self.expect(K::In).then_some(())?;
        let iter = self.expr();
        self.expect(K::RParen).then_some(())?;
        self.keyword_before_block(
            K::Do,
            &codes::MISSING_DO,
            "écrire `foreach (_x in liste) do { ... }`",
        );
        let body = self.block()?;
        Some(Stmt::Foreach {
            span: start.to(body.span),
            binding,
            ty,
            iter,
            body,
        })
    }

    // ---------------------------------------------------------------- expressions
    //
    // Priorités, de la plus faible à la plus forte :
    //   ||  →  &&  →  == !=  →  < > <= >= is  →  + -  →  * / %  →  ! - (unaires)  →  appel, membre, index
    // Les niveaux d'égalité et de comparaison sont non associatifs (MCF0113).

    fn expr(&mut self) -> Expr {
        self.or_expr()
    }

    fn binary(lhs: Expr, op: BinaryOp, rhs: Expr) -> Expr {
        Expr {
            span: lhs.span.to(rhs.span),
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        }
    }

    fn or_expr(&mut self) -> Expr {
        let mut lhs = self.and_expr();
        while self.eat(&K::OrOr) {
            let rhs = self.and_expr();
            lhs = Self::binary(lhs, BinaryOp::Or, rhs);
        }
        lhs
    }

    fn and_expr(&mut self) -> Expr {
        let mut lhs = self.equality();
        while self.eat(&K::AndAnd) {
            let rhs = self.equality();
            lhs = Self::binary(lhs, BinaryOp::And, rhs);
        }
        lhs
    }

    fn equality_op(&self) -> Option<BinaryOp> {
        match self.peek() {
            K::EqEq => Some(BinaryOp::Eq),
            K::NotEq => Some(BinaryOp::NotEq),
            _ => None,
        }
    }

    fn relational_op(&self) -> Option<BinaryOp> {
        match self.peek() {
            K::Lt => Some(BinaryOp::Lt),
            K::Gt => Some(BinaryOp::Gt),
            K::Le => Some(BinaryOp::Le),
            K::Ge => Some(BinaryOp::Ge),
            _ => None,
        }
    }

    fn chained(&mut self) {
        let span = self.span();
        self.diags.push(
            Diagnostic::new(&codes::CHAINED_COMPARISON, span, "comparaisons enchaînées")
                .suggestion("séparer avec `&&`, ex. `a < b && b < c`, ou ajouter des parenthèses"),
        );
    }

    fn equality(&mut self) -> Expr {
        let mut lhs = self.relational();
        if let Some(op) = self.equality_op() {
            self.bump();
            let rhs = self.relational();
            lhs = Self::binary(lhs, op, rhs);
            while let Some(op) = self.equality_op() {
                self.chained();
                self.bump();
                let rhs = self.relational();
                lhs = Self::binary(lhs, op, rhs);
            }
        }
        lhs
    }

    fn relational(&mut self) -> Expr {
        let mut lhs = self.additive();
        let mut seen = false;
        loop {
            if let Some(op) = self.relational_op() {
                if seen {
                    self.chained();
                }
                self.bump();
                let rhs = self.additive();
                lhs = Self::binary(lhs, op, rhs);
            } else if self.at(&K::Is) {
                if seen {
                    self.chained();
                }
                self.bump();
                let Some(ty) = self.type_ref() else {
                    return Expr {
                        span: lhs.span,
                        kind: ExprKind::Error,
                    };
                };
                lhs = Expr {
                    span: lhs.span.to(ty.span),
                    kind: ExprKind::Is {
                        expr: Box::new(lhs),
                        ty,
                    },
                };
            } else {
                return lhs;
            }
            seen = true;
        }
    }

    fn additive(&mut self) -> Expr {
        let mut lhs = self.multiplicative();
        loop {
            let op = match self.peek() {
                K::Plus => BinaryOp::Add,
                K::Minus => BinaryOp::Sub,
                _ => return lhs,
            };
            self.bump();
            let rhs = self.multiplicative();
            lhs = Self::binary(lhs, op, rhs);
        }
    }

    fn multiplicative(&mut self) -> Expr {
        let mut lhs = self.unary();
        loop {
            let op = match self.peek() {
                K::Star => BinaryOp::Mul,
                K::Slash => BinaryOp::Div,
                K::Percent => BinaryOp::Rem,
                _ => return lhs,
            };
            self.bump();
            let rhs = self.unary();
            lhs = Self::binary(lhs, op, rhs);
        }
    }

    fn unary(&mut self) -> Expr {
        let op = match self.peek() {
            K::Bang => UnaryOp::Not,
            K::Minus => UnaryOp::Neg,
            _ => return self.postfix(),
        };
        let start = self.bump().span;
        let expr = self.unary();
        Expr {
            span: start.to(expr.span),
            kind: ExprKind::Unary {
                op,
                expr: Box::new(expr),
            },
        }
    }

    fn call_args(&mut self) -> Option<Vec<Expr>> {
        self.expect(K::LParen).then_some(())?;
        let mut args = Vec::new();
        if !self.at(&K::RParen) {
            loop {
                args.push(self.expr());
                if !self.eat(&K::Comma) {
                    break;
                }
            }
        }
        self.expect(K::RParen).then_some(args)
    }

    fn postfix(&mut self) -> Expr {
        let mut expr = self.primary();
        loop {
            match self.peek() {
                K::LParen => {
                    let Some(args) = self.call_args() else {
                        return expr;
                    };
                    expr = Expr {
                        span: expr.span.to(self.prev_span()),
                        kind: ExprKind::Call {
                            callee: Box::new(expr),
                            args,
                        },
                    };
                }
                K::Dot => {
                    self.bump();
                    let Some(field) = self.global_ident() else {
                        return expr;
                    };
                    expr = Expr {
                        span: expr.span.to(field.span),
                        kind: ExprKind::Member {
                            object: Box::new(expr),
                            field,
                        },
                    };
                }
                K::LBracket => {
                    self.bump();
                    let index = self.expr();
                    if !self.expect(K::RBracket) {
                        return expr;
                    }
                    expr = Expr {
                        span: expr.span.to(self.prev_span()),
                        kind: ExprKind::Index {
                            object: Box::new(expr),
                            index: Box::new(index),
                        },
                    };
                }
                // `Nom { champ: ... }` ou `Nom { }` : littéral de structure. Le test sur les tokens
                // suivants évite toute confusion avec un bloc.
                K::LBrace
                    if expr_path(&expr).is_some()
                        && (self.peek_at(1) == &K::RBrace
                            || (self.peek_at(1) == &K::Ident && self.peek_at(2) == &K::Colon)) =>
                {
                    let path = expr_path(&expr).expect("chemin vérifié");
                    let Some(fields) = self.struct_literal_fields() else {
                        return expr;
                    };
                    expr = Expr {
                        span: expr.span.to(self.prev_span()),
                        kind: ExprKind::StructLit { path, fields },
                    };
                }
                _ => return expr,
            }
        }
    }

    fn struct_literal_fields(&mut self) -> Option<Vec<FieldInit>> {
        self.bump(); // {
        let mut fields = Vec::new();
        while !self.at(&K::RBrace) {
            let name = self.global_ident()?;
            self.expect(K::Colon).then_some(())?;
            let value = self.expr();
            fields.push(FieldInit {
                span: name.span.to(value.span),
                name,
                value,
            });
            if !self.eat(&K::Comma) {
                break;
            }
        }
        self.expect(K::RBrace).then_some(fields)
    }

    fn primary(&mut self) -> Expr {
        let span = self.span();
        let kind = match self.peek().clone() {
            K::Int => ExprKind::Int {
                text: self.text(span).to_string(),
            },
            K::Float => ExprKind::Float {
                text: self.text(span).to_string(),
            },
            K::Str(value) => ExprKind::Str { value },
            K::True => ExprKind::Bool { value: true },
            K::False => ExprKind::Bool { value: false },
            K::Ident | K::LocalIdent => ExprKind::Name {
                ident: Ident {
                    name: self.text(span).to_string(),
                    span,
                },
            },
            K::LParen => {
                self.bump();
                let inner = self.expr();
                let end = self.span();
                self.expect(K::RParen);
                return Expr {
                    span: span.to(end),
                    kind: ExprKind::Paren {
                        expr: Box::new(inner),
                    },
                };
            }
            K::LBracket => {
                self.bump();
                let mut items = Vec::new();
                if !self.at(&K::RBracket) {
                    loop {
                        items.push(self.expr());
                        if !self.eat(&K::Comma) {
                            break;
                        }
                    }
                }
                let end = self.span();
                self.expect(K::RBracket);
                return Expr {
                    span: span.to(end),
                    kind: ExprKind::Array { items },
                };
            }
            _ => {
                self.unexpected(&codes::EXPECTED_EXPRESSION, "une expression");
                return Expr {
                    span,
                    kind: ExprKind::Error,
                };
            }
        };
        self.bump();
        Expr { kind, span }
    }
}

/// Chemin `A`, `A.B.C` formé uniquement d'identifiants globaux.
fn expr_path(expr: &Expr) -> Option<Path> {
    match &expr.kind {
        ExprKind::Name { ident } if !ident.is_local() => Some(Path {
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
