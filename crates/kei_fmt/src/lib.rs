//! Kei の正規形フォーマッタ。AST の意味的変更は禁止(ARCHITECTURE.md)。
//!
//! 正規形の規則は docs/fmt-style.md に定義する。出力は span を除く AST のみの
//! 純関数であり、`fmt(fmt(x)) == fmt(x)`(冪等)かつ
//! `parse(fmt(parse(src))) == parse(src)`(span 除去比較)を満たす。
//!
//! M19 以降、`//` 行コメントは `format_source` 経由でソース行から
//! 再構築されて保持される。`format_module(&Module)` 単独は AST だけを
//! 入力に取るため、コメントは持たない(proptest 等の純粋経路向け)。

use kei_syntax::ast::*;
use kei_syntax::{Comment, Module, SyntaxError};

const INDENT: &str = "  ";

/// パース済みモジュールを正規形テキストにする(コメントなし)。
/// 空モジュールは空文字列。`format_source` 経由の場合はコメントが
/// 保持されるが、こちらは AST のみを入力に取るため失われる(proptest 用)。
pub fn format_module(module: &Module) -> String {
    format_with_comments(module, &[])
}

/// ソーステキストを整形する。構文エラーがある場合は整形せずエラーを返す
/// (壊れた入力を「それらしく」書き換えない)。`//` コメントは保持される(M19)。
pub fn format_source(source: &str) -> Result<String, Vec<SyntaxError>> {
    let result = kei_syntax::parse_module(source);
    if !result.errors.is_empty() {
        return Err(result.errors);
    }
    Ok(format_with_comments(&result.module, &result.comments))
}

/// コメント付きでモジュールを整形する。`comments` はソース順。
/// `format_module` / `format_source` の共通実装で、外部からは呼ばない
/// (PR #73 review: 公開する必然性が無いので `pub(crate)`)。
pub(crate) fn format_with_comments(module: &Module, comments: &[Comment]) -> String {
    let mut fmt = Fmt::new(comments);
    fmt.emit_module(module);
    fmt.finish()
}

// ---- 中核: コメント付きフォーマッタ ----

struct Fmt<'a> {
    out: String,
    comments: &'a [Comment],
    cursor: usize,
    /// 次のチャンクの前にセクション区切り(空行)が必要か。
    need_separator: bool,
    /// 最後に emit したトップレベル要素の終端行(`finish` で末尾コメントの
    /// 直前に空行を入れるかをソース行間で決めるため。PR #73 review)。
    last_top_line: Option<u32>,
}

impl<'a> Fmt<'a> {
    fn new(comments: &'a [Comment]) -> Self {
        Self {
            out: String::new(),
            comments,
            cursor: 0,
            need_separator: false,
            last_top_line: None,
        }
    }

    fn finish(mut self) -> String {
        // 末尾の余剰コメントを取り込む。PR #73 review: 旧実装は常に
        // "\n\n" で区切っていたため、最後のチャンク直後に続くコメントでも
        // 空行が挿入されていた。ソース行間(gap)を見て決める:
        //   gap >= 2 → 空行を挟む(ソース上で意図的に間を空けていた)
        //   gap == 1 → 改行 1 つだけ(ソース上で直後に書かれていた)
        if self.cursor < self.comments.len() {
            let comment_line = self.comments[self.cursor].span.start.line;
            let gap = self
                .last_top_line
                .map(|l| comment_line.saturating_sub(l))
                .unwrap_or(0);
            if !self.out.is_empty() && !self.out.ends_with('\n') {
                self.out.push('\n');
            }
            if gap >= 2 && !self.out.is_empty() {
                self.out.push('\n');
            }
            self.need_separator = false;
            self.flush_remaining(0);
        }
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.out
    }

    // ---- 出力ヘルパ ----

    fn push(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn newline(&mut self) {
        self.out.push('\n');
    }

    fn indent(&mut self, level: usize) {
        for _ in 0..level {
            self.out.push_str(INDENT);
        }
    }

    fn write_separator_if_needed(&mut self) {
        if self.need_separator {
            self.out.push_str("\n\n");
            self.need_separator = false;
        }
    }

    // ---- コメントストリーム ----

    fn peek(&self) -> Option<&'a Comment> {
        self.comments.get(self.cursor)
    }

    fn advance(&mut self) {
        self.cursor += 1;
    }

