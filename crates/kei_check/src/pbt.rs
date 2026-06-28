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

use std::collections::{HashMap, HashSet};
use std::fmt;

use kei_syntax::ast;

use crate::check::contract_expr_text;
use crate::report::{ContractInfo, ContractKind, Verification};
use crate::{Diagnostic, Fix, Position, Severity, Span};

mod seed_codes {
    /// シードファイルの文法エラー(期待値フィールド混入を含む)。
    pub const SEED_GRAMMAR: &str = "KEI-E4006";
    /// シード入力が対象関数の requires / 型 / 名前に適合しない。
    pub const SEED_INVALID: &str = "KEI-E4007";
    /// シード入力が ensures を破った(反例)。生成テストと同じ KEI-E4005。
    pub const SEED_COUNTEREXAMPLE: &str = "KEI-E4005";
}

/// 評価器が扱う値。
/// 段階1(M15): スカラ(Int / Bool / String)のみ。
/// 段階2(M23 / #60): List / Record / Tagged を加え、List 引数を取る集計・計画関数の
/// `ensures` も generative に上げる。tagged は emit と同様に「型情報を持つ underlying」
/// として扱い、比較・等値は underlying と同等。
///
/// `PartialEq` は手書き(PR #76 review): record の field 順は構築経路で変わる
/// (`record_candidates` は宣言順、`RecordLit` 評価はソース順)ので derived な
/// positional 比較では同値判定が偽陽性 counterexample を生む。同じ name + 同じ
/// (field name → value) 集合なら等値とする構造比較に倒す。
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    /// `List<T>` の不変な並び。要素は同種(checker が保証)。
    List(Vec<Value>),
    /// record 値。フィールド名 → 値。順序は宣言順を保ち、反例の散文に使う。
    Record {
        name: String,
        fields: Vec<(String, Value)>,
    },
    /// tagged 型値。emit では underlying を branded していて runtime 上の構造は同じ。
    Tagged(String, Box<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (
                Value::Record {
                    name: an,
                    fields: af,
                },
                Value::Record {
                    name: bn,
                    fields: bf,
                },
            ) => {
                if an != bn || af.len() != bf.len() {
                    return false;
                }
                af.iter()
                    .all(|(k, v)| bf.iter().any(|(k2, v2)| k == k2 && v == v2))
            }
            (Value::Tagged(an, av), Value::Tagged(bn, bv)) => an == bn && av == bv,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::List(xs) => {
                write!(f, "[")?;
                for (i, v) in xs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Record { name, fields } => {
                write!(f, "{name} {{ ")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, " }}")
            }
            Value::Tagged(name, inner) => write!(f, "{name}({inner})"),
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
    /// 呼び出し先の `requires` を満たさず実行時なら違反送出になる(描述付き)。
    /// emit は全呼び出しで requires を検査するため、生成テストもそれに合わせる。
    Precondition(String),
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
    /// 部分検査(List 長 0..=2 など bounded サンプル)で通った場合 `true`。
    /// `apply_generative` がこのフラグを見て `Verification::Generative`(全数)と
    /// `Verification::Bounded`(部分)を選び分ける(PR #76 review)。
    pub bounded: bool,
}

#[derive(Debug, Clone)]
pub struct CounterExample {
    /// パラメータ名 → 値(最小化済み)。
    pub inputs: Vec<(String, Value)>,
    /// 破れた `ensures` 節の Kei ソース表記、または(precondition のとき)呼び出し先前提の描述。
    pub clause: String,
    /// 診断位置(ensures 節 / precondition なら関数名)の span。
    pub clause_span: kei_syntax::Span,
    /// 反例の種別。true なら呼び出し先 `requires` 違反(throw)、false なら `ensures` 違反。
    pub precondition: bool,
}

impl CounterExample {
    /// `available = 1, step = 0` のような入力の散文表記。
    pub fn inputs_text(&self) -> String {
        inputs_text(&self.inputs)
    }
}

/// 入力ベクタの散文表記(`available = 1, step = 0`)。生成経路・シード経路の反例メッセージで
/// 表記を 1 か所に集約する(複数箇所でフォーマットが乖離して golden が割れるのを防ぐ)。
fn inputs_text(inputs: &[(String, Value)]) -> String {
    inputs
        .iter()
        .map(|(n, v)| format!("{n} = {v}"))
        .collect::<Vec<_>>()
        .join(", ")
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
    // M23: record / tagged を解決できる軽量コンテキスト。候補生成の境界は
    // 「モジュール内の宣言だけ」(import 先の型は段階保守で対象外)。
    let ctx = DomainCtx::build(module);

    let mut out = Vec::new();
    for item in &module.items {
        let ast::Item::Func(f) = item else { continue };
        if let Some(outcome) = run_function(f, &funcs, &ctx) {
            out.push(outcome);
        }
    }
    out
}

