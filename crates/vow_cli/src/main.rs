//! `vow` CLI。check / fmt / build / test のサブコマンドは M0 以降で実装する。
//! 言語処理ロジックは持たず、Diagnostic の散文整形と引数解釈・ファイル IO のみを担う。

use vow_check as _;
use vow_emit as _;
use vow_fmt as _;
use vow_syntax as _;

fn main() {
    eprintln!("vow: not yet implemented (see docs/vow-roadmap-goals.md)");
    std::process::exit(1);
}
