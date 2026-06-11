//! Pact の正規形フォーマッタ。AST の意味的変更は禁止(ARCHITECTURE.md)。
//!
//! 正規形の規則は docs/fmt-style.md に定義する。出力は span を除く AST のみの
//! 純関数であり、`fmt(fmt(x)) == fmt(x)`(冪等)かつ
//! `parse(fmt(parse(src))) == parse(src)`(span 除去比較)を満たす。
//!
//! 既知の制約: コメントはレキサーが捨てるため整形で失われる(v0.1 仕様)。

use pact_syntax::ast::*;
use pact_syntax::{Module, SyntaxError};

const INDENT: &str = "  ";

/// パース済みモジュールを正規形テキストにする。空モジュールは空文字列。
pub fn format_module(module: &Module) -> String {
    let mut chunks: Vec<String> = Vec::new();
    if let Some(decl) = &module.decl {
        chunks.push(format!("module {}", path_text(&decl.path)));
    }
    if !module.imports.is_empty() {
        let lines: Vec<String> = module.imports.iter().map(import_text).collect();
        chunks.push(lines.join("\n"));
    }
    for item in &module.items {
        chunks.push(item_text(item));
    }
    if chunks.is_empty() {
        return String::new();
    }
    let mut out = chunks.join("\n\n");
    out.push('\n');
    out
}

/// ソーステキストを整形する。構文エラーがある場合は整形せずエラーを返す
/// (壊れた入力を「それらしく」書き換えない)。
pub fn format_source(source: &str) -> Result<String, Vec<SyntaxError>> {
    let result = pact_syntax::parse_module(source);
    if !result.errors.is_empty() {
        return Err(result.errors);
    }
    Ok(format_module(&result.module))
}

// ---- 宣言 ----

fn path_text(path: &[Ident]) -> String {
    path.iter()
        .map(|i| i.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn import_text(import: &Import) -> String {
    let mut s = format!("import {}", path_text(&import.path));
    if !import.names.is_empty() {
        let names: Vec<&str> = import.names.iter().map(|i| i.name.as_str()).collect();
        s.push_str(" { ");
        s.push_str(&names.join(", "));
        s.push_str(" }");
    } else if let Some(alias) = &import.alias {
        s.push_str(" as ");
        s.push_str(&alias.name);
    }
    s
}

fn item_text(item: &Item) -> String {
    match item {
        Item::TypeAlias(alias) => type_alias_text(alias),
        Item::Record(record) => record_text(record),
        Item::Enum(decl) => enum_text(decl),
        Item::Func(func) => func_text(func),
    }
}

fn type_alias_text(alias: &TypeAlias) -> String {
    let mut s = format!("type {} = {}", alias.name.name, type_text(&alias.ty));
    if let Some(tag) = &alias.tag {
        s.push_str(" tagged ");
        s.push_str(&string_literal(tag));
    }
    s
}

fn record_text(record: &RecordDecl) -> String {
    if record.fields.is_empty() {
        return format!("record {} {{}}", record.name.name);
    }
    let mut s = format!("record {} {{\n", record.name.name);
    for field in &record.fields {
        s.push_str(INDENT);
        s.push_str(&field_def_text(field));
        s.push('\n');
    }
    s.push('}');
    s
}

fn enum_text(decl: &EnumDecl) -> String {
    if decl.variants.is_empty() {
        return format!("enum {} {{}}", decl.name.name);
    }
    let mut s = format!("enum {} {{\n", decl.name.name);
    for variant in &decl.variants {
        s.push_str(INDENT);
        s.push_str(&variant_text(variant));
        s.push('\n');
    }
    s.push('}');
    s
}

fn variant_text(variant: &Variant) -> String {
    let name = &variant.name.name;
    match &variant.payload {
        VariantPayload::Unit => name.clone(),
        VariantPayload::Tuple { types } => {
            let types: Vec<String> = types.iter().map(type_text).collect();
            format!("{}({})", name, types.join(", "))
        }
        VariantPayload::Record { fields } => {
            if fields.is_empty() {
                return format!("{name} {{}}");
            }
            let fields: Vec<String> = fields.iter().map(field_def_text).collect();
            format!("{} {{ {} }}", name, fields.join(", "))
        }
    }
}

fn field_def_text(field: &FieldDef) -> String {
    format!("{}: {}", field.name.name, type_text(&field.ty))
}

fn type_text(ty: &Type) -> String {
    let mut s = path_text(&ty.path);
    if !ty.args.is_empty() {
        let args: Vec<String> = ty.args.iter().map(type_text).collect();
        s.push('<');
        s.push_str(&args.join(", "));
        s.push('>');
    }
    s
}

// ---- 関数 ----

fn func_text(func: &FuncDecl) -> String {
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.name, type_text(&p.ty)))
        .collect();
    let mut s = format!("func {}({})", func.name.name, params.join(", "));
    if let Some(ret) = &func.ret {
        s.push_str(" -> ");
        s.push_str(&type_text(ret));
    }

    let has_clauses =
        !(func.uses.is_empty() && func.requires.is_empty() && func.ensures.is_empty());
    if !func.uses.is_empty() {
        let effects: Vec<String> = func.uses.iter().map(|e| path_text(&e.path)).collect();
        s.push_str(&format!("\n{INDENT}uses {}", effects.join(", ")));
    }
    for expr in &func.requires {
        s.push_str(&format!("\n{INDENT}requires {}", expr_text(expr, 0, false)));
    }
    for expr in &func.ensures {
        s.push_str(&format!("\n{INDENT}ensures {}", expr_text(expr, 0, false)));
    }

    // 契約節があれば `{` は独立行、なければシグネチャ行に続ける。
    s.push(if has_clauses { '\n' } else { ' ' });
    s.push_str(&block_text(&func.body, 0));
    s
}

