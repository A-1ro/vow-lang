//! 引数解釈のみ(ARCHITECTURE.md: kei_cli は引数解釈・ファイル IO・
//! Diagnostic の散文整形だけを持ち、言語処理ロジックを持たない)。
//!
//! ここでは argv を [`Command`] に写すだけで、ファイル IO も検査もしない。
//! 解釈に失敗したら [`UsageError`](使用法エラー = 終了コード 2)を返す。

use std::path::PathBuf;

/// 解釈済みのサブコマンド。実行は各ランナー(check / fmt / build / test)が担う。
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// `kei check <file> [--json]`
    Check { file: PathBuf, json: bool },
    /// `kei fmt <file> [--check | --write]`
    Fmt { file: PathBuf, mode: FmtMode },
    /// `kei build <dir> [--out-dir <dir>] [--no-source-map]`
    Build {
        dir: PathBuf,
        /// 出力先。`None` のとき既定 `<dir>/dist/`(ランナーが解決する)。
        out_dir: Option<PathBuf>,
        /// source map を既定 on にするか。`--no-source-map` で `false`。
        source_map: bool,
    },
    /// `kei test [<dir>]`。省略時はカレントディレクトリ。
    Test { dir: PathBuf },
    /// `kei mcp`。MCP サーバーを stdio で起動する(引数なし)。
    Mcp,
    /// `kei help` / `--help` / `-h`。使い方を stdout に出して終了コード 0。
    Help,
    /// `kei version` / `--version` / `-V`。
    Version,
}

/// `kei fmt` の動作モード。既定は stdout 出力(非破壊)。
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
        "build" => parse_build(it),
        "test" => parse_test(it),
        "mcp" => parse_mcp(it),
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

fn parse_build(mut it: impl Iterator<Item = String>) -> Result<Command, UsageError> {
    let mut dir: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut source_map = true;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out-dir" => {
                if out_dir.is_some() {
                    return Err(UsageError::new("--out-dir given more than once"));
                }
                let val = it
                    .next()
                    .ok_or_else(|| UsageError::new("--out-dir requires a <dir> value"))?;
                out_dir = Some(PathBuf::from(val));
            }
            "--no-source-map" => source_map = false,
            "--help" | "-h" => return Ok(Command::Help),
            opt if is_option(opt) => {
                return Err(UsageError::new(format!(
                    "unknown option '{opt}' for 'build'"
                )));
            }
            _ => {
                if dir.is_some() {
                    return Err(UsageError::new("build takes exactly one <dir>"));
                }
                dir = Some(PathBuf::from(arg));
            }
        }
    }
    let dir = dir.ok_or_else(|| UsageError::new("build requires a <dir> argument"))?;
    Ok(Command::Build {
        dir,
        out_dir,
        source_map,
    })
}

fn parse_test(it: impl Iterator<Item = String>) -> Result<Command, UsageError> {
    let mut dir: Option<PathBuf> = None;
    for arg in it {
        match arg.as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            opt if is_option(opt) => {
                return Err(UsageError::new(format!(
                    "unknown option '{opt}' for 'test'"
                )));
            }
            _ => {
                if dir.is_some() {
                    return Err(UsageError::new("test takes at most one <dir>"));
                }
                dir = Some(PathBuf::from(arg));
            }
        }
    }
    Ok(Command::Test {
        dir: dir.unwrap_or_else(|| PathBuf::from(".")),
    })
}

