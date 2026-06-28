//! 契約の検証レベル報告(M12 / #23)。
//!
//! 「契約が**書かれている**こと」と「その契約が実際に**機械検証された**こと」は
//! 別物。検証レベルは**ソース構文に書き分けず**、`kei check` の構造化出力に載せる
//! (spec/kei-spec-v0.2.md §3「契約は不変・検証は成長」)。
//!
//! - `static`     … コンパイル時に成立が判定済み(v0.2 は定数畳み込みで真になる契約)
//! - `generative` … 契約から生成した property-based test で反例ゼロ(v0.3 / M15 / #26)
//! - `runtime`    … 実行時アサーションへ展開(v0.1 既定。大半はこれ)
//! - `trusted`    … 外部・人間レビュー・テストで保証(検証器の管轄外。v0.2 では未産出)
//! - `unchecked`  … 明示的に未検証(v0.2 では未産出)
//!
//! 強さの序列は `static` > `generative` > `runtime`。static で証明できないものは
//! generative(全生成入力で反例なし)、それも難しければ runtime、という連続的扱い。

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
    /// 全生成入力(Int の境界値や Bool の全域)で反例ゼロ。**universal coverage**。
    Generative,
    /// 部分サンプル(List 長 0..=2、record の小ドメイン cartesian)で反例ゼロ。
    /// 全数検査ではないので generative とは区別する(PR #76 review)。
    Bounded,
    Runtime,
    Trusted,
    Unchecked,
}

impl Verification {
    pub fn as_str(self) -> &'static str {
        match self {
            Verification::Static => "static",
            Verification::Generative => "generative",
            Verification::Bounded => "bounded",
            Verification::Runtime => "runtime",
            Verification::Trusted => "trusted",
            Verification::Unchecked => "unchecked",
        }
    }
}
