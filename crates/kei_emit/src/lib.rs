//! Kei → TypeScript トランスパイラ + source map(ARCHITECTURE.md: 検査の再実装禁止)。
//!
//! 入力ソースを kei_syntax でパースし、kei_check の検査がエラーゼロの場合のみ
//! TS を生成する。検査エラーがあれば Diagnostic をそのまま返す(独自エラー型を
//! 外部に漏らさない)。
//!
//! 出力 TS の形(M4 設計合意の記録は spec §5):
//! - 全関数は同期(非同期は v0.2 以降で `uses Async` として検討)
//! - `record` → readonly object 型 + 同名ファクトリ関数
//! - `enum` → `kind` 判別の tagged union + 同名コンストラクタ集
//! - tagged 型 → branded type(`__keiTag`)+ 同名コンストラクタ関数
//! - `requires` / `ensures` → `KeiContractViolation`(@kei/runtime)を投げる
//!   実行時アサーション。`ensures` は本体を IIFE に包んで戻り値を検査する
//! - `uses` は doc コメントとして残す(spec §5)

mod emit;
mod sourcemap;

use kei_check::Diagnostic;

/// 1 モジュールのトランスパイル結果。
#[derive(Debug, Clone)]
pub struct EmitOutput {
    /// 生成 TypeScript(末尾に `//# sourceMappingURL=...` を含む)。
    pub ts: String,
    /// source map v3 の JSON。
    pub map: String,
    /// モジュール宣言から導出した出力先相対パス(例: `contracts/withdraw.ts`)。
    pub ts_path: String,
}

/// 1 ソースファイルをトランスパイルする。`file` はリポジトリルートからの相対パスで、
/// Diagnostic の span と source map の `sources` の双方に使われる。
pub fn emit_module(file: &str, source: &str) -> Result<EmitOutput, Vec<Diagnostic>> {
    let parsed = kei_syntax::parse_module(source);
    let mut diags = kei_check::syntax_diagnostics(file, &parsed.errors);
    diags.extend(kei_check::check_module(file, &parsed.module));
    if !diags.is_empty() {
        return Err(diags);
    }
    Ok(emit::emit_checked(file, source, &parsed.module))
}
