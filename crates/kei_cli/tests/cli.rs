//! `kei` CLI の統合テスト。実バイナリ(`env!("CARGO_BIN_EXE_kei")`)を
//! プロセス起動し、stdout / stderr / 終了コードを検証する。
//!
//! - golden 部(契約本文は tests/cli/ の fixture 側):
//!   - `checks/<name>.kei` → `kei check` の散文(`<name>.check.txt`)と
//!     `kei check --json`(`<name>.check.json`)を snapshot 比較。
//!   - `fmt/<name>.input.kei`(+ `<name>.expected.kei`)→ 正規形 stdout 一致 /
//!     `--check` で未整形を exit 1 検出(差分は `<name>.fmtcheck.txt`)/
//!     正規形入力は `--check` exit 0。`.expected.kei` が無い入力は構文エラー扱いで
//!     整形せず Diagnostic を stderr に出して exit 1(`<name>.fmt.txt`)。
//! - 挙動部: 終了コード規約(0 / 1 / 2)・使用法エラー・`--write`・help / version。
//!
//! golden の再生成: `UPDATE_GOLDEN=1 cargo test -p kei_cli --test cli`
//! (golden の変更は人間レビュー必須 — ARCHITECTURE.md 不変条件 3)

use std::collections::BTreeMap;
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

/// リポジトリルートを cwd に `kei` を起動する。相対パス引数は span.file に
/// そのまま入るため、golden 内のパスがマシン非依存になる。
fn run_kei(args: &[&str]) -> Run {
    run_kei_env(args, &[])
}

/// `run_kei` に環境変数の上書きを足した版(`kei test` が子へ env を伝播する確認用)。
fn run_kei_env(args: &[&str], envs: &[(&str, &str)]) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kei"));
    cmd.current_dir(repo_root()).args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd.output().expect("spawn kei");
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
// golden: kei check(散文 / --json)
// ---------------------------------------------------------------------------

