//! `vow` CLI のエントリポイント。check / fmt サブコマンドを提供する。
//!
//! ARCHITECTURE.md(vow_cli 行): 言語処理ロジックは持たず、引数解釈・
//! ファイル IO・Diagnostic の散文整形だけを担い、検査・整形は
//! vow_check / vow_fmt / vow_syntax に委譲する。`build` / `test` は M7。
//!
//! 終了コード規約(M6 事前合意・全サブコマンド共通):
//! - `0` 成功(検査エラーなし / 整形済み)
//! - `1` 診断エラー検出(check)・未整形(fmt --check)・構文エラー(fmt)
//! - `2` 使用法エラー(引数不正・ファイル不在)

use std::path::Path;
use std::process::ExitCode;

// vow_emit は build / test(M7)で使う。M6 の依存グラフ(ARCHITECTURE.md)を保つ。
use vow_emit as _;

mod check;
mod cli;
mod fmt;
mod render;

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
