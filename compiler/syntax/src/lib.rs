//! Analyse syntaxique du langage MCF : lexer, AST et parser.

pub mod ast;
pub mod lexer;
pub mod parser;

pub use parser::{parse, parse_ui, ParseResult, UiParseResult};
