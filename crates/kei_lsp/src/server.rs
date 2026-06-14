//! lsp-server による同期 stdio サーバーループ。
//!
//! tower-lsp(tokio/async)ではなく lsp-server + lsp-types(同期・rust-analyzer
//! と同系統)を採用している。理由は ARCHITECTURE.md / CLAUDE.md の「薄いアダプタ・
//! 最小依存」方針: 本サーバーの実処理は同期関数 `kei_check::check_module` の呼び出し
//! だけで、非同期ランタイム(tokio)を持ち込む必要がない。kei_mcp が serde_json だけで
//! JSON-RPC を手回ししているのと同じ精神で、ここも同期ブロッキングループに留める。
//!
//! 対応機能(v0.1 最小):
//! - `initialize` — textDocumentSync=FULL, hoverProvider を広告。
//! - `textDocument/didOpen` / `didChange` — 全文を受け取り再検査し、
//!   `textDocument/publishDiagnostics` を返す。
//! - `textDocument/didClose` — 文書を破棄し、診断をクリアする。
//! - `textDocument/hover` — 関数名上で契約(uses/requires/ensures)を表示。

use std::collections::HashMap;
use std::error::Error;

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
    Notification as NotificationTrait, PublishDiagnostics,
};
use lsp_types::request::{HoverRequest, Request as RequestTrait};
use lsp_types::{
    Hover, HoverContents, HoverParams, HoverProviderCapability, MarkupContent, MarkupKind,
    PublishDiagnosticsParams, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    Uri,
};

use crate::analysis::{self, CursorPosition};
use crate::convert::diagnostic_to_lsp;

/// 開いている文書の最新テキスト(URI 文字列 → ソース全文)。
///
/// キーは `Uri` ではなく文字列表現にする。`lsp_types::Uri`(fluent-uri)は
/// 内部にパースキャッシュ用の `Cell` を持つため、`HashMap` のキーに使うと
/// clippy の `mutable_key_type` に当たる(意味的には不変だが)。文字列キーは
/// その回避であり、URI の同一性は文字列一致で十分。
type DocumentStore = HashMap<String, String>;

/// stdio で言語サーバーを起動し、クライアントが切断するまで処理する。
pub fn run_stdio() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(server_capabilities())?;
    let _init_params = connection.initialize(capabilities)?;
    main_loop(&connection)?;
    io_threads.join()?;
    Ok(())
}

/// 本サーバーが広告するケーパビリティ。
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        // 全文同期: 編集のたびに本文全体を受け取り再検査する(差分は v0.1 では扱わない)。
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    }
}

/// initialize 完了後のメインループ。`Connection::memory()` でも駆動できるよう
/// `Connection` だけに依存する。
pub fn main_loop(connection: &Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut docs: DocumentStore = HashMap::new();
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                if let Some(resp) = handle_request(req, &docs) {
                    connection.sender.send(Message::Response(resp))?;
                }
            }
            Message::Notification(note) => {
                for publish in handle_notification(note, &mut docs) {
                    connection.sender.send(Message::Notification(publish))?;
                }
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

/// リクエストを処理し、応答が必要なら返す。未対応メソッドは握りつぶす
/// (lsp-server は未応答 id をエラーにしないため、ここでは None を返す)。
fn handle_request(req: Request, docs: &DocumentStore) -> Option<Response> {
    match req.method.as_str() {
        HoverRequest::METHOD => {
            let (id, params) = cast_request::<HoverRequest>(req)?;
            // ストアにある本文に対してホバーを解決する。未知 URI は null hover。
            let key = params
                .text_document_position_params
                .text_document
                .uri
                .as_str();
            let source = docs.get(key).map(String::as_str).unwrap_or("");
            Some(hover_response_with_source(id, &params, source))
        }
        _ => None,
    }
}

/// 通知を処理し、送り返す publishDiagnostics 通知(0/1 件)を返す。
fn handle_notification(note: Notification, docs: &mut DocumentStore) -> Vec<Notification> {
    match note.method.as_str() {
        DidOpenTextDocument::METHOD => {
            if let Ok(params) = cast_notification::<DidOpenTextDocument>(note) {
                let uri = params.text_document.uri;
                let text = params.text_document.text;
                docs.insert(uri.as_str().to_string(), text.clone());
                return vec![publish_for(&uri, &text)];
            }
        }
        DidChangeTextDocument::METHOD => {
            if let Ok(params) = cast_notification::<DidChangeTextDocument>(note) {
                // FULL 同期なので最後の content change が全文。
                if let Some(change) = params.content_changes.into_iter().next_back() {
                    let uri = params.text_document.uri;
                    docs.insert(uri.as_str().to_string(), change.text.clone());
                    return vec![publish_for(&uri, &change.text)];
                }
            }
        }
        DidCloseTextDocument::METHOD => {
            if let Ok(params) = cast_notification::<DidCloseTextDocument>(note) {
                let uri = params.text_document.uri;
                docs.remove(uri.as_str());
                // 閉じた文書の診断はクリアする(空配列を publish)。
                return vec![publish_clear(&uri)];
            }
        }
        _ => {}
    }
    Vec::new()
}

/// ソースを検査し、URI 向けの publishDiagnostics 通知を作る。
fn publish_for(uri: &Uri, source: &str) -> Notification {
    let diags = analysis::compute_diagnostics(source);
    let lsp_diags = diags.iter().map(diagnostic_to_lsp).collect();
    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics: lsp_diags,
        version: None,
    };
    Notification::new(PublishDiagnostics::METHOD.to_string(), params)
}

