//! End-to-end coverage for the final six v1 capabilities: semantic tokens,
//! inlay hints, code actions, code lens, workspace symbol, document highlight.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, CodeLensRequest, DocumentHighlightRequest, InlayHintRequest, Initialize,
    Request as _, SemanticTokensFullRequest, WorkspaceSymbolRequest,
};
use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeActionResponse, CodeLens,
    CodeLensParams, DidOpenTextDocumentParams, DocumentHighlight, DocumentHighlightParams,
    InitializeParams, InitializedParams, InlayHint, InlayHintLabel, InlayHintParams,
    PartialResultParams, Position, PublishDiagnosticsParams, Range, SemanticTokens,
    SemanticTokensParams, SemanticTokensResult, SymbolInformation, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
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
fn semantic_tokens_emits_directive_keywords() {
    let (client, _h) = spawn_server();
    handshake(&client);
    let src = "#[account(discriminator = 1, set_inner)]\npub struct Counter { pub n: u64 }\n";
    let uri = Uri::from_str("file:///tmp/sem.rs").unwrap();
    open(&client, uri.clone(), src);
    await_initial_diagnostics(&client, &uri);

    send_request::<SemanticTokensFullRequest>(
        &client,
        400,
        SemanticTokensParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 400);
    let response: SemanticTokensResult =
        serde_json::from_value(resp.result.expect("tokens result")).unwrap();
    let tokens: SemanticTokens = match response {
        SemanticTokensResult::Tokens(t) => t,
        _ => panic!("expected tokens"),
    };
    // Two known keywords on the first line: `discriminator` and `set_inner`.
    assert!(
        tokens.data.len() >= 2,
        "expected at least two keyword tokens, got {} entries",
        tokens.data.len()
    );
}

#[test]
fn inlay_hint_shows_discriminator_at_account_ref() {
    let (client, _h) = spawn_server();
    handshake(&client);

    let state_src = "#[account(discriminator = 7)]\npub struct Counter { pub n: u64 }\n";
    let inc_src = "#[derive(Accounts)]\npub struct Inc<'info> {\n    pub counter: &'info mut Account<Counter>,\n}\n";
    let state_uri = Uri::from_str("file:///tmp/state.rs").unwrap();
    let inc_uri = Uri::from_str("file:///tmp/inc.rs").unwrap();
    open(&client, state_uri.clone(), state_src);
    open(&client, inc_uri.clone(), inc_src);
    await_initial_diagnostics(&client, &state_uri);
    await_initial_diagnostics(&client, &inc_uri);

    send_request::<InlayHintRequest>(
        &client,
        500,
        InlayHintParams {
            text_document: TextDocumentIdentifier { uri: inc_uri.clone() },
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 100, character: 0 },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        },
    );
    let resp = await_response(&client, 500);
    let hints: Vec<InlayHint> =
        serde_json::from_value(resp.result.expect("inlay result")).unwrap();
    assert!(!hints.is_empty(), "expected at least one inlay hint");
    let label = match &hints[0].label {
        InlayHintLabel::String(s) => s.clone(),
        InlayHintLabel::LabelParts(parts) => {
            parts.iter().map(|p| p.value.as_str()).collect::<String>()
        }
    };
    assert!(
        label.contains("[7]"),
        "inlay hint should contain `[7]`, got: {}",
        label
    );
}

#[test]
fn code_action_offers_account_attribute_insertion_on_bare_struct() {
    let (client, _h) = spawn_server();
    handshake(&client);
    let src = "pub struct Counter { pub n: u64 }\n";
    let uri = Uri::from_str("file:///tmp/bare.rs").unwrap();
    open(&client, uri.clone(), src);
    await_initial_diagnostics(&client, &uri);

    let col = column_of(src, 0, "Counter");
    send_request::<CodeActionRequest>(
        &client,
        600,
        CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position { line: 0, character: col },
                end: Position { line: 0, character: col },
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 600);
    let response: CodeActionResponse =
        serde_json::from_value(resp.result.expect("code action result")).unwrap();
    assert!(
        response.iter().any(|c| match c {
            CodeActionOrCommand::CodeAction(a) =>
                a.title.contains("#[account(discriminator = 1)]"),
            _ => false,
        }),
        "expected an insertion action, got {:?}",
        response.iter().map(|c| match c {
            CodeActionOrCommand::CodeAction(a) => &a.title,
            CodeActionOrCommand::Command(c) => &c.title,
        }).collect::<Vec<_>>()
    );
}