// ---- 文 ----

fn block_text(block: &Block, level: usize) -> String {
    if block.stmts.is_empty() {
        return "{}".to_string();
    }
    let mut s = String::from("{\n");
    for stmt in &block.stmts {
        s.push_str(&INDENT.repeat(level + 1));
        s.push_str(&stmt_text(stmt, level + 1));
        s.push('\n');
    }
    s.push_str(&INDENT.repeat(level));
    s.push('}');
    s
}

fn stmt_text(stmt: &Stmt, level: usize) -> String {
    match stmt {
        Stmt::Let(stmt) => let_text(stmt),
        Stmt::If(stmt) => if_text(stmt, level),
        Stmt::Return(stmt) => match &stmt.value {
            Some(expr) => format!("return {}", expr_text(expr, 0, false)),
            None => "return".to_string(),
        },
        Stmt::Expr(stmt) => expr_text(&stmt.expr, 0, false),
    }
}

fn let_text(stmt: &LetStmt) -> String {
    let mut s = format!("let {}", stmt.name.name);
    if let Some(ty) = &stmt.ty {
        s.push_str(": ");
        s.push_str(&type_text(ty));
    }
    s.push_str(" = ");
    s.push_str(&expr_text(&stmt.value, 0, false));
    if let Some(fail) = &stmt.else_fail {
        s.push_str(" else fail ");
        s.push_str(&expr_text(fail, 0, false));
    }
    s
}

fn if_text(stmt: &IfStmt, level: usize) -> String {
    // if 条件はパーサが record リテラルを禁止する文脈(no_struct)。
    let mut s = format!(
        "if {} {}",
        expr_text(&stmt.cond, 0, true),
        block_text(&stmt.then_block, level)
    );
    match &stmt.else_branch {
        None => {}
        Some(ElseBranch::Block(block)) => {
            s.push_str(" else ");
            s.push_str(&block_text(block, level));
        }
        Some(ElseBranch::If(nested)) => {
            s.push_str(" else ");
            s.push_str(&if_text(nested, level));
        }
    }
    s
}

// ---- 式 ----

/// 二項演算子の優先順位(数値が大きいほど強く結合)。
fn bin_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Implies => 1,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 2,
        BinOp::Add | BinOp::Sub => 3,
        BinOp::Mul | BinOp::Div => 4,
    }
}

fn bin_op_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Implies => "implies",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
    }
}

fn prec(expr: &Expr) -> u8 {
    match expr {
        Expr::Binary { op, .. } => bin_prec(*op),
        Expr::Unary { .. } => 5,
        Expr::Field { .. } | Expr::Call { .. } | Expr::RecordLit { .. } => 6,
        Expr::Int { .. } | Expr::Str { .. } | Expr::Bool { .. } | Expr::Name { .. } => 7,
    }
}

/// `min_prec` 未満の結合強度なら括弧で包む。`no_struct` は if 条件と同じ
/// 「record リテラル禁止」文脈の伝播(括弧・引数・フィールド値で解除)。
fn expr_text(expr: &Expr, min_prec: u8, no_struct: bool) -> String {
    let needs_paren =
        prec(expr) < min_prec || (no_struct && matches!(expr, Expr::RecordLit { .. }));
    if needs_paren {
        return format!("({})", expr_text(expr, 0, false));
    }
    match expr {
        Expr::Int { value, .. } => value.to_string(),
        Expr::Str { value, .. } => string_literal(value),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Name { name, .. } => name.clone(),
        Expr::Field { base, name, .. } => {
            format!("{}.{}", expr_text(base, 6, no_struct), name.name)
        }
        Expr::Call { callee, args, .. } => {
            let args: Vec<String> = args.iter().map(|a| expr_text(a, 0, false)).collect();
            format!("{}({})", expr_text(callee, 6, no_struct), args.join(", "))
        }
        Expr::Unary { op, expr, .. } => {
            let op = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            format!("{}{}", op, expr_text(expr, 5, no_struct))
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let p = bin_prec(*op);
            // implies のみ右結合(parser::parse_implies)。
            let (lhs_min, rhs_min) = if *op == BinOp::Implies {
                (p + 1, p)
            } else {
                (p, p + 1)
            };
            format!(
                "{} {} {}",
                expr_text(lhs, lhs_min, no_struct),
                bin_op_text(*op),
                expr_text(rhs, rhs_min, no_struct)
            )
        }
        Expr::RecordLit { path, fields, .. } => {
            let head = path_text(path);
            if fields.is_empty() {
                return format!("{head} {{}}");
            }
            let fields: Vec<String> = fields
                .iter()
                .map(|f| match &f.value {
                    Some(value) => format!("{}: {}", f.name.name, expr_text(value, 0, false)),
                    None => f.name.name.clone(),
                })
                .collect();
            format!("{} {{ {} }}", head, fields.join(", "))
        }
    }
}

/// レキサーのエスケープ仕様(\" \\ \n \t \r)と対になる文字列リテラル化。
fn string_literal(value: &str) -> String {
    let mut s = String::with_capacity(value.len() + 2);
    s.push('"');
    for c in value.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\t' => s.push_str("\\t"),
            '\r' => s.push_str("\\r"),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}
