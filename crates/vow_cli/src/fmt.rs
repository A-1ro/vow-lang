//! `vow fmt` のランナー。ファイル IO と vow_fmt への委譲、出力整形だけを行う。
//!
//! - 既定(`Stdout`): 正規形を stdout に出す(非破壊)。
//! - `--check`: 整形済みなら終了コード 0、未整形なら差分を stderr に出して 1。
//! - `--write`: 正規形でファイルを上書き(差分があるときのみ書く)。
//! - いずれのモードも、構文エラー入力は整形せず Diagnostic を stderr に出して 1
//!   (vow_fmt::format_source の方針: 壊れた入力を「それらしく」書き換えない)。
//!
//! 整形済みソース(stdout)と Diagnostic / 差分(stderr)を別ストリームに分け、
//! `vow fmt foo.vow > foo.fmt` でエラー表示が混ざらないようにする。

use std::path::Path;

use crate::cli::{FmtMode, UsageError};
use crate::render;

/// `vow fmt`。成功時は終了コード(0 / 1)、ファイル IO 失敗は使用法エラー。
pub fn run(file: &Path, mode: FmtMode) -> Result<u8, UsageError> {
    let source = crate::read_source(file)?;
    let name = file.to_string_lossy().into_owned();

    match vow_fmt::format_source(&source) {
        Ok(formatted) => match mode {
            FmtMode::Stdout => {
                print!("{formatted}");
                Ok(0)
            }
            FmtMode::Check => {
                if formatted == source {
                    Ok(0)
                } else {
                    eprint!("{}", render::format_diff(&name, &source, &formatted));
                    Ok(1)
                }
            }
            FmtMode::Write => {
                if formatted != source {
                    std::fs::write(file, &formatted)
                        .map_err(|e| UsageError(format!("cannot write {}: {e}", file.display())))?;
                }
                Ok(0)
            }
        },
        Err(errors) => {
            // 構文エラーは検査エラーと同じく終了コード 1(使用法エラー 2 ではない)。
            let diags = vow_check::syntax_diagnostics(&name, &errors);
            eprint!("{}", render::diagnostics(&source, &diags));
            Ok(1)
        }
    }
}
