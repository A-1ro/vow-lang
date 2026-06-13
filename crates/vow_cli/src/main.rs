//! `vow` CLI のエントリポイント。check / fmt / build / test サブコマンドを提供する。
//!
//! ARCHITECTURE.md(vow_cli 行): 言語処理ロジックは持たず、引数解釈・
//! ファイル IO・Diagnostic の散文整形だけを担い、検査・整形・トランスパイルは
//! vow_check / vow_fmt / vow_syntax / vow_emit に委譲する。`test` は dev ビルド後に
//! プロジェクトの `npm test` へ委譲する薄いラッパーで、テストランナーの知識を持たない。
//!
//! 終了コード規約(M6 事前合意・全サブコマンド共通):
//! - `0` 成功(検査エラーなし / 整形済み / ビルド成功 / テスト全件パス)
//! - `1` 診断エラー検出(check / build)・未整形(fmt --check)・構文エラー(fmt)・テスト失敗(test)
//! - `2` 使用法エラー(引数不正・ファイル/ディレクトリ不在)

use std::path::Path;
use std::process::ExitCode;

mod build;
mod check;
mod cli;
mod fmt;
mod render;
mod test;

use cli::{Command, UsageError, USAGE};

const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match cli::parse(args) {
        Ok(Command::Help) => {
            print!("{USAGE}");
            0
        }
        Ok(Command::Version) => {
            println!("vow {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Ok(Command::Check { file, json }) => dispatch(check::run(&file, json)),
        Ok(Command::Fmt { file, mode }) => dispatch(fmt::run(&file, mode)),
        Ok(Command::Build {
            dir,
            out_dir,
            source_map,
        }) => dispatch(build::run(&dir, out_dir.as_deref(), source_map)),
        Ok(Command::Test { dir }) => dispatch(test::run(&dir)),
        Err(UsageError(msg)) => {
            eprintln!("vow: {msg}\n\n{USAGE}");
            EXIT_USAGE
        }
    };
    ExitCode::from(code)
}

/// ランナーの結果を終了コードに写す。実行中の使用法エラー(ファイル IO 失敗)は
/// 使い方を併記せず一行で報告する。
fn dispatch(result: Result<u8, UsageError>) -> u8 {
    match result {
        Ok(code) => code,
        Err(UsageError(msg)) => {
            eprintln!("vow: {msg}");
            EXIT_USAGE
        }
    }
}

/// ファイル読込の共通ヘルパ。読めなければ使用法エラー(終了コード 2)。
fn read_source(file: &Path) -> Result<String, UsageError> {
    std::fs::read_to_string(file)
        .map_err(|e| UsageError(format!("cannot read {}: {e}", file.display())))
}
