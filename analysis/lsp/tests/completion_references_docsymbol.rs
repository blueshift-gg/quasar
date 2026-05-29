//! End-to-end coverage for completion, textDocument/references, and
//! textDocument/documentSymbol.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{
    Completion, DocumentSymbolRequest, Initialize, References, Request as _,
};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionParams, CompletionResponse, CompletionTriggerKind,
    DidOpenTextDocumentParams, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    InitializeParams, InitializedParams, Location, PartialResultParams, Position,
    PublishDiagnosticsParams, ReferenceContext, ReferenceParams, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use quasar_lsp::{capabilities::server_capabilities, Server};
use std::str::FromStr;
use std::time::Duration;

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

fn await_response(conn: &Connection, id: i32) -> Response {
    let target = RequestId::from(id);
    let msg = recv_until(
        conn,
        |m| matches!(m, Message::Response(r) if r.id == target),
        Duration::from_secs(5),
    )
    .expect("response did not arrive in 5s");
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

fn column_of(text: &str, line: u32, needle: &str) -> u32 {
    let line_text = text.lines().nth(line as usize).expect("line exists");
    let byte_col = line_text.find(needle).expect("needle on line");
    line_text[..byte_col].encode_utf16().count() as u32
}

#[test]
fn completion_inside_account_type_arg_returns_account_types() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let state_src = "\
#[account(discriminator = 1)]\n\
pub struct Counter { pub n: u64 }\n\
\n\
#[account(discriminator = 2)]\n\
pub struct Vault { pub balance: u64 }\n\
\n\
quasar_lang::define_account!(pub struct Mint => [checks::ZeroPod]: MintData);\n\
pub struct MintData { pub supply: u64 }\n\
";
    let inc_src = "\
#[derive(Accounts)]\n\
pub struct Increment<'info> {\n\
    pub counter: &'info mut Account<Counter>,\n\
}\n\
";

    let state_uri = Uri::from_str("file:///tmp/state.rs").unwrap();
    let inc_uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    open(&client, state_uri.clone(), state_src);
    open(&client, inc_uri.clone(), inc_src);
    await_initial_diagnostics(&client, &state_uri);
    await_initial_diagnostics(&client, &inc_uri);

    // Cursor just after the `<` of `Account<Counter>`.
    let col = column_of(inc_src, 2, "Account<") + "Account<".encode_utf16().count() as u32;
    send_request::<Completion>(
        &client,
        100,
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: inc_uri.clone() },
                position: Position { line: 2, character: col },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        },
    );
    let resp = await_response(&client, 100);
    let value = resp.result.expect("completion result");
    let response: CompletionResponse =
        serde_json::from_value(value).expect("completion parses");
    let items: Vec<CompletionItem> = match response {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Counter"), "labels should include Counter: {:?}", labels);
    assert!(labels.contains(&"Vault"), "labels should include Vault: {:?}", labels);
    assert!(
        labels.contains(&"Mint"),
        "define_account! types should appear in completion too: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"Increment"),
        "AccountsStruct items should not show up as account-type completions: {:?}",
        labels
    );
}

#[test]
fn completion_outside_account_type_arg_returns_null() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let inc_src = "\
#[derive(Accounts)]\n\
pub struct Increment<'info> {\n\
    pub counter: &'info mut Account<Counter>,\n\
}\n\
";
    let inc_uri = Uri::from_str("file:///tmp/instructions.rs").unwrap();
    open(&client, inc_uri.clone(), inc_src);
    await_initial_diagnostics(&client, &inc_uri);

    let col = column_of(inc_src, 1, "pub");
    send_request::<Completion>(
        &client,
        110,
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: inc_uri.clone() },
                position: Position { line: 1, character: col },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        },
    );
    let resp = await_response(&client, 110);
    assert!(
        matches!(&resp.result, Some(v) if v.is_null()),
        "completion outside Account<...> region should be null, got {:?}",
        resp.result
    );
}

