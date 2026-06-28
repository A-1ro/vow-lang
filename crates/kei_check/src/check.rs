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
use crate::imports::{ModuleResolver, NoopResolver, ResolvedTypeDef, ResolvedVariant};
use crate::report::{CheckReport, ContractInfo, ContractKind, Verification};
use crate::types::Ty;
use crate::{Diagnostic, Fix, Position, Severity, Span, SuggestedContract, TextEdit};

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
    pub const MATCH_NOT_EXHAUSTIVE: &str = "KEI-E2007";
    pub const MATCH_UNREACHABLE_ARM: &str = "KEI-E2008";
    pub const MATCH_PATTERN: &str = "KEI-E2009";
    pub const UNSUPPORTED_EQUALITY: &str = "KEI-E2010";
    pub const EFFECT_UNDECLARED: &str = "KEI-E3001";
    pub const UNKNOWN_EFFECT: &str = "KEI-E3002";
    pub const DUPLICATE_EXTERN: &str = "KEI-E3003";
    pub const UNDECLARED_EXTERN_CALL: &str = "KEI-E3004";
    pub const QUERY_WITH_EFFECTS: &str = "KEI-E3005";
    pub const IMPURE_CONTRACT: &str = "KEI-E4001";
    pub const CONTRACT_CONSTRUCT: &str = "KEI-E4002";
    pub const CONST_FALSE_CONTRACT: &str = "KEI-E4003";
    pub const NON_QUERY_IN_CONTRACT: &str = "KEI-E4004";
    pub const GENERATIVE_COUNTEREXAMPLE: &str = "KEI-E4005";
    pub const CONTRACT_MISSING: &str = "KEI-E4008";
}

/// 検査オプション(M16 / #44)。既定はすべて off で、`check_module` /
/// `check_module_report` の従来挙動を保つ。strict 系はすべて**オプトイン**で、
/// 既存の golden を壊さない段階移行のための辺。
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckOptions {
    /// `extern` 未宣言の外部 namespace 呼び出しを警告する(KEI-E3004)。
    /// 既定 off。`kei check --strict-extern` で on。
    pub strict_extern: bool,
    /// 契約から property-based test を生成・実行する(M15 / #26)。既定 off。
    /// `kei check --generative` で on。純粋関数の ensures を generative へ格上げし、
    /// 反例があれば KEI-E4005 を出す。
    pub generative: bool,
    /// 構造化修正提案(M18 / #24)を出す。既定 off。`kei check --suggest-contracts` で on。
    /// 契約の無い純粋関数に ContractMissing(KEI-E4008 / suggested_contract)を提案する。
    pub suggest_contracts: bool,
}

/// 1 モジュールを検査し、出現位置に対応した順序で Diagnostic を返す(既定オプション)。
/// `file` はリポジトリルートからの相対パス(span の `file` フィールドに入る)。
pub fn check_module(file: &str, module: &ast::Module) -> Vec<Diagnostic> {
    check_module_with(file, module, CheckOptions::default())
}

/// [`check_module`] にオプションを与えた版(M16)。strict-extern 等の opt-in 検査を制御する。
pub fn check_module_with(file: &str, module: &ast::Module, opts: CheckOptions) -> Vec<Diagnostic> {
    check_module_with_resolver(file, module, opts, &NoopResolver)
}

/// [`check_module_with`] に import 解決リゾルバを与えた版(M20 / #55)。
/// `resolver` が `import` 先の型定義(record / enum / type alias)を返せば、
/// その名前は内部表に直接展開され、フィールドアクセスや match 網羅性が
/// 通常通り検査される。`None` を返す import は従来通り opaque。
pub fn check_module_with_resolver(
    file: &str,
    module: &ast::Module,
    opts: CheckOptions,
    resolver: &dyn ModuleResolver,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let (env, fn_sigs) = Env::build(file, module, &mut diags, resolver);
    for (item, sig) in module.items.iter().zip(&fn_sigs) {
        if let (ast::Item::Func(f), Some(sig)) = (item, sig) {
            FnChecker {
                env: &env,
                diags: &mut diags,
                func: f,
                sig,
                mode: Mode::Body,
                scopes: Vec::new(),
                opts,
                list_ops: None,
            }
            .check();
        }
    }
    // 構造化修正提案(M18 / #24)。opt-in。契約の無い純粋関数に ContractMissing を提案する。
    if opts.suggest_contracts {
        suggest_missing_contracts(&env, module, &mut diags);
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

/// List コンビネータのメソッド呼び出しの位置(Call span の開始 `(line, col)`)を集める(M9)。
/// emit はこの**権威的な型情報**だけを根拠に `get`/`fold`/`all`/`any`/`isEmpty` を配列メソッドへ
/// 写す(構文だけでレシーバが List か判別すると、外部呼び出しの連鎖や同名フィールドを誤写する)。
/// 検査器(型推論)そのものを使って収集するので、検査の再実装にはならない。
///
/// resolver なしの版(後方互換)。import 由来の型は opaque 扱いなので、
/// import 先 record の `List` フィールドに対するメソッド呼び出しは
/// 検出されない(emit が外部呼び出しとして写す)。M20 / #55 と整合的に
/// 解決させたい場合は [`list_op_spans_with_resolver`] を使う。
pub fn list_op_spans(module: &ast::Module) -> std::collections::HashSet<(u32, u32)> {
    list_op_spans_with_resolver(module, &NoopResolver)
}

/// [`list_op_spans`] に import 解決リゾルバを与えた版(M20 / #55)。
/// `Env::build` を同じ resolver で組むので、`kei check_module_with_resolver` と
/// 整合的な型情報の元で List メソッドを判定する。
pub fn list_op_spans_with_resolver(
    module: &ast::Module,
    resolver: &dyn ModuleResolver,
) -> std::collections::HashSet<(u32, u32)> {
    let file = "";
    let mut diags = Vec::new();
    let (env, fn_sigs) = Env::build(file, module, &mut diags, resolver);
    let mut spans = HashSet::new();
    for (item, sig) in module.items.iter().zip(&fn_sigs) {
        if let (ast::Item::Func(f), Some(sig)) = (item, sig) {
            FnChecker {
                env: &env,
                diags: &mut diags,
                func: f,
                sig,
                mode: Mode::Body,
                scopes: Vec::new(),
                opts: CheckOptions::default(),
                list_ops: Some(&mut spans),
            }
            .check();
        }
    }
    spans
}

/// 検査結果(診断)に加えて、各契約の達成検証レベルを併せて返す(M12)。
/// `kei check --json` が出力する構造化レポート。
pub fn check_module_report(file: &str, module: &ast::Module) -> CheckReport {
    check_module_report_with(file, module, CheckOptions::default())
}

/// [`check_module_report`] にオプションを与えた版(M16)。
pub fn check_module_report_with(
    file: &str,
    module: &ast::Module,
    opts: CheckOptions,
) -> CheckReport {
    check_module_report_with_resolver(file, module, opts, &NoopResolver)
}

/// [`check_module_report_with`] にリゾルバを与えた版(M20 / #55)。
/// 検査経路だけリゾルバを通し、契約レポート(`collect_contracts`)・PBT は
/// 既存のままにする(契約自体は対象モジュール内のシグネチャから直接組む)。
pub fn check_module_report_with_resolver(
    file: &str,
    module: &ast::Module,
    opts: CheckOptions,
    resolver: &dyn ModuleResolver,
) -> CheckReport {
    let mut diagnostics = check_module_with_resolver(file, module, opts, resolver);
    let mut contracts = collect_contracts(file, module);

    // 契約ベース PBT(M15 / #26)。静的検査がクリーンなときだけ走らせる(壊れた AST に
    // 評価器をかけない)。純粋関数の ensures を全生成入力で検証し、反例ゼロなら generative へ
    // 格上げ、反例があれば KEI-E4005 を出す。
    if opts.generative && !diagnostics.iter().any(|d| d.severity == Severity::Error) {
        apply_generative(file, module, &mut diagnostics, &mut contracts);
    }

    CheckReport {
        diagnostics,
        contracts,
    }
}

/// PBT の結果を診断・契約レベルに反映する(M15)。生成・判定は kei_check::pbt が担い、
/// ここは「結果 → 検証レベル格上げ / 反例診断」への写像だけを行う。
fn apply_generative(
    file: &str,
    module: &ast::Module,
    diagnostics: &mut Vec<Diagnostic>,
    contracts: &mut [ContractInfo],
) {
    for outcome in crate::pbt::run_module(module) {
        if outcome.passed {
            // 反例ゼロ: この関数の ensures(runtime 止まり)を generative へ格上げ。
            for c in contracts.iter_mut() {
                if c.func == outcome.func
                    && c.kind == ContractKind::Ensures
                    && c.verification == Verification::Runtime
                {
                    c.verification = Verification::Generative;
                }
            }
        } else if let Some(ce) = &outcome.counterexample {
            let span = Span {
                file: file.to_string(),
                start: Position {
                    line: ce.clause_span.start.line,
                    col: ce.clause_span.start.col,
                },
                end: Position {
                    line: ce.clause_span.end.line,
                    col: ce.clause_span.end.col,
                },
            };
            // 反例の種別で文面を分ける: ensures 違反 / 呼び出し先 requires 違反(throw)。
            let message = if ce.precondition {
                format!(
                    "'{}' throws for a generated input ({}): {}",
                    outcome.func,
                    ce.inputs_text(),
                    ce.clause
                )
            } else {
                format!(
                    "ensures '{}' of '{}' is violated by a generated input ({})",
                    ce.clause,
                    outcome.func,
                    ce.inputs_text()
                )
            };
            diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    codes::GENERATIVE_COUNTEREXAMPLE,
                    message,
                    span,
                    vec![direction(
                        "Fix the implementation to satisfy the contract, or correct the contract",
                    )],
                )
                .expect("generative counterexample carries a fix"),
            );
        }
    }
    // 反例診断を出現位置順に整列(check_module と同じ規約)。
    diagnostics.sort_by(|a, b| {
        (a.span.start.line, a.span.start.col, a.code.as_str()).cmp(&(
            b.span.start.line,
            b.span.start.col,
            b.code.as_str(),
        ))
    });
}