/// 純粋関数 1 つを生成テストする。対象外なら `None`。
fn run_function(
    f: &ast::FuncDecl,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    ctx: &DomainCtx,
) -> Option<PropertyOutcome> {
    // 対象条件: 純粋(uses なし)・ensures あり・全パラメータがスカラ / List / record 生成可能。
    if !f.uses.is_empty() || f.ensures.is_empty() {
        return None;
    }
    let mut domains: Vec<Vec<Value>> = Vec::new();
    // 部分検査かどうか(PR #76 review): List 型 or 解決済み record/alias 型を
    // パラメータに持つ関数は bounded サンプルでしか検証できない。
    let mut bounded = false;
    for p in &f.params {
        if is_bounded_param_ty(&p.ty, ctx) {
            bounded = true;
        }
        // 各パラメータ独立で循環ガードを始める(パラメータ間は別物)。
        let mut visiting = HashSet::new();
        domains.push(candidate_values(&p.ty, ctx, false, &mut visiting)?);
    }

    // 生成ケース総数(各次元の候補数の積)を、デカルト積を実体化する前に見積もる。
    // 候補は Int=11 / String=3 / Bool=2 値なので Int 引数 N 個で 11^N になり、放置すると
    // 巨大 Vec の実体化で OOM/ハングに至る。上限超過・桁あふれは「全数検査できない」ため
    // 過信せず対象外にする(159/166/193 行と同じ安全側の哲学=部分検査で generative に
    // 上げない)。スキップした関数は verification レポートで runtime のまま現れる。
    const MAX_GENERATIVE_CASES: usize = 100_000;
    match domains
        .iter()
        .try_fold(1usize, |acc, d| acc.checked_mul(d.len()))
    {
        Some(n) if n <= MAX_GENERATIVE_CASES => {}
        _ => return None,
    }

    let combos = cartesian(&domains);
    let mut cases_checked = 0usize;
    // 反例: (入力, 破れた節のテキスト, 診断 span, precondition か)。
    let mut failures: Vec<(Vec<Value>, String, kei_syntax::Span, bool)> = Vec::new();
    let mut evaluable = false;

    for combo in &combos {
        let env: HashMap<String, Value> = f
            .params
            .iter()
            .map(|p| p.name.name.clone())
            .zip(combo.iter().cloned())
            .collect();

        // requires を満たす入力だけを検査対象にする(満たさない入力は捨てる)。
        // PR #76 review: `Err(Precondition)` は **この入力の評価過程で別関数の
        // requires が壊れた** ことを意味する。これは「その入力での前提評価が
        // 行えなかった」だけで、関数全体を対象外にする必要はない。当該入力だけ
        // 捨てて次へ進む(他の入力で生成的検証が成立する余地を残す)。`Unsupported`
        // は評価器が扱えない構文に触れたことなので、関数全体の前提評価信頼性が
        // 揺らぐ → 過信せず対象外。
        match all_hold(&f.requires, &env, funcs) {
            Ok(false) => continue,
            Ok(true) => {}
            Err(EvalError::Unsupported) => return None,
            Err(EvalError::Precondition(_)) | Err(EvalError::Trap) => continue,
        }

        // 関数本体を評価して result を得る。呼び出し先の requires 違反(throw)は反例。
        let result = match eval_func_call(f, combo, funcs, 0) {
            Ok(v) => v,
            Err(EvalError::Unsupported) => return None,
            Err(EvalError::Trap) => continue,
            Err(EvalError::Precondition(desc)) => {
                evaluable = true;
                cases_checked += 1;
                failures.push((combo.clone(), desc, f.name.span, true));
                continue;
            }
        };
        evaluable = true;

        // ensures を評価(result と old(param)=入力値を束縛)。
        let mut ens_env = env.clone();
        ens_env.insert("result".to_string(), result);
        for clause in &f.ensures {
            match eval_bool(clause, &ens_env, funcs, true) {
                Ok(true) => {}
                Ok(false) => failures.push((
                    combo.clone(),
                    contract_expr_text(clause),
                    clause.span(),
                    false,
                )),
                // ensures が評価できない(範囲外 / trap / 前提違反)→ その入力での契約成立を
                // 確認できない。実行時はその ensures が走り false / throw しうるので、黙って
                // スキップして generative に昇格させるのは過信。検証不能として対象外にする
                // (安全側に runtime のまま)。
                Err(_) => return None,
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
        .min_by_key(|(combo, ..)| size_metric(combo))
        .map(|(combo, clause, span, precondition)| CounterExample {
            inputs: f
                .params
                .iter()
                .map(|p| p.name.name.clone())
                .zip(combo.iter().cloned())
                .collect(),
            clause: clause.clone(),
            clause_span: *span,
            precondition: *precondition,
        });

    Some(PropertyOutcome {
        func: f.name.name.clone(),
        passed: counterexample.is_none(),
        cases_checked,
        counterexample,
        bounded,
    })
}

/// パラメータ型が部分検査(List 長 0..=2 / record 小ドメイン cartesian)に
/// なるかを判定(PR #76 review)。プリミティブ(Int/Bool/String)とその
/// alias は generative(全数 enumerable)とみなす。
fn is_bounded_param_ty(t: &ast::Type, ctx: &DomainCtx) -> bool {
    if t.path.len() != 1 {
        return false;
    }
    let root = t.path[0].name.as_str();
    if root == "List" {
        return true;
    }
    if ctx.records.contains_key(root) {
        return true;
    }
    if let Some(a) = ctx.aliases.get(root) {
        // tagged alias の underlying が bounded ならそれも bounded。
        return is_bounded_param_ty(&a.ty, ctx);
    }
    false
}

/// モジュール内宣言の軽量コンテキスト(record / type alias を引く)。M23 で
/// 候補生成が record / tagged を解決するために必要。
struct DomainCtx<'a> {
    records: HashMap<&'a str, &'a ast::RecordDecl>,
    aliases: HashMap<&'a str, &'a ast::TypeAlias>,
}

impl<'a> DomainCtx<'a> {
    fn build(module: &'a ast::Module) -> Self {
        let records: HashMap<&str, &ast::RecordDecl> = module
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Record(r) => Some((r.name.name.as_str(), r)),
                _ => None,
            })
            .collect();
        let aliases: HashMap<&str, &ast::TypeAlias> = module
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::TypeAlias(a) => Some((a.name.name.as_str(), a)),
                _ => None,
            })
            .collect();
        Self { records, aliases }
    }
}

/// 型ごとの決定的な候補入力集合。生成不能型(Option / Result / enum / 解決不能 import)は
/// `None`(対象外)。`deep` は List / record の **内部** で使う subdomain への切り替え
/// フラグで、組み合わせ爆発を抑える。
///
/// `visiting` は **現在展開中の record / alias 名** を覚える循環ガード(PR #76 review):
/// `record Node { next: Node }` や `type A = B; type B = A` のような自己参照型に対して、
/// 旧実装はスタックオーバーフローでクラッシュしていた。再入した型は対象外 (None) に倒し、
/// 関数を generative から静かに脱落させる(過信して反例なしに昇格させない)。
fn candidate_values(
    t: &ast::Type,
    ctx: &DomainCtx,
    deep: bool,
    visiting: &mut HashSet<String>,
) -> Option<Vec<Value>> {
    if t.path.len() != 1 {
        return None;
    }
    let root = t.path[0].name.as_str();
    if t.args.is_empty() {
        match root {
            "Int" => Some(int_domain(deep)),
            "Bool" => Some(vec![Value::Bool(false), Value::Bool(true)]),
            "String" => Some(str_domain(deep)),
            r if ctx.records.contains_key(r) => record_candidates(r, ctx, visiting),
            r if ctx.aliases.contains_key(r) => alias_candidates(r, ctx, deep, visiting),
            _ => None,
        }
    } else if root == "List" && t.args.len() == 1 {
        list_candidates(&t.args[0], ctx, visiting)
    } else {
        None
    }
}

fn int_domain(deep: bool) -> Vec<Value> {
    // 浅い文脈ではエッジ値を含む 11 値、深い文脈(List/Record 内)は 5 値に絞って爆発回避。
    // PR #76 review: 深い領域も「ゼロ周辺」(`-1, 0, 1`)+「大きめの境界」(`-100, 100`)を
    // 残す。`[-3, 0, 3]` だけだと `result != -200`/オーバーフロー領域が踏まれない。
    let arr: &[i64] = if deep {
        &[-100, -1, 0, 1, 100]
    } else {
        &[-100, -10, -3, -2, -1, 0, 1, 2, 3, 10, 100]
    };
    arr.iter().map(|n| Value::Int(*n)).collect()
}

