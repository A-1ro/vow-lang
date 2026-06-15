//! 4 ツールの中身(spec / check / fmt / examples)。
//!
//! 言語処理は kei_check / kei_fmt / kei_syntax に委譲し、ここでは
//! 埋め込み spec/examples の取り回しと検査結果の整形だけを行う
//! (ARCHITECTURE.md: kei_mcp は言語処理ロジックを持たない)。

use crate::embedded;

/// インライン source に与える合成ファイル名。Diagnostic の span.file に入る。
pub const SYNTHETIC_FILE: &str = "source.kei";

const MAIN_SPEC: &str = "kei-spec-v0.1.md";
const DIAGNOSTIC_SCHEMA: &str = "diagnostic-schema.md";

/// 1 ツール呼び出しの結果。`is_error` は MCP の `isError` に対応する。
pub struct ToolOutcome {
    pub text: String,
    pub is_error: bool,
}

impl ToolOutcome {
    fn ok(text: impl Into<String>) -> Self {
        ToolOutcome {
            text: text.into(),
            is_error: false,
        }
    }

    fn err(text: impl Into<String>) -> Self {
        ToolOutcome {
            text: text.into(),
            is_error: true,
        }
    }
}

/// 必須引数欠落。tools/call の引数バリデーション失敗時に使う。
pub fn missing_arg(name: &str) -> ToolOutcome {
    ToolOutcome::err(format!("Missing required argument '{name}'."))
}

/// 未知のツール名。
pub fn unknown_tool(name: &str) -> ToolOutcome {
    ToolOutcome::err(format!("Unknown tool '{name}'."))
}

// ---- kei_spec ----

/// 仕様セクション・エラーコード解説の即引き。
///
/// - 空 topic … 索引(セクション番号・追加ドラフト・エラーコード一覧)
/// - `KEI-Exxxx` … `spec/errors/{code}.md`
/// - `"diagnostic"` を含む … diagnostic スキーマ
/// - 本文のレベル2セクション … 番号・見出しキーワードで照合(MAIN_SPEC)
/// - 追加仕様ドラフト … MAIN_SPEC 以外の spec/*.md をファイル名 stem のキーワードで照合
///   (例: topic `"collections"` → `kei-spec-v0.3-collections.md` 全文)
pub fn run_spec(topic: &str) -> ToolOutcome {
    let t = topic.trim();
    if t.is_empty() {
        return ToolOutcome::ok(spec_index());
    }

    let upper = t.to_uppercase();
    if upper.starts_with("KEI-E") {
        let rel = format!("errors/{upper}.md");
        return match embedded::spec_file(&rel) {
            Some(content) => ToolOutcome::ok(content),
            None => ToolOutcome::err(format!("Unknown error code '{t}'.\n\n{}", spec_index())),
        };
    }

    let lower = t.to_lowercase();
    if lower.contains("diagnostic") {
        if let Some(content) = embedded::spec_file(DIAGNOSTIC_SCHEMA) {
            return ToolOutcome::ok(content);
        }
    }

    let needle = normalize_topic(&lower);

    if let Some(main) = embedded::spec_file(MAIN_SPEC) {
        for (num, title, body) in sections(main) {
            if needle == num || title.to_lowercase().contains(&needle) {
                return ToolOutcome::ok(body);
            }
        }
    }

    // MAIN_SPEC 以外の追加ドラフト(v0.3 collections 等)はファイル名 stem で引く。
    // 短い数値 topic がセクション番号と取り違えられないよう 3 文字以上に限る。
    if needle.len() >= 3 {
        for (stem, content) in extra_spec_docs() {
            if stem.to_lowercase().contains(needle.as_str()) {
                return ToolOutcome::ok(content.to_string());
            }
        }
    }

    ToolOutcome::err(format!(
        "No spec section matched '{t}'.\n\n{}",
        spec_index()
    ))
}

/// MAIN_SPEC・diagnostic-schema・errors/ 以外の spec ドキュメントを
/// `(ファイル名 stem, 内容)` で返す。build.rs が spec/**/*.md を全て埋め込むため、
/// 追加ドラフト(例: kei-spec-v0.3-collections.md)は自動で kei_spec の経路に乗る。
fn extra_spec_docs() -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    for &(rel, content) in embedded::spec_files() {
        if rel == MAIN_SPEC || rel == DIAGNOSTIC_SCHEMA || rel.starts_with("errors/") {
            continue;
        }
        if let Some(stem) = rel.strip_suffix(".md") {
            out.push((stem, content));
        }
    }
    out
}

/// topic を見出し照合用に正規化する(先頭の `§`/`#` と前後空白、末尾の `.` を除去)。
fn normalize_topic(lower: &str) -> String {
    lower
        .trim()
        .trim_start_matches(['§', '#'])
        .trim()
        .trim_end_matches('.')
        .to_string()
}

