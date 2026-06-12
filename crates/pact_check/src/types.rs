//! 検査用の型表現。AST の型参照(`pact_syntax::ast::Type`)を解決した正規形。
//!
//! [`Ty::Unknown`] は import 先など単一ファイル検査では解決できない型を表し、
//! あらゆる型と互換になる。検査をすり抜けさせるための明示的な穴であり、
//! カスケードエラーを防ぐ役割も兼ねる。

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Str,
    Bool,
    /// 値を返さない関数の戻り値。ソース上に対応する構文はない。
    Unit,
    Record(String),
    Enum(String),
    /// `type AccountId = String tagged "AccountId"` の幽霊型。
    /// 同名タグ同士のみ互換で、基底型とは混同できない(spec §2.2)。
    Tagged {
        name: String,
        underlying: Box<Ty>,
    },
    Result(Box<Ty>, Box<Ty>),
    Option(Box<Ty>),
    Unknown,
}

impl Ty {
    /// 互換判定。Unknown は全てと互換。
    pub fn compatible(&self, other: &Ty) -> bool {
        use Ty::*;
        match (self, other) {
            (Unknown, _) | (_, Unknown) => true,
            (Int, Int) | (Str, Str) | (Bool, Bool) | (Unit, Unit) => true,
            (Record(a), Record(b)) | (Enum(a), Enum(b)) => a == b,
            (Tagged { name: a, .. }, Tagged { name: b, .. }) => a == b,
            (Result(o1, e1), Result(o2, e2)) => o1.compatible(o2) && e1.compatible(e2),
            (Option(a), Option(b)) => a.compatible(b),
            _ => false,
        }
    }

    /// 算術・順序比較の対象になれるか(Int または Int 基底の tagged 型)。
    pub fn is_numeric(&self) -> bool {
        match self {
            Ty::Int | Ty::Unknown => true,
            Ty::Tagged { underlying, .. } => underlying.is_numeric(),
            _ => false,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => write!(f, "Int"),
            Ty::Str => write!(f, "String"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Unit => write!(f, "Unit"),
            Ty::Record(n) | Ty::Enum(n) | Ty::Tagged { name: n, .. } => write!(f, "{n}"),
            Ty::Result(t, e) => write!(f, "Result<{t}, {e}>"),
            Ty::Option(t) => write!(f, "Option<{t}>"),
            Ty::Unknown => write!(f, "_"),
        }
    }
}