fn str_domain(deep: bool) -> Vec<Value> {
    if deep {
        // PR #76 review: 深い領域でも **空文字** は境界として残す
        // (`requires items.all(nonEmpty)` のような契約が踏まれるように)。
        vec![Value::Str(String::new()), Value::Str("a".to_string())]
    } else {
        vec![
            Value::Str(String::new()),
            Value::Str("a".to_string()),
            Value::Str("abc".to_string()),
        ]
    }
}

fn record_candidates(
    name: &str,
    ctx: &DomainCtx,
    visiting: &mut HashSet<String>,
) -> Option<Vec<Value>> {
    // 循環ガード(PR #76 review): 自己参照(`record Node { next: Node }`)で
    // 同じ record を再展開しようとしたら None で打ち切る(対象外)。
    if !visiting.insert(name.to_string()) {
        return None;
    }
    let rec = ctx.records.get(name)?;
    let mut field_doms: Vec<(String, Vec<Value>)> = Vec::with_capacity(rec.fields.len());
    for f in &rec.fields {
        let dom = match candidate_values(&f.ty, ctx, true, visiting) {
            Some(d) => d,
            None => {
                visiting.remove(name);
                return None;
            }
        };
        field_doms.push((f.name.name.clone(), dom));
    }
    visiting.remove(name);
    let total = field_doms
        .iter()
        .try_fold(1usize, |a, (_, d)| a.checked_mul(d.len()))?;
    if total > 512 {
        // record 1 個の候補数が大きすぎると List 化で爆発する。安全側に対象外。
        // PR #76 review: 現状この経路で None が返ると関数全体が PBT 対象外になり、
        // ContractInfo の verification が `Runtime` のまま残る(ユーザには「なぜ
        // bounded に上がらない?」が見えない無言の脱落)。skip 理由を ContractInfo に
        // 載せる API 変更は大きいので、CLI レベルでの可視化は別 PR(follow-up)で扱う。
        return None;
    }
    // 全フィールド域のデカルト積 → `Value::Record` 列。`cartesian` ヘルパに
    // 委譲して二重実装を避ける(PR #76 review)。
    let combos = cartesian(
        &field_doms
            .iter()
            .map(|(_, d)| d.clone())
            .collect::<Vec<_>>(),
    );
    Some(
        combos
            .into_iter()
            .map(|combo| {
                let fields: Vec<(String, Value)> = field_doms
                    .iter()
                    .zip(combo)
                    .map(|((fname, _), v)| (fname.clone(), v))
                    .collect();
                Value::Record {
                    name: name.to_string(),
                    fields,
                }
            })
            .collect(),
    )
}

fn alias_candidates(
    name: &str,
    ctx: &DomainCtx,
    deep: bool,
    visiting: &mut HashSet<String>,
) -> Option<Vec<Value>> {
    // 循環ガード(`type A = B; type B = A`)。同じ alias を再展開しようとしたら
    // None で打ち切る(PR #76 review)。
    if !visiting.insert(name.to_string()) {
        return None;
    }
    let a = ctx.aliases.get(name)?;
    let underlying = candidate_values(&a.ty, ctx, deep, visiting);
    visiting.remove(name);
    let underlying = underlying?;
    match &a.tag {
        Some(_) => Some(
            underlying
                .into_iter()
                .map(|u| Value::Tagged(name.to_string(), Box::new(u)))
                .collect(),
        ),
        None => Some(underlying),
    }
}

fn list_candidates(
    elem_ty: &ast::Type,
    ctx: &DomainCtx,
    visiting: &mut HashSet<String>,
) -> Option<Vec<Value>> {
    let elem_dom = candidate_values(elem_ty, ctx, true, visiting)?;
    // 長さ 0..=2 を列挙。境界(空・単一・複数)を網羅。長さ 2 で要素のデカルト積が
    // 大きいと組み合わせ爆発するので、上限 30 を超えるときは「同一要素ペア」のみに絞る。
    // PR #76 review: 容量見積は実際の push 回数に合わせる(else 分岐の同一ペアモードでは
    // n + n の倍数だけ。`Vec::with_capacity(1 + n + n.pow(2))` は record 512 元から
    // 流れて来ると ~262K スロットの過剰確保になる)。
    let pair_total = elem_dom.len().saturating_mul(elem_dom.len());
    let length2_count = if pair_total <= 30 {
        pair_total
    } else {
        elem_dom.len()
    };
    let cap = 1 + elem_dom.len() + length2_count;
    let mut out: Vec<Value> = Vec::with_capacity(cap);
    out.push(Value::List(Vec::new()));
    for v in &elem_dom {
        out.push(Value::List(vec![v.clone()]));
    }
    if pair_total <= 30 {
        for a in &elem_dom {
            for b in &elem_dom {
                out.push(Value::List(vec![a.clone(), b.clone()]));
            }
        }
    } else {
        for v in &elem_dom {
            out.push(Value::List(vec![v.clone(), v.clone()]));
        }
    }
    Some(out)
}

/// 反例の「小ささ」(最小化用)。Int は絶対値、String は長さの総和。
/// List は長さ + 要素の総和、Record はフィールド値の総和、Tagged は underlying。
fn size_metric(combo: &[Value]) -> i64 {
    combo.iter().map(value_size).sum()
}

fn value_size(v: &Value) -> i64 {
    match v {
        Value::Int(n) => n.unsigned_abs() as i64,
        Value::Bool(_) => 0,
        Value::Str(s) => s.len() as i64,
        Value::List(xs) => xs.len() as i64 + xs.iter().map(value_size).sum::<i64>(),
        Value::Record { fields, .. } => fields.iter().map(|(_, v)| value_size(v)).sum(),
        Value::Tagged(_, inner) => value_size(inner),
    }
}

