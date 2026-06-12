//! M4 e2e(docs/pact-roadmap-goals.md):
//! examples/ の全 .pact をトランスパイルし、
//! (1) `tsc --strict --noEmit` がエラーゼロ、
//! (2) vitest の実行テスト(期待出力一致・requires 違反・source map 解決)が
//! 全件パスすることを検証する。
//!
//! Node ツールチェイン(npm / npx)が必要。runtime/ と tests/e2e/ の
//! npm install・runtime のビルドもこのテストが面倒を見る。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root exists")
}

fn run(program: &str, args: &[&str], cwd: &Path) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {program} {}: {e}", args.join(" ")));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "command failed in {}: {program} {}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        cwd.display(),
        args.join(" "),
    );
    stdout
}

fn collect_pact_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_pact_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "pact") {
            out.push(path);
        }
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create target dir");
    for entry in fs::read_dir(from).expect("readable stub dir") {
        let entry = entry.expect("readable dir entry");
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            fs::copy(&src, &dst).expect("copy stub file");
        }
    }
}

#[test]
fn e2e_transpile_typecheck_and_run() {
    let root = repo_root();
    let runtime = root.join("runtime");
    let e2e = root.join("tests/e2e");

    // 1. @pact/runtime を依存解決してビルド(dist/ を生成)。
    run("npm", &["install", "--no-audit", "--no-fund"], &runtime);
    run("npm", &["run", "build"], &runtime);

    // 2. e2e ハーネスの依存解決(@pact/runtime は file: で runtime/ を参照)。
    run("npm", &["install", "--no-audit", "--no-fund"], &e2e);

    // 3. generated/ を作り直し、スタブ(core.money / infra.*)を配置する。
    let generated = e2e.join("generated");
    if generated.exists() {
        fs::remove_dir_all(&generated).expect("clean generated dir");
    }
    copy_tree(&e2e.join("stubs"), &generated);

    // 4. examples/ の全 .pact をトランスパイルして generated/ に書き出す。
    let mut files = Vec::new();
    collect_pact_files(&root.join("examples"), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no examples to transpile");
    for path in &files {
        let src = fs::read_to_string(path).expect("readable example");
        let rel = path
            .strip_prefix(&root)
            .expect("example under repo root")
            .to_string_lossy()
            .replace('\\', "/");
        let out = pact_emit::emit_module(&rel, &src).unwrap_or_else(|d| {
            panic!(
                "{rel}: examples must transpile cleanly: {}",
                serde_json::to_string_pretty(&d).expect("serializable")
            )
        });
        let ts_path = generated.join(&out.ts_path);
        fs::create_dir_all(ts_path.parent().expect("ts path has a parent"))
            .expect("create output dir");
        fs::write(&ts_path, &out.ts).expect("write generated TS");
        fs::write(ts_path.with_extension("ts.map"), &out.map).expect("write source map");
        println!("transpiled {rel} -> tests/e2e/generated/{}", out.ts_path);
    }

    // 5. tsc --strict --noEmit がエラーゼロ(goal 条件 1)。
    run("npx", &["tsc", "--strict", "--noEmit"], &e2e);
    println!("tsc --strict --noEmit: OK (zero errors)");

    // 6. vitest 実行テスト全件パス(goal 条件 2〜4)。
    let stdout = run("npx", &["vitest", "run"], &e2e);
    println!("{stdout}");
}
