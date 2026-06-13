//! `vow` CLI の統合テスト。実バイナリ(`env!("CARGO_BIN_EXE_vow")`)を
//! プロセス起動し、stdout / stderr / 終了コードを検証する。
//!
//! - golden 部(契約本文は tests/cli/ の fixture 側):
//!   - `checks/<name>.vow` → `vow check` の散文(`<name>.check.txt`)と
//!     `vow check --json`(`<name>.check.json`)を snapshot 比較。
//!   - `fmt/<name>.input.vow`(+ `<name>.expected.vow`)→ 正規形 stdout 一致 /
//!     `--check` で未整形を exit 1 検出(差分は `<name>.fmtcheck.txt`)/
//!     正規形入力は `--check` exit 0。`.expected.vow` が無い入力は構文エラー扱いで
//!     整形せず Diagnostic を stderr に出して exit 1(`<name>.fmt.txt`)。
//! - 挙動部: 終了コード規約(0 / 1 / 2)・使用法エラー・`--write`・help / version。
//!
//! golden の再生成: `UPDATE_GOLDEN=1 cargo test -p vow_cli --test cli`
//! (golden の変更は人間レビュー必須 — ARCHITECTURE.md 不変条件 3)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cli_dir() -> PathBuf {
    repo_root().join("tests/cli")
}

fn update_golden() -> bool {
    std::env::var_os("UPDATE_GOLDEN").is_some()
}

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

/// リポジトリルートを cwd に `vow` を起動する。相対パス引数は span.file に
/// そのまま入るため、golden 内のパスがマシン非依存になる。
fn run_vow(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_vow"))
        .current_dir(repo_root())
        .args(args)
        .output()
        .expect("spawn vow");
    Run {
        stdout: String::from_utf8(output.stdout).expect("stdout is utf-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is utf-8"),
        code: output.status.code().expect("process exited with a code"),
    }
}

/// snapshot 比較(UPDATE_GOLDEN なら actual を書き出す)。差異は failures に積む。
fn expect_golden(path: &Path, actual: &str, failures: &mut Vec<String>) {
    if update_golden() {
        fs::write(path, actual).expect("write golden");
        return;
    }
    match fs::read_to_string(path) {
        Ok(expected) if expected == actual => {}
        Ok(expected) => failures.push(format!(
            "{}: differs\n--- expected ---\n{expected}--- actual ---\n{actual}",
            path.display()
        )),
        Err(e) => failures.push(format!("{}: missing golden ({e})", path.display())),
    }
}

