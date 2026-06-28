//! `kei check` のランナー。ファイル読込と kei_syntax / kei_check への委譲、
//! 出力整形だけを行う(言語処理ロジックは持たない)。
//!
//! - 既定: 散文 Diagnostic + 契約の検証レベル要約を stdout に出す。
//! - `--json`: `CheckReport`(`{ diagnostics, contracts }`)の整形 JSON を出す。
//! - 終了コード: error 深刻度の Diagnostic が 1 件でもあれば 1、なければ 0。

use std::path::Path;

use kei_check::{CheckOptions, CheckReport, Severity};

use crate::cli::UsageError;
use crate::render;

/// `kei check`。成功時は終了コード(0 / 1)、ファイル読込失敗は使用法エラー。
pub fn run(
    file: &Path,
    json: bool,
    strict_extern: bool,
    generative: bool,
    suggest_contracts: bool,
) -> Result<u8, UsageError> {
    let source = crate::read_source(file)?;
    let name = file.to_string_lossy().into_owned();
    let opts = CheckOptions {
        strict_extern,
        generative,
        suggest_contracts,
    };

    // kei_mcp::tools::run_check と同方針: 構文エラーがあるときは壊れた AST に
    // 意味検査をかけず、構文 Diagnostic だけを返す(契約レポートも出さない)。
    let parsed = kei_syntax::parse_module(&source);
    let mut report = if parsed.errors.is_empty() {
        // M20: `module a.b.c` から project root を逆算し、可能なら import 先を解決する。
        // root が割り出せない / 解決できないファイルは従来どおり opaque で検査続行。
        match crate::resolve::derive_root(file, &parsed.module) {
            Some(root) => {
                let resolver = crate::resolve::FsModuleResolver::new(root);
                kei_check::check_module_report_with_resolver(&name, &parsed.module, opts, &resolver)
            }
            None => kei_check::check_module_report_with(&name, &parsed.module, opts),
        }
    } else {
        CheckReport {
            diagnostics: kei_check::syntax_diagnostics(&name, &parsed.errors),
            contracts: Vec::new(),
        }
    };

    // シード注入(M15 段階2): --generative 時、`<stem>.seeds` が隣にあれば検証する。入力のみの
    // シードを requires に照らし違反シードを弾き、ensures を破ったシードはその契約を generative
    // から runtime へ降格する(レポートが「generative」と KEI-E4005 を同時に主張する矛盾を防ぐ)。
    // 生成・判定・降格は kei_check に置く。
    let mut seed_block: Option<(String, Vec<kei_check::Diagnostic>)> = None;
    if generative && parsed.errors.is_empty() {
        let seed_path = file.with_extension("seeds");
        if seed_path.is_file() {
            let seed_src = crate::read_source(&seed_path)?;
            let seed_name = seed_path.to_string_lossy().into_owned();
            let seed_diags = kei_check::pbt::check_seeds(
                &seed_name,
                &seed_src,
                &parsed.module,
                &mut report.contracts,
            );
            seed_block = Some((seed_src, seed_diags));
        }
    }

    let seed_has_error = seed_block
        .as_ref()
        .map(|(_, ds)| ds.iter().any(|d| d.severity == Severity::Error))
        .unwrap_or(false);
    let has_error = report
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
        || seed_has_error;

    if json {
        // シード診断は別ファイル span を持つので、診断配列へ併合して 1 つの JSON にする。
        if let Some((_, seed_diags)) = &seed_block {
            report.diagnostics.extend(seed_diags.iter().cloned());
        }
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
        // シード診断は自身のソースに対して整形する(キャレットがシードファイルを指す)。
        if let Some((seed_src, seed_diags)) = &seed_block {
            let seed_out = render::diagnostics(seed_src, seed_diags);
            if !seed_out.is_empty() {
                if !diags.is_empty() || !contracts.is_empty() {
                    println!();
                }
                print!("{seed_out}");
            }
        }
    }

    Ok(u8::from(has_error))
}
