//! tests/golden/fmt/ の golden test ランナー(契約本文は fixture 側)。
//!
//! - `{name}.input.pact` を整形した結果が `{name}.expected.pact` と一致すること
//! - 期待出力そのものが正規形であること(冪等性)
//! - 整形前後で span を除く AST が一致すること(意味的変更の禁止)
//!
//! 期待ファイルの再生成: `UPDATE_GOLDEN=1 cargo test -p pact_fmt --test golden_fmt`
//! (golden の変更は人間レビュー必須 — ARCHITECTURE.md 不変条件 3)

mod util;

use std::fs;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/fmt")
}

#[test]
fn golden_fmt() {
    let dir = golden_dir();
    let mut cases: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("readable dir entry").path();
            let name = path.file_name()?.to_str()?;
            name.strip_suffix(".input.pact").map(str::to_string)
        })
        .collect();
    cases.sort();
    assert!(
        !cases.is_empty(),
        "no golden fixtures found in {}",
        dir.display()
    );

    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut failures = Vec::new();

    for name in &cases {
        let input =
            fs::read_to_string(dir.join(format!("{name}.input.pact"))).expect("readable input");
        let expected_path = dir.join(format!("{name}.expected.pact"));

        let actual = match pact_fmt::format_source(&input) {
            Ok(text) => text,
            Err(errors) => {
                failures.push(format!("{name}: input does not parse: {errors:?}"));
                continue;
            }
        };

        if update {
            fs::write(&expected_path, &actual).expect("writable expected file");
            continue;
        }

        let expected = match fs::read_to_string(&expected_path) {
            Ok(text) => text,
            Err(e) => {
                failures.push(format!("{name}: missing expected file ({e})"));
                continue;
            }
        };
        if actual != expected {
            failures.push(format!(
                "{name}: output differs from {name}.expected.pact\n--- actual ---\n{actual}"
            ));
            continue;
        }

        // 冪等性: 期待出力はそれ自身が正規形。
        match pact_fmt::format_source(&expected) {
            Ok(again) if again == expected => {}
            Ok(again) => failures.push(format!(
                "{name}: expected output is not a fixed point\n--- refmt ---\n{again}"
            )),
            Err(errors) => failures.push(format!(
                "{name}: expected output does not parse: {errors:?}"
            )),
        }

        // 意味的変更の禁止: 整形前後で span を除く AST が一致する。
        let before = pact_syntax::parse_module(&input);
        let after = pact_syntax::parse_module(&actual);
        if util::ast_json(&before.module) != util::ast_json(&after.module) {
            failures.push(format!("{name}: formatting changed the AST"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} golden case(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
