//! Kei の言語サーバー(LSP)クレート。
//!
//! ARCHITECTURE.md の依存規則に従い、依存は `kei_lsp → kei_check / kei_syntax /
//! kei_fmt` の一方向のみ。LSP サーバーは kei_check が出す構造化 [`Diagnostic`] と
//! kei_syntax の AST を LSP プロトコルに翻訳するアダプタに徹し、検査・整形・
//! パースは一切再実装しない(kei_cli / kei_mcp と同じ「言語処理を持たない薄い層」)。
//!
//! 構成:
//! - [`analysis`] — ソース文字列を取り、Diagnostic / Hover の素を返す純関数。
//!   サーバー I/O から独立しており、プロセス起動なしで単体テストできる。
//! - [`convert`] — kei の Diagnostic / Span ↔ LSP 型の境界変換。
//! - [`server`] — lsp-server による同期 stdio ループ(initialize / didOpen /
//!   didChange → publishDiagnostics / hover)。
//!
//! [`Diagnostic`]: kei_check::Diagnostic

pub mod analysis;
pub mod convert;
pub mod server;

pub use server::run_stdio;
