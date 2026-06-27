//! kei_emit の単体テスト: 生成 TS の形・検査エラー時の拒否・source map の解決。

use std::fs;
use std::path::{Path, PathBuf};

fn emit(src: &str) -> kei_emit::EmitOutput {
    kei_emit::emit_module("test.kei", src).unwrap_or_else(|d| {
        panic!(
            "expected clean emit, got diagnostics: {}",
            serde_json::to_string_pretty(&d).expect("serializable")
        )
    })
}

#[test]
fn record_emits_type_and_factory() {
    let out = emit(concat!(
        "record Point {\n",
        "  x: Int\n",
        "  y: Int\n",
        "}\n",
        "\n",
        "func origin() -> Point {\n",
        "  return Point { x: 0, y: 0 }\n",
        "}\n",
    ));
    assert!(out.ts.contains("export type Point = {"));
    assert!(out.ts.contains("readonly x: number;"));
    assert!(out
        .ts
        .contains("export function Point(fields: Point): Point {"));
    assert!(out.ts.contains("return Point({ x: 0, y: 0 });"));
}

#[test]
fn enum_emits_tagged_union_and_constructors() {
    let out = emit(concat!(
        "type OrderId = String tagged \"OrderId\"\n",
        "\n",
        "enum OrderStatus {\n",
        "  Draft\n",
        "  Submitted(OrderId)\n",
        "  Rejected { reason: String, retryable: Bool }\n",
        "}\n",
    ));
    assert!(out.ts.contains("| { readonly kind: \"Draft\" }"));
    assert!(out
        .ts
        .contains("| { readonly kind: \"Submitted\"; readonly values: readonly [OrderId] }"));
    assert!(out.ts.contains(
        "| { readonly kind: \"Rejected\"; readonly fields: { readonly reason: string; readonly retryable: boolean } };"
    ));
    assert!(out
        .ts
        .contains("Draft: { kind: \"Draft\" } as OrderStatus,"));
    assert!(out.ts.contains(
        "Submitted: (v0: OrderId): OrderStatus => ({ kind: \"Submitted\", values: [v0] }),"
    ));
    // tagged 型は branded type + コンストラクタ。
    assert!(out
        .ts
        .contains("export type OrderId = string & { readonly __keiTag: \"OrderId\" };"));
    assert!(out
        .ts
        .contains("export function OrderId(value: string): OrderId {"));
}

#[test]
fn requires_emits_structured_violation() {
    let out = emit(concat!(
        "func half(n: Int) -> Int\n",
        "  requires n > 0\n",
        "{\n",
        "  return n / 2\n",
        "}\n",
    ));
    assert!(out.ts.contains("if (!(n > 0)) {"));
    assert!(out.ts.contains("throw new KeiContractViolation({"));
    assert!(out.ts.contains("clause: \"requires\","));
    assert!(out.ts.contains("condition: \"n > 0\","));
    assert!(out.ts.contains("file: \"test.kei\","));
    assert!(out.ts.contains("line: 2,"));
    // Int 除算は切り捨て。
    assert!(out.ts.contains("return Math.trunc(n / 2);"));
    // ランタイム import。
    assert!(out
        .ts
        .contains("import { KeiContractViolation } from \"@kei/runtime\";"));
}

#[test]
fn ensures_wraps_body_and_captures_old() {
    let out = emit(concat!(
        "func increment(count: Int, step: Int) -> Int\n",
        "  requires step > 0\n",
        "  ensures result == old(count) + step\n",
        "{\n",
        "  return count + step\n",
        "}\n",
    ));
    assert!(out.ts.contains("const kei$old$0 = count;"));
    assert!(out.ts.contains("const kei$result = ((): number => {"));
    assert!(out.ts.contains("if (!(kei$result === kei$old$0 + step)) {"));
    assert!(out.ts.contains("clause: \"ensures\","));
    assert!(out
        .ts
        .contains("condition: \"result == old(count) + step\","));
    assert!(out.ts.contains("return kei$result;"));
}

/// 生成 TS から `condition: "<json>"` の値を出現順に抜き出し、JSON エスケープを解いて返す。
/// `condition` は `ts_string`(= serde_json)で TS 文字列リテラル化されているので往復で復元できる。
fn extract_conditions(ts: &str) -> Vec<String> {
    ts.lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("condition: ")?;
            let json = rest.strip_suffix(',').unwrap_or(rest);
            Some(serde_json::from_str::<String>(json).expect("condition is a JSON string literal"))
        })
        .collect()
}

