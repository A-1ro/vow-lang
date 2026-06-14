//! ソース文字列を入力に取り、LSP 機能の素となる結果を返す純関数群。
//!
//! 言語処理は kei_syntax(パース)/ kei_check(意味検査)へ委譲し、ここでは
//! その結果を LSP 機能向けに取り回すだけ(ARCHITECTURE.md: kei_lsp は
//! 言語処理ロジックを持たない)。サーバーループ(I/O)から独立した純関数に
//! することで、プロセス起動なしに単体テストできる。

use kei_check::Diagnostic as KeiDiagnostic;
use kei_syntax::ast;
use kei_syntax::span::{Position as SynPosition, Span as SynSpan};

/// 合成ファイル名。Diagnostic の span.file に入る(LSP では URI 側を使うため表示には出ない)。
pub const SYNTHETIC_FILE: &str = "source.kei";

/// 構文 + 意味検査を実行し、kei の構造化 Diagnostic を返す。
///
/// `kei check` / `kei_mcp::tools::run_check` と同方針: 構文エラーがあるときは
/// 壊れた AST に意味検査をかけず、構文 Diagnostic だけを返す。
pub fn compute_diagnostics(source: &str) -> Vec<KeiDiagnostic> {
    let parsed = kei_syntax::parse_module(source);
    let mut diags = kei_check::syntax_diagnostics(SYNTHETIC_FILE, &parsed.errors);
    if parsed.errors.is_empty() {
        diags.extend(kei_check::check_module(SYNTHETIC_FILE, &parsed.module));
    }
    diags
}

/// LSP の 0 始まり位置。`character` は Unicode スカラー値単位(convert と同じ近似)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub line: u32,
    pub character: u32,
}

/// カーソル位置の関数宣言からホバー用 Markdown を組み立てる。
///
/// 関数名の上にカーソルがあるとき、その関数の契約(`uses` / `requires` /
/// `ensures`)と署名を「合意書」として表示する。契約式は再フォーマットせず
/// ソースの該当 span をそのまま抜き出す(kei_fmt の正規形を壊さないため)。
///
/// 該当する関数がなければ `None`(LSP は null hover を返す)。
pub fn hover_markdown(source: &str, cursor: CursorPosition) -> Option<String> {
    let parsed = kei_syntax::parse_module(source);
    let func = find_func_at(&parsed.module, cursor)?;
    Some(render_func_contract(source, func))
}

/// カーソル(0 始まり)がいずれかの関数の **名前** span 上にあればその関数を返す。
fn find_func_at(module: &ast::Module, cursor: CursorPosition) -> Option<&ast::FuncDecl> {
    module.items.iter().find_map(|item| match item {
        ast::Item::Func(f) if span_contains(f.name.span, cursor) => Some(f),
        _ => None,
    })
}

/// 0 始まりカーソルが、1 始まり・end 排他の span に含まれるか。
fn span_contains(span: SynSpan, cursor: CursorPosition) -> bool {
    let pos = SynPosition::new(cursor.line + 1, cursor.character + 1);
    let after_start = (pos.line, pos.col) >= (span.start.line, span.start.col);
    let before_end = (pos.line, pos.col) < (span.end.line, span.end.col);
    after_start && before_end
}

