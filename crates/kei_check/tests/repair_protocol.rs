//! Agent Repair Protocol(M18 / #24)の統合テスト。
//!
//! 構造化修正提案(`suggested_contract`)をエージェントが機械適用 → 再検証すると
//! 診断が減ることを確認する。提案は後方互換(`fixes` と独立した追加フィールド)。

use kei_check::{check_module_with, CheckOptions, Severity};

fn check(src: &str) -> Vec<kei_check::Diagnostic> {
    let parsed = kei_syntax::parse_module(src);
    assert!(
        parsed.errors.is_empty(),
        "fixture must parse: {:?}",
        parsed.errors
    );
    let opts = CheckOptions {
        suggest_contracts: true,
        ..Default::default()
    };
    check_module_with("repair.kei", &parsed.module, opts)
}

/// 提案された構造化差分(suggested_contract)を関数シグネチャへ適用する。
/// 本体を開く `{` 行の直前に `  ensures <expr>` を差し込む(requires の有無に依らず動く)。
fn apply_ensures(src: &str, expr: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut inserted = false;
    for line in src.lines() {
        if !inserted && line.trim() == "{" {
            out.push(format!("  ensures {expr}"));
            inserted = true;
        }
        out.push(line.to_string());
    }
    out.join("\n") + "\n"
}

#[test]
fn apply_suggested_contract_reduces_diagnostics() {
    // 適用前: 契約(ensures)が無い純粋関数 → ContractMissing(KEI-E4008)1 件。
    let before_src = "module t\n\n\
        func decrementAvailable(available: Int) -> Int\n\
        \x20 requires available > 0\n\
        {\n\
        \x20 return available - 1\n\
        }\n";
    let before = check(before_src);
    assert_eq!(
        before.len(),
        1,
        "expected exactly one ContractMissing: {before:?}"
    );
    let d = &before[0];
    assert_eq!(d.code, "KEI-E4008");

    // 構造化提案を取り出す(後方互換: fixes も残っている)。
    let sc = d
        .suggested_contract
        .as_ref()
        .expect("ContractMissing carries a suggested_contract");
    assert_eq!(sc.kind, "ContractMissing");
    assert_eq!(sc.clause, "ensures");
    assert!(
        !d.fixes.is_empty(),
        "fixes must remain (backward compatible)"
    );

    // 提案を適用 → 再検証。
    let after_src = apply_ensures(before_src, &sc.expr);
    let after = check(&after_src);

    // 診断が減る(ContractMissing が解消)。
    assert!(
        after.len() < before.len(),
        "applying the proposal must reduce diagnostics: before={}, after={}",
        before.len(),
        after.len()
    );
    // 適用した契約は check-clean(エラーを生まない。result はまさに本体の式)。
    assert!(
        after.iter().all(|d| d.severity != Severity::Error),
        "applied contract must not introduce errors: {after:?}"
    );
    assert!(
        !after.iter().any(|d| d.code == "KEI-E4008"),
        "ContractMissing must be gone after applying the suggestion"
    );
}

#[test]
fn suggested_bool_postcondition_is_parenthesized_and_applies_clean() {
    // Bool 戻りで本体が比較式の関数。提案は `result == (x > 0)` のように括弧で包まれ、
    // 適用しても `(result == x) > 0` に化けず check-clean になる(PR #50 第6レビュー)。
    let before_src = "module t\n\nfunc positive(x: Int) -> Bool\n{\n  return x > 0\n}\n";
    let before = check(before_src);
    let d = &before[0];
    let sc = d.suggested_contract.as_ref().expect("suggested_contract");
    assert_eq!(
        sc.expr, "result == (x > 0)",
        "binary body must be parenthesized"
    );

    let after_src = apply_ensures(before_src, &sc.expr);
    let after = check(&after_src);
    assert!(
        after.iter().all(|d| d.severity != Severity::Error),
        "applied parenthesized contract must be check-clean: {after:?}"
    );
    assert!(
        !after.iter().any(|d| d.code == "KEI-E4008"),
        "ContractMissing must be gone after applying"
    );
}

#[test]
fn default_mode_emits_no_suggestions() {
    // suggest_contracts は opt-in。既定の検査は ContractMissing を出さない。
    let src = "module t\n\nfunc f(x: Int) -> Int\n{\n  return x\n}\n";
    let parsed = kei_syntax::parse_module(src);
    let default = check_module_with("repair.kei", &parsed.module, CheckOptions::default());
    assert!(
        default.is_empty(),
        "default mode must not suggest: {default:?}"
    );
}
