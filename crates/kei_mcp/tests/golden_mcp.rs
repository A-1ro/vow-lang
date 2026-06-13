//! tests/mcp/ の MCP 統合 golden test ランナー(契約本文は fixture 側)。
//!
//! 各ケースは `{name}.request.json`(単一 JSON-RPC リクエスト)と
//! `{name}.response.json`(期待レスポンス)のペア。[`kei_mcp::Server::handle`]
//! にリクエストを渡し、返ったレスポンス Value を期待 JSON と構造比較する
//! (プロセス起動なしでリクエスト→レスポンスを検証)。
//!
//! 期待ファイルの再生成: `UPDATE_GOLDEN=1 cargo test -p kei_mcp --test golden_mcp`
//! (golden の変更は人間レビュー必須 — ARCHITECTURE.md 不変条件 3)

use std::fs;
use std::path::{Path, PathBuf};

use kei_mcp::Server;
use serde_json::Value;

fn mcp_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mcp")
}

fn case_names() -> Vec<String> {
    let dir = mcp_dir();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("readable dir entry").path();
            let name = path.file_name()?.to_str()?;
            name.strip_suffix(".request.json").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

#[test]
fn golden_mcp() {
    let dir = mcp_dir();
    let cases = case_names();
    assert!(!cases.is_empty(), "no fixtures in {}", dir.display());

    let server = Server::new();
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    for name in &cases {
        let req_text =
            fs::read_to_string(dir.join(format!("{name}.request.json"))).expect("readable request");
        let request: Value = serde_json::from_str(&req_text)
            .unwrap_or_else(|e| panic!("{name}.request.json is not valid JSON: {e}"));

        let actual = match server.handle(&request) {
            Some(response) => response,
            None => {
                failures.push(format!(
                    "{name}: request produced no response (notification?); fixtures must be requests"
                ));
                continue;
            }
        };

        let expected_path = dir.join(format!("{name}.response.json"));
        if update {
            let mut text = serde_json::to_string_pretty(&actual).expect("serializable");
            text.push('\n');
            fs::write(&expected_path, text).expect("writable response file");
            continue;
        }

        let expected_text = match fs::read_to_string(&expected_path) {
            Ok(text) => text,
            Err(e) => {
                failures.push(format!("{name}: missing {name}.response.json ({e})"));
                continue;
            }
        };
        let expected: Value =
            serde_json::from_str(&expected_text).expect("response fixture is valid JSON");

        if actual != expected {
            failures.push(format!(
                "{name}: response differs from {name}.response.json\n--- actual ---\n{}",
                serde_json::to_string_pretty(&actual).expect("serializable")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} MCP golden case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// goal のカバー範囲検証: 4 ツールがちょうど tools/list に出ること、
/// 各ツールに inputSchema があること。
#[test]
fn tools_list_exposes_four_tools() {
    let server = Server::new();
    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list"
    });
    let response = server.handle(&request).expect("tools/list responds");
    let tools = response["result"]["tools"]
        .as_array()
        .expect("tools is an array");

    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("tool name"))
        .collect();
    assert_eq!(
        names,
        vec!["kei_spec", "kei_check", "kei_fmt", "kei_examples"],
        "tools/list must expose exactly the four M5 tools in order"
    );

    for tool in tools {
        assert!(
            tool["inputSchema"]["type"] == "object",
            "tool {} must declare an object inputSchema",
            tool["name"]
        );
    }
}

/// 通知(id なし)には応答しないこと。
#[test]
fn notifications_get_no_response() {
    let server = Server::new();
    let request = serde_json::json!({
        "jsonrpc": "2.0", "method": "notifications/initialized"
    });
    assert!(server.handle(&request).is_none());
}