/// 関数宣言を契約付き署名の Markdown に整形する。
fn render_func_contract(source: &str, func: &ast::FuncDecl) -> String {
    let mut header = format!("func {}(", func.name.name);
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.name, slice_span(source, p.ty.span)))
        .collect();
    header.push_str(&params.join(", "));
    header.push(')');
    if let Some(ret) = &func.ret {
        header.push_str(" -> ");
        header.push_str(&slice_span(source, ret.span));
    }

    let mut code = header;
    if !func.uses.is_empty() {
        let effects: Vec<String> = func
            .uses
            .iter()
            .map(|e| {
                e.path
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect();
        code.push_str(&format!("\n  uses {}", effects.join(", ")));
    }
    for clause in &func.requires {
        code.push_str(&format!(
            "\n  requires {}",
            slice_span(source, clause.span())
        ));
    }
    for clause in &func.ensures {
        code.push_str(&format!(
            "\n  ensures {}",
            slice_span(source, clause.span())
        ));
    }

    let mut md = format!("```kei\n{code}\n```");
    // 契約の意味づけ(spec §3 エフェクト / §4 契約)を散文で添える。
    let mut notes: Vec<&str> = Vec::new();
    if !func.uses.is_empty() {
        notes.push("**uses** — このコードが行ってよい副作用(ケーパビリティ)。宣言外のエフェクトはコンパイルエラー。");
    }
    if !func.requires.is_empty() {
        notes.push("**requires** — 呼び出し側が満たすべき事前条件(実行時アサーション)。");
    }
    if !func.ensures.is_empty() {
        notes.push("**ensures** — この関数が保証する事後条件(`result` で戻り値を参照)。");
    }
    if !notes.is_empty() {
        md.push_str("\n\n");
        md.push_str(&notes.join("\n\n"));
    }
    md
}

/// 1 始まり span に対応するソース部分文字列を取り出す(複数行は改行込み)。
fn slice_span(source: &str, span: SynSpan) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let (sl, sc) = (span.start.line as usize, span.start.col as usize);
    let (el, ec) = (span.end.line as usize, span.end.col as usize);
    if sl == 0 || sl > lines.len() {
        return String::new();
    }
    if sl == el {
        return char_slice(lines[sl - 1], sc - 1, ec - 1);
    }
    let mut out = String::new();
    // 最初の行: start 列から行末まで。
    out.push_str(&char_slice(lines[sl - 1], sc - 1, usize::MAX));
    for line in lines
        .iter()
        .take(el.min(lines.len()).saturating_sub(1))
        .skip(sl)
    {
        out.push('\n');
        out.push_str(line);
    }
    // 最終行: 先頭から end 列まで。
    if el <= lines.len() {
        out.push('\n');
        out.push_str(&char_slice(lines[el - 1], 0, ec - 1));
    }
    out
}

/// Unicode スカラー値単位で `[start, end)` を切り出す(列指定が span と同じ単位)。
fn char_slice(line: &str, start: usize, end: usize) -> String {
    line.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // self-contained: import なしでも検査が通るよう Int だけで構成する。
    // 契約(uses/requires/ensures)はホバー表示の確認用。
    const WITHDRAW: &str = r#"module contracts.withdraw

func withdraw(amount: Int) -> Int
  uses Database.Read, Database.Write
  requires amount > 0
  ensures result >= 0
{
  return amount
}
"#;

    /// `withdraw` の関数名(3 行目)の上でホバーすると契約が出る。
    fn cursor_on_withdraw_name() -> CursorPosition {
        // 0 始まり: line 2 (3 行目), `func ` の後ろ、'w' のあたり。
        CursorPosition {
            line: 2,
            character: 8,
        }
    }

    #[test]
    fn hover_shows_contract() {
        let md = hover_markdown(WITHDRAW, cursor_on_withdraw_name()).expect("hover on func name");
        assert!(md.contains("func withdraw("), "signature: {md}");
        assert!(
            md.contains("uses Database.Read, Database.Write"),
            "uses: {md}"
        );
        assert!(md.contains("requires amount > 0"), "requires: {md}");
        assert!(md.contains("ensures result >= 0"), "ensures: {md}");
        assert!(md.contains("**uses**"), "uses note: {md}");
        assert!(md.contains("**requires**"), "requires note: {md}");
        assert!(md.contains("**ensures**"), "ensures note: {md}");
    }

    #[test]
    fn hover_off_function_name_is_none() {
        // 1 行目 (module 宣言) の上にはホバー対象がない。
        let none = hover_markdown(
            WITHDRAW,
            CursorPosition {
                line: 0,
                character: 0,
            },
        );
        assert!(none.is_none());
    }

    #[test]
    fn compute_diagnostics_flags_undeclared_effect() {
        // ローカル関数 writeRow が Database.Write を使うのに、呼び出し元 save は
        // uses 宣言を持たない → エフェクト検査エラー(推移的伝播。spec §3.1)。
        let src = r#"module m

func writeRow(id: Int) -> Bool
  uses Database.Write
{
  return true
}

func save(id: Int) -> Bool {
  return writeRow(id)
}
"#;
        let diags = compute_diagnostics(src);
        assert!(
            diags.iter().any(|d| d.code.starts_with("KEI-E3")),
            "expected an effect diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn compute_diagnostics_clean_source_is_empty() {
        // WITHDRAW は Int だけで構成され、エフェクト・契約・型のいずれも違反しない。
        let diags = compute_diagnostics(WITHDRAW);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }
}
