//! Kei MCP サーバー。spec/ と examples/ をビルド時に埋め込み(ARCHITECTURE.md
//! 不変条件 2)、エージェント向けの取扱説明書として配信する。
//!
//! 言語処理ロジックは持たず、検査・整形は kei_check / kei_fmt / kei_syntax に
//! 委譲する。プロトコル処理は [`Server::handle`] が担い、stdio トランスポートは
//! [`run_stdio`] が包む。起動経路は単一で、`kei-mcp` バイナリ(`src/main.rs`)も
//! `kei mcp` サブコマンド(kei_cli)も同じ [`run_stdio`] を呼ぶ。

use std::io::{self, BufRead, Write};

use serde_json::Value;

pub mod embedded;
pub mod server;
pub mod tools;

pub use server::Server;

/// stdio トランスポートで MCP サーバーを駆動する。改行区切り JSON-RPC を 1 行ずつ
/// 読み、[`Server::handle`] に渡し、応答(通知は無し)を 1 行ずつ書き戻す。stdin が
/// 閉じる(EOF)まで処理を続け、正常終了する。`kei-mcp` バイナリと `kei mcp`
/// サブコマンドの双方がこの単一エントリを共有する。
pub fn run_stdio() -> io::Result<()> {
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
