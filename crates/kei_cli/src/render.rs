//! Diagnostic の散文整形と `fmt --check` の差分整形。
//!
//! 散文整形は CLI に許された唯一の「ロジック」(ARCHITECTURE.md kei_cli 行・
//! M6 事前合意)。構造化 Diagnostic が正で、ここはその派生表示にすぎない。
//! Diagnostic の全要素(severity / code / message / span / fix)を欠落なく描く。
//!
//! 形式は rustc 風:
//! ```text
//! error[KEI-E3001]: <message>
//!   --> <file>:<line>:<col>
//!    |
//! 10 | <ソース行>
//!    |          ^^^^^^^^^^^^
//!    = fix: <fix title>
//! ```

use kei_check::{ContractInfo, Diagnostic, Severity};

/// 契約の検証レベル要約(散文)。`--json` の `contracts` の派生表示。
/// 契約が無ければ空文字列。各行は `<func> <kind> <expr>  [<level>]`。
pub fn contracts(infos: &[ContractInfo]) -> String {
    if infos.is_empty() {
        return String::new();
    }
    let mut out = String::from("verification:\n");
    for c in infos {
        out.push_str(&format!(
            "  {} {} {}  [{}]\n",
            c.func,
            c.kind.as_str(),
            c.expr,
            c.verification.as_str()
        ));
    }
    out
}

/// Diagnostic 列を散文に整形する。`source` はキャレット下線のためのソース全文。
/// 各 Diagnostic ブロックは空行で区切り、全体は改行で終わる。空列は空文字列。
pub fn diagnostics(source: &str, diags: &[Diagnostic]) -> String {
    if diags.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let blocks: Vec<String> = diags.iter().map(|d| render_one(&lines, d)).collect();
    let mut out = blocks.join("\n");
    out.push('\n');
    out
}

fn render_one(lines: &[&str], d: &Diagnostic) -> String {
    let mut out = String::new();
    let severity = severity_word(d.severity);
    out.push_str(&format!("{severity}[{}]: {}\n", d.code, d.message));

    let start = &d.span.start;
    let gutter = start.line.to_string().len();
    let pad = " ".repeat(gutter);

    out.push_str(&format!(
        "{pad}--> {}:{}:{}\n",
        d.span.file, start.line, start.col
    ));
    out.push_str(&format!("{pad} |\n"));

    if let Some(src) = lines.get((start.line - 1) as usize) {
        out.push_str(&format!("{:>gutter$} | {src}\n", start.line));
        let lead = " ".repeat((start.col.saturating_sub(1)) as usize);
        let carets = "^".repeat(caret_len(d, src));
        out.push_str(&format!("{pad} | {lead}{carets}\n"));
    }

    for fix in &d.fixes {
        out.push_str(&format!("{pad} = fix: {}\n", fix.title));
    }
    out
}

fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// キャレットの長さ(Unicode スカラー値単位)。最低 1。span が複数行にまたがる
/// ときは開始行の末尾までを下線にする(下線は開始行のみに描く)。
fn caret_len(d: &Diagnostic, src: &str) -> usize {
    let (start, end) = (&d.span.start, &d.span.end);
    let len = if start.line == end.line {
        end.col.saturating_sub(start.col)
    } else {
        (src.chars().count() as u32).saturating_sub(start.col.saturating_sub(1))
    };
    (len as usize).max(1)
}

/// `fmt --check` が未整形を検出したときの差分表示。
/// 現状(`current`)から正規形(`formatted`)への行単位 unified diff
/// (`-` 行を消し `+` 行を足すと正規形になる)。
pub fn format_diff(file: &str, current: &str, formatted: &str) -> String {
    let mut out = format!("{file}: not formatted (run `kei fmt --write {file}`)\n");
    out.push_str(&line_diff(current, formatted));
    out
}

/// LCS による行単位 diff。`a` 由来のみ `-`、`b` 由来のみ `+`、共通は ` `。
fn line_diff(a: &str, b: &str) -> String {
    let a: Vec<&str> = a.lines().collect();
    let b: Vec<&str> = b.lines().collect();
    let (n, m) = (a.len(), b.len());

    // lcs[i][j] = a[i..] と b[j..] の最長共通部分列の長さ。
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out = String::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push_str(&format!(" {}\n", a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push_str(&format!("-{}\n", a[i]));
            i += 1;
        } else {
            out.push_str(&format!("+{}\n", b[j]));
            j += 1;
        }
    }
    for line in &a[i..] {
        out.push_str(&format!("-{line}\n"));
    }
    for line in &b[j..] {
        out.push_str(&format!("+{line}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kei_check::{Fix, Position, Span, TextEdit};

    fn diag() -> Diagnostic {
        Diagnostic::new(
            Severity::Error,
            "KEI-E3001",
            "effect 'Database.Write' used but not declared in 'uses' clause",
            Span {
                file: "transfer.kei".to_string(),
                start: Position { line: 10, col: 10 },
                end: Position { line: 10, col: 22 },
            },
            vec![Fix {
                title: "Add 'uses Database.Write' to 'save'".to_string(),
                edits: vec![TextEdit {
                    span: Span {
                        file: "transfer.kei".to_string(),
                        start: Position { line: 10, col: 10 },
                        end: Position { line: 10, col: 10 },
                    },
                    new_text: String::new(),
                }],
            }],
        )
        .unwrap()
    }

    #[test]
    fn renders_all_diagnostic_elements() {
        let source = "module m\n\nfunc save(id: Int) -> Bool {\n  return writeRow(id)\n}\n\
                      \n\n\n\n  return writeRow(id)\n";
        let out = diagnostics(source, &[diag()]);
        assert!(out.contains("error[KEI-E3001]: effect 'Database.Write'"));
        assert!(out.contains("--> transfer.kei:10:10"));
        assert!(out.contains("10 |"));
        assert!(out.contains("^^^^^^^^^^^^"));
        assert!(out.contains("= fix: Add 'uses Database.Write' to 'save'"));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn empty_when_no_diagnostics() {
        assert_eq!(diagnostics("anything", &[]), "");
    }

    #[test]
    fn caret_is_at_least_one() {
        let mut d = diag();
        d.span.end = d.span.start;
        let out = diagnostics("x\n".repeat(12).as_str(), &[d]);
        assert!(out.contains(" ^\n"));
    }

    #[test]
    fn diff_marks_added_and_removed_lines() {
        let diff = line_diff("let x = (a)\n", "let x = a\n");
        assert!(diff.contains("-let x = (a)"));
        assert!(diff.contains("+let x = a"));
    }
}
