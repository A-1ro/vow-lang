//! M23 / #60: List / record / tagged 引数の bounded 検証が機能することを
//! 固定する。スカラだけだった段階1 と違い、`List<R>` を入力に取る集計・計画
//! 関数の `ensures` も反例探索の対象に入る。
//!
//! ※ PR #76 review 反映: List/record 引数は部分サンプル(長さ 0..=2 など)で
//!    しか網羅できないため、verification level は `Generative`(全数)ではなく
//!    `Bounded`(部分検査)で報告される。テスト名・コメントの "generative" は
//!    歴史的経緯で残る("bounded への昇格" の方が正確)。

use kei_check::pbt::run_module;

fn module(src: &str) -> kei_syntax::ast::Module {
    let parsed = kei_syntax::parse_module(src);
    assert!(
        parsed.errors.is_empty(),
        "test source must parse: {:?}",
        parsed.errors
    );
    parsed.module
}

#[test]
fn list_argument_function_lifts_to_generative_when_clean() {
    // List<Int> を取る純粋関数。fold で総和、ensures は単調(全要素非負ならば
    // 結果も非負)。要素 candidate に負の値が混ざるが requires.all で弾かれる。
    let src = "module t\n\
               func nonNeg(x: Int) -> Bool { return x >= 0 }\n\
               func plus(acc: Int, x: Int) -> Int { return acc + x }\n\
               func total(xs: List<Int>) -> Int\n  requires xs.all(nonNeg)\n  ensures result >= 0\n{\n  return xs.fold(0, plus)\n}\n";
    let m = module(src);
    let outcomes = run_module(&m);
    let total = outcomes
        .iter()
        .find(|o| o.func == "total")
        .expect("total must be evaluated");
    assert!(
        total.passed,
        "non-negative fold of non-negative xs should be clean: {:?}",
        total.counterexample
    );
    assert!(total.cases_checked > 0, "must check at least one case");
}

#[test]
fn list_argument_function_finds_counterexample_when_contract_too_weak() {
    // requires が不十分なケース: 要素に負がある(`fold(0, plus)` が負になる)が
    // ensures `result >= 0` を要求している。反例が見つかる。
    let src = "module t\n\
               func plus(acc: Int, x: Int) -> Int { return acc + x }\n\
               func total(xs: List<Int>) -> Int\n  ensures result >= 0\n{\n  return xs.fold(0, plus)\n}\n";
    let m = module(src);
    let outcomes = run_module(&m);
    let total = outcomes
        .iter()
        .find(|o| o.func == "total")
        .expect("total must be evaluated");
    assert!(
        !total.passed,
        "missing requires should be caught by generative"
    );
    let ce = total
        .counterexample
        .as_ref()
        .expect("must produce counterexample");
    assert!(
        !ce.precondition,
        "must be an ensures counterexample, not a precondition violation"
    );
}

#[test]
fn record_argument_function_evaluates_field_access() {
    // record をパラメータに取る純粋関数。フィールドアクセスを契約で使う。
    let src = "module t\n\
               record P { qty: Int }\n\
               func nonNegQty(p: P) -> Bool { return p.qty >= 0 }\n\
               func double(p: P) -> Int\n  requires nonNegQty(p)\n  ensures result >= 0\n{\n  return p.qty + p.qty\n}\n";
    let m = module(src);
    let outcomes = run_module(&m);
    let double = outcomes
        .iter()
        .find(|o| o.func == "double")
        .expect("double must be evaluated");
    assert!(
        double.passed,
        "double of non-negative qty must satisfy ensures: {:?}",
        double.counterexample
    );
}

#[test]
fn list_of_record_argument_lifts_to_generative() {
    // M23 / #60 の中心ケース: List<Product> を入力に取り、record フィールドを
    // 集計する関数を generative に上げられること。
    let src = "module t\n\
               record P { v: Int }\n\
               func nn(p: P) -> Bool { return p.v >= 0 }\n\
               func add(acc: Int, p: P) -> Int { return acc + p.v }\n\
               func total(ps: List<P>) -> Int\n  requires ps.all(nn)\n  ensures result >= 0\n{\n  return ps.fold(0, add)\n}\n";
    let m = module(src);
    let outcomes = run_module(&m);
    let total = outcomes
        .iter()
        .find(|o| o.func == "total")
        .expect("total must be evaluated");
    assert!(
        total.passed,
        "non-negative fold over List<Product>-like should be clean: {:?}",
        total.counterexample
    );
}

/// PR #76 review: List 引数を持つ関数の verification は `Bounded`(部分検査)
/// であり、`Generative`(全数)とは区別される。スカラのみの関数とは振る舞いが
/// 異なることを CheckReport レベルで固定する。
#[test]
fn list_argument_lifts_to_bounded_not_generative() {
    use kei_check::{check_module_report_with, CheckOptions, Verification};

    let src = "module t\n\
               func plus(acc: Int, x: Int) -> Int { return acc + x }\n\
               func nonNeg(x: Int) -> Bool { return x >= 0 }\n\
               // List 引数 → Bounded\n\
               func total(xs: List<Int>) -> Int\n  requires xs.all(nonNeg)\n  ensures result >= 0\n{\n  return xs.fold(0, plus)\n}\n\
               // スカラのみ → Generative\n\
               func incr(n: Int) -> Int\n  requires n >= 0\n  ensures result > n\n{\n  return n + 1\n}\n";
    let m = module(src);
    let report = check_module_report_with(
        "t.kei",
        &m,
        CheckOptions {
            generative: true,
            ..CheckOptions::default()
        },
    );
    let total_ens = report
        .contracts
        .iter()
        .find(|c| c.func == "total" && c.expr == "result >= 0")
        .expect("total ensures expected");
    assert_eq!(
        total_ens.verification,
        Verification::Bounded,
        "List arg must lift only to Bounded (not Generative)"
    );
    let incr_ens = report
        .contracts
        .iter()
        .find(|c| c.func == "incr" && c.expr == "result > n")
        .expect("incr ensures expected");
    assert_eq!(
        incr_ens.verification,
        Verification::Generative,
        "scalar-only function should still lift to Generative"
    );
}

#[test]
fn map_filter_length_ensures_lift_to_generative() {
    // planAllReorders と同型の関係: filter + map の結果長 <= 入力長 という
    // 集計契約を generative で検証できる。
    let src = "module t\n\
               record P { keep: Bool }\n\
               func k(p: P) -> Bool { return p.keep }\n\
               func to_q(p: P) -> Int { return 1 }\n\
               func plan(ps: List<P>) -> List<Int>\n  ensures result.length <= ps.length\n{\n  return ps.filter(k).map(to_q)\n}\n";
    let m = module(src);
    let outcomes = run_module(&m);
    let plan = outcomes
        .iter()
        .find(|o| o.func == "plan")
        .expect("plan must be evaluated");
    assert!(
        plan.passed,
        "filter then map preserves length bound: {:?}",
        plan.counterexample
    );
}
