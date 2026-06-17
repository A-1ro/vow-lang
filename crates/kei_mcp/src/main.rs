//! Kei MCP サーバーの stdio エントリ。起動ループ本体は [`kei_mcp::run_stdio`] に
//! あり、ここはそれを呼ぶだけ(`kei mcp` サブコマンドと共有する単一経路)。

use std::io;

fn main() -> io::Result<()> {
    kei_mcp::run_stdio()
}
