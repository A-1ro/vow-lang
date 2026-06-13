//! kei_syntax の内部エラー型 → [`Diagnostic`] の境界変換。
//!
//! ARCHITECTURE.md 不変条件 1: Diagnostic は kei_check が唯一の定義元であり、
//! 各クレートの内部エラーはこの境界で Diagnostic に写す。kei_syntax の
//! `SyntaxError` は常に fix ヒントを 1 つ携えるため、「全 Diagnostic に
//! 最低 1 つの fix 候補」の不変条件をここで満たせる。

use crate::{Diagnostic, Fix, Position, Severity, Span, TextEdit};

/// 構文エラー列を Diagnostic 列へ変換する。`file` はリポジトリルートからの相対パス。
pub fn syntax_diagnostics(file: &str, errors: &[kei_syntax::SyntaxError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| {
            let fix = Fix {
                title: e.fix.title.clone(),
                edits: e
                    .fix
                    .edits
                    .iter()
                    .map(|(span, new_text)| TextEdit {
                        span: convert_span(file, *span),
                        new_text: new_text.clone(),
                    })
                    .collect(),
            };
            Diagnostic::new(
                Severity::Error,
                e.code,
                e.message.clone(),
                convert_span(file, e.span),
                vec![fix],
            )
            .expect("syntax errors always carry a fix hint")
        })
        .collect()
}

fn convert_span(file: &str, span: kei_syntax::Span) -> Span {
    Span {
        file: file.to_string(),
        start: Position {
            line: span.start.line,
            col: span.start.col,
        },
        end: Position {
            line: span.end.line,
            col: span.end.col,
        },
    }
}