/// 各次元の候補のデカルト積(全組み合わせ)。
///
/// PR #76 review TODO: 現状は最大 100K combo を Vec で全物化する。1 個ずつ消費
/// するだけのループで使う場合、ピークメモリが膨らむ。mixed-radix iterator 化が
/// 望ましいが、`run_function` の反例最小化が後段で再走査するので即時値消費の
/// 単純 iterator では足りない。`failures: Vec<(Vec<Value>, ...)>` 側を見直す
/// follow-up が必要。
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
    // 呼び出し先の requires を引数で検査する。emit は全呼び出しで requires をアサートし、
    // 満たさなければ実行時に違反送出するので、生成テストも前提違反を Precondition として扱う
    // (満たさない入力で本体を素通り評価して generative に上げてしまう穴を塞ぐ)。
    for req in &f.requires {
        match eval_bool(req, &env, funcs, false) {
            Ok(true) => {}
            // 前提が定量的に偽 → 実行時 throw。反例として扱う。
            Ok(false) => {
                return Err(EvalError::Precondition(format!(
                    "requires '{}' of '{}' is not satisfied",
                    contract_expr_text(req),
                    f.name.name
                )))
            }
            // 前提が評価器の範囲外 / 評価破綻 → この呼び出しの妥当性を判定できない。
            // 実行時はその requires が走り throw しうるので、寛容スキップは過信になる。
            // 検証不能として伝播し(Unsupported)、呼び出し元を generative に上げない。
            Err(_) => return Err(EvalError::Unsupported),
        }
    }
    let result = eval_block(&f.body, env.clone(), funcs, depth)?;

    // 呼び出し先の ensures も検査する。emit は全呼び出しで ensures をアサートするため、
    // 呼び出し先が自身の事後条件を破ると実行時に throw する。トップレベル(depth 0)の
    // ensures は run_function 側が節ごとに反例報告するのでここでは見ない。ネスト呼び出し
    // (depth > 0)だけ、破れた ensures を Precondition(= 呼び出しが throw)として伝播し、
    // 呼び出し元が generative に上がるのを防ぐ。
    if depth > 0 {
        let mut ens_env = env;
        ens_env.insert("result".to_string(), result.clone());
        for ens in &f.ensures {
            match eval_bool(ens, &ens_env, funcs, true) {
                Ok(true) => {}
                Ok(false) => {
                    return Err(EvalError::Precondition(format!(
                        "ensures '{}' of '{}' is not satisfied",
                        contract_expr_text(ens),
                        f.name.name
                    )))
                }
                // ネスト先の ensures が評価不能 → 妥当性を判定できない → 検証不能。
                Err(_) => return Err(EvalError::Unsupported),
            }
        }
    }
    Ok(result)
}

/// ブロックを評価し、`return` の値を返す。`return` が無ければ Unsupported。
/// 関数本体用。`return` に必ず到達することを要求する点だけが [`eval_block_opt`] と違う。
fn eval_block(
    block: &ast::Block,
    env: HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    depth: usize,
) -> Result<Value, EvalError> {
    eval_block_opt(block, env, funcs, depth)?.ok_or(EvalError::Unsupported)
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
        // `implies` は右辺を短絡評価する。emit は `!(lhs) || rhs` に展開して短絡するので、
        // 前件が偽なら右辺(契約で守られたヘルパー呼び出しなど)を評価しない。前件が偽でも
        // 右辺を評価すると、`x > 0 implies positiveCheck(x)` で x<=0 のとき positiveCheck の
        // requires 違反を拾い、実行時には起きない反例を作ってしまう。
        ast::Expr::Binary {
            op: ast::BinOp::Implies,
            lhs,
            rhs,
            ..
        } => match eval_expr(lhs, env, funcs, in_ensures, depth)? {
            Value::Bool(false) => Ok(Value::Bool(true)),
            Value::Bool(true) => match eval_expr(rhs, env, funcs, in_ensures, depth)? {
                v @ Value::Bool(_) => Ok(v),
                _ => Err(EvalError::Unsupported),
            },
            _ => Err(EvalError::Unsupported),
        },
        ast::Expr::Binary {
            op: ast::BinOp::Or,
            lhs,
            rhs,
            ..
        } => match eval_expr(lhs, env, funcs, in_ensures, depth)? {
            Value::Bool(true) => Ok(Value::Bool(true)),
            Value::Bool(false) => match eval_expr(rhs, env, funcs, in_ensures, depth)? {
                v @ Value::Bool(_) => Ok(v),
                _ => Err(EvalError::Unsupported),
            },
            _ => Err(EvalError::Unsupported),
        },
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
                    // `old(...)` は進入時状態のスナップショット。emit は本体実行前
                    // (kei$result 初期化前)にキャプチャするので、`old(result)` は実行時に
                    // 未初期化参照で壊れる。評価器でも進入時環境(= result を除いたパラメータ
                    // のみ)で引数を評価し、`old(result)` を未解決=検証不能にして post-state の
                    // result を流し込んで generative に上げてしまう穴を塞ぐ。純粋関数では
                    // パラメータは不変なので `old(param) == param`。
                    let mut entry_env = env.clone();
                    entry_env.remove("result");
                    return eval_expr(&args[0], &entry_env, funcs, in_ensures, depth);
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
                // tagged 明示コンストラクタ `T(value)`(M22 / #57)の評価は段階1では
                // 未対応(Unsupported)。PR #76 review: かつてここで「1-arg Name 呼び出し
                // すべてを `Value::Tagged(name, underlying)` に包む」フォールバックを
                // 置いていたが、`Ok(x)` / `Err(x)` / `Some(x)` / ユーザ定義 record 名
                // などが誤って Tagged 化され、`Value::Record` との等値判定が偽陽性
                // counterexample を生む。tagged 名を解決するには module コンテキスト
                // (DomainCtx::aliases)を eval_expr まで持ち回す必要があるため、段階保守
                // として一律 Unsupported に倒す。tagged 引数の関数自体は candidate_values
                // が `Value::Tagged` を生成するので、let/return/Field 経路では透過する。
                return Err(EvalError::Unsupported);
            }
            // List コンビネータのメソッド呼び出し: `xs.method(args...)`。
            // PR #76 review: tagged alias で wrap された List(`type Items = List<Int>
            // tagged "Items"`)も同じ呼び出しを受けるので、Tagged を透過してから
            // 判定する。
            if let ast::Expr::Field { base, name, .. } = callee.as_ref() {
                let recv = eval_expr(base, env, funcs, in_ensures, depth)?;
                let unwrapped = unwrap_tagged(&recv);
                if let Value::List(xs) = unwrapped {
                    return eval_list_method(
                        name.name.as_str(),
                        xs,
                        args,
                        env,
                        funcs,
                        in_ensures,
                        depth,
                    );
                }
                return Err(EvalError::Unsupported);
            }
            Err(EvalError::Unsupported)
        }
        // M23: ListLit / RecordLit / Field を評価器に追加。これにより List 引数を取る
        // 集計・計画関数(`totalStockValue` 等)も generative の対象に入る。
        ast::Expr::ListLit { elements, .. } => {
            let mut xs = Vec::with_capacity(elements.len());
            for el in elements {
                xs.push(eval_expr(el, env, funcs, in_ensures, depth)?);
            }
            Ok(Value::List(xs))
        }
        ast::Expr::RecordLit { path, fields, .. } => {
            // 単一名の record(`R { ... }`)のみ評価する。enum バリアントの record 形は段階1
            // と同じく対象外(network/database のような外部値が混入することを避ける段階保守)。
            if path.len() != 1 {
                return Err(EvalError::Unsupported);
            }
            let name = path[0].name.clone();
            let mut out: Vec<(String, Value)> = Vec::with_capacity(fields.len());
            for f in fields {
                let v = match &f.value {
                    Some(expr) => eval_expr(expr, env, funcs, in_ensures, depth)?,
                    None => env
                        .get(&f.name.name)
                        .cloned()
                        .ok_or(EvalError::Unsupported)?,
                };
                out.push((f.name.name.clone(), v));
            }
            Ok(Value::Record { name, fields: out })
        }
        ast::Expr::Field { base, name, .. } => {
            let v = eval_expr(base, env, funcs, in_ensures, depth)?;
            match v {
                Value::Record { fields, .. } => fields
                    .iter()
                    .find(|(n, _)| n == &name.name)
                    .map(|(_, val)| val.clone())
                    .ok_or(EvalError::Unsupported),
                // List のフィールドアクセスは現状 `length` のみ(契約での `result.length` 等)。
                // PR #76 review: tagged で包まれた List も同じく拾う。
                Value::List(xs) if name.name == "length" => Ok(Value::Int(xs.len() as i64)),
                Value::Tagged(_, inner)
                    if matches!(*inner, Value::List(_)) && name.name == "length" =>
                {
                    if let Value::List(xs) = *inner {
                        Ok(Value::Int(xs.len() as i64))
                    } else {
                        unreachable!()
                    }
                }
                Value::Tagged(_, inner) => match *inner {
                    Value::Record { fields, .. } => fields
                        .iter()
                        .find(|(n, _)| n == &name.name)
                        .map(|(_, val)| val.clone())
                        .ok_or(EvalError::Unsupported),
                    _ => Err(EvalError::Unsupported),
                },
                _ => Err(EvalError::Unsupported),
            }
        }
        // 段階1の評価器はスカラのみ(match / Option / Result は未対応)。
        _ => Err(EvalError::Unsupported),
    }
}

