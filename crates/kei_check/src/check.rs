//! 意味検査本体(M3): 名前解決(E1xxx)・型検査(E2xxx)・エフェクト検査(E3xxx)・
//! 契約純粋性検査(E4xxx)。
//!
//! v0.1 は単一ファイル検査。import された名前の中身(型・エフェクト)は解決
//! できないため [`Ty::Unknown`] として信頼境界の外に置く。エフェクトの発生源は
//! ローカル関数の `uses` 宣言のみで、ローカル呼び出しを通じて呼び出し元へ
//! 推移的に伝播する(spec §3.1)。契約式(requires / ensures)の中では
//! エフェクトを持つ関数の呼び出しを禁止する(spec §4: 契約式は副作用禁止)。

use std::collections::{HashMap, HashSet};

use kei_syntax::ast;
use kei_syntax::{Position as SynPosition, Span as SynSpan};

use crate::effects;
use crate::types::Ty;
use crate::{Diagnostic, Fix, Position, Severity, Span, TextEdit};

mod codes {
    pub const UNDEFINED_NAME: &str = "KEI-E1001";
    pub const UNDEFINED_TYPE: &str = "KEI-E1002";
    pub const DUPLICATE_DEF: &str = "KEI-E1003";
    pub const IMPORT_CONFLICT: &str = "KEI-E1004";
    pub const TYPE_MISMATCH: &str = "KEI-E2001";
    pub const UNKNOWN_FIELD: &str = "KEI-E2002";
    pub const UNKNOWN_VARIANT: &str = "KEI-E2003";
    pub const RECORD_LITERAL: &str = "KEI-E2004";
    pub const TAGGED_CONFUSION: &str = "KEI-E2005";
    pub const TYPE_ARITY: &str = "KEI-E2006";
    pub const EFFECT_UNDECLARED: &str = "KEI-E3001";
    pub const UNKNOWN_EFFECT: &str = "KEI-E3002";
    pub const IMPURE_CONTRACT: &str = "KEI-E4001";
    pub const CONTRACT_CONSTRUCT: &str = "KEI-E4002";
}

/// 1 モジュールを検査し、出現位置に対応した順序で Diagnostic を返す。
/// `file` はリポジトリルートからの相対パス(span の `file` フィールドに入る)。
pub fn check_module(file: &str, module: &ast::Module) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let (env, fn_sigs) = Env::build(file, module, &mut diags);
    for (item, sig) in module.items.iter().zip(&fn_sigs) {
        if let (ast::Item::Func(f), Some(sig)) = (item, sig) {
            FnChecker {
                env: &env,
                diags: &mut diags,
                func: f,
                sig,
                mode: Mode::Body,
                scopes: Vec::new(),
            }
            .check();
        }
    }
    // パーサのエラー整列(parse_module)と同じ規約: ソース上の出現位置順。
    diags.sort_by(|a, b| {
        (a.span.start.line, a.span.start.col, a.code.as_str()).cmp(&(
            b.span.start.line,
            b.span.start.col,
            b.code.as_str(),
        ))
    });
    diags
}

// ---------------------------------------------------------------------------
// モジュール環境(名前解決の対象表)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameKind {
    Record,
    Enum,
    Alias,
    Func,
    Import,
}

#[derive(Debug, Clone)]
enum VariantDef {
    Unit,
    Tuple(Vec<Ty>),
    Record(Vec<(String, Ty)>),
}

#[derive(Debug, Clone)]
struct FuncSig {
    params: Vec<(String, Ty)>,
    ret: Ty,
    /// 検証済み(標準階層に存在する)エフェクトのみ。
    effects: Vec<String>,
    /// `uses` 節末尾の位置。「`, X` を追記する」fix の挿入位置に使う。
    uses_end: Option<SynPosition>,
}

struct Env {
    file: String,
    kinds: HashMap<String, NameKind>,
    records: HashMap<String, Vec<(String, Ty)>>,
    enums: HashMap<String, Vec<(String, VariantDef)>>,
    aliases: HashMap<String, Ty>,
    funcs: HashMap<String, FuncSig>,
}

