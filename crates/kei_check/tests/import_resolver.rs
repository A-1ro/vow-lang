//! M20: import 境界の型解決の挙動検査。HashMap-backed の fake resolver で、
//! resolver があるときと無いときで `import` 名の検査が切り替わることを確認する。
//!
//! `kei_cli` の `FsModuleResolver` は別途 CLI 統合テストで覆う。ここでは
//! resolver の有無による check 経路の差だけに焦点を絞る。

use std::collections::HashMap;

use kei_check::imports::{
    ModuleResolver, NoopResolver, ResolvedModule, ResolvedTypeDef, ResolvedVariant,
};
use kei_check::types::Ty;
use kei_check::{check_module_with, check_module_with_resolver, CheckOptions};

struct FakeResolver {
    modules: HashMap<Vec<String>, ResolvedModule>,
}

impl FakeResolver {
    fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }
    fn insert(mut self, path: &[&str], type_defs: Vec<(&str, ResolvedTypeDef)>) -> Self {
        let path: Vec<String> = path.iter().map(|s| s.to_string()).collect();
        let type_defs: HashMap<String, ResolvedTypeDef> = type_defs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        self.modules
            .insert(path.clone(), ResolvedModule { path, type_defs });
        self
    }
}

impl ModuleResolver for FakeResolver {
    fn resolve(&self, path: &[String]) -> Option<ResolvedModule> {
        self.modules.get(path).cloned()
    }
}

fn parse(src: &str) -> kei_syntax::ast::Module {
    let parsed = kei_syntax::parse_module(src);
    assert!(
        parsed.errors.is_empty(),
        "test source should parse cleanly: {:?}",
        parsed.errors
    );
    parsed.module
}

/// import した record のフィールド名タイプミスが KEI-E2002 で検出されること。
#[test]
fn imported_record_unknown_field_is_detected() {
    let src = "module t.consumer\n\
               import t.product { Product }\n\
               func bad(p: Product) -> Int {\n\
                   return p.nonexistentField\n\
               }\n";
    let module = parse(src);

    // 既定(NoopResolver 経由)では従来通り素通り(opaque)。
    let diags = check_module_with("consumer.kei", &module, CheckOptions::default());
    assert!(
        diags.is_empty(),
        "without resolver, import is opaque and should not raise diagnostics; got {diags:?}"
    );

    // FakeResolver で Product を解決すると、存在しないフィールドが検出される。
    let resolver = FakeResolver::new().insert(
        &["t", "product"],
        vec![(
            "Product",
            ResolvedTypeDef::Record(vec![("id".to_string(), Ty::Int)]),
        )],
    );
    let diags =
        check_module_with_resolver("consumer.kei", &module, CheckOptions::default(), &resolver);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E2002"),
        "expected KEI-E2002 for unknown field on imported record; got {codes:?}"
    );
}

/// import した record のフィールド型が伝播して KEI-E2001 が出ること。
#[test]
fn imported_record_field_type_propagates() {
    let src = "module t.consumer\n\
               import t.product { Product }\n\
               func bad(p: Product) -> Bool {\n\
                   return p.id\n\
               }\n";
    let module = parse(src);

    let resolver = FakeResolver::new().insert(
        &["t", "product"],
        vec![(
            "Product",
            ResolvedTypeDef::Record(vec![("id".to_string(), Ty::Int)]),
        )],
    );
    let diags =
        check_module_with_resolver("consumer.kei", &module, CheckOptions::default(), &resolver);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E2001"),
        "expected KEI-E2001 for Int returned where Bool expected; got {codes:?}"
    );
}

/// import した enum の match 非網羅が検出される(KEI-E2007)。
#[test]
fn imported_enum_match_non_exhaustive_is_detected() {
    let src = "module t.consumer\n\
               import t.status { Status }\n\
               func bad(s: Status) -> Int {\n\
                   return match s {\n\
                       Status.Active => 1\n\
                   }\n\
               }\n";
    let module = parse(src);

    let resolver = FakeResolver::new().insert(
        &["t", "status"],
        vec![(
            "Status",
            ResolvedTypeDef::Enum(vec![
                ("Active".to_string(), ResolvedVariant::Unit),
                ("Closed".to_string(), ResolvedVariant::Unit),
            ]),
        )],
    );
    let diags =
        check_module_with_resolver("consumer.kei", &module, CheckOptions::default(), &resolver);
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E2007"),
        "expected KEI-E2007 for non-exhaustive match on imported enum; got {codes:?}"
    );
}

