//! `kei mcp`: MCP サーバーを stdio で起動する。サーバー本体は kei_mcp に委譲し、
//! ここは配線だけを持つ(ARCHITECTURE.md: kei_cli は言語処理ロジックを持たない)。

/// MCP サーバーを起動する。stdin が閉じる(EOF)まで JSON-RPC を処理し、正常終了で
/// `0`。stdio の IO 失敗はサーバー異常として一行報告し、終了コード `1` に写す。
pub fn run() -> u8 {
    match kei_mcp::run_stdio() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("kei: mcp server error: {e}");
            1
        }
    }
}