impl Env {
    fn build(
        file: &str,
        module: &ast::Module,
        diags: &mut Vec<Diagnostic>,
    ) -> (Env, Vec<Option<FuncSig>>) {
        let mut env = Env {
            file: file.to_string(),
            kinds: HashMap::new(),
            records: HashMap::new(),
            enums: HashMap::new(),
            aliases: HashMap::new(),
            funcs: HashMap::new(),
        };
        // 名前 → 最初の定義位置(重複メッセージと「最初の定義のみ有効」の判定)。
        let mut first: HashMap<String, SynSpan> = HashMap::new();

        // 1) import 名の登録
        for imp in &module.imports {
            let mut names: Vec<&ast::Ident> = imp.names.iter().collect();
            if let Some(alias) = &imp.alias {
                names.push(alias);
            }
            if names.is_empty() {
                if let Some(last) = imp.path.last() {
                    names.push(last);
                }
            }
            for ident in names {
                if let Some(prev) = first.get(&ident.name) {
                    env.push(
                        diags,
                        codes::IMPORT_CONFLICT,
                        format!(
                            "name '{}' is imported more than once (first introduced at line {})",
                            ident.name, prev.start.line
                        ),
                        ident.span,
                        vec![direction("Remove or alias the duplicate import")],
                    );
                } else {
                    env.kinds.insert(ident.name.clone(), NameKind::Import);
                    first.insert(ident.name.clone(), ident.span);
                }
            }
        }

        // 2) item 名の登録(最初の定義が有効。以降は重複エラー)
        for item in &module.items {
            let (ident, kind) = match item {
                ast::Item::TypeAlias(a) => (&a.name, NameKind::Alias),
                ast::Item::Record(r) => (&r.name, NameKind::Record),
                ast::Item::Enum(e) => (&e.name, NameKind::Enum),
                ast::Item::Func(f) => (&f.name, NameKind::Func),
            };
            match (env.kinds.get(&ident.name), first.get(&ident.name)) {
                (Some(NameKind::Import), Some(_)) => {
                    env.push(
                        diags,
                        codes::IMPORT_CONFLICT,
                        format!("'{}' conflicts with an imported name", ident.name),
                        ident.span,
                        vec![direction(format!(
                            "Rename '{}' or remove the import",
                            ident.name
                        ))],
                    );
                }
                (Some(_), Some(prev)) => {
                    let prev_line = prev.start.line;
                    env.push(
                        diags,
                        codes::DUPLICATE_DEF,
                        format!(
                            "duplicate definition of '{}' (first defined at line {prev_line})",
                            ident.name
                        ),
                        ident.span,
                        vec![direction("Rename one of the definitions")],
                    );
                }
                _ => {
                    env.kinds.insert(ident.name.clone(), kind);
                    first.insert(ident.name.clone(), ident.span);
                }
            }
        }

        let is_primary = |first: &HashMap<String, SynSpan>, ident: &ast::Ident| {
            first.get(&ident.name) == Some(&ident.span)
        };

        // 3) 型エイリアスの解決(循環ガード付き)
        let raw_aliases: HashMap<&str, &ast::TypeAlias> = module
            .items
            .iter()
            .filter_map(|item| match item {
                ast::Item::TypeAlias(a) if is_primary(&first, &a.name) => {
                    Some((a.name.name.as_str(), a))
                }
                _ => None,
            })
            .collect();
        let alias_names: Vec<&str> = module
            .items
            .iter()
            .filter_map(|item| match item {
                ast::Item::TypeAlias(a) if is_primary(&first, &a.name) => {
                    Some(a.name.name.as_str())
                }
                _ => None,
            })
            .collect();
        let mut visiting = Vec::new();
        for name in alias_names {
            ensure_alias(name, &raw_aliases, &mut env, &mut visiting, diags);
        }

        // 4) record / enum 定義の構築(フィールド・バリアントの重複検査込み)
        for item in &module.items {
            match item {
                ast::Item::Record(r) if is_primary(&first, &r.name) => {
                    let mut fields = Vec::new();
                    for f in &r.fields {
                        if fields.iter().any(|(n, _)| n == &f.name.name) {
                            env.push(
                                diags,
                                codes::DUPLICATE_DEF,
                                format!(
                                    "duplicate field '{}' in record '{}'",
                                    f.name.name, r.name.name
                                ),
                                f.name.span,
                                vec![direction("Rename or remove the duplicate field")],
                            );
                            continue;
                        }
                        let ty = env.resolve_ty(&f.ty, diags);
                        fields.push((f.name.name.clone(), ty));
                    }
                    env.records.insert(r.name.name.clone(), fields);
                }
                ast::Item::Enum(e) if is_primary(&first, &e.name) => {
                    let mut variants: Vec<(String, VariantDef)> = Vec::new();
                    for v in &e.variants {
                        if variants.iter().any(|(n, _)| n == &v.name.name) {
                            env.push(
                                diags,
                                codes::DUPLICATE_DEF,
                                format!(
                                    "duplicate variant '{}' in enum '{}'",
                                    v.name.name, e.name.name
                                ),
                                v.name.span,
                                vec![direction("Rename or remove the duplicate variant")],
                            );
                            continue;
                        }
                        let def = match &v.payload {
                            ast::VariantPayload::Unit => VariantDef::Unit,
                            ast::VariantPayload::Tuple { types } => VariantDef::Tuple(
                                types.iter().map(|t| env.resolve_ty(t, diags)).collect(),
                            ),
                            ast::VariantPayload::Record { fields } => {
                                let mut fs = Vec::new();
                                for f in fields {
                                    if fs.iter().any(|(n, _): &(String, Ty)| n == &f.name.name) {
                                        env.push(
                                            diags,
                                            codes::DUPLICATE_DEF,
                                            format!(
                                                "duplicate field '{}' in variant '{}'",
                                                f.name.name, v.name.name
                                            ),
                                            f.name.span,
                                            vec![direction("Rename or remove the duplicate field")],
                                        );
                                        continue;
                                    }
                                    let ty = env.resolve_ty(&f.ty, diags);
                                    fs.push((f.name.name.clone(), ty));
                                }
                                VariantDef::Record(fs)
                            }
                        };
                        variants.push((v.name.name.clone(), def));
                    }
                    env.enums.insert(e.name.name.clone(), variants);
                }
                _ => {}
            }
        }

        // 5) 関数シグネチャの構築。重複定義でも自分の body 検査用 sig は作るが、
        //    呼び出し解決(env.funcs)には最初の定義のみ登録する。
        let mut fn_sigs: Vec<Option<FuncSig>> = Vec::with_capacity(module.items.len());
        for item in &module.items {
            let ast::Item::Func(f) = item else {
                fn_sigs.push(None);
                continue;
            };
            let mut params: Vec<(String, Ty)> = Vec::new();
            for p in &f.params {
                if params.iter().any(|(n, _)| n == &p.name.name) {
                    env.push(
                        diags,
                        codes::DUPLICATE_DEF,
                        format!("duplicate parameter '{}' in '{}'", p.name.name, f.name.name),
                        p.name.span,
                        vec![direction("Rename or remove the duplicate parameter")],
                    );
                    continue;
                }
                let ty = env.resolve_ty(&p.ty, diags);
                params.push((p.name.name.clone(), ty));
            }
            let ret = f
                .ret
                .as_ref()
                .map(|t| env.resolve_ty(t, diags))
                .unwrap_or(Ty::Unit);
            let mut declared = Vec::new();
            for u in &f.uses {
                let path: Vec<&str> = u.path.iter().map(|i| i.name.as_str()).collect();
                let path = path.join(".");
                if effects::is_known(&path) {
                    if !declared.contains(&path) {
                        declared.push(path);
                    }
                } else {
                    let fix = match suggestion(&path, effects::STANDARD_EFFECTS.iter().copied()) {
                        Some(s) => env.replace_fix(format!("Did you mean '{s}'?"), u.span, &s),
                        None => direction(
                            "Use a standard effect (IO, Network.*, File.*, Database.*, Clock, Random, Audit.Log)",
                        ),
                    };
                    env.push(
                        diags,
                        codes::UNKNOWN_EFFECT,
                        format!("unknown effect '{path}'"),
                        u.span,
                        vec![fix],
                    );
                }
            }
            let sig = FuncSig {
                params,
                ret,
                effects: declared,
                uses_end: f.uses.last().map(|u| u.span.end),
            };
            if is_primary(&first, &f.name) {
                env.funcs.insert(f.name.name.clone(), sig.clone());
            }
            fn_sigs.push(Some(sig));
        }

        (env, fn_sigs)
    }

    fn span(&self, s: SynSpan) -> Span {
        Span {
            file: self.file.clone(),
            start: Position {
                line: s.start.line,
                col: s.start.col,
            },
            end: Position {
                line: s.end.line,
                col: s.end.col,
            },
        }
    }

    fn push(
        &self,
        diags: &mut Vec<Diagnostic>,
        code: &str,
        message: String,
        span: SynSpan,
        fixes: Vec<Fix>,
    ) {
        diags.push(
            Diagnostic::new(Severity::Error, code, message, self.span(span), fixes)
                .expect("checker diagnostics always carry at least one fix"),
        );
    }

    fn replace_fix(&self, title: String, span: SynSpan, new_text: &str) -> Fix {
        Fix {
            title,
            edits: vec![TextEdit {
                span: self.span(span),
                new_text: new_text.to_string(),
            }],
        }
    }