    /// `start_line` 未満の行にあるコメントを drain し、各行に
    /// `indent` 段のインデントを付けて emit する(各行末は `\n`)。
    /// 呼び出し時点で出力末尾が行頭(`\n` か空)であること。
    fn flush_leading(&mut self, start_line: u32, level: usize) {
        while let Some(c) = self.peek() {
            if c.span.start.line >= start_line {
                break;
            }
            self.indent(level);
            self.out.push_str("//");
            self.out.push_str(&c.text);
            self.out.push('\n');
            self.advance();
        }
    }

    /// 出力末尾(行内)に同行コメントを追記する。
    fn flush_trailing_on(&mut self, line: u32) {
        if let Some(c) = self.peek() {
            if c.span.start.line == line {
                self.out.push_str(" //");
                self.out.push_str(&c.text);
                self.advance();
            }
        }
    }

    fn flush_remaining(&mut self, level: usize) {
        let mut first = true;
        while let Some(c) = self.peek() {
            if !first {
                self.out.push('\n');
            }
            first = false;
            self.indent(level);
            self.out.push_str("//");
            self.out.push_str(&c.text);
            self.advance();
        }
    }

    // ---- モジュール ----

    fn emit_module(&mut self, m: &Module) {
        // ファイル頭部の leading(モジュール宣言 / 最初の import / 最初の item の前)
        if let Some(first_line) = earliest_top_line(m) {
            self.flush_leading(first_line, 0);
        }

        if let Some(decl) = &m.decl {
            self.write_separator_if_needed();
            self.push("module ");
            self.push(&path_text(&decl.path));
            self.flush_trailing_on(decl.span.end.line);
            self.need_separator = true;
            self.last_top_line = Some(decl.span.end.line);
        }

        if !m.imports.is_empty() {
            self.write_separator_if_needed();
            for (i, import) in m.imports.iter().enumerate() {
                if i > 0 {
                    self.newline();
                }
                // import 群の途中に出現する leading コメントはセクション内インライン
                self.flush_leading(import.span.start.line, 0);
                self.push(&import_text(import));
                self.flush_trailing_on(import.span.end.line);
                self.last_top_line = Some(import.span.end.line);
            }
            self.need_separator = true;
        }

        let mut i = 0;
        while i < m.items.len() {
            if matches!(m.items[i], Item::Extern(_)) {
                self.write_separator_if_needed();
                let mut first_in_group = true;
                while let Some(Item::Extern(e)) = m.items.get(i) {
                    if !first_in_group {
                        self.newline();
                    }
                    first_in_group = false;
                    self.flush_leading(e.span.start.line, 0);
                    self.push(&extern_text(e));
                    self.flush_trailing_on(e.span.end.line);
                    self.last_top_line = Some(e.span.end.line);
                    i += 1;
                }
                self.need_separator = true;
            } else {
                let item = &m.items[i];
                self.write_separator_if_needed();
                self.flush_leading(item.span().start.line, 0);
                self.emit_item(item);
                self.flush_trailing_on(item.span().end.line);
                self.last_top_line = Some(item.span().end.line);
                self.need_separator = true;
                i += 1;
            }
        }
    }

    // ---- 項目 ----

    fn emit_item(&mut self, item: &Item) {
        match item {
            Item::TypeAlias(t) => self.push(&type_alias_text(t)),
            Item::Record(r) => self.emit_record(r),
            Item::Enum(e) => self.emit_enum(e),
            Item::Func(f) => self.emit_func(f),
            Item::Extern(e) => self.push(&extern_text(e)),
        }
    }

    /// record 本体を Fmt メソッドとして出力(M19 / PR #73 review)。
    /// 旧 `record_text` は AST だけのテキスト生成だったため、フィールド間の
    /// `// comment` が走査されず消えていた。フィールド行ごとに
    /// leading/trailing flush を入れ、最後のフィールドと `}` の間にも
    /// 残コメントを取り込む。
    fn emit_record(&mut self, r: &RecordDecl) {
        if r.fields.is_empty() {
            self.push(&format!("record {} {{}}", r.name.name));
            return;
        }
        self.push(&format!("record {} {{", r.name.name));
        self.newline();
        for field in &r.fields {
            self.flush_leading(field.span.start.line, 1);
            self.indent(1);
            self.push(&field_def_text(field));
            self.flush_trailing_on(field.span.end.line);
            self.newline();
        }
        // 最終フィールドの行と `}` の行の間に残った leading コメントを拾う。
        self.flush_leading(r.span.end.line, 1);
        self.push("}");
    }