/// 診断クリア(空配列)の publishDiagnostics 通知。
fn publish_clear(uri: &Uri) -> Notification {
    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics: Vec::new(),
        version: None,
    };
    Notification::new(PublishDiagnostics::METHOD.to_string(), params)
}

/// ストアの本文を使ってホバー応答を作る(本文がない / 該当関数がなければ null)。
pub fn hover_response_with_source(id: RequestId, params: &HoverParams, source: &str) -> Response {
    let pos = params.text_document_position_params.position;
    let cursor = CursorPosition {
        line: pos.line,
        character: pos.character,
    };
    match analysis::hover_markdown(source, cursor) {
        Some(md) => {
            let hover = Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: md,
                }),
                range: None,
            };
            Response::new_ok(id, hover)
        }
        None => Response::new_ok(id, serde_json::Value::Null),
    }
}

fn cast_request<R>(req: Request) -> Option<(RequestId, R::Params)>
where
    R: RequestTrait,
    R::Params: serde::de::DeserializeOwned,
{
    match req.extract::<R::Params>(R::METHOD) {
        Ok(value) => Some(value),
        Err(ExtractError::MethodMismatch(_)) => None,
        Err(ExtractError::JsonError { .. }) => None,
    }
}

fn cast_notification<N>(note: Notification) -> Result<N::Params, ExtractError<Notification>>
where
    N: NotificationTrait,
    N::Params: serde::de::DeserializeOwned,
{
    note.extract::<N::Params>(N::METHOD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{
        DidOpenTextDocumentParams, Position, TextDocumentIdentifier, TextDocumentItem,
        TextDocumentPositionParams, WorkDoneProgressParams,
    };

    fn uri() -> Uri {
        use std::str::FromStr;
        Uri::from_str("file:///tmp/a.kei").unwrap()
    }

    #[test]
    fn did_open_clean_source_publishes_empty_diagnostics() {
        let mut docs = DocumentStore::new();
        let note = Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri(),
                    language_id: "kei".to_string(),
                    version: 1,
                    text: "module m\n\nfunc f() -> Int {\n  return 0\n}\n".to_string(),
                },
            },
        );
        let out = handle_notification(note, &mut docs);
        assert_eq!(out.len(), 1);
        let params: PublishDiagnosticsParams =
            serde_json::from_value(out[0].params.clone()).unwrap();
        assert_eq!(params.uri, uri());
        assert!(params.diagnostics.is_empty(), "{:?}", params.diagnostics);
        assert!(docs.contains_key(uri().as_str()));
    }

    #[test]
    fn did_open_effect_error_publishes_diagnostic() {
        let mut docs = DocumentStore::new();
        // ローカル呼び出しで宣言外エフェクトが推移的に漏れるパターン。
        let src = "module m\n\nfunc writeRow(id: Int) -> Bool\n  uses Database.Write\n{\n  return true\n}\n\nfunc save(id: Int) -> Bool {\n  return writeRow(id)\n}\n";
        let note = Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri(),
                    language_id: "kei".to_string(),
                    version: 1,
                    text: src.to_string(),
                },
            },
        );
        let out = handle_notification(note, &mut docs);
        let params: PublishDiagnosticsParams =
            serde_json::from_value(out[0].params.clone()).unwrap();
        assert!(
            params
                .diagnostics
                .iter()
                .any(|d| matches!(&d.code, Some(lsp_types::NumberOrString::String(c)) if c.starts_with("KEI-E3"))),
            "expected effect diagnostic, got {:?}",
            params.diagnostics
        );
    }

    #[test]
    fn did_close_clears_diagnostics() {
        let mut docs = DocumentStore::new();
        docs.insert(uri().as_str().to_string(), "module m\n".to_string());
        let note = Notification::new(
            DidCloseTextDocument::METHOD.to_string(),
            lsp_types::DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri() },
            },
        );
        let out = handle_notification(note, &mut docs);
        assert_eq!(out.len(), 1);
        let params: PublishDiagnosticsParams =
            serde_json::from_value(out[0].params.clone()).unwrap();
        assert!(params.diagnostics.is_empty());
        assert!(!docs.contains_key(uri().as_str()));
    }

    #[test]
    fn hover_on_function_name_returns_contract() {
        let src = "module m\n\nfunc withdraw(amount: Money) -> Int\n  requires amount > 0\n{\n  return 0\n}\n";
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri() },
                // 3 行目(0 始まり line=2)の "withdraw" の上。
                position: Position {
                    line: 2,
                    character: 8,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        let resp = hover_response_with_source(RequestId::from(1), &params, src);
        let value = resp.result.expect("hover result");
        let hover: Hover = serde_json::from_value(value).expect("non-null hover");
        match hover.contents {
            HoverContents::Markup(m) => {
                assert!(m.value.contains("func withdraw("), "{}", m.value);
                assert!(m.value.contains("requires amount > 0"), "{}", m.value);
            }
            other => panic!("expected markup, got {other:?}"),
        }
    }
}
