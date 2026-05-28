//! End-to-end: spawn the server in-process via `Connection::memory()`, send
//! initialize + didOpen, assert the expected `publishDiagnostics` arrives.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{Initialize, Request as _};
use lsp_types::{
    DidOpenTextDocumentParams, InitializeParams, InitializedParams, PublishDiagnosticsParams,
    TextDocumentItem, Uri,
};
use quasar_lsp::{capabilities::server_capabilities, Server};
use std::str::FromStr;
use std::time::Duration;

/// Helper to bring up a Server in another thread on top of an in-memory
/// connection. Returns the client side of the connection and the join handle.
fn spawn_server() -> (Connection, std::thread::JoinHandle<()>) {
    let (server_conn, client_conn) = Connection::memory();
    let handle = std::thread::spawn(move || {
        let caps =
            serde_json::to_value(server_capabilities()).expect("server caps serialise");
        // initialize() blocks until the client sends Initialize, then sends the
        // response and returns the params.
        let init_value = server_conn.initialize(caps).expect("initialize handshake");
        let _params: InitializeParams =
            serde_json::from_value(init_value).expect("init params parse");

        let server = Server::new(server_conn.sender.clone());
        server.run(&server_conn).expect("server loop runs");
    });
    (client_conn, handle)
}

fn send_initialize(conn: &Connection) {
    let req = Request {
        id: RequestId::from(1),
        method: Initialize::METHOD.to_string(),
        params: serde_json::to_value(InitializeParams::default()).unwrap(),
    };
    conn.sender.send(Message::Request(req)).unwrap();

    // Wait for the initialize response.
    let response = recv_until(conn, |m| matches!(m, Message::Response(_)), Duration::from_secs(5))
        .expect("initialize response");
    match response {
        Message::Response(Response { error: None, .. }) => {}
        other => panic!("unexpected initialize response: {:?}", other),
    }

    // Per spec, send the Initialized notification.
    let notif = Notification {
        method: Initialized::METHOD.to_string(),
        params: serde_json::to_value(InitializedParams {}).unwrap(),
    };
    conn.sender.send(Message::Notification(notif)).unwrap();
}

fn send_did_open(conn: &Connection, uri: Uri, text: &str) {
    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri,
            language_id: "rust".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    let notif = Notification {
        method: DidOpenTextDocument::METHOD.to_string(),
        params: serde_json::to_value(params).unwrap(),
    };
    conn.sender.send(Message::Notification(notif)).unwrap();
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

fn await_diagnostics(conn: &Connection, uri: &Uri) -> Vec<lsp_types::Diagnostic> {
    let msg = recv_until(
        conn,
        |m| {
            if let Message::Notification(n) = m {
                if n.method == PublishDiagnostics::METHOD {
                    if let Ok(params) =
                        serde_json::from_value::<PublishDiagnosticsParams>(n.params.clone())
                    {
                        return params.uri == *uri;
                    }
                }
            }
            false
        },
        Duration::from_secs(5),
    )
    .expect("publishDiagnostics did not arrive in 5s");
    match msg {
        Message::Notification(n) => {
            serde_json::from_value::<PublishDiagnosticsParams>(n.params)
                .unwrap()
                .diagnostics
        }
        _ => unreachable!(),
    }
}

#[test]
fn server_publishes_diagnostics_for_malformed_account_attribute() {
    let (client, _server_handle) = spawn_server();
    send_initialize(&client);

    let uri = Uri::from_str("file:///tmp/broken.rs").unwrap();
    send_did_open(
        &client,
        uri.clone(),
        // missing `discriminator` or `unsafe_no_disc`: triggers
        // AccountAttrMissingDiscriminatorOrUnsafe diagnostic
        "#[account(set_inner)]\npub struct Broken { pub x: u64 }\n",
    );

    let diagnostics = await_diagnostics(&client, &uri);
    assert!(
        !diagnostics.is_empty(),
        "expected at least one diagnostic, got none"
    );
    let codes: Vec<_> = diagnostics
        .iter()
        .filter_map(|d| match d.code.as_ref()? {
            lsp_types::NumberOrString::String(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        codes.iter().any(|c| c
            .contains("missing_discriminator_or_unsafe")),
        "expected missing-discriminator code, got {:?}",
        codes
    );
    let sources: Vec<_> = diagnostics
        .iter()
        .filter_map(|d| d.source.as_deref())
        .collect();
    assert!(
        sources.iter().all(|s| *s == "quasar"),
        "all diagnostics should have source=\"quasar\", got {:?}",
        sources
    );
}

#[test]
fn server_publishes_unknown_account_type_for_cross_file_reference() {
    let (client, _server_handle) = spawn_server();
    send_initialize(&client);

    let uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    // Single-file workspace: `Account<Counter>` references a type the workspace
    // doesn't know about, which the resolver flags as UnknownAccountType.
    send_did_open(
        &client,
        uri.clone(),
        "#[derive(Accounts)]\npub struct Increment<'info> {\n    pub counter: &'info mut Account<Counter>,\n}\n",
    );

    let diagnostics = await_diagnostics(&client, &uri);
    let codes: Vec<_> = diagnostics
        .iter()
        .filter_map(|d| match d.code.as_ref()? {
            lsp_types::NumberOrString::String(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        codes.iter().any(|c| c.contains("unknown_account_type")),
        "expected unknown-account-type code, got {:?}",
        codes
    );
}

#[test]
fn server_publishes_empty_diagnostics_on_clean_input() {
    let (client, _server_handle) = spawn_server();
    send_initialize(&client);

    let uri = Uri::from_str("file:///tmp/clean.rs").unwrap();
    send_did_open(
        &client,
        uri.clone(),
        "#[account(discriminator = 1)]\npub struct Counter { pub n: u64 }\n",
    );

    let diagnostics = await_diagnostics(&client, &uri);
    assert!(
        diagnostics.is_empty(),
        "clean input should produce no diagnostics, got {:?}",
        diagnostics
    );
}