/// #32: 契約式の Kei 表記は `kei_check::contract_expr_text` を唯一の正規実装とし、`kei_emit` は
/// そこへ委譲する。検証レポートの `CheckReport.contracts[].expr` と実行時診断の
/// `KeiContractViolation.condition` は**バイト一致が要件**。優先順位・結合方向・括弧最小化・
/// 否定・呼び出し・フィールド・`match` を網羅する契約群で両経路の出力が一致することを固定し、
/// 将来どちらかが二重実装へ逆戻りしても検出できるようにする(出典: PR #31 レビュー 指摘1)。
#[test]
fn contract_expr_text_is_single_source_for_report_and_runtime() {
    let src = concat!(
        "module a.b\n",
        "\n",
        "enum Status {\n",
        "  Open\n",
        "  Closed\n",
        "}\n",
        "\n",
        "func classify(s: Status) -> Int {\n",
        "  return match s { Status.Open => 1, Status.Closed => 0 }\n",
        "}\n",
        "\n",
        "func f(a: Int, b: Int, c: Int) -> Int\n",
        "  requires a + b * c > 0\n",
        "  requires (a + b) * c > 0\n",
        "  requires a - (b - c) == 0\n",
        "  requires a > 0 implies b > 0 implies c > 0\n",
        "  requires (a > 0 implies b > 0) implies c > 0\n",
        "  requires !(a == b)\n",
        "  requires -a < b\n",
        "  ensures result == old(a) + old(b)\n",
        "{\n",
        "  return a + b + c\n",
        "}\n",
    );

    // 検証レポート側(kei_check が組む expr)。
    let parsed = kei_syntax::parse_module(src);
    assert!(parsed.errors.is_empty(), "test source must parse cleanly");
    let report = kei_check::check_module_report("test.kei", &parsed.module);
    let report_exprs: Vec<String> = report.contracts.iter().map(|c| c.expr.clone()).collect();
    assert!(!report_exprs.is_empty(), "expected contracts in report");

    // 実行時診断側(kei_emit が出す condition)。requires→ensures の宣言順で並ぶ。
    let runtime_conditions = extract_conditions(&emit(src).ts);

    assert_eq!(
        report_exprs, runtime_conditions,
        "CheckReport.contracts[].expr と KeiContractViolation.condition はバイト一致が要件(#32)"
    );
}

/// 右ネストした同順位の等値/関係比較は括弧を保つ。`==` は JS で左結合なので、
/// `c == (a == b)` を `c === a === b` と書くと `(c === a) === b` に化ける(PR #50
/// 独立レビューの指摘)。`rhs_min` を一段上げて右辺の括弧を維持する。
#[test]
fn right_nested_equality_keeps_parentheses() {
    let out = emit(concat!(
        "func f(a: String, b: String, c: Bool) -> Bool {\n",
        "  return c == (a == b)\n",
        "}\n",
    ));
    assert!(
        out.ts.contains("return c === (a === b);"),
        "right-nested equality must keep parens: {}",
        out.ts
    );
    // 逆に過剰括弧は付けない: `<` は JS で `===` より強く結合するため、
    // `a == (b < c)` の右辺は括弧なし `a === b < c`(= `a === (b < c)`)で正しい。
    let out2 = emit(concat!(
        "func g(a: Bool, b: Int, c: Int) -> Bool {\n",
        "  return a == (b < c)\n",
        "}\n",
    ));
    assert!(
        out2.ts.contains("return a === b < c;"),
        "relational rhs of equality needs no parens (binds tighter): {}",
        out2.ts
    );
}

#[test]
fn else_fail_unwraps_via_shared_discriminant() {
    let out = emit(concat!(
        "module a.b\n",
        "\n",
        "import infra.database as Database\n",
        "\n",
        "enum E {\n",
        "  NotFound(String)\n",
        "}\n",
        "\n",
        "func f(key: String) -> Result<Int, E> {\n",
        "  let v = Database.fetch(key) else fail E.NotFound(key)\n",
        "  return Ok(v)\n",
        "}\n",
    ));
    assert!(out.ts.contains("const v$ = Database.fetch(key);"));
    assert!(out.ts.contains("if (!v$.ok) {"));
    assert!(out.ts.contains("return Err(E.NotFound(key));"));
    assert!(out.ts.contains("const v = v$.value;"));
    // モジュール a.b(深さ 1)からの相対 import。
    assert!(out
        .ts
        .contains("import * as Database from \"../infra/database\";"));
    assert_eq!(out.ts_path, "a/b.ts");
}

