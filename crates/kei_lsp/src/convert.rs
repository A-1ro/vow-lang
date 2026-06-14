//! kei_check の構造化 [`Diagnostic`] と LSP の型の境界変換。
//!
//! ARCHITECTURE.md: kei_lsp は言語処理ロジックを持たず、kei_check が出した
//! Diagnostic を LSP の Diagnostic に写すだけのアダプタに徹する。検査・整形・
//! パースは一切再実装しない。
//!
//! 位置系の差異に注意する:
//! - kei (spec/diagnostic-schema.md): `line` / `col` ともに **1 始まり**、
//!   `col` は **Unicode スカラー値**単位。
//! - LSP: `line` / `character` ともに **0 始まり**、`character` は既定で
//!   **UTF-16 コードユニット**単位。
//!
//! v0.1 の最小実装では BMP 内(サロゲートペアなし)を前提に、スカラー値単位の
//! 列をそのまま 0 始まりへずらして使う。非 BMP 文字を含む行での列ずれは
//! 既知の制約として後段(UTF-16 オフセット計算)で解消する。

use kei_check::{Diagnostic as KeiDiagnostic, Position as KeiPosition, Severity, Span as KeiSpan};
use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, NumberOrString, Position as LspPosition,
    Range as LspRange,
};

/// kei の 1 始まり [`KeiPosition`] を LSP の 0 始まり [`LspPosition`] に写す。
/// 1 行 1 列より手前(理論上はあり得ない)に飽和しないよう saturating で減算する。
pub fn position_to_lsp(p: KeiPosition) -> LspPosition {
    LspPosition {
        line: p.line.saturating_sub(1),
        character: p.col.saturating_sub(1),
    }
}

/// kei の [`KeiSpan`] を LSP の [`LspRange`] に写す(file は捨てる)。
pub fn span_to_range(span: &KeiSpan) -> LspRange {
    LspRange {
        start: position_to_lsp(span.start),
        end: position_to_lsp(span.end),
    }
}

fn severity_to_lsp(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// kei の [`KeiDiagnostic`] を LSP の [`LspDiagnostic`] に写す。
///
/// - `code` は LSP の `code`(文字列)へ。
/// - `fixes` の先頭タイトル群は人間向けに `message` 末尾へ畳み込む。
///   機械適用(CodeAction)は後段の機能で `fixes` の TextEdit を使う。
pub fn diagnostic_to_lsp(diag: &KeiDiagnostic) -> LspDiagnostic {
    let mut message = diag.message.clone();
    // fix の方向(タイトル)を一行ずつ添える。全 Diagnostic は最低 1 つ fix を持つ。
    for fix in &diag.fixes {
        message.push_str("\n  fix: ");
        message.push_str(&fix.title);
    }

    LspDiagnostic {
        range: span_to_range(&diag.span),
        severity: Some(severity_to_lsp(diag.severity)),
        code: Some(NumberOrString::String(diag.code.clone())),
        code_description: None,
        source: Some("kei".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kei_check::{Fix, Position, Span};

    #[test]
    fn position_is_zero_based() {
        let p = position_to_lsp(Position { line: 1, col: 1 });
        assert_eq!(p.line, 0);
        assert_eq!(p.character, 0);
    }

    #[test]
    fn position_saturates_at_zero() {
        let p = position_to_lsp(Position { line: 0, col: 0 });
        assert_eq!(p.line, 0);
        assert_eq!(p.character, 0);
    }

    #[test]
    fn diagnostic_carries_code_source_and_fix() {
        let diag = KeiDiagnostic {
            severity: Severity::Error,
            code: "KEI-E3001".to_string(),
            message: "effect not declared".to_string(),
            span: Span {
                file: "source.kei".to_string(),
                start: Position { line: 3, col: 5 },
                end: Position { line: 3, col: 10 },
            },
            fixes: vec![Fix {
                title: "Add 'Database.Write' to uses clause".to_string(),
                edits: vec![],
            }],
        };
        let lsp = diagnostic_to_lsp(&diag);
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(lsp.source.as_deref(), Some("kei"));
        assert_eq!(
            lsp.code,
            Some(NumberOrString::String("KEI-E3001".to_string()))
        );
        assert_eq!(
            lsp.range.start,
            LspPosition {
                line: 2,
                character: 4
            }
        );
        assert_eq!(
            lsp.range.end,
            LspPosition {
                line: 2,
                character: 9
            }
        );
        assert!(lsp.message.contains("effect not declared"));
        assert!(lsp.message.contains("Add 'Database.Write' to uses clause"));
    }
}
