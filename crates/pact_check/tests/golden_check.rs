//! tests/golden/check/ の golden test ランナー(契約本文は fixture 側)。
//!
//! - `ok_*.pact` … 構文エラーゼロでパースでき、`check_module` が Diagnostic を
//!   返さないこと(expected.json は `[]`)
//! - `err_*.pact` … 1 件以上の Diagnostic を返し、その JSON が
//!   `{name}.expected.json` と一致すること
//!
//! 期待ファイルの再生成: `UPDATE_GOLDEN=1 cargo test -p pact_check --test golden_check`
//! (golden の変更は人間レビュー必須 — ARCHITECTURE.md 不変条件 3)

use std::fs;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/check")
}

fn fixture_names() -> Vec<String> {
    let dir = golden_dir();
    let mut cases: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("readable dir entry").path();
            let name = path.file_name()?.to_str()?;
            name.strip_suffix(".pact").map(str::to_string)
        })
        .collect();
    cases.sort();
    cases
}

fn check_fixture(name: &str, source: &str) -> Result<Vec<pact_check::Diagnostic>, String> {
    let result = pact_syntax::parse_module(source);
    if !result.errors.is_empty() {
        let summary: Vec<String> = result
            .errors
            .iter()
            .map(|e| format!("{} {} ({})", e.code, e.message, e.span))
            .collect();
        return Err(format!(
            "check fixtures must be syntactically valid, got {} syntax error(s):\n  {}",
            result.errors.len(),
            summary.join("\n  ")
        ));
    }
    let file = format!("tests/golden/check/{name}.pact");
    Ok(pact_check::check_module(&file, &result.module))
}

fn actual_json(name: &str, source: &str) -> Result<serde_json::Value, String> {
    let diags = check_fixture(name, source)?;
    if name.starts_with("err_") && diags.is_empty() {
        return Err("expected at least one diagnostic, but the check passed".to_string());
    }
    if name.starts_with("ok_") && !diags.is_empty() {
        let summary: Vec<String> = diags
            .iter()
            .map(|d| format!("{} {}", d.code, d.message))
            .collect();
        return Err(format!(
            "expected no diagnostics, got {}:\n  {}",
            diags.len(),
            summary.join("\n  ")
        ));
    }
    serde_json::to_value(&diags).map_err(|e| e.to_string())
}

#[test]
fn golden_check() {
    let dir = golden_dir();
    let cases = fixture_names();
    assert!(
        !cases.is_empty(),
        "no golden fixtures found in {}",
        dir.display()
    );

    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    for name in &cases {
        let source = fs::read_to_string(dir.join(format!("{name}.pact"))).expect("readable .pact");
        let expected_path = dir.join(format!("{name}.expected.json"));

        let actual = match actual_json(name, &source) {
            Ok(value) => value,
            Err(msg) => {
                failures.push(format!("{name}: {msg}"));
                continue;
            }
        };

        if update {
            let mut text = serde_json::to_string_pretty(&actual).expect("serializable");
            text.push('\n');
            fs::write(&expected_path, text).expect("writable expected file");
            continue;
        }

        let expected_text = match fs::read_to_string(&expected_path) {
            Ok(text) => text,
            Err(e) => {
                failures.push(format!("{name}: missing expected file ({e})"));
                continue;
            }
        };
        let expected: serde_json::Value =
            serde_json::from_str(&expected_text).expect("expected file is valid JSON");

        if actual != expected {
            failures.push(format!(
                "{name}: output differs from {name}.expected.json\n--- actual ---\n{}",
                serde_json::to_string_pretty(&actual).expect("serializable")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} golden case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// 不変条件 2(CLAUDE.md / spec/diagnostic-schema.md): 全 Diagnostic に
/// span・採番ルール準拠の code・最低 1 つの fix 候補が含まれる。
#[test]
fn check_diagnostics_satisfy_invariants() {
    let dir = golden_dir();
    for name in fixture_names() {
        let source =
            fs::read_to_string(dir.join(format!("{name}.pact"))).expect("readable fixture");
        let diags = check_fixture(&name, &source).expect("fixture parses");
        for d in &diags {
            let code_ok = d.code.len() == 10
                && d.code.starts_with("PACT-E")
                && d.code[6..].chars().all(|c| c.is_ascii_digit())
                && matches!(d.code.as_bytes()[6], b'1'..=b'4');
            assert!(
                code_ok,
                "{name}: code must match PACT-E[1-4]xxx, got '{}'",
                d.code
            );
            assert!(
                !d.fixes.is_empty(),
                "{name}: diagnostic {} has no fix candidate",
                d.code
            );
            assert_eq!(
                d.span.file,
                format!("tests/golden/check/{name}.pact"),
                "{name}: span.file mismatch"
            );
            assert!(
                d.span.start.line >= 1 && d.span.start.col >= 1,
                "{name}: span positions are 1-based"
            );
            for fix in &d.fixes {
                assert!(!fix.title.is_empty(), "{name}: fix without title");
                for edit in &fix.edits {
                    assert!(
                        edit.span.start.line >= 1 && edit.span.start.col >= 1,
                        "{name}: fix edit span is 1-based"
                    );
                }
            }
        }
    }
}

/// goal のカバー範囲検証: 4 検査領域(名前解決 / 型 / エフェクト / 契約)の
/// err fixture がそれぞれ存在し、対応カテゴリの Diagnostic を最低 1 件含む。
#[test]
fn all_check_categories_covered() {
    let dir = golden_dir();
    let categories = [
        ("err_name_", '1'),
        ("err_type_", '2'),
        ("err_effect_", '3'),
        ("err_contract_", '4'),
    ];
    for (prefix, category) in categories {
        let fixtures: Vec<String> = fixture_names()
            .into_iter()
            .filter(|n| n.starts_with(prefix))
            .collect();
        assert!(
            !fixtures.is_empty(),
            "no fixtures with prefix '{prefix}' (category {category})"
        );
        for name in fixtures {
            let source =
                fs::read_to_string(dir.join(format!("{name}.pact"))).expect("readable fixture");
            let diags = check_fixture(&name, &source).expect("fixture parses");
            assert!(
                diags.iter().any(|d| d.code.as_bytes()[6] == category as u8),
                "{name}: expected at least one PACT-E{category}xxx diagnostic, got: {:?}",
                diags.iter().map(|d| d.code.clone()).collect::<Vec<_>>()
            );
        }
    }
}
