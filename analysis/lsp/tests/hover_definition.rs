//! Hover and goto-definition request handlers wired end-to-end through the
//! in-process LSP transport.

use {
    lsp_server::{Connection, Message, Notification, Request, RequestId, Response},
    lsp_types::{
        notification::{DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics},
        request::{GotoDefinition, HoverRequest, Initialize, Request as _},
        DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, Hover,
        HoverContents, HoverParams, InitializeParams, InitializedParams, Location, Position,
        PublishDiagnosticsParams, TextDocumentIdentifier, TextDocumentItem,
        TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    },
    quasar_lsp::{capabilities::server_capabilities, Server},
    std::{str::FromStr, time::Duration},
};

fn spawn_server() -> (Connection, std::thread::JoinHandle<()>) {
    let (server_conn, client_conn) = Connection::memory();
    let handle = std::thread::spawn(move || {
        let caps = serde_json::to_value(server_capabilities()).unwrap();
        let init_value = server_conn.initialize(caps).expect("initialize handshake");
        let _params: InitializeParams = serde_json::from_value(init_value).unwrap();
        let server = Server::new(server_conn.sender.clone());
        server.run(&server_conn).expect("server loop runs");
    });
    (client_conn, handle)
}

fn handshake(conn: &Connection) {
    let req = Request {
        id: RequestId::from(1),
        method: Initialize::METHOD.to_string(),
        params: serde_json::to_value(InitializeParams::default()).unwrap(),
    };
    conn.sender.send(Message::Request(req)).unwrap();

    let response = recv_until(
        conn,
        |m| matches!(m, Message::Response(_)),
        Duration::from_secs(5),
    )
    .expect("initialize response");
    match response {
        Message::Response(Response { error: None, .. }) => {}
        other => panic!("unexpected initialize response: {:?}", other),
    }
    conn.sender
        .send(Message::Notification(Notification {
            method: Initialized::METHOD.to_string(),
            params: serde_json::to_value(InitializedParams {}).unwrap(),
        }))
        .unwrap();
}

fn open(conn: &Connection, uri: Uri, text: &str) {
    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri,
            language_id: "rust".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    conn.sender
        .send(Message::Notification(Notification {
            method: DidOpenTextDocument::METHOD.to_string(),
            params: serde_json::to_value(params).unwrap(),
        }))
        .unwrap();
}

fn await_initial_diagnostics(conn: &Connection, uri: &Uri) {
    let _ = recv_until(
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
        Duration::from_secs(5),
    );
}

fn send_request<R>(conn: &Connection, id: i32, params: R::Params)
where
    R: lsp_types::request::Request,
    R::Params: serde::Serialize,
{
    conn.sender
        .send(Message::Request(Request {
            id: RequestId::from(id),
            method: R::METHOD.to_string(),
            params: serde_json::to_value(params).unwrap(),
        }))
        .unwrap();
}

fn await_response<T: serde::de::DeserializeOwned>(conn: &Connection, id: i32) -> Response {
    let target = RequestId::from(id);
    let msg = recv_until(
        conn,
        |m| matches!(m, Message::Response(r) if r.id == target),
        Duration::from_secs(5),
    )
    .expect("response did not arrive in 5s");
    let _: std::marker::PhantomData<T> = std::marker::PhantomData;
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

// Find a UTF-16 column index for the first occurrence of `needle` on the given
// line (1-indexed). Falls back to 0 if not found.
fn column_of(text: &str, line: u32, needle: &str) -> u32 {
    let line_text = text.lines().nth(line as usize).expect("line exists");
    let byte_col = line_text.find(needle).expect("needle on line");
    line_text[..byte_col].encode_utf16().count() as u32
}

#[test]
fn hover_on_account_type_returns_resolved_content() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let counter_src = "#[account(discriminator = 1)]\npub struct Counter { pub n: u64 }\n";
    let inc_src = "#[derive(Accounts)]\npub struct Increment<'info> {\n    pub counter: &'info \
                   mut Account<Counter>,\n}\n";

    let counter_uri = Uri::from_str("file:///tmp/state.rs").unwrap();
    let inc_uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    open(&client, counter_uri.clone(), counter_src);
    open(&client, inc_uri.clone(), inc_src);

    await_initial_diagnostics(&client, &counter_uri);
    await_initial_diagnostics(&client, &inc_uri);

    // Position cursor on `Counter` inside `Account<Counter>` (line index 2,
    // i.e. third line of inc_src — `    pub counter: &'info mut
    // Account<Counter>,`).
    let col = column_of(inc_src, 2, "Counter");
    send_request::<HoverRequest>(
        &client,
        10,
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: inc_uri.clone(),
                },
                position: Position {
                    line: 2,
                    character: col,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        },
    );
    let resp = await_response::<Hover>(&client, 10);
    let hover: Hover =
        serde_json::from_value(resp.result.expect("hover result")).expect("hover parses");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup contents");
    };
    assert!(
        markup.value.contains("Counter"),
        "hover should mention the type name, got: {}",
        markup.value
    );
    assert!(
        markup.value.contains("state.rs"),
        "hover should mention the defining file, got: {}",
        markup.value
    );
}

