//! 構造化 Diagnostic。スキーマは spec/diagnostic-schema.md が正。
//!
//! - JSON が正、散文は派生(CLI 側で整形する)
//! - 全 Diagnostic に span・code・最低 1 つの fix 候補を含める

use serde::{Deserialize, Serialize};

/// 診断の深刻度。JSON では小文字文字列(`"error"` 等)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// ソース上の位置。line / col ともに 1 始まり、col は Unicode スカラー値単位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub col: u32,
}

/// ソース上の範囲。start は含み、end は含まない(排他)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// リポジトリルートからの相対パス。
    pub file: String,
    pub start: Position,
    pub end: Position,
}

/// 機械適用可能な単一の編集。挿入は `start == end` の span で表現する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub span: Span,
    pub new_text: String,
}

/// 修正候補。`edits` が空の場合は修正の方向のみを提示する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    pub title: String,
    pub edits: Vec<TextEdit>,
}

/// 構造化診断。`kei check --json` は `Vec<Diagnostic>` を出力する。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// `KEI-E[カテゴリ1桁][連番3桁]` 形式(spec/diagnostic-schema.md 採番ルール)。
    pub code: String,
    pub message: String,
    pub span: Span,
    /// 最低 1 要素。[`Diagnostic::new`] 経由の構築でこれを保証する。
    pub fixes: Vec<Fix>,
}

impl Diagnostic {
    /// fix 候補を最低 1 つ要求するコンストラクタ。
    ///
    /// `fixes` が空の場合は `None` を返す(全 Diagnostic に
    /// 最低 1 つの fix 候補を含めるという不変条件の入口検査)。
    pub fn new(
        severity: Severity,
        code: impl Into<String>,
        message: impl Into<String>,
        span: Span,
        fixes: Vec<Fix>,
    ) -> Option<Self> {
        if fixes.is_empty() {
            return None;
        }
        Some(Self {
            severity,
            code: code.into(),
            message: message.into(),
            span,
            fixes,
        })
    }
}
