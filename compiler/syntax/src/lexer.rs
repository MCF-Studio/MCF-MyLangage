//! Analyse lexicale. Règles : `docs/grammaire/lexique.md`.

use mcf_diagnostics::{codes, Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    /// Identifiant global : `[A-Za-z][A-Za-z0-9_]*`
    Ident,
    /// Identifiant local : `_[A-Za-z][A-Za-z0-9_]*`
    LocalIdent,
    Int,
    Float,
    /// Chaîne, valeur déjà décodée.
    Str(String),

    Func,
    Scope,
    Struct,
    Enum,
    Import,
    Event,
    On,
    Emit,
    If,
    Then,
    Else,
    While,
    For,
    Foreach,
    In,
    Do,
    Return,
    Break,
    Continue,
    Is,
    True,
    False,

    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semi,
    Colon,
    Comma,
    Dot,
    Assign,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    AndAnd,
    OrOr,
    Bang,

    Eof,
}

impl TokenKind {
    /// Description courte pour les messages d'erreur.
    pub fn describe(&self) -> String {
        use TokenKind::*;
        let s = match self {
            Ident => "un identifiant",
            LocalIdent => "un identifiant local",
            Int => "un entier",
            Float => "un décimal",
            Str(_) => "une chaîne",
            Eof => "la fin du fichier",
            other => return format!("`{}`", other.symbol()),
        };
        s.to_string()
    }

    pub fn symbol(&self) -> &'static str {
        use TokenKind::*;
        match self {
            Func => "func",
            Struct => "struct",
            Enum => "enum",
            Scope => "scope",
            Import => "import",
            Event => "event",
            On => "on",
            Emit => "emit",
            If => "if",
            Then => "then",
            Else => "else",
            While => "while",
            For => "for",
            Foreach => "foreach",
            In => "in",
            Do => "do",
            Return => "return",
            Break => "break",
            Continue => "continue",
            Is => "is",
            True => "true",
            False => "false",
            LBrace => "{",
            RBrace => "}",
            LParen => "(",
            RParen => ")",
            LBracket => "[",
            RBracket => "]",
            Semi => ";",
            Colon => ":",
            Comma => ",",
            Dot => ".",
            Assign => "=",
            EqEq => "==",
            NotEq => "!=",
            Lt => "<",
            Gt => ">",
            Le => "<=",
            Ge => ">=",
            Plus => "+",
            Minus => "-",
            Star => "*",
            Slash => "/",
            Percent => "%",
            AndAnd => "&&",
            OrOr => "||",
            Bang => "!",
            Ident | LocalIdent | Int | Float | Str(_) | Eof => "",
        }
    }
}

pub const KEYWORDS: &[(&str, TokenKind)] = &[
    ("func", TokenKind::Func),
    ("struct", TokenKind::Struct),
    ("enum", TokenKind::Enum),
    ("scope", TokenKind::Scope),
    ("import", TokenKind::Import),
    ("event", TokenKind::Event),
    ("on", TokenKind::On),
    ("emit", TokenKind::Emit),
    ("if", TokenKind::If),
    ("then", TokenKind::Then),
    ("else", TokenKind::Else),
    ("while", TokenKind::While),
    ("for", TokenKind::For),
    ("foreach", TokenKind::Foreach),
    ("in", TokenKind::In),
    ("do", TokenKind::Do),
    ("return", TokenKind::Return),
    ("break", TokenKind::Break),
    ("continue", TokenKind::Continue),
    ("is", TokenKind::Is),
    ("true", TokenKind::True),
    ("false", TokenKind::False),
];

