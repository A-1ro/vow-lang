//! Kei MCP サーバーの stdio エントリ。改行区切り JSON-RPC を 1 行ずつ読み、
//! [`Server::handle`] に渡し、応答(通知は無し)を 1 行ずつ書き戻す。

use std::io::{self, BufRead, Write};

use kei_mcp::Server;
use serde_json::Value;

fn main() -> io::Result<()> {
    let server = Server::new();
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => server.handle(&request),
            Err(e) => Some(parse_error(&e.to_string())),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut out, &response)?;
            out.write_all(b"\n")?;
            out.flush()?;
        }
    }
    Ok(())
}

fn parse_error(message: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": { "code": -32700, "message": format!("Parse error: {message}") },
    })
}
