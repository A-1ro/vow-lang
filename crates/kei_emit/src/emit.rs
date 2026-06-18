//! 検査済み AST → TypeScript の生成本体。
//!
//! 検査(型・名前・エフェクト)は kei_check 済みであることを前提とし、
//! ここでは構文情報のみを使って出力を組み立てる(検査の再実装禁止)。

use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

use kei_check::contract_expr_text as kei_expr_text;
use kei_syntax::ast::{self, BinOp, UnaryOp};
use kei_syntax::Span;

use crate::sourcemap::{self, Mapping};
use crate::EmitOutput;

const RUNTIME_MODULE: &str = "@kei/runtime";
const INDENT: &str = "  ";

/// `list_ops` は kei_check が確定した「List コンビネータ呼び出し位置」(Call span の開始
/// `(line, col)`)の集合。emit はこれだけを根拠に配列メソッドへ写す(構文ヒューリスティックで
/// レシーバ型を推測しない。外部呼び出しの連鎖・同名フィールドの誤写を防ぐ。M9)。
pub fn emit_checked(
    file: &str,
    source: &str,
    module: &ast::Module,
    list_ops: &HashSet<(u32, u32)>,
) -> EmitOutput {
    let ts_path = ts_path_for(file, module);
    let ts_file = ts_path.rsplit('/').next().expect("non-empty path");
    let mut e = Emitter {
        src_file: file,
        module,
        list_ops,
        out: Out::default(),
        old_counter: 0,
        in_ensures: false,
        match_counter: 0,
    };
    e.emit_module();
    let mut ts = e.out.buf;
    let map = sourcemap::build_map(ts_file, file, source, &e.out.mappings);
    let _ = writeln!(ts, "//# sourceMappingURL={ts_file}.map");
    EmitOutput { ts, map, ts_path }
}

/// 出力先相対パス。モジュール宣言(ファイルパスと 1:1)を優先し、
/// 宣言がない場合は入力ファイル名の拡張子だけを差し替える。
fn ts_path_for(file: &str, module: &ast::Module) -> String {
    if let Some(decl) = &module.decl {
        let parts: Vec<&str> = decl.path.iter().map(|i| i.name.as_str()).collect();
        return format!("{}.ts", parts.join("/"));
    }
    let stem = file
        .rsplit('/')
        .next()
        .and_then(|n| n.strip_suffix(".kei"))
        .unwrap_or("module");
    format!("{stem}.ts")
}

// ---------------------------------------------------------------------------
// 出力バッファ + source map 記録
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Out {
    buf: String,
    line: u32,
    col: u32,
    indent: usize,
    mappings: Vec<Mapping>,
}

impl Out {
    fn frag(&mut self, text: &str) {
        debug_assert!(!text.contains('\n'), "frag must not contain newlines");
        self.buf.push_str(text);
        self.col += text.chars().count() as u32;
    }

    fn newline(&mut self) {
        self.buf.push('\n');
        self.line += 1;
        self.col = 0;
    }

    fn start_line(&mut self) {
        for _ in 0..self.indent {
            self.frag(INDENT);
        }
    }

    /// 単独行を書く(マッピングなし)。
    fn line(&mut self, text: &str) {
        self.start_line();
        self.frag(text);
        self.newline();
    }

    /// 現在の出力位置 → `span.start` のマッピングを記録する。
    fn map(&mut self, span: Span) {
        self.mappings.push(Mapping {
            gen_line: self.line,
            gen_col: self.col,
            src_line: span.start.line.saturating_sub(1),
            src_col: span.start.col.saturating_sub(1),
        });
    }
}

// ---------------------------------------------------------------------------
// ランタイム import の収集(構文走査のみ)
// ---------------------------------------------------------------------------

struct RuntimeUses<'a> {
    names: BTreeSet<&'static str>,
    /// List コンビネータ呼び出し位置(検査器由来)。`get` の keiListGet import 判定に使う。
    list_ops: &'a HashSet<(u32, u32)>,
}

impl<'a> RuntimeUses<'a> {
    fn collect(module: &ast::Module, list_ops: &'a HashSet<(u32, u32)>) -> Self {
        let mut u = RuntimeUses {
            names: BTreeSet::new(),
            list_ops,
        };
        for item in &module.items {
            match item {
                ast::Item::TypeAlias(a) => u.ty(&a.ty),
                ast::Item::Record(r) => {
                    for f in &r.fields {
                        u.ty(&f.ty);
                    }
                }
                ast::Item::Enum(e) => {
                    for v in &e.variants {
                        match &v.payload {
                            ast::VariantPayload::Unit => {}
                            ast::VariantPayload::Tuple { types } => {
                                types.iter().for_each(|t| u.ty(t))
                            }
                            ast::VariantPayload::Record { fields } => {
                                fields.iter().for_each(|f| u.ty(&f.ty))
                            }
                        }
                    }
                }
                ast::Item::Func(f) => {
                    for p in &f.params {
                        u.ty(&p.ty);
                    }
                    if let Some(ret) = &f.ret {
                        u.ty(ret);
                    }
                    if !f.requires.is_empty() || !f.ensures.is_empty() {
                        u.names.insert("KeiContractViolation");
                    }
                    for c in f.requires.iter().chain(&f.ensures) {
                        u.expr(c);
                    }
                    u.block(&f.body);
                }
                // extern は型・エフェクトの署名のみ。TS には何も生成しない。
                ast::Item::Extern(_) => {}
            }
        }
        u
    }