/// `dir` 内で `suffix` で終わるファイル名から suffix を除いた名前を昇順で返す。
fn fixture_names(dir: &Path, suffix: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("dir entry").path();
            let name = path.file_name()?.to_str()?;
            name.strip_suffix(suffix).map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// golden: vow check(散文 / --json)
// ---------------------------------------------------------------------------

#[test]
fn check_golden() {
    let dir = cli_dir().join("checks");
    let names = fixture_names(&dir, ".vow");
    assert!(!names.is_empty(), "no check fixtures in {}", dir.display());

    let mut failures = Vec::new();
    for name in &names {
        let rel = format!("tests/cli/checks/{name}.vow");

        // 既定: 散文を stdout に。stderr は空。
        let prose = run_vow(&["check", &rel]);
        expect_golden(
            &dir.join(format!("{name}.check.txt")),
            &prose.stdout,
            &mut failures,
        );
        if !prose.stderr.is_empty() {
            failures.push(format!(
                "{name}: check prose leaked to stderr:\n{}",
                prose.stderr
            ));
        }

        // --json: Diagnostic[] を stdout に。stderr は空。
        let json = run_vow(&["check", "--json", &rel]);
        expect_golden(
            &dir.join(format!("{name}.check.json")),
            &json.stdout,
            &mut failures,
        );
        if !json.stderr.is_empty() {
            failures.push(format!(
                "{name}: check --json leaked to stderr:\n{}",
                json.stderr
            ));
        }

        // 終了コード: err_* は 1、ok_* は 0(golden_check と同じ命名規約)。
        let expected = if name.starts_with("err_") { 1 } else { 0 };
        for (label, run) in [("prose", &prose), ("json", &json)] {
            if run.code != expected {
                failures.push(format!(
                    "{name} ({label}): exit {} but expected {expected}",
                    run.code
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} check case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

// ---------------------------------------------------------------------------
// golden: vow fmt(整形 / --check / 構文エラー)
// ---------------------------------------------------------------------------

#[test]
fn fmt_golden() {
    let dir = cli_dir().join("fmt");
    let names = fixture_names(&dir, ".input.vow");
    assert!(!names.is_empty(), "no fmt fixtures in {}", dir.display());

    let mut failures = Vec::new();
    for name in &names {
        let input_rel = format!("tests/cli/fmt/{name}.input.vow");
        let expected_path = dir.join(format!("{name}.expected.vow"));

        if expected_path.exists() {
            // 整形ケース: 既定は正規形を stdout に(exit 0、stderr 空)。
            let fmt = run_vow(&["fmt", &input_rel]);
            expect_golden(&expected_path, &fmt.stdout, &mut failures);
            if fmt.code != 0 || !fmt.stderr.is_empty() {
                failures.push(format!(
                    "{name}: fmt(default) code={} stderr={:?}",
                    fmt.code, fmt.stderr
                ));
            }

            // --check 未整形: exit 1、差分を stderr に、stdout 空。
            let chk = run_vow(&["fmt", "--check", &input_rel]);
            expect_golden(
                &dir.join(format!("{name}.fmtcheck.txt")),
                &chk.stderr,
                &mut failures,
            );
            if chk.code != 1 || !chk.stdout.is_empty() {
                failures.push(format!(
                    "{name}: fmt --check(unformatted) code={} stdout={:?}",
                    chk.code, chk.stdout
                ));
            }

            // --check 正規形入力: exit 0、無出力。
            let ok = run_vow(&[
                "fmt",
                "--check",
                &format!("tests/cli/fmt/{name}.expected.vow"),
            ]);
            if ok.code != 0 || !ok.stdout.is_empty() || !ok.stderr.is_empty() {
                failures.push(format!(
                    "{name}: fmt --check(canonical) not clean: code={} stdout={:?} stderr={:?}",
                    ok.code, ok.stdout, ok.stderr
                ));
            }
        } else {
            // 構文エラーケース: 整形せず Diagnostic を stderr に、exit 1、stdout 空。
            let fmt = run_vow(&["fmt", &input_rel]);
            expect_golden(
                &dir.join(format!("{name}.fmt.txt")),
                &fmt.stderr,
                &mut failures,
            );
            if fmt.code != 1 || !fmt.stdout.is_empty() {
                failures.push(format!(
                    "{name}: fmt(syntax error) code={} stdout={:?}",
                    fmt.code, fmt.stdout
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} fmt case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

// ---------------------------------------------------------------------------
// 挙動: 終了コード・使用法エラー・--write・help / version
// ---------------------------------------------------------------------------

#[test]
fn check_exit_codes_and_json_shape() {
    // クリーンファイル: exit 0、散文は無出力、--json は "[]"。
    let ok = run_vow(&["check", "tests/cli/checks/ok_options.vow"]);
    assert_eq!(ok.code, 0);
    assert_eq!(ok.stdout, "");
    let ok_json = run_vow(&["check", "--json", "tests/cli/checks/ok_options.vow"]);
    assert_eq!(ok_json.code, 0);
    assert_eq!(ok_json.stdout, "[]\n");

    // エラーありファイル: exit 1。--json は構造化 Diagnostic[](パース可能)。
    let err = run_vow(&["check", "tests/cli/checks/err_effect.vow"]);
    assert_eq!(err.code, 1);
    assert!(err.stdout.contains("error[VOW-E3001]"));
    let err_json = run_vow(&["check", "--json", "tests/cli/checks/err_effect.vow"]);
    assert_eq!(err_json.code, 1);
    let parsed: serde_json::Value =
        serde_json::from_str(&err_json.stdout).expect("--json emits valid JSON");
    let arr = parsed.as_array().expect("Diagnostic[] is a JSON array");
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["code"], "VOW-E3001");
    assert_eq!(arr[0]["severity"], "error");
}

#[test]
fn fmt_write_rewrites_in_place() {
    // 書き換えテストは一時ディレクトリで(fixture を汚さない)。
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("write_me.vow");
    let source = fs::read_to_string(cli_dir().join("fmt/messy.input.vow")).expect("read fixture");
    let canonical =
        fs::read_to_string(cli_dir().join("fmt/messy.expected.vow")).expect("read fixture");
    fs::write(&tmp, &source).expect("seed temp file");

    let path = tmp.to_str().expect("utf-8 temp path");
    let write = run_vow(&["fmt", "--write", path]);
    assert_eq!(write.code, 0, "stderr={:?}", write.stderr);
    assert_eq!(write.stdout, "", "--write must not print to stdout");
    assert_eq!(fs::read_to_string(&tmp).expect("read back"), canonical);

    // 整形後は --check が通る(冪等)。
    let recheck = run_vow(&["fmt", "--check", path]);
    assert_eq!(recheck.code, 0);
}

#[test]
fn usage_errors_exit_2() {
    // 引数不正・未知サブコマンド・排他フラグ・ファイル不在はすべて exit 2。
    for args in [
        vec![],
        vec!["frobnicate", "x.vow"],
        vec!["check"],
        vec!["check", "a.vow", "b.vow"],
        vec!["check", "--bogus", "a.vow"],
        vec!["fmt"],
        vec!["fmt", "--check", "--write", "tests/cli/fmt/messy.input.vow"],
        vec!["build", "."],
    ] {
        let run = run_vow(&args);
        assert_eq!(run.code, 2, "args {args:?} should be a usage error");
        assert!(
            run.stdout.is_empty(),
            "usage errors must not write stdout: {args:?}"
        );
        assert!(
            !run.stderr.is_empty(),
            "usage errors must explain themselves: {args:?}"
        );
    }
}

#[test]
fn missing_file_exits_2() {
    for sub in [
        vec!["check", "tests/cli/checks/nope.vow"],
        vec!["fmt", "tests/cli/fmt/nope.vow"],
    ] {
        let run = run_vow(&sub);
        assert_eq!(run.code, 2, "{sub:?} on a missing file is a usage error");
        assert!(
            run.stderr.contains("cannot read"),
            "stderr={:?}",
            run.stderr
        );
    }
}

#[test]
fn help_and_version_exit_0() {
    let help = run_vow(&["--help"]);
    assert_eq!(help.code, 0);
    assert!(help.stdout.contains("USAGE:"));
    assert!(help.stderr.is_empty());

    let version = run_vow(&["--version"]);
    assert_eq!(version.code, 0);
    assert!(version.stdout.starts_with("vow "));
}
