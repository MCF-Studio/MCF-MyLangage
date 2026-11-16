//! Diagnostics du compilateur MCF.
//!
//! Chaque diagnostic possède un identifiant stable (`MCFxxxx`) déclaré dans [`codes`].
//! Un identifiant publié ne change jamais de sens : on en ajoute, on n'en réutilise pas.

use serde::Serialize;
use serde_json::{json, Value};

pub mod codes;

pub use codes::Code;

/// Plage d'octets `[start, end)` dans un fichier source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Lexical,
    Syntax,
    Semantic,
    Type,
    Ownership,
    Import,
    Ui,
    Config,
    Security,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static Code,
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    pub expected: Option<String>,
    pub received: Option<String>,
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn new(code: &'static Code, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: code.severity,
            span,
            message: message.into(),
            expected: None,
            received: None,
            suggestion: None,
        }
    }

    pub fn expected(mut self, value: impl Into<String>) -> Self {
        self.expected = Some(value.into());
        self
    }

    pub fn received(mut self, value: impl Into<String>) -> Self {
        self.received = Some(value.into());
        self
    }

    pub fn suggestion(mut self, value: impl Into<String>) -> Self {
        self.suggestion = Some(value.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Mode strict : aucun avertissement ne peut passer silencieusement.
    pub fn promote_to_error(&mut self) {
        self.severity = Severity::Error;
    }
}

/// Fichier source avec index des débuts de ligne pour convertir les offsets.
pub struct SourceFile {
    pub name: String,
    pub text: String,
    line_starts: Vec<usize>,
}

/// Position humaine, 1-based ; la colonne compte des caractères Unicode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self {
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn location(&self, offset: usize) -> Location {
        let offset = offset.min(self.text.len());
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let line_start = self.line_starts[line_index];
        let column = self.text[line_start..offset].chars().count() + 1;
        Location {
            line: line_index + 1,
            column,
        }
    }

    fn line_text(&self, line: usize) -> &str {
        let start = self.line_starts[line - 1];
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.text.len());
        self.text[start..end].trim_end_matches(['\n', '\r'])
    }

    /// Rendu lisible par un humain, avec extrait du code et soulignement.
    pub fn render(&self, diag: &Diagnostic) -> String {
        let start = self.location(diag.span.start);
        let end = self.location(diag.span.end);
        let severity = match diag.severity {
            Severity::Error => "erreur",
            Severity::Warning => "avertissement",
        };
        let mut out = format!(
            "{severity}[{}] {}: {}\n  --> {}:{}:{}\n",
            diag.code.id, diag.code.name, diag.message, self.name, start.line, start.column
        );
        let line = self.line_text(start.line);
        let gutter = start.line.to_string();
        let pad = " ".repeat(gutter.len());
        let width = if end.line == start.line {
            end.column.saturating_sub(start.column).max(1)
        } else {
            1
        };
        out.push_str(&format!("{pad} |\n{gutter} | {line}\n{pad} | "));
        out.push_str(&" ".repeat(start.column - 1));
        out.push_str(&"^".repeat(width));
        out.push('\n');
        if let Some(e) = &diag.expected {
            out.push_str(&format!("{pad} = attendu : {e}\n"));
        }
        if let Some(r) = &diag.received {
            out.push_str(&format!("{pad} = reçu : {r}\n"));
        }
        if let Some(s) = &diag.suggestion {
            out.push_str(&format!("{pad} = suggestion : {s}\n"));
        }
        out
    }

    /// Rendu JSON stable destiné aux outils et agents IA.
    pub fn to_json(&self, diag: &Diagnostic) -> Value {
        let start = self.location(diag.span.start);
        let end = self.location(diag.span.end);
        json!({
            "id": diag.code.id,
            "name": diag.code.name,
            "category": diag.code.category,
            "severity": diag.severity,
            "file": self.name,
            "start": start,
            "end": end,
            "byte_span": diag.span,
            "message": diag.message,
            "expected": diag.expected,
            "received": diag.received,
            "suggestion": diag.suggestion,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_counts_lines_and_unicode_columns() {
        let file = SourceFile::new("t.mcf", "a\néé x\n");
        assert_eq!(file.location(0), Location { line: 1, column: 1 });
        // "éé " = 5 octets, 3 caractères
        assert_eq!(file.location(7), Location { line: 2, column: 4 });
    }

    #[test]
    fn render_contains_id_position_and_caret() {
        let file = SourceFile::new("t.mcf", "x: Int = @;\n");
        let diag = Diagnostic::new(
            &codes::UNEXPECTED_CHARACTER,
            Span::new(9, 10),
            "caractère inattendu `@`",
        );
        let text = file.render(&diag);
        assert!(text.contains("MCF0001"));
        assert!(text.contains("t.mcf:1:10"));
        assert!(text.contains("         ^"));
    }

    #[test]
    fn codes_are_unique() {
        let mut ids: Vec<_> = codes::ALL.iter().map(|c| c.id).collect();
        ids.sort();
        let len = ids.len();
        ids.dedup();
        assert_eq!(len, ids.len(), "identifiant de diagnostic dupliqué");
    }
}
