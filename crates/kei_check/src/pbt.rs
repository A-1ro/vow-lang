//! 契約ベース PBT 生成(M15 / #26 段階1+2)。
//!
//! Kei の契約はテストの二大要素を内包する: `requires` = 入力の生成制約、
//! `ensures` = テストオラクル。本モジュールは契約を読んで property-based test を
//! **生成・実行**する。
//!
//! **中核原則(捏造不能性):** シード/生成器は「入力」のみを供給し、「期待値」を持たない。
//! オラクルは契約(`ensures`)のみが担う。AI がテストを通す唯一の方法を「実装を契約に
//! 合わせる」ことだけに限定し、「テストを通すために期待値を歪める」捏造経路を**言語構造から
//! 排除**する(第一条の権限分離をテストドメインへ拡張)。
//!
//! 純粋関数だけが対象。エフェクト(`uses`)・外部呼び出し・評価器が扱えない構文を含む
//! 関数は静かに対象外(`generative` には上がらず `runtime` のまま)。生成・判定ロジックは
//! ここ(kei_check)に置き、kei_cli は委譲のみ(CLAUDE.md)。

use std::collections::HashMap;
use std::fmt;

use kei_syntax::ast;

use crate::check::contract_expr_text;
use crate::{Diagnostic, Fix, Position, Severity, Span};

mod seed_codes {
    /// シードファイルの文法エラー(期待値フィールド混入を含む)。
    pub const SEED_GRAMMAR: &str = "KEI-E4006";
    /// シード入力が対象関数の requires / 型 / 名前に適合しない。
    pub const SEED_INVALID: &str = "KEI-E4007";
    /// シード入力が ensures を破った(反例)。生成テストと同じ KEI-E4005。
    pub const SEED_COUNTEREXAMPLE: &str = "KEI-E4005";
}

/// 評価器が扱う値(段階1: スカラのみ)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s:?}"),
        }
    }
}

/// 評価できない構文・実行時破綻。対象関数を PBT から外す合図(クラッシュさせない)。
#[derive(Debug, Clone)]
enum EvalError {
    /// 評価器が未対応の構文(match / record / Option / Result / 外部呼び出し等)。
    Unsupported,
    /// 実行時に表現不能(ゼロ除算・オーバーフロー)。その入力ケースを捨てる。
    Trap,
}

/// 1 関数の生成テスト結果。
#[derive(Debug, Clone)]
pub struct PropertyOutcome {
    pub func: String,
    /// 全ての生成入力で全 `ensures` が成り立った。
    pub passed: bool,
    /// `requires` を満たし実際に検査した入力ケース数。
    pub cases_checked: usize,
    /// 反例(最小化済み入力 + 破れた ensures のソース表記)。`passed` なら `None`。
    pub counterexample: Option<CounterExample>,
}

#[derive(Debug, Clone)]
pub struct CounterExample {
    /// パラメータ名 → 値(最小化済み)。
    pub inputs: Vec<(String, Value)>,
    /// 破れた `ensures` 節の Kei ソース表記。
    pub clause: String,
    /// 破れた `ensures` 節の span(診断位置に使う)。
    pub clause_span: kei_syntax::Span,
}

