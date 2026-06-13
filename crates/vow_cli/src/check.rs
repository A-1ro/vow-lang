//! `vow check` のランナー。ファイル読込と vow_syntax / vow_check への委譲、
//! 出力整形だけを行う(言語処理ロジックは持たない)。
//!
//! - 既定: 散文 Diagnostic を stdout に出す。
//! - `--json`: `Diagnostic[]` の整形 JSON を stdout に出す(空でも `[]`)。
//! - 終了コード: error 深刻度の Diagnostic が 1 件でもあれば 1、なければ 0。

use std::path::Path;

use vow_check::Severity;

use crate::cli::UsageError;
use crate::render;

/// `vow check`。成功時は終了コード(0 / 1)、ファイル読込失敗は使用法エラー。
pub fn run(file: &Path, json: bool) -> Result<u8, UsageError> {
    let source = crate::read_source(file)?;
    let name = file.to_string_lossy().into_owned();

    // vow_mcp::tools::run_check と同方針: 構文エラーがあるときは壊れた AST に
    // 意味検査をかけず、構文 Diagnostic だけを返す。
    let parsed = vow_syntax::parse_module(&source);
    let mut diags = vow_check::syntax_diagnostics(&name, &parsed.errors);
    if parsed.errors.is_empty() {
        diags.extend(vow_check::check_module(&name, &parsed.module));
    }

    let has_error = diags.iter().any(|d| d.severity == Severity::Error);

    if json {
        let mut out =
            serde_json::to_string_pretty(&diags).expect("Diagnostic[] is always serializable");
        out.push('\n');
        print!("{out}");
    } else {
        print!("{}", render::diagnostics(&source, &diags));
    }

    Ok(u8::from(has_error))
}
