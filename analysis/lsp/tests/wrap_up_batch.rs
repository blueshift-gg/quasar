//! Coverage for the wrap-up batch: multi-folder activation, fs-watcher
//! registration, and the completed diagnostic-driven code actions.

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::{
    DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{CodeActionRequest, Initialize, RegisterCapability, Request as _};
use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Diagnostic,
    DidOpenTextDocumentParams, InitializeParams, InitializedParams, PartialResultParams, Position,
    PublishDiagnosticsParams, Range, TextDocumentIdentifier, TextDocumentItem, Uri,
    WorkDoneProgressParams,
};
use quasar_lsp::{capabilities::server_capabilities, Server, WorkspaceConfig};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

fn spawn_with_config(config: WorkspaceConfig) -> Connection {
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

fn spawn_with_watching(root: PathBuf) -> Connection {
    let (server_conn, client_conn) = Connection::memory();
    std::thread::spawn(move || {
        let caps = serde_json::to_value(server_capabilities()).unwrap();
        let init_value = server_conn.initialize(caps).expect("handshake");
        let _: InitializeParams = serde_json::from_value(init_value).unwrap();
        let server =
            Server::with_workspace_roots(server_conn.sender.clone(), vec![root], true);
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

fn open(conn: &Connection, uri: Uri, text: &str) {
    conn.sender
        .send(Message::Notification(Notification {
            method: DidOpenTextDocument::METHOD.to_string(),
            params: serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "rust".into(),
                    version: 1,
                    text: text.into(),
                },
            })
            .unwrap(),
        }))
        .unwrap();
}

fn await_diagnostics(conn: &Connection, uri: &Uri) -> Vec<Diagnostic> {
    let msg = recv_until(
        conn,
        |m| {
            if let Message::Notification(n) = m {
                if n.method == PublishDiagnostics::METHOD {
                    if let Ok(p) =
                        serde_json::from_value::<PublishDiagnosticsParams>(n.params.clone())
                    {
                        return p.uri == *uri;
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

fn send_code_action(conn: &Connection, id: i32, uri: Uri, range: Range, diags: Vec<Diagnostic>) {
    let params = CodeActionParams {
        text_document: TextDocumentIdentifier { uri },
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

fn await_code_action(conn: &Connection, id: i32) -> CodeActionResponse {
    let target = RequestId::from(id);
    let msg = recv_until(
        conn,
        |m| matches!(m, Message::Response(r) if r.id == target),
        Duration::from_secs(20),
    )
    .expect("code action response");
    match msg {
        Message::Response(r) => serde_json::from_value(r.result.expect("result")).unwrap(),
        _ => unreachable!(),
    }
}

fn action_titles(resp: &CodeActionResponse) -> Vec<String> {
    resp.iter()
        .map(|c| match c {
            CodeActionOrCommand::CodeAction(a) => a.title.clone(),
            CodeActionOrCommand::Command(c) => c.title.clone(),
        })
        .collect()
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

#[test]
fn multi_folder_services_files_in_every_crate_root() {
    let config = WorkspaceConfig {
        workspace_roots: vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")],
        quasar_crate_roots: vec![
            PathBuf::from("/tmp/a/prog"),
            PathBuf::from("/tmp/b/prog"),
        ],
        known_account_types: Vec::new(),
        indexed_source_files: Vec::new(),
    };
    let client = spawn_with_config(config);
    handshake(&client);

    let a = Uri::from_str("file:///tmp/a/prog/src/lib.rs").unwrap();
    let b = Uri::from_str("file:///tmp/b/prog/src/lib.rs").unwrap();
    let bad = "#[account(set_inner)]\npub struct X { pub n: u64 }\n";
    open(&client, a.clone(), bad);
    open(&client, b.clone(), bad);

    // Collect publishes for both files without discarding whichever arrives
    // first. The server may publish the same file more than once (membership
    // changes trigger an all-open-files pass); we keep the latest per uri.
    let mut seen: std::collections::HashMap<Uri, Vec<Diagnostic>> =
        std::collections::HashMap::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while seen.len() < 2 && std::time::Instant::now() < deadline {
        let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
            break;
        };
        let Ok(msg) = client.receiver.recv_timeout(remaining) else {
            break;
        };
        if let Message::Notification(n) = msg {
            if n.method == PublishDiagnostics::METHOD {
                if let Ok(p) =
                    serde_json::from_value::<PublishDiagnosticsParams>(n.params.clone())
                {
                    if (p.uri == a || p.uri == b) && !p.diagnostics.is_empty() {
                        seen.insert(p.uri, p.diagnostics);
                    }
                }
            }
        }
    }

    assert!(seen.contains_key(&a), "folder A file should be serviced");
    assert!(seen.contains_key(&b), "folder B file should be serviced");
}

#[test]
fn registers_cargo_watchers_on_initialized() {
    // Use a non-existent root so the background cargo metadata fails and
    // degrades; the watcher registration on `initialized` is independent of
    // that and must still fire.
    let client = spawn_with_watching(PathBuf::from("/tmp/does-not-exist-quasar-test"));
    handshake(&client);

    let msg = recv_until(
        &client,
        |m| matches!(m, Message::Request(r) if r.method == RegisterCapability::METHOD),
        Duration::from_secs(20),
    )
    .expect("server should send client/registerCapability");
    match msg {
        Message::Request(req) => {
            let params: lsp_types::RegistrationParams =
                serde_json::from_value(req.params).unwrap();
            assert!(
                params
                    .registrations
                    .iter()
                    .any(|r| r.method == "workspace/didChangeWatchedFiles"),
                "should register a didChangeWatchedFiles watcher"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn code_action_add_discriminator_and_unsafe_for_missing_disc() {
    let config = WorkspaceConfig {
        workspace_roots: vec![PathBuf::from("/tmp/ws")],
        quasar_crate_roots: vec![PathBuf::from("/tmp/ws")],
        known_account_types: Vec::new(),
        indexed_source_files: Vec::new(),
    };
    let client = spawn_with_config(config);
    handshake(&client);

    let uri = Uri::from_str("file:///tmp/ws/lib.rs").unwrap();
    open(
        &client,
        uri.clone(),
        "#[account(set_inner)]\npub struct Broken { pub n: u64 }\n",
    );
    let diags = await_diagnostics(&client, &uri);
    let missing = diags
        .iter()
        .find(|d| {
            matches!(&d.code, Some(lsp_types::NumberOrString::String(s))
                if s.contains("missing_discriminator_or_unsafe"))
        })
        .expect("missing-discriminator diagnostic")
        .clone();

    send_code_action(&client, 50, uri.clone(), missing.range, vec![missing.clone()]);
    let resp = await_code_action(&client, 50);
    let titles = action_titles(&resp);
    assert!(
        titles.iter().any(|t| t.contains("discriminator = 1")),
        "expected add-discriminator action, got {:?}",
        titles
    );
    assert!(
        titles.iter().any(|t| t.contains("unsafe_no_disc")),
        "expected add-unsafe action, got {:?}",
        titles
    );
}

#[test]
fn code_action_insert_account_on_bare_struct() {
    let config = WorkspaceConfig {
        workspace_roots: vec![PathBuf::from("/tmp/ws")],
        quasar_crate_roots: vec![PathBuf::from("/tmp/ws")],
        known_account_types: Vec::new(),
        indexed_source_files: Vec::new(),
    };
    let client = spawn_with_config(config);
    handshake(&client);

    let uri = Uri::from_str("file:///tmp/ws/bare.rs").unwrap();
    open(&client, uri.clone(), "pub struct Counter { pub n: u64 }\n");
    let _ = await_diagnostics(&client, &uri);

    // Cursor on the struct name.
    let range = Range {
        start: Position { line: 0, character: 11 },
        end: Position { line: 0, character: 11 },
    };
    send_code_action(&client, 70, uri.clone(), range, vec![]);
    let resp = await_code_action(&client, 70);
    let titles = action_titles(&resp);
    assert!(
        titles.iter().any(|t| t.contains("Insert #[account")),
        "expected insert-account action, got {:?}",
        titles
    );
}
