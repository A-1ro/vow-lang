//! ファイルシステム経由の `ModuleResolver` 実装(M20 / #55)。
//!
//! - 入力ファイル `<F>` の `module a.b.c` 宣言からプロジェクト root を逆算する。
//!   `<F>` の親を `path.len()` 段遡って root とし、`a/b/c.kei` を一意に解決する。
//! - 循環 import と再解決を避けるため `visiting` セットと `cache` を持つ。
//! - 解決中に対象モジュールがパースエラーを起こした場合は `None`(opaque)に
//!   倒し、consumer の検査をブロックしない(致命的でない健全性ギャップは
//!   既定挙動と同じ "opaque" 段階移行)。
//!
//! 副作用(fs / parse)を kei_check の外に押し出す境界。kei_check は
//! [`kei_check::ModuleResolver`] トレイトだけを知る(ARCHITECTURE.md)。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use kei_check::imports::{module_type_defs, ModuleResolver, ResolvedModule};
use kei_syntax::ast;

pub struct FsModuleResolver {
    root: PathBuf,
    cache: RefCell<HashMap<Vec<String>, Option<ResolvedModule>>>,
    visiting: RefCell<HashSet<Vec<String>>>,
}

impl FsModuleResolver {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            cache: RefCell::new(HashMap::new()),
            visiting: RefCell::new(HashSet::new()),
        }
    }
}

impl ModuleResolver for FsModuleResolver {
    fn resolve(&self, path: &[String]) -> Option<ResolvedModule> {
        let key: Vec<String> = path.to_vec();
        if let Some(cached) = self.cache.borrow().get(&key) {
            return cached.clone();
        }
        // 循環ガード: 解決中に同じ path を再要求されたら None を返す
        // (深いネストを許す。AST の型定義抽出は今回 transitive を辿らないため、
        // 実害は出ない)。
        if !self.visiting.borrow_mut().insert(key.clone()) {
            return None;
        }

        let mut file = self.root.clone();
        for seg in path {
            file.push(seg);
        }
        file.set_extension("kei");

        let resolved = std::fs::read_to_string(&file)
            .ok()
            .and_then(|src| {
                let parsed = kei_syntax::parse_module(&src);
                if !parsed.errors.is_empty() {
                    return None;
                }
                Some(parsed.module)
            })
            .map(|m| ResolvedModule {
                path: path.to_vec(),
                type_defs: module_type_defs(&m),
            });

        self.visiting.borrow_mut().remove(&key);
        self.cache.borrow_mut().insert(key, resolved.clone());
        resolved
    }
}

/// `module a.b.c` 宣言と入力ファイルパスから project root を逆算する。
/// `<root>/a/b/c.kei` 規約に従わないファイル(`module` 宣言が無い / 段数が合わない /
/// 親が足りない / ファイル配置が宣言と食い違う)では `None` を返し、CLI は
/// resolver 無しで従来通り opaque な検査にフォールバックする。
///
/// 「規約に従わないファイル」を検出するため、以下を順に確認:
///
/// 1. `module` 宣言があり、`path.len() >= 1`。
/// 2. ファイル名(stem)が `path` の **最後のセグメント** と一致する。
/// 3. ファイルの祖先ディレクトリ名が `path` の **逆順** と一致する
///    (`module a.b.c` のファイルは `.../a/b/c.kei` に置かれている)。
///
/// 2 / 3 を満たさないと、root を黙って間違って算出してしまい、resolver が
/// 関係ないファイルを開く危険があるので、明示的に `None` を返す。
pub fn derive_root(file: &Path, module: &ast::Module) -> Option<PathBuf> {
    let decl = module.decl.as_ref()?;
    let segments: Vec<&str> = decl.path.iter().map(|i| i.name.as_str()).collect();
    if segments.is_empty() {
        return None;
    }
    // ファイル名 stem が `path` の末尾と一致するか。
    let stem = file.file_stem().and_then(|s| s.to_str())?;
    if stem != *segments.last().unwrap() {
        return None;
    }
    // 親ディレクトリ名が `path` 中間セグメントを逆順で辿るか確認しつつ root を取り出す。
    let mut p = file.to_path_buf();
    if !p.pop() {
        return None;
    }
    for seg in segments.iter().rev().skip(1) {
        let dir_name = p.file_name().and_then(|s| s.to_str())?;
        if dir_name != *seg {
            return None;
        }
        if !p.pop() {
            return None;
        }
    }
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kei_syntax::ast;

    fn module_with_decl(path: &[&str]) -> ast::Module {
        let src = format!("module {}\n", path.join("."));
        let parsed = kei_syntax::parse_module(&src);
        assert!(parsed.errors.is_empty());
        parsed.module
    }

    #[test]
    fn derive_root_pops_segments_for_matching_layout() {
        let m = module_with_decl(&["a", "b", "c"]);
        let root = derive_root(Path::new("project/a/b/c.kei"), &m).expect("should derive");
        assert_eq!(root, PathBuf::from("project"));
    }

    #[test]
    fn derive_root_rejects_when_file_stem_does_not_match_last_segment() {
        // `module x.y` だが `y` でなく `irregular.kei` に置かれている。
        let m = module_with_decl(&["x", "y"]);
        assert!(derive_root(Path::new("project/x/irregular.kei"), &m).is_none());
    }

    #[test]
    fn derive_root_rejects_when_directory_does_not_match_intermediate_segment() {
        // `module a.b.c` だがファイルが `a/different/c.kei` に置かれている。
        let m = module_with_decl(&["a", "b", "c"]);
        assert!(derive_root(Path::new("project/a/different/c.kei"), &m).is_none());
    }

    #[test]
    fn derive_root_returns_none_when_module_decl_missing() {
        let parsed = kei_syntax::parse_module("");
        assert!(parsed.errors.is_empty());
        assert!(derive_root(Path::new("anything.kei"), &parsed.module).is_none());
    }
}
