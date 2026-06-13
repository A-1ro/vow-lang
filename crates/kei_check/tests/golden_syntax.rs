//! tests/golden/syntax/ の golden test ランナー(契約本文は fixture 側)。
//!
//! - `ok_*.kei` … エラーゼロでパースでき、AST の JSON ダンプが
//!   `{name}.expected.json` と一致すること
//! - `err_*.kei` … 1 件以上の Diagnostic を返し、その JSON が
//!   `{name}.expected.json` と一致すること
//!
//! 期待ファイルの再生成: `UPDATE_GOLDEN=1 cargo test -p kei_check --test golden_syntax`
//! (golden の変更は人間レビュー必須 — ARCHITECTURE.md 不変条件 3)

use std::fs;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/syntax")
}

/// fixture 名 → 実際の出力 JSON(ok: AST ダンプ / err: Diagnostic 配列)。
fn actual_json(name: &str, source: &str) -> Result<serde_json::Value, String> {
    let result = kei_syntax::parse_module(source);
    if name.starts_with("err_") {
        let file = format!("tests/golden/syntax/{name}.kei");
        let diags = kei_check::syntax_diagnostics(&file, &result.errors);
        if diags.is_empty() {
            return Err("expected at least one diagnostic, but parsing succeeded".to_string());
        }
        serde_json::to_value(&diags).map_err(|e| e.to_string())
    } else {
        if !result.errors.is_empty() {
            let summary: Vec<String> = result
                .errors
                .iter()
                .map(|e| format!("{} {} ({})", e.code, e.message, e.span))
                .collect();
            return Err(format!(
                "expected no diagnostics, got {}:\n  {}",
                result.errors.len(),
                summary.join("\n  ")
            ));
        }
        serde_json::to_value(&result.module).map_err(|e| e.to_string())
    }
}

#[test]
fn golden_syntax() {
    let dir = golden_dir();
    let mut cases: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("readable dir entry").path();
            let name = path.file_name()?.to_str()?;
            name.strip_suffix(".kei").map(str::to_string)
        })
        .collect();
    cases.sort();
    assert!(
        !cases.is_empty(),
        "no golden fixtures found in {}",
        dir.display()
    );

    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    for name in &cases {
        let source = fs::read_to_string(dir.join(format!("{name}.kei"))).expect("readable .kei");
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

/// エラー回復: 異常系ファイルで複数 Diagnostic を返せること(M1 完了条件)。
#[test]
fn error_recovery_yields_multiple_diagnostics() {
    let dir = golden_dir();
    for name in ["err_multiple", "err_reserved_ident"] {
        let source = fs::read_to_string(dir.join(format!("{name}.kei"))).expect("readable fixture");
        let result = kei_syntax::parse_module(&source);
        let diags = kei_check::syntax_diagnostics(
            &format!("tests/golden/syntax/{name}.kei"),
            &result.errors,
        );
        assert!(
            diags.len() >= 2,
            "{name}: expected multiple diagnostics from error recovery, got {}",
            diags.len()
        );
    }
}

/// 不変条件 2: 構文 Diagnostic にも span・code・最低 1 つの fix 候補が含まれる。
#[test]
fn syntax_diagnostics_satisfy_invariants() {
    let dir = golden_dir();
    for entry in fs::read_dir(&dir).expect("readable golden dir") {
        let path = entry.expect("readable dir entry").path();
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".kei"))
        else {
            continue;
        };
        if !name.starts_with("err_") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("readable fixture");
        let result = kei_syntax::parse_module(&source);
        let diags = kei_check::syntax_diagnostics(
            &format!("tests/golden/syntax/{name}.kei"),
            &result.errors,
        );
        for d in &diags {
            assert!(
                d.code.starts_with("KEI-E0"),
                "{name}: syntax diagnostics use category 0, got {}",
                d.code
            );
            assert!(!d.fixes.is_empty(), "{name}: diagnostic without fix");
            assert!(d.span.start.line >= 1 && d.span.start.col >= 1);
        }
    }
}