/// List コンビネータの評価。`elem` は呼び出し時のレシーバの要素型(emit と同じく
/// 内部判別子なし: 全要素同種を仮定。Length/isEmpty はゼロ引数、all/any/filter は
/// 1 引数の関数参照、fold は (init, fn) の 2 引数、get は (index) の 1 引数)。
fn eval_list_method(
    method: &str,
    xs: &[Value],
    args: &[ast::Expr],
    env: &HashMap<String, Value>,
    funcs: &HashMap<&str, &ast::FuncDecl>,
    in_ensures: bool,
    depth: usize,
) -> Result<Value, EvalError> {
    match method {
        "length" => {
            if !args.is_empty() {
                return Err(EvalError::Unsupported);
            }
            Ok(Value::Int(xs.len() as i64))
        }
        "isEmpty" => {
            if !args.is_empty() {
                return Err(EvalError::Unsupported);
            }
            Ok(Value::Bool(xs.is_empty()))
        }
        "get" => {
            // `get(i)` は Option<T> を返すが、段階2の評価器は Option 値を持たないので
            // Unsupported に倒す(get を使う関数は generative に上げない)。
            Err(EvalError::Unsupported)
        }
        "all" | "any" | "map" | "filter" => {
            if args.len() != 1 {
                return Err(EvalError::Unsupported);
            }
            // 関数引数は名前付き関数参照(M9 / spec v0.3-collections §4.1)。
            let fname = match &args[0] {
                ast::Expr::Name { name, .. } => name.clone(),
                _ => return Err(EvalError::Unsupported),
            };
            let f = funcs
                .get(fname.as_str())
                .copied()
                .ok_or(EvalError::Unsupported)?;
            if !f.uses.is_empty() {
                return Err(EvalError::Unsupported);
            }
            // PR #76 review: 4 メソッドが同形のループだったので apply ヘルパに
            // 委譲して per-element 評価の重複を解消する。Bool 消費は各分岐側で行う。
            let apply = |x: &Value| eval_func_call(f, std::slice::from_ref(x), funcs, depth + 1);
            match method {
                "all" => {
                    for x in xs {
                        match apply(x)? {
                            Value::Bool(true) => {}
                            Value::Bool(false) => return Ok(Value::Bool(false)),
                            _ => return Err(EvalError::Unsupported),
                        }
                    }
                    Ok(Value::Bool(true))
                }
                "any" => {
                    for x in xs {
                        match apply(x)? {
                            Value::Bool(true) => return Ok(Value::Bool(true)),
                            Value::Bool(false) => {}
                            _ => return Err(EvalError::Unsupported),
                        }
                    }
                    Ok(Value::Bool(false))
                }
                "map" => {
                    let mut out = Vec::with_capacity(xs.len());
                    for x in xs {
                        out.push(apply(x)?);
                    }
                    Ok(Value::List(out))
                }
                "filter" => {
                    let mut out = Vec::new();
                    for x in xs {
                        match apply(x)? {
                            Value::Bool(true) => out.push(x.clone()),
                            Value::Bool(false) => {}
                            _ => return Err(EvalError::Unsupported),
                        }
                    }
                    Ok(Value::List(out))
                }
                _ => unreachable!(),
            }
        }
        "fold" => {
            if args.len() != 2 {
                return Err(EvalError::Unsupported);
            }
            // PR #76 review: init を `in_ensures=false` 固定で評価していたため、
            // `ensures result == xs.fold(old(seed), addFn)` のように init に
            // `old(...)` を入れると Unsupported で落ちていた。外側の文脈を伝播する。
            let init = eval_expr(&args[0], env, funcs, in_ensures, depth)?;
            let fname = match &args[1] {
                ast::Expr::Name { name, .. } => name.clone(),
                _ => return Err(EvalError::Unsupported),
            };
            let f = funcs
                .get(fname.as_str())
                .copied()
                .ok_or(EvalError::Unsupported)?;
            if !f.uses.is_empty() {
                return Err(EvalError::Unsupported);
            }
            let mut acc = init;
            for x in xs {
                acc = eval_func_call(f, &[acc, x.clone()], funcs, depth + 1)?;
            }
            Ok(acc)
        }
        _ => Err(EvalError::Unsupported),
    }
}

/// Tagged を underlying まで剥がす(PR #76 review)。pbt の比較・算術・List
/// メソッドは「tagged は underlying と等価」が前提なので、入口で透過する。
fn unwrap_tagged(v: &Value) -> &Value {
    let mut cur = v;
    while let Value::Tagged(_, inner) = cur {
        cur = inner;
    }
    cur
}