#[test]
fn references_finds_account_type_uses_across_files() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let state_src = "\
#[account(discriminator = 1)]\n\
pub struct Counter { pub n: u64 }\n\
";
    let a_src = "\
#[derive(Accounts)]\n\
pub struct Inc<'info> {\n\
    pub counter: &'info mut Account<Counter>,\n\
}\n\
";
    let b_src = "\
#[derive(Accounts)]\n\
pub struct Dec<'info> {\n\
    pub counter: &'info mut Account<Counter>,\n\
}\n\
";
    let state_uri = Uri::from_str("file:///tmp/state.rs").unwrap();
    let a_uri = Uri::from_str("file:///tmp/inc.rs").unwrap();
    let b_uri = Uri::from_str("file:///tmp/dec.rs").unwrap();
    open(&client, state_uri.clone(), state_src);
    open(&client, a_uri.clone(), a_src);
    open(&client, b_uri.clone(), b_src);
    await_initial_diagnostics(&client, &state_uri);
    await_initial_diagnostics(&client, &a_uri);
    await_initial_diagnostics(&client, &b_uri);

    // Cursor on Counter inside Account<Counter> in inc.rs
    let col = column_of(a_src, 2, "Counter");
    send_request::<References>(
        &client,
        200,
        ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: a_uri.clone() },
                position: Position { line: 2, character: col },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: true,
            },
        },
    );
    let resp = await_response(&client, 200);
    let locations: Vec<Location> =
        serde_json::from_value(resp.result.expect("references result")).unwrap();

    let uris: Vec<&Uri> = locations.iter().map(|l| &l.uri).collect();
    assert!(
        uris.contains(&&a_uri),
        "inc.rs should be in references: {:?}",
        uris
    );
    assert!(
        uris.contains(&&b_uri),
        "dec.rs should be in references: {:?}",
        uris
    );
    assert!(
        uris.contains(&&state_uri),
        "state.rs declaration site should be included: {:?}",
        uris
    );
}

#[test]
fn references_without_include_declaration_omits_def_site() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let state_src = "\
#[account(discriminator = 1)]\n\
pub struct Counter { pub n: u64 }\n\
";
    let inc_src = "\
#[derive(Accounts)]\n\
pub struct Inc<'info> {\n\
    pub counter: &'info mut Account<Counter>,\n\
}\n\
";
    let state_uri = Uri::from_str("file:///tmp/state.rs").unwrap();
    let inc_uri = Uri::from_str("file:///tmp/inc.rs").unwrap();
    open(&client, state_uri.clone(), state_src);
    open(&client, inc_uri.clone(), inc_src);
    await_initial_diagnostics(&client, &state_uri);
    await_initial_diagnostics(&client, &inc_uri);

    let col = column_of(inc_src, 2, "Counter");
    send_request::<References>(
        &client,
        210,
        ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: inc_uri.clone() },
                position: Position { line: 2, character: col },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: false,
            },
        },
    );
    let resp = await_response(&client, 210);
    let locations: Vec<Location> =
        serde_json::from_value(resp.result.expect("references result")).unwrap();
    assert!(
        locations.iter().all(|l| l.uri != state_uri),
        "definition site should be excluded when include_declaration=false: {:?}",
        locations.iter().map(|l| &l.uri).collect::<Vec<_>>()
    );
    assert!(
        locations.iter().any(|l| l.uri == inc_uri),
        "use site in inc.rs should still appear"
    );
}

#[test]
fn document_symbol_lists_both_account_and_accounts_struct() {
    let (client, _handle) = spawn_server();
    handshake(&client);

    let src = "\
#[account(discriminator = 1)]\n\
pub struct Counter { pub n: u64 }\n\
\n\
#[derive(Accounts)]\n\
pub struct Increment<'info> {\n\
    pub counter: &'info mut Account<Counter>,\n\
}\n\
";
    let uri = Uri::from_str("file:///tmp/mixed.rs").unwrap();
    open(&client, uri.clone(), src);
    await_initial_diagnostics(&client, &uri);

    send_request::<DocumentSymbolRequest>(
        &client,
        300,
        DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 300);
    let response: DocumentSymbolResponse =
        serde_json::from_value(resp.result.expect("docsymbol result")).unwrap();
    let symbols: Vec<DocumentSymbol> = match response {
        DocumentSymbolResponse::Nested(s) => s,
        DocumentSymbolResponse::Flat(_) => panic!("expected nested document symbols"),
    };
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Counter"), "Counter should be a symbol");
    assert!(names.contains(&"Increment"), "Increment should be a symbol");

    let counter_detail = symbols
        .iter()
        .find(|s| s.name == "Counter")
        .and_then(|s| s.detail.as_deref())
        .unwrap();
    assert_eq!(counter_detail, "#[account]");

    let inc_detail = symbols
        .iter()
        .find(|s| s.name == "Increment")
        .and_then(|s| s.detail.as_deref())
        .unwrap();
    assert_eq!(inc_detail, "#[derive(Accounts)]");
}