#[test]
fn hover_outside_quasar_region_returns_null() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let inc_src = "#[derive(Accounts)]\npub struct Increment<'info> {\n    pub counter: &'info \
                   mut Account<Counter>,\n}\n";
    let inc_uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    open(&client, inc_uri.clone(), inc_src);
    await_initial_diagnostics(&client, &inc_uri);

    // Cursor on `pub` keyword — outside any Quasar region.
    let col = column_of(inc_src, 1, "pub");
    send_request::<HoverRequest>(
        &client,
        20,
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: inc_uri.clone(),
                },
                position: Position {
                    line: 1,
                    character: col,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        },
    );
    let resp = await_response::<Hover>(&client, 20);
    assert!(
        matches!(&resp.result, Some(v) if v.is_null()),
        "hover outside Quasar region should be null, got {:?}",
        resp.result
    );
}

#[test]
fn goto_definition_resolves_account_type_across_files() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let counter_src = "#[account(discriminator = 1)]\npub struct Counter { pub n: u64 }\n";
    let inc_src = "#[derive(Accounts)]\npub struct Increment<'info> {\n    pub counter: &'info \
                   mut Account<Counter>,\n}\n";

    let counter_uri = Uri::from_str("file:///tmp/state.rs").unwrap();
    let inc_uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    open(&client, counter_uri.clone(), counter_src);
    open(&client, inc_uri.clone(), inc_src);

    await_initial_diagnostics(&client, &counter_uri);
    await_initial_diagnostics(&client, &inc_uri);

    let col = column_of(inc_src, 2, "Counter");
    send_request::<GotoDefinition>(
        &client,
        30,
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: inc_uri.clone(),
                },
                position: Position {
                    line: 2,
                    character: col,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        },
    );
    let resp = await_response::<GotoDefinitionResponse>(&client, 30);
    let value = resp.result.expect("definition result");
    let response: GotoDefinitionResponse =
        serde_json::from_value(value).expect("goto response parses");
    let location: Location = match response {
        GotoDefinitionResponse::Scalar(l) => l,
        other => panic!("expected single location, got {:?}", other),
    };
    assert_eq!(location.uri, counter_uri);
    // The Counter identifier sits on line 1 of state.rs (zero-indexed, after the
    // attribute line). The range covers the identifier; assert its line is 1.
    assert_eq!(
        location.range.start.line, 1,
        "Counter should be on line 1 of state.rs"
    );
}

#[test]
fn goto_definition_unknown_account_type_returns_null() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let inc_src = "#[derive(Accounts)]\npub struct Increment<'info> {\n    pub counter: &'info \
                   mut Account<Missing>,\n}\n";
    let inc_uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    open(&client, inc_uri.clone(), inc_src);
    await_initial_diagnostics(&client, &inc_uri);

    let col = column_of(inc_src, 2, "Missing");
    send_request::<GotoDefinition>(
        &client,
        40,
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: inc_uri.clone(),
                },
                position: Position {
                    line: 2,
                    character: col,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        },
    );
    let resp = await_response::<GotoDefinitionResponse>(&client, 40);
    assert!(
        matches!(&resp.result, Some(v) if v.is_null()),
        "goto-def on unknown type should be null, got {:?}",
        resp.result
    );
}
