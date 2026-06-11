//! ソース位置情報。line / col ともに 1 始まり、col は Unicode スカラー値単位
//! (spec/diagnostic-schema.md の Position と同じ規約)。
//!
//! pact_check の `Span` は file を持つが、こちらは単一ソース内の範囲のみを表す。
//! file の付与は Diagnostic への変換境界(pact_check 側)で行う。

use std::fmt;

use serde::{Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub col: u32,
}

impl Position {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }
}

/// ソース上の範囲。start は含み、end は含まない(排他)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// 幅ゼロの span(挿入位置の表現などに使う)。
    pub fn point(pos: Position) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    /// self の開始から other の終了までを覆う span。
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start.line, self.start.col, self.end.line, self.end.col
        )
    }
}

// golden test の AST ダンプを読みやすく保つため、span は
// "開始行:開始列-終了行:終了列" の文字列としてシリアライズする。
impl Serialize for Span {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
