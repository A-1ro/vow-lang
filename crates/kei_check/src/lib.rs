//! Kei の意味検査(名前解決・型・エフェクト・契約)クレート。
//!
//! ワークスペース全体で共有する [`Diagnostic`] 型の唯一の定義元
//! (ARCHITECTURE.md 不変条件 1)。検査本体は [`check_module`]。

pub mod check;
pub mod diagnostic;
pub mod effects;
pub mod imports;
pub mod pbt;
pub mod report;
pub mod syntax;
pub mod types;

pub use check::{
    check_module, check_module_report, check_module_report_with, check_module_report_with_resolver,
    check_module_with, check_module_with_resolver, contract_expr_text, contract_pattern_text,
    list_op_spans, list_op_spans_with_resolver, CheckOptions,
};
pub use diagnostic::{Diagnostic, Fix, Position, Severity, Span, SuggestedContract, TextEdit};
pub use imports::{
    module_type_defs, ModuleResolver, NoopResolver, ResolvedModule, ResolvedTypeDef,
    ResolvedVariant,
};
pub use report::{CheckReport, ContractInfo, ContractKind, Verification};
pub use syntax::syntax_diagnostics;