    fn ty(&mut self, t: &ast::Type) {
        if t.path.len() == 1 {
            match t.path[0].name.as_str() {
                "Result" => {
                    self.names.insert("Result");
                }
                "Option" => {
                    self.names.insert("Option");
                }
                _ => {}
            }
        }
        for a in &t.args {
            self.ty(a);
        }
    }

    fn block(&mut self, b: &ast::Block) {
        for s in &b.stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &ast::Stmt) {
        match s {
            ast::Stmt::Let(l) => {
                if let Some(t) = &l.ty {
                    self.ty(t);
                }
                self.expr(&l.value);
                if let Some(f) = &l.else_fail {
                    // else fail は Err(...) で早期 return する形に展開される。
                    self.names.insert("Err");
                    self.expr(f);
                }
            }
            ast::Stmt::If(i) => self.if_stmt(i),
            ast::Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    self.expr(v);
                }
            }
            ast::Stmt::Expr(e) => self.expr(&e.expr),
        }
    }

    fn if_stmt(&mut self, i: &ast::IfStmt) {
        self.expr(&i.cond);
        self.block(&i.then_block);
        match &i.else_branch {
            Some(ast::ElseBranch::If(nested)) => self.if_stmt(nested),
            Some(ast::ElseBranch::Block(b)) => self.block(b),
            None => {}
        }
    }

    fn expr(&mut self, e: &ast::Expr) {
        match e {
            ast::Expr::Int { .. } | ast::Expr::Str { .. } | ast::Expr::Bool { .. } => {}
            ast::Expr::Name { .. } => {}
            ast::Expr::Field { base, .. } => self.expr(base),
            ast::Expr::Call { callee, args, .. } => {
                if let ast::Expr::Name { name, .. } = callee.as_ref() {
                    match name.as_str() {
                        "Ok" => {
                            self.names.insert("Ok");
                        }
                        "Err" => {
                            self.names.insert("Err");
                        }
                        "Some" => {
                            self.names.insert("Some");
                        }
                        "None" => {
                            self.names.insert("None");
                        }
                        _ => {}
                    }
                } else {
                    // List.get(i) は範囲外 None を返すランタイムヘルパーへ写す(M9)。emit と
                    // 同じ判定(is_list_get)で、検査器が List.get と確定した位置だけ import する。
                    if is_list_get(callee.as_ref(), args, self.list_ops) {
                        self.names.insert("keiListGet");
                    }
                    self.expr(callee);
                }
                for a in args {
                    self.expr(a);
                }
            }
            ast::Expr::Unary { expr, .. } => self.expr(expr),
            ast::Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            ast::Expr::RecordLit { fields, .. } => {
                for f in fields {
                    if let Some(v) = &f.value {
                        self.expr(v);
                    }
                }
            }
            ast::Expr::Match {
                scrutinee, arms, ..
            } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.expr(&arm.body);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TS 式の優先順位(括弧の最小化)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Or,
    Equality,
    Relational,
    Additive,
    Multiplicative,
    Unary,
    Postfix,
}

fn bin_prec(op: BinOp) -> Prec {
    match op {
        BinOp::Implies => Prec::Or,
        BinOp::Eq | BinOp::Ne => Prec::Equality,
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => Prec::Relational,
        BinOp::Add | BinOp::Sub => Prec::Additive,
        BinOp::Mul | BinOp::Div => Prec::Multiplicative,
    }
}

fn ts_bin_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "===",
        BinOp::Ne => "!==",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Implies => "||", // emit_binary が `!(lhs) || rhs` に展開する
    }
}

// ---------------------------------------------------------------------------
// エミッタ本体
// ---------------------------------------------------------------------------

struct Emitter<'a> {
    src_file: &'a str,
    module: &'a ast::Module,
    /// List コンビネータ呼び出し位置(Call span 開始)。検査器由来の権威的な型情報。
    list_ops: &'a HashSet<(u32, u32)>,
    out: Out,
    /// ensures 内 `old(...)` の通し番号。事前キャプチャと同じ走査順で消費する。
    old_counter: usize,
    in_ensures: bool,
    /// match 式ごとに一意なスクルティニ変数名(`kei$m0`, `kei$m1`, ...)を割り当てる。
    match_counter: usize,
}

