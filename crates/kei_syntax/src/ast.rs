//! Kei AST 定義。全ノードが span を保持する。型の知識は持たない(ARCHITECTURE.md)。
//!
//! `Serialize` 実装は golden test(tests/golden/syntax/)の AST JSON ダンプが
//! 利用する。enum は `"kind"` タグ付きでシリアライズされる。

use serde::Serialize;

use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// 1 ソースファイルのパース結果ルート。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Module {
    pub decl: Option<ModuleDecl>,
    pub imports: Vec<Import>,
    pub items: Vec<Item>,
    pub span: Span,
}

/// `module payments.transfer`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleDecl {
    pub path: Vec<Ident>,
    pub span: Span,
}

/// `import core.money { Money }` / `import infra.database as Database`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Import {
    pub path: Vec<Ident>,
    /// `{ ... }` で列挙された名前。空ならモジュール全体参照。
    pub names: Vec<Ident>,
    pub alias: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum Item {
    TypeAlias(TypeAlias),
    Record(RecordDecl),
    Enum(EnumDecl),
    Func(FuncDecl),
    Extern(ExternDecl),
}

/// 外部境界の署名宣言(M11)。
/// `extern Time.now() -> Int uses Clock` のように、import した名前空間配下の
/// 外部関数の戻り型・エフェクトを宣言する。本体は持たない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternDecl {
    /// 純粋観測子(query)か(M14 / #45)。`extern query Database.availableOf(...)` で true。
    /// query は副作用のない論理的読み取りで、契約式から呼べる(`uses` は持てない)。
    pub query: bool,
    /// `[Time, now]` / `[Database, fetchBalance]` / `[Audit, Log, record]`
    pub path: Vec<Ident>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub uses: Vec<EffectRef>,
    pub span: Span,
}

/// `type AccountId = String tagged "AccountId"`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeAlias {
    pub name: Ident,
    pub ty: Type,
    /// 幽霊型タグ(`tagged "..."`)。
    pub tag: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordDecl {
    pub name: Ident,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<Variant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Variant {
    pub name: Ident,
    pub payload: VariantPayload,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum VariantPayload {
    /// `Timeout`
    Unit,
    /// `NotFound(AccountId)`
    Tuple { types: Vec<Type> },
    /// `InsufficientFunds { needed: Money, had: Money }`
    Record { fields: Vec<FieldDef> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuncDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub uses: Vec<EffectRef>,
    pub requires: Vec<Expr>,
    pub ensures: Vec<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

/// `uses` 節の 1 エフェクト参照(例: `Database.Write`)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectRef {
    pub path: Vec<Ident>,
    pub span: Span,
}

/// 型参照。`Result<TransferReceipt, TransferError>` 等。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Type {
    pub path: Vec<Ident>,
    pub args: Vec<Type>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum Stmt {
    Let(LetStmt),
    If(IfStmt),
    Return(ReturnStmt),
    Expr(ExprStmt),
}

/// `let x: T = expr else fail expr`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LetStmt {
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: Expr,
    /// `else fail <expr>` の失敗値。
    pub else_fail: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IfStmt {
    pub cond: Expr,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum ElseBranch {
    If(Box<IfStmt>),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum Expr {
    Int {
        value: i64,
        span: Span,
    },
    Str {
        value: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Name {
        name: String,
        span: Span,
    },
    /// `expr.field`
    Field {
        base: Box<Expr>,
        name: Ident,
        span: Span,
    },
    /// `callee(args...)`
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `Path { field: expr, shorthand }`
    RecordLit {
        path: Vec<Ident>,
        fields: Vec<RecordLitField>,
        span: Span,
    },
    /// `match <scrutinee> { <pattern> => <expr>, ... }`(網羅分解。M10)
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int { span, .. }
            | Expr::Str { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Name { span, .. }
            | Expr::Field { span, .. }
            | Expr::Call { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::RecordLit { span, .. }
            | Expr::Match { span, .. } => *span,
        }
    }
}

/// `match` の 1 腕(`<pattern> => <body>`)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// 1 段のコンストラクタパターン。`path` はコンストラクタ
/// (`Some` / `None` / `Ok` / `Err` または `Enum.Variant`)、`payload` は束縛形。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub path: Vec<Ident>,
    pub payload: PatternPayload,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum PatternPayload {
    /// `None` / `E.Unit`(束縛なし)
    Unit,
    /// `Some(x)` / `Ok(x)` / `E.V(a, b)`(位置束縛)
    Tuple { bindings: Vec<Ident> },
    /// `E.V { a, b }`(名前付きフィールドをそのまま束縛するショートハンド)
    Record { fields: Vec<Ident> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordLitField {
    pub name: Ident,
    /// `None` はショートハンド(`Transfer { from, to }`)。
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Implies,
}
