//! ビルド時埋め込みの検証(ARCHITECTURE.md 不変条件 2)。
//!
//! 「spec/ と examples/ の内容がビルド時にサーバーへ埋め込まれ、spec 更新→
//! 再ビルドで応答が変わる」を保証するためのテスト。埋め込みは `include_str!`
//! でファイル内容を焼き込むため、spec/ を編集して再ビルドすると埋め込み内容が
//! 変わり、`kei_spec` / `kei_examples` の応答も必ず変わる。`include_str!` は
//! 対象ファイルを rustc の依存に登録するので、`cargo test` は spec 編集後に
//! 必ず kei_mcp を再コンパイルしてからこのテストを走らせる。
//!
//! 本テストは「埋め込み内容 == いまのディスク上の内容」を表明することで、
//! 上記パイプラインが陳腐化・乖離していないことを契約として固定する。
//! 埋め込みが古ければ(再ビルドで応答が変わらなければ)このテストが落ちる。

use std::fs;
use std::path::{Path, PathBuf};

use kei_mcp::{embedded, Server};
use serde_json::{json, Value};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `base` 直下の拡張子 `ext` ファイルを `(相対パス, 内容)` で集める(パス昇順)。
fn read_tree(base: &Path, ext: &str) -> Vec<(String, String)> {
    fn walk(dir: &Path, base: &Path, ext: &str, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).expect("readable dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, base, ext, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                let rel = path
                    .strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .replace('\\', "/");
                let content = fs::read_to_string(&path).expect("readable file");
                out.push((rel, content));
            }
        }
    }
    let mut out = Vec::new();
    walk(base, base, ext, &mut out);
    out.sort();
    out
}

#[test]
fn embedded_spec_matches_disk() {
    let disk = read_tree(&workspace_root().join("spec"), "md");
    let embedded: Vec<(String, String)> = embedded::spec_files()
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect();
    assert_eq!(
        embedded, disk,
        "embedded spec/ drifted from disk — rebuild must re-embed spec/ verbatim"
    );
}

#[test]
fn embedded_examples_match_disk() {
    let disk = read_tree(&workspace_root().join("examples"), "kei");
    let embedded: Vec<(String, String)> = embedded::example_files()
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect();
    assert_eq!(
        embedded, disk,
        "embedded examples/ drifted from disk — rebuild must re-embed examples/ verbatim"
    );
}

/// 応答が埋め込み=ディスク内容そのものを配信していることの End-to-End 確認。
/// spec を編集して再ビルドすれば、このエラーコード解説の応答テキストも変わる。
#[test]
fn kei_spec_serves_current_error_doc() {
    let server = Server::new();
    let disk = fs::read_to_string(workspace_root().join("spec/errors/KEI-E3001.md"))
        .expect("error doc exists");

    let request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "kei_spec", "arguments": { "topic": "KEI-E3001" } }
    });
    let response = server.handle(&request).expect("kei_spec responds");
    let text = served_text(&response);

    assert_eq!(
        text, disk,
        "kei_spec must serve the on-disk error doc verbatim"
    );
    assert!(!served_is_error(&response));
}

/// 応答が埋め込み=ディスクの example 内容を配信していることの確認。
#[test]
fn kei_examples_serves_current_example_body() {
    let server = Server::new();
    let disk = fs::read_to_string(workspace_root().join("examples/contracts/withdraw.kei"))
        .expect("example exists");

    let request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "kei_examples", "arguments": { "query": "withdraw" } }
    });
    let response = server.handle(&request).expect("kei_examples responds");
    let text = served_text(&response);

    assert!(
        text.contains(disk.trim_end()),
        "kei_examples must serve the on-disk example body verbatim"
    );
}

fn served_text(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .expect("text content")
        .to_string()
}

fn served_is_error(response: &Value) -> bool {
    response["result"]["isError"].as_bool().unwrap_or(false)
}
