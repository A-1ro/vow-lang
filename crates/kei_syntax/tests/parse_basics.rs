//! パーサの基本性質テスト。網羅的な入出力契約は tests/golden/syntax/ が担う。

use kei_syntax::ast::{Expr, Item, Stmt};

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