impl Emitter<'_> {
    fn emit_module(&mut self) {
        self.out
            .line(&format!("// Generated by kei_emit from {}.", self.src_file));
        self.out
            .line("// Do not edit; regenerate with `kei build`.");

        let runtime = RuntimeUses::collect(self.module, self.list_ops);
        let mut wrote_imports = false;
        if !runtime.names.is_empty() {
            let names: Vec<&str> = runtime.names.iter().copied().collect();
            self.out.newline();
            self.out.line(&format!(
                "import {{ {} }} from \"{RUNTIME_MODULE}\";",
                names.join(", ")
            ));
            wrote_imports = true;
        }
        if !self.module.imports.is_empty() {
            if !wrote_imports {
                self.out.newline();
            }
            for imp in &self.module.imports {
                self.emit_import(imp);
            }
        }

        for item in &self.module.items {
            // extern は外部境界の署名(検査専用)。TS 出力は持たない。
            if matches!(item, ast::Item::Extern(_)) {
                continue;
            }
            self.out.newline();
            match item {
                ast::Item::TypeAlias(a) => self.emit_alias(a),
                ast::Item::Record(r) => self.emit_record(r),
                ast::Item::Enum(e) => self.emit_enum(e),
                ast::Item::Func(f) => self.emit_func(f),
                ast::Item::Extern(_) => unreachable!("extern items are skipped above"),
            }
        }
    }

    /// モジュールパス → 相対 import 指定子。モジュールパスはファイルパスと
    /// 1:1(spec §2.3)なので、自身の階層数だけ `../` で上がってから辿る。
    fn import_specifier(&self, path: &[ast::Ident]) -> String {
        let depth = self
            .module
            .decl
            .as_ref()
            .map(|d| d.path.len().saturating_sub(1))
            .unwrap_or(0);
        let prefix = if depth == 0 {
            "./".to_string()
        } else {
            "../".repeat(depth)
        };
        let parts: Vec<&str> = path.iter().map(|i| i.name.as_str()).collect();
        format!("{prefix}{}", parts.join("/"))
    }

    fn emit_import(&mut self, imp: &ast::Import) {
        let spec = self.import_specifier(&imp.path);
        self.out.start_line();
        self.out.map(imp.span);
        if !imp.names.is_empty() {
            let names: Vec<&str> = imp.names.iter().map(|i| i.name.as_str()).collect();
            self.out.frag(&format!(
                "import {{ {} }} from \"{spec}\";",
                names.join(", ")
            ));
        } else {
            let binding = imp
                .alias
                .as_ref()
                .map(|a| a.name.as_str())
                .or_else(|| imp.path.last().map(|i| i.name.as_str()))
                .unwrap_or("mod");
            self.out
                .frag(&format!("import * as {binding} from \"{spec}\";"));
        }
        self.out.newline();
    }

    // -- 型 -----------------------------------------------------------------

    fn ts_type(&self, t: &ast::Type) -> String {
        if t.path.len() > 1 {
            let parts: Vec<&str> = t.path.iter().map(|i| i.name.as_str()).collect();
            return parts.join(".");
        }
        let name = t.path[0].name.as_str();
        match name {
            "Int" => "number".to_string(),
            "String" => "string".to_string(),
            "Bool" => "boolean".to_string(),
            "Result" => format!(
                "Result<{}, {}>",
                self.ts_type(&t.args[0]),
                self.ts_type(&t.args[1])
            ),
            "Option" => format!("Option<{}>", self.ts_type(&t.args[0])),
            // List<T> → 不変配列 readonly T[](spec v0.3-collections §9)。
            "List" => format!("readonly {}[]", self.ts_type_paren_for_array(&t.args[0])),
            _ if t.args.is_empty() => name.to_string(),
            _ => {
                let args: Vec<String> = t.args.iter().map(|a| self.ts_type(a)).collect();
                format!("{name}<{}>", args.join(", "))
            }
        }
    }

    /// 配列要素型の TS 表記。要素がさらに List(配列)なら括弧で包む
    /// (`readonly (readonly U[])[]`。`readonly readonly U[][]` は不正)。
    fn ts_type_paren_for_array(&self, t: &ast::Type) -> String {
        let inner = self.ts_type(t);
        if t.path.len() == 1 && t.path[0].name == "List" {
            format!("({inner})")
        } else {
            inner
        }
    }

    fn emit_alias(&mut self, a: &ast::TypeAlias) {
        let name = &a.name.name;
        let underlying = self.ts_type(&a.ty);
        match &a.tag {
            Some(tag) => {
                self.out.start_line();
                self.out.map(a.span);
                self.out.frag(&format!(
                    "export type {name} = {underlying} & {{ readonly __keiTag: \"{tag}\" }};"
                ));
                self.out.newline();
                self.out.newline();
                self.out.line(&format!(
                    "export function {name}(value: {underlying}): {name} {{"
                ));
                self.out.indent += 1;
                self.out.line(&format!("return value as {name};"));
                self.out.indent -= 1;
                self.out.line("}");
            }
            None => {
                self.out.start_line();
                self.out.map(a.span);
                self.out
                    .frag(&format!("export type {name} = {underlying};"));
                self.out.newline();
            }
        }
    }

    fn emit_record(&mut self, r: &ast::RecordDecl) {
        let name = &r.name.name;
        self.out.start_line();
        self.out.map(r.span);
        self.out.frag(&format!("export type {name} = {{"));
        self.out.newline();
        self.out.indent += 1;
        for f in &r.fields {
            self.out.start_line();
            self.out.map(f.span);
            self.out.frag(&format!(
                "readonly {}: {};",
                f.name.name,
                self.ts_type(&f.ty)
            ));
            self.out.newline();
        }
        self.out.indent -= 1;
        self.out.line("};");
        self.out.newline();
        self.out.line(&format!(
            "export function {name}(fields: {name}): {name} {{"
        ));
        self.out.indent += 1;
        self.out.line("return fields;");
        self.out.indent -= 1;
        self.out.line("}");
    }

    fn variant_record_fields_ty(&self, fields: &[ast::FieldDef]) -> String {
        let parts: Vec<String> = fields
            .iter()
            .map(|f| format!("readonly {}: {}", f.name.name, self.ts_type(&f.ty)))
            .collect();
        format!("{{ {} }}", parts.join("; "))
    }

    fn emit_enum(&mut self, e: &ast::EnumDecl) {
        let name = &e.name.name;
        self.out.start_line();
        self.out.map(e.span);
        self.out.frag(&format!("export type {name} ="));
        self.out.newline();
        self.out.indent += 1;
        for v in &e.variants {
            let kind = &v.name.name;
            let member = match &v.payload {
                ast::VariantPayload::Unit => format!("| {{ readonly kind: \"{kind}\" }}"),
                ast::VariantPayload::Tuple { types } => {
                    let tys: Vec<String> = types.iter().map(|t| self.ts_type(t)).collect();
                    format!(
                        "| {{ readonly kind: \"{kind}\"; readonly values: readonly [{}] }}",
                        tys.join(", ")
                    )
                }
                ast::VariantPayload::Record { fields } => format!(
                    "| {{ readonly kind: \"{kind}\"; readonly fields: {} }}",
                    self.variant_record_fields_ty(fields)
                ),
            };
            self.out.start_line();
            self.out.map(v.span);
            self.out.frag(&member);
            self.out.newline();
        }
        // 最終バリアント行の末尾にセミコロンを置くと正規形が崩れるため独立行。
        self.out.indent -= 1;
        // union 末尾の `;` は最後のメンバー行に含める形に整える。
        trim_trailing_newline_and_terminate(&mut self.out);

        self.out.newline();
        self.out.line(&format!("export const {name} = {{"));
        self.out.indent += 1;
        for v in &e.variants {
            let kind = &v.name.name;
            match &v.payload {
                ast::VariantPayload::Unit => {
                    self.out
                        .line(&format!("{kind}: {{ kind: \"{kind}\" }} as {name},"));
                }
                ast::VariantPayload::Tuple { types } => {
                    let params: Vec<String> = types
                        .iter()
                        .enumerate()
                        .map(|(i, t)| format!("v{i}: {}", self.ts_type(t)))
                        .collect();
                    let args: Vec<String> = (0..types.len()).map(|i| format!("v{i}")).collect();
                    self.out.line(&format!(
                        "{kind}: ({}): {name} => ({{ kind: \"{kind}\", values: [{}] }}),",
                        params.join(", "),
                        args.join(", ")
                    ));
                }
                ast::VariantPayload::Record { fields } => {
                    self.out.line(&format!(
                        "{kind}: (fields: {}): {name} => ({{ kind: \"{kind}\", fields }}),",
                        self.variant_record_fields_ty(fields)
                    ));
                }
            }
        }
        self.out.indent -= 1;
        self.out.line("};");
    }

    // -- 関数 ---------------------------------------------------------------

    fn emit_func(&mut self, f: &ast::FuncDecl) {
        self.emit_func_doc(f);

        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name.name, self.ts_type(&p.ty)))
            .collect();
        let ret = f
            .ret
            .as_ref()
            .map(|t| self.ts_type(t))
            .unwrap_or_else(|| "void".to_string());

        self.out.start_line();
        self.out.map(f.span);
        self.out.frag(&format!(
            "export function {}({}): {ret} {{",
            f.name.name,
            params.join(", ")
        ));
        self.out.newline();
        self.out.indent += 1;

        for clause in &f.requires {
            self.emit_contract_check(clause, "requires", &f.name.name, None);
        }

        let old_exprs = collect_old_exprs(&f.ensures);
        for (i, expr) in old_exprs.iter().enumerate() {
            self.out.start_line();
            self.out.map(expr.span());
            self.out.frag(&format!("const kei$old${i} = "));
            self.emit_expr(expr, Prec::Or);
            self.out.frag(";");
            self.out.newline();
        }

        if f.ensures.is_empty() {
            self.emit_block_stmts(&f.body);
        } else {
            self.out
                .line(&format!("const kei$result = ((): {ret} => {{"));
            self.out.indent += 1;
            self.emit_block_stmts(&f.body);
            self.out.indent -= 1;
            self.out.line("})();");
            self.old_counter = 0;
            self.in_ensures = true;
            for clause in &f.ensures {
                self.emit_contract_check(clause, "ensures", &f.name.name, Some("kei$result"));
            }
            self.in_ensures = false;
            self.out.line("return kei$result;");
        }

        self.out.indent -= 1;
        self.out.line("}");
    }

    /// 契約を doc コメントとして残す(spec §5: uses は TS 出力にコメントで残す)。
    fn emit_func_doc(&mut self, f: &ast::FuncDecl) {
        if f.uses.is_empty() && f.requires.is_empty() && f.ensures.is_empty() {
            return;
        }
        self.out.line("/**");
        if !f.uses.is_empty() {
            let effects: Vec<String> = f
                .uses
                .iter()
                .map(|u| {
                    u.path
                        .iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join(".")
                })
                .collect();
            self.out.line(&format!(" * uses {}", effects.join(", ")));
        }
        for c in &f.requires {
            self.out.line(&format!(" * requires {}", kei_expr_text(c)));
        }
        for c in &f.ensures {
            self.out.line(&format!(" * ensures {}", kei_expr_text(c)));
        }
        self.out.line(" */");
    }

    fn emit_contract_check(
        &mut self,
        clause: &ast::Expr,
        kind: &str,
        func: &str,
        _result_var: Option<&str>,
    ) {
        let span = clause.span();
        self.out.start_line();
        self.out.map(span);
        self.out.frag("if (!(");
        let was_ensures = self.in_ensures;
        self.emit_expr(clause, Prec::Or);
        self.in_ensures = was_ensures;
        self.out.frag(")) {");
        self.out.newline();
        self.out.indent += 1;
        self.out.start_line();
        self.out.map(span);
        self.out.frag("throw new KeiContractViolation({");
        self.out.newline();
        self.out.indent += 1;
        self.out.line(&format!("clause: \"{kind}\","));
        self.out.line(&format!("func: \"{func}\","));
        self.out.line(&format!(
            "condition: {},",
            ts_string(&kei_expr_text(clause))
        ));
        self.out
            .line(&format!("file: {},", ts_string(self.src_file)));
        self.out.line(&format!("line: {},", span.start.line));
        self.out.line(&format!("col: {},", span.start.col));
        self.out.indent -= 1;
        self.out.line("});");
        self.out.indent -= 1;
        self.out.line("}");
    }

    // -- 文 -----------------------------------------------------------------

    fn emit_block_stmts(&mut self, b: &ast::Block) {
        for s in &b.stmts {
            self.emit_stmt(s);
        }
    }

    fn emit_stmt(&mut self, s: &ast::Stmt) {
        match s {
            ast::Stmt::Let(l) => self.emit_let(l),
            ast::Stmt::If(i) => self.emit_if(i),
            ast::Stmt::Return(r) => {
                self.out.start_line();
                self.out.map(r.span);
                match &r.value {
                    Some(v) => {
                        self.out.frag("return ");
                        self.emit_expr(v, Prec::Or);
                        self.out.frag(";");
                    }
                    None => self.out.frag("return;"),
                }
                self.out.newline();
            }
            ast::Stmt::Expr(e) => {
                self.out.start_line();
                self.out.map(e.span);
                self.emit_expr(&e.expr, Prec::Or);
                self.out.frag(";");
                self.out.newline();
            }
        }
    }

    fn emit_let(&mut self, l: &ast::LetStmt) {
        let name = &l.name.name;
        let ann = l.ty.as_ref().map(|t| format!(": {}", self.ts_type(t)));
        match &l.else_fail {
            None => {
                self.out.start_line();
                self.out.map(l.span);
                self.out
                    .frag(&format!("const {name}{} = ", ann.as_deref().unwrap_or("")));
                self.emit_expr(&l.value, Prec::Or);
                self.out.frag(";");
                self.out.newline();
            }
            Some(fail) => {
                // Option / Result 共有の判別子 `ok` で分岐し、失敗値で早期 return。
                self.out.start_line();
                self.out.map(l.span);
                self.out.frag(&format!("const {name}$ = "));
                self.emit_expr(&l.value, Prec::Or);
                self.out.frag(";");
                self.out.newline();
                self.out.line(&format!("if (!{name}$.ok) {{"));
                self.out.indent += 1;
                self.out.start_line();
                self.out.map(fail.span());
                self.out.frag("return Err(");
                self.emit_expr(fail, Prec::Or);
                self.out.frag(");");
                self.out.newline();
                self.out.indent -= 1;
                self.out.line("}");
                self.out.line(&format!(
                    "const {name}{} = {name}$.value;",
                    ann.as_deref().unwrap_or("")
                ));
            }
        }
    }

    fn emit_if(&mut self, i: &ast::IfStmt) {
        self.out.start_line();
        self.emit_if_from_line_start(i);
    }

    fn emit_if_from_line_start(&mut self, i: &ast::IfStmt) {
        self.out.map(i.span);
        self.out.frag("if (");
        self.emit_expr(&i.cond, Prec::Or);
        self.out.frag(") {");
        self.out.newline();
        self.out.indent += 1;
        self.emit_block_stmts(&i.then_block);
        self.out.indent -= 1;
        match &i.else_branch {
            None => self.out.line("}"),
            Some(ast::ElseBranch::If(nested)) => {
                self.out.start_line();
                self.out.frag("} else ");
                self.emit_if_from_line_start(nested);
            }
            Some(ast::ElseBranch::Block(b)) => {
                self.out.line("} else {");
                self.out.indent += 1;
                self.emit_block_stmts(b);
                self.out.indent -= 1;
                self.out.line("}");
            }
        }
    }

    // -- 式 -----------------------------------------------------------------

    fn emit_expr(&mut self, e: &ast::Expr, parent: Prec) {
        match e {
            ast::Expr::Int { value, .. } => self.out.frag(&value.to_string()),
            ast::Expr::Str { value, .. } => self.out.frag(&ts_string(value)),
            ast::Expr::Bool { value, .. } => self.out.frag(if *value { "true" } else { "false" }),
            ast::Expr::Name { name, .. } => {
                // ensures 内の `result` は事後条件検査用の戻り値変数を指す。
                if self.in_ensures && name == "result" {
                    self.out.frag("kei$result");
                } else {
                    self.out.frag(name);
                }
            }
            ast::Expr::Field { base, name, .. } => {
                // フィールドアクセスはそのまま写す。List.isEmpty はメソッド形 `xs.isEmpty()`
                // なので emit_call 側で `.length === 0` に写る(ここに来るのは純粋なフィールド
                // アクセスで、レコードの `isEmpty` フィールドも安全にそのまま出る)。
                self.emit_expr(base, Prec::Postfix);
                self.out.frag(&format!(".{}", name.name));
            }
            ast::Expr::Call { callee, args, .. } => self.emit_call(callee, args),
            ast::Expr::Unary { op, expr, .. } => {
                let needs_paren = parent > Prec::Unary;
                if needs_paren {
                    self.out.frag("(");
                }
                self.out.frag(match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                });
                self.emit_expr(expr, Prec::Unary);
                if needs_paren {
                    self.out.frag(")");
                }
            }
            ast::Expr::Binary { op, lhs, rhs, .. } => self.emit_binary(*op, lhs, rhs, parent),
            ast::Expr::RecordLit { path, fields, .. } => {
                let parts: Vec<&str> = path.iter().map(|i| i.name.as_str()).collect();
                self.out.frag(&format!("{}({{ ", parts.join(".")));
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        self.out.frag(", ");
                    }
                    match &f.value {
                        None => self.out.frag(&f.name.name),
                        Some(v) => {
                            self.out.frag(&format!("{}: ", f.name.name));
                            self.emit_expr(v, Prec::Or);
                        }
                    }
                }
                self.out.frag(" })");
            }
            ast::Expr::Match {
                scrutinee, arms, ..
            } => self.emit_match(scrutinee, arms),
        }
    }

    /// `match` を即時実行アロー関数(IIFE)に展開する。各腕は判別子で分岐する
    /// `if` ガードに落ち、束縛は腕の冒頭で `const` する。網羅性はチェッカが
    /// 保証するため、末尾の `throw` は到達不能(opaque な import 値の防御)。
    fn emit_match(&mut self, scrutinee: &ast::Expr, arms: &[ast::MatchArm]) {
        let id = self.match_counter;
        self.match_counter += 1;
        let var = format!("kei$m{id}");
        self.out.frag("(() => {");
        self.out.newline();
        self.out.indent += 1;
        self.out.start_line();
        self.out.map(scrutinee.span());
        self.out.frag(&format!("const {var} = "));
        self.emit_expr(scrutinee, Prec::Or);
        self.out.frag(";");
        self.out.newline();
        for arm in arms {
            self.emit_match_arm(&var, arm);
        }
        self.out.line("throw new Error(\"non-exhaustive match\");");
        self.out.indent -= 1;
        self.out.start_line();
        self.out.frag("})()");
    }

    fn emit_match_arm(&mut self, var: &str, arm: &ast::MatchArm) {
        let ctor: Vec<&str> = arm.pattern.path.iter().map(|i| i.name.as_str()).collect();
        let cond = match ctor.as_slice() {
            ["Some"] | ["Ok"] => format!("{var}.ok"),
            ["None"] | ["Err"] => format!("!{var}.ok"),
            _ => {
                let v = ctor.last().copied().unwrap_or("");
                format!("{var}.kind === {}", ts_string(v))
            }
        };
        self.out.start_line();
        self.out.map(arm.pattern.span);
        self.out.frag(&format!("if ({cond}) {{"));
        self.out.newline();
        self.out.indent += 1;
        self.emit_pattern_bindings(var, &arm.pattern);
        self.out.start_line();
        self.out.map(arm.body.span());
        self.out.frag("return ");
        self.emit_expr(&arm.body, Prec::Or);
        self.out.frag(";");
        self.out.newline();
        self.out.indent -= 1;
        self.out.line("}");
    }

    fn emit_pattern_bindings(&mut self, var: &str, pat: &ast::Pattern) {
        let ctor: Vec<&str> = pat.path.iter().map(|i| i.name.as_str()).collect();
        match (&ctor[..], &pat.payload) {
            (["Some"] | ["Ok"], ast::PatternPayload::Tuple { bindings }) => {
                if let Some(b) = bindings.first() {
                    self.out.line(&format!("const {} = {var}.value;", b.name));
                }
            }
            (["Err"], ast::PatternPayload::Tuple { bindings }) => {
                if let Some(b) = bindings.first() {
                    self.out.line(&format!("const {} = {var}.error;", b.name));
                }
            }
            (_, ast::PatternPayload::Tuple { bindings }) => {
                for (i, b) in bindings.iter().enumerate() {
                    self.out
                        .line(&format!("const {} = {var}.values[{i}];", b.name));
                }
            }
            (_, ast::PatternPayload::Record { fields }) => {
                for f in fields {
                    self.out
                        .line(&format!("const {} = {var}.fields.{};", f.name, f.name));
                }
            }
            (_, ast::PatternPayload::Unit) => {}
        }
    }

    fn emit_call(&mut self, callee: &ast::Expr, args: &[ast::Expr]) {
        if let ast::Expr::Name { name, .. } = callee {
            if name == "old" {
                // 事前キャプチャ済みの値を参照する(収集と同じ走査順)。
                let i = self.old_counter;
                self.old_counter += 1;
                self.out.frag(&format!("kei$old${i}"));
                return;
            }
        }
        // List コンビネータのメソッド呼び出し(M9 / spec v0.3-collections §9)。
        // **この呼び出し位置を検査器が List 操作と確定している**ときだけ配列メソッドへ写す
        // (構文ヒューリスティックでレシーバ型を推測しない。外部呼び出しの連鎖
        // `Database.reader().get(id)` や let 束縛 opaque 値の誤写を防ぐ)。
        // 鍵はメソッド名トークン(`name`)の位置。Call の span は連鎖だとレシーバ先頭で
        // 揃ってしまい(`db.get(id).get(0)` の内外で同じ)衝突するため使わない。
        //
        // get -> keiListGet は import 収集(RuntimeUses)と同じ is_list_get で判定を共有する
        // (両者がずれて「呼び出しはあるが import 無し」になる事故を防ぐ)。
        if is_list_get(callee, args, self.list_ops) {
            if let ast::Expr::Field { base, .. } = callee {
                self.out.frag("keiListGet(");
                self.emit_expr(base, Prec::Or);
                self.out.frag(", ");
                self.emit_expr(&args[0], Prec::Or);
                self.out.frag(")");
                return;
            }
        }
        if let ast::Expr::Field { base, name, .. } = callee {
            let is_list_op = self
                .list_ops
                .contains(&(name.span.start.line, name.span.start.col));
            if is_list_op {
                match name.name.as_str() {
                    // isEmpty() → `(xs.length === 0)`。
                    "isEmpty" if args.is_empty() => {
                        self.out.frag("(");
                        self.emit_expr(base, Prec::Postfix);
                        self.out.frag(".length === 0)");
                        return;
                    }
                    // fold(init, f) → reduce(f, init)(引数順が逆)。
                    "fold" if args.len() == 2 => {
                        self.emit_expr(base, Prec::Postfix);
                        self.out.frag(".reduce(");
                        self.emit_expr(&args[1], Prec::Or);
                        self.out.frag(", ");
                        self.emit_expr(&args[0], Prec::Or);
                        self.out.frag(")");
                        return;
                    }
                    // all/any → every/some。map/filter は同名の配列メソッドへ素通り。
                    "all" | "any" => {
                        let js = if name.name == "all" { "every" } else { "some" };
                        self.emit_expr(base, Prec::Postfix);
                        self.out.frag(&format!(".{js}("));
                        for (i, a) in args.iter().enumerate() {
                            if i > 0 {
                                self.out.frag(", ");
                            }
                            self.emit_expr(a, Prec::Or);
                        }
                        self.out.frag(")");
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.emit_expr(callee, Prec::Postfix);
        self.out.frag("(");
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                self.out.frag(", ");
            }
            self.emit_expr(a, Prec::Or);
        }
        self.out.frag(")");
    }

    fn emit_binary(&mut self, op: BinOp, lhs: &ast::Expr, rhs: &ast::Expr, parent: Prec) {
        let prec = bin_prec(op);
        let needs_paren = parent > prec;
        if needs_paren {
            self.out.frag("(");
        }
        if op == BinOp::Implies {
            // `a implies b` → `!(a) || b`(∨ の結合律により入れ子も意味を保つ)。
            self.out.frag("!(");
            self.emit_expr(lhs, Prec::Or);
            self.out.frag(") || ");
            self.emit_expr(rhs, Prec::Or);
        } else if op == BinOp::Div {
            // Int 除算は 0 方向への切り捨て。
            self.out.frag("Math.trunc(");
            self.emit_expr(lhs, Prec::Multiplicative);
            self.out.frag(" / ");
            self.emit_expr(rhs, Prec::Unary);
            self.out.frag(")");
        } else {
            self.emit_expr(lhs, prec);
            self.out.frag(&format!(" {} ", ts_bin_op(op)));
            // 左結合演算子の右辺は 1 段高い優先度で出す。同優先度が右にネストしたとき
            // (`a == (b == c)`)に括弧を保つため(括弧を落とすと JS の左結合で
            // `(a == b) == c` に化けて結果が変わる)。Equality / Relational も対象。
            let rhs_min = match prec {
                Prec::Equality => Prec::Relational,
                Prec::Relational => Prec::Additive,
                Prec::Additive => Prec::Multiplicative,
                Prec::Multiplicative => Prec::Unary,
                p => p,
            };
            self.emit_expr(rhs, rhs_min);
        }
        if needs_paren {
            self.out.frag(")");
        }
    }
}

/// enum union の最終メンバー行末に `;` を付ける(直前の改行を巻き戻して付加)。
fn trim_trailing_newline_and_terminate(out: &mut Out) {
    if out.buf.ends_with('\n') {
        out.buf.pop();
        out.line -= 1;
        out.buf.push(';');
        out.buf.push('\n');
        out.line += 1;
        out.col = 0;
    }
}

/// ensures 節から `old(...)` の引数式を走査順に集める。
fn collect_old_exprs(ensures: &[ast::Expr]) -> Vec<&ast::Expr> {
    fn walk<'a>(e: &'a ast::Expr, out: &mut Vec<&'a ast::Expr>) {
        match e {
            ast::Expr::Call { callee, args, .. } => {
                if let ast::Expr::Name { name, .. } = callee.as_ref() {
                    if name == "old" {
                        if let Some(arg) = args.first() {
                            out.push(arg);
                        }
                        return;
                    }
                }
                walk(callee, out);
                args.iter().for_each(|a| walk(a, out));
            }
            ast::Expr::Field { base, .. } => walk(base, out),
            ast::Expr::Unary { expr, .. } => walk(expr, out),
            ast::Expr::Binary { lhs, rhs, .. } => {
                walk(lhs, out);
                walk(rhs, out);
            }
            ast::Expr::RecordLit { fields, .. } => {
                for f in fields {
                    if let Some(v) = &f.value {
                        walk(v, out);
                    }
                }
            }
            ast::Expr::Match {
                scrutinee, arms, ..
            } => {
                walk(scrutinee, out);
                for arm in arms {
                    walk(&arm.body, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in ensures {
        walk(c, &mut out);
    }
    out
}

/// TS 文字列リテラル(JSON エスケープは TS と互換)。
fn ts_string(s: &str) -> String {
    serde_json::to_string(s).expect("strings are serializable")
}

/// `callee(args)` が「検査器が確定した List.get 呼び出し」か。`get` だけは `keiListGet`
/// ランタイムヘルパーへ写るため、import 収集(`RuntimeUses`)と emit の両方が**同じ判定**を
/// 必要とする。二つがずれると「呼び出しはあるが import が無い」TS になるので 1 か所に集約する。
/// 鍵はメソッド名トークンの位置(Call span は連鎖だと衝突する)。
fn is_list_get(callee: &ast::Expr, args: &[ast::Expr], list_ops: &HashSet<(u32, u32)>) -> bool {
    matches!(
        callee,
        ast::Expr::Field { name, .. }
            if name.name == "get"
                && args.len() == 1
                && list_ops.contains(&(name.span.start.line, name.span.start.col))
    )
}

// ---------------------------------------------------------------------------
// 契約式の Kei ソース表記(condition フィールドと doc コメント用)
// ---------------------------------------------------------------------------
//
// 表記の正規実装は `kei_check::contract_expr_text`(#32、ファイル冒頭で `kei_expr_text` として
// import)。`condition` は `CheckReport.contracts[].expr` とバイト一致が要件なので、優先順位・
// 結合方向・文字列エスケープをここで再実装せず、検査クレートの単一実装へ委譲する。
