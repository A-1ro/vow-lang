//! import 境界の型定義解決(M20 / #55)。
//!
//! `import a.b { X }` で持ち込んだ型を **対象モジュールを解決して** 検査する
//! ための公開境界。kei_check 本体は副作用フリーを保ち、ファイル読込やパースは
//! [`ModuleResolver`] の実装(例: `kei_cli::FsModuleResolver`)に委譲する。
//!
//! 解決成果は [`ResolvedModule`] に詰める。consumer 側の `Env::build` は
//! 結果に応じて import 名を `NameKind::Record`/`Enum`/`Alias` に昇格し、
//! 既存の型検査経路に乗せる。解決不能(`resolve` が `None` を返す)は
//! 従来通り `NameKind::Import` の opaque 扱い(M20 既定挙動)。

use std::collections::HashMap;

use kei_syntax::ast;

use crate::types::Ty;

/// 解決済みのバリアント表現(`Env::VariantDef` の公開ミラー)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedVariant {
    Unit,
    Tuple(Vec<Ty>),
    Record(Vec<(String, Ty)>),
}

/// 解決済みの型定義(record / enum / type alias)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTypeDef {
    Record(Vec<(String, Ty)>),
    Enum(Vec<(String, ResolvedVariant)>),
    Alias(Ty),
}

/// 解決された 1 モジュール。`path` は `module a.b.c` の各セグメント、
/// `type_defs` は名前 → 型定義の表(関数・extern は含めない)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModule {
    pub path: Vec<String>,
    pub type_defs: HashMap<String, ResolvedTypeDef>,
}

/// import 先のモジュール解決インターフェース。実装は kei_cli が
/// `FsModuleResolver`(ファイルシステム読込+パース)を提供する。
/// kei_check のテストは HashMap-backed の fake で代用する。
pub trait ModuleResolver {
    fn resolve(&self, path: &[String]) -> Option<ResolvedModule>;
}

/// 何も解決しないリゾルバ(既存の単一ファイル検査と等価)。
pub struct NoopResolver;

impl ModuleResolver for NoopResolver {
    fn resolve(&self, _path: &[String]) -> Option<ResolvedModule> {
        None
    }
}

/// パース済みモジュールから型定義テーブルを抽出する。`FsModuleResolver` が
/// 対象 .kei を `parse_module` した直後に呼ぶ想定。Diagnostic は出さない
/// (対象モジュール自体の検査は別経路。ここは「他モジュールから引ける形」
/// に変換するだけ)。
///
/// 制限: 対象モジュール内の **import** はここでは追跡しない。対象モジュール
/// 内の `import` 由来の型参照は `Ty::Unknown` に倒す(深い検査は別途)。
pub fn module_type_defs(module: &ast::Module) -> HashMap<String, ResolvedTypeDef> {
    let local_records: Vec<&str> = module
        .items
        .iter()
        .filter_map(|i| match i {
            ast::Item::Record(r) => Some(r.name.name.as_str()),
            _ => None,
        })
        .collect();
    let local_enums: Vec<&str> = module
        .items
        .iter()
        .filter_map(|i| match i {
            ast::Item::Enum(e) => Some(e.name.name.as_str()),
            _ => None,
        })
        .collect();
    let local_aliases: HashMap<&str, &ast::TypeAlias> = module
        .items
        .iter()
        .filter_map(|i| match i {
            ast::Item::TypeAlias(a) => Some((a.name.name.as_str(), a)),
            _ => None,
        })
        .collect();

    // alias の循環ガード付き解決(対象モジュール内のみ)。
    let mut alias_tys: HashMap<String, Ty> = HashMap::new();
    let mut visiting: Vec<String> = Vec::new();
    for name in local_aliases.keys() {
        ty_of_alias(
            name,
            &local_records,
            &local_enums,
            &local_aliases,
            &mut alias_tys,
            &mut visiting,
        );
    }

    let mut defs = HashMap::new();
    for item in &module.items {
        match item {
            ast::Item::Record(r) => {
                let fields: Vec<(String, Ty)> = r
                    .fields
                    .iter()
                    .map(|f| {
                        let mut visiting = Vec::new();
                        (
                            f.name.name.clone(),
                            ty_of(
                                &f.ty,
                                &local_records,
                                &local_enums,
                                &local_aliases,
                                &mut alias_tys,
                                &mut visiting,
                            ),
                        )
                    })
                    .collect();
                defs.insert(r.name.name.clone(), ResolvedTypeDef::Record(fields));
            }
            ast::Item::Enum(e) => {
                let variants: Vec<(String, ResolvedVariant)> = e
                    .variants
                    .iter()
                    .map(|v| {
                        let rv = match &v.payload {
                            ast::VariantPayload::Unit => ResolvedVariant::Unit,
                            ast::VariantPayload::Tuple { types } => ResolvedVariant::Tuple(
                                types
                                    .iter()
                                    .map(|t| {
                                        let mut visiting = Vec::new();
                                        ty_of(
                                            t,
                                            &local_records,
                                            &local_enums,
                                            &local_aliases,
                                            &mut alias_tys,
                                            &mut visiting,
                                        )
                                    })
                                    .collect(),
                            ),
                            ast::VariantPayload::Record { fields } => ResolvedVariant::Record(
                                fields
                                    .iter()
                                    .map(|f| {
                                        let mut visiting = Vec::new();
                                        (
                                            f.name.name.clone(),
                                            ty_of(
                                                &f.ty,
                                                &local_records,
                                                &local_enums,
                                                &local_aliases,
                                                &mut alias_tys,
                                                &mut visiting,
                                            ),
                                        )
                                    })
                                    .collect(),
                            ),
                        };
                        (v.name.name.clone(), rv)
                    })
                    .collect();
                defs.insert(e.name.name.clone(), ResolvedTypeDef::Enum(variants));
            }
            ast::Item::TypeAlias(a) => {
                let ty = alias_tys.get(&a.name.name).cloned().unwrap_or(Ty::Unknown);
                defs.insert(a.name.name.clone(), ResolvedTypeDef::Alias(ty));
            }
            _ => {}
        }
    }
    defs
}

