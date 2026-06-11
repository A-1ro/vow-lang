//! Pact の意味検査(名前解決・型・エフェクト・契約)クレート。
//!
//! ワークスペース全体で共有する [`Diagnostic`] 型の唯一の定義元
//! (ARCHITECTURE.md 不変条件 1)。検査ロジック本体は M3 で実装する。

pub mod diagnostic;
pub mod syntax;

pub use diagnostic::{Diagnostic, Fix, Position, Severity, Span, TextEdit};
pub use syntax::syntax_diagnostics;