/// 本文をレベル2見出し(`## N. Title`)で `(番号, 見出し, セクション本文)` に分割する。
/// レベル3(`### ...`)以降は直近レベル2セクションの本文に含める。
fn sections(spec: &str) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    let mut current: Option<(String, String, Vec<&str>)> = None;
    for line in spec.lines() {
        if let Some((num, title)) = level2_heading(line) {
            if let Some((n, t, body)) = current.take() {
                out.push((n, t, body.join("\n")));
            }
            current = Some((num, title, vec![line]));
        } else if let Some((_, _, body)) = current.as_mut() {
            body.push(line);
        }
    }
    if let Some((n, t, body)) = current.take() {
        out.push((n, t, body.join("\n")));
    }
    out
}

/// `## N. Title` 形式の行から `(N, Title)` を取り出す。レベル3以降は None。
fn level2_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## ")?;
    let (num, title) = rest.split_once(". ")?;
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((num.to_string(), title.to_string()))
}

fn spec_index() -> String {
    let mut s = String::from("# Kei spec index\n\n");
    s.push_str(
        "`kei_spec` の `topic` にセクション番号・見出しキーワード・エラーコードを渡すと\n\
         該当箇所を返す。topic を空にするとこの索引を返す。\n\n",
    );
    s.push_str("## セクション (kei-spec-v0.1.md)\n\n");
    if let Some(main) = embedded::spec_file(MAIN_SPEC) {
        for (num, title, _) in sections(main) {
            s.push_str(&format!("- {num}. {title}\n"));
        }
    }
    let extra = extra_spec_docs();
    if !extra.is_empty() {
        s.push_str("\n## 追加仕様ドラフト (spec/)\n\n");
        for (stem, _) in &extra {
            let topic = stem.rsplit('-').next().unwrap_or(stem);
            s.push_str(&format!("- {stem}.md (topic: \"{topic}\")\n"));
        }
    }
    s.push_str("\n## Diagnostic スキーマ\n\n- diagnostic-schema (topic: \"diagnostic\")\n");
    s.push_str("\n## エラーコード (spec/errors/)\n\n");
    for (rel, _) in embedded::spec_files() {
        if let Some(code) = rel
            .strip_prefix("errors/")
            .and_then(|r| r.strip_suffix(".md"))
        {
            s.push_str(&format!("- {code}\n"));
        }
    }
    s
}

// ---- kei_check ----

/// 構文+意味検査。Diagnostic[] の整形 JSON を返す(検査成功は is_error=false。
/// Diagnostic はエラーではなくデータとして返す)。
pub fn run_check(source: &str) -> ToolOutcome {
    let parsed = kei_syntax::parse_module(source);
    let mut diags = kei_check::syntax_diagnostics(SYNTHETIC_FILE, &parsed.errors);
    // 構文エラーがあるときは壊れた AST に意味検査をかけない(golden_check と同方針)。
    if parsed.errors.is_empty() {
        diags.extend(kei_check::check_module(SYNTHETIC_FILE, &parsed.module));
    }
    ToolOutcome::ok(diagnostics_json(&diags))
}

// ---- kei_fmt ----

/// 正規形整形。構文エラーがある入力は整形せず Diagnostic[] を is_error で返す
/// (壊れた入力を「それらしく」書き換えない — kei_fmt の方針)。
pub fn run_fmt(source: &str) -> ToolOutcome {
    match kei_fmt::format_source(source) {
        Ok(formatted) => ToolOutcome::ok(formatted),
        Err(errors) => {
            let diags = kei_check::syntax_diagnostics(SYNTHETIC_FILE, &errors);
            ToolOutcome::err(diagnostics_json(&diags))
        }
    }
}

fn diagnostics_json(diags: &[kei_check::Diagnostic]) -> String {
    serde_json::to_string_pretty(diags).unwrap_or_else(|_| "[]".to_string())
}

// ---- kei_examples ----

/// イディオム検索。query をパス・本文に部分一致(大小無視)で照合し、
/// 一致した例を返す。空 query は一覧を返す。
pub fn run_examples(query: &str) -> ToolOutcome {
    let files = embedded::example_files();
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return ToolOutcome::ok(examples_index(files));
    }

    let mut out = String::new();
    for (path, content) in files {
        if path.to_lowercase().contains(&q) || content.to_lowercase().contains(&q) {
            out.push_str(&format!(
                "### examples/{path}\n\n```kei\n{}\n```\n\n",
                content.trim_end()
            ));
        }
    }

    if out.is_empty() {
        ToolOutcome::err(format!(
            "No example matched '{query}'.\n\n{}",
            examples_index(files)
        ))
    } else {
        ToolOutcome::ok(out.trim_end().to_string())
    }
}

fn examples_index(files: &[(&str, &str)]) -> String {
    let mut s = String::from("# Kei examples\n\n");
    s.push_str(
        "`kei_examples` の `query` にキーワードを渡すと該当する例を返す\n\
         (パス・本文を部分一致で検索)。query を空にするとこの一覧を返す。\n\n",
    );
    for (path, _) in files {
        s.push_str(&format!("- examples/{path}\n"));
    }
    s
}
