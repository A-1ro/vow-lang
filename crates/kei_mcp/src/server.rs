//! MCP (JSON-RPC 2.0) ディスパッチ。トランスポート(stdio)からは独立した
//! 純関数 [`Server::handle`] で、リクエスト Value → レスポンス Value を返す
//! (notification は `None`)。これにより tests/mcp/ の golden test が
//! プロセス起動なしでリクエスト→レスポンスを検証できる。

use serde_json::{json, Value};

use crate::tools::{self, ToolOutcome};

/// 対応する MCP プロトコルバージョン。
pub const PROTOCOL_VERSION: &str = "2024-11-05";
/// サーバー名(serverInfo)。
pub const SERVER_NAME: &str = "kei-mcp";
/// サーバーバージョン(Cargo パッケージ版数)。
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Kei MCP サーバー。状態は埋め込み静的データのみで、インスタンスは空。
#[derive(Debug, Default, Clone, Copy)]
pub struct Server;

impl Server {
    pub fn new() -> Self {
        Server
    }

    /// JSON-RPC リクエストを処理する。`id` を持つ通常リクエストは `Some(response)`、
    /// 通知(`id` なし)は `None` を返す。
    pub fn handle(&self, request: &Value) -> Option<Value> {
        let method = request.get("method").and_then(Value::as_str);

        // 通知(id なし)は応答しない。
        let id = request.get("id").cloned();
        id.as_ref()?;

        let method = match method {
            Some(m) => m,
            None => {
                return Some(error(id, -32600, "Invalid Request: missing 'method'"));
            }
        };
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        let response = match method {
            "initialize" => success(id, initialize_result()),
            "ping" => success(id, json!({})),
            "tools/list" => success(id, tools_list_result()),
            "tools/call" => tools_call(id, &params),
            other => error(id, -32601, &format!("Method not found: {other}")),
        };
        Some(response)
    }
}

fn tools_call(id: Option<Value>, params: &Value) -> Value {
    let name = match params.get("name").and_then(Value::as_str) {
        Some(n) => n,
        None => return error(id, -32602, "Invalid params: missing tool 'name'"),
    };
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let str_arg = |key: &str| args.get(key).and_then(Value::as_str);

    let outcome = match name {
        "kei_spec" => tools::run_spec(str_arg("topic").unwrap_or("")),
        "kei_check" => match str_arg("source") {
            Some(src) => tools::run_check(src),
            None => tools::missing_arg("source"),
        },
        "kei_fmt" => match str_arg("source") {
            Some(src) => tools::run_fmt(src),
            None => tools::missing_arg("source"),
        },
        "kei_examples" => tools::run_examples(str_arg("query").unwrap_or("")),
        other => tools::unknown_tool(other),
    };
    success(id, tool_result(&outcome))
}

fn tool_result(outcome: &ToolOutcome) -> Value {
    json!({
        "content": [ { "type": "text", "text": outcome.text } ],
        "isError": outcome.is_error,
    })
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
    })
}

/// tools/list の応答。spec §6.1 のツール定義と入力名(topic/source/query)に一致させる。
fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "kei_spec",
                "description": "Look up the Kei language spec. Pass `topic` as a section number (e.g. \"3\"), a heading keyword, or an error code (e.g. \"KEI-E3001\"); omit it for the index.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "topic": {
                            "type": "string",
                            "description": "Section number, heading keyword, or error code. Empty returns the index."
                        }
                    },
                    "required": [],
                    "additionalProperties": false
                }
            },
            {
                "name": "kei_check",
                "description": "Statically check Kei source (syntax + types + effects + contracts). Returns a Diagnostic[] JSON array, each with span, code, and at least one fix candidate.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "Kei source text to check." }
                    },
                    "required": ["source"],
                    "additionalProperties": false
                }
            },
            {
                "name": "kei_fmt",
                "description": "Format Kei source into canonical form. On a syntax error it does not reformat and returns the Diagnostic[] instead (isError).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "Kei source text to format." }
                    },
                    "required": ["source"],
                    "additionalProperties": false
                }
            },
            {
                "name": "kei_examples",
                "description": "Search Kei example snippets by keyword (matches path and body). Omit `query` to list all examples.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Keyword to search example paths and bodies. Empty lists all." }
                    },
                    "required": [],
                    "additionalProperties": false
                }
            }
        ]
    })
}

fn success(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message },
    })
}