/// 構造化修正提案: ContractMissing(M18 / #24)。契約の無い純粋関数で、本体が単一の
/// `return <expr>` のものに、本体から導いた `ensures result == <expr>` を提案する。
/// 提案は機械適用可能(`suggested_contract`)で、適用すると check-clean(`result` は構築上
/// その式)になり、`--generative` では `generative` まで上がる。warning(opt-in)。
fn suggest_missing_contracts(env: &Env, module: &ast::Module, diags: &mut Vec<Diagnostic>) {
    for item in &module.items {
        let ast::Item::Func(f) = item else { continue };
        if !f.uses.is_empty() || !f.ensures.is_empty() {
            continue;
        }
        let Some(ret) = &f.ret else { continue };
        if !is_scalar_type(ret) {
            continue;
        }
        // 本体がちょうど 1 つの `return <expr>` のときだけ、式から事後条件を導ける。
        let [ast::Stmt::Return(r)] = f.body.stmts.as_slice() else {
            continue;
        };
        let Some(value) = &r.value else { continue };
        // 本体式が二項式なら括弧で包む。`result == x > 0` は `==` と `>` の優先順位で
        // `result == (x > 0)` に化けたり、`implies` が `result` を巻き込んだりするため、
        // 提案を適用したら必ず「result と本体式の比較」になるよう明示する。
        let body = contract_expr_text(value);
        let body = if matches!(value, ast::Expr::Binary { .. }) {
            format!("({body})")
        } else {
            body
        };
        let expr = format!("result == {body}");
        let diag = Diagnostic::new(
            Severity::Warning,
            codes::CONTRACT_MISSING,
            format!(
                "function '{}' has no postcondition; an ensures can be derived from its body",
                f.name.name
            ),
            env.span(f.name.span),
            vec![direction(format!("Add 'ensures {expr}'"))],
        )
        .expect("contract-missing diagnostic carries a fix")
        .with_suggested_contract(SuggestedContract {
            kind: "ContractMissing".to_string(),
            function: f.name.name.clone(),
            clause: "ensures".to_string(),
            expr,
        });
        diags.push(diag);
    }
}

/// スカラ組み込み型(`==` で事後条件にしやすい)か。
fn is_scalar_type(t: &ast::Type) -> bool {
    t.path.len() == 1
        && t.args.is_empty()
        && matches!(t.path[0].name.as_str(), "Int" | "Bool" | "String")
}

/// 各関数の requires / ensures を走査し、検証レベルを判定して報告を組む。
fn collect_contracts(file: &str, module: &ast::Module) -> Vec<ContractInfo> {
    let mut out = Vec::new();
    for item in &module.items {
        let ast::Item::Func(f) = item else { continue };
        for c in &f.requires {
            out.push(contract_info(file, &f.name.name, ContractKind::Requires, c));
        }
        for c in &f.ensures {
            out.push(contract_info(file, &f.name.name, ContractKind::Ensures, c));
        }
    }
    out
}

fn contract_info(file: &str, func: &str, kind: ContractKind, expr: &ast::Expr) -> ContractInfo {
    let s = expr.span();
    ContractInfo {
        func: func.to_string(),
        kind,
        expr: contract_expr_text(expr),
        verification: verification_of(expr),
        span: Span {
            file: file.to_string(),
            start: Position {
                line: s.start.line,
                col: s.start.col,
            },
            end: Position {
                line: s.end.line,
                col: s.end.col,
            },
        },
    }
}

/// 契約式を定数畳み込みで三分類する(M17 / #35)。`verification_of`(報告)と
/// 恒偽診断(`check_contract_clause` の KEI-E4003)が共有する単一の判定点。
/// 片方だけ分岐が増えてサイレント乖離する事故を防ぐ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContractTruth {
    /// 定数畳み込みで `true`。コンパイル時に成立が確定。
    AlwaysTrue,
    /// 定数畳み込みで `false`。処理系が反証済み(必ず違反する)。
    AlwaysFalse,
    /// 変数を含むなど定数畳み込み不能。実行時に判定する。
    Unknown,
}

fn classify_contract(expr: &ast::Expr) -> ContractTruth {
    match const_eval(expr) {
        Some(ConstVal::Bool(true)) => ContractTruth::AlwaysTrue,
        Some(ConstVal::Bool(false)) => ContractTruth::AlwaysFalse,
        _ => ContractTruth::Unknown,
    }
}

/// v0.2 の検証レベル判定(最小実装)。純粋・定数評価可能で**真**に畳める契約は
/// コンパイル時に成立が確定するため `static`。それ以外は実行時アサーションの
/// `runtime`(spec/kei-spec-v0.2.md §3)。SMT による本格 static は v1.0 送り。
///
/// 恒偽(`AlwaysFalse`)は `verification_of` 上は `runtime` に倒すが、実際には
/// `check_contract_clause` が KEI-E4003 でコンパイルエラーにするため実行時へは
/// 到達しない(報告の `static` は「成立が判定済み」を意味し、反証済みとは区別する)。
fn verification_of(expr: &ast::Expr) -> Verification {
    match classify_contract(expr) {
        ContractTruth::AlwaysTrue => Verification::Static,
        ContractTruth::AlwaysFalse | ContractTruth::Unknown => Verification::Runtime,
    }
}

#[derive(Debug, Clone)]
enum ConstVal {
    Int(i64),
    Bool(bool),
    Str(String),
}

