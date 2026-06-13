//! 引数解釈のみ(ARCHITECTURE.md: vow_cli は引数解釈・ファイル IO・
//! Diagnostic の散文整形だけを持ち、言語処理ロジックを持たない)。
//!
//! ここでは argv を [`Command`] に写すだけで、ファイル IO も検査もしない。
//! 解釈に失敗したら [`UsageError`](使用法エラー = 終了コード 2)を返す。

use std::path::PathBuf;

/// 解釈済みのサブコマンド。実行は各ランナー(check / fmt)が担う。
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// `vow check <file> [--json]`
    Check { file: PathBuf, json: bool },
    /// `vow fmt <file> [--check | --write]`
    Fmt { file: PathBuf, mode: FmtMode },
    /// `vow help` / `--help` / `-h`。使い方を stdout に出して終了コード 0。
    Help,
    /// `vow version` / `--version` / `-V`。
    Version,
}

/// `vow fmt` の動作モード。既定は stdout 出力(非破壊)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmtMode {
    /// 整形結果を stdout に出す(既定・非破壊)。
    Stdout,
    /// 整形済みかを検証する。未整形なら差分を出して終了コード 1。
    Check,
    /// ファイルを整形結果で上書きする。
    Write,
}

/// 引数解釈の失敗。終了コード 2(使用法エラー)に対応する。
#[derive(Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

impl UsageError {
    fn new(msg: impl Into<String>) -> Self {
        UsageError(msg.into())
    }
}

/// プログラム名を除いた引数列を [`Command`] に解釈する。
pub fn parse(args: Vec<String>) -> Result<Command, UsageError> {
    let mut it = args.into_iter();
    let Some(sub) = it.next() else {
        return Err(UsageError::new("no subcommand given"));
    };
    match sub.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "version" | "--version" | "-V" => Ok(Command::Version),
        "check" => parse_check(it),
        "fmt" => parse_fmt(it),
        "build" | "test" => Err(UsageError::new(format!(
            "subcommand '{sub}' is not implemented yet (planned for M7)"
        ))),
        other => Err(UsageError::new(format!("unknown subcommand '{other}'"))),
    }
}

fn parse_check(it: impl Iterator<Item = String>) -> Result<Command, UsageError> {
    let mut file: Option<PathBuf> = None;
    let mut json = false;
    for arg in it {
        match arg.as_str() {
            "--json" => {
                if json {
                    return Err(UsageError::new("--json given more than once"));
                }
                json = true;
            }
            "--help" | "-h" => return Ok(Command::Help),
            opt if is_option(opt) => {
                return Err(UsageError::new(format!(
                    "unknown option '{opt}' for 'check'"
                )));
            }
            _ => {
                if file.is_some() {
                    return Err(UsageError::new("check takes exactly one <file>"));
                }
                file = Some(PathBuf::from(arg));
            }
        }
    }
    let file = file.ok_or_else(|| UsageError::new("check requires a <file> argument"))?;
    Ok(Command::Check { file, json })
}

fn parse_fmt(it: impl Iterator<Item = String>) -> Result<Command, UsageError> {
    let mut file: Option<PathBuf> = None;
    let mut mode: Option<FmtMode> = None;
    for arg in it {
        match arg.as_str() {
            "--check" => set_mode(&mut mode, FmtMode::Check)?,
            "--write" => set_mode(&mut mode, FmtMode::Write)?,
            "--help" | "-h" => return Ok(Command::Help),
            opt if is_option(opt) => {
                return Err(UsageError::new(format!("unknown option '{opt}' for 'fmt'")));
            }
            _ => {
                if file.is_some() {
                    return Err(UsageError::new("fmt takes exactly one <file>"));
                }
                file = Some(PathBuf::from(arg));
            }
        }
    }
    let file = file.ok_or_else(|| UsageError::new("fmt requires a <file> argument"))?;
    Ok(Command::Fmt {
        file,
        mode: mode.unwrap_or(FmtMode::Stdout),
    })
}

/// `--check` と `--write` は排他。既に別モードが立っていれば使用法エラー。
fn set_mode(slot: &mut Option<FmtMode>, mode: FmtMode) -> Result<(), UsageError> {
    match slot {
        Some(existing) if *existing != mode => Err(UsageError::new(
            "fmt: --check and --write cannot be combined",
        )),
        _ => {
            *slot = Some(mode);
            Ok(())
        }
    }
}

/// `-` 単体(stdin 慣習)はオプション扱いしない。それ以外の `-` 始まりはオプション。
fn is_option(arg: &str) -> bool {
    arg.starts_with('-') && arg != "-"
}

/// `--help` / エラー時に表示する使い方。
pub const USAGE: &str = "\
vow — the Vow toolchain

USAGE:
    vow check <file> [--json]    意味検査(既定は散文 Diagnostic、--json で Diagnostic[])
    vow fmt <file> [--check | --write]
                                 正規形整形(既定は stdout、--check は検証、--write は上書き)
    vow help | --help | -h       この使い方を表示
    vow version | --version | -V バージョンを表示

EXIT CODES:
    0  成功(検査エラーなし / 整形済み)
    1  診断エラー検出(check)・未整形(fmt --check)・構文エラー(fmt)
    2  使用法エラー(引数不正・ファイル不在)
";

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Command, UsageError> {
        parse(args.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn check_plain() {
        assert_eq!(
            parse_args(&["check", "a.vow"]),
            Ok(Command::Check {
                file: PathBuf::from("a.vow"),
                json: false,
            })
        );
    }

    #[test]
    fn check_json_flag_either_order() {
        let expected = Ok(Command::Check {
            file: PathBuf::from("a.vow"),
            json: true,
        });
        assert_eq!(parse_args(&["check", "a.vow", "--json"]), expected);
        assert_eq!(parse_args(&["check", "--json", "a.vow"]), expected);
    }

    #[test]
    fn fmt_modes() {
        assert_eq!(
            parse_args(&["fmt", "a.vow"]),
            Ok(Command::Fmt {
                file: PathBuf::from("a.vow"),
                mode: FmtMode::Stdout,
            })
        );
        assert_eq!(
            parse_args(&["fmt", "a.vow", "--check"]),
            Ok(Command::Fmt {
                file: PathBuf::from("a.vow"),
                mode: FmtMode::Check,
            })
        );
        assert_eq!(
            parse_args(&["fmt", "--write", "a.vow"]),
            Ok(Command::Fmt {
                file: PathBuf::from("a.vow"),
                mode: FmtMode::Write,
            })
        );
    }

    #[test]
    fn usage_errors() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["check"]).is_err());
        assert!(parse_args(&["fmt"]).is_err());
        assert!(parse_args(&["check", "a.vow", "b.vow"]).is_err());
        assert!(parse_args(&["check", "--bogus", "a.vow"]).is_err());
        assert!(parse_args(&["fmt", "a.vow", "--check", "--write"]).is_err());
        assert!(parse_args(&["frobnicate", "a.vow"]).is_err());
        assert!(parse_args(&["build", "."]).is_err());
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse_args(&["--help"]), Ok(Command::Help));
        assert_eq!(parse_args(&["-h"]), Ok(Command::Help));
        assert_eq!(parse_args(&["help"]), Ok(Command::Help));
        assert_eq!(parse_args(&["--version"]), Ok(Command::Version));
        assert_eq!(parse_args(&["version"]), Ok(Command::Version));
    }
}