#[test]
fn implies_emits_disjunction() {
    let out = emit(concat!(
        "func f(a: Bool, b: Bool) -> Bool\n",
        "  requires a implies b\n",
        "{\n",
        "  return b\n",
        "}\n",
    ));
    assert!(out.ts.contains("if (!(!(a) || b)) {"));
}

#[test]
fn or_and_remainder_emit_with_kei_int_semantics() {
    let out = emit(concat!(
        "func validLot(amount: Int, minLot: Int, caseSize: Int) -> Bool\n",
        "  requires caseSize > 0\n",
        "  ensures result == (amount == 0 || amount >= minLot)\n",
        "{\n",
        "  return amount == 0 || amount % caseSize == 0\n",
        "}\n",
    ));
    assert!(out
        .ts
        .contains("return amount === 0 || amount % caseSize === 0;"));
    assert!(out
        .ts
        .contains("condition: \"result == (amount == 0 || amount >= minLot)\","));
}

#[test]
fn remainder_emits_plain_percent() {
    let out = emit(concat!(
        "module a.b\n",
        "\n",
        "import infra.random as Random\n",
        "\n",
        "extern Random.next() -> Int uses Random\n",
        "\n",
        "func bounded() -> Int\n",
        "  uses Random\n",
        "{\n",
        "  return Random.next() % (Random.next() + 1)\n",
        "}\n",
    ));
    let needle = "return Random.next() % (Random.next() + 1);";
    assert!(out.ts.contains(needle), "unexpected TS:\n{}", out.ts);
}

#[test]
fn check_errors_reject_emit() {
    let err = kei_emit::emit_module("bad.kei", "func f() -> Int {\n  return missing\n}\n")
        .expect_err("undefined name must reject emit");
    assert!(err.iter().any(|d| d.code == "KEI-E1001"));
}

#[test]
fn list_combinators_emit_to_array_methods() {
    let out = emit(concat!(
        "func isPos(x: Int) -> Bool {\n",
        "  return x > 0\n",
        "}\n",
        "\n",
        "func add(acc: Int, x: Int) -> Int {\n",
        "  return acc + x\n",
        "}\n",
        "\n",
        "func g(xs: List<Int>) -> Int {\n",
        "  return xs.fold(0, add)\n",
        "}\n",
        "\n",
        "func first(xs: List<Int>) -> Option<Int> {\n",
        "  return xs.get(0)\n",
        "}\n",
        "\n",
        "func anyPos(xs: List<Int>) -> Bool {\n",
        "  return xs.any(isPos)\n",
        "}\n",
        "\n",
        "func empty(xs: List<Int>) -> Bool {\n",
        "  return xs.isEmpty()\n",
        "}\n",
    ));
    assert!(
        out.ts.contains("xs.reduce(add, 0)"),
        "fold → reduce(f, init): {}",
        out.ts
    );
    assert!(
        out.ts.contains("keiListGet(xs, 0)"),
        "get → keiListGet: {}",
        out.ts
    );
    assert!(out.ts.contains("xs.some(isPos)"), "any → some: {}", out.ts);
    assert!(
        out.ts.contains("xs.length === 0"),
        "isEmpty() → .length === 0: {}",
        out.ts
    );
}