/// resolver が見つからない import は従来通り opaque を維持する。
#[test]
fn unknown_import_remains_opaque() {
    let src = "module t.consumer\n\
               import t.unknown { Thing }\n\
               func bad(p: Thing) -> Int {\n\
                   return p.whatever\n\
               }\n";
    let module = parse(src);

    let resolver = FakeResolver::new();
    let diags =
        check_module_with_resolver("consumer.kei", &module, CheckOptions::default(), &resolver);
    assert!(
        diags.is_empty(),
        "unresolved imports must stay opaque (no diagnostics emitted); got {diags:?}"
    );

    // NoopResolver でも同じ挙動。
    let diags = check_module_with_resolver(
        "consumer.kei",
        &module,
        CheckOptions::default(),
        &NoopResolver,
    );
    assert!(diags.is_empty(), "NoopResolver should also stay opaque");
}

/// import で解決された型名と同名のローカル定義は KEI-E1004(IMPORT_CONFLICT)で
/// 報告される(PR #74 review)。M20 以前は import が `NameKind::Import` だけだったが、
/// 解決後は `Record/Enum/Alias` になるため、出自(import か local か)を保持しないと
/// DUPLICATE_DEF に流れ込んでしまう。
#[test]
fn imported_name_conflicts_with_local_definition_as_import_conflict() {
    let src = "module t.consumer\n\
               import t.product { Product }\n\
               record Product { id: Int }\n";
    let module = parse(src);

    let resolver = FakeResolver::new().insert(
        &["t", "product"],
        vec![(
            "Product",
            ResolvedTypeDef::Record(vec![("id".to_string(), Ty::Int)]),
        )],
    );
    let diags = kei_check::check_module_with_resolver(
        "consumer.kei",
        &module,
        kei_check::CheckOptions::default(),
        &resolver,
    );
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"KEI-E1004"),
        "expected KEI-E1004 (IMPORT_CONFLICT) when a local record shadows a resolved import; got {codes:?}"
    );
    assert!(
        !codes.contains(&"KEI-E1003"),
        "must not fall back to DUPLICATE_DEF for import-vs-local; got {codes:?}"
    );
}

/// `module_type_defs` は循環 alias で **無限再帰せず** 終了する(PR #74 review)。
/// `ty_of` 内の `aliases.contains_key(r)` 分岐で fresh `visiting` を作っていた
/// 旧実装は `type A = B / type B = A` でスタックオーバーフローしていた。
#[test]
fn module_type_defs_handles_cyclic_aliases_without_overflow() {
    let src = "module t.cyclic\n\
               type A = B\n\
               type B = A\n";
    let module = parse(src);
    // 終了することが第一目標(panic / stack overflow ではなく値を返すこと)。
    let defs = kei_check::module_type_defs(&module);
    // 循環は `Unknown` に倒される(`Alias(Ty::Unknown)` で抽出される)。
    assert!(matches!(
        defs.get("A"),
        Some(ResolvedTypeDef::Alias(Ty::Unknown))
    ));
    assert!(matches!(
        defs.get("B"),
        Some(ResolvedTypeDef::Alias(Ty::Unknown))
    ));
}

/// `module_type_defs` がローカル定義を一通り正しく抽出することを確認する。
#[test]
fn module_type_defs_extracts_records_enums_aliases() {
    let src = "module t.product\n\
               type ProductId = Int tagged \"ProductId\"\n\
               record Product {\n\
                 id: ProductId\n\
                 name: String\n\
               }\n\
               enum Status {\n\
                 Active\n\
                 Closed { reason: String }\n\
               }\n";
    let module = parse(src);
    let defs = kei_check::module_type_defs(&module);
    assert!(defs.contains_key("ProductId"), "alias missing: {defs:?}");
    assert!(defs.contains_key("Product"), "record missing: {defs:?}");
    assert!(defs.contains_key("Status"), "enum missing: {defs:?}");
    match defs.get("Product") {
        Some(ResolvedTypeDef::Record(fields)) => {
            let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
            assert_eq!(names, vec!["id", "name"]);
        }
        other => panic!("unexpected Product def: {other:?}"),
    }
    match defs.get("Status") {
        Some(ResolvedTypeDef::Enum(variants)) => {
            let names: Vec<&str> = variants.iter().map(|(n, _)| n.as_str()).collect();
            assert_eq!(names, vec!["Active", "Closed"]);
        }
        other => panic!("unexpected Status def: {other:?}"),
    }
}
