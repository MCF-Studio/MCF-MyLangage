//! Parser des fichiers `.uimcf` (phase 10). Grammaire : `docs/grammaire/interfaces.md`.
//!
//! Réutilise le parser MCF pour les imports, les types et les expressions. `ui`, `prop`,
//! `state`, `view`, `style` et `bind` sont des mots contextuels : des identifiants ordinaires
//! reconnus ici par leur position. La logique (fonctions, variables, instructions) est
//! analysée pour garder une bonne reprise, puis refusée (MCF0715).

use super::{Parser, K};
use crate::ast::*;
use crate::lexer::lex_ui;
use mcf_diagnostics::{codes, Diagnostic, Span};

pub struct UiParseResult {
    pub module: UiModule,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse_ui(src: &str) -> UiParseResult {
    let (tokens, mut diagnostics) = lex_ui(src);
    let mut p = Parser {
        src,
        tokens,
        pos: 0,
        diags: Vec::new(),
        panic: false,
    };
    let module = p.ui_module();
    diagnostics.append(&mut p.diags);
    diagnostics.sort_by_key(|d| d.span.start);
    UiParseResult {
        module,
        diagnostics,
    }
}

impl Parser<'_> {
    /// Identifiant dont le texte est `word` (mot contextuel).
    fn at_word(&self, word: &str) -> bool {
        self.at(&K::Ident) && self.text(self.span()) == word
    }

    fn ui_module(&mut self) -> UiModule {
        let mut m = UiModule {
            imports: Vec::new(),
            styles: Vec::new(),
            components: Vec::new(),
        };
        while !self.at(&K::Eof) {
            let start = self.pos;
            if self.at(&K::RBrace) {
                let span = self.bump().span;
                self.diags.push(
                    Diagnostic::new(&codes::UNEXPECTED_TOKEN, span, "`}` sans `{` correspondant")
                        .received("`}`"),
                );
                continue;
            }
            if self.at(&K::Import) {
                if let Some(i) = self.import() {
                    m.imports.push(i);
                }
            } else if self.at_word("ui") && self.peek_at(1) == &K::Ident {
                if let Some(c) = self.ui_decl() {
                    m.components.push(c);
                }
            } else if self.at_word("style") && self.peek_at(1) == &K::Ident {
                if let Some(s) = self.style_decl() {
                    m.styles.push(s);
                }
            } else {
                self.logic_item("à la racine d'un fichier `.uimcf`");
            }
            if self.panic {
                self.recover();
            }
            if self.pos == start {
                self.bump();
            }
        }
        m
    }

    /// Déclaration MCF (fonction, variable…) ou instruction trouvée dans un `.uimcf` : analysée
    /// pour la sauter proprement, puis refusée.
    fn logic_item(&mut self, place: &str) {
        let start = self.span();
        let is_item = matches!(
            self.peek(),
            K::Func | K::Scope | K::Struct | K::Enum | K::Event | K::On | K::Config
        ) || (matches!(self.peek(), K::Ident | K::LocalIdent)
            && self.peek_at(1) == &K::Colon);
        if !is_item && !self.starts_statement() {
            self.unexpected(
                &codes::UNEXPECTED_TOKEN,
                &format!("`import`, `style` ou `ui` {place}"),
            );
            return;
        }
        let mut ignored = Vec::new();
        let before = self.diags.len();
        if is_item {
            self.item_into(&mut ignored);
        } else {
            self.stmt();
            if self.panic {
                self.recover();
            }
        }
        // Les erreurs de syntaxe de ce qui est refusé n'apprennent rien de plus.
        self.diags.truncate(before);
        let span = start.to(self.prev_span());
        self.diags.push(
            Diagnostic::new(
                &codes::UI_LOGIC_NOT_ALLOWED,
                span,
                format!("pas de logique {place} : un `.uimcf` décrit seulement l'interface"),
            )
            .suggestion(
                "écrire ce code dans un fichier `.mcf`, puis l'importer (`import nom;`) ou l'appeler depuis un événement (`onClick: nom`)",
            ),
        );
    }

    fn ui_decl(&mut self) -> Option<UiDecl> {
        let start = self.bump().span;
        let name = self.global_ident()?;
        self.expect(K::LBrace).then_some(())?;
        let mut members = Vec::new();
        let mut view = None;
        let mut styles = Vec::new();
        while !matches!(self.peek(), K::RBrace | K::Eof) {
            let before = self.pos;
            if (self.at_word("prop") || self.at_word("state"))
                && matches!(self.peek_at(1), K::Ident | K::LocalIdent)
            {
                if let Some(m) = self.ui_member() {
                    members.push(m);
                }
            } else if self.at_word("view") && self.peek_at(1) == &K::LBrace {
                let span = self.bump().span;
                let nodes = self.view_block();
                if view.is_some() {
                    self.diags.push(
                        Diagnostic::new(
                            &codes::UI_NAME_CONFLICT,
                            span,
                            "un composant n'a qu'une seule `view`",
                        )
                        .suggestion("regrouper les éléments dans la première `view`"),
                    );
                } else {
                    view = nodes;
                }
            } else if self.at_word("style") && self.peek_at(1) == &K::Ident {
                if let Some(s) = self.style_decl() {
                    styles.push(s);
                }
            } else {
                self.logic_item("dans un composant `ui`");
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
                Diagnostic::new(&codes::UNCLOSED_DELIMITER, start, "`ui` jamais fermé")
                    .suggestion("ajouter `}`"),
            );
        }
        Some(UiDecl {
            name,
            members,
            view,
            styles,
            span: start.to(end),
        })
    }

