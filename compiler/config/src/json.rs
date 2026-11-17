//! Lecture JSON (RFC 8259) avec commentaires `//` et `/* */`. Chaque valeur garde sa position
//! pour que les diagnostics pointent au bon endroit du `.cfg`.
//!
//! La lecture s'arrête à la première erreur de syntaxe : au-delà, la structure du fichier n'est
//! plus fiable et les erreurs suivantes seraient des conséquences de la première.

use mcf_diagnostics::{codes, Diagnostic, Span};

use crate::{MAX_DEPTH, MAX_SIZE};

/// Les erreurs de lecture sont rares et grosses : elles voyagent dans une `Box`.
fn error(code: &'static mcf_diagnostics::Code, span: Span, message: String) -> Box<Diagnostic> {
    Box::new(crate::error(code, span, message))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Json {
    pub kind: JsonKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum JsonKind {
    Null,
    Bool(bool),
    /// Texte du nombre tel qu'écrit ; `integer` : ni partie décimale ni exposant.
    Number {
        text: String,
        integer: bool,
    },
    Str(String),
    Array(Vec<Json>),
    /// Membres dans l'ordre du fichier, clés toutes différentes.
    Object(Vec<Member>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub key: String,
    pub key_span: Span,
    pub value: Json,
}

impl JsonKind {
    /// « une chaîne », « un objet »… pour les messages.
    pub fn describe(&self) -> &'static str {
        match self {
            JsonKind::Null => "`null`",
            JsonKind::Bool(_) => "un booléen",
            JsonKind::Number { integer: true, .. } => "un nombre entier",
            JsonKind::Number { .. } => "un nombre décimal",
            JsonKind::Str(_) => "une chaîne",
            JsonKind::Array(_) => "un tableau",
            JsonKind::Object(_) => "un objet",
        }
    }
}

/// Lit un fichier `.cfg` complet : une seule valeur JSON, entourée d'espaces ou de commentaires.
pub fn parse(text: &str) -> Result<Json, Box<Diagnostic>> {
    if text.len() > MAX_SIZE {
        return Err(error(
            &codes::CONFIG_LIMIT_EXCEEDED,
            Span::new(0, 0),
            format!(
                "fichier de {} octets : la limite est de {} Mio",
                text.len(),
                MAX_SIZE / (1024 * 1024)
            ),
        ));
    }
    let mut p = Parser { src: text, pos: 0 };
    p.skip_space()?;
    if p.pos == text.len() {
        return Err(error(
            &codes::CONFIG_SYNTAX,
            Span::new(0, 0),
            "fichier vide : une valeur JSON est attendue (souvent un objet `{ ... }`)".into(),
        ));
    }
    let value = p.value(0)?;
    p.skip_space()?;
    if p.pos < text.len() {
        return Err(p.fail(
            p.char_span(),
            "texte après la fin de la valeur JSON",
            Some("un fichier `.cfg` contient une seule valeur ; regrouper les valeurs dans un objet `{ ... }`"),
        ));
    }
    Ok(value)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn char_span(&self) -> Span {
        let len = self.peek().map_or(0, char::len_utf8);
        Span::new(self.pos, self.pos + len)
    }

    fn fail(&self, span: Span, message: &str, suggestion: Option<&str>) -> Box<Diagnostic> {
        let mut d = crate::error(&codes::CONFIG_SYNTAX, span, message.to_string());
        if let Some(s) = suggestion {
            d = d.suggestion(s);
        }
        Box::new(d)
    }

    fn found(&self) -> String {
        match self.peek() {
            None => "la fin du fichier".into(),
            Some(c) => format!("`{c}`"),
        }
    }

    /// Espaces JSON (espace, tabulation, retours à la ligne) et commentaires.
    fn skip_space(&mut self) -> Result<(), Box<Diagnostic>> {
        loop {
            let rest = &self.src[self.pos..];
            if let Some(c) = rest
                .chars()
                .next()
                .filter(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
            {
                self.pos += c.len_utf8();
            } else if rest.starts_with("//") {
                self.pos += rest.find('\n').unwrap_or(rest.len());
            } else if let Some(body) = rest.strip_prefix("/*") {
                match body.find("*/") {
                    Some(end) => self.pos += end + 4,
                    None => {
                        return Err(self.fail(
                            Span::new(self.pos, self.pos + 2),
                            "commentaire `/*` jamais fermé",
                            Some("fermer le commentaire avec `*/`"),
                        ))
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, Box<Diagnostic>> {
        let start = self.pos;
        match self.peek() {
            Some('{') | Some('[') if depth >= MAX_DEPTH => Err(error(
                &codes::CONFIG_LIMIT_EXCEEDED,
                self.char_span(),
                format!("plus de {MAX_DEPTH} niveaux d'imbrication"),
            )),
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"') => {
                let s = self.string()?;
                Ok(Json {
                    kind: JsonKind::Str(s),
                    span: Span::new(start, self.pos),
                })
            }
            Some('\'') => Err(self.fail(
                self.char_span(),
                "chaîne entre apostrophes",
                Some("en JSON, les chaînes s'écrivent entre guillemets doubles : \"texte\""),
            )),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            Some(c) if c.is_ascii_alphabetic() => self.word(),
            _ => Err(self.fail(
                self.char_span(),
                &format!("valeur JSON attendue, trouvé {}", self.found()),
                None,
            )),
        }
    }

    fn word(&mut self) -> Result<Json, Box<Diagnostic>> {
        let start = self.pos;
        let len = self.src[start..]
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(self.src.len() - start);
        self.pos += len;
        let span = Span::new(start, self.pos);
        let kind = match &self.src[start..self.pos] {
            "true" => JsonKind::Bool(true),
            "false" => JsonKind::Bool(false),
            "null" => JsonKind::Null,
            w => {
                let hint = match w {
                    "True" | "TRUE" => "écrire `true` en minuscules".to_string(),
                    "False" | "FALSE" => "écrire `false` en minuscules".to_string(),
                    _ => format!("une chaîne s'écrit entre guillemets : \"{w}\""),
                };
                return Err(self.fail(
                    span,
                    &format!("`{w}` n'est pas une valeur JSON"),
                    Some(&hint),
                ));
            }
        };
        Ok(Json { kind, span })
    }

    fn number(&mut self) -> Result<Json, Box<Diagnostic>> {
        let start = self.pos;
        let bytes = self.src.as_bytes();
        let digits = |p: &mut usize| {
            let s = *p;
            while *p < bytes.len() && bytes[*p].is_ascii_digit() {
                *p += 1;
            }
            *p - s
        };
        let mut p = start;
        let bad = |p: usize, what: &str| {
            self.fail(
                Span::new(start, p.max(start + 1)),
                &format!("nombre mal formé : {what}"),
                None,
            )
        };
        if bytes[p] == b'-' {
            p += 1;
        }
        let int_start = p;
        let n = digits(&mut p);
        if n == 0 {
            return Err(bad(p, "chiffre attendu"));
        }
        if n > 1 && bytes[int_start] == b'0' {
            return Err(bad(p, "pas de zéro au début d'un nombre"));
        }
        let mut integer = true;
        if p < bytes.len() && bytes[p] == b'.' {
            p += 1;
            integer = false;
            if digits(&mut p) == 0 {
                return Err(bad(p, "chiffre attendu après le point"));
            }
        }
        if p < bytes.len() && (bytes[p] == b'e' || bytes[p] == b'E') {
            p += 1;
            integer = false;
            if p < bytes.len() && (bytes[p] == b'+' || bytes[p] == b'-') {
                p += 1;
            }
            if digits(&mut p) == 0 {
                return Err(bad(p, "chiffre attendu dans l'exposant"));
            }
        }
        self.pos = p;
        Ok(Json {
            kind: JsonKind::Number {
                text: self.src[start..p].to_string(),
                integer,
            },
            span: Span::new(start, p),
        })
    }

    /// Chaîne JSON, `self.pos` sur le guillemet ouvrant.
    fn string(&mut self) -> Result<String, Box<Diagnostic>> {
        let open = self.pos;
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.fail(
                    Span::new(open, open + 1),
                    "chaîne jamais fermée",
                    Some("fermer la chaîne avec `\"`"),
                ));
            };
            match c {
                '"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                '\\' => {
                    out.push(self.escape()?);
                }
                c if (c as u32) < 0x20 => {
                    return Err(self.fail(
                        self.char_span(),
                        "caractère de contrôle (retour à la ligne, tabulation…) dans une chaîne",
                        Some("l'écrire avec un échappement : \\n, \\t, \\r"),
                    ));
                }
                c => {
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
    }

    /// Échappement, `self.pos` sur la barre oblique inverse.
    fn escape(&mut self) -> Result<char, Box<Diagnostic>> {
        let start = self.pos;
        self.pos += 1;
        let Some(c) = self.peek() else {
            return Err(self.fail(Span::new(start, self.pos), "échappement incomplet", None));
        };
        self.pos += c.len_utf8();
        Ok(match c {
            '"' => '"',
            '\\' => '\\',
            '/' => '/',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                let high = self.hex4(start)?;
                if (0xD800..0xDC00).contains(&high) {
                    // Paire de substitution UTF-16 : `😀`.
                    if !self.src[self.pos..].starts_with("\\u") {
                        return Err(self.lone_surrogate(start));
                    }
                    self.pos += 2;
                    let low = self.hex4(start)?;
                    if !(0xDC00..0xE000).contains(&low) {
                        return Err(self.lone_surrogate(start));
                    }
                    let code = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                    char::from_u32(code).ok_or_else(|| self.lone_surrogate(start))?
                } else {
                    char::from_u32(high).ok_or_else(|| self.lone_surrogate(start))?
                }
            }
            other => {
                return Err(self.fail(
                    Span::new(start, self.pos),
                    &format!("échappement inconnu `\\{other}`"),
                    Some("échappements JSON : \\\" \\\\ \\/ \\b \\f \\n \\r \\t \\uXXXX"),
                ))
            }
        })
    }

    fn hex4(&mut self, start: usize) -> Result<u32, Box<Diagnostic>> {
        let hex = self.src.get(self.pos..self.pos + 4).unwrap_or_default();
        match u32::from_str_radix(hex, 16) {
            Ok(v) if hex.len() == 4 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                self.pos += 4;
                Ok(v)
            }
            _ => Err(self.fail(
                Span::new(start, (self.pos + 4).min(self.src.len())),
                "`\\u` doit être suivi de 4 chiffres hexadécimaux",
                None,
            )),
        }
    }

    fn lone_surrogate(&self, start: usize) -> Box<Diagnostic> {
        self.fail(
            Span::new(start, self.pos),
            "caractère `\\u` invalide (moitié de paire UTF-16)",
            None,
        )
    }

    fn array(&mut self, depth: usize) -> Result<Json, Box<Diagnostic>> {
        let start = self.pos;
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_space()?;
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Json {
                kind: JsonKind::Array(items),
                span: Span::new(start, self.pos),
            });
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_space()?;
            match self.peek() {
                Some(',') => {
                    let comma = self.char_span();
                    self.pos += 1;
                    self.skip_space()?;
                    if self.peek() == Some(']') {
                        return Err(trailing_comma(self, comma));
                    }
                }
                Some(']') => {
                    self.pos += 1;
                    return Ok(Json {
                        kind: JsonKind::Array(items),
                        span: Span::new(start, self.pos),
                    });
                }
                _ => {
                    return Err(self.fail(
                        self.char_span(),
                        &format!("`,` ou `]` attendu, trouvé {}", self.found()),
                        Some("séparer les éléments par `,` et fermer le tableau par `]`"),
                    ))
                }
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, Box<Diagnostic>> {
        let start = self.pos;
        self.pos += 1;
        let mut members: Vec<Member> = Vec::new();
        self.skip_space()?;
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Json {
                kind: JsonKind::Object(members),
                span: Span::new(start, self.pos),
            });
        }
        loop {
            let key_start = self.pos;
            let key = match self.peek() {
                Some('"') => self.string()?,
                Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '\'' => {
                    let end = self.src[key_start..]
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '\'')
                        .map_or(self.src.len(), |e| key_start + e);
                    let word = self.src[key_start..end].trim_matches('\'');
                    return Err(self.fail(
                        Span::new(key_start, end),
                        "clé sans guillemets doubles",
                        Some(&format!(
                            "en JSON, une clé s'écrit entre guillemets : \"{word}\""
                        )),
                    ));
                }
                _ => {
                    return Err(self.fail(
                        self.char_span(),
                        &format!("clé entre guillemets attendue, trouvé {}", self.found()),
                        None,
                    ))
                }
            };
            let key_span = Span::new(key_start, self.pos);
            if members.iter().any(|m| m.key == key) {
                return Err(Box::new(
                    crate::error(
                        &codes::CONFIG_DUPLICATE_KEY,
                        key_span,
                        format!("la clé \"{key}\" est déjà présente dans cet objet"),
                    )
                    .suggestion("garder une seule fois chaque clé"),
                ));
            }
            self.skip_space()?;
            if self.peek() != Some(':') {
                return Err(self.fail(
                    self.char_span(),
                    &format!("`:` attendu après la clé, trouvé {}", self.found()),
                    None,
                ));
            }
            self.pos += 1;
            self.skip_space()?;
            let value = self.value(depth + 1)?;
            members.push(Member {
                key,
                key_span,
                value,
            });
            self.skip_space()?;
            match self.peek() {
                Some(',') => {
                    let comma = self.char_span();
                    self.pos += 1;
                    self.skip_space()?;
                    if self.peek() == Some('}') {
                        return Err(trailing_comma(self, comma));
                    }
                }
                Some('}') => {
                    self.pos += 1;
                    return Ok(Json {
                        kind: JsonKind::Object(members),
                        span: Span::new(start, self.pos),
                    });
                }
                _ => {
                    return Err(self.fail(
                        self.char_span(),
                        &format!("`,` ou `}}` attendu, trouvé {}", self.found()),
                        Some("séparer les membres par `,` et fermer l'objet par `}`"),
                    ))
                }
            }
        }
    }
}

fn trailing_comma(p: &Parser<'_>, comma: Span) -> Box<Diagnostic> {
    p.fail(
        comma,
        "virgule après le dernier élément",
        Some("JSON n'accepte pas de virgule finale : la retirer"),
    )
}
