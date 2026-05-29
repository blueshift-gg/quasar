//! End-to-end: spawn the server in-process via `Connection::memory()`, send
//! initialize + didOpen, assert the expected `publishDiagnostics` arrives.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{Initialize, Request as _};
use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams, InitializedParams,
    NumberOrString, PublishDiagnosticsParams, TextDocumentContentChangeEvent, TextDocumentItem,
    Uri, VersionedTextDocumentIdentifier,
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

fn send_did_change(conn: &Connection, uri: Uri, text: &str) {
    let params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier { uri, version: 2 },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_string(),
        }],
    };
    let notif = Notification {
        method: DidChangeTextDocument::METHOD.to_string(),
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
fn genuinely_unknown_account_type_is_diagnosed() {
    let (client, _server_handle) = spawn_server();
    send_initialize(&client);

    let uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    // Empty workspace config (no cargo metadata in this in-memory test), so
    // `Counter` is in neither the workspace nor any indexed dependency — a
    // genuinely-unknown type, which is diagnosed.
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
        "genuinely unknown account type should be diagnosed, got {:?}",
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

#[test]
fn editing_a_definition_file_refreshes_other_open_files() {
    // Regression: a `didChange` to a definition file must refresh cross-file
    // diagnostics in *other* open files, not just the edited one. Here defining
    // `Counter` in state.rs must clear the unknown-type error on `Account<Counter>`
    // in the (unedited) instructions file.
    let (client, _server_handle) = spawn_server();
    send_initialize(&client);

    let inc = Uri::from_str("file:///tmp/refresh_inc.rs").unwrap();
    let state = Uri::from_str("file:///tmp/refresh_state.rs").unwrap();

    send_did_open(
        &client,
        inc.clone(),
        "#[derive(Accounts)]\npub struct Inc<'info> {\n    pub counter: &'info mut Account<Counter>,\n}\n",
    );
    // Initially `Counter` is undefined → unknown-account-type error.
    let initial = await_diagnostics(&client, &inc);
    assert!(
        initial.iter().any(|d| matches!(&d.code,
            Some(NumberOrString::String(s)) if s.contains("unknown_account_type"))),
        "expected unknown_account_type before Counter is defined, got {:?}",
        initial
    );

    // Open an empty definition file, then define `Counter` via didChange.
    send_did_open(&client, state.clone(), "");
    send_did_change(
        &client,
        state.clone(),
        "#[account(discriminator = 1)]\npub struct Counter { pub n: u64 }\n",
    );

    // The instructions file must be republished without the unknown-type error.
    // recv_until skips any stale (pre-change) republish that still carries it.
    let cleared = recv_until(
        &client,
        |m| {
            if let Message::Notification(n) = m {
                if n.method == PublishDiagnostics::METHOD {
                    if let Ok(p) =
                        serde_json::from_value::<PublishDiagnosticsParams>(n.params.clone())
                    {
                        return p.uri == inc
                            && !p.diagnostics.iter().any(|d| matches!(&d.code,
                                Some(NumberOrString::String(s)) if s.contains("unknown_account_type")));
                    }
                }
            }
            false
        },
        Duration::from_secs(5),
    );
    assert!(
        cleared.is_some(),
        "editing the definition file must refresh and clear the other file's diagnostic"
    );
}
