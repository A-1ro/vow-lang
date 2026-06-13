//! ビルド時に焼き込んだ spec/ と examples/ へのアクセス。
//!
//! 実体は `build.rs` が生成する `OUT_DIR/embedded.rs`(`SPEC_FILES` /
//! `EXAMPLE_FILES`)。各エントリは `(相対パス, 内容)` で、相対パスは
//! それぞれ spec/ ・ examples/ ルートからの `/` 区切りパス。

include!(concat!(env!("OUT_DIR"), "/embedded.rs"));

/// 埋め込み済み spec ファイル一覧(相対パス昇順)。
pub fn spec_files() -> &'static [(&'static str, &'static str)] {
    SPEC_FILES
}

/// 埋め込み済み examples ファイル一覧(相対パス昇順)。
pub fn example_files() -> &'static [(&'static str, &'static str)] {
    EXAMPLE_FILES
}

/// spec ルートからの相対パスで内容を引く(例: `"errors/KEI-E3001.md"`)。
pub fn spec_file(relpath: &str) -> Option<&'static str> {
    SPEC_FILES
        .iter()
        .find(|(p, _)| *p == relpath)
        .map(|(_, c)| *c)
}