fn eval_binary(op: ast::BinOp, l: Value, r: Value) -> Result<Value, EvalError> {
    use ast::BinOp::*;
    use Value::{Bool, Int};
    // PR #76 review: tagged スカラの算術・比較は underlying と同等扱い。
    // 入口でアンラップしてから既存パターンに乗せる。
    let l = unwrap_tagged(&l).clone();
    let r = unwrap_tagged(&r).clone();
    match (op, l, r) {
        (Add, Int(a), Int(b)) => a.checked_add(b).map(Int).ok_or(EvalError::Trap),
        (Sub, Int(a), Int(b)) => a.checked_sub(b).map(Int).ok_or(EvalError::Trap),
        (Mul, Int(a), Int(b)) => a.checked_mul(b).map(Int).ok_or(EvalError::Trap),
        (Div, Int(_), Int(0)) => Err(EvalError::Trap),
        (Div, Int(a), Int(b)) => a.checked_div(b).map(Int).ok_or(EvalError::Trap),
        (Rem, Int(_), Int(0)) => Err(EvalError::Trap),
        (Rem, Int(a), Int(b)) => {
            let q = a.checked_div(b).ok_or(EvalError::Trap)?;
            a.checked_sub(q.checked_mul(b).ok_or(EvalError::Trap)?)
                .map(Int)
                .ok_or(EvalError::Trap)
        }
        (Eq, a, b) => Ok(Bool(a == b)),
        (Ne, a, b) => Ok(Bool(a != b)),
        // 順序比較は Int 限定(checker が KEI-E2001 で String/合成型を弾く)。
        (Lt, Int(a), Int(b)) => Ok(Bool(a < b)),
        (Gt, Int(a), Int(b)) => Ok(Bool(a > b)),
        (Le, Int(a), Int(b)) => Ok(Bool(a <= b)),
        (Ge, Int(a), Int(b)) => Ok(Bool(a >= b)),
        // `Or` / `Implies` は eval_expr が短絡処理するためここには来ない(到達不能)。
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
///
/// シードが ensures を破った関数の契約は、`contracts` 上で `generative` から `runtime` へ
/// **降格**する(生成器の固定ドメインでは反例ゼロでも、シードがドメイン外で破ったなら
/// その契約は generative とは言えない。レポートが「generative」と「KEI-E4005」を同時に
/// 主張する矛盾を防ぐ)。
pub fn check_seeds(
    file: &str,
    source: &str,
    module: &ast::Module,
    contracts: &mut [ContractInfo],
) -> Vec<Diagnostic> {
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

    // (func, Some(ensures 式) / None=全 ensures)= シードが破った契約の降格対象。
    let mut downgrades: Vec<(String, Option<String>)> = Vec::new();
    for seed in &seeds {
        validate_seed(seed, &funcs, file, &mut diags, &mut downgrades);
    }
    // 降格を契約レベルに反映(generative → runtime)。
    for (func, expr) in &downgrades {
        for c in contracts.iter_mut() {
            if c.func == *func
                && c.kind == ContractKind::Ensures
                && c.verification == Verification::Generative
                && expr.as_ref().is_none_or(|e| &c.expr == e)
            {
                c.verification = Verification::Runtime;
            }
        }
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
    /// 閉じ引用符なしで EOF に達した文字列(文法エラーにする)。
    UnterminatedStr,
    /// 数字を伴わない `-` / 桁あふれなどの不正な整数(文法エラーにする)。
    BadInt,
    /// 文法に属さない未知の文字(`!` など)。寛容に飛ばさず文法エラーにする。
    Unknown(char),
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
                let mut terminated = false;
                while i < chars.len() {
                    if chars[i] == '"' {
                        terminated = true;
                        i += 1; // 閉じ "
                        col += 1;
                        break;
                    }
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
                    // 生の改行を含む文字列でも後続トークンの行・列が揃うよう advance を使う
                    // (col だけ進めると改行後の位置がずれる)。
                    advance(chars[i], &mut line, &mut col);
                    i += 1;
                }
                // 複数行をまたぐと col が start_col より小さくなりうるので saturating。
                let len = col.saturating_sub(start_col).max(1);
                // 閉じ引用符を見ずに EOF に達したら未終端文字列(文法エラー)。
                let kind = if terminated {
                    SeedTok::Str(s)
                } else {
                    SeedTok::UnterminatedStr
                };
                toks.push(SeedToken {
                    kind,
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
                let mut digits = 0usize;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    num.push(chars[i]);
                    digits += 1;
                    i += 1;
                    col += 1;
                }
                let len = (col - start_col).max(1);
                // 数字を 1 つも伴わない(`-` 単体)/ 桁あふれは不正な整数リテラル。
                // 黙って 0 に丸めず文法エラーにする(誤った入力を生成検査へ注入しない)。
                let kind = match num.parse::<i64>() {
                    Ok(v) if digits > 0 => SeedTok::Int(v),
                    _ => SeedTok::BadInt,
                };
                toks.push(SeedToken {
                    kind,
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
                // 未知文字は寛容に飛ばさず、未知トークンとして残す。パーサの各エラー経路が
                // KEI-E4006 で弾く(`input { x: 1 ! }` や `x!: 1` のような不正なシードが
                // 黙って通る穴を塞ぐ)。
                toks.push(SeedToken {
                    kind: SeedTok::Unknown(c),
                    line: start_line,
                    col: start_col,
                    len: 1,
                });
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
            SeedTok::UnterminatedStr => {
                let t = self.cur();
                let (l, c, len) = (t.line, t.col, t.len);
                self.err(
                    "unterminated string literal (missing closing '\"')".to_string(),
                    l,
                    c,
                    len,
                    "Close the string with '\"'",
                );
                None
            }
            SeedTok::BadInt => {
                let t = self.cur();
                let (l, c, len) = (t.line, t.col, t.len);
                self.err(
                    "malformed integer literal".to_string(),
                    l,
                    c,
                    len,
                    "Write a valid integer (e.g. -1, 0, 42)",
                );
                None
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
    downgrades: &mut Vec<(String, Option<String>)>,
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
    let inputs_text = || inputs_text(&seed.inputs);
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
        let counterexample = |diags: &mut Vec<Diagnostic>, msg: String| {
            diags.push(
                Diagnostic::new(
                    Severity::Error,
                    seed_codes::SEED_COUNTEREXAMPLE,
                    msg,
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
        };
        match eval_func_call(f, &args, funcs, 0) {
            Ok(result) => {
                let mut ens_env = env.clone();
                ens_env.insert("result".to_string(), result);
                for clause in &f.ensures {
                    let expr = contract_expr_text(clause);
                    match eval_bool(clause, &ens_env, funcs, true) {
                        Ok(true) => {}
                        // ensures が偽 → 反例。
                        Ok(false) => {
                            counterexample(
                                diags,
                                format!(
                                    "ensures '{}' of '{}' is violated by the seeded input ({})",
                                    expr,
                                    seed.func,
                                    inputs_text()
                                ),
                            );
                            downgrades.push((seed.func.clone(), Some(expr)));
                        }
                        // ensures の評価自体が throw する(節内ヘルパーの契約違反など)→ 実行時も
                        // その ensures チェックが throw する。反例として報告し降格する。
                        Err(EvalError::Precondition(desc)) => {
                            counterexample(
                                diags,
                                format!(
                                    "ensures '{}' of '{}' throws for the seeded input ({}): {}",
                                    expr,
                                    seed.func,
                                    inputs_text(),
                                    desc
                                ),
                            );
                            downgrades.push((seed.func.clone(), Some(expr)));
                        }
                        // 評価器の範囲外 / trap はこのシードでは判定不能(寛容にスキップ)。
                        Err(_) => {}
                    }
                }
            }
            // 本体が呼び出し先の requires を満たさず実行時 throw する入力も反例。
            Err(EvalError::Precondition(desc)) => {
                counterexample(
                    diags,
                    format!(
                        "'{}' throws for the seeded input ({}): {}",
                        seed.func,
                        inputs_text(),
                        desc
                    ),
                );
                // 関数全体が throw する → その関数の ensures はどれも generative とは言えない。
                downgrades.push((seed.func.clone(), None));
            }
            // 評価不能な関数(record / Option 戻り等)は寛容にスキップ。
            Err(_) => {}
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
    fn callee_precondition_violation_is_a_counterexample() {
        // wrapper は requires が無いのに、requires を持つ positiveOnly を呼ぶ。
        // x <= 0 では実行時に positiveOnly の requires 違反で throw する → generative にせず反例。
        let m = module(
            "module t\n\nfunc positiveOnly(y: Int) -> Int\n  requires y > 0\n  ensures result == y\n{\n  return y\n}\n\nfunc wrapper(x: Int) -> Int\n  ensures result == x\n{\n  return positiveOnly(x)\n}\n",
        );
        let out = run_module(&m);
        let wrapper = out
            .iter()
            .find(|o| o.func == "wrapper")
            .expect("wrapper is analyzed");
        assert!(
            !wrapper.passed,
            "wrapper must not be generative: it throws for x <= 0"
        );
        let ce = wrapper.counterexample.as_ref().expect("counterexample");
        assert!(ce.precondition, "failure is a callee precondition throw");
        assert!(
            ce.clause.contains("positiveOnly"),
            "counterexample names the unmet precondition: {}",
            ce.clause
        );
        // positiveOnly 自身は requires を満たす入力でのみ検査され、generative。
        let pos = out
            .iter()
            .find(|o| o.func == "positiveOnly")
            .expect("positiveOnly is analyzed");
        assert!(pos.passed, "positiveOnly is correct under its requires");
    }

    #[test]
    fn callee_ensures_violation_is_a_counterexample() {
        // broken は自身の ensures(result == y)を破る(y + 1 を返す)。wrapper は broken を
        // 呼ぶだけで自分の ensures(result == x + 1)は本体結果と一致するが、実行時は broken の
        // ensures が throw する。よって wrapper は generative に上げず反例になる。
        let m = module(
            "module t\n\nfunc broken(y: Int) -> Int\n  ensures result == y\n{\n  return y + 1\n}\n\nfunc wrapper(x: Int) -> Int\n  ensures result == x + 1\n{\n  return broken(x)\n}\n",
        );
        let out = run_module(&m);
        let wrapper = out
            .iter()
            .find(|o| o.func == "wrapper")
            .expect("wrapper is analyzed");
        assert!(
            !wrapper.passed,
            "wrapper must not be generative: broken's ensures throws at runtime"
        );
        let ce = wrapper.counterexample.as_ref().expect("counterexample");
        assert!(ce.precondition, "failure is a callee contract throw");
        assert!(
            ce.clause.contains("broken"),
            "counterexample names the violated callee contract: {}",
            ce.clause
        );
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
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(diags.is_empty(), "valid seeds should be clean: {diags:?}");
    }

    #[test]
    fn requires_violating_seed_is_rejected() {
        let m = decrement_module();
        // available: 0 は requires available > 0 を満たさない。
        let src = "seeds for decrementAvailable {\n  input { available: 0 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, seed_codes::SEED_INVALID);
        assert!(diags[0].message.contains("requires"));
    }

    #[test]
    fn expected_value_in_seed_is_a_grammar_error() {
        let m = decrement_module();
        // 期待値を持たせようとすると文法エラー(捏造不能性を構造で保証)。
        let src = "seeds for decrementAvailable {\n  expected { result: 0 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
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
        let diags = check_seeds("t.seeds", src, &m, &mut []);
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
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(
            diags.is_empty(),
            "correct impl + valid seed = clean: {diags:?}"
        );
    }

    #[test]
    fn unterminated_seed_string_is_a_grammar_error() {
        // 閉じ引用符の無い文字列は文法エラー(EOF まで読んで黙って Str にしない)。
        let m = decrement_module();
        let src = "seeds for decrementAvailable {\n  input { available: \"oops }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(
            diags
                .iter()
                .any(|d| d.code == seed_codes::SEED_GRAMMAR
                    && d.message.contains("unterminated string")),
            "unterminated string must be KEI-E4006: {diags:?}"
        );
    }

    #[test]
    fn implies_short_circuits_guarded_helper() {
        // `x > 0 implies positiveCheck(x)` は前件が偽(x<=0)なら右辺を評価しない。
        // emit の `!(lhs) || rhs` と同じ短絡。x<=0 で positiveCheck の requires 違反を
        // 拾って偽反例を作らないことを、シード経路と生成経路の両方で確認する。
        let m = module(
            "module t\n\nfunc positiveCheck(y: Int) -> Bool\n  requires y > 0\n{\n  return true\n}\n\nfunc f(x: Int) -> Int\n  ensures x > 0 implies positiveCheck(x)\n{\n  return x\n}\n",
        );
        // 生成経路: 全入力(負も含む)で反例ゼロ → f は generative。
        let out = run_module(&m);
        let f = out.iter().find(|o| o.func == "f").expect("f analyzed");
        assert!(
            f.passed,
            "implies must short-circuit; no false counterexample: {:?}",
            f.counterexample
        );
        // シード経路: x = -5(前件偽)でも反例にならない。
        let src = "seeds for f {\n  input { x: -5 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(
            diags.is_empty(),
            "guarded helper under a false antecedent must not be a counterexample: {diags:?}"
        );
    }

    #[test]
    fn seed_with_throwing_ensures_is_reported() {
        // ensures 自体が throw する(節内ヘルパー positiveCheck の requires を result が破る)。
        // 実行時はその ensures チェックが throw する → シードは反例として報告される。
        let m = module(
            "module t\n\nfunc positiveCheck(y: Int) -> Bool\n  requires y > 0\n{\n  return true\n}\n\nfunc f(x: Int) -> Int\n  ensures positiveCheck(result)\n{\n  return x\n}\n",
        );
        let src = "seeds for f {\n  input { x: -5 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(
            diags
                .iter()
                .any(|d| d.code == seed_codes::SEED_COUNTEREXAMPLE && d.message.contains("throws")),
            "a seed whose ensures throws must be a counterexample: {diags:?}"
        );
    }

    #[test]
    fn unknown_seed_character_is_a_grammar_error() {
        // シードファイル中の未知の句読点は寛容に飛ばさず KEI-E4006 にする。
        let m = decrement_module();
        // 値の後ろの `!`(`input { available: 7 ! }`)。
        let src = "seeds for decrementAvailable {\n  input { available: 7 ! }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(
            diags.iter().any(|d| d.code == seed_codes::SEED_GRAMMAR),
            "stray '!' after a value must be KEI-E4006: {diags:?}"
        );
        // フィールド名に紛れた `!`(`x!: 1`)も弾く。
        let src2 = "seeds for decrementAvailable {\n  input { available!: 7 }\n}\n";
        let diags2 = check_seeds("t.seeds", src2, &m, &mut []);
        assert!(
            diags2.iter().any(|d| d.code == seed_codes::SEED_GRAMMAR),
            "stray '!' in a field name must be KEI-E4006: {diags2:?}"
        );
    }

    #[test]
    fn old_result_is_not_validated_against_post_state() {
        // `ensures old(result) == result` は、emit が本体実行前(kei$result 初期化前)に
        // old(...) をキャプチャするため実行時は未初期化参照で壊れる。評価器が post-state の
        // result を old に流し込んで generative(passed)に昇格させないことを確認する
        // (進入時環境に result は無いので old(result) は検証不能 → 対象外)。
        let m = module(
            "module t\n\nfunc f(x: Int) -> Int\n  ensures old(result) == result\n{\n  return x\n}\n",
        );
        let out = run_module(&m);
        assert!(
            !out.iter().any(|o| o.func == "f"),
            "old(result) must be unverifiable, not generative: {out:?}"
        );
        // シード経路でも偽の「合格」にならない(評価不能として寛容スキップ=反例も出さない)。
        let src = "seeds for f {\n  input { x: 3 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == seed_codes::SEED_COUNTEREXAMPLE),
            "old(result) seed must not be reported as a spurious counterexample: {diags:?}"
        );
    }

    #[test]
    fn oversized_generative_space_is_skipped() {
        // 候補は Int=11 値。Int 引数 6 個で 11^6≈177 万ケース > 上限 → 全数検査不能として
        // 対象外(generative に上げず runtime のまま)。巨大 Vec を実体化しないのでハングしない。
        let big = module(
            "module t\n\nfunc big(a: Int, b: Int, c: Int, d: Int, e: Int, g: Int) -> Int\n  ensures result == a\n{\n  return a\n}\n",
        );
        assert!(
            !run_module(&big).iter().any(|o| o.func == "big"),
            "oversized input space must be skipped, not generatively verified"
        );
        // 上限以下(2 Int = 121 ケース)は従来どおり生成検査される。
        let small = module(
            "module t\n\nfunc small(a: Int, b: Int) -> Int\n  ensures result == a\n{\n  return a\n}\n",
        );
        let out = run_module(&small);
        let o = out
            .iter()
            .find(|o| o.func == "small")
            .expect("small input space is generatively verified");
        assert!(
            o.passed && o.cases_checked > 0,
            "small must be verified: {o:?}"
        );
    }

    #[test]
    fn multiline_seed_string_aligns_following_token_line() {
        // 生の改行を含むシード文字列の後ろのトークン位置が、改行ぶん正しく送られる。
        // `!` はシードファイルの 3 行目(文字列の改行後)にあるので、診断行も 3 になる
        // (改行を col だけで数えていた頃は 2 行目にずれていた)。
        let m = module(
            "module t\n\nfunc f(s: String) -> String\n  ensures result == s\n{\n  return s\n}\n",
        );
        let src = "seeds for f {\n  input { s: \"ab\ncd\" ! }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        let stray = diags
            .iter()
            .find(|d| d.code == seed_codes::SEED_GRAMMAR)
            .expect("stray '!' after a multi-line string is a grammar error");
        assert_eq!(
            stray.span.start.line, 3,
            "diagnostic must sit on the post-newline line: {stray:?}"
        );
    }

    #[test]
    fn malformed_seed_integer_is_a_grammar_error() {
        // `-`(数字なし)は黙って 0 にせず文法エラー。
        let m = decrement_module();
        let src = "seeds for decrementAvailable {\n  input { available: - }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(
            diags
                .iter()
                .any(|d| d.code == seed_codes::SEED_GRAMMAR
                    && d.message.contains("malformed integer")),
            "malformed integer must be KEI-E4006: {diags:?}"
        );
    }

    #[test]
    fn seed_counterexample_downgrades_generative_contract() {
        use crate::report::{ContractInfo, ContractKind, Verification};
        // 壊れた実装。生成器は固定ドメイン(1,2,3,10,100…)で反例を出すが、ここでは
        // 「もし generative に上がっていたら」をシミュレートして降格を確認する。
        let m = module(
            "module t\n\nfunc decrementAvailable(available: Int) -> Int\n  requires available > 0\n  ensures result == old(available) - 1\n{\n  return available - 2\n}\n",
        );
        let mut contracts = vec![ContractInfo {
            func: "decrementAvailable".to_string(),
            kind: ContractKind::Ensures,
            expr: "result == old(available) - 1".to_string(),
            verification: Verification::Generative,
            span: crate::Span {
                file: "t.kei".to_string(),
                start: crate::Position { line: 1, col: 1 },
                end: crate::Position { line: 1, col: 2 },
            },
        }];
        let src = "seeds for decrementAvailable {\n  input { available: 7 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut contracts);
        assert!(diags
            .iter()
            .any(|d| d.code == seed_codes::SEED_COUNTEREXAMPLE));
        assert_eq!(
            contracts[0].verification,
            Verification::Runtime,
            "a seed that breaks ensures must downgrade generative → runtime"
        );
    }

    #[test]
    fn seed_for_unknown_function_is_rejected() {
        let m = decrement_module();
        let src = "seeds for nope {\n  input { available: 1 }\n}\n";
        let diags = check_seeds("t.seeds", src, &m, &mut []);
        assert!(diags
            .iter()
            .any(|d| d.code == seed_codes::SEED_INVALID && d.message.contains("unknown function")));
    }
}