/// `kei mcp` は引数を取らない(stdio で JSON-RPC を待つ)。余分な引数は使用法エラー。
fn parse_mcp(mut it: impl Iterator<Item = String>) -> Result<Command, UsageError> {
    match it.next() {
        None => Ok(Command::Mcp),
        Some(arg) if arg == "--help" || arg == "-h" => Ok(Command::Help),
        Some(other) => Err(UsageError::new(format!(
            "mcp takes no arguments (got '{other}')"
        ))),
    }
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
kei — the Kei toolchain

USAGE:
    kei check <file> [--json]    意味検査(既定は散文 Diagnostic、--json で Diagnostic[])
    kei fmt <file> [--check | --write]
                                 正規形整形(既定は stdout、--check は検証、--write は上書き)
    kei build <dir> [--out-dir <dir>] [--no-source-map]
                                 <dir> 配下の全 .kei を検査し TS + source map を出力
                                 (既定の出力先は <dir>/dist/。1 件でもエラーなら何も書かない)
    kei test [<dir>]             dev ビルド(契約 on)後、プロジェクトの `npm test` を起動
    kei mcp                      MCP サーバーを stdio で起動(spec/examples を引く取説サーバー)
    kei help | --help | -h       この使い方を表示
    kei version | --version | -V バージョンを表示

EXIT CODES:
    0  成功(検査エラーなし / 整形済み / ビルド成功 / テスト全件パス)
    1  診断エラー検出(check / build)・未整形(fmt --check)・構文エラー(fmt)・テスト失敗(test)
    2  使用法エラー(引数不正・ファイル/ディレクトリ不在)
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
            parse_args(&["check", "a.kei"]),
            Ok(Command::Check {
                file: PathBuf::from("a.kei"),
                json: false,
            })
        );
    }

    #[test]
    fn check_json_flag_either_order() {
        let expected = Ok(Command::Check {
            file: PathBuf::from("a.kei"),
            json: true,
        });
        assert_eq!(parse_args(&["check", "a.kei", "--json"]), expected);
        assert_eq!(parse_args(&["check", "--json", "a.kei"]), expected);
    }

    #[test]
    fn fmt_modes() {
        assert_eq!(
            parse_args(&["fmt", "a.kei"]),
            Ok(Command::Fmt {
                file: PathBuf::from("a.kei"),
                mode: FmtMode::Stdout,
            })
        );
        assert_eq!(
            parse_args(&["fmt", "a.kei", "--check"]),
            Ok(Command::Fmt {
                file: PathBuf::from("a.kei"),
                mode: FmtMode::Check,
            })
        );
        assert_eq!(
            parse_args(&["fmt", "--write", "a.kei"]),
            Ok(Command::Fmt {
                file: PathBuf::from("a.kei"),
                mode: FmtMode::Write,
            })
        );
    }

    #[test]
    fn build_and_test() {
        assert_eq!(
            parse_args(&["build", "src"]),
            Ok(Command::Build {
                dir: PathBuf::from("src"),
                out_dir: None,
                source_map: true,
            })
        );
        assert_eq!(
            parse_args(&["build", "src", "--out-dir", "out", "--no-source-map"]),
            Ok(Command::Build {
                dir: PathBuf::from("src"),
                out_dir: Some(PathBuf::from("out")),
                source_map: false,
            })
        );
        assert_eq!(
            parse_args(&["test", "proj"]),
            Ok(Command::Test {
                dir: PathBuf::from("proj"),
            })
        );
        assert_eq!(
            parse_args(&["test"]),
            Ok(Command::Test {
                dir: PathBuf::from("."),
            })
        );
    }

    #[test]
    fn mcp_takes_no_args() {
        assert_eq!(parse_args(&["mcp"]), Ok(Command::Mcp));
        assert_eq!(parse_args(&["mcp", "--help"]), Ok(Command::Help));
        assert!(parse_args(&["mcp", "foo"]).is_err());
        assert!(parse_args(&["mcp", "--bogus"]).is_err());
    }

    #[test]
    fn usage_errors() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["check"]).is_err());
        assert!(parse_args(&["fmt"]).is_err());
        assert!(parse_args(&["check", "a.kei", "b.kei"]).is_err());
        assert!(parse_args(&["check", "--bogus", "a.kei"]).is_err());
        assert!(parse_args(&["fmt", "a.kei", "--check", "--write"]).is_err());
        assert!(parse_args(&["frobnicate", "a.kei"]).is_err());
        assert!(parse_args(&["build"]).is_err());
        assert!(parse_args(&["build", "a", "b"]).is_err());
        assert!(parse_args(&["build", "src", "--out-dir"]).is_err());
        assert!(parse_args(&["build", "src", "--bogus"]).is_err());
        assert!(parse_args(&["test", "a", "b"]).is_err());
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
