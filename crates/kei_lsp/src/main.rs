//! Kei 言語サーバー(LSP)の stdio エントリ。
//!
//! クライアント(VS Code 拡張など)と stdio で JSON-RPC を話し、
//! 検査結果(Diagnostics)とホバー(契約表示)を提供する。言語処理は
//! kei_check / kei_syntax / kei_fmt に委譲する薄いアダプタ(kei_lsp::server)。

use std::process::ExitCode;

fn main() -> ExitCode {
    match kei_lsp::run_stdio() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kei-lsp: {e}");
            ExitCode::FAILURE
        }
    }
}