    /// enum 本体を Fmt メソッドとして出力(M19 / PR #73 review)。`emit_record`
    /// と同じ理由でバリアント間 / 末尾 `}` 前のコメントを保持する。
    fn emit_enum(&mut self, decl: &EnumDecl) {
        if decl.variants.is_empty() {
            self.push(&format!("enum {} {{}}", decl.name.name));
            return;
        }
        self.push(&format!("enum {} {{", decl.name.name));
        self.newline();
        for variant in &decl.variants {
            self.flush_leading(variant.span.start.line, 1);
            self.indent(1);
            self.push(&variant_text(variant));
            self.flush_trailing_on(variant.span.end.line);
            self.newline();
        }
        self.flush_leading(decl.span.end.line, 1);
        self.push("}");
    }

    fn emit_func(&mut self, func: &FuncDecl) {
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name.name, type_text(&p.ty)))
            .collect();
        self.push(&format!("func {}({})", func.name.name, params.join(", ")));
        if let Some(ret) = &func.ret {
            self.push(" -> ");
            self.push(&type_text(ret));
        }

        let has_clauses =
            !(func.uses.is_empty() && func.requires.is_empty() && func.ensures.is_empty());

        // PR #73 review: シグネチャ行末の trailing コメントを取り込んでから改行する
        // (これを忘れると後続の `flush_leading(requires.start)` がシグネチャ行の
        //  comment を silently 消費して misplace する)。has_clauses=false の場合は
        // `{` がシグネチャと同じ行に来るので、シグネチャ行 trailing は実は `{` 同行
        // trailing と区別がつかない。その場合は emit_block の open brace 取り込みに
        // 委ねて、ここでは取らない。
        let sig_end_line = func
            .ret
            .as_ref()
            .map(|r| r.span.end.line)
            .or_else(|| func.params.last().map(|p| p.span.end.line))
            .unwrap_or(func.name.span.end.line);
        if has_clauses {
            self.flush_trailing_on(sig_end_line);
        }

        if !func.uses.is_empty() {
            self.newline();
            // M19 / PR #73 review: uses 行の直上にも leading コメントを取り込む。
            // 旧実装は uses 行手前で flush_leading を呼ばなかったため、その位置の
            // コメントが後段の requires 冒頭まで遅延して付け替えられていた。
            let first_uses_line = func.uses[0].span.start.line;
            self.flush_leading(first_uses_line, 1);
            self.indent(1);
            self.push("uses ");
            let effects: Vec<String> = func.uses.iter().map(|e| path_text(&e.path)).collect();
            self.push(&effects.join(", "));
            if let Some(last) = func.uses.last() {
                self.flush_trailing_on(last.span.end.line);
            }
        }

        for expr in &func.requires {
            self.newline();
            let start = expr.span().start.line;
            self.flush_leading(start, 1);
            self.indent(1);
            self.push("requires ");
            self.push(&expr_text(expr, 0, false, 1));
            self.flush_trailing_on(expr.span().end.line);
        }
        for expr in &func.ensures {
            self.newline();
            let start = expr.span().start.line;
            self.flush_leading(start, 1);
            self.indent(1);
            self.push("ensures ");
            self.push(&expr_text(expr, 0, false, 1));
            self.flush_trailing_on(expr.span().end.line);
        }

        if has_clauses {
            self.newline();
        } else {
            self.push(" ");
        }
        self.emit_block(&func.body, 0);
    }

    // ---- ブロック・文 ----

    fn emit_block(&mut self, block: &Block, level: usize) {
        self.emit_block_inner(block, level, /* close_trailing = */ true);
    }

    /// `emit_block` の内部実装。`close_trailing` が false のときは `}` 同行 trailing を
    /// **取り込まない**(emit_if の then-block 側で、後続の `else` 連結のために
    /// 呼び出し側が改行を制御する)。
    fn emit_block_inner(&mut self, block: &Block, level: usize, close_trailing: bool) {
        if block.stmts.is_empty() {
            let has_inside = match self.peek() {
                Some(c) => c.span.start.line < block.span.end.line,
                None => false,
            };
            if !has_inside {
                self.push("{}");
                if close_trailing {
                    self.flush_trailing_on(block.span.end.line);
                }
                return;
            }
            self.push("{");
            // open brace 同行 trailing を取り込む(PR #73 review)。
            self.flush_trailing_on(block.span.start.line);
            self.newline();
            self.flush_leading(block.span.end.line, level + 1);
            self.indent(level);
            self.push("}");
            if close_trailing {
                self.flush_trailing_on(block.span.end.line);
            }
            return;
        }

        self.push("{");
        // open brace 同行 trailing を取り込む。これを取り損ねると、後段の
        // `flush_leading(stmt.start)` が `{` 同行コメントを body 内 leading として
        // 吸収してしまう(`func f() { // hi` のケース)。PR #73 review。
        self.flush_trailing_on(block.span.start.line);
        self.newline();
        for stmt in &block.stmts {
            let span = stmt.span();
            self.flush_leading(span.start.line, level + 1);
            self.indent(level + 1);
            self.emit_stmt(stmt, level + 1);
            self.flush_trailing_on(span.end.line);
            self.newline();
        }
        self.flush_leading(block.span.end.line, level + 1);
        self.indent(level);
        self.push("}");
        if close_trailing {
            self.flush_trailing_on(block.span.end.line);
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt, level: usize) {
        match stmt {
            Stmt::Let(s) => self.push(&let_text(s, level)),
            Stmt::Return(s) => match &s.value {
                Some(expr) => {
                    self.push("return ");
                    self.push(&expr_text(expr, 0, false, level));
                }
                None => self.push("return"),
            },
            Stmt::Expr(s) => self.push(&expr_text(&s.expr, 0, false, level)),
            Stmt::If(s) => self.emit_if(s, level),
        }
    }

    fn emit_if(&mut self, stmt: &IfStmt, level: usize) {
        // if 条件はパーサが record リテラルを禁止する文脈(no_struct)。
        self.push("if ");
        self.push(&expr_text(&stmt.cond, 0, true, level));
        self.push(" ");
        // else が続くとき、then-block の `}` 同行 trailing は emit_if 側で
        // 取り扱う(取ってから改行+インデントを挟み、else を別行に出す)。
        // emit_block 内で取ると `} // hi else { ... }` のような壊れた連結に
        // なる(PR #73 review: emit_if が then-block `}` 直後のコメントを
        // else 側に流入させる問題への対応)。
        let has_else = stmt.else_branch.is_some();
        self.emit_block_inner(&stmt.then_block, level, !has_else);
        match &stmt.else_branch {
            None => {}
            Some(branch) => {
                let close_line = stmt.then_block.span.end.line;
                let has_close_trailing =
                    self.peek().is_some_and(|c| c.span.start.line == close_line);
                if has_close_trailing {
                    // `} // trailing then\n  else { ... }`
                    self.flush_trailing_on(close_line);
                    self.newline();
                    self.indent(level);
                } else {
                    self.push(" ");
                }
                self.push("else ");
                match branch {
                    ElseBranch::Block(block) => self.emit_block(block, level),
                    ElseBranch::If(nested) => self.emit_if(nested, level),
                }
            }
        }
    }
}

