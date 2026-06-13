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
fn check_errors_reject_emit() {
    let err = kei_emit::emit_module("bad.kei", "func f() -> Int {\n  return missing\n}\n")
        .expect_err("undefined name must reject emit");
    assert!(err.iter().any(|d| d.code == "KEI-E1001"));
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
