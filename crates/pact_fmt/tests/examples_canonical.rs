//! examples/ 配下の全 .pact が正規形(`fmt(src) == src`)であることの保証。
//! 「Milestone 完了ごとに pact fmt を全コードベースへ適用する」不変条件
//! (CLAUDE.md)を機械検証に落としたもの。

use std::fs;
use std::path::{Path, PathBuf};

fn collect_pact_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_pact_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "pact") {
            out.push(path);
        }
    }
}

#[test]
fn examples_are_canonical() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files = Vec::new();
    collect_pact_files(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .pact files in {}", dir.display());

    for path in &files {
        let src = fs::read_to_string(path).expect("readable example");
        let formatted = pact_fmt::format_source(&src)
            .unwrap_or_else(|e| panic!("{}: must parse cleanly: {e:?}", path.display()));
        assert_eq!(
            src,
            formatted,
            "{}: example is not in canonical form; run pact fmt",
            path.display()
        );
    }
}
