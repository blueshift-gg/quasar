//! Activation gating: a [`WorkspaceConfig`] limits the LSP to files that
//! live under known Quasar crate roots. Files outside receive no
//! diagnostics, and a previously-published diagnostic gets cleared on the
//! file's didOpen.

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::{
    DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{Initialize, Request as _};
use lsp_types::{
    DidOpenTextDocumentParams, InitializeParams, InitializedParams, PublishDiagnosticsParams,
    TextDocumentItem, Uri,
};
use quasar_lsp::{capabilities::server_capabilities, Server, WorkspaceConfig};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

fn spawn_server_with_config(config: WorkspaceConfig) -> (Connection, std::thread::JoinHandle<()>) {
    let (server_conn, client_conn) = Connection::memory();
    let handle = std::thread::spawn(move || {
        let caps = serde_json::to_value(server_capabilities()).unwrap();
        let init_value = server_conn.initialize(caps).expect("initialize handshake");
        let _: InitializeParams = serde_json::from_value(init_value).unwrap();
        let server = Server::for_test_with_config(server_conn.sender.clone(), config);
        server.run(&server_conn).expect("server loop runs");
    });
    (client_conn, handle)
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
        Duration::from_secs(5),
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

fn await_diagnostics(conn: &Connection, uri: &Uri, timeout: Duration) -> Vec<lsp_types::Diagnostic> {
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
        timeout,
    )
    .expect("publishDiagnostics did not arrive in time");
    match msg {
        Message::Notification(n) => {
            serde_json::from_value::<PublishDiagnosticsParams>(n.params)
                .unwrap()
                .diagnostics
        }
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

#[test]
fn file_inside_quasar_crate_gets_diagnostics() {
    let config = WorkspaceConfig {
        workspace_root: PathBuf::from("/tmp/ws"),
        quasar_crate_roots: vec![PathBuf::from("/tmp/ws/program")],
    };
    let (client, _h) = spawn_server_with_config(config);
    handshake(&client);

    let uri = Uri::from_str("file:///tmp/ws/program/src/lib.rs").unwrap();
    open(
        &client,
        uri.clone(),
        "#[account(set_inner)]\npub struct Bad { pub n: u64 }\n",
    );

    let diags = await_diagnostics(&client, &uri, Duration::from_secs(5));
    assert!(
        !diags.is_empty(),
        "in-Quasar file should receive at least one diagnostic"
    );
}

#[test]
fn file_outside_quasar_crate_gets_empty_diagnostics() {
    let config = WorkspaceConfig {
        workspace_root: PathBuf::from("/tmp/ws"),
        quasar_crate_roots: vec![PathBuf::from("/tmp/ws/program")],
    };
    let (client, _h) = spawn_server_with_config(config);
    handshake(&client);

    let outside = Uri::from_str("file:///tmp/ws/other/src/lib.rs").unwrap();
    open(
        &client,
        outside.clone(),
        "#[account(set_inner)]\npub struct Bad { pub n: u64 }\n",
    );

    // The gate publishes EMPTY diagnostics (clearing) immediately on
    // didOpen for non-Quasar files.
    let diags = await_diagnostics(&client, &outside, Duration::from_secs(5));
    assert!(
        diags.is_empty(),
        "out-of-Quasar file should receive empty diagnostics, got {:?}",
        diags
    );
}

#[test]
fn no_config_degrades_to_all_files_active() {
    let (server_conn, client_conn) = Connection::memory();
    let _handle = std::thread::spawn(move || {
        let caps = serde_json::to_value(server_capabilities()).unwrap();
        let init_value = server_conn.initialize(caps).expect("initialize handshake");
        let _: InitializeParams = serde_json::from_value(init_value).unwrap();
        let server = Server::new(server_conn.sender.clone());
        server.run(&server_conn).expect("server loop runs");
    });

    handshake(&client_conn);

    let uri = Uri::from_str("file:///tmp/anything.rs").unwrap();
    open(
        &client_conn,
        uri.clone(),
        "#[account(set_inner)]\npub struct Bad { pub n: u64 }\n",
    );

    let diags = await_diagnostics(&client_conn, &uri, Duration::from_secs(5));
    assert!(
        !diags.is_empty(),
        "with no config, all files are serviced"
    );
}
