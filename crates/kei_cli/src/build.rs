//! `kei build` のランナー。ディレクトリ単位のトランスパイルを担う。
//!
//! ARCHITECTURE.md(kei_cli 行): 言語処理は kei_emit に委譲する。ここが持つのは
//! ディレクトリ走査・all-or-nothing 制御・ファイル IO・Diagnostic の散文整形だけ。
//!
//! 振る舞い(docs/kei-roadmap-goals.md M7 事前合意):
//! - `<dir>` 配下の `**/*.kei` を再帰収集(出力先 out-dir 配下は走査しない)。
//! - 全ファイルを**先に**検査し、1 件でもエラーがあれば**何も書かず**全 Diagnostic を
//!   stderr に出して exit 1(中途半端な dist/ を残さない)。
//! - 全件クリーンなら out-dir(既定 `<dir>/dist/`)に emit の `ts_path`
//!   (モジュールパス由来)で 1:1 配置。source map は既定 on、`--no-source-map` で抑止。
//!
//! 出力ストリーム規約: 生成物はファイルに書くため stdout は使わず、進捗と診断は
//! すべて stderr に出す(`kei build` を別コマンドへパイプしても汚さない)。

use std::path::{Path, PathBuf};

use kei_check::{Diagnostic, Severity};
use kei_emit::EmitOutput;

use crate::cli::UsageError;
use crate::render;

/// `kei build`。成功は終了コード 0、検査エラーは 1、ディレクトリ/IO 不正は使用法エラー。
pub fn run(dir: &Path, out_dir: Option<&Path>, source_map: bool) -> Result<u8, UsageError> {
    if !dir.is_dir() {
        return Err(UsageError(format!(
            "build: '{}' is not a directory",
            dir.display()
        )));
    }
    let out = out_dir.map_or_else(|| dir.join("dist"), Path::to_path_buf);

    // out-dir(dist)を入力走査から除外する。まだ無ければ除外不要。
    let out_canon = out.canonicalize().ok();
    let mut files = Vec::new();
    collect_kei_files(dir, out_canon.as_deref(), &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(UsageError(format!(
            "build: no .kei files found under '{}'",
            dir.display()
        )));
    }

    // 全ファイルを先に検査(all-or-nothing)。1 件でもエラーなら何も書かない。
    let mut outputs: Vec<EmitOutput> = Vec::new();
    let mut failures: Vec<(String, Vec<Diagnostic>)> = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file)
            .map_err(|e| UsageError(format!("cannot read {}: {e}", file.display())))?;
        let rel = rel_path(dir, file);
        match kei_emit::emit_module(&rel, &source) {
            Ok(out) => outputs.push(out),
            Err(diags) => failures.push((source, diags)),
        }
    }

    if !failures.is_empty() {
        let mut errors = 0usize;
        for (source, diags) in &failures {
            errors += diags
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count();
            eprint!("{}", render::diagnostics(source, diags));
        }
        eprintln!(
            "kei build: {errors} error(s) in {} file(s); no output written",
            failures.len()
        );
        return Ok(1);
    }

    // 検査クリーン。out-dir に TS(+ source map)を ts_path 通りに配置する。
    for out_mod in &outputs {
        let ts_dest = out.join(&out_mod.ts_path);
        if let Some(parent) = ts_dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| UsageError(format!("cannot create {}: {e}", parent.display())))?;
        }
        let ts = if source_map {
            out_mod.ts.clone()
        } else {
            strip_source_mapping_url(&out_mod.ts)
        };
        std::fs::write(&ts_dest, ts)
            .map_err(|e| UsageError(format!("cannot write {}: {e}", ts_dest.display())))?;
        if source_map {
            let map_dest = out.join(format!("{}.map", out_mod.ts_path));
            std::fs::write(&map_dest, &out_mod.map)
                .map_err(|e| UsageError(format!("cannot write {}: {e}", map_dest.display())))?;
        }
    }

    let mut paths: Vec<&str> = outputs.iter().map(|o| o.ts_path.as_str()).collect();
    paths.sort_unstable();
    eprintln!(
        "kei build: wrote {} module(s) to {}",
        outputs.len(),
        out.display()
    );
    for p in paths {
        eprintln!("  {p}");
    }
    Ok(0)
}

/// `<dir>` 配下を再帰し `*.kei` を集める。`skip`(out-dir の正規化パス)配下は降りない。
fn collect_kei_files(
    dir: &Path,
    skip: Option<&Path>,
    out: &mut Vec<PathBuf>,
) -> Result<(), UsageError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| UsageError(format!("cannot read {}: {e}", dir.display())))?;
    for entry in entries {
        let path = entry
            .map_err(|e| UsageError(format!("cannot read entry in {}: {e}", dir.display())))?
            .path();
        if path.is_dir() {
            if skip.is_some() && path.canonicalize().ok().as_deref() == skip {
                continue;
            }
            collect_kei_files(&path, skip, out)?;
        } else if path.extension().is_some_and(|e| e == "kei") {
            out.push(path);
        }
    }
    Ok(())
}

/// `<dir>` を基準にした入力ファイルの相対パス(`/` 区切り)。Diagnostic の span と
/// source map の `sources`、生成 TS の先頭コメントの双方に使う(配置非依存で安定)。
fn rel_path(dir: &Path, file: &Path) -> String {
    file.strip_prefix(dir)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

/// `--no-source-map` 時に、生成 TS 末尾の `//# sourceMappingURL=...` 行を取り除く。
/// emit が常に埋め込むコメントを CLI 層で落とすだけで、言語処理には触れない。
fn strip_source_mapping_url(ts: &str) -> String {
    let mut out = String::with_capacity(ts.len());
    for line in ts.lines() {
        if line.starts_with("//# sourceMappingURL=") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_path_is_dir_relative_with_forward_slashes() {
        let rel = rel_path(Path::new("a/b"), Path::new("a/b/sub/m.kei"));
        assert_eq!(rel, "sub/m.kei");
    }

    #[test]
    fn strip_removes_only_the_source_mapping_comment() {
        let ts = "export const x = 1;\n//# sourceMappingURL=m.ts.map\n";
        assert_eq!(strip_source_mapping_url(ts), "export const x = 1;\n");
    }
}
