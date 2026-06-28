//! M22 / #57: List リテラル `[a, b, c]` と tagged 明示コンストラクタ
//! (`ProductId("...")`)の型検査挙動を固定する。

use kei_check::{check_module, Severity};

fn check(src: &str) -> Vec<(String, String)> {
    let parsed = kei_syntax::parse_module(src);
    assert!(
        parsed.errors.is_empty(),
        "test source should parse: {:?}",
        parsed.errors
    );
    check_module("t.kei", &parsed.module)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| (d.code.clone(), d.message.clone()))
        .collect()
}

#[test]
fn list_lit_with_compatible_elements_is_clean() {
    let src = "module t\n\
               func xs() -> List<Int> { return [1, 2, 3] }\n";
    let diags = check(src);
    assert!(diags.is_empty(), "expected no errors, got {diags:?}");
}

#[test]
fn list_lit_with_mixed_types_reports_type_mismatch() {
    let src = "module t\n\
               func xs() -> List<Int> { return [1, \"two\", 3] }\n";
    let diags = check(src);
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E2001"),
        "expected KEI-E2001 for mixed list elements; got {codes:?}"
    );
}

#[test]
fn empty_list_lit_unifies_with_context() {
    let src = "module t\n\
               func xs() -> List<Int> { return [] }\n";
    let diags = check(src);
    assert!(
        diags.is_empty(),
        "empty list literal should unify with annotated type; got {diags:?}"
    );
}

#[test]
fn tagged_constructor_clean() {
    let src = "module t\n\
               type ProductId = String tagged \"ProductId\"\n\
               func mk() -> ProductId { return ProductId(\"P-001\") }\n";
    let diags = check(src);
    assert!(
        diags.is_empty(),
        "tagged ctor with matching underlying should be clean; got {diags:?}"
    );
}

#[test]
fn raw_base_assignment_to_tagged_still_blocked() {
    let src = "module t\n\
               type ProductId = String tagged \"ProductId\"\n\
               func bad() -> ProductId { return \"P-001\" }\n";
    let diags = check(src);
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E2005"),
        "raw base -> tagged must still raise KEI-E2005; got {codes:?}"
    );
}

#[test]
fn tagged_constructor_with_wrong_arg_type_reports_mismatch() {
    let src = "module t\n\
               type ProductId = String tagged \"ProductId\"\n\
               func bad() -> ProductId { return ProductId(42) }\n";
    let diags = check(src);
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E2001"),
        "tagged ctor with wrong arg type must be KEI-E2001; got {codes:?}"
    );
}

#[test]
fn tagged_constructor_with_wrong_arity_reports_mismatch() {
    let src = "module t\n\
               type ProductId = String tagged \"ProductId\"\n\
               func bad() -> ProductId { return ProductId(\"a\", \"b\") }\n";
    let diags = check(src);
    let codes: Vec<&str> = diags.iter().map(|(c, _)| c.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E2001"),
        "tagged ctor with wrong arity must be KEI-E2001; got {codes:?}"
    );
}

#[test]
fn list_of_records_constructed_with_record_lit() {
    let src = "module t\n\
               record P { qty: Int }\n\
               func nn(p: P) -> Bool { return p.qty >= 0 }\n\
               func sample() -> List<P> { return [P { qty: 1 }, P { qty: 2 }] }\n";
    let diags = check(src);
    assert!(diags.is_empty(), "expected clean, got {diags:?}");
}

/// 異種要素の List リテラルは **要素位置のミスマッチだけ** を報告し、
/// 戻り値型の List<...> でさらに重ねて報告しない(同じ根本原因の二重診断回避)。
#[test]
fn mixed_list_lit_does_not_double_report_at_return_site() {
    let src = "module t\n\
               func xs() -> List<Int> { return [1, \"two\", 3] }\n";
    let diags = check(src);
    let mismatch_count = diags.iter().filter(|(c, _)| c == "KEI-E2001").count();
    assert_eq!(
        mismatch_count, 1,
        "mixed list must report exactly one KEI-E2001 (per-element), \
         not also one at the return site; got {diags:?}"
    );
}