    /// `prop nom: Type [= valeur];` ou `state nom: Type = valeur;`
    fn ui_member(&mut self) -> Option<UiMember> {
        let start = self.span();
        let kind = if self.text(start) == "prop" {
            UiMemberKind::Prop
        } else {
            UiMemberKind::State
        };
        self.bump();
        let name = self.ident()?;
        if name.is_local() {
            self.diags.push(
                Diagnostic::new(
                    &codes::INVALID_FIELD_NAME,
                    name.span,
                    format!("`{}` : un membre de composant s'écrit sans `_`", name.name),
                )
                .suggestion(format!("`{}`", name.name.trim_start_matches('_'))),
            );
        }
        self.expect(K::Colon).then_some(())?;
        let ty = self.type_decl()?;
        let init = if self.eat(&K::Assign) {
            Some(self.expr())
        } else {
            None
        };
        if kind == UiMemberKind::State && init.is_none() {
            let at = self.prev_span().end;
            self.diags.push(
                Diagnostic::new(
                    &codes::UI_MISSING_PROPERTY,
                    Span::new(at, at),
                    format!("le `state` `{}` doit avoir une valeur initiale", name.name),
                )
                .suggestion("ajouter `= valeur`"),
            );
        }
        self.expect_semi();
        Some(UiMember {
            kind,
            span: start.to(self.prev_span()),
            name,
            ty,
            init,
        })
    }

    /// `style nom { propriété: valeur; … }`
    fn style_decl(&mut self) -> Option<StyleDecl> {
        let start = self.bump().span;
        let name = self.global_ident()?;
        self.expect(K::LBrace).then_some(())?;
        let mut props = Vec::new();
        while !matches!(self.peek(), K::RBrace | K::Eof) {
            let before = self.pos;
            if let Some(p) = self.ui_prop(false) {
                props.push(p);
                self.expect_semi();
            }
            if self.panic {
                self.recover();
            }
            if self.pos == before {
                self.bump();
            }
        }
        let end = self.span();
        self.expect(K::RBrace);
        Some(StyleDecl {
            name,
            props,
            span: start.to(end),
        })
    }

    /// `[bind] nom: valeur`
    fn ui_prop(&mut self, allow_bind: bool) -> Option<UiProp> {
        let start = self.span();
        let bind = allow_bind
            && self.at_word("bind")
            && self.peek_at(1) == &K::Ident
            && self.peek_at(2) == &K::Colon;
        if bind {
            self.bump();
        }
        if !(self.at(&K::Ident) && self.peek_at(1) == &K::Colon) {
            self.unexpected(&codes::UNEXPECTED_TOKEN, "une propriété `nom: valeur`");
            return None;
        }
        let name = self.ident()?;
        self.bump(); // `:`
        let value = self.ui_value();
        Some(UiProp {
            bind,
            span: start.to(value.span()),
            name,
            value,
        })
    }

    /// Valeur de propriété : couleur, pourcentage, `emit`, affectation ou expression.
    fn ui_value(&mut self) -> UiValue {
        let start = self.span();
        if let K::Color(hex) = self.peek().clone() {
            self.bump();
            return UiValue::Color { hex, span: start };
        }
        if matches!(self.peek(), K::Int | K::Float)
            && self.peek_at(1) == &K::Percent
            && matches!(self.peek_at(2), K::Semi | K::Comma | K::RParen | K::RBrace)
        {
            let value = self.text(start).parse().unwrap_or(0.0);
            self.bump();
            let end = self.bump().span;
            return UiValue::Percent {
                value,
                span: start.to(end),
            };
        }
        if self.at(&K::Emit) {
            self.bump();
            if let Some(event) = self.path() {
                if let Some(args) = self.call_args() {
                    return UiValue::Emit {
                        event,
                        args,
                        span: start.to(self.prev_span()),
                    };
                }
            }
            return UiValue::Expr(self.error_expr(start));
        }
        let expr = self.expr();
        if self.eat(&K::Assign) {
            let value = self.expr();
            return UiValue::Assign {
                span: expr.span.to(value.span),
                target: expr,
                value,
            };
        }
        UiValue::Expr(expr)
    }