#[test]
fn check_golden() {
    let dir = cli_dir().join("checks");
    let names = fixture_names(&dir, ".kei");
    assert!(!names.is_empty(), "no check fixtures in {}", dir.display());

    let mut failures = Vec::new();
    for name in &names {
        let rel = format!("tests/cli/checks/{name}.kei");

        // 既定: 散文を stdout に。stderr は空。
        let prose = run_kei(&["check", &rel]);
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
        let json = run_kei(&["check", "--json", &rel]);
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
// golden: kei check --strict-extern(M16 / #44)
// ---------------------------------------------------------------------------

/// strict-extern は **オプトイン**。同じ #22 再現コードが、既定の `kei check` では
/// 通り(extern 未宣言の外部呼び出しは opaque)、`--strict-extern` でのみ警告される
/// (extern 宣言の追加を促す)ことを対で固定する。
#[test]
fn strict_extern_golden() {
    let dir = cli_dir().join("checks");
    let rel = "tests/cli/checks/strict_extern.kei";
    let mut failures = Vec::new();

    // 既定: 未宣言の外部呼び出しは通る(警告なし・exit 0)。
    let default = run_kei(&["check", rel]);
    if !default.stdout.is_empty() || default.code != 0 {
        failures.push(format!(
            "default check should be clean: code={} stdout={:?}",
            default.code, default.stdout
        ));
    }

    // strict: KEI-E3004 警告(stage a は warning なので exit は 0)。散文 / --json を固定。
    let strict = run_kei(&["check", "--strict-extern", rel]);
    expect_golden(
        &dir.join("strict_extern.strict.txt"),
        &strict.stdout,
        &mut failures,
    );
    if strict.code != 0 {
        failures.push(format!(
            "strict check warning must not change exit code (stage a): code={}",
            strict.code
        ));
    }

    let strict_json = run_kei(&["check", "--strict-extern", "--json", rel]);
    expect_golden(
        &dir.join("strict_extern.strict.json"),
        &strict_json.stdout,
        &mut failures,
    );
    // #22 再現: warning に KEI-E3004 と extern 宣言の fix が載る。
    let parsed: serde_json::Value =
        serde_json::from_str(&strict_json.stdout).expect("--json emits valid JSON");
    let diags = parsed["diagnostics"]
        .as_array()
        .expect("diagnostics is an array");
    if !diags.iter().any(|d| {
        d["code"] == "KEI-E3004"
            && d["severity"] == "warning"
            && !d["fixes"].as_array().map(|f| f.is_empty()).unwrap_or(true)
    }) {
        failures.push(format!(
            "strict --json must carry a KEI-E3004 warning with a fix: {strict_json:?}",
            strict_json = strict_json.stdout
        ));
    }

    assert!(
        failures.is_empty(),
        "{} strict-extern case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

// ---------------------------------------------------------------------------
// golden: kei check --generative(M15 / #26)
// ---------------------------------------------------------------------------

/// 契約ベース PBT。正しい実装は ensures が generative へ格上げされ(全生成入力で反例ゼロ)、
/// わざと壊した実装(`available - 2`)は最小化された反例付きで KEI-E4005 を出す。
/// オラクルは契約のみ・シードは入力のみ(捏造不能性)を、出力で固定する。
#[test]
fn generative_golden() {
    let dir = cli_dir().join("checks");
    let mut failures = Vec::new();

    // 正しい実装: ensures が generative。exit 0。
    let ok_rel = "tests/cli/checks/gen_decrement.kei";
    let ok = run_kei(&["check", "--generative", ok_rel]);
    expect_golden(
        &dir.join("gen_decrement.gen.txt"),
        &ok.stdout,
        &mut failures,
    );
    if ok.code != 0 {
        failures.push(format!(
            "generative(correct) should exit 0, got {}",
            ok.code
        ));
    }
    let ok_json = run_kei(&["check", "--generative", "--json", ok_rel]);
    let parsed: serde_json::Value =
        serde_json::from_str(&ok_json.stdout).expect("--json emits valid JSON");
    let has_generative = parsed["contracts"]
        .as_array()
        .map(|cs| cs.iter().any(|c| c["verification"] == "generative"))
        .unwrap_or(false);
    if !has_generative {
        failures.push(format!(
            "correct impl must report a 'generative' contract:\n{}",
            ok_json.stdout
        ));
    }

    // 壊れた実装: 最小化反例付き KEI-E4005、exit 1。
    let bad_rel = "tests/cli/checks/gen_decrement_broken.kei";
    let bad = run_kei(&["check", "--generative", bad_rel]);
    expect_golden(
        &dir.join("gen_decrement_broken.gen.txt"),
        &bad.stdout,
        &mut failures,
    );
    if bad.code != 1 {
        failures.push(format!(
            "generative(broken) should exit 1, got {}",
            bad.code
        ));
    }
    if !bad.stdout.contains("KEI-E4005") || !bad.stdout.contains("available = 1") {
        failures.push(format!(
            "broken impl must report KEI-E4005 with minimized input 'available = 1':\n{}",
            bad.stdout
        ));
    }

    // 既定(--generative なし)では壊れた実装も静的には通る(PBT は opt-in)。
    let bad_default = run_kei(&["check", bad_rel]);
    if bad_default.code != 0 {
        failures.push(format!(
            "without --generative the broken impl is statically clean, got exit {}",
            bad_default.code
        ));
    }

    assert!(
        failures.is_empty(),
        "{} generative case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// シード注入(M15 段階2): `<stem>.seeds` を --generative で自動検証する。入力のみの
/// シードを requires に照らし、違反シード(available: 0)を KEI-E4007 で弾く。期待値を
/// 書く構文が無い(seeds は入力のみ)ことは pbt の unit test で固定。
#[test]
fn seed_injection_golden() {
    let dir = cli_dir().join("checks");
    let rel = "tests/cli/checks/seed_decrement.kei";
    let mut failures = Vec::new();

    let run = run_kei(&["check", "--generative", rel]);
    expect_golden(
        &dir.join("seed_decrement.gen.txt"),
        &run.stdout,
        &mut failures,
    );
    if run.code != 1 {
        failures.push(format!(
            "a requires-violating seed must make exit 1, got {}",
            run.code
        ));
    }
    if !run.stdout.contains("KEI-E4007") || !run.stdout.contains("seed_decrement.seeds") {
        failures.push(format!(
            "seed validation must point at the .seeds file with KEI-E4007:\n{}",
            run.stdout
        ));
    }

    // 既定(--generative なし)ではシードを読まない(opt-in)。
    let default = run_kei(&["check", rel]);
    if default.code != 0 || default.stdout.contains("KEI-E4007") {
        failures.push(format!(
            "without --generative, seeds are not validated: code={} out={:?}",
            default.code, default.stdout
        ));
    }

    assert!(
        failures.is_empty(),
        "{} seed case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// シード注入の検証レベル整合(PR #50 第4レビュー P2): 生成器の固定ドメインでは反例ゼロでも、
/// シードがドメイン外(`x = 7`)で `ensures` を破ったら、その契約は `generative` ではなく
/// `runtime` を報告する。`--json` が「generative」と KEI-E4005 を同時に主張しないことを固定。
#[test]
fn seed_failure_downgrades_contract_to_runtime() {
    let rel = "tests/cli/checks/seed_downgrade.kei";
    let run = run_kei(&["check", "--generative", "--json", rel]);
    let parsed: serde_json::Value =
        serde_json::from_str(&run.stdout).expect("--json emits valid JSON");

    let diags = parsed["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        diags.iter().any(|d| d["code"] == "KEI-E4005"),
        "seed must produce a KEI-E4005 counterexample: {}",
        run.stdout
    );
    let contracts = parsed["contracts"].as_array().expect("contracts array");
    // シードが破った ensures が generative のまま残っていないこと(矛盾の防止)。
    assert!(
        contracts
            .iter()
            .all(|c| !(c["kind"] == "ensures" && c["verification"] == "generative")),
        "a seed-broken contract must not stay 'generative': {}",
        run.stdout
    );
}

// ---------------------------------------------------------------------------
// golden: kei fmt(整形 / --check / 構文エラー)
// ---------------------------------------------------------------------------

#[test]
fn fmt_golden() {
    let dir = cli_dir().join("fmt");
    let names = fixture_names(&dir, ".input.kei");
    assert!(!names.is_empty(), "no fmt fixtures in {}", dir.display());

    let mut failures = Vec::new();
    for name in &names {
        let input_rel = format!("tests/cli/fmt/{name}.input.kei");
        let expected_path = dir.join(format!("{name}.expected.kei"));

        if expected_path.exists() {
            // 整形ケース: 既定は正規形を stdout に(exit 0、stderr 空)。
            let fmt = run_kei(&["fmt", &input_rel]);
            expect_golden(&expected_path, &fmt.stdout, &mut failures);
            if fmt.code != 0 || !fmt.stderr.is_empty() {
                failures.push(format!(
                    "{name}: fmt(default) code={} stderr={:?}",
                    fmt.code, fmt.stderr
                ));
            }

            // --check 未整形: exit 1、差分を stderr に、stdout 空。
            let chk = run_kei(&["fmt", "--check", &input_rel]);
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
            let ok = run_kei(&[
                "fmt",
                "--check",
                &format!("tests/cli/fmt/{name}.expected.kei"),
            ]);
            if ok.code != 0 || !ok.stdout.is_empty() || !ok.stderr.is_empty() {
                failures.push(format!(
                    "{name}: fmt --check(canonical) not clean: code={} stdout={:?} stderr={:?}",
                    ok.code, ok.stdout, ok.stderr
                ));
            }
        } else {
            // 構文エラーケース: 整形せず Diagnostic を stderr に、exit 1、stdout 空。
            let fmt = run_kei(&["fmt", &input_rel]);
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
    // クリーンで契約なしのファイル: exit 0、散文は無出力、--json は空レポート。
    let ok = run_kei(&["check", "tests/cli/checks/ok_options.kei"]);
    assert_eq!(ok.code, 0);
    assert_eq!(ok.stdout, "");
    let ok_json = run_kei(&["check", "--json", "tests/cli/checks/ok_options.kei"]);
    assert_eq!(ok_json.code, 0);
    let ok_parsed: serde_json::Value =
        serde_json::from_str(&ok_json.stdout).expect("--json emits valid JSON");
    assert_eq!(ok_parsed["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(ok_parsed["contracts"].as_array().unwrap().len(), 0);

    // エラーありファイル: exit 1。--json は CheckReport(diagnostics に Diagnostic[])。
    let err = run_kei(&["check", "tests/cli/checks/err_effect.kei"]);
    assert_eq!(err.code, 1);
    assert!(err.stdout.contains("error[KEI-E3001]"));
    let err_json = run_kei(&["check", "--json", "tests/cli/checks/err_effect.kei"]);
    assert_eq!(err_json.code, 1);
    let parsed: serde_json::Value =
        serde_json::from_str(&err_json.stdout).expect("--json emits valid JSON");
    let arr = parsed["diagnostics"]
        .as_array()
        .expect("diagnostics is a JSON array");
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["code"], "KEI-E3001");
    assert_eq!(arr[0]["severity"], "error");

    // 契約付きファイル: --json の contracts に検証レベルが載る。
    let c_json = run_kei(&["check", "--json", "tests/cli/checks/ok_contract.kei"]);
    assert_eq!(c_json.code, 0);
    let c_parsed: serde_json::Value =
        serde_json::from_str(&c_json.stdout).expect("--json emits valid JSON");
    let contracts = c_parsed["contracts"]
        .as_array()
        .expect("contracts is a JSON array");
    assert!(!contracts.is_empty());
    assert!(contracts
        .iter()
        .all(|c| c["verification"].is_string() && c["expr"].is_string()));
}

#[test]
fn fmt_write_rewrites_in_place() {
    // 書き換えテストは一時ディレクトリで(fixture を汚さない)。
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("write_me.kei");
    let source = fs::read_to_string(cli_dir().join("fmt/messy.input.kei")).expect("read fixture");
    let canonical =
        fs::read_to_string(cli_dir().join("fmt/messy.expected.kei")).expect("read fixture");
    fs::write(&tmp, &source).expect("seed temp file");

    let path = tmp.to_str().expect("utf-8 temp path");
    let write = run_kei(&["fmt", "--write", path]);
    assert_eq!(write.code, 0, "stderr={:?}", write.stderr);
    assert_eq!(write.stdout, "", "--write must not print to stdout");
    assert_eq!(fs::read_to_string(&tmp).expect("read back"), canonical);

    // 整形後は --check が通る(冪等)。
    let recheck = run_kei(&["fmt", "--check", path]);
    assert_eq!(recheck.code, 0);
}

#[test]
fn usage_errors_exit_2() {
    // 引数不正・未知サブコマンド・排他フラグ・ファイル不在はすべて exit 2。
    for args in [
        vec![],
        vec!["frobnicate", "x.kei"],
        vec!["check"],
        vec!["check", "a.kei", "b.kei"],
        vec!["check", "--bogus", "a.kei"],
        vec!["fmt"],
        vec!["fmt", "--check", "--write", "tests/cli/fmt/messy.input.kei"],
        vec!["build"],
        vec!["build", "a", "b"],
        vec!["build", "src", "--out-dir"],
        vec!["test", "a", "b"],
    ] {
        let run = run_kei(&args);
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
        vec!["check", "tests/cli/checks/nope.kei"],
        vec!["fmt", "tests/cli/fmt/nope.kei"],
    ] {
        let run = run_kei(&sub);
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
    let help = run_kei(&["--help"]);
    assert_eq!(help.code, 0);
    assert!(help.stdout.contains("USAGE:"));
    assert!(help.stderr.is_empty());

    let version = run_kei(&["--version"]);
    assert_eq!(version.code, 0);
    assert!(version.stdout.starts_with("kei "));
}

// ---------------------------------------------------------------------------
// golden: kei build(出力ツリー)/ 挙動: all-or-nothing・--no-source-map
// ---------------------------------------------------------------------------

/// ディレクトリツリーを (リポジトリ非依存の相対パス -> 内容) に集める。
fn collect_tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(base, &path, map);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .replace('\\', "/");
                map.insert(rel, fs::read_to_string(&path).expect("read file"));
            }
        }
    }
    let mut map = BTreeMap::new();
    if root.is_dir() {
        walk(root, root, &mut map);
    }
    map
}

/// 一時出力先を作り直して返す(前回の生成物を残さない)。
fn fresh_out(name: &str) -> PathBuf {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if out.exists() {
        fs::remove_dir_all(&out).expect("clean tmp out dir");
    }
    out
}

#[test]
fn build_golden_tree() {
    let out = fresh_out("build_app");
    let out_str = out.to_str().expect("utf-8 tmp path");
    let run = run_kei(&["build", "tests/cli/projects/app", "--out-dir", out_str]);
    assert_eq!(run.code, 0, "build failed: stderr={:?}", run.stderr);
    assert_eq!(run.stdout, "", "build must not write stdout");
    assert!(
        run.stderr.contains("wrote 2 module(s)"),
        "build summary missing: stderr={:?}",
        run.stderr
    );

    let actual = collect_tree(&out);
    let expected_dir = cli_dir().join("projects/app/expected");

    if update_golden() {
        if expected_dir.exists() {
            fs::remove_dir_all(&expected_dir).expect("clean expected");
        }
        for (rel, content) in &actual {
            let dest = expected_dir.join(rel);
            fs::create_dir_all(dest.parent().expect("golden parent")).expect("mkdir golden");
            fs::write(&dest, content).expect("write golden");
        }
        return;
    }

    let expected = collect_tree(&expected_dir);
    assert!(
        !expected.is_empty(),
        "no golden under {} (regenerate with UPDATE_GOLDEN=1)",
        expected_dir.display()
    );
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "build output tree differs from golden (paths)"
    );
    for (rel, content) in &expected {
        assert_eq!(
            actual.get(rel),
            Some(content),
            "{rel}: build output differs from golden"
        );
    }
}

#[test]
fn build_all_or_nothing_writes_nothing_on_error() {
    let out = fresh_out("build_broken");
    let out_str = out.to_str().expect("utf-8 tmp path");
    let run = run_kei(&["build", "tests/cli/projects/broken", "--out-dir", out_str]);
    assert_eq!(run.code, 1, "broken build must exit 1");
    assert!(
        run.stdout.is_empty(),
        "stdout must stay empty: {:?}",
        run.stdout
    );
    assert!(
        run.stderr.contains("error[KEI-E3001]"),
        "diagnostic missing: stderr={:?}",
        run.stderr
    );
    assert!(
        run.stderr.contains("no output written"),
        "all-or-nothing note missing: stderr={:?}",
        run.stderr
    );
    let tree = collect_tree(&out);
    assert!(
        tree.is_empty(),
        "all-or-nothing: nothing must be written, got {:?}",
        tree.keys().collect::<Vec<_>>()
    );
}

#[test]
fn build_missing_dir_exits_2() {
    let run = run_kei(&["build", "tests/cli/projects/does-not-exist"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("is not a directory"),
        "stderr={:?}",
        run.stderr
    );
}

#[test]
fn build_no_source_map_omits_maps_and_comment() {
    let out = fresh_out("build_app_nomap");
    let out_str = out.to_str().expect("utf-8 tmp path");
    let run = run_kei(&[
        "build",
        "tests/cli/projects/app",
        "--out-dir",
        out_str,
        "--no-source-map",
    ]);
    assert_eq!(run.code, 0, "stderr={:?}", run.stderr);
    let tree = collect_tree(&out);
    assert!(
        tree.keys().all(|k| !k.ends_with(".map")),
        "no .map files expected: {:?}",
        tree.keys().collect::<Vec<_>>()
    );
    assert!(tree.contains_key("app/math.ts"), "ts still emitted");
    for (k, v) in &tree {
        assert!(
            !v.contains("sourceMappingURL"),
            "{k} still references a source map"
        );
    }
}

// ---------------------------------------------------------------------------
// 挙動: kei test(dev ビルド → npm test 委譲・契約 on)。Node が必要。
// ---------------------------------------------------------------------------

fn has_npm() -> bool {
    Command::new("npm")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// プロジェクトディレクトリで npm を実行し、失敗したら出力ごと panic する。
fn npm(args: &[&str], cwd: &Path) {
    let out = Command::new("npm")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("spawn npm {}: {e}", args.join(" ")));
    assert!(
        out.status.success(),
        "npm {} failed in {}:\n--- stdout ---\n{}\n--- stderr ---\n{}",
        args.join(" "),
        cwd.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// @kei/runtime をビルド(dist が無ければ install + build)。e2e と同じ前提。
fn ensure_runtime_built(root: &Path) {
    if root.join("runtime/dist/index.js").is_file() {
        return;
    }
    let runtime = root.join("runtime");
    npm(&["install", "--no-audit", "--no-fund"], &runtime);
    npm(&["run", "build"], &runtime);
}

#[test]
fn kei_test_builds_then_runs_contracts() {
    if !has_npm() {
        eprintln!("skipping kei_test_builds_then_runs_contracts: npm not found");
        return;
    }
    let root = repo_root().canonicalize().expect("repo root");
    ensure_runtime_built(&root);

    let project = root.join("tests/cli/projects/app");
    let dist = project.join("dist");
    if dist.exists() {
        fs::remove_dir_all(&dist).expect("clean project dist");
    }
    npm(&["install", "--no-audit", "--no-fund"], &project);

    // (a) 既定: dev ビルド(契約 on)→ vitest 全件パス → exit 0。
    let pass = run_kei(&["test", "tests/cli/projects/app"]);
    assert_eq!(
        pass.code, 0,
        "kei test should pass:\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pass.stdout, pass.stderr
    );

    // (b) 直前にビルドした dist が tsc --strict --noEmit でエラーゼロ(goal 条件 3)。
    let tsc = Command::new("npx")
        .args(["tsc", "--strict", "--noEmit"])
        .current_dir(&project)
        .output()
        .expect("spawn tsc");
    assert!(
        tsc.status.success(),
        "tsc --strict --noEmit failed:\n{}\n{}",
        String::from_utf8_lossy(&tsc.stdout),
        String::from_utf8_lossy(&tsc.stderr),
    );

    // (c) requires 違反を捕捉しないテストは失敗 → npm test 非ゼロ → kei test 非ゼロ
    //     (goal 条件 4)。env が子(npm→vitest→node)まで伝播することも確認する。
    let fail = run_kei_env(
        &["test", "tests/cli/projects/app"],
        &[("KEI_EXPECT_VIOLATION", "uncaught")],
    );
    assert_ne!(
        fail.code, 0,
        "a requires violation must make kei test exit non-zero"
    );
    let combined = format!("{}{}", fail.stdout, fail.stderr);
    assert!(
        combined.contains("KeiContractViolation"),
        "the contract violation must surface KeiContractViolation:\n{combined}"
    );
}

// ---------------------------------------------------------------------------
// M20: import 境界の型解決(#55)
// ---------------------------------------------------------------------------

/// `kei check <file>` がファイルの `module` 宣言から project root を逆算し、
/// `import` 先の record を解決して **存在しないフィールド** を検出する。
/// 正常系は同じ仕組みで素通りすることも対で固定する。
#[test]
fn typed_imports_consumer_good_is_clean() {
    let run = run_kei(&[
        "check",
        "tests/cli/projects/typed_imports/consumer_good.kei",
    ]);
    assert_eq!(
        run.code, 0,
        "valid import of a typed record should be clean: code={} stdout={:?} stderr={:?}",
        run.code, run.stdout, run.stderr
    );
}

#[test]
fn typed_imports_consumer_bad_detects_unknown_field() {
    let run = run_kei(&["check", "tests/cli/projects/typed_imports/consumer_bad.kei"]);
    assert_eq!(
        run.code, 1,
        "unknown field on imported record must exit 1: stderr={:?}",
        run.stderr
    );
    assert!(
        run.stdout.contains("KEI-E2002"),
        "expected KEI-E2002 on imported record field access: stdout={:?}",
        run.stdout
    );
    assert!(
        run.stdout.contains("nonexistentField"),
        "diagnostic should name the missing field: stdout={:?}",
        run.stdout
    );
}
