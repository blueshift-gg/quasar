//! Constraint-validation LSP features: diagnostics + the "Add `mut`" fix.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{CodeActionRequest, Initialize, Request as _};
use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Diagnostic,
    DidOpenTextDocumentParams, InitializeParams, InitializedParams, PartialResultParams,
    PublishDiagnosticsParams, TextDocumentIdentifier, TextDocumentItem, Uri,
    WorkDoneProgressParams,
};
use quasar_lsp::{capabilities::server_capabilities, Server, WorkspaceConfig};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

const URI: &str = "file:///tmp/ws/ix.rs";

fn spawn() -> Connection {
    let config = WorkspaceConfig {
        workspace_roots: vec![PathBuf::from("/tmp/ws")],
        quasar_crate_roots: vec![PathBuf::from("/tmp/ws")],
        known_account_types: Vec::new(),
        indexed_source_files: Vec::new(),
    };
    let (server_conn, client_conn) = Connection::memory();
    std::thread::spawn(move || {
        let caps = serde_json::to_value(server_capabilities()).unwrap();
        let init_value = server_conn.initialize(caps).expect("handshake");
        let _: InitializeParams = serde_json::from_value(init_value).unwrap();
        let server = Server::for_test_with_config(server_conn.sender.clone(), config);
        server.run(&server_conn).expect("server loop");
    });
    client_conn
}

fn handshake(conn: &Connection) {
    conn.sender
        .send(Message::Request(Request {
            id: RequestId::from(1),
            method: Initialize::METHOD.to_string(),
            params: serde_json::to_value(InitializeParams::default()).unwrap(),
        }))
        .unwrap();
    let _ = recv_until(
        conn,
        |m| matches!(m, Message::Response(_)),
        Duration::from_secs(20),
    );
    conn.sender
        .send(Message::Notification(Notification {
            method: Initialized::METHOD.to_string(),
            params: serde_json::to_value(InitializedParams {}).unwrap(),
        }))
        .unwrap();
}

fn open(conn: &Connection, text: &str) {
    conn.sender
        .send(Message::Notification(Notification {
            method: DidOpenTextDocument::METHOD.to_string(),
            params: serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Uri::from_str(URI).unwrap(),
                    language_id: "rust".into(),
                    version: 1,
                    text: text.into(),
                },
            })
            .unwrap(),
        }))
        .unwrap();
}

fn await_diagnostics(conn: &Connection) -> Vec<Diagnostic> {
    let uri = Uri::from_str(URI).unwrap();
    let msg = recv_until(
        conn,
        |m| {
            if let Message::Notification(n) = m {
                if n.method == PublishDiagnostics::METHOD {
                    if let Ok(p) =
                        serde_json::from_value::<PublishDiagnosticsParams>(n.params.clone())
                    {
                        return p.uri == uri && !p.diagnostics.is_empty();
                    }
                }
            }
            false
        },
        Duration::from_secs(20),
    )
    .expect("publishDiagnostics");
    match msg {
        Message::Notification(n) => {
            serde_json::from_value::<PublishDiagnosticsParams>(n.params)
                .unwrap()
                .diagnostics
        }
        _ => unreachable!(),
    }
}

fn send_code_action(conn: &Connection, id: i32, diags: Vec<Diagnostic>, range: lsp_types::Range) {
    let params = CodeActionParams {
        text_document: TextDocumentIdentifier {
            uri: Uri::from_str(URI).unwrap(),
        },
        range,
        context: CodeActionContext {
            diagnostics: diags,
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    conn.sender
        .send(Message::Request(Request {
            id: RequestId::from(id),
            method: CodeActionRequest::METHOD.to_string(),
            params: serde_json::to_value(params).unwrap(),
        }))
        .unwrap();
}

fn await_response(conn: &Connection, id: i32) -> Response {
    let target = RequestId::from(id);
    let msg = recv_until(
        conn,
        |m| matches!(m, Message::Response(r) if r.id == target),
        Duration::from_secs(20),
    )
    .expect("response");
    match msg {
        Message::Response(r) => r,
        _ => unreachable!(),
    }
}

fn recv_until(
    conn: &Connection,
    pred: impl Fn(&Message) -> bool,
    timeout: Duration,
) -> Option<Message> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.checked_duration_since(std::time::Instant::now())?;
        match conn.receiver.recv_timeout(remaining).ok()? {
            msg if pred(&msg) => return Some(msg),
            _ => continue,
        }
    }
}

const REALLOC_NO_MUT: &str = "#[derive(Accounts)]\npub struct Resize<'info> {\n    pub payer: Signer,\n    #[account(realloc = 128, payer = payer)]\n    pub account: &'info Account<Thing>,\n}\n";

#[test]
fn constraint_violation_is_published() {
    let client = spawn();
    handshake(&client);
    open(&client, REALLOC_NO_MUT);
    let diags = await_diagnostics(&client);
    let codes: Vec<_> = diags
        .iter()
        .filter_map(|d| match d.code.as_ref()? {
            lsp_types::NumberOrString::String(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        codes.iter().any(|c| c.contains("accounts_constraint_violation")),
        "expected constraint violation diagnostic, got {:?}",
        codes
    );
}

#[test]
fn add_mut_code_action_offered_for_requires_mut() {
    let client = spawn();
    handshake(&client);
    open(&client, REALLOC_NO_MUT);
    let diags = await_diagnostics(&client);
    let violation = diags
        .iter()
        .find(|d| {
            matches!(&d.code, Some(lsp_types::NumberOrString::String(s))
                if s.contains("accounts_constraint_violation"))
                && d.message.contains("requires `mut`")
        })
        .expect("requires-mut constraint diagnostic")
        .clone();

    send_code_action(&client, 50, vec![violation.clone()], violation.range);
    let resp = await_response(&client, 50);
    let response: CodeActionResponse =
        serde_json::from_value(resp.result.expect("code action result")).unwrap();
    let action = response
        .iter()
        .find_map(|c| match c {
            CodeActionOrCommand::CodeAction(a) if a.title == "Add `mut`" => Some(a.clone()),
            _ => None,
        })
        .expect("Add `mut` action");
    let edit = action.edit.expect("edit");
    let changes = edit.changes.expect("changes");
    let edits = &changes[&Uri::from_str(URI).unwrap()];
    assert!(
        edits[0].new_text.contains("mut"),
        "inserted text should add mut"
    );
}