/// 回帰(PR #50 再レビュー P2): レコードが `isEmpty` フィールドを持っても、フィールド
/// アクセス `bag.isEmpty` は書き換えない(`.length === 0` への誤写を防ぐ)。List の
/// `xs.isEmpty()` はメソッド形なので衝突せず `.length === 0` に写る。
#[test]
fn record_field_named_is_empty_is_not_rewritten() {
    let out = emit(concat!(
        "record Bag {\n",
        "  isEmpty: Bool\n",
        "  size: Int\n",
        "}\n",
        "\n",
        "func flag(b: Bag) -> Bool {\n",
        "  return b.isEmpty\n",
        "}\n",
        "\n",
        "func vacant(xs: List<Int>) -> Bool {\n",
        "  return xs.isEmpty()\n",
        "}\n",
    ));
    assert!(
        out.ts.contains("return b.isEmpty;"),
        "record field stays a field access: {}",
        out.ts
    );
    assert!(
        !out.ts.contains("b.length === 0"),
        "record field must not be rewritten: {}",
        out.ts
    );
    assert!(
        out.ts.contains("(xs.length === 0)"),
        "List isEmpty() still rewrites: {}",
        out.ts
    );
}

/// 回帰(PR #50 レビュー P2): メソッド名が偶然 List コンビネータ名でも、レシーバが
/// import した外部名前空間なら書き換えない(`extern Database.get(id)` を keiListGet に
/// 誤変換しない)。逆に同名のローカル List レシーバは従来どおり書き換える。
#[test]
fn extern_namespace_calls_are_not_rewritten_as_list_helpers() {
    let out = emit(concat!(
        "import infra.database as Database\n",
        "\n",
        "extern Database.get(id: Int) -> Int uses Database.Read\n",
        "\n",
        "func lookup(id: Int) -> Int\n",
        "  uses Database.Read\n",
        "{\n",
        "  return Database.get(id)\n",
        "}\n",
        "\n",
        "func firstOf(xs: List<Int>) -> Option<Int> {\n",
        "  return xs.get(0)\n",
        "}\n",
    ));
    // 外部呼び出しは素直なメソッド呼び出し。
    assert!(
        out.ts.contains("return Database.get(id);"),
        "extern call must stay a plain call: {}",
        out.ts
    );
    assert!(
        !out.ts.contains("keiListGet(Database"),
        "extern call must not become keiListGet: {}",
        out.ts
    );
    // 同名でもローカル List レシーバは keiListGet へ。
    assert!(
        out.ts.contains("keiListGet(xs, 0)"),
        "list get must still rewrite: {}",
        out.ts
    );
}

/// 回帰(PR #50 第3レビュー P2): 連鎖した外部呼び出し `Database.reader().get(id)` は、
/// レシーバが計算値でも List ではない(検査器が List 操作と認めない)。span-set を根拠に
/// するので、keiListGet へ誤変換せず素直なメソッド呼び出しを出す。
#[test]
fn chained_extern_call_is_not_rewritten() {
    let out = emit(concat!(
        "import infra.database as Database\n",
        "\n",
        "extern Database.reader() -> Int uses Database.Read\n",
        "extern Database.get(id: Int) -> Int uses Database.Read\n",
        "\n",
        "func lookup(id: Int) -> Int\n",
        "  uses Database.Read\n",
        "{\n",
        "  return Database.get(id)\n",
        "}\n",
    ));
    assert!(
        out.ts.contains("return Database.get(id);"),
        "chained/extern call must stay a plain call: {}",
        out.ts
    );
    assert!(
        !out.ts.contains("keiListGet"),
        "no List helper for an external call: {}",
        out.ts
    );
}

/// 回帰(Claude レビュー): 連鎖で Call span がレシーバ先頭に揃っても、List 書き換えの
/// 鍵をメソッド名トークン位置にしているので内外の同名 `get` を取り違えない。extern が
/// List を返し、その結果に List.get を連鎖する `Database.get(id).get(0)` で、内側の extern
/// 呼び出しは素のまま・外側だけ keiListGet に写ることを固定する。
#[test]
fn chained_list_op_on_extern_result_keys_by_method_token() {
    let out = emit(concat!(
        "import infra.db as Database\n",
        "\n",
        "extern Database.get(id: Int) -> List<Int> uses Database.Read\n",
        "\n",
        "func f(id: Int) -> Option<Int>\n",
        "  uses Database.Read\n",
        "{\n",
        "  return Database.get(id).get(0)\n",
        "}\n",
    ));
    assert!(
        out.ts.contains("return keiListGet(Database.get(id), 0);"),
        "inner extern get must stay a plain call; only the outer List get rewrites: {}",
        out.ts
    );
    assert!(
        !out.ts.contains("keiListGet(Database,"),
        "the inner extern call must not be rewritten as a List helper: {}",
        out.ts
    );
}

// ---- source map ----

