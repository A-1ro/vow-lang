//! `kei check` のランナー。ファイル読込と kei_syntax / kei_check への委譲、
//! 出力整形だけを行う(言語処理ロジックは持たない)。
//!
//! - 既定: 散文 Diagnostic + 契約の検証レベル要約を stdout に出す。
//! - `--json`: `CheckReport`(`{ diagnostics, contracts }`)の整形 JSON を出す。
//! - 終了コード: error 深刻度の Diagnostic が 1 件でもあれば 1、なければ 0。

use std::path::Path;

use kei_check::{CheckReport, Severity};

use crate::cli::UsageError;
use crate::render;

/// `kei check`。成功時は終了コード(0 / 1)、ファイル読込失敗は使用法エラー。
pub fn run(file: &Path, json: bool) -> Result<u8, UsageError> {
    let source = crate::read_source(file)?;
    let name = file.to_string_lossy().into_owned();

    // kei_mcp::tools::run_check と同方針: 構文エラーがあるときは壊れた AST に
    // 意味検査をかけず、構文 Diagnostic だけを返す(契約レポートも出さない)。
    let parsed = kei_syntax::parse_module(&source);
    let report = if parsed.errors.is_empty() {
        kei_check::check_module_report(&name, &parsed.module)
    } else {
        CheckReport {
            diagnostics: kei_check::syntax_diagnostics(&name, &parsed.errors),
            contracts: Vec::new(),
        }
    };

    let has_error = report
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error);

    if json {
        let mut out =
            serde_json::to_string_pretty(&report).expect("CheckReport is always serializable");
        out.push('\n');
        print!("{out}");
    } else {
        let diags = render::diagnostics(&source, &report.diagnostics);
        let contracts = render::contracts(&report.contracts);
        print!("{diags}");
        if !diags.is_empty() && !contracts.is_empty() {
            println!();
        }
        print!("{contracts}");
    }

    Ok(u8::from(has_error))
}