/// 変数を含まない純粋な式を定数畳み込みする。畳めなければ `None`。
fn const_eval(expr: &ast::Expr) -> Option<ConstVal> {
    use ast::{BinOp::*, UnaryOp};
    match expr {
        ast::Expr::Int { value, .. } => Some(ConstVal::Int(*value)),
        ast::Expr::Bool { value, .. } => Some(ConstVal::Bool(*value)),
        ast::Expr::Str { value, .. } => Some(ConstVal::Str(value.clone())),
        ast::Expr::Unary { op, expr, .. } => match (op, const_eval(expr)?) {
            (UnaryOp::Neg, ConstVal::Int(n)) => Some(ConstVal::Int(n.checked_neg()?)),
            (UnaryOp::Not, ConstVal::Bool(b)) => Some(ConstVal::Bool(!b)),
            _ => None,
        },
        ast::Expr::Binary { op, lhs, rhs, .. } => {
            let l = const_eval(lhs)?;
            let r = const_eval(rhs)?;
            match (op, l, r) {
                (Add, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Int(a.checked_add(b)?)),
                (Sub, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Int(a.checked_sub(b)?)),
                (Mul, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Int(a.checked_mul(b)?)),
                (Div, ConstVal::Int(a), ConstVal::Int(b)) if b != 0 => {
                    Some(ConstVal::Int(a.checked_div(b)?))
                }
                (Rem, ConstVal::Int(a), ConstVal::Int(b)) if b != 0 => {
                    let q = a.checked_div(b)?;
                    Some(ConstVal::Int(a.checked_sub(q.checked_mul(b)?)?))
                }
                (Eq, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Bool(a == b)),
                (Ne, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Bool(a != b)),
                (Lt, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Bool(a < b)),
                (Gt, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Bool(a > b)),
                (Le, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Bool(a <= b)),
                (Ge, ConstVal::Int(a), ConstVal::Int(b)) => Some(ConstVal::Bool(a >= b)),
                (Eq, ConstVal::Bool(a), ConstVal::Bool(b)) => Some(ConstVal::Bool(a == b)),
                (Ne, ConstVal::Bool(a), ConstVal::Bool(b)) => Some(ConstVal::Bool(a != b)),
                // 文字列定数の等値比較(`requires "a" == "b"` 等の恒偽/恒真を畳む。M17 / #35)。
                (Eq, ConstVal::Str(a), ConstVal::Str(b)) => Some(ConstVal::Bool(a == b)),
                (Ne, ConstVal::Str(a), ConstVal::Str(b)) => Some(ConstVal::Bool(a != b)),
                (Or, ConstVal::Bool(a), ConstVal::Bool(b)) => Some(ConstVal::Bool(a || b)),
                (Implies, ConstVal::Bool(a), ConstVal::Bool(b)) => Some(ConstVal::Bool(!a || b)),
                _ => None,
            }
        }
        _ => None,
    }
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

/// `match` の被検査対象の構造。スクルティニの型から導出する。
enum MatchShape {
    /// `Option<T>`(内側 T)
    Option(Ty),
    /// `Result<T, E>`
    Result(Ty, Ty),
    /// ユーザー enum(名前 + バリアント定義)
    Enum(String, Vec<(String, VariantDef)>),
    /// import 由来など解決不能。網羅性検査は行わない(寛容)
    Unknown,
    /// `Int` / `String` 等、match できない具体型
    NonMatchable(Ty),
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

/// 外部境界の署名(M11)。`extern Time.now() -> Int uses Clock` の登録形。
#[derive(Debug, Clone)]
struct ExternSig {
    /// 純粋観測子(`extern query`、M14 / #45)。契約式から呼べる論理的読み取り。
    query: bool,
    params: Vec<(String, Ty)>,
    ret: Ty,
    effects: Vec<String>,
}

struct Env {
    file: String,
    kinds: HashMap<String, NameKind>,
    records: HashMap<String, Vec<(String, Ty)>>,
    enums: HashMap<String, Vec<(String, VariantDef)>>,
    aliases: HashMap<String, Ty>,
    funcs: HashMap<String, FuncSig>,
    /// 外部関数の署名。フルパス(`"Database.fetchBalance"`)で引く。
    externs: HashMap<String, ExternSig>,
}

impl Env {
    fn build(
        file: &str,
        module: &ast::Module,
        diags: &mut Vec<Diagnostic>,
        resolver: &dyn ModuleResolver,
    ) -> (Env, Vec<Option<FuncSig>>) {
        let mut env = Env {
            file: file.to_string(),
            kinds: HashMap::new(),
            records: HashMap::new(),
            enums: HashMap::new(),
            aliases: HashMap::new(),
            funcs: HashMap::new(),
            externs: HashMap::new(),
        };
        // 名前 → 最初の定義位置(重複メッセージと「最初の定義のみ有効」の判定)。
        let mut first: HashMap<String, SynSpan> = HashMap::new();
        // M20: import で導入された名前を別途記録する。`env.kinds` の値は
        // 解決の結果(Record / Enum / Alias / Import)で更新されるため、
        // 名前の **出自**(import か local か)を kinds だけでは区別できない。
        // IMPORT_CONFLICT vs DUPLICATE_DEF の判定に使う。
        let mut imported_names: HashSet<String> = HashSet::new();

        // 1) import 名の登録(M20: resolver があれば対象モジュールの型定義を引いて
        //    Record/Enum/Alias として持ち込み、無ければ従来通り opaque な Import に倒す)。
        for imp in &module.imports {
            let path_strs: Vec<String> = imp.path.iter().map(|i| i.name.clone()).collect();
            let resolved = resolver.resolve(&path_strs);
            let is_namespace_alias = imp.alias.is_some();

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
                    continue;
                }
                // namespace alias(`import a.b as Database`)は単一型ではなく
                // 名前空間。今回は従来通り opaque のまま据え置く(`Database.X` 経由の
                // 型解決は将来拡張)。
                let kind = if is_namespace_alias {
                    NameKind::Import
                } else if let Some(rm) = resolved.as_ref() {
                    match rm.type_defs.get(&ident.name) {
                        Some(ResolvedTypeDef::Record(fields)) => {
                            env.records.insert(ident.name.clone(), fields.clone());
                            NameKind::Record
                        }
                        Some(ResolvedTypeDef::Enum(variants)) => {
                            let internal: Vec<(String, VariantDef)> = variants
                                .iter()
                                .map(|(n, v)| {
                                    let def = match v {
                                        ResolvedVariant::Unit => VariantDef::Unit,
                                        ResolvedVariant::Tuple(ts) => VariantDef::Tuple(ts.clone()),
                                        ResolvedVariant::Record(fs) => {
                                            VariantDef::Record(fs.clone())
                                        }
                                    };
                                    (n.clone(), def)
                                })
                                .collect();
                            env.enums.insert(ident.name.clone(), internal);
                            NameKind::Enum
                        }
                        Some(ResolvedTypeDef::Alias(ty)) => {
                            env.aliases.insert(ident.name.clone(), ty.clone());
                            NameKind::Alias
                        }
                        None => NameKind::Import,
                    }
                } else {
                    NameKind::Import
                };
                env.kinds.insert(ident.name.clone(), kind);
                imported_names.insert(ident.name.clone());
                first.insert(ident.name.clone(), ident.span);
            }
        }

        // 2) item 名の登録(最初の定義が有効。以降は重複エラー)
        for item in &module.items {
            let (ident, kind) = match item {
                ast::Item::TypeAlias(a) => (&a.name, NameKind::Alias),
                ast::Item::Record(r) => (&r.name, NameKind::Record),
                ast::Item::Enum(e) => (&e.name, NameKind::Enum),
                ast::Item::Func(f) => (&f.name, NameKind::Func),
                // extern はローカル名を導入しない(外部パスの署名のみ)。
                ast::Item::Extern(_) => continue,
            };
            match first.get(&ident.name) {
                // M20: import で解決された名前(Record/Enum/Alias でも)とローカル
                // 定義が衝突したら IMPORT_CONFLICT を出す。`env.kinds` の値だけで
                // 判定すると、解決済み import が `NameKind::Record` 等になっているため
                // DUPLICATE_DEF に流れ込んでしまう。`imported_names` 集合で出自を保つ。
                Some(_) if imported_names.contains(&ident.name) => {
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
                Some(prev) => {
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
                None => {
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

        // 6) extern 署名の登録(M11)。重複は E3003、未知エフェクトは E3002。
        for item in &module.items {
            let ast::Item::Extern(e) = item else { continue };
            let key = e
                .path
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            if env.externs.contains_key(&key) {
                env.push(
                    diags,
                    codes::DUPLICATE_EXTERN,
                    format!("duplicate extern signature for '{key}'"),
                    e.span,
                    vec![direction(format!(
                        "Remove the duplicate extern for '{key}'"
                    ))],
                );
                continue;
            }
            let mut params = Vec::new();
            for p in &e.params {
                let ty = env.resolve_ty(&p.ty, diags);
                params.push((p.name.name.clone(), ty));
            }
            let ret = e
                .ret
                .as_ref()
                .map(|t| env.resolve_ty(t, diags))
                .unwrap_or(Ty::Unit);
            // 純粋観測子(query)は副作用を持てない。`uses` が付いていればエラー(M14)。
            if e.query && !e.uses.is_empty() {
                env.push(
                    diags,
                    codes::QUERY_WITH_EFFECTS,
                    format!(
                        "query observer '{key}' must be pure; a 'query' extern cannot declare 'uses'"
                    ),
                    e.span,
                    vec![direction("Remove 'uses' from the query, or drop 'query' if it has effects")],
                );
            }
            let mut effects = Vec::new();
            for u in &e.uses {
                let path = u
                    .path
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if effects::is_known(&path) {
                    if !effects.contains(&path) {
                        effects.push(path);
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
            env.externs.insert(
                key,
                ExternSig {
                    query: e.query,
                    params,
                    ret,
                    effects,
                },
            );
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
        self.push_sev(diags, Severity::Error, code, message, span, fixes);
    }

    fn push_sev(
        &self,
        diags: &mut Vec<Diagnostic>,
        severity: Severity,
        code: &str,
        message: String,
        span: SynSpan,
        fixes: Vec<Fix>,
    ) {
        diags.push(
            Diagnostic::new(severity, code, message, self.span(span), fixes)
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
            // 第三の組み込みジェネリクス(M9 / spec v0.3-collections §3)。型引数 1。
            "List" => {
                if !self.check_type_args(t, 1, diags) {
                    return Ty::Unknown;
                }
                Ty::List(Box::new(self.resolve_ty(&t.args[0], diags)))
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
                    let builtins = ["Int", "String", "Bool", "Result", "Option", "List"];
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
    opts: CheckOptions,
    /// List コンビネータのメソッド呼び出し位置(Call span の開始 line,col)を収集する
    /// 任意のシンク(M9 / emit の権威的な型情報)。通常検査では `None`。
    list_ops: Option<&'a mut HashSet<(u32, u32)>>,
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

    fn push_warning(&mut self, code: &str, message: String, span: SynSpan, fixes: Vec<Fix>) {
        self.env
            .push_sev(self.diags, Severity::Warning, code, message, span, fixes);
    }

    /// strict-extern(M16 / #44): `extern` 未宣言の外部 namespace 呼び出しを警告する。
    /// 宣言があれば `call_extern` が検証下に入れる。無いと従来は opaque で素通りするため、
    /// #22 の「外部呼び出しのエフェクト漏れ(uses 宣言漏れ)」が検出されない。strict では
    /// その呼び出しを警告し、`extern` 宣言の追加を促す(段階移行: まず warning)。
    fn warn_undeclared_extern(&mut self, key: &str, span: SynSpan) {
        self.push_warning(
            codes::UNDECLARED_EXTERN_CALL,
            format!(
                "external call '{key}' has no 'extern' declaration; its return type and effects are unverified"
            ),
            span,
            vec![direction(format!(
                "Declare 'extern {key}(...) -> ... uses ...' to bring this external call under verification"
            ))],
        );
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
        } else if classify_contract(clause) == ContractTruth::AlwaysFalse {
            // 定数恒偽(`requires false` / `requires 1 > 2`)は処理系が反証済み。
            // 実行時アサーションに落とさず、コンパイル時に静的エラーにする(M17 / #35)。
            let kw = match self.mode {
                Mode::Requires => "requires",
                Mode::Ensures => "ensures",
                Mode::Body => "contract",
            };
            self.push(
                codes::CONST_FALSE_CONTRACT,
                format!("contract is always false; this '{kw}' can never be satisfied"),
                clause.span(),
                vec![direction(
                    "Replace the contract with a satisfiable condition",
                )],
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
            ast::Expr::Match {
                scrutinee,
                arms,
                span,
            } => self.infer_match(scrutinee, arms, *span),
            ast::Expr::ListLit { elements, span } => self.infer_list_lit(elements, *span),
        }
    }

    /// List リテラル(M22 / #57)の要素を unify し、`Ty::List(T)` を返す。
    /// 要素が空なら `Ty::List(Unknown)`(let の型注釈や引数位置で具体化される)。
    /// 要素間の型不一致は KEI-E2001(最初の要素の型を期待値として後続要素にぶつける)。
    ///
    /// 要素間に不整合がある場合は要素型を `Unknown` に倒して返す。caller の
    /// `check_assign(Ty::List(expected), Ty::List(found))` が **同じ根本原因で
    /// 二重診断** を出すのを防ぐため(`Unknown` は全型と互換)。
    fn infer_list_lit(&mut self, elements: &[ast::Expr], _span: SynSpan) -> Ty {
        if elements.is_empty() {
            return Ty::List(Box::new(Ty::Unknown));
        }
        let first = self.infer(&elements[0]);
        let mut all_compatible = true;
        for e in &elements[1..] {
            let t = self.infer(e);
            let before = self.diags.len();
            self.check_assign(&first, &t, e.span());
            if self.diags.len() > before {
                all_compatible = false;
            }
        }
        if all_compatible {
            Ty::List(Box::new(first))
        } else {
            Ty::List(Box::new(Ty::Unknown))
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
            // List のプロパティ(引数なし)は `length` のみ。`isEmpty` を含むメソッドは
            // infer_call で処理するため、ここに来るのは呼び出しなしのアクセス(M9)。
            // `isEmpty` をメソッドにしているのは emit の曖昧性回避: レコードが `isEmpty`
            // フィールドを持つと `bag.isEmpty`(フィールドアクセス)を `.length === 0` へ
            // 誤写しうる。呼び出し形 `xs.isEmpty()` ならフィールドアクセスと構文的に区別でき、
            // レコードは呼べるフィールドを持てない(検査が弾く)ので衝突しない。
            Ty::List(_) => match name.name.as_str() {
                "length" => Ty::Int,
                "isEmpty" | "get" | "map" | "filter" | "fold" | "all" | "any" => {
                    let m = name.name.clone();
                    self.push(
                        codes::UNKNOWN_FIELD,
                        format!("'{m}' is a List method; call it as 'xs.{m}(...)'"),
                        name.span,
                        vec![direction(format!("Call the method: 'xs.{m}(...)'"))],
                    );
                    Ty::Unknown
                }
                _ => {
                    let members = [
                        "length", "isEmpty", "get", "map", "filter", "fold", "all", "any",
                    ];
                    let fix = match suggestion(&name.name, members.iter().copied()) {
                        Some(s) => {
                            self.env
                                .replace_fix(format!("Did you mean '{s}'?"), name.span, &s)
                        }
                        None => direction(format!("Use one of: {}", members.join(", "))),
                    };
                    self.push(
                        codes::UNKNOWN_FIELD,
                        format!("no member '{}' on 'List'", name.name),
                        name.span,
                        vec![fix],
                    );
                    Ty::Unknown
                }
            },
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
                // 外部境界(import した名前空間配下)の呼び出しで extern 署名が
                // あれば照合する。無ければ従来どおり opaque(M11 段階移行)。strict-extern
                // 時のみ、未宣言の外部呼び出しを警告する(M16 / #44)。
                if let Some(path) = field_path(callee) {
                    if self.lookup_scope(&path[0]).is_none()
                        && self.env.kinds.get(&path[0]) == Some(&NameKind::Import)
                    {
                        let key = path.join(".");
                        if let Some(sig) = self.env.externs.get(&key).cloned() {
                            return self.call_extern(&key, &sig, args, span);
                        }
                        if self.opts.strict_extern {
                            self.warn_undeclared_extern(&key, span);
                        }
                        // 未宣言の外部呼び出しは opaque: 引数だけ検査して Unknown を返す。
                        for a in args {
                            self.infer(a);
                        }
                        return Ty::Unknown;
                    }
                }
                // List コンビネータのメソッド呼び出し(M9)。レシーバが値のときだけ
                // 型を推論して List を見分ける(型名・未定義名の従来のエラー経路を保つ)。
                let base_is_value = match base.as_ref() {
                    ast::Expr::Name { name, .. } => self.lookup_scope(name).is_some(),
                    _ => true,
                };
                if base_is_value {
                    let base_ty = self.infer(base);
                    if let Ty::List(elem) = base_ty.clone() {
                        return self.list_method(&elem, vname, args, span);
                    }
                    let callee_ty = self.field_on(&base_ty, vname);
                    return self.call_value(&callee_ty, args, span);
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

    /// List コンビネータのメソッド呼び出し(M9 / spec v0.3-collections §4)。
    /// `elem` は要素型 T。関数引数は名前付き関数参照のみ許す(案2、§4.1)。
    fn list_method(
        &mut self,
        elem: &Ty,
        method: &ast::Ident,
        args: &[ast::Expr],
        span: SynSpan,
    ) -> Ty {
        // この呼び出しは「List レシーバ上のメソッド呼び出し」だと型推論が確定した点。emit は
        // この集合だけを根拠に配列メソッドへ写す(M9 / 権威的な型情報)。鍵は **メソッド名
        // トークンの位置**(`method.span`)。Call の span は連鎖だとレシーバ先頭で揃って
        // 衝突する(`db.get(id).get(0)` の内外で同一)ため使わない。メソッド名トークンは
        // 呼び出しごとに必ず異なる位置にある。
        if let Some(ops) = self.list_ops.as_deref_mut() {
            ops.insert((method.span.start.line, method.span.start.col));
        }
        match method.name.as_str() {
            // get(index: Int) -> Option<T>(範囲外は None)。
            "get" => {
                self.expect_arity(&method.name, 1, args, span);
                if let Some(a) = args.first() {
                    let at = self.infer(a);
                    self.check_assign(&Ty::Int, &at, a.span());
                }
                for a in args.iter().skip(1) {
                    self.infer(a);
                }
                Ty::Option(Box::new(elem.clone()))
            }
            // map(f: (T) -> U) -> List<U>。U は f の戻り型。
            "map" => {
                if !self.expect_arity(&method.name, 1, args, span) {
                    for a in args {
                        self.infer(a);
                    }
                    return Ty::List(Box::new(Ty::Unknown));
                }
                let u = self.check_combinator_fn_arg(
                    &args[0],
                    std::slice::from_ref(elem),
                    None,
                    "map",
                    span,
                );
                Ty::List(Box::new(u))
            }
            // filter(pred: (T) -> Bool) -> List<T>。
            "filter" => {
                if self.expect_arity(&method.name, 1, args, span) {
                    self.check_combinator_fn_arg(
                        &args[0],
                        std::slice::from_ref(elem),
                        Some(&Ty::Bool),
                        "filter",
                        span,
                    );
                }
                Ty::List(Box::new(elem.clone()))
            }
            // fold(init: U, f: (U, T) -> U) -> U。U は init の型。
            "fold" => {
                if !self.expect_arity(&method.name, 2, args, span) {
                    for a in args {
                        self.infer(a);
                    }
                    return Ty::Unknown;
                }
                let u = self.infer(&args[0]);
                self.check_combinator_fn_arg(
                    &args[1],
                    &[u.clone(), elem.clone()],
                    Some(&u),
                    "fold",
                    span,
                );
                u
            }
            // all / any(pred: (T) -> Bool) -> Bool。段階1の「量化子の代わり」。
            "all" | "any" => {
                if self.expect_arity(&method.name, 1, args, span) {
                    self.check_combinator_fn_arg(
                        &args[0],
                        std::slice::from_ref(elem),
                        Some(&Ty::Bool),
                        &method.name,
                        span,
                    );
                }
                Ty::Bool
            }
            // isEmpty(引数なしメソッド)-> Bool。空か。emit 衝突回避のためメソッド形
            // (`xs.isEmpty()`)で持つ(field_on のコメント参照)。
            "isEmpty" => {
                self.expect_arity(&method.name, 0, args, span);
                for a in args {
                    self.infer(a);
                }
                Ty::Bool
            }
            // length はプロパティ(引数なし)。呼び出し形で書いた。
            "length" => {
                self.push(
                    codes::TYPE_MISMATCH,
                    "'length' is a List property; write 'xs.length' without arguments".to_string(),
                    span,
                    vec![direction("Remove the call: 'xs.length'")],
                );
                for a in args {
                    self.infer(a);
                }
                Ty::Int
            }
            other => {
                let members = [
                    "length", "isEmpty", "get", "map", "filter", "fold", "all", "any",
                ];
                let fix = match suggestion(other, members.iter().copied()) {
                    Some(s) => {
                        self.env
                            .replace_fix(format!("Did you mean '{s}'?"), method.span, &s)
                    }
                    None => direction(format!("Use one of: {}", members.join(", "))),
                };
                self.push(
                    codes::UNKNOWN_FIELD,
                    format!("no method '{other}' on 'List'"),
                    method.span,
                    vec![fix],
                );
                for a in args {
                    self.infer(a);
                }
                Ty::Unknown
            }
        }
    }

    /// 引数の個数を検査する。合っていれば true。
    fn expect_arity(
        &mut self,
        name: &str,
        expected: usize,
        args: &[ast::Expr],
        span: SynSpan,
    ) -> bool {
        if args.len() == expected {
            return true;
        }
        self.push(
            codes::TYPE_MISMATCH,
            format!(
                "'{name}' takes {expected} argument(s), found {}",
                args.len()
            ),
            span,
            vec![direction("Adjust the arguments")],
        );
        false
    }

    /// コンビネータ引数の関数参照(案2 / spec v0.3-collections §4.1)。第一級関数値は
    /// 導入せず、トップレベル/同一モジュールの**純粋関数名**だけを引数位置で許す。
    /// 期待シグネチャ(params -> ret)と照合し、関数のエフェクトを呼び出し元へ
    /// 推移伝播する(契約式なら副作用は KEI-E4001)。戻り型を返す(map / fold の U 決定用)。
    fn check_combinator_fn_arg(
        &mut self,
        arg: &ast::Expr,
        params: &[Ty],
        ret: Option<&Ty>,
        method: &str,
        span: SynSpan,
    ) -> Ty {
        let ast::Expr::Name { name, span: nspan } = arg else {
            self.infer(arg);
            self.push(
                codes::TYPE_MISMATCH,
                format!(
                    "'{method}' expects a named function reference here; functions are not first-class values in Kei"
                ),
                arg.span(),
                vec![direction("Pass a top-level function name, e.g. 'xs.map(toItem)'")],
            );
            return Ty::Unknown;
        };
        // スコープ変数は関数参照になれない(関数は値ではない)。
        if self.lookup_scope(name).is_some() || self.env.kinds.get(name) != Some(&NameKind::Func) {
            self.push(
                codes::TYPE_MISMATCH,
                format!("'{name}' is not a function; '{method}' takes a named function reference"),
                *nspan,
                vec![direction("Reference a top-level function by name")],
            );
            return Ty::Unknown;
        }
        let sig = self
            .env
            .funcs
            .get(name)
            .cloned()
            .expect("Func kind implies a registered signature");
        if sig.params.len() != params.len() {
            self.push(
                codes::TYPE_MISMATCH,
                format!(
                    "function '{name}' takes {} parameter(s), but '{method}' passes {}",
                    sig.params.len(),
                    params.len()
                ),
                *nspan,
                vec![direction("Use a function with the expected parameters")],
            );
            return sig.ret.clone();
        }
        for ((pname, pty), expected) in sig.params.iter().zip(params) {
            if !pty.compatible(expected) {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!(
                        "'{method}' passes '{expected}' to parameter '{pname}' of '{name}', which expects '{pty}'"
                    ),
                    *nspan,
                    vec![direction(format!("Make '{name}' accept '{expected}'"))],
                );
            }
        }
        if let Some(expected_ret) = ret {
            if !sig.ret.compatible(expected_ret) {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!(
                        "'{method}' expects the function to return '{expected_ret}', but '{name}' returns '{}'",
                        sig.ret
                    ),
                    *nspan,
                    vec![direction(format!("Make '{name}' return '{expected_ret}'"))],
                );
            }
        }
        // 関数のエフェクトを呼び出し元へ推移伝播(§8.1。契約式なら KEI-E4001)。
        self.check_call_effects(&sig.effects, name, span);
        sig.ret.clone()
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
                // Tagged 明示コンストラクタ(M22 / #57):
                //   `let id = ProductId("P-001")` のような呼び出しを、
                //   alias の underlying と互換な引数 1 個を渡したときだけ許可する。
                //   非 tagged な alias や、underlying 不一致は従来通りエラー。
                if let Some(Ty::Tagged {
                    name: tag,
                    underlying,
                }) = self.env.aliases.get(name).cloned()
                {
                    let underlying = (*underlying).clone();
                    if args.len() != 1 {
                        for a in args {
                            self.infer(a);
                        }
                        self.push(
                            codes::TYPE_MISMATCH,
                            format!(
                                "tagged constructor '{name}' takes exactly 1 argument, found {}",
                                args.len()
                            ),
                            span,
                            vec![direction(format!(
                                "Call '{name}(<{underlying}>)' with a single value"
                            ))],
                        );
                        return Ty::Tagged {
                            name: tag,
                            underlying: Box::new(underlying),
                        };
                    }
                    let arg_ty = self.infer(&args[0]);
                    self.check_assign(&underlying, &arg_ty, args[0].span());
                    return Ty::Tagged {
                        name: tag,
                        underlying: Box::new(underlying),
                    };
                }
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

        self.check_call_effects(&callee.effects, name, span);
        callee.ret
    }

    /// 外部境界の呼び出し(M11)。extern 署名と引数を照合し、戻り型を返し、
    /// 宣言エフェクトを呼び出し元の uses へ推移伝播する(境界越しの E3001)。
    fn call_extern(&mut self, key: &str, sig: &ExternSig, args: &[ast::Expr], span: SynSpan) -> Ty {
        if args.len() != sig.params.len() {
            self.push(
                codes::TYPE_MISMATCH,
                format!(
                    "external function '{key}' takes {} argument(s), found {}",
                    sig.params.len(),
                    args.len()
                ),
                span,
                vec![direction("Adjust the arguments")],
            );
        }
        for (arg, (_, pty)) in args.iter().zip(&sig.params) {
            let at = self.infer(arg);
            self.check_assign(&pty.clone(), &at, arg.span());
        }
        for arg in args.iter().skip(sig.params.len()) {
            self.infer(arg);
        }
        // 契約式の中から呼べる外部関数は **query 観測子(純粋な論理的読み取り)だけ**
        // (M14 / #45)。query は無副作用なので old() でスナップショットでき、本体が
        // 外部状態をどう変えるかを ensures から直接読み取れる。非 query の extern を
        // 契約から呼ぶのは禁止(副作用の有無に関わらず観測子として宣言させる)。
        if self.mode != Mode::Body && !sig.query {
            self.push(
                codes::NON_QUERY_IN_CONTRACT,
                format!(
                    "external function '{key}' may only be called in a contract if declared 'extern query'; contracts observe state, they do not act on it"
                ),
                span,
                vec![direction(format!(
                    "Declare 'extern query {key}(...)' if it is a pure observer"
                ))],
            );
        } else {
            self.check_call_effects(&sig.effects, key, span);
        }
        sig.ret.clone()
    }

    /// 呼び出し先(ローカル関数 / extern)の宣言エフェクトを呼び出し元の uses に
    /// 照合する。本体では未宣言を E3001、契約式では副作用を E4001。
    fn check_call_effects(&mut self, effects: &[String], callee: &str, span: SynSpan) {
        if self.mode == Mode::Body {
            // 呼び出し先の宣言エフェクトが呼び出し元の uses に包含されているか
            // (推移的伝播 + 階層包含判定)。
            for eff in effects {
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
                        "effect '{eff}' used but not declared in 'uses' clause of '{caller}' (required by call to '{callee}')"
                    ),
                    span,
                    vec![fix],
                );
            }
        } else if !effects.is_empty() {
            // 契約純粋性検査: 契約式の中ではエフェクトを持つ呼び出しを禁止(spec §4)。
            self.push(
                codes::IMPURE_CONTRACT,
                format!(
                    "call to '{callee}' (uses {}) is not allowed in a contract; contract expressions must be pure",
                    effects.join(", ")
                ),
                span,
                vec![direction("Move the effectful call into the function body")],
            );
        }
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

    // -- match 式 ----------------------------------------------------------

    fn infer_match(&mut self, scrutinee: &ast::Expr, arms: &[ast::MatchArm], span: SynSpan) -> Ty {
        let scrut_ty = self.infer(scrutinee);
        let shape = self.match_shape(&scrut_ty);

        if let MatchShape::NonMatchable(t) = &shape {
            let t = t.clone();
            self.push(
                codes::MATCH_PATTERN,
                format!(
                    "cannot match on type '{t}'; match works on Option, Result, or enum values"
                ),
                scrutinee.span(),
                vec![direction("Match on an Option, Result, or enum value")],
            );
            // 本体の式は型検査だけしておく(カスケード防止)。
            for arm in arms {
                self.scopes.push(HashMap::new());
                self.infer(&arm.body);
                self.scopes.pop();
            }
            return Ty::Unknown;
        }

        let mut seen: Vec<String> = Vec::new();
        let mut result_ty = Ty::Unknown;
        let mut have_result = false;

        for arm in arms {
            let (key, bindings) = self.check_pattern(&arm.pattern, &shape);
            if let Some(k) = &key {
                if seen.contains(k) {
                    self.push(
                        codes::MATCH_UNREACHABLE_ARM,
                        format!("unreachable match arm: '{k}' is already covered above"),
                        arm.pattern.span,
                        vec![direction("Remove the duplicate arm")],
                    );
                } else {
                    seen.push(k.clone());
                }
            }
            self.scopes.push(bindings);
            let body_ty = self.infer(&arm.body);
            self.scopes.pop();
            if !have_result {
                result_ty = body_ty;
                have_result = true;
            } else if !body_ty.compatible(&result_ty) {
                self.push(
                    codes::TYPE_MISMATCH,
                    format!(
                        "match arms have incompatible types: expected '{result_ty}', found '{body_ty}'"
                    ),
                    arm.body.span(),
                    vec![direction("Make every arm produce the same type")],
                );
            } else if matches!(result_ty, Ty::Unknown) {
                // 先行する腕の本体型が Unknown(エラー由来など)だったとき、後続の具体型で
                // 結果型を回復する。Unknown は何とでも compatible なので上の不一致検査は通っており、
                // ここで具体型に差し替えると以降の不一致を実型で判定でき、波及エラーを抑えられる。
                result_ty = body_ty;
            }
        }

        self.check_exhaustiveness(&shape, &seen, span);
        result_ty
    }

    fn match_shape(&self, scrut: &Ty) -> MatchShape {
        match scrut {
            Ty::Option(t) => MatchShape::Option((**t).clone()),
            Ty::Result(t, e) => MatchShape::Result((**t).clone(), (**e).clone()),
            Ty::Enum(name) => match self.env.enums.get(name) {
                Some(variants) => MatchShape::Enum(name.clone(), variants.clone()),
                None => MatchShape::Unknown,
            },
            Ty::Tagged { underlying, .. } => self.match_shape(underlying),
            Ty::Unknown => MatchShape::Unknown,
            other => MatchShape::NonMatchable(other.clone()),
        }
    }

    /// パターンを `shape` に照合し、(網羅キー, 束縛) を返す。
    /// 不適合は KEI-E2009 を積み、束縛は付くだけ付ける(カスケード防止)。
    fn check_pattern(
        &mut self,
        pat: &ast::Pattern,
        shape: &MatchShape,
    ) -> (Option<String>, HashMap<String, Ty>) {
        let ctor: Vec<&str> = pat.path.iter().map(|i| i.name.as_str()).collect();
        match shape {
            MatchShape::Option(inner) => match ctor.as_slice() {
                ["Some"] => {
                    let b = self.bind_ctor_payload(pat, "Some", std::slice::from_ref(inner));
                    (Some("Some".to_string()), b)
                }
                ["None"] => {
                    self.expect_unit_payload(pat, "None");
                    (Some("None".to_string()), HashMap::new())
                }
                _ => {
                    self.bad_pattern(pat, "Some(x)' or 'None");
                    (None, self.loose_bindings(pat))
                }
            },
            MatchShape::Result(ok, err) => match ctor.as_slice() {
                ["Ok"] => {
                    let b = self.bind_ctor_payload(pat, "Ok", std::slice::from_ref(ok));
                    (Some("Ok".to_string()), b)
                }
                ["Err"] => {
                    let b = self.bind_ctor_payload(pat, "Err", std::slice::from_ref(err));
                    (Some("Err".to_string()), b)
                }
                _ => {
                    self.bad_pattern(pat, "Ok(x)' or 'Err(e)");
                    (None, self.loose_bindings(pat))
                }
            },
            MatchShape::Enum(name, variants) => self.check_enum_pattern(pat, name, variants),
            MatchShape::Unknown => {
                // 解決不能なスクルティニ: 束縛だけ付けて寛容に通す。
                let key = ctor.join(".");
                (Some(key), self.loose_bindings(pat))
            }
            MatchShape::NonMatchable(_) => (None, self.loose_bindings(pat)),
        }
    }

    fn check_enum_pattern(
        &mut self,
        pat: &ast::Pattern,
        enum_name: &str,
        variants: &[(String, VariantDef)],
    ) -> (Option<String>, HashMap<String, Ty>) {
        let ctor: Vec<&str> = pat.path.iter().map(|i| i.name.as_str()).collect();
        // enum パターンは `Enum.Variant` の 2 段形を要求する(構築形と対称)。
        let vname = match ctor.as_slice() {
            [e, v] if *e == enum_name => *v,
            _ => {
                self.push(
                    codes::MATCH_PATTERN,
                    format!(
                        "pattern '{}' does not match scrutinee of type '{enum_name}'; use '{enum_name}.Variant'",
                        ctor.join(".")
                    ),
                    pat.span,
                    vec![direction(format!("Write a '{enum_name}.Variant' pattern"))],
                );
                return (None, self.loose_bindings(pat));
            }
        };
        let Some((_, def)) = variants.iter().find(|(n, _)| n == vname) else {
            let names: Vec<&str> = variants.iter().map(|(n, _)| n.as_str()).collect();
            let fix = match suggestion(vname, names.iter().copied()) {
                Some(s) => {
                    self.env
                        .replace_fix(format!("Did you mean '{s}'?"), pat.path[1].span, &s)
                }
                None => direction(format!("Use one of the variants of '{enum_name}'")),
            };
            self.push(
                codes::MATCH_PATTERN,
                format!("no variant '{vname}' on enum '{enum_name}'"),
                pat.span,
                vec![fix],
            );
            return (None, self.loose_bindings(pat));
        };
        let bindings = match (def, &pat.payload) {
            (VariantDef::Unit, ast::PatternPayload::Unit) => HashMap::new(),
            (VariantDef::Tuple(tys), ast::PatternPayload::Tuple { bindings }) => {
                if bindings.len() != tys.len() {
                    self.push(
                        codes::MATCH_PATTERN,
                        format!(
                            "variant '{vname}' of '{enum_name}' binds {} value(s), found {}",
                            tys.len(),
                            bindings.len()
                        ),
                        pat.span,
                        vec![direction("Match the number of bound values")],
                    );
                }
                self.bind_tuple(pat, bindings, tys)
            }
            (VariantDef::Record(fields), ast::PatternPayload::Record { fields: pat_fields }) => {
                self.bind_record(pat, pat_fields, fields, enum_name, vname)
            }
            _ => {
                let want = match def {
                    VariantDef::Unit => format!("'{enum_name}.{vname}'"),
                    VariantDef::Tuple(_) => format!("'{enum_name}.{vname}(...)'"),
                    VariantDef::Record(_) => format!("'{enum_name}.{vname} {{ ... }}'"),
                };
                self.push(
                    codes::MATCH_PATTERN,
                    format!("variant '{vname}' of '{enum_name}' must be matched as {want}"),
                    pat.span,
                    vec![direction(format!("Write the pattern as {want}"))],
                );
                self.loose_bindings(pat)
            }
        };
        (Some(vname.to_string()), bindings)
    }

    /// 組み込みコンストラクタ(`Some` / `Ok` / `Err`)のペイロード束縛。
    /// 位置束縛を要求し、個数が合わなければ KEI-E2009。
    fn bind_ctor_payload(
        &mut self,
        pat: &ast::Pattern,
        name: &str,
        tys: &[Ty],
    ) -> HashMap<String, Ty> {
        match &pat.payload {
            ast::PatternPayload::Tuple { bindings } => {
                if bindings.len() != tys.len() {
                    self.push(
                        codes::MATCH_PATTERN,
                        format!(
                            "'{name}' binds exactly {} value(s), found {}",
                            tys.len(),
                            bindings.len()
                        ),
                        pat.span,
                        vec![direction(format!("Write '{name}(x)'"))],
                    );
                }
                self.bind_tuple(pat, bindings, tys)
            }
            _ => {
                self.push(
                    codes::MATCH_PATTERN,
                    format!("'{name}' binds its payload; write '{name}(x)'"),
                    pat.span,
                    vec![direction(format!("Write '{name}(x)'"))],
                );
                self.loose_bindings(pat)
            }
        }
    }

    fn expect_unit_payload(&mut self, pat: &ast::Pattern, name: &str) {
        if !matches!(pat.payload, ast::PatternPayload::Unit) {
            self.push(
                codes::MATCH_PATTERN,
                format!("'{name}' takes no payload; write '{name}'"),
                pat.span,
                vec![direction(format!("Write '{name}'"))],
            );
        }
    }

    /// 位置束縛(`Some(x)` / `E.V(a, b)`)を型に対応づける。
    fn bind_tuple(
        &mut self,
        pat: &ast::Pattern,
        bindings: &[ast::Ident],
        tys: &[Ty],
    ) -> HashMap<String, Ty> {
        let mut out = HashMap::new();
        for (i, b) in bindings.iter().enumerate() {
            let ty = tys.get(i).cloned().unwrap_or(Ty::Unknown);
            self.insert_binding(&mut out, b, ty, pat);
        }
        out
    }

    /// 名前付き束縛(`E.V { a, b }`)。全フィールドの列挙を要求する。
    fn bind_record(
        &mut self,
        pat: &ast::Pattern,
        pat_fields: &[ast::Ident],
        def_fields: &[(String, Ty)],
        enum_name: &str,
        vname: &str,
    ) -> HashMap<String, Ty> {
        let mut out = HashMap::new();
        let mut listed: HashSet<String> = HashSet::new();
        for f in pat_fields {
            listed.insert(f.name.clone());
            match def_fields.iter().find(|(n, _)| n == &f.name) {
                Some((_, ty)) => self.insert_binding(&mut out, f, ty.clone(), pat),
                None => {
                    let names: Vec<&str> = def_fields.iter().map(|(n, _)| n.as_str()).collect();
                    let fix = match suggestion(&f.name, names.iter().copied()) {
                        Some(s) => self
                            .env
                            .replace_fix(format!("Did you mean '{s}'?"), f.span, &s),
                        None => {
                            direction(format!("Use one of the fields of '{enum_name}.{vname}'"))
                        }
                    };
                    self.push(
                        codes::MATCH_PATTERN,
                        format!("no field '{}' on '{enum_name}.{vname}'", f.name),
                        f.span,
                        vec![fix],
                    );
                }
            }
        }
        let missing: Vec<&str> = def_fields
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| !listed.contains(*n))
            .collect();
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|n| format!("'{n}'")).collect();
            self.push(
                codes::MATCH_PATTERN,
                format!(
                    "match pattern for '{enum_name}.{vname}' must bind all field(s); missing {}",
                    list.join(", ")
                ),
                pat.span,
                vec![direction("Bind every field of the variant")],
            );
        }
        out
    }

    fn insert_binding(
        &mut self,
        out: &mut HashMap<String, Ty>,
        name: &ast::Ident,
        ty: Ty,
        _pat: &ast::Pattern,
    ) {
        if out.contains_key(&name.name) {
            self.push(
                codes::DUPLICATE_DEF,
                format!("duplicate binding '{}' in this pattern", name.name),
                name.span,
                vec![direction("Rename one of the bound variables")],
            );
        } else {
            out.insert(name.name.clone(), ty);
        }
    }

    /// 不適合パターンでも、束縛名は Unknown 型で導入しておく(本体の検査を続ける)。
    fn loose_bindings(&self, pat: &ast::Pattern) -> HashMap<String, Ty> {
        let mut out = HashMap::new();
        match &pat.payload {
            ast::PatternPayload::Unit => {}
            ast::PatternPayload::Tuple { bindings } => {
                for b in bindings {
                    out.insert(b.name.clone(), Ty::Unknown);
                }
            }
            ast::PatternPayload::Record { fields } => {
                for f in fields {
                    out.insert(f.name.clone(), Ty::Unknown);
                }
            }
        }
        out
    }

    fn bad_pattern(&mut self, pat: &ast::Pattern, expected: &str) {
        let ctor: Vec<&str> = pat.path.iter().map(|i| i.name.as_str()).collect();
        self.push(
            codes::MATCH_PATTERN,
            format!(
                "pattern '{}' does not match the scrutinee; expected '{expected}'",
                ctor.join(".")
            ),
            pat.span,
            vec![direction(format!("Write a '{expected}' pattern"))],
        );
    }

    fn check_exhaustiveness(&mut self, shape: &MatchShape, seen: &[String], span: SynSpan) {
        let required: Vec<String> = match shape {
            MatchShape::Option(_) => vec!["Some".to_string(), "None".to_string()],
            MatchShape::Result(..) => vec!["Ok".to_string(), "Err".to_string()],
            MatchShape::Enum(_, variants) => variants.iter().map(|(n, _)| n.clone()).collect(),
            MatchShape::Unknown | MatchShape::NonMatchable(_) => return,
        };
        let missing: Vec<String> = required.into_iter().filter(|r| !seen.contains(r)).collect();
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|m| format!("'{m}'")).collect();
            self.push(
                codes::MATCH_NOT_EXHAUSTIVE,
                format!(
                    "match is not exhaustive: missing arm(s) for {}",
                    list.join(", ")
                ),
                span,
                vec![direction(format!(
                    "Add an arm for each of: {}",
                    missing.join(", ")
                ))],
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
                } else if !lt.is_equatable() || !rt.is_equatable() {
                    // 型は一致するが合成型。emit は `===`(参照等価)しか出せず、
                    // 構造等価にならない(例: `result == xs.get(0)` は非空リストで
                    // 常に偽 → 契約が常に失敗する)。スカラー限定にする(spec v0.3)。
                    let bad = if !lt.is_equatable() { &lt } else { &rt };
                    self.push(
                        codes::UNSUPPORTED_EQUALITY,
                        format!(
                            "equality is not supported on '{bad}'; == / != compare scalars only \
                             (Int, String, Bool, and tagged scalars)"
                        ),
                        span,
                        vec![direction(
                            "Compare scalar fields, or use match to inspect the value",
                        )],
                    );
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
            Add | Sub | Mul | Div | Rem => {
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
            Or | Implies => {
                for (t, e) in [(&lt, lhs), (&rt, rhs)] {
                    if !t.compatible(&Ty::Bool) {
                        let op_text = if matches!(op, Or) { "||" } else { "implies" };
                        self.push(
                            codes::TYPE_MISMATCH,
                            format!("'{op_text}' requires Bool operands, found '{t}'"),
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

/// 契約式の Kei ソース表記の**唯一の正規実装**(#32)。
///
/// `CheckReport.contracts[].expr`(検証レポート)と実行時 `KeiContractViolation.condition`
/// は**バイト一致が要件**。後者を生成する `kei_emit` はこの関数へ委譲するため、優先順位表・
/// 結合方向・文字列エスケープを二重実装しない(片方だけ変えてサイレント乖離する事故を構造的に防ぐ)。
pub fn contract_expr_text(e: &ast::Expr) -> String {
    // kei_emit と同じ優先順位(Equality < Relational)。括弧を最小化する。
    fn bin_prec(op: ast::BinOp) -> u8 {
        use ast::BinOp::*;
        match op {
            Implies => 0,
            Or => 1,
            Eq | Ne => 2,
            Lt | Gt | Le | Ge => 3,
            Add | Sub => 4,
            Mul | Div | Rem => 5,
        }
    }
    fn bin_op_text(op: ast::BinOp) -> &'static str {
        use ast::BinOp::*;
        match op {
            Eq => "==",
            Ne => "!=",
            Lt => "<",
            Gt => ">",
            Le => "<=",
            Ge => ">=",
            Add => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            Rem => "%",
            Or => "||",
            Implies => "implies",
        }
    }
    // 子が二項式で親より弱く結合するときだけ括弧で包む。Postfix は 6 相当。
    fn child(e: &ast::Expr, parent: u8) -> String {
        let needs_paren = matches!(e, ast::Expr::Binary { op, .. } if bin_prec(*op) < parent);
        let text = contract_expr_text(e);
        if needs_paren {
            format!("({text})")
        } else {
            text
        }
    }
    match e {
        ast::Expr::Int { value, .. } => value.to_string(),
        ast::Expr::Str { value, .. } => contract_string_literal(value),
        ast::Expr::Bool { value, .. } => value.to_string(),
        ast::Expr::Name { name, .. } => name.clone(),
        ast::Expr::Field { base, name, .. } => format!("{}.{}", child(base, 6), name.name),
        ast::Expr::Call { callee, args, .. } => {
            let args: Vec<String> = args.iter().map(contract_expr_text).collect();
            format!("{}({})", child(callee, 6), args.join(", "))
        }
        ast::Expr::Unary { op, expr, .. } => {
            let sym = match op {
                ast::UnaryOp::Neg => "-",
                ast::UnaryOp::Not => "!",
            };
            format!("{sym}{}", child(expr, 6))
        }
        ast::Expr::Binary { op, lhs, rhs, .. } => {
            let p = bin_prec(*op);
            // implies は右結合、他は左結合(parser::parse_implies)。同優先度の子は
            // 結合方向に応じて括弧を保つ(`(a implies b) implies c` / `a - (b - c)`)。
            let (lhs_min, rhs_min) = if matches!(op, ast::BinOp::Implies) {
                (p + 1, p)
            } else {
                (p, p + 1)
            };
            format!(
                "{} {} {}",
                child(lhs, lhs_min),
                bin_op_text(*op),
                child(rhs, rhs_min)
            )
        }
        ast::Expr::RecordLit { path, fields, .. } => {
            let head: Vec<&str> = path.iter().map(|i| i.name.as_str()).collect();
            let fs: Vec<String> = fields
                .iter()
                .map(|f| match &f.value {
                    None => f.name.name.clone(),
                    Some(v) => format!("{}: {}", f.name.name, contract_expr_text(v)),
                })
                .collect();
            format!("{} {{ {} }}", head.join("."), fs.join(", "))
        }
        ast::Expr::Match {
            scrutinee, arms, ..
        } => {
            let arms: Vec<String> = arms
                .iter()
                .map(|arm| {
                    format!(
                        "{} => {}",
                        contract_pattern_text(&arm.pattern),
                        contract_expr_text(&arm.body)
                    )
                })
                .collect();
            format!(
                "match {} {{ {} }}",
                contract_expr_text(scrutinee),
                arms.join(", ")
            )
        }
        ast::Expr::ListLit { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(contract_expr_text).collect();
            format!("[{}]", elems.join(", "))
        }
    }
}

/// パターンの Kei ソース表記の正規実装(#32)。[`contract_expr_text`] と同様 `kei_emit` が委譲する。
pub fn contract_pattern_text(pat: &ast::Pattern) -> String {
    let head: Vec<&str> = pat.path.iter().map(|i| i.name.as_str()).collect();
    let head = head.join(".");
    match &pat.payload {
        ast::PatternPayload::Unit => head,
        ast::PatternPayload::Tuple { bindings } => {
            let bs: Vec<&str> = bindings.iter().map(|i| i.name.as_str()).collect();
            format!("{head}({})", bs.join(", "))
        }
        ast::PatternPayload::Record { fields } => {
            let fs: Vec<&str> = fields.iter().map(|i| i.name.as_str()).collect();
            format!("{head} {{ {} }}", fs.join(", "))
        }
    }
}

fn contract_string_literal(value: &str) -> String {
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

/// `Database.fetchBalance` のような Name/Field 連鎖をドット区切りのパスに戻す。
/// extern 署名の照合に使う(それ以外の式なら `None`)。
fn field_path(expr: &ast::Expr) -> Option<Vec<String>> {
    match expr {
        ast::Expr::Name { name, .. } => Some(vec![name.clone()]),
        ast::Expr::Field { base, name, .. } => {
            let mut path = field_path(base)?;
            path.push(name.name.clone());
            Some(path)
        }
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `requires <src>` を 1 つ持つ関数をパースし、その契約式を取り出す。
    fn requires_expr(src: &str) -> ast::Expr {
        let module = kei_syntax::parse_module(&format!(
            "module t\n\nfunc f(x: Int) -> Int\n  requires {src}\n{{\n  return x\n}}\n"
        ));
        assert!(
            module.errors.is_empty(),
            "test contract must parse: {:?}",
            module.errors
        );
        let ast::Item::Func(f) = &module.module.items[0] else {
            panic!("expected a function item");
        };
        f.requires[0].clone()
    }

    #[test]
    fn classify_contract_three_way() {
        // 恒真: 定数畳み込みで true。
        assert_eq!(
            classify_contract(&requires_expr("true")),
            ContractTruth::AlwaysTrue
        );
        assert_eq!(
            classify_contract(&requires_expr("2 > 1")),
            ContractTruth::AlwaysTrue
        );
        // 恒偽: 定数畳み込みで false。
        assert_eq!(
            classify_contract(&requires_expr("false")),
            ContractTruth::AlwaysFalse
        );
        assert_eq!(
            classify_contract(&requires_expr("1 > 2")),
            ContractTruth::AlwaysFalse
        );
        // 文字列定数の比較も畳む(#35 フォローアップ)。
        assert_eq!(
            classify_contract(&requires_expr("\"a\" == \"b\"")),
            ContractTruth::AlwaysFalse
        );
        assert_eq!(
            classify_contract(&requires_expr("\"a\" == \"a\"")),
            ContractTruth::AlwaysTrue
        );
        assert_eq!(
            classify_contract(&requires_expr("\"a\" != \"b\"")),
            ContractTruth::AlwaysTrue
        );
        // 変数を含む契約は定数畳み込み不能。
        assert_eq!(
            classify_contract(&requires_expr("x > 0")),
            ContractTruth::Unknown
        );
    }

    #[test]
    fn verification_levels_unchanged() {
        // 恒真は static、それ以外(恒偽・変数あり)は runtime のまま(M17 で不変)。
        assert_eq!(
            verification_of(&requires_expr("true")),
            Verification::Static
        );
        assert_eq!(
            verification_of(&requires_expr("false")),
            Verification::Runtime
        );
        assert_eq!(
            verification_of(&requires_expr("x > 0")),
            Verification::Runtime
        );
    }
}