/// Mots réservés pour des évolutions prévues par le cahier des charges.
/// Ils ne peuvent pas servir d'identifiants afin de pouvoir les introduire sans casser de code.
/// `ui`, `state`, `view`, `style` ne sont PAS réservés : ce seront des mots contextuels des fichiers `.uimcf`.
pub const RESERVED: &[&str] = &[
    "action",
    "as",
    "async",
    "await",
    "config",
    "const",
    "interface",
    "let",
    "match",
    "null",
    "secret",
    "set",
    "switch",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(src: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut lexer = Lexer {
        src,
        pos: 0,
        tokens: Vec::new(),
        diags: Vec::new(),
    };
    lexer.run();
    (lexer.tokens, lexer.diags)
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start, self.pos),
        });
    }

    fn error(&mut self, diag: Diagnostic) {
        self.diags.push(diag);
    }

    fn run(&mut self) {
        loop {
            self.skip_trivia();
            let start = self.pos;
            let Some(c) = self.bump() else {
                self.push(TokenKind::Eof, start);
                return;
            };
            use TokenKind::*;
            let kind = match c {
                'a'..='z' | 'A'..='Z' => {
                    self.ident(start);
                    continue;
                }
                '_' => {
                    self.local_ident(start);
                    continue;
                }
                '0'..='9' => {
                    self.number(start);
                    continue;
                }
                '"' => {
                    self.string(start);
                    continue;
                }
                '{' => LBrace,
                '}' => RBrace,
                '(' => LParen,
                ')' => RParen,
                '[' => LBracket,
                ']' => RBracket,
                ';' => Semi,
                ':' => Colon,
                ',' => Comma,
                '.' => Dot,
                '+' => Plus,
                '-' => Minus,
                '*' => Star,
                '/' => Slash,
                '%' => Percent,
                '=' => {
                    if self.eat('=') {
                        EqEq
                    } else {
                        Assign
                    }
                }
                '!' => {
                    if self.eat('=') {
                        NotEq
                    } else {
                        Bang
                    }
                }
                '<' => {
                    if self.eat('=') {
                        Le
                    } else {
                        Lt
                    }
                }
                '>' => {
                    if self.eat('=') {
                        Ge
                    } else {
                        Gt
                    }
                }
                '&' if self.eat('&') => AndAnd,
                '|' if self.eat('|') => OrOr,
                // `&` ou `|` seul : erreur, mais on produit l'opérateur logique pour éviter une cascade.
                '&' | '|' => {
                    let (kind, fix) = if c == '&' {
                        (AndAnd, "&&")
                    } else {
                        (OrOr, "||")
                    };
                    self.error(
                        Diagnostic::new(
                            &codes::UNEXPECTED_CHARACTER,
                            Span::new(start, self.pos),
                            format!("caractère inattendu `{c}`"),
                        )
                        .suggestion(format!("l'opérateur logique s'écrit `{fix}` (pas d'opérateurs bit à bit pour l'instant)")),
                    );
                    kind
                }
                other => {
                    let mut d = Diagnostic::new(
                        &codes::UNEXPECTED_CHARACTER,
                        Span::new(start, self.pos),
                        format!("caractère inattendu `{other}`"),
                    );
                    d = match other {
                        '&' => d.suggestion("le ET logique s'écrit `&&`"),
                        '|' => d.suggestion("le OU logique s'écrit `||`"),
                        '\'' => d.suggestion(
                            "les chaînes s'écrivent entre guillemets doubles `\"...\"`",
                        ),
                        _ => d,
                    };
                    self.error(d);
                    continue;
                }
            };
            self.push(kind, start);
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    let start = self.pos;
                    self.pos += 2;
                    loop {
                        if self.src[self.pos..].starts_with("*/") {
                            self.pos += 2;
                            break;
                        }
                        if self.bump().is_none() {
                            self.error(
                                Diagnostic::new(
                                    &codes::UNTERMINATED_BLOCK_COMMENT,
                                    Span::new(start, start + 2),
                                    "commentaire `/*` jamais fermé",
                                )
                                .suggestion("ajouter `*/` à la fin du commentaire"),
                            );
                            break;
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn eat_ident_continue(&mut self) {
        while self.peek().is_some_and(is_ident_continue) {
            self.bump();
        }
    }

    fn ident(&mut self, start: usize) {
        self.eat_ident_continue();
        let text = &self.src[start..self.pos];
        if let Some((_, kind)) = KEYWORDS.iter().find(|(k, _)| *k == text) {
            self.push(kind.clone(), start);
            return;
        }
        if RESERVED.contains(&text) {
            let mut d = Diagnostic::new(
                &codes::RESERVED_KEYWORD,
                Span::new(start, self.pos),
                format!("`{text}` est un mot réservé"),
            );
            d = match text {
                "set" => d.suggestion(
                    "`set` n'existe plus depuis l'édition 1.1 : écrire `nom: Type = valeur;`",
                ),
                "action" => d.suggestion(
                    "les fonctions se déclarent avec `func nom(_param: Type): Retour { }`",
                ),
                _ => d.suggestion(format!("renommer l'identifiant, par exemple `{text}Value`")),
            };
            self.error(d);
            // `set` (édition 1.0) précédait une déclaration : on l'ignore pour analyser la suite normalement.
            if text == "set" {
                return;
            }
        }
        self.push(TokenKind::Ident, start);
    }

    fn local_ident(&mut self, start: usize) {
        let valid = self.peek().is_some_and(|c| c.is_ascii_alphabetic());
        self.eat_ident_continue();
        if !valid {
            let text = &self.src[start..self.pos];
            self.error(
                Diagnostic::new(
                    &codes::INVALID_LOCAL_IDENTIFIER,
                    Span::new(start, self.pos),
                    format!("identifiant local invalide `{text}`"),
                )
                .expected("`_` suivi d'une lettre, ex. `_value`"),
            );
        }
        self.push(TokenKind::LocalIdent, start);
    }

    fn number(&mut self, start: usize) {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        let mut kind = TokenKind::Int;
        if self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
            kind = TokenKind::Float;
        }
        if self.peek().is_some_and(is_ident_continue) {
            self.eat_ident_continue();
            self.error(
                Diagnostic::new(
                    &codes::INVALID_NUMBER_LITERAL,
                    Span::new(start, self.pos),
                    format!(
                        "littéral numérique invalide `{}`",
                        &self.src[start..self.pos]
                    ),
                )
                .suggestion("un identifiant ne peut pas commencer par un chiffre"),
            );
        }
        self.push(kind, start);
    }

    fn string(&mut self, start: usize) {
        let mut value = String::new();
        loop {
            let esc_start = self.pos;
            match self.bump() {
                None => {
                    self.error(
                        Diagnostic::new(
                            &codes::UNTERMINATED_STRING,
                            Span::new(start, start + 1),
                            "chaîne jamais fermée",
                        )
                        .suggestion("ajouter `\"` à la fin de la chaîne"),
                    );
                    break;
                }
                Some('"') => break,
                Some('\n') => {
                    self.pos -= 1;
                    self.error(
                        Diagnostic::new(
                            &codes::NEWLINE_IN_STRING,
                            Span::new(start, self.pos),
                            "retour à la ligne dans une chaîne",
                        )
                        .suggestion("fermer la chaîne sur la même ligne ou utiliser `\\n`"),
                    );
                    break;
                }
                Some('\\') => match self.bump() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => value.push('\r'),
                    Some('0') => value.push('\0'),
                    Some('\\') => value.push('\\'),
                    Some('"') => value.push('"'),
                    Some('u') if self.peek() == Some('{') => {
                        self.bump();
                        let hex_start = self.pos;
                        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                            self.bump();
                        }
                        let hex = &self.src[hex_start..self.pos];
                        let decoded = (!hex.is_empty() && hex.len() <= 6)
                            .then(|| u32::from_str_radix(hex, 16).ok().and_then(char::from_u32))
                            .flatten();
                        if self.eat('}') {
                            if let Some(ch) = decoded {
                                value.push(ch);
                                continue;
                            }
                        }
                        self.error(Diagnostic::new(
                            &codes::INVALID_ESCAPE,
                            Span::new(esc_start, self.pos),
                            "échappement Unicode invalide",
                        ).expected("`\\u{XXXX}` avec 1 à 6 chiffres hexadécimaux d'un point de code valide"));
                    }
                    other => {
                        if other == Some('\n') {
                            self.pos -= 1;
                        }
                        self.error(
                            Diagnostic::new(
                                &codes::INVALID_ESCAPE,
                                Span::new(esc_start, self.pos),
                                "séquence d'échappement inconnue",
                            )
                            .expected("\\n, \\t, \\r, \\0, \\\\, \\\" ou \\u{XXXX}"),
                        );
                    }
                },
                Some(c) => value.push(c),
            }
        }
        self.push(TokenKind::Str(value), start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        let (toks, diags) = lex(src);
        assert!(diags.is_empty(), "diagnostics inattendus : {diags:?}");
        toks.into_iter().map(|t| t.kind).collect()
    }

    fn first_error(src: &str) -> &'static str {
        let (_, diags) = lex(src);
        diags.first().expect("une erreur était attendue").code.id
    }

    #[test]
    fn declarations() {
        assert_eq!(
            kinds("playerName: String = \"Alex\";"),
            vec![Ident, Colon, Ident, Assign, Str("Alex".into()), Semi, Eof]
        );
        assert_eq!(kinds("_speed: Float = 4.5;")[0], LocalIdent);
        assert_eq!(kinds("4.5")[0], Float);
    }

    #[test]
    fn keywords_and_operators() {
        assert_eq!(
            kinds("if (a <= b && !c) then { }"),
            vec![
                If, LParen, Ident, Le, Ident, AndAnd, Bang, Ident, RParen, Then, LBrace, RBrace,
                Eof
            ]
        );
        assert_eq!(
            kinds("a == b != c"),
            vec![Ident, EqEq, Ident, NotEq, Ident, Eof]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(kinds("a // x\n/* y\n z */ b"), vec![Ident, Ident, Eof]);
    }

    #[test]
    fn string_escapes() {
        assert_eq!(kinds(r#""a\n\"b\u{e9}""#)[0], Str("a\n\"bé".into()));
    }

    #[test]
    fn member_access_on_int_is_not_float() {
        assert_eq!(kinds("1.x"), vec![Int, Dot, Ident, Eof]);
    }

    #[test]
    fn errors_have_stable_codes() {
        assert_eq!(first_error("@"), "MCF0001");
        assert_eq!(first_error("\"abc"), "MCF0002");
        assert_eq!(first_error("/* abc"), "MCF0003");
        assert_eq!(first_error(r#""\q""#), "MCF0004");
        assert_eq!(first_error("_1x"), "MCF0005");
        assert_eq!(first_error("_"), "MCF0005");
        assert_eq!(first_error("12ab"), "MCF0006");
        assert_eq!(first_error("\"ab\ncd\""), "MCF0007");
        assert_eq!(first_error("set x: Int = 1;"), "MCF0008");
    }

    #[test]
    fn error_recovery_keeps_lexing() {
        let (toks, diags) = lex("a @ b");
        assert_eq!(diags.len(), 1);
        assert_eq!(toks.len(), 3);
    }
}