/// 最初に現れる top-level 要素の開始行(コメントの先頭ドレインに使う)。
fn earliest_top_line(m: &Module) -> Option<u32> {
    if let Some(d) = &m.decl {
        return Some(d.span.start.line);
    }
    if let Some(i) = m.imports.first() {
        return Some(i.span.start.line);
    }
    if let Some(it) = m.items.first() {
        return Some(it.span().start.line);
    }
    None
}

// ---- 既存テキスト生成ヘルパ群(AST のみで完結する純関数) ----

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

fn extern_text(decl: &ExternDecl) -> String {
    let params: Vec<String> = decl
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.name, type_text(&p.ty)))
        .collect();
    let kw = if decl.query { "extern query" } else { "extern" };
    let mut s = format!("{kw} {}({})", path_text(&decl.path), params.join(", "));
    if let Some(ret) = &decl.ret {
        s.push_str(" -> ");
        s.push_str(&type_text(ret));
    }
    if !decl.uses.is_empty() {
        let effects: Vec<String> = decl.uses.iter().map(|e| path_text(&e.path)).collect();
        s.push_str(" uses ");
        s.push_str(&effects.join(", "));
    }
    s
}

fn type_alias_text(alias: &TypeAlias) -> String {
    let mut s = format!("type {} = {}", alias.name.name, type_text(&alias.ty));
    if let Some(tag) = &alias.tag {
        s.push_str(" tagged ");
        s.push_str(&string_literal(tag));
    }
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

fn let_text(stmt: &LetStmt, level: usize) -> String {
    let mut s = format!("let {}", stmt.name.name);
    if let Some(ty) = &stmt.ty {
        s.push_str(": ");
        s.push_str(&type_text(ty));
    }
    s.push_str(" = ");
    s.push_str(&expr_text(&stmt.value, 0, false, level));
    if let Some(fail) = &stmt.else_fail {
        s.push_str(" else fail ");
        s.push_str(&expr_text(fail, 0, false, level));
    }
    s
}

// ---- 式 ----

/// 二項演算子の優先順位(数値が大きいほど強く結合)。
fn bin_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Implies => 1,
        BinOp::Or => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 5,
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
        BinOp::Rem => "%",
        BinOp::Or => "||",
    }
}

