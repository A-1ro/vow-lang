//! kei_syntax 内部の構文エラー型。
//!
//! Diagnostic 型の唯一の定義元は kei_check であり(ARCHITECTURE.md 不変条件 1)、
//! 本型はクレート境界で kei_check 側の変換関数により Diagnostic へ写される。
//! 「全 Diagnostic に最低 1 つの fix 候補」の不変条件を境界で満たせるよう、
//! 構文エラーは常に [`FixHint`] を 1 つ携える。

use crate::span::Span;

/// 字句・構文エラー(カテゴリ 0: `KEI-E0xxx`)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    /// spec/diagnostic-schema.md の採番ルールに従うエラーコード。
    pub code: &'static str,
    /// 人間・エージェント双方が読む一文(英語)。
    pub message: String,
    pub span: Span,
    pub fix: FixHint,
}

/// 修正候補のヒント。`edits` が空の場合は修正の方向のみを提示する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixHint {
    pub title: String,
    /// (置換対象 span, 置換後テキスト) の列。挿入は幅ゼロ span で表現する。
    pub edits: Vec<(Span, String)>,
}

impl FixHint {
    pub fn direction(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            edits: Vec::new(),
        }
    }

    pub fn replace(title: impl Into<String>, span: Span, new_text: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            edits: vec![(span, new_text.into())],
        }
    }
}

/// 字句・構文カテゴリのエラーコード一覧。spec/errors/ の解説ページと 1:1 対応。
pub mod codes {
    /// 字句解析: 言語が認識しない文字
    pub const UNEXPECTED_CHAR: &str = "KEI-E0001";
    /// 字句解析: 閉じられていない文字列リテラル
    pub const UNTERMINATED_STRING: &str = "KEI-E0002";
    /// 字句解析: 不正なエスケープシーケンス
    pub const INVALID_ESCAPE: &str = "KEI-E0003";
    /// 字句解析: 表現範囲外の整数リテラル
    pub const INT_OUT_OF_RANGE: &str = "KEI-E0004";
    /// 構文解析: 予期しないトークン
    pub const UNEXPECTED_TOKEN: &str = "KEI-E0101";
    /// 構文解析: 予約語の識別子使用
    pub const RESERVED_IDENT: &str = "KEI-E0102";
    /// 構文解析: 閉じられていないデリミタ
    pub const UNCLOSED_DELIMITER: &str = "KEI-E0103";
    /// 構文解析: 契約節キーワードの綴り間違い(uses / requires / ensures)
    pub const UNKNOWN_CLAUSE: &str = "KEI-E0104";
}