impl CounterExample {
    /// `available = 1, step = 0` のような入力の散文表記。
    pub fn inputs_text(&self) -> String {
        self.inputs
            .iter()
            .map(|(n, v)| format!("{n} = {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// モジュール内の純粋関数を生成テストする。対象外の関数は結果に現れない。
pub fn run_module(module: &ast::Module) -> Vec<PropertyOutcome> {
    let funcs: HashMap<&str, &ast::FuncDecl> = module
        .items
        .iter()
        .filter_map(|it| match it {
            ast::Item::Func(f) => Some((f.name.name.as_str(), f)),
            _ => None,
        })
        .collect();

    let mut out = Vec::new();
    for item in &module.items {
        let ast::Item::Func(f) = item else { continue };
        if let Some(outcome) = run_function(f, &funcs) {
            out.push(outcome);
        }
    }
    out
}

/// 純粋関数 1 つを生成テストする。対象外なら `None`。
fn run_function(
    f: &ast::FuncDecl,
    funcs: &HashMap<&str, &ast::FuncDecl>,
) -> Option<PropertyOutcome> {
    // 対象条件: 純粋(uses なし)・ensures あり・全パラメータがスカラ生成可能。
    if !f.uses.is_empty() || f.ensures.is_empty() {
        return None;
    }
    let mut domains: Vec<Vec<Value>> = Vec::new();
    for p in &f.params {
        domains.push(candidate_values(&p.ty)?);
    }

    let combos = cartesian(&domains);
    let mut cases_checked = 0usize;
    let mut failures: Vec<(Vec<Value>, &ast::Expr)> = Vec::new();
    let mut evaluable = false;

    for combo in &combos {
        let env: HashMap<String, Value> = f
            .params
            .iter()
            .map(|p| p.name.name.clone())
            .zip(combo.iter().cloned())
            .collect();

        // requires を満たす入力だけを検査対象にする(満たさない入力は捨てる)。
        match all_hold(&f.requires, &env, funcs) {
            Ok(false) => continue,
            Ok(true) => {}
            Err(EvalError::Unsupported) => return None, // 契約が評価不能 → 対象外
            Err(EvalError::Trap) => continue,
        }

        // 関数本体を評価して result を得る。
        let result = match eval_func_call(f, combo, funcs, 0) {
            Ok(v) => v,
            Err(EvalError::Unsupported) => return None,
            Err(EvalError::Trap) => continue,
        };
        evaluable = true;

        // ensures を評価(result と old(param)=入力値を束縛)。
        let mut ens_env = env.clone();
        ens_env.insert("result".to_string(), result);
        for clause in &f.ensures {
            match eval_bool(clause, &ens_env, funcs, true) {
                Ok(true) => {}
                Ok(false) => failures.push((combo.clone(), clause)),
                Err(EvalError::Unsupported) => return None,
                Err(EvalError::Trap) => {}
            }
        }
        cases_checked += 1;
    }

    // requires を満たす入力が 1 件も無い / 一度も評価できなかった → 対象外。
    if !evaluable || cases_checked == 0 {
        return None;
    }

    // 反例があれば最小化(入力サイズ最小のもの)して報告。
    let counterexample = failures
        .iter()
        .min_by_key(|(combo, _)| size_metric(combo))
        .map(|(combo, clause)| CounterExample {
            inputs: f
                .params
                .iter()
                .map(|p| p.name.name.clone())
                .zip(combo.iter().cloned())
                .collect(),
            clause: contract_expr_text(clause),
            clause_span: clause.span(),
        });

    Some(PropertyOutcome {
        func: f.name.name.clone(),
        passed: counterexample.is_none(),
        cases_checked,
        counterexample,
    })
}

/// 型ごとの決定的な候補入力集合。生成不能型(record / Option 等)は `None`(対象外)。
fn candidate_values(t: &ast::Type) -> Option<Vec<Value>> {
    if t.path.len() != 1 || !t.args.is_empty() {
        return None;
    }
    match t.path[0].name.as_str() {
        "Int" => Some(
            [-100, -10, -3, -2, -1, 0, 1, 2, 3, 10, 100]
                .iter()
                .map(|n| Value::Int(*n))
                .collect(),
        ),
        "Bool" => Some(vec![Value::Bool(false), Value::Bool(true)]),
        "String" => Some(vec![
            Value::Str(String::new()),
            Value::Str("a".to_string()),
            Value::Str("abc".to_string()),
        ]),
        _ => None,
    }
}

/// 反例の「小ささ」(最小化用)。Int は絶対値、String は長さの総和。
fn size_metric(combo: &[Value]) -> i64 {
    combo
        .iter()
        .map(|v| match v {
            Value::Int(n) => n.unsigned_abs() as i64,
            Value::Bool(_) => 0,
            Value::Str(s) => s.len() as i64,
        })
        .sum()
}

/// 各次元の候補のデカルト積(全組み合わせ)。
fn cartesian(domains: &[Vec<Value>]) -> Vec<Vec<Value>> {
    let mut acc: Vec<Vec<Value>> = vec![Vec::new()];
    for dom in domains {
        let mut next = Vec::new();
        for prefix in &acc {
            for v in dom {
                let mut row = prefix.clone();
                row.push(v.clone());
                next.push(row);
            }
        }
        acc = next;
    }
    acc
}

/// 全 `requires` が成り立つか(`old` は requires では使えないので false 固定で評価)。
fn all_hold(
    clauses: &[ast::Expr],
    env: &HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
) -> Result<bool, EvalError> {
    for c in clauses {
        if !eval_bool(c, env, funcs, false)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn eval_bool(
    e: &ast::Expr,
    env: &HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    in_ensures: bool,
) -> Result<bool, EvalError> {
    match eval_expr(e, env, funcs, in_ensures, 0)? {
        Value::Bool(b) => Ok(b),
        _ => Err(EvalError::Unsupported),
    }
}

/// 関数を入力ベクタで評価する。深さ制限で無限再帰を防ぐ。
fn eval_func_call(
    f: &ast::FuncDecl,
    args: &[Value],
    funcs: &HashMap<&str, &ast::FuncDecl>,
    depth: usize,
) -> Result<Value, EvalError> {
    if depth > 64 {
        return Err(EvalError::Unsupported);
    }
    let env: HashMap<String, Value> = f
        .params
        .iter()
        .map(|p| p.name.name.clone())
        .zip(args.iter().cloned())
        .collect();
    eval_block(&f.body, env, funcs, depth)
}

/// ブロックを評価し、`return` の値を返す。`return` が無ければ Unsupported。
fn eval_block(
    block: &ast::Block,
    mut env: HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    depth: usize,
) -> Result<Value, EvalError> {
    for stmt in &block.stmts {
        match stmt {
            ast::Stmt::Let(l) => {
                if l.else_fail.is_some() {
                    return Err(EvalError::Unsupported);
                }
                let v = eval_expr(&l.value, &env, funcs, false, depth)?;
                env.insert(l.name.name.clone(), v);
            }
            ast::Stmt::Return(r) => {
                let Some(v) = &r.value else {
                    return Err(EvalError::Unsupported);
                };
                return eval_expr(v, &env, funcs, false, depth);
            }
            ast::Stmt::If(i) => {
                if let Some(v) = eval_if(i, &env, funcs, depth)? {
                    return Ok(v);
                }
            }
            ast::Stmt::Expr(_) => return Err(EvalError::Unsupported),
        }
    }
    Err(EvalError::Unsupported)
}

/// `if` を評価。分岐内の `return` に達したら Some(値)、達しなければ None。
fn eval_if(
    i: &ast::IfStmt,
    env: &HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    depth: usize,
) -> Result<Option<Value>, EvalError> {
    let cond = match eval_expr(&i.cond, env, funcs, false, depth)? {
        Value::Bool(b) => b,
        _ => return Err(EvalError::Unsupported),
    };
    if cond {
        return eval_block_opt(&i.then_block, env.clone(), funcs, depth);
    }
    match &i.else_branch {
        Some(ast::ElseBranch::Block(b)) => eval_block_opt(b, env.clone(), funcs, depth),
        Some(ast::ElseBranch::If(nested)) => eval_if(nested, env, funcs, depth),
        None => Ok(None),
    }
}

/// ブロックを評価し、`return` に達したら Some(値)。落ちたら None(後続の文へ続く)。
fn eval_block_opt(
    block: &ast::Block,
    mut env: HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    depth: usize,
) -> Result<Option<Value>, EvalError> {
    for stmt in &block.stmts {
        match stmt {
            ast::Stmt::Let(l) => {
                if l.else_fail.is_some() {
                    return Err(EvalError::Unsupported);
                }
                let v = eval_expr(&l.value, &env, funcs, false, depth)?;
                env.insert(l.name.name.clone(), v);
            }
            ast::Stmt::Return(r) => {
                let Some(v) = &r.value else {
                    return Err(EvalError::Unsupported);
                };
                return Ok(Some(eval_expr(v, &env, funcs, false, depth)?));
            }
            ast::Stmt::If(i) => {
                if let Some(v) = eval_if(i, &env, funcs, depth)? {
                    return Ok(Some(v));
                }
            }
            ast::Stmt::Expr(_) => return Err(EvalError::Unsupported),
        }
    }
    Ok(None)
}

fn eval_expr(
    e: &ast::Expr,
    env: &HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    in_ensures: bool,
    depth: usize,
) -> Result<Value, EvalError> {
    match e {
        ast::Expr::Int { value, .. } => Ok(Value::Int(*value)),
        ast::Expr::Bool { value, .. } => Ok(Value::Bool(*value)),
        ast::Expr::Str { value, .. } => Ok(Value::Str(value.clone())),
        ast::Expr::Name { name, .. } => env.get(name).cloned().ok_or(EvalError::Unsupported),
        ast::Expr::Unary { op, expr, .. } => {
            let v = eval_expr(expr, env, funcs, in_ensures, depth)?;
            match (op, v) {
                (ast::UnaryOp::Neg, Value::Int(n)) => {
                    n.checked_neg().map(Value::Int).ok_or(EvalError::Trap)
                }
                (ast::UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                _ => Err(EvalError::Unsupported),
            }
        }
        ast::Expr::Binary { op, lhs, rhs, .. } => {
            let l = eval_expr(lhs, env, funcs, in_ensures, depth)?;
            let r = eval_expr(rhs, env, funcs, in_ensures, depth)?;
            eval_binary(*op, l, r)
        }
        ast::Expr::Call { callee, args, .. } => {
            // `old(x)`: 純粋関数では進入時=入力値。ensures でのみ意味を持つ。
            if let ast::Expr::Name { name, .. } = callee.as_ref() {
                if name == "old" {
                    if !in_ensures || args.len() != 1 {
                        return Err(EvalError::Unsupported);
                    }
                    return eval_expr(&args[0], env, funcs, in_ensures, depth);
                }
                // ローカル純粋関数の呼び出し。
                if let Some(callee_fn) = funcs.get(name.as_str()) {
                    if !callee_fn.uses.is_empty() {
                        return Err(EvalError::Unsupported);
                    }
                    let mut argv = Vec::with_capacity(args.len());
                    for a in args {
                        argv.push(eval_expr(a, env, funcs, in_ensures, depth)?);
                    }
                    return eval_func_call(callee_fn, &argv, funcs, depth + 1);
                }
            }
            Err(EvalError::Unsupported)
        }
        // 段階1の評価器はスカラのみ(match / record / field / Option / Result は未対応)。
        _ => Err(EvalError::Unsupported),
    }
}

fn eval_binary(op: ast::BinOp, l: Value, r: Value) -> Result<Value, EvalError> {
    use ast::BinOp::*;
    use Value::{Bool, Int};
    match (op, l, r) {
        (Add, Int(a), Int(b)) => a.checked_add(b).map(Int).ok_or(EvalError::Trap),
        (Sub, Int(a), Int(b)) => a.checked_sub(b).map(Int).ok_or(EvalError::Trap),
        (Mul, Int(a), Int(b)) => a.checked_mul(b).map(Int).ok_or(EvalError::Trap),
        (Div, Int(_), Int(0)) => Err(EvalError::Trap),
        (Div, Int(a), Int(b)) => a.checked_div(b).map(Int).ok_or(EvalError::Trap),
        (Eq, a, b) => Ok(Bool(a == b)),
        (Ne, a, b) => Ok(Bool(a != b)),
        (Lt, Int(a), Int(b)) => Ok(Bool(a < b)),
        (Gt, Int(a), Int(b)) => Ok(Bool(a > b)),
        (Le, Int(a), Int(b)) => Ok(Bool(a <= b)),
        (Ge, Int(a), Int(b)) => Ok(Bool(a >= b)),
        (Implies, Bool(a), Bool(b)) => Ok(Bool(!a || b)),
        _ => Err(EvalError::Unsupported),
    }
}

// ===========================================================================
// 段階2: シード注入(#26 段階2)
//
// シードファイルは **入力のみ** を供給する。文法に期待値(`expected` / `output` /
// `result`)を書く構文が存在せず、書けばパーサが弾く——「オラクルは契約だけ、シードは
// 入力だけ」を**言語構造で**保証する(捏造不能性)。kei check はシード入力を対象関数の
// requires に照らし、違反シードを弾き、適合シードを ensures で検査する(注入)。
//
// 文法:
//   seeds for <fn> {
//     input { <field>: <literal>, ... }
//     input { ... }
//   }
// ===========================================================================

/// シードファイルを検査し、診断(文法エラー / requires 違反 / ensures 反例)を返す。
/// `file` は span に入れる相対パス、`source` はシードファイル本文。
pub fn check_seeds(file: &str, source: &str, module: &ast::Module) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let tokens = lex_seeds(source);
    let mut parser = SeedParser {
        toks: &tokens,
        pos: 0,
        file,
        diags: &mut diags,
    };
    let seeds = parser.parse();

    let funcs: HashMap<&str, &ast::FuncDecl> = module
        .items
        .iter()
        .filter_map(|it| match it {
            ast::Item::Func(f) => Some((f.name.name.as_str(), f)),
            _ => None,
        })
        .collect();

    for seed in &seeds {
        validate_seed(seed, &funcs, file, &mut diags);
    }
    diags.sort_by(|a, b| {
        (a.span.start.line, a.span.start.col).cmp(&(b.span.start.line, b.span.start.col))
    });
    diags
}

#[derive(Debug)]
struct SeedToken {
    kind: SeedTok,
    line: u32,
    col: u32,
    len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SeedTok {
    Ident(String),
    Int(i64),
    Str(String),
    LBrace,
    RBrace,
    Colon,
    Comma,
    Eof,
}

/// シードファイル用の最小トークナイザ(行・列を 1 始まりで保持)。
fn lex_seeds(src: &str) -> Vec<SeedToken> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0usize;
    let (mut line, mut col) = (1u32, 1u32);
    let advance = |c: char, line: &mut u32, col: &mut u32| {
        if c == '\n' {
            *line += 1;
            *col = 1;
        } else {
            *col += 1;
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' || c.is_whitespace() {
            advance(c, &mut line, &mut col);
            i += 1;
            continue;
        }
        // 行コメント `#`。
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            continue;
        }
        let (start_line, start_col) = (line, col);
        match c {
            '{' => {
                toks.push(SeedToken {
                    kind: SeedTok::LBrace,
                    line,
                    col,
                    len: 1,
                });
                col += 1;
                i += 1;
            }
            '}' => {
                toks.push(SeedToken {
                    kind: SeedTok::RBrace,
                    line,
                    col,
                    len: 1,
                });
                col += 1;
                i += 1;
            }
            ':' => {
                toks.push(SeedToken {
                    kind: SeedTok::Colon,
                    line,
                    col,
                    len: 1,
                });
                col += 1;
                i += 1;
            }
            ',' => {
                toks.push(SeedToken {
                    kind: SeedTok::Comma,
                    line,
                    col,
                    len: 1,
                });
                col += 1;
                i += 1;
            }
            '"' => {
                i += 1;
                col += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        col += 1;
                        s.push(match chars[i] {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                    } else {
                        s.push(chars[i]);
                    }
                    i += 1;
                    col += 1;
                }
                i += 1; // 閉じ "
                col += 1;
                let len = (col - start_col).max(1);
                toks.push(SeedToken {
                    kind: SeedTok::Str(s),
                    line: start_line,
                    col: start_col,
                    len,
                });
            }
            c if c == '-' || c.is_ascii_digit() => {
                let mut num = String::new();
                if c == '-' {
                    num.push('-');
                    i += 1;
                    col += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    num.push(chars[i]);
                    i += 1;
                    col += 1;
                }
                let value = num.parse::<i64>().unwrap_or(0);
                let len = (col - start_col).max(1);
                toks.push(SeedToken {
                    kind: SeedTok::Int(value),
                    line: start_line,
                    col: start_col,
                    len,
                });
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut id = String::new();
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    id.push(chars[i]);
                    i += 1;
                    col += 1;
                }
                let len = (col - start_col).max(1);
                toks.push(SeedToken {
                    kind: SeedTok::Ident(id),
                    line: start_line,
                    col: start_col,
                    len,
                });
            }
            _ => {
                // 未知文字は 1 つ食べて飛ばす(寛容)。
                col += 1;
                i += 1;
            }
        }
    }
    toks.push(SeedToken {
        kind: SeedTok::Eof,
        line,
        col,
        len: 0,
    });
    toks
}

#[derive(Debug)]
struct Seed {
    func: String,
    func_line: u32,
    func_col: u32,
    inputs: Vec<(String, Value)>,
    line: u32,
    col: u32,
}

struct SeedParser<'a> {
    toks: &'a [SeedToken],
    pos: usize,
    file: &'a str,
    diags: &'a mut Vec<Diagnostic>,
}

impl SeedParser<'_> {
    fn cur(&self) -> &SeedToken {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn bump(&mut self) -> &SeedToken {
        let p = self.pos.min(self.toks.len() - 1);
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        &self.toks[p]
    }

    fn err(&mut self, msg: String, line: u32, col: u32, len: u32, fix: &str) {
        let span = Span {
            file: self.file.to_string(),
            start: Position { line, col },
            end: Position {
                line,
                col: col + len,
            },
        };
        self.diags.push(
            Diagnostic::new(
                Severity::Error,
                seed_codes::SEED_GRAMMAR,
                msg,
                span,
                vec![Fix {
                    title: fix.to_string(),
                    edits: vec![],
                }],
            )
            .expect("seed grammar diagnostic carries a fix"),
        );
    }

    fn parse(&mut self) -> Vec<Seed> {
        let mut seeds = Vec::new();
        loop {
            match &self.cur().kind {
                SeedTok::Eof => break,
                SeedTok::Ident(id) if id == "seeds" => {
                    if let Some(block) = self.parse_block() {
                        seeds.extend(block);
                    } else {
                        break; // 回復不能
                    }
                }
                _ => {
                    let t = self.cur();
                    let (line, col, len) = (t.line, t.col, t.len);
                    self.err(
                        "expected 'seeds for <fn> { ... }'".to_string(),
                        line,
                        col,
                        len,
                        "Start a seed block with 'seeds for <fn>'",
                    );
                    break;
                }
            }
        }
        seeds
    }

    fn parse_block(&mut self) -> Option<Vec<Seed>> {
        self.bump(); // 'seeds'
                     // 'for'
        match &self.cur().kind {
            SeedTok::Ident(id) if id == "for" => {
                self.bump();
            }
            _ => {
                let t = self.cur();
                let (line, col, len) = (t.line, t.col, t.len);
                self.err(
                    "expected 'for' after 'seeds'".to_string(),
                    line,
                    col,
                    len,
                    "Write 'seeds for <fn>'",
                );
                return None;
            }
        }
        // 関数名
        let (func, fline, fcol) = match &self.cur().kind {
            SeedTok::Ident(id) => {
                let name = id.clone();
                let t = self.cur();
                let (l, c) = (t.line, t.col);
                self.bump();
                (name, l, c)
            }
            _ => {
                let t = self.cur();
                let (line, col, len) = (t.line, t.col, t.len);
                self.err(
                    "expected a function name".to_string(),
                    line,
                    col,
                    len,
                    "Name the function the seeds target",
                );
                return None;
            }
        };
        if !self.expect_lbrace() {
            return None;
        }
        let mut seeds = Vec::new();
        loop {
            match &self.cur().kind {
                SeedTok::RBrace => {
                    self.bump();
                    break;
                }
                SeedTok::Eof => {
                    let t = self.cur();
                    let (line, col) = (t.line, t.col);
                    self.err(
                        "unclosed seed block; expected '}'".to_string(),
                        line,
                        col,
                        1,
                        "Close the seed block with '}'",
                    );
                    break;
                }
                SeedTok::Ident(id) if id == "input" => {
                    if let Some(seed) = self.parse_input(&func, fline, fcol) {
                        seeds.push(seed);
                    } else {
                        return Some(seeds);
                    }
                }
                SeedTok::Ident(id) => {
                    // `expected` / `output` / `result` 等は捏造経路。文法上禁止であることを明示。
                    let bad = id.clone();
                    let t = self.cur();
                    let (line, col, len) = (t.line, t.col, t.len);
                    let msg = if matches!(
                        bad.as_str(),
                        "expected" | "output" | "result" | "returns"
                    ) {
                        format!("'{bad}' is not allowed: seed files supply inputs only — the oracle is the contract (ensures), never the seed")
                    } else {
                        format!("expected 'input {{ ... }}', found '{bad}'")
                    };
                    self.err(msg, line, col, len, "Provide only 'input { ... }' cases");
                    return Some(seeds);
                }
                _ => {
                    let t = self.cur();
                    let (line, col, len) = (t.line, t.col, t.len);
                    self.err(
                        "expected 'input { ... }' or '}'".to_string(),
                        line,
                        col,
                        len,
                        "Provide 'input { ... }' cases",
                    );
                    return Some(seeds);
                }
            }
        }
        Some(seeds)
    }

    fn parse_input(&mut self, func: &str, fline: u32, fcol: u32) -> Option<Seed> {
        let kw = self.bump(); // 'input'
        let (line, col) = (kw.line, kw.col);
        if !self.expect_lbrace() {
            return None;
        }
        let mut inputs = Vec::new();
        loop {
            match &self.cur().kind {
                SeedTok::RBrace => {
                    self.bump();
                    break;
                }
                SeedTok::Eof => {
                    let t = self.cur();
                    let (l, c) = (t.line, t.col);
                    self.err(
                        "unclosed 'input'; expected '}'".to_string(),
                        l,
                        c,
                        1,
                        "Close 'input' with '}'",
                    );
                    return None;
                }
                SeedTok::Ident(_) => {
                    let SeedTok::Ident(field) = self.bump().kind.clone() else {
                        unreachable!()
                    };
                    if !self.expect_colon() {
                        return None;
                    }
                    let value = self.parse_literal()?;
                    inputs.push((field, value));
                    // 任意のカンマ。
                    if self.cur().kind == SeedTok::Comma {
                        self.bump();
                    }
                }
                _ => {
                    let t = self.cur();
                    let (l, c, len) = (t.line, t.col, t.len);
                    self.err(
                        "expected a field name or '}'".to_string(),
                        l,
                        c,
                        len,
                        "Write '<field>: <value>'",
                    );
                    return None;
                }
            }
        }
        Some(Seed {
            func: func.to_string(),
            func_line: fline,
            func_col: fcol,
            inputs,
            line,
            col,
        })
    }

    fn parse_literal(&mut self) -> Option<Value> {
        match self.cur().kind.clone() {
            SeedTok::Int(n) => {
                self.bump();
                Some(Value::Int(n))
            }
            SeedTok::Str(s) => {
                self.bump();
                Some(Value::Str(s))
            }
            SeedTok::Ident(id) if id == "true" => {
                self.bump();
                Some(Value::Bool(true))
            }
            SeedTok::Ident(id) if id == "false" => {
                self.bump();
                Some(Value::Bool(false))
            }
            _ => {
                let t = self.cur();
                let (l, c, len) = (t.line, t.col, t.len);
                self.err(
                    "expected an Int, String, or Bool literal".to_string(),
                    l,
                    c,
                    len,
                    "Use a literal value (no expressions)",
                );
                None
            }
        }
    }

    fn expect_lbrace(&mut self) -> bool {
        if self.cur().kind == SeedTok::LBrace {
            self.bump();
            true
        } else {
            let t = self.cur();
            let (l, c, len) = (t.line, t.col, t.len);
            self.err(
                "expected '{'".to_string(),
                l,
                c,
                len,
                "Open a block with '{'",
            );
            false
        }
    }

    fn expect_colon(&mut self) -> bool {
        if self.cur().kind == SeedTok::Colon {
            self.bump();
            true
        } else {
            let t = self.cur();
            let (l, c, len) = (t.line, t.col, t.len);
            self.err(
                "expected ':' after the field name".to_string(),
                l,
                c,
                len,
                "Write '<field>: <value>'",
            );
            false
        }
    }
}

/// シード 1 件を対象関数の requires / 型 / 名前に照らす(注入)。違反は診断にする。
fn validate_seed(
    seed: &Seed,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    file: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let span = |line: u32, col: u32| Span {
        file: file.to_string(),
        start: Position { line, col },
        end: Position { line, col: col + 1 },
    };
    let invalid = |diags: &mut Vec<Diagnostic>, msg: String, line: u32, col: u32, fix: &str| {
        diags.push(
            Diagnostic::new(
                Severity::Error,
                seed_codes::SEED_INVALID,
                msg,
                span(line, col),
                vec![Fix {
                    title: fix.to_string(),
                    edits: vec![],
                }],
            )
            .expect("seed validation diagnostic carries a fix"),
        );
    };

    let Some(f) = funcs.get(seed.func.as_str()) else {
        invalid(
            diags,
            format!("seeds target unknown function '{}'", seed.func),
            seed.func_line,
            seed.func_col,
            "Reference a function defined in the module",
        );
        return;
    };

    // 入力を関数のパラメータへ対応づける(名前・個数・型をチェック)。
    let mut env: HashMap<String, Value> = HashMap::new();
    for (name, value) in &seed.inputs {
        match f.params.iter().find(|p| &p.name.name == name) {
            Some(p) => {
                if !value_matches_type(value, &p.ty) {
                    invalid(
                        diags,
                        format!("seed input '{name}' has the wrong type for '{}'", seed.func),
                        seed.line,
                        seed.col,
                        "Match the parameter's declared type",
                    );
                }
                env.insert(name.clone(), value.clone());
            }
            None => invalid(
                diags,
                format!("seed input '{name}' is not a parameter of '{}'", seed.func),
                seed.line,
                seed.col,
                "Use the function's parameter names",
            ),
        }
    }
    // 全パラメータが供給されているか。
    for p in &f.params {
        if !env.contains_key(&p.name.name) {
            invalid(
                diags,
                format!(
                    "seed for '{}' is missing input '{}'",
                    seed.func, p.name.name
                ),
                seed.line,
                seed.col,
                "Supply every parameter as an input",
            );
            return;
        }
    }

    // requires 適合(無効なシードを弾く)。評価不能な requires は寛容にスキップ。
    let inputs_text = || {
        seed.inputs
            .iter()
            .map(|(n, v)| format!("{n} = {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut requires_ok = true;
    for clause in &f.requires {
        match eval_bool(clause, &env, funcs, false) {
            Ok(true) => {}
            Ok(false) => {
                requires_ok = false;
                invalid(
                    diags,
                    format!(
                        "seed input ({}) does not satisfy requires '{}' of '{}'",
                        inputs_text(),
                        contract_expr_text(clause),
                        seed.func
                    ),
                    seed.line,
                    seed.col,
                    "Provide an input that satisfies the function's requires",
                );
            }
            Err(_) => {}
        }
    }

    // シード注入(#26 段階2): requires を満たすシードを ensures で検査する。生成器の固定
    // ドメイン外の人手の edge case でも、契約(オラクル)で反例を捕まえる。評価不能な関数
    // (record / Option 戻り等)は寛容にスキップ。捏造不能性: シードは入力だけ、判定は ensures。
    if requires_ok {
        let args: Vec<Value> = f
            .params
            .iter()
            .map(|p| env.get(&p.name.name).cloned().unwrap_or(Value::Int(0)))
            .collect();
        if let Ok(result) = eval_func_call(f, &args, funcs, 0) {
            let mut ens_env = env.clone();
            ens_env.insert("result".to_string(), result);
            for clause in &f.ensures {
                if let Ok(false) = eval_bool(clause, &ens_env, funcs, true) {
                    diags.push(
                        Diagnostic::new(
                            Severity::Error,
                            seed_codes::SEED_COUNTEREXAMPLE,
                            format!(
                                "ensures '{}' of '{}' is violated by the seeded input ({})",
                                contract_expr_text(clause),
                                seed.func,
                                inputs_text()
                            ),
                            span(seed.line, seed.col),
                            vec![Fix {
                                title:
                                    "Fix the implementation to satisfy the contract, or correct the contract"
                                        .to_string(),
                                edits: vec![],
                            }],
                        )
                        .expect("seed counterexample carries a fix"),
                    );
                }
            }
        }
    }
}

fn value_matches_type(v: &Value, t: &ast::Type) -> bool {
    if t.path.len() != 1 {
        return true; // 解決不能型は寛容
    }
    matches!(
        (v, t.path[0].name.as_str()),
        (Value::Int(_), "Int") | (Value::Bool(_), "Bool") | (Value::Str(_), "String")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(src: &str) -> ast::Module {
        let parsed = kei_syntax::parse_module(src);
        assert!(
            parsed.errors.is_empty(),
            "test source must parse: {:?}",
            parsed.errors
        );
        parsed.module
    }

    #[test]
    fn correct_function_passes() {
        let m = module(
            "module t\n\nfunc decrementAvailable(available: Int) -> Int\n  requires available > 0\n  ensures result == old(available) - 1\n{\n  return available - 1\n}\n",
        );
        let out = run_module(&m);
        assert_eq!(out.len(), 1);
        assert!(
            out[0].passed,
            "correct impl must pass: {:?}",
            out[0].counterexample
        );
        assert!(out[0].cases_checked > 0);
    }

    #[test]
    fn broken_function_reports_minimized_counterexample() {
        let m = module(
            "module t\n\nfunc decrementAvailable(available: Int) -> Int\n  requires available > 0\n  ensures result == old(available) - 1\n{\n  return available - 2\n}\n",
        );
        let out = run_module(&m);
        assert_eq!(out.len(), 1);
        assert!(!out[0].passed);
        let ce = out[0].counterexample.as_ref().expect("counterexample");
        // requires available > 0 を満たす最小の失敗入力は available = 1。
        assert_eq!(ce.inputs, vec![("available".to_string(), Value::Int(1))]);
    }

    #[test]
    fn effectful_function_is_skipped() {
        let m = module(
            "module t\n\nfunc f(x: Int) -> Int\n  uses Clock\n  ensures result == x\n{\n  return x\n}\n",
        );
        assert!(run_module(&m).is_empty());
    }

    fn decrement_module() -> ast::Module {
        module(
            "module t\n\nfunc decrementAvailable(available: Int) -> Int\n  requires available > 0\n  ensures result == old(available) - 1\n{\n  return available - 1\n}\n",
        )
    }

    #[test]
    fn valid_seed_passes() {
        let m = decrement_module();
        let src = "seeds for decrementAvailable {\n  input { available: 1 }\n  input { available: 42 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m);
        assert!(diags.is_empty(), "valid seeds should be clean: {diags:?}");
    }

    #[test]
    fn requires_violating_seed_is_rejected() {
        let m = decrement_module();
        // available: 0 は requires available > 0 を満たさない。
        let src = "seeds for decrementAvailable {\n  input { available: 0 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, seed_codes::SEED_INVALID);
        assert!(diags[0].message.contains("requires"));
    }

    #[test]
    fn expected_value_in_seed_is_a_grammar_error() {
        let m = decrement_module();
        // 期待値を持たせようとすると文法エラー(捏造不能性を構造で保証)。
        let src = "seeds for decrementAvailable {\n  expected { result: 0 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].code, seed_codes::SEED_GRAMMAR);
        assert!(diags[0].message.contains("inputs only"));
    }

    #[test]
    fn seed_violating_ensures_is_a_counterexample() {
        // 壊れた実装(2 減らす)に、requires を満たす(が生成ドメイン外の)シード 7 を当てると、
        // ensures 反例(KEI-E4005)になる。シード注入が ensures をオラクルに使う証拠。
        let m = module(
            "module t\n\nfunc decrementAvailable(available: Int) -> Int\n  requires available > 0\n  ensures result == old(available) - 1\n{\n  return available - 2\n}\n",
        );
        let src = "seeds for decrementAvailable {\n  input { available: 7 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m);
        assert!(
            diags
                .iter()
                .any(|d| d.code == seed_codes::SEED_COUNTEREXAMPLE),
            "a requires-satisfying seed that breaks ensures must be a counterexample: {diags:?}"
        );
    }

    #[test]
    fn valid_seed_against_correct_impl_has_no_counterexample() {
        // 正しい実装には、requires を満たすシードで反例が出ない。
        let m = decrement_module();
        let src = "seeds for decrementAvailable {\n  input { available: 7 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m);
        assert!(
            diags.is_empty(),
            "correct impl + valid seed = clean: {diags:?}"
        );
    }

    #[test]
    fn seed_for_unknown_function_is_rejected() {
        let m = decrement_module();
        let src = "seeds for nope {\n  input { available: 1 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m);
        assert!(diags
            .iter()
            .any(|d| d.code == seed_codes::SEED_INVALID && d.message.contains("unknown function")));
    }
}
