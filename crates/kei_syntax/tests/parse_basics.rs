//! パーサの基本性質テスト。網羅的な入出力契約は tests/golden/syntax/ が担う。

use kei_syntax::ast::{BinOp, Expr, Item, Stmt};

#[test]
fn empty_source_parses_to_empty_module() {
    let result = kei_syntax::parse_module("");
    assert!(result.errors.is_empty());
    assert!(result.module.decl.is_none());
    assert!(result.module.items.is_empty());
}

#[test]
fn operator_precedence_mul_binds_tighter_than_add() {
    let src = "func f() -> Int {\n  return 1 + 2 * 3\n}\n";
    let result = kei_syntax::parse_module(src);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let Item::Func(f) = &result.module.items[0] else {
        panic!("expected func");
    };
    let Stmt::Return(ret) = &f.body.stmts[0] else {
        panic!("expected return");
    };
    // (1 + (2 * 3)) になること
    let Some(Expr::Binary { op, rhs, .. }) = &ret.value else {
        panic!("expected binary expr");
    };
    assert_eq!(format!("{op:?}"), "Add");
    assert!(matches!(**rhs, Expr::Binary { .. }));
}

#[test]
fn operator_precedence_rem_and_or_are_in_expected_tiers() {
    let src = "func f(a: Int, b: Int, c: Int) -> Bool {\n  return a + b % c == 0 || a > 10 implies b > 0\n}\n";
    let result = kei_syntax::parse_module(src);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let Item::Func(f) = &result.module.items[0] else {
        panic!("expected func");
    };
    let Stmt::Return(ret) = &f.body.stmts[0] else {
        panic!("expected return");
    };
    let Some(Expr::Binary { op, lhs, rhs, .. }) = &ret.value else {
        panic!("expected implication");
    };
    assert_eq!(*op, BinOp::Implies);
    assert!(matches!(**lhs, Expr::Binary { op: BinOp::Or, .. }));
    assert!(matches!(**rhs, Expr::Binary { op: BinOp::Gt, .. }));

    let Expr::Binary {
        lhs: or_lhs,
        rhs: or_rhs,
        ..
    } = lhs.as_ref()
    else {
        panic!("expected or");
    };
    assert!(matches!(**or_rhs, Expr::Binary { op: BinOp::Gt, .. }));
    let Expr::Binary {
        op: BinOp::Eq,
        lhs: eq_lhs,
        ..
    } = or_lhs.as_ref()
    else {
        panic!("expected equality");
    };
    let Expr::Binary {
        op: BinOp::Add,
        rhs: add_rhs,
        ..
    } = eq_lhs.as_ref()
    else {
        panic!("expected addition");
    };
    assert!(matches!(**add_rhs, Expr::Binary { op: BinOp::Rem, .. }));
}

#[test]
fn record_literal_is_not_parsed_in_if_condition() {
    // if 条件では `{` はブロック開始であり record リテラルにならない
    let src =
        "func f(s: State) -> Bool {\n  if s.ready {\n    return true\n  }\n  return false\n}\n";
    let result = kei_syntax::parse_module(src);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn errors_are_sorted_by_source_position() {
    let src = "func f(record: Int) -> Int {\n  let type = 1 @ 2\n  return 0\n}\n";
    let result = kei_syntax::parse_module(src);
    assert!(result.errors.len() >= 2);
    let positions: Vec<_> = result
        .errors
        .iter()
        .map(|e| (e.span.start.line, e.span.start.col))
        .collect();
    let mut sorted = positions.clone();
    sorted.sort();
    assert_eq!(positions, sorted);
}

#[test]
fn parse_continues_after_broken_declaration() {
    // 壊れた宣言の後でも次の宣言を拾える(宣言レベルのエラー回復)
    let src = "func broken(\n\nrecord Ok {\n  x: Int\n}\n";
    let result = kei_syntax::parse_module(src);
    assert!(!result.errors.is_empty());
    assert!(result
        .module
        .items
        .iter()
        .any(|item| matches!(item, Item::Record(r) if r.name.name == "Ok")));
}
