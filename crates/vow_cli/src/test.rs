//! `vow test` のランナー。dev ビルド(契約 on)→ プロジェクトの `npm test` への委譲。
//!
//! ARCHITECTURE.md / docs/vow-roadmap-goals.md M7 事前合意:
//! v0.1 の Vow に test 構文は無い。`vow test` は「dev ビルド → Node のテストランナー」
//! の薄いラッパーであり、テストランナーの知識を持たない。具体的には:
//!
//! 1. `<dir>` を dev ビルド(契約 on = 既定 emit。source map も付ける)して `<dir>/dist/` へ。
//! 2. プロジェクトの `npm test`(package.json の test スクリプト)に委譲する。
//!
//! ランナー選定(vitest 等)・依存解決はプロジェクト側の責務(spec §8「やらない」)。
//!
//! dev ビルドで契約が TS に焼き込まれるため、`requires` 違反は実行時に
//! `VowContractViolation`(@vow/runtime)として送出され、捕捉しないテストは失敗 →
//! `npm test` が非ゼロ終了 → `vow test` もそれを写して非ゼロ終了する。

use std::path::Path;
use std::process::Command;

use crate::build;
use crate::cli::UsageError;

/// `vow test`。テスト全件パスで 0、テスト失敗/ビルドエラーで 1、
/// ディレクトリ不正や `npm` 起動不可は使用法エラー。
pub fn run(dir: &Path) -> Result<u8, UsageError> {
    if !dir.is_dir() {
        return Err(UsageError(format!(
            "test: '{}' is not a directory",
            dir.display()
        )));
    }
    if !dir.join("package.json").is_file() {
        return Err(UsageError(format!(
            "test: '{}' has no package.json (vow test delegates to the project's `npm test`)",
            dir.display()
        )));
    }

    // 1. dev ビルド(契約 on)。エラーがあれば build が全 Diagnostic を出して 1 を返す。
    let dist = dir.join("dist");
    let build_code = build::run(dir, Some(&dist), true)?;
    if build_code != 0 {
        return Ok(build_code);
    }

    // 2. プロジェクトの `npm test` に委譲。子の stdout/stderr は継承して利用者に
    //    そのまま見せ、終了コードだけを 0/1 に写す(環境変数も継承する)。
    eprintln!("vow test: running `npm test` in {}", dir.display());
    let status = Command::new("npm")
        .args(["test", "--silent"])
        .current_dir(dir)
        .status()
        .map_err(|e| {
            UsageError(format!(
                "test: cannot run `npm test` in {} ({e}); is Node installed?",
                dir.display()
            ))
        })?;

    Ok(u8::from(!status.success()))
}
