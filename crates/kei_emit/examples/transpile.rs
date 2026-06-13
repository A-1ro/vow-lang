//! 単一 .kei ファイルを TypeScript に変換する開発用ツール(`kei build` の最小版)。
//!
//! 使い方:
//!   cargo run -p kei_emit --example transpile -- <input.kei> [output.ts]
//!
//! 検査(構文・型・エフェクト・契約)がエラーゼロのときだけ TS を出力する。
//! エラーがあれば Diagnostic[] を JSON で stderr に出して終了コード 1 で終わる。
//! output.ts を省略すると生成 TS を stdout に出す。指定すると output.ts と
//! output.ts.map(source map)を書き出す。

use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next() else {
        eprintln!("usage: transpile <input.kei> [output.ts]");
        return ExitCode::FAILURE;
    };
    let output = args.next();

    let source = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {input}: {e}");
            return ExitCode::FAILURE;
        }
    };

    match kei_emit::emit_module(&input, &source) {
        Ok(out) => match output {
            Some(path) => {
                let map_path = format!("{path}.map");
                if let Some(parent) = Path::new(&path).parent() {
                    let _ = fs::create_dir_all(parent);
                }
                fs::write(&path, &out.ts).expect("write TS");
                fs::write(&map_path, &out.map).expect("write source map");
                eprintln!("transpiled {input} -> {path} (+ {map_path})");
                ExitCode::SUCCESS
            }
            None => {
                print!("{}", out.ts);
                ExitCode::SUCCESS
            }
        },
        Err(diags) => {
            eprintln!(
                "{input}: check failed with {} diagnostic(s):\n{}",
                diags.len(),
                serde_json::to_string_pretty(&diags).expect("serializable diagnostics")
            );
            ExitCode::FAILURE
        }
    }
}
