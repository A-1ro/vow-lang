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

/// 構造化修正提案(Agent Repair Protocol / M18 / #24)。`Fix`(テキスト編集)の
/// **意味論的強化版**で、契約レベルの差分をエージェントが機械適用 → 再検証できる形にする。
/// `Diagnostic.fixes` とは独立した追加フィールドで、後方互換(消費側が知らなくても壊れない)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedContract {
    /// 提案種別(例: `"ContractMissing"`)。エージェントが分岐に使う。
    pub kind: String,
    /// 契約を加える対象関数名。
    pub function: String,
    /// 契約節の種別(`"requires"` | `"ensures"`)。
    pub clause: String,
    /// 提案する契約式の Kei ソース表記(`contract_expr_text` と同じ表記)。
    pub expr: String,
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
    /// 構造化修正提案(M18 / #24)。`None` のときは JSON に現れない(後方互換)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_contract: Option<SuggestedContract>,
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
            suggested_contract: None,
        })
    }

    /// 構造化修正提案を載せる(M18)。`fixes` とは独立した追加情報。
    pub fn with_suggested_contract(mut self, suggested: SuggestedContract) -> Self {
        self.suggested_contract = Some(suggested);
        self
    }
}