fn ty_of_alias(
    name: &str,
    records: &[&str],
    enums: &[&str],
    aliases: &HashMap<&str, &ast::TypeAlias>,
    alias_tys: &mut HashMap<String, Ty>,
    visiting: &mut Vec<String>,
) -> Ty {
    if let Some(cached) = alias_tys.get(name) {
        return cached.clone();
    }
    if visiting.iter().any(|v| v == name) {
        return Ty::Unknown;
    }
    let Some(a) = aliases.get(name) else {
        return Ty::Unknown;
    };
    visiting.push(name.to_string());
    let base = ty_of(&a.ty, records, enums, aliases, alias_tys, visiting);
    let ty = match &a.tag {
        Some(tag) => Ty::Tagged {
            name: tag.clone(),
            underlying: Box::new(base),
        },
        None => base,
    };
    visiting.pop();
    alias_tys.insert(name.to_string(), ty.clone());
    ty
}

fn ty_of(
    t: &ast::Type,
    records: &[&str],
    enums: &[&str],
    aliases: &HashMap<&str, &ast::TypeAlias>,
    alias_tys: &mut HashMap<String, Ty>,
    visiting: &mut Vec<String>,
) -> Ty {
    // 防御: path が空でも panic させない(エラー回復で起こりうる)。
    if t.path.is_empty() || t.path.len() > 1 {
        return Ty::Unknown;
    }
    let root = t.path[0].name.as_str();
    match root {
        "Int" if t.args.is_empty() => Ty::Int,
        "String" if t.args.is_empty() => Ty::Str,
        "Bool" if t.args.is_empty() => Ty::Bool,
        "Option" if t.args.len() == 1 => Ty::Option(Box::new(ty_of(
            &t.args[0], records, enums, aliases, alias_tys, visiting,
        ))),
        "List" if t.args.len() == 1 => Ty::List(Box::new(ty_of(
            &t.args[0], records, enums, aliases, alias_tys, visiting,
        ))),
        "Result" if t.args.len() == 2 => Ty::Result(
            Box::new(ty_of(
                &t.args[0], records, enums, aliases, alias_tys, visiting,
            )),
            Box::new(ty_of(
                &t.args[1], records, enums, aliases, alias_tys, visiting,
            )),
        ),
        r if records.contains(&r) => Ty::Record(r.to_string()),
        r if enums.contains(&r) => Ty::Enum(r.to_string()),
        // **呼び出し側の `visiting` を共有** して循環 alias を断つ。fresh vec を
        // 作っていた以前の実装は `type A = B / type B = A` で無限再帰した。
        r if aliases.contains_key(r) => {
            ty_of_alias(r, records, enums, aliases, alias_tys, visiting)
        }
        _ => Ty::Unknown,
    }
}