    /// AST 型参照 → [`Ty`]。組み込み型(Int / String / Bool / Result / Option)が
    /// 最優先で、次にローカル定義・import 名を引く。
    fn resolve_ty(&self, t: &ast::Type, diags: &mut Vec<Diagnostic>) -> Ty {
        let root = &t.path[0].name;
        if t.path.len() > 1 {
            if self.kinds.get(root) == Some(&NameKind::Import) {
                for a in &t.args {
                    self.resolve_ty(a, diags);
                }
                return Ty::Unknown;
            }
            let full: Vec<&str> = t.path.iter().map(|i| i.name.as_str()).collect();
            let full = full.join(".");
            self.push(
                diags,
                codes::UNDEFINED_TYPE,
                format!("undefined type '{full}'"),
                t.span,
                vec![direction(format!("Define or import '{full}'"))],
            );
            return Ty::Unknown;
        }
        match root.as_str() {
            "Int" => self.simple_builtin(t, Ty::Int, diags),
            "String" => self.simple_builtin(t, Ty::Str, diags),
            "Bool" => self.simple_builtin(t, Ty::Bool, diags),
            "Result" => {
                if !self.check_type_args(t, 2, diags) {
                    return Ty::Unknown;
                }
                Ty::Result(
                    Box::new(self.resolve_ty(&t.args[0], diags)),
                    Box::new(self.resolve_ty(&t.args[1], diags)),
                )
            }
            "Option" => {
                if !self.check_type_args(t, 1, diags) {
                    return Ty::Unknown;
                }
                Ty::Option(Box::new(self.resolve_ty(&t.args[0], diags)))
            }
            _ => match self.kinds.get(root) {
                Some(NameKind::Record) => {
                    self.check_type_args(t, 0, diags);
                    Ty::Record(root.clone())
                }
                Some(NameKind::Enum) => {
                    self.check_type_args(t, 0, diags);
                    Ty::Enum(root.clone())
                }
                Some(NameKind::Alias) => {
                    self.check_type_args(t, 0, diags);
                    self.aliases.get(root).cloned().unwrap_or(Ty::Unknown)
                }
                Some(NameKind::Import) => {
                    for a in &t.args {
                        self.resolve_ty(a, diags);
                    }
                    Ty::Unknown
                }
                Some(NameKind::Func) => {
                    self.push(
                        diags,
                        codes::UNDEFINED_TYPE,
                        format!("'{root}' is a function, not a type"),
                        t.span,
                        vec![direction("Reference a type here")],
                    );
                    Ty::Unknown
                }
                None => {
                    let builtins = ["Int", "String", "Bool", "Result", "Option"];
                    let candidates = self
                        .kinds
                        .iter()
                        .filter(|(_, k)| **k != NameKind::Func)
                        .map(|(n, _)| n.as_str())
                        .chain(builtins);
                    let fix = match suggestion(root, candidates) {
                        Some(s) => {
                            self.replace_fix(format!("Did you mean '{s}'?"), t.path[0].span, &s)
                        }
                        None => direction(format!("Define or import '{root}'")),
                    };
                    self.push(
                        diags,
                        codes::UNDEFINED_TYPE,
                        format!("undefined type '{root}'"),
                        t.span,
                        vec![fix],
                    );
                    Ty::Unknown
                }
            },
        }
    }

    fn simple_builtin(&self, t: &ast::Type, ty: Ty, diags: &mut Vec<Diagnostic>) -> Ty {
        self.check_type_args(t, 0, diags);
        ty
    }

    fn check_type_args(&self, t: &ast::Type, expected: usize, diags: &mut Vec<Diagnostic>) -> bool {
        if t.args.len() == expected {
            return true;
        }
        let name = &t.path[0].name;
        let message = if expected == 0 {
            format!("type '{name}' takes no type arguments")
        } else {
            format!(
                "type '{name}' takes {expected} type argument(s), found {}",
                t.args.len()
            )
        };
        self.push(
            diags,
            codes::TYPE_ARITY,
            message,
            t.span,
            vec![direction("Adjust the type arguments")],
        );
        false
    }
}

/// 型エイリアスを依存順に解決する。循環は E1002 を報告して Unknown で打ち切る。
fn ensure_alias(
    name: &str,
    raw: &HashMap<&str, &ast::TypeAlias>,
    env: &mut Env,
    visiting: &mut Vec<String>,
    diags: &mut Vec<Diagnostic>,
) {
    if env.aliases.contains_key(name) {
        return;
    }
    let Some(decl) = raw.get(name) else { return };
    if visiting.iter().any(|v| v == name) {
        env.push(
            diags,
            codes::UNDEFINED_TYPE,
            format!("type alias cycle involving '{name}'"),
            decl.name.span,
            vec![direction("Break the cycle by removing one of the aliases")],
        );
        env.aliases.insert(name.to_string(), Ty::Unknown);
        return;
    }
    visiting.push(name.to_string());
    let mut deps = Vec::new();
    collect_alias_deps(&decl.ty, env, &mut deps);
    for dep in deps {
        ensure_alias(&dep, raw, env, visiting, diags);
    }
    let underlying = env.resolve_ty(&decl.ty, diags);
    let ty = if decl.tag.is_some() {
        Ty::Tagged {
            name: name.to_string(),
            underlying: Box::new(underlying),
        }
    } else {
        underlying
    };
    env.aliases.insert(name.to_string(), ty);
    visiting.pop();
}

fn collect_alias_deps(t: &ast::Type, env: &Env, out: &mut Vec<String>) {
    if t.path.len() == 1 && env.kinds.get(&t.path[0].name) == Some(&NameKind::Alias) {
        out.push(t.path[0].name.clone());
    }
    for a in &t.args {
        collect_alias_deps(a, env, out);
    }
}

// ---------------------------------------------------------------------------
// 関数本体・契約の検査
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Body,
    Requires,
    Ensures,
}

struct FnChecker<'a> {
    env: &'a Env,
    diags: &'a mut Vec<Diagnostic>,
    func: &'a ast::FuncDecl,
    sig: &'a FuncSig,
    mode: Mode,
    /// 内側ほど後ろ。`scopes[0]` はパラメータ。
    scopes: Vec<HashMap<String, Ty>>,
}

