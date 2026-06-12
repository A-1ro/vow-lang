//! examples/ 配下の全 .pact が構文・意味検査ともにエラーゼロであることの保証。
//! examples はトランスパイル(M4 e2e)と MCP 配信(M5)の素材であり、
//! 常に「コンパイルが通る見本」でなければならない。

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
fn examples_check_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("examples");
    let mut files = Vec::new();
    collect_pact_files(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .pact files in {}", dir.display());

    for path in &files {
        let src = fs::read_to_string(path).expect("readable example");
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let parsed = pact_syntax::parse_module(&src);
        let mut diags = pact_check::syntax_diagnostics(&rel, &parsed.errors);
        diags.extend(pact_check::check_module(&rel, &parsed.module));
        assert!(
            diags.is_empty(),
            "{rel}: examples must check cleanly, got:\n{}",
            serde_json::to_string_pretty(&diags).expect("serializable diagnostics")
        );
    }
}
