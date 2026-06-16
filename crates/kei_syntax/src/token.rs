//! トークン定義。改行は文・フィールドの区切りとして意味を持つため
//! `Newline` トークンとして表面化する(パーサが文脈に応じて読み飛ばす)。

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // リテラル・識別子
    Ident,
    Int,
    Str,
    // キーワード
    Module,
    Import,
    As,
    Type,
    Record,
    Enum,
    Func,
    Uses,
    Requires,
    Ensures,
    Let,
    If,
    Else,
    Fail,
    Return,
    Tagged,
    True,
    False,
    Implies,
    Match,
    Extern,
    // デリミタ・区切り
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Arrow,
    FatArrow,
    // 演算子
    Eq,
    EqEq,
    NotEq,
    Lt,
    Gt,
    Le,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    // 構造
    Newline,
    Eof,
}

impl TokenKind {
    pub fn keyword(text: &str) -> Option<TokenKind> {
        Some(match text {
            "module" => TokenKind::Module,
            "import" => TokenKind::Import,
            "as" => TokenKind::As,
            "type" => TokenKind::Type,
            "record" => TokenKind::Record,
            "enum" => TokenKind::Enum,
            "func" => TokenKind::Func,
            "uses" => TokenKind::Uses,
            "requires" => TokenKind::Requires,
            "ensures" => TokenKind::Ensures,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "fail" => TokenKind::Fail,
            "return" => TokenKind::Return,
            "tagged" => TokenKind::Tagged,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "implies" => TokenKind::Implies,
            "match" => TokenKind::Match,
            "extern" => TokenKind::Extern,
            _ => return None,
        })
    }

    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            TokenKind::Module
                | TokenKind::Import
                | TokenKind::As
                | TokenKind::Type
                | TokenKind::Record
                | TokenKind::Enum
                | TokenKind::Func
                | TokenKind::Uses
                | TokenKind::Requires
                | TokenKind::Ensures
                | TokenKind::Let
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::Fail
                | TokenKind::Return
                | TokenKind::Tagged
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Implies
                | TokenKind::Match
                | TokenKind::Extern
        )
    }

    /// 固定綴りトークンの正規テキスト(挿入 fix の生成に使う)。
    pub fn literal(self) -> Option<&'static str> {
        Some(match self {
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Comma => ",",
            TokenKind::Colon => ":",
            TokenKind::Dot => ".",
            TokenKind::Arrow => "->",
            TokenKind::FatArrow => "=>",
            TokenKind::Eq => "=",
            TokenKind::EqEq => "==",
            TokenKind::NotEq => "!=",
            TokenKind::Lt => "<",
            TokenKind::Gt => ">",
            TokenKind::Le => "<=",
            TokenKind::Ge => ">=",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Bang => "!",
            TokenKind::Module => "module",
            TokenKind::Import => "import",
            TokenKind::As => "as",
            TokenKind::Type => "type",
            TokenKind::Record => "record",
            TokenKind::Enum => "enum",
            TokenKind::Func => "func",
            TokenKind::Uses => "uses",
            TokenKind::Requires => "requires",
            TokenKind::Ensures => "ensures",
            TokenKind::Let => "let",
            TokenKind::If => "if",
            TokenKind::Else => "else",
            TokenKind::Fail => "fail",
            TokenKind::Return => "return",
            TokenKind::Tagged => "tagged",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::Implies => "implies",
            TokenKind::Match => "match",
            TokenKind::Extern => "extern",
            _ => return None,
        })
    }

    /// エラーメッセージ用の説明。
    pub fn describe(self) -> &'static str {
        match self {
            TokenKind::Ident => "identifier",
            TokenKind::Int => "integer literal",
            TokenKind::Str => "string literal",
            TokenKind::Newline => "end of line",
            TokenKind::Eof => "end of file",
            other => other.literal().unwrap_or("token"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    /// Ident は識別子名、Str はエスケープ解決済みの値、Int は数字列。
    /// 固定綴りトークンは正規テキスト。
    pub text: String,
    pub span: Span,
}

impl Token {
    /// エラーメッセージ向けの「found …」表記。
    pub fn found_label(&self) -> String {
        match self.kind {
            TokenKind::Ident => format!("identifier '{}'", self.text),
            TokenKind::Int => format!("integer literal '{}'", self.text),
            TokenKind::Str => "string literal".to_string(),
            TokenKind::Newline => "end of line".to_string(),
            TokenKind::Eof => "end of file".to_string(),
            k if k.is_keyword() => format!("keyword '{}'", self.text),
            _ => format!("'{}'", self.text),
        }
    }
}