fn prec(expr: &Expr) -> u8 {
    match expr {
        Expr::Binary { op, .. } => bin_prec(*op),
        Expr::Unary { .. } => 5,
        Expr::Field { .. } | Expr::Call { .. } | Expr::RecordLit { .. } => 6,
        Expr::Int { .. }
        | Expr::Str { .. }
        | Expr::Bool { .. }
        | Expr::Name { .. }
        | Expr::Match { .. }
        | Expr::ListLit { .. } => 7,
    }
}

/// `min_prec` 未満の結合強度なら括弧で包む。`no_struct` は if 条件と同じ
/// 「record リテラル禁止」文脈の伝播(括弧・引数・フィールド値で解除)。
/// `level` は式が始まる行のインデント段数(`match` の複数行展開に使う)。
fn expr_text(expr: &Expr, min_prec: u8, no_struct: bool, level: usize) -> String {
    let needs_paren =
        prec(expr) < min_prec || (no_struct && matches!(expr, Expr::RecordLit { .. }));
    if needs_paren {
        return format!("({})", expr_text(expr, 0, false, level));
    }
    match expr {
        Expr::Int { value, .. } => value.to_string(),
        Expr::Str { value, .. } => string_literal(value),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Name { name, .. } => name.clone(),
        Expr::Field { base, name, .. } => {
            format!("{}.{}", expr_text(base, 6, no_struct, level), name.name)
        }
        Expr::Call { callee, args, .. } => {
            let args: Vec<String> = args.iter().map(|a| expr_text(a, 0, false, level)).collect();
            format!(
                "{}({})",
                expr_text(callee, 6, no_struct, level),
                args.join(", ")
            )
        }
        Expr::Unary { op, expr, .. } => {
            let op = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            format!("{}{}", op, expr_text(expr, 6, no_struct, level))
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
                expr_text(lhs, lhs_min, no_struct, level),
                bin_op_text(*op),
                expr_text(rhs, rhs_min, no_struct, level)
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
                    Some(value) => {
                        format!("{}: {}", f.name.name, expr_text(value, 0, false, level))
                    }
                    None => f.name.name.clone(),
                })
                .collect();
            format!("{} {{ {} }}", head, fields.join(", "))
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            // scrutinee はパーサが record リテラルを禁止する文脈(no_struct)。
            let mut s = format!("match {} {{\n", expr_text(scrutinee, 0, true, level));
            for arm in arms {
                s.push_str(&INDENT.repeat(level + 1));
                s.push_str(&match_arm_text(arm, level + 1));
                s.push('\n');
            }
            s.push_str(&INDENT.repeat(level));
            s.push('}');
            s
        }
        Expr::ListLit { elements, .. } => {
            let elems: Vec<String> = elements
                .iter()
                .map(|e| expr_text(e, 0, false, level))
                .collect();
            format!("[{}]", elems.join(", "))
        }
    }
}

fn match_arm_text(arm: &MatchArm, level: usize) -> String {
    format!(
        "{} => {}",
        pattern_text(&arm.pattern),
        expr_text(&arm.body, 0, false, level)
    )
}

fn pattern_text(pat: &Pattern) -> String {
    let head = path_text(&pat.path);
    match &pat.payload {
        PatternPayload::Unit => head,
        PatternPayload::Tuple { bindings } => {
            let bs: Vec<&str> = bindings.iter().map(|i| i.name.as_str()).collect();
            format!("{head}({})", bs.join(", "))
        }
        PatternPayload::Record { fields } => {
            let fs: Vec<&str> = fields.iter().map(|i| i.name.as_str()).collect();
            format!("{head} {{ {} }}", fs.join(", "))
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