impl FnChecker<'_> {
    fn check(mut self) {
        let mut top = HashMap::new();
        for (n, t) in &self.sig.params {
            top.insert(n.clone(), t.clone());
        }
        self.scopes.push(top);

        self.mode = Mode::Requires;
        for clause in &self.func.requires {
            self.check_contract_clause(clause);
        }

        self.mode = Mode::Ensures;
        let mut ensures_scope = HashMap::new();
        ensures_scope.insert("result".to_string(), self.sig.ret.clone());
        self.scopes.push(ensures_scope);
        for clause in &self.func.ensures {
            self.check_contract_clause(clause);
        }
        self.scopes.pop();

        self.mode = Mode::Body;
        self.check_block(&self.func.body);
    }

    fn push(&mut self, code: &str, message: String, span: SynSpan, fixes: Vec<Fix>) {
        self.env.push(self.diags, code, message, span, fixes);
    }

    fn check_contract_clause(&mut self, clause: &ast::Expr) {
        let t = self.infer(clause);
        if !t.compatible(&Ty::Bool) {
            self.push(
                codes::TYPE_MISMATCH,
                format!("contract clause must be a Bool expression, found '{t}'"),
                clause.span(),
                vec![direction("Use a Bool condition")],
            );
        }
    }

    fn check_block(&mut self, block: &ast::Block) {
        self.scopes.push(HashMap::new());
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        self.scopes.pop();
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Let(l) => self.check_let(l),
            ast::Stmt::If(i) => self.check_if(i),
            ast::Stmt::Return(r) => self.check_return(r),
            ast::Stmt::Expr(e) => {
                self.infer(&e.expr);
            }
        }
    }

    fn check_let(&mut self, l: &ast::LetStmt) {
        let value_ty = self.infer(&l.value);
        let mut bound = value_ty;
        if let Some(fail) = &l.else_fail {
            bound = match bound {
                Ty::Option(t) => *t,
                Ty::Result(t, _) => *t,
                Ty::Unknown => Ty::Unknown,
                other => {
                    self.push(
                        codes::TYPE_MISMATCH,
                        format!("'else fail' requires an Option or Result value, found '{other}'"),
                        l.value.span(),
                        vec![direction("Unwrap only Option or Result values")],
                    );
                    Ty::Unknown
                }
            };
            let fail_ty = self.infer(fail);
            match self.sig.ret.clone() {
                Ty::Result(_, err) => self.check_assign(&err, &fail_ty, fail.span()),
                Ty::Unknown => {}
                other => {
                    let fname = &self.func.name.name;
                    self.push(
                        codes::TYPE_MISMATCH,
                        format!(
                            "'else fail' requires the enclosing function to return Result, but '{fname}' returns '{other}'"
                        ),
                        fail.span(),
                        vec![direction("Change the return type to Result<..., ...>")],
                    );
                }
            }
        }
        if let Some(ann) = &l.ty {
            let ann_ty = self.env.resolve_ty(ann, self.diags);
            self.check_assign(&ann_ty, &bound, l.value.span());
            // 注釈を信頼して伝播する(カスケードエラー防止)。
            bound = ann_ty;
        }
        let scope = self.scopes.last_mut().expect("at least one scope");
        if scope.contains_key(&l.name.name) {
            let name = l.name.name.clone();
            self.push(
                codes::DUPLICATE_DEF,
                format!("duplicate definition of '{name}' in this block"),
                l.name.span,
                vec![direction("Rename the new binding")],
            );
        } else {
            scope.insert(l.name.name.clone(), bound);
        }
    }

    fn check_if(&mut self, i: &ast::IfStmt) {
        let cond = self.infer(&i.cond);
        if !cond.compatible(&Ty::Bool) {
            self.push(
                codes::TYPE_MISMATCH,
                format!("if condition must be 'Bool', found '{cond}'"),
                i.cond.span(),
                vec![direction("Use a Bool condition")],
            );
        }
        self.check_block(&i.then_block);
        match &i.else_branch {
            Some(ast::ElseBranch::If(nested)) => self.check_if(nested),
            Some(ast::ElseBranch::Block(b)) => self.check_block(b),
            None => {}
        }
    }

    fn check_return(&mut self, r: &ast::ReturnStmt) {
        let fname = self.func.name.name.clone();
        let ret = self.sig.ret.clone();
        match (&r.value, &ret) {
            (None, Ty::Unit) => {}
            (None, other) => {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!("'{fname}' must return a value of type '{other}'"),
                    r.span,
                    vec![direction(format!("Return a value of type '{other}'"))],
                );
            }
            (Some(value), Ty::Unit) => {
                let t = self.infer(value);
                if !t.compatible(&Ty::Unit) {
                    self.push(
                        codes::TYPE_MISMATCH,
                        format!("'{fname}' does not return a value, found '{t}'"),
                        value.span(),
                        vec![direction(
                            "Remove the return value or declare a return type",
                        )],
                    );
                }
            }
            (Some(value), expected) => {
                let t = self.infer(value);
                self.check_assign(expected, &t, value.span());
            }
        }
    }

    // -- 型の照合 -----------------------------------------------------------

    /// `found` を `expected` の文脈に置けるか検査し、不一致なら混同の種類に応じた
    /// Diagnostic(E2001 / E2005)を積む。
    fn check_assign(&mut self, expected: &Ty, found: &Ty, span: SynSpan) {
        if found.compatible(expected) {
            return;
        }
        match (expected, found) {
            (Ty::Tagged { name, underlying }, f) if underlying.compatible(f) => {
                self.push(
                    codes::TAGGED_CONFUSION,
                    format!(
                        "expected '{name}', found '{f}'; a tagged type does not mix with its base type"
                    ),
                    span,
                    vec![direction(format!("Construct a '{name}' value explicitly"))],
                );
            }
            (e, Ty::Tagged { name, underlying }) if underlying.compatible(e) => {
                self.push(
                    codes::TAGGED_CONFUSION,
                    format!(
                        "expected '{e}', found '{name}'; a tagged type does not mix with its base type"
                    ),
                    span,
                    vec![direction("Align both sides to the same tagged type")],
                );
            }
            (Ty::Tagged { name: a, .. }, Ty::Tagged { name: b, .. }) => {
                self.push(
                    codes::TAGGED_CONFUSION,
                    format!("cannot use '{b}' where '{a}' is expected; distinct tagged types do not mix"),
                    span,
                    vec![direction(format!("Pass a '{a}' value instead"))],
                );
            }
            (Ty::Result(ok, _), f) if ok.compatible(f) => {
                self.mismatch_with_wrap(expected, f, span, "Ok");
            }
            (Ty::Option(some), f) if some.compatible(f) => {
                self.mismatch_with_wrap(expected, f, span, "Some");
            }
            (Ty::Result(..), Ty::Option(..)) => {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!("expected '{expected}', found '{found}'"),
                    span,
                    vec![direction("Use Ok/Err instead of Some/None")],
                );
            }
            (Ty::Option(..), Ty::Result(..)) => {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!("expected '{expected}', found '{found}'"),
                    span,
                    vec![direction("Use Some/None instead of Ok/Err")],
                );
            }
            _ => {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!("expected '{expected}', found '{found}'"),
                    span,
                    vec![direction(format!(
                        "Change the expression to type '{expected}'"
                    ))],
                );
            }
        }
    }

    fn mismatch_with_wrap(&mut self, expected: &Ty, found: &Ty, span: SynSpan, ctor: &str) {
        let fix = Fix {
            title: format!("Wrap the value in '{ctor}(...)'"),
            edits: vec![
                TextEdit {
                    span: self.env.span(SynSpan::point(span.start)),
                    new_text: format!("{ctor}("),
                },
                TextEdit {
                    span: self.env.span(SynSpan::point(span.end)),
                    new_text: ")".to_string(),
                },
            ],
        };
        self.push(
            codes::TYPE_MISMATCH,
            format!("expected '{expected}', found '{found}'"),
            span,
            vec![fix],
        );
    }

    // -- 式の型推論 ---------------------------------------------------------

    fn infer(&mut self, expr: &ast::Expr) -> Ty {
        match expr {
            ast::Expr::Int { .. } => Ty::Int,
            ast::Expr::Str { .. } => Ty::Str,
            ast::Expr::Bool { .. } => Ty::Bool,
            ast::Expr::Name { name, span } => self.infer_name(name, *span),
            ast::Expr::Field { base, name, span } => self.infer_field(base, name, *span),
            ast::Expr::Call { callee, args, span } => self.infer_call(callee, args, *span),
            ast::Expr::Unary { op, expr, .. } => self.infer_unary(*op, expr),
            ast::Expr::Binary { op, lhs, rhs, span } => self.infer_binary(*op, lhs, rhs, *span),
            ast::Expr::RecordLit { path, fields, span } => {
                self.infer_record_lit(path, fields, *span)
            }
        }
    }

    fn lookup_scope(&self, name: &str) -> Option<Ty> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn infer_name(&mut self, name: &str, span: SynSpan) -> Ty {
        if let Some(t) = self.lookup_scope(name) {
            return t;
        }
        if self.mode == Mode::Requires && name == "result" {
            self.push(
                codes::CONTRACT_CONSTRUCT,
                "'result' is only available in 'ensures' clauses".to_string(),
                span,
                vec![direction("Move this condition to an 'ensures' clause")],
            );
            return Ty::Unknown;
        }
        match self.env.kinds.get(name) {
            Some(NameKind::Func) => {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!("function '{name}' must be called; functions are not values in v0.1"),
                    span,
                    vec![direction(format!("Call '{name}(...)'"))],
                );
                Ty::Unknown
            }
            Some(NameKind::Record) | Some(NameKind::Enum) | Some(NameKind::Alias) => {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!("type '{name}' cannot be used as a value"),
                    span,
                    vec![direction(format!("Construct a value of type '{name}'"))],
                );
                Ty::Unknown
            }
            Some(NameKind::Import) => Ty::Unknown,
            None => {
                let scope_names: Vec<String> =
                    self.scopes.iter().flat_map(|s| s.keys().cloned()).collect();
                let candidates = scope_names
                    .iter()
                    .map(|s| s.as_str())
                    .chain(self.env.kinds.keys().map(|s| s.as_str()));
                let fix = match suggestion(name, candidates) {
                    Some(s) => self
                        .env
                        .replace_fix(format!("Did you mean '{s}'?"), span, &s),
                    None => direction(format!("Define or import '{name}'")),
                };
                self.push(
                    codes::UNDEFINED_NAME,
                    format!("undefined name '{name}'"),
                    span,
                    vec![fix],
                );
                Ty::Unknown
            }
        }
    }

    fn infer_field(&mut self, base: &ast::Expr, name: &ast::Ident, _span: SynSpan) -> Ty {
        if let ast::Expr::Name { name: root, .. } = base {
            if self.lookup_scope(root).is_none() {
                match self.env.kinds.get(root.as_str()) {
                    Some(NameKind::Enum) => return self.variant_ref(root.clone(), name),
                    Some(NameKind::Import) => return Ty::Unknown,
                    Some(NameKind::Record) | Some(NameKind::Alias) => {
                        let root = root.clone();
                        self.push(
                            codes::UNKNOWN_FIELD,
                            format!("type '{root}' has no member '{}'", name.name),
                            name.span,
                            vec![direction("Access fields on a value, not on the type")],
                        );
                        return Ty::Unknown;
                    }
                    Some(NameKind::Func) => {
                        let root = root.clone();
                        self.push(
                            codes::TYPE_MISMATCH,
                            format!("'{root}' is a function; call it before accessing fields"),
                            name.span,
                            vec![direction(format!("Write '{root}(...).{}'", name.name))],
                        );
                        return Ty::Unknown;
                    }
                    None => {} // 未定義は一般経路(infer_name)で E1001 を報告する
                }
            }
        }
        let base_ty = self.infer(base);
        self.field_on(&base_ty, name)
    }

    fn field_on(&mut self, base: &Ty, name: &ast::Ident) -> Ty {
        match base {
            Ty::Unknown => Ty::Unknown,
            Ty::Tagged { underlying, .. } => self.field_on(&underlying.clone(), name),
            Ty::Record(r) => {
                let fields = self.env.records.get(r).cloned().unwrap_or_default();
                if let Some((_, ty)) = fields.iter().find(|(n, _)| n == &name.name) {
                    return ty.clone();
                }
                let field_names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
                let fix = match suggestion(&name.name, field_names.iter().copied()) {
                    Some(s) => self
                        .env
                        .replace_fix(format!("Did you mean '{s}'?"), name.span, &s),
                    None => direction(format!("Use one of the fields of '{r}'")),
                };
                self.push(
                    codes::UNKNOWN_FIELD,
                    format!("no field '{}' on record '{r}'", name.name),
                    name.span,
                    vec![fix],
                );
                Ty::Unknown
            }
            Ty::Result(..) => self.builtin_member(base, name, &["isOk", "isErr"]),
            Ty::Option(..) => self.builtin_member(base, name, &["isSome", "isNone"]),
            other => {
                let other = other.clone();
                self.push(
                    codes::UNKNOWN_FIELD,
                    format!("type '{other}' has no fields"),
                    name.span,
                    vec![direction("Access fields only on record values")],
                );
                Ty::Unknown
            }
        }
    }

    fn builtin_member(&mut self, base: &Ty, name: &ast::Ident, members: &[&str]) -> Ty {
        if members.contains(&name.name.as_str()) {
            return Ty::Bool;
        }
        let base = base.clone();
        self.push(
            codes::UNKNOWN_FIELD,
            format!(
                "no member '{}' on '{base}'; available members: {}",
                name.name,
                members.join(", ")
            ),
            name.span,
            vec![direction(format!("Use one of: {}", members.join(", ")))],
        );
        Ty::Unknown
    }

    /// `Enum.Variant`(呼び出しなしのバリアント参照)。Unit バリアントのみ値になれる。
    fn variant_ref(&mut self, enum_name: String, vname: &ast::Ident) -> Ty {
        match self.find_variant(&enum_name, &vname.name) {
            Some(VariantDef::Unit) => Ty::Enum(enum_name),
            Some(VariantDef::Tuple(_)) => {
                let v = vname.name.clone();
                self.push(
                    codes::UNKNOWN_VARIANT,
                    format!(
                        "variant '{v}' of '{enum_name}' carries a payload; construct it with '{enum_name}.{v}(...)'"
                    ),
                    vname.span,
                    vec![direction("Provide the payload arguments")],
                );
                Ty::Enum(enum_name)
            }
            Some(VariantDef::Record(_)) => {
                let v = vname.name.clone();
                self.push(
                    codes::UNKNOWN_VARIANT,
                    format!(
                        "variant '{v}' of '{enum_name}' has named fields; construct it with '{enum_name}.{v} {{ ... }}'"
                    ),
                    vname.span,
                    vec![direction("Provide the named fields")],
                );
                Ty::Enum(enum_name)
            }
            None => {
                self.unknown_variant(&enum_name, vname);
                Ty::Unknown
            }
        }
    }

    fn find_variant(&self, enum_name: &str, vname: &str) -> Option<VariantDef> {
        self.env
            .enums
            .get(enum_name)?
            .iter()
            .find(|(n, _)| n == vname)
            .map(|(_, d)| d.clone())
    }

    fn unknown_variant(&mut self, enum_name: &str, vname: &ast::Ident) {
        let variants: Vec<String> = self
            .env
            .enums
            .get(enum_name)
            .map(|vs| vs.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default();
        let fix = match suggestion(&vname.name, variants.iter().map(|s| s.as_str())) {
            Some(s) => self
                .env
                .replace_fix(format!("Did you mean '{s}'?"), vname.span, &s),
            None => direction(format!("Use one of the variants of '{enum_name}'")),
        };
        self.push(
            codes::UNKNOWN_VARIANT,
            format!("no variant '{}' on enum '{enum_name}'", vname.name),
            vname.span,
            vec![fix],
        );
    }

    fn infer_call(&mut self, callee: &ast::Expr, args: &[ast::Expr], span: SynSpan) -> Ty {
        match callee {
            ast::Expr::Name { name, span: nspan } if self.lookup_scope(name).is_none() => {
                self.call_named(&name.clone(), *nspan, args, span)
            }
            ast::Expr::Field {
                base, name: vname, ..
            } => {
                if let ast::Expr::Name { name: root, .. } = base.as_ref() {
                    if self.lookup_scope(root).is_none()
                        && self.env.kinds.get(root.as_str()) == Some(&NameKind::Enum)
                    {
                        return self.call_variant(&root.clone(), vname, args, span);
                    }
                }
                let callee_ty = self.infer(callee);
                self.call_value(&callee_ty, args, span)
            }
            _ => {
                let callee_ty = self.infer(callee);
                self.call_value(&callee_ty, args, span)
            }
        }
    }

    fn call_value(&mut self, callee_ty: &Ty, args: &[ast::Expr], span: SynSpan) -> Ty {
        for a in args {
            self.infer(a);
        }
        if *callee_ty != Ty::Unknown {
            let callee_ty = callee_ty.clone();
            self.push(
                codes::TYPE_MISMATCH,
                format!("expression of type '{callee_ty}' is not callable"),
                span,
                vec![direction("Call a function")],
            );
        }
        Ty::Unknown
    }

    fn call_named(&mut self, name: &str, nspan: SynSpan, args: &[ast::Expr], span: SynSpan) -> Ty {
        match name {
            "Ok" | "Err" | "Some" => {
                let arg_ty = self.check_ctor_args(name, 1, args, span);
                let inner = Box::new(arg_ty);
                return match name {
                    "Ok" => Ty::Result(inner, Box::new(Ty::Unknown)),
                    "Err" => Ty::Result(Box::new(Ty::Unknown), inner),
                    _ => Ty::Option(inner),
                };
            }
            "None" => {
                self.check_ctor_args(name, 0, args, span);
                return Ty::Option(Box::new(Ty::Unknown));
            }
            "old" => {
                if self.mode != Mode::Ensures {
                    self.push(
                        codes::CONTRACT_CONSTRUCT,
                        "'old(...)' is only available in 'ensures' clauses".to_string(),
                        span,
                        vec![direction("Move this condition to an 'ensures' clause")],
                    );
                }
                return self.check_ctor_args(name, 1, args, span);
            }
            _ => {}
        }
        match self.env.kinds.get(name) {
            Some(NameKind::Func) => self.call_local(name, args, span),
            Some(NameKind::Import) => {
                for a in args {
                    self.infer(a);
                }
                Ty::Unknown
            }
            Some(NameKind::Record) => {
                for a in args {
                    self.infer(a);
                }
                self.push(
                    codes::RECORD_LITERAL,
                    format!("record '{name}' is constructed with '{name} {{ ... }}', not a function call"),
                    span,
                    vec![direction("Use a record literal")],
                );
                Ty::Record(name.to_string())
            }
            Some(NameKind::Enum) => {
                for a in args {
                    self.infer(a);
                }
                self.push(
                    codes::UNKNOWN_VARIANT,
                    format!("enum '{name}' needs a variant: '{name}.Variant(...)'"),
                    span,
                    vec![direction("Name the variant to construct")],
                );
                Ty::Enum(name.to_string())
            }
            Some(NameKind::Alias) => {
                for a in args {
                    self.infer(a);
                }
                self.push(
                    codes::TYPE_MISMATCH,
                    format!("type '{name}' is not callable"),
                    span,
                    vec![direction("Call a function")],
                );
                Ty::Unknown
            }
            None => {
                for a in args {
                    self.infer(a);
                }
                let ctors = ["Ok", "Err", "Some", "None"];
                let scope_names: Vec<String> =
                    self.scopes.iter().flat_map(|s| s.keys().cloned()).collect();
                let candidates = scope_names
                    .iter()
                    .map(|s| s.as_str())
                    .chain(self.env.kinds.keys().map(|s| s.as_str()))
                    .chain(ctors);
                let fix = match suggestion(name, candidates) {
                    Some(s) => self
                        .env
                        .replace_fix(format!("Did you mean '{s}'?"), nspan, &s),
                    None => direction(format!("Define or import '{name}'")),
                };
                self.push(
                    codes::UNDEFINED_NAME,
                    format!("undefined name '{name}'"),
                    nspan,
                    vec![fix],
                );
                Ty::Unknown
            }
        }
    }

    /// 組み込みコンストラクタ(`Ok` / `Err` / `Some` / `None` / `old`)の引数検査。
    /// 1 引数のものは引数の型を返す。
    fn check_ctor_args(
        &mut self,
        name: &str,
        expected: usize,
        args: &[ast::Expr],
        span: SynSpan,
    ) -> Ty {
        if args.len() != expected {
            self.push(
                codes::TYPE_MISMATCH,
                format!(
                    "'{name}' takes exactly {expected} argument(s), found {}",
                    args.len()
                ),
                span,
                vec![direction("Adjust the arguments")],
            );
        }
        let mut first = Ty::Unknown;
        for (i, a) in args.iter().enumerate() {
            let t = self.infer(a);
            if i == 0 {
                first = t;
            }
        }
        first
    }

    fn call_local(&mut self, name: &str, args: &[ast::Expr], span: SynSpan) -> Ty {
        let callee = self
            .env
            .funcs
            .get(name)
            .cloned()
            .expect("kind Func implies a registered signature");
        if args.len() != callee.params.len() {
            self.push(
                codes::TYPE_MISMATCH,
                format!(
                    "'{name}' takes {} argument(s), found {}",
                    callee.params.len(),
                    args.len()
                ),
                span,
                vec![direction("Adjust the arguments")],
            );
        }
        for (arg, (_, pty)) in args.iter().zip(&callee.params) {
            let at = self.infer(arg);
            self.check_assign(&pty.clone(), &at, arg.span());
        }
        for arg in args.iter().skip(callee.params.len()) {
            self.infer(arg);
        }

        if self.mode == Mode::Body {
            // エフェクト検査: 呼び出し先の宣言エフェクトが呼び出し元の uses に
            // 包含されているか(推移的伝播 + 階層包含判定)。
            for eff in &callee.effects {
                if self.sig.effects.iter().any(|d| effects::covers(d, eff)) {
                    continue;
                }
                let caller = &self.func.name.name;
                let fix = match self.sig.uses_end {
                    Some(end) => Fix {
                        title: format!("Add '{eff}' to uses clause"),
                        edits: vec![TextEdit {
                            span: self.env.span(SynSpan::point(end)),
                            new_text: format!(", {eff}"),
                        }],
                    },
                    None => direction(format!("Add 'uses {eff}' to '{caller}'")),
                };
                self.env.push(
                    self.diags,
                    codes::EFFECT_UNDECLARED,
                    format!(
                        "effect '{eff}' used but not declared in 'uses' clause of '{caller}' (required by call to '{name}')"
                    ),
                    span,
                    vec![fix],
                );
            }
        } else if !callee.effects.is_empty() {
            // 契約純粋性検査: 契約式の中ではエフェクトを持つ関数を呼べない(spec §4)。
            self.push(
                codes::IMPURE_CONTRACT,
                format!(
                    "call to '{name}' (uses {}) is not allowed in a contract; contract expressions must be pure",
                    callee.effects.join(", ")
                ),
                span,
                vec![direction("Move the effectful call into the function body")],
            );
        }
        callee.ret
    }

    fn call_variant(
        &mut self,
        enum_name: &str,
        vname: &ast::Ident,
        args: &[ast::Expr],
        span: SynSpan,
    ) -> Ty {
        match self.find_variant(enum_name, &vname.name) {
            Some(VariantDef::Tuple(tys)) => {
                if args.len() != tys.len() {
                    let v = &vname.name;
                    self.push(
                        codes::UNKNOWN_VARIANT,
                        format!(
                            "variant '{v}' of '{enum_name}' takes {} value(s), found {}",
                            tys.len(),
                            args.len()
                        ),
                        span,
                        vec![direction("Adjust the payload")],
                    );
                }
                for (arg, ty) in args.iter().zip(&tys) {
                    let at = self.infer(arg);
                    self.check_assign(&ty.clone(), &at, arg.span());
                }
                for arg in args.iter().skip(tys.len()) {
                    self.infer(arg);
                }
                Ty::Enum(enum_name.to_string())
            }
            Some(VariantDef::Unit) => {
                for a in args {
                    self.infer(a);
                }
                let v = &vname.name;
                self.push(
                    codes::UNKNOWN_VARIANT,
                    format!(
                        "variant '{v}' of '{enum_name}' takes no payload; write '{enum_name}.{v}'"
                    ),
                    span,
                    vec![direction(format!(
                        "Remove the arguments: '{enum_name}.{v}'"
                    ))],
                );
                Ty::Enum(enum_name.to_string())
            }
            Some(VariantDef::Record(_)) => {
                for a in args {
                    self.infer(a);
                }
                let v = &vname.name;
                self.push(
                    codes::UNKNOWN_VARIANT,
                    format!(
                        "variant '{v}' of '{enum_name}' has named fields; construct it with '{enum_name}.{v} {{ ... }}'"
                    ),
                    span,
                    vec![direction("Use named fields")],
                );
                Ty::Enum(enum_name.to_string())
            }
            None => {
                for a in args {
                    self.infer(a);
                }
                self.unknown_variant(enum_name, vname);
                Ty::Unknown
            }
        }
    }

    fn infer_record_lit(
        &mut self,
        path: &[ast::Ident],
        fields: &[ast::RecordLitField],
        span: SynSpan,
    ) -> Ty {
        let root = &path[0].name;
        if path.len() == 1 {
            match self.env.kinds.get(root.as_str()) {
                Some(NameKind::Record) => {
                    let def = self.env.records.get(root).cloned().unwrap_or_default();
                    let owner = root.clone();
                    self.check_record_fields(&def, fields, &owner, span);
                    Ty::Record(owner)
                }
                Some(NameKind::Alias) => {
                    let resolved = self.env.aliases.get(root).cloned().unwrap_or(Ty::Unknown);
                    match &resolved {
                        Ty::Record(r) => {
                            let def = self.env.records.get(r).cloned().unwrap_or_default();
                            let owner = r.clone();
                            self.check_record_fields(&def, fields, &owner, span);
                            resolved
                        }
                        Ty::Unknown => {
                            self.infer_lit_fields(fields);
                            Ty::Unknown
                        }
                        _ => {
                            let root = root.clone();
                            self.infer_lit_fields(fields);
                            self.push(
                                codes::RECORD_LITERAL,
                                format!("'{root}' is not a record type"),
                                span,
                                vec![direction("Construct a record type here")],
                            );
                            Ty::Unknown
                        }
                    }
                }
                Some(NameKind::Enum) => {
                    let root = root.clone();
                    self.infer_lit_fields(fields);
                    self.push(
                        codes::UNKNOWN_VARIANT,
                        format!("enum '{root}' needs a variant: '{root}.Variant {{ ... }}'"),
                        span,
                        vec![direction("Name the variant to construct")],
                    );
                    Ty::Enum(root)
                }
                Some(NameKind::Import) => {
                    self.infer_lit_fields(fields);
                    Ty::Unknown
                }
                Some(NameKind::Func) => {
                    let root = root.clone();
                    self.infer_lit_fields(fields);
                    self.push(
                        codes::RECORD_LITERAL,
                        format!("'{root}' is not a record type"),
                        span,
                        vec![direction("Construct a record type here")],
                    );
                    Ty::Unknown
                }
                None => {
                    let root = root.clone();
                    self.infer_lit_fields(fields);
                    let candidates: Vec<&str> = self
                        .env
                        .kinds
                        .iter()
                        .filter(|(_, k)| matches!(k, NameKind::Record | NameKind::Enum))
                        .map(|(n, _)| n.as_str())
                        .collect();
                    let fix = match suggestion(&root, candidates.iter().copied()) {
                        Some(s) => {
                            self.env
                                .replace_fix(format!("Did you mean '{s}'?"), path[0].span, &s)
                        }
                        None => direction(format!("Define or import '{root}'")),
                    };
                    self.push(
                        codes::UNDEFINED_NAME,
                        format!("undefined name '{root}'"),
                        path[0].span,
                        vec![fix],
                    );
                    Ty::Unknown
                }
            }
        } else if path.len() == 2 && self.env.kinds.get(root.as_str()) == Some(&NameKind::Enum) {
            let enum_name = root.clone();
            let vname = &path[1];
            match self.find_variant(&enum_name, &vname.name) {
                Some(VariantDef::Record(def)) => {
                    let owner = format!("{enum_name}.{}", vname.name);
                    self.check_record_fields(&def, fields, &owner, span);
                    Ty::Enum(enum_name)
                }
                Some(VariantDef::Tuple(_)) => {
                    let v = &vname.name;
                    self.infer_lit_fields(fields);
                    self.push(
                        codes::UNKNOWN_VARIANT,
                        format!(
                            "variant '{v}' of '{enum_name}' carries a positional payload; construct it with '{enum_name}.{v}(...)'"
                        ),
                        span,
                        vec![direction("Use positional arguments")],
                    );
                    Ty::Enum(enum_name)
                }
                Some(VariantDef::Unit) => {
                    let v = &vname.name;
                    self.infer_lit_fields(fields);
                    self.push(
                        codes::UNKNOWN_VARIANT,
                        format!(
                            "variant '{v}' of '{enum_name}' takes no payload; write '{enum_name}.{v}'"
                        ),
                        span,
                        vec![direction(format!("Remove the braces: '{enum_name}.{v}'"))],
                    );
                    Ty::Enum(enum_name)
                }
                None => {
                    self.infer_lit_fields(fields);
                    self.unknown_variant(&enum_name, vname);
                    Ty::Unknown
                }
            }
        } else if self.env.kinds.get(root.as_str()) == Some(&NameKind::Import) {
            self.infer_lit_fields(fields);
            Ty::Unknown
        } else {
            let full: Vec<&str> = path.iter().map(|i| i.name.as_str()).collect();
            let full = full.join(".");
            self.infer_lit_fields(fields);
            if self.env.kinds.contains_key(root.as_str()) {
                self.push(
                    codes::RECORD_LITERAL,
                    format!("'{full}' is not a record type"),
                    span,
                    vec![direction("Construct a record type here")],
                );
            } else {
                self.push(
                    codes::UNDEFINED_NAME,
                    format!("undefined name '{full}'"),
                    span,
                    vec![direction(format!("Define or import '{full}'"))],
                );
            }
            Ty::Unknown
        }
    }

    /// 期待型が分からないリテラルでも、フィールド値の式は検査しておく。
    fn infer_lit_fields(&mut self, fields: &[ast::RecordLitField]) {
        for f in fields {
            match &f.value {
                Some(e) => {
                    self.infer(e);
                }
                None => {
                    self.infer_name(&f.name.name.clone(), f.name.span);
                }
            }
        }
    }

    fn check_record_fields(
        &mut self,
        def: &[(String, Ty)],
        fields: &[ast::RecordLitField],
        owner: &str,
        span: SynSpan,
    ) {
        let mut seen: HashSet<String> = HashSet::new();
        for f in fields {
            if !seen.insert(f.name.name.clone()) {
                self.push(
                    codes::RECORD_LITERAL,
                    format!("duplicate field '{}' in '{owner}' literal", f.name.name),
                    f.name.span,
                    vec![direction("Remove the duplicate field")],
                );
                continue;
            }
            let value_ty = match &f.value {
                Some(e) => self.infer(e),
                None => self.infer_name(&f.name.name.clone(), f.name.span),
            };
            match def.iter().find(|(n, _)| n == &f.name.name) {
                Some((_, ft)) => self.check_assign(&ft.clone(), &value_ty, f.span),
                None => {
                    let field_names: Vec<&str> = def.iter().map(|(n, _)| n.as_str()).collect();
                    let fix = match suggestion(&f.name.name, field_names.iter().copied()) {
                        Some(s) => {
                            self.env
                                .replace_fix(format!("Did you mean '{s}'?"), f.name.span, &s)
                        }
                        None => direction(format!("Use one of the fields of '{owner}'")),
                    };
                    self.push(
                        codes::UNKNOWN_FIELD,
                        format!("no field '{}' on '{owner}'", f.name.name),
                        f.name.span,
                        vec![fix],
                    );
                }
            }
        }
        let missing: Vec<&str> = def
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| !seen.contains(*n))
            .collect();
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|n| format!("'{n}'")).collect();
            self.push(
                codes::RECORD_LITERAL,
                format!("missing field(s) {} in '{owner}' literal", list.join(", ")),
                span,
                vec![direction("Add the missing field(s)")],
            );
        }
    }

    fn infer_unary(&mut self, op: ast::UnaryOp, expr: &ast::Expr) -> Ty {
        let t = self.infer(expr);
        match op {
            ast::UnaryOp::Neg => {
                if !t.is_numeric() {
                    self.push(
                        codes::TYPE_MISMATCH,
                        format!("negation requires an Int operand, found '{t}'"),
                        expr.span(),
                        vec![direction("Use an Int expression")],
                    );
                    return Ty::Unknown;
                }
                t
            }
            ast::UnaryOp::Not => {
                if !t.compatible(&Ty::Bool) {
                    self.push(
                        codes::TYPE_MISMATCH,
                        format!("'!' requires a Bool operand, found '{t}'"),
                        expr.span(),
                        vec![direction("Use a Bool expression")],
                    );
                }
                Ty::Bool
            }
        }
    }

    fn infer_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: SynSpan,
    ) -> Ty {
        use ast::BinOp::*;
        let lt = self.infer(lhs);
        let rt = self.infer(rhs);
        match op {
            Eq | Ne => {
                if !lt.compatible(&rt) {
                    self.compare_mismatch(&lt, &rt, span);
                }
                Ty::Bool
            }
            Lt | Gt | Le | Ge => {
                self.expect_numeric(&lt, lhs.span(), "ordering comparison");
                self.expect_numeric(&rt, rhs.span(), "ordering comparison");
                if lt.is_numeric() && rt.is_numeric() && !lt.compatible(&rt) {
                    self.compare_mismatch(&lt, &rt, span);
                }
                Ty::Bool
            }
            Add | Sub | Mul | Div => {
                let lok = self.expect_numeric(&lt, lhs.span(), "arithmetic");
                let rok = self.expect_numeric(&rt, rhs.span(), "arithmetic");
                if lok && rok && !lt.compatible(&rt) {
                    self.compare_mismatch(&lt, &rt, span);
                    return Ty::Unknown;
                }
                if !(lok && rok) {
                    return Ty::Unknown;
                }
                if lt == Ty::Unknown {
                    rt
                } else {
                    lt
                }
            }
            Implies => {
                for (t, e) in [(&lt, lhs), (&rt, rhs)] {
                    if !t.compatible(&Ty::Bool) {
                        self.push(
                            codes::TYPE_MISMATCH,
                            format!("'implies' requires Bool operands, found '{t}'"),
                            e.span(),
                            vec![direction("Use Bool expressions")],
                        );
                    }
                }
                Ty::Bool
            }
        }
    }

    fn expect_numeric(&mut self, t: &Ty, span: SynSpan, what: &str) -> bool {
        if t.is_numeric() {
            return true;
        }
        let t = t.clone();
        self.push(
            codes::TYPE_MISMATCH,
            format!("{what} requires Int operands, found '{t}'"),
            span,
            vec![direction("Use Int expressions")],
        );
        false
    }

    /// 比較・算術での型不一致。tagged 型の混同なら E2005、それ以外は E2001。
    fn compare_mismatch(&mut self, lt: &Ty, rt: &Ty, span: SynSpan) {
        let tagged_confusion = match (lt, rt) {
            (Ty::Tagged { underlying, .. }, other) | (other, Ty::Tagged { underlying, .. }) => {
                underlying.compatible(other) || matches!(other, Ty::Tagged { .. })
            }
            _ => false,
        };
        if tagged_confusion {
            self.push(
                codes::TAGGED_CONFUSION,
                format!("cannot compare '{lt}' with '{rt}'; tagged types do not mix"),
                span,
                vec![direction("Align both sides to the same tagged type")],
            );
        } else {
            self.push(
                codes::TYPE_MISMATCH,
                format!("cannot compare '{lt}' with '{rt}'"),
                span,
                vec![direction("Compare values of the same type")],
            );
        }
    }
}

// ---------------------------------------------------------------------------
// fix 構築・名前提案
// ---------------------------------------------------------------------------

fn direction(title: impl Into<String>) -> Fix {
    Fix {
        title: title.into(),
        edits: vec![],
    }
}

/// 編集距離 2 以内で最も近い候補(同距離なら辞書順で決定的に)。
fn suggestion<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        if c == name {
            continue;
        }
        let d = levenshtein(name, c);
        if d <= 2 && d < name.chars().count() {
            let better = match best {
                None => true,
                Some((bd, bc)) => d < bd || (d == bd && c < bc),
            };
            if better {
                best = Some((d, c));
            }
        }
    }
    best.map(|(_, c)| c.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
