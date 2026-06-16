//! 契約の検証レベル報告(M12 / #23)。
//!
//! 「契約が**書かれている**こと」と「その契約が実際に**機械検証された**こと」は
//! 別物。検証レベルは**ソース構文に書き分けず**、`kei check` の構造化出力に載せる
//! (spec/kei-spec-v0.2.md §3「契約は不変・検証は成長」)。
//!
//! - `static`   … コンパイル時に成立が判定済み(v0.2 は定数畳み込みで真になる契約)
//! - `runtime`  … 実行時アサーションへ展開(v0.1 既定。大半はこれ)
//! - `trusted`  … 外部・人間レビュー・テストで保証(検証器の管轄外。v0.2 では未産出)
//! - `unchecked`… 明示的に未検証(v0.2 では未産出)

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, Span};

/// `kei check --json` の構造化出力。診断(エラー/警告)と契約の検証レベルを併せて運ぶ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub contracts: Vec<ContractInfo>,
}

/// 1 契約節(requires / ensures)の検証レベル報告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractInfo {
    /// 契約を宣言している関数名。
    pub func: String,
    pub kind: ContractKind,
    /// 契約式の Kei ソース表記(`KeiContractViolation.condition` と同じ表記)。
    pub expr: String,
    /// 処理系が**達成できた**検証レベル(書き手の選択ではない)。
    pub verification: Verification,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContractKind {
    Requires,
    Ensures,
}

impl ContractKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ContractKind::Requires => "requires",
            ContractKind::Ensures => "ensures",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verification {
    Static,
    Runtime,
    Trusted,
    Unchecked,
}

impl Verification {
    pub fn as_str(self) -> &'static str {
        match self {
            Verification::Static => "static",
            Verification::Runtime => "runtime",
            Verification::Trusted => "trusted",
            Verification::Unchecked => "unchecked",
        }
    }
}