    /// `{ nœuds }` ; `None` si `{` manque.
    fn view_block(&mut self) -> Option<Vec<ViewNode>> {
        let start = self.span();
        self.expect(K::LBrace).then_some(())?;
        let mut nodes = Vec::new();
        while !matches!(self.peek(), K::RBrace | K::Eof) {
            let before = self.pos;
            if let Some(n) = self.view_node() {
                nodes.push(n);
            }
            if self.panic {
                self.recover();
            }
            if self.pos == before {
                self.bump();
            }
        }
        if !self.eat(&K::RBrace) {
            self.diags.push(
                Diagnostic::new(&codes::UNCLOSED_DELIMITER, start, "bloc `{` jamais fermé")
                    .suggestion("ajouter `}`"),
            );
        }
        Some(nodes)
    }

    fn view_node(&mut self) -> Option<ViewNode> {
        match self.peek() {
            K::If => self.ui_if().map(ViewNode::If),
            K::Foreach => self.ui_foreach().map(ViewNode::Foreach),
            K::Ident if matches!(self.peek_at(1), K::LParen | K::LBrace | K::Semi) => {
                self.element().map(ViewNode::Element)
            }
            K::Semi => {
                self.stray_semicolon();
                None
            }
            _ => {
                self.logic_item("dans une `view`");
                None
            }
        }
    }

    fn element(&mut self) -> Option<Element> {
        let name = self.global_ident()?;
        let mut positional = Vec::new();
        let mut props = Vec::new();
        if self.eat(&K::LParen) {
            while !matches!(self.peek(), K::RParen | K::Eof) {
                let named = self.at(&K::Ident) && self.peek_at(1) == &K::Colon
                    || self.at_word("bind")
                        && self.peek_at(1) == &K::Ident
                        && self.peek_at(2) == &K::Colon;
                if named {
                    props.push(self.ui_prop(true)?);
                } else {
                    positional.push(self.expr());
                }
                if !self.eat(&K::Comma) {
                    break;
                }
            }
            self.expect(K::RParen).then_some(())?;
        }
        let children = if self.at(&K::LBrace) {
            Some(self.view_block()?)
        } else {
            self.expect_semi();
            None
        };
        Some(Element {
            span: name.span.to(self.prev_span()),
            name,
            positional,
            props,
            children,
        })
    }

    fn ui_if(&mut self) -> Option<UiIf> {
        let start = self.bump().span;
        let cond = self.condition()?;
        self.keyword_before_block(
            K::Then,
            &codes::MISSING_THEN,
            "écrire `if (condition) then { ... }`",
        );
        let then_nodes = self.view_block()?;
        let else_nodes = if self.eat(&K::Else) {
            if self.at(&K::If) {
                Some(vec![ViewNode::If(self.ui_if()?)])
            } else {
                Some(self.view_block()?)
            }
        } else {
            None
        };
        Some(UiIf {
            cond,
            then_nodes,
            else_nodes,
            span: start.to(self.prev_span()),
        })
    }

    fn ui_foreach(&mut self) -> Option<UiForeach> {
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
        let body = self.view_block()?;
        Some(UiForeach {
            binding,
            ty,
            iter,
            body,
            span: start.to(self.prev_span()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes_of(src: &str) -> Vec<&'static str> {
        parse_ui(src)
            .diagnostics
            .iter()
            .map(|d| d.code.id)
            .collect()
    }

    const EXAMPLE: &str = r#"
import clics;
style titre { fontSize: 24; }
ui Compteur {
    prop libelle: String = "Clics";
    state ouvert: Bool = false;
    view {
        Box(style: carte, width: 50%) {
            Text(libelle + " : " + clics, style: titre);
            Button("+1", onClick: ajouter);
            Button("Détails", onClick: ouvert = !ouvert);
            Input(bind value: recherche, onSubmit: emit Cherche(recherche));
            if (ouvert) then { Text("a"); } else if (clics > 2) then { Box; } else { Box {} }
            foreach (_x in noms) do { Text(_x); }
        }
    }
    style carte { background: #1e1e2e; padding: 16; }
}
"#;

    #[test]
    fn example_parses() {
        let r = parse_ui(EXAMPLE);
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let c = &r.module.components[0];
        assert_eq!(c.members.len(), 2);
        assert_eq!(c.styles.len(), 1);
        let ViewNode::Element(root) = &c.view.as_ref().unwrap()[0] else {
            panic!()
        };
        assert_eq!(root.children.as_ref().unwrap().len(), 6);
        assert!(matches!(root.props[1].value, UiValue::Percent { value, .. } if value == 50.0));
    }

    #[test]
    fn logic_is_refused() {
        assert_eq!(
            codes_of("func f(): Void { }\nui A { view { } }"),
            ["MCF0715"]
        );
        assert_eq!(codes_of("x: Int = 1;"), ["MCF0715"]);
        assert_eq!(
            codes_of("ui A { func f(): Void { } view { x = 1; } }"),
            ["MCF0715", "MCF0715"]
        );
    }

    #[test]
    fn colors_are_checked() {
        assert_eq!(codes_of("style s { color: #12345; }"), ["MCF0009"]);
        assert_eq!(
            codes_of("style s { color: #abc; background: #FFFFFF80; }"),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn state_needs_initial_value() {
        assert_eq!(codes_of("ui A { state n: Int; view { } }"), ["MCF0704"]);
    }
}
