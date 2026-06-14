//! `kei_lsp::server::main_loop` を `Connection::memory()` で端から端まで駆動する
//! 統合テスト。プロトコルの実フロー(didOpen → publishDiagnostics、hover、
//! shutdown/exit)を実バイナリ起動なしで検証する。
//!
//! 手動確認(initialize → didOpen → publishDiagnostics が返るか)の自動化版。

use std::str::FromStr;
use std::thread;

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::{
    DidOpenTextDocument, Notification as NotificationTrait, PublishDiagnostics,
};
use lsp_types::request::{HoverRequest, Request as RequestTrait};
use lsp_types::{
    DidOpenTextDocumentParams, Hover, HoverContents, HoverParams, NumberOrString, Position,
    PublishDiagnosticsParams, TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams,
    Uri, WorkDoneProgressParams,
};

const SRC: &str = "module m\n\nfunc writeRow(id: Int) -> Bool\n  uses Database.Write\n{\n  return true\n}\n\nfunc save(id: Int) -> Bool {\n  return writeRow(id)\n}\n";

fn uri() -> Uri {
    Uri::from_str("file:///tmp/loop.kei").unwrap()
}

#[test]
fn full_flow_open_hover_shutdown() {
    let (server, client) = Connection::memory();

    // サーバーループを別スレッドで回す(main_loop は initialize 後の状態を前提とするが
    // initialize 自体には依存しないため、memory transport ではそのまま起動できる)。
    let handle = thread::spawn(move || {
        kei_lsp::server::main_loop(&server).expect("main loop");
    });

    // 1) didOpen(エフェクト未宣言のソース)を送る。
    let did_open = Notification::new(
        DidOpenTextDocument::METHOD.to_string(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri(),
                language_id: "kei".to_string(),
                version: 1,
                text: SRC.to_string(),
            },
        },
    );
    client.sender.send(Message::Notification(did_open)).unwrap();

    // publishDiagnostics 通知が返り、エフェクト診断を含む。
    let msg = client.receiver.recv().expect("publishDiagnostics");
    let note = match msg {
        Message::Notification(n) => n,
        other => panic!("expected notification, got {other:?}"),
    };
    assert_eq!(note.method, PublishDiagnostics::METHOD);
    let params: PublishDiagnosticsParams = serde_json::from_value(note.params).unwrap();
    assert!(
        params.diagnostics.iter().any(|d| matches!(
            &d.code,
            Some(NumberOrString::String(c)) if c.starts_with("KEI-E3")
        )),
        "expected effect diagnostic, got {:?}",
        params.diagnostics
    );

    // 2) hover を関数名 writeRow(3 行目, 0 始まり line=2)の上に送る。
    let hover_req = Request::new(
        RequestId::from(1),
        HoverRequest::METHOD.to_string(),
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri() },
                position: Position {
                    line: 2,
                    character: 8,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        },
    );
    client.sender.send(Message::Request(hover_req)).unwrap();

    let msg = client.receiver.recv().expect("hover response");
    let resp = match msg {
        Message::Response(r) => r,
        other => panic!("expected response, got {other:?}"),
    };
    let hover: Hover = serde_json::from_value(resp.result.expect("non-null hover")).unwrap();
    match hover.contents {
        HoverContents::Markup(m) => {
            assert!(m.value.contains("func writeRow("), "{}", m.value);
            assert!(m.value.contains("uses Database.Write"), "{}", m.value);
        }
        other => panic!("expected markup hover, got {other:?}"),
    }

    // 3) shutdown → exit でループを正常終了させる。
    let shutdown = Request::new(RequestId::from(2), "shutdown".to_string(), ());
    client.sender.send(Message::Request(shutdown)).unwrap();
    let _ = client.receiver.recv().expect("shutdown response");
    let exit = Notification::new("exit".to_string(), ());
    client.sender.send(Message::Notification(exit)).unwrap();

    handle.join().expect("server thread joins");
}
