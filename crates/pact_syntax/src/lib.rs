//! Pact のレキサー・パーサ・AST 定義クレート。型の知識は持たない。
//!
//! エラーがあっても可能な範囲の AST と複数の [`SyntaxError`] を返す
//! (エラー回復)。構文エラーは pact_check の境界変換で Diagnostic になる
//! (ARCHITECTURE.md 不変条件 1)。

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod token;

pub use ast::Module;
pub use error::{FixHint, SyntaxError};
pub use span::{Position, Span};

/// パース結果。エラーがあっても `module` は回復済みの部分 AST を保持する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    pub module: Module,
    /// ソース上の出現位置順(行・列)に整列済み。
    pub errors: Vec<SyntaxError>,
}

/// 1 ソースファイルをパースする。
pub fn parse_module(source: &str) -> ParseResult {
    let (tokens, lex_errors) = lexer::lex(source);
    let mut parser = parser::Parser::new(tokens, lex_errors);
    let module = parser.parse_module();
    let mut errors = parser.into_errors();
    errors.sort_by_key(|e| (e.span.start.line, e.span.start.col));
    ParseResult { module, errors }
}