fn vlq_decode(s: &str) -> Vec<Vec<i64>> {
    const CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut segments = Vec::new();
    let mut cur = Vec::new();
    let mut value: i64 = 0;
    let mut shift = 0;
    for c in s.chars() {
        let digit = CHARS.find(c).expect("valid base64 char") as i64;
        value |= (digit & 0x1f) << shift;
        if digit & 0x20 != 0 {
            shift += 5;
        } else {
            let negative = value & 1 == 1;
            let mut v = value >> 1;
            if negative {
                v = -v;
            }
            cur.push(v);
            value = 0;
            shift = 0;
        }
    }
    if !cur.is_empty() {
        segments.push(cur);
    }
    segments
}

/// mappings 文字列 → (生成行, 生成列, ソース行, ソース列) の絶対値リスト(全て 0 始まり)。
fn decode_mappings(mappings: &str) -> Vec<(u32, u32, u32, u32)> {
    let mut out = Vec::new();
    let mut src_line: i64 = 0;
    let mut src_col: i64 = 0;
    for (gen_line, group) in mappings.split(';').enumerate() {
        let mut gen_col: i64 = 0;
        for seg in group.split(',').filter(|s| !s.is_empty()) {
            let fields = &vlq_decode(seg)[0];
            assert!(fields.len() >= 4, "segment must carry a source position");
            gen_col += fields[0];
            src_line += fields[2];
            src_col += fields[3];
            out.push((
                gen_line as u32,
                gen_col as u32,
                src_line as u32,
                src_col as u32,
            ));
        }
    }
    out
}

#[test]
fn source_map_resolves_contract_violation_to_kei_line() {
    let src = concat!(
        "module contracts.demo\n",    // 1
        "\n",                         // 2
        "func half(n: Int) -> Int\n", // 3
        "  requires n > 0\n",         // 4 ← 契約節
        "{\n",                        // 5
        "  return n / 2\n",           // 6
        "}\n",                        // 7
    );
    let out = kei_emit::emit_module("contracts/demo.kei", src).expect("clean emit");

    let map: serde_json::Value = serde_json::from_str(&out.map).expect("valid JSON map");
    assert_eq!(map["version"], 3);
    assert_eq!(map["sources"][0], "contracts/demo.kei");
    assert_eq!(map["sourcesContent"][0], src);

    let decoded = decode_mappings(map["mappings"].as_str().expect("mappings string"));

    // 生成 TS で契約違反を投げる行を探し、.kei の requires 行(4行目)に解決される
    // ことを検証する。
    let throw_line = out
        .ts
        .lines()
        .position(|l| l.contains("throw new KeiContractViolation"))
        .expect("generated TS throws on contract violation") as u32;
    let mapping = decoded
        .iter()
        .filter(|(gl, ..)| *gl == throw_line)
        .min_by_key(|(_, gc, ..)| *gc)
        .unwrap_or_else(|| panic!("no mapping for generated line {throw_line}"));
    assert_eq!(
        mapping.2 + 1,
        4,
        "throw must resolve to the requires clause line"
    );

    // return 文の行も .kei の 6 行目へ解決される。
    let return_line = out
        .ts
        .lines()
        .position(|l| l.contains("return Math.trunc(n / 2);"))
        .expect("generated TS contains the return") as u32;
    let mapping = decoded
        .iter()
        .filter(|(gl, ..)| *gl == return_line)
        .min_by_key(|(_, gc, ..)| *gc)
        .unwrap_or_else(|| panic!("no mapping for generated line {return_line}"));
    assert_eq!(mapping.2 + 1, 6);
}

// ---- examples/ 全件 ----

fn collect_kei_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_kei_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "kei") {
            out.push(path);
        }
    }
}

#[test]
fn all_examples_emit_cleanly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("examples");
    let mut files = Vec::new();
    collect_kei_files(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty());

    for path in &files {
        let src = fs::read_to_string(path).expect("readable example");
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let out = kei_emit::emit_module(&rel, &src).unwrap_or_else(|d| {
            panic!(
                "{rel}: examples must transpile cleanly: {}",
                serde_json::to_string_pretty(&d).expect("serializable")
            )
        });
        assert!(out.ts.contains("//# sourceMappingURL="));
        assert!(!out.map.is_empty());
    }
}