#[test]
fn code_lens_reports_reference_count_above_account_type() {
    let (client, _h) = spawn_server();
    handshake(&client);
    let state_src = "#[account(discriminator = 1)]\npub struct Counter { pub n: u64 }\n";
    let inc_src = "#[derive(Accounts)]\npub struct Inc<'info> {\n    pub counter: &'info mut Account<Counter>,\n}\n";
    let inc2_src = "#[derive(Accounts)]\npub struct Dec<'info> {\n    pub counter: &'info mut Account<Counter>,\n}\n";

    let state_uri = Uri::from_str("file:///tmp/state.rs").unwrap();
    let inc_uri = Uri::from_str("file:///tmp/inc.rs").unwrap();
    let inc2_uri = Uri::from_str("file:///tmp/dec.rs").unwrap();
    open(&client, state_uri.clone(), state_src);
    open(&client, inc_uri.clone(), inc_src);
    open(&client, inc2_uri.clone(), inc2_src);
    await_initial_diagnostics(&client, &state_uri);
    await_initial_diagnostics(&client, &inc_uri);
    await_initial_diagnostics(&client, &inc2_uri);

    send_request::<CodeLensRequest>(
        &client,
        700,
        CodeLensParams {
            text_document: TextDocumentIdentifier { uri: state_uri.clone() },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 700);
    let lenses: Vec<CodeLens> =
        serde_json::from_value(resp.result.expect("codelens result")).unwrap();
    assert_eq!(lenses.len(), 1);
    let title = lenses[0]
        .command
        .as_ref()
        .map(|c| c.title.as_str())
        .unwrap_or("");
    assert_eq!(title, "2 references", "got: {}", title);
}

#[test]
fn workspace_symbol_filters_by_query_substring() {
    let (client, _h) = spawn_server();
    handshake(&client);

    let src_a = "#[account(discriminator = 1)]\npub struct Counter { pub n: u64 }\n";
    let src_b = "#[account(discriminator = 2)]\npub struct Vault { pub n: u64 }\n";
    let src_c =
        "#[derive(Accounts)]\npub struct Increment<'info> { pub counter: &'info Account<Counter> }\n";
    let a_uri = Uri::from_str("file:///tmp/a.rs").unwrap();
    let b_uri = Uri::from_str("file:///tmp/b.rs").unwrap();
    let c_uri = Uri::from_str("file:///tmp/c.rs").unwrap();
    open(&client, a_uri.clone(), src_a);
    open(&client, b_uri.clone(), src_b);
    open(&client, c_uri.clone(), src_c);
    await_initial_diagnostics(&client, &a_uri);
    await_initial_diagnostics(&client, &b_uri);
    await_initial_diagnostics(&client, &c_uri);

    send_request::<WorkspaceSymbolRequest>(
        &client,
        800,
        WorkspaceSymbolParams {
            query: "Cou".to_string(),
            partial_result_params: PartialResultParams::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        },
    );
    let resp = await_response(&client, 800);
    let response: WorkspaceSymbolResponse =
        serde_json::from_value(resp.result.expect("workspace symbol result")).unwrap();
    let symbols: Vec<SymbolInformation> = match response {
        WorkspaceSymbolResponse::Flat(s) => s,
        WorkspaceSymbolResponse::Nested(_) => panic!("expected flat response"),
    };
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Counter"), "Counter should match query \"Cou\"");
    assert!(!names.contains(&"Vault"), "Vault should NOT match \"Cou\"");
}

#[test]
fn document_highlight_finds_same_file_uses() {
    let (client, _h) = spawn_server();
    handshake(&client);

    let src = "#[account(discriminator = 1)]\npub struct Counter { pub n: u64 }\n\n#[derive(Accounts)]\npub struct A<'info> { pub a: &'info Account<Counter>, pub b: &'info Account<Counter> }\n";
    let uri = Uri::from_str("file:///tmp/multi.rs").unwrap();
    open(&client, uri.clone(), src);
    await_initial_diagnostics(&client, &uri);

    // Cursor on Counter inside the Accounts struct (line 4).
    let col = column_of(src, 4, "Counter");
    send_request::<DocumentHighlightRequest>(
        &client,
        900,
        DocumentHighlightParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line: 4, character: col },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 900);
    let highlights: Vec<DocumentHighlight> =
        serde_json::from_value(resp.result.expect("highlight result")).unwrap();
    // Expect: two use sites on line 4 plus the definition on line 1 = 3.
    assert!(
        highlights.len() >= 3,
        "expected at least 3 highlights (2 uses + 1 def), got {}",
        highlights.len()
    );
}
