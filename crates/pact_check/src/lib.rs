//! Pact の意味検査(名前解決・型・エフェクト・契約)クレート。
//!
//! ワークスペース全体で共有する [`Diagnostic`] 型の唯一の定義元
//! (ARCHITECTURE.md 不変条件 1)。検査本体は [`check_module`]。

pub mod check;
pub mod diagnostic;
pub mod effects;
pub mod syntax;
pub mod types;

pub use check::check_module;
pub use diagnostic::{Diagnostic, Fix, Position, Severity, Span, TextEdit};
pub use syntax::syntax_diagnostics;
