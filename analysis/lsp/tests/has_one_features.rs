//! has_one LSP features: diagnostics, goto-definition to the account-type
//! field, and the "add field to account type" code action.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{CodeActionRequest, GotoDefinition, Initialize, Request as _};
use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Diagnostic,
    DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
    InitializedParams, Location, PartialResultParams, Position, PublishDiagnosticsParams,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri,
    WorkDoneProgressParams,
};
use quasar_lsp::{capabilities::server_capabilities, Server, WorkspaceConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

const VAULT_URI: &str = "file:///tmp/ws/state.rs";
const IX_URI: &str = "file:///tmp/ws/ix.rs";

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

fn open(conn: &Connection, uri: &str, text: &str) {
    conn.sender
        .send(Message::Notification(Notification {
            method: DidOpenTextDocument::METHOD.to_string(),
            params: serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Uri::from_str(uri).unwrap(),
                    language_id: "rust".into(),
                    version: 1,
                    text: text.into(),
                },
            })
            .unwrap(),
        }))
        .unwrap();
}

/// Collect publishes until the given uri has been seen (latest wins).
fn collect_diagnostics(conn: &Connection, uri: &Uri) -> Vec<Diagnostic> {
    let mut latest: HashMap<Uri, Vec<Diagnostic>> = HashMap::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        if latest.contains_key(uri) {
            return latest.remove(uri).unwrap();
        }
        let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
            return Vec::new();
        };
        let Ok(msg) = conn.receiver.recv_timeout(remaining) else {
            return Vec::new();
        };
        if let Message::Notification(n) = msg {
            if n.method == PublishDiagnostics::METHOD {
                if let Ok(p) = serde_json::from_value::<PublishDiagnosticsParams>(n.params) {
                    latest.insert(p.uri, p.diagnostics);
                }
            }
        }
    }
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

fn column_of(text: &str, line: u32, needle: &str) -> u32 {
    let line_text = text.lines().nth(line as usize).expect("line");
    let byte_col = line_text.find(needle).expect("needle");
    line_text[..byte_col].encode_utf16().count() as u32
}

const VAULT: &str = "#[account(discriminator = 3)]\npub struct Vault {\n    pub authority: Address,\n    pub amount: u64,\n}\n";

#[test]
fn has_one_missing_field_publishes_diagnostic() {
    let client = spawn();
    handshake(&client);
    open(&client, VAULT_URI, VAULT);
    // `manager` is a sibling binding but Vault has no `manager` field.
    let ix = "#[derive(Accounts)]\npub struct C<'info> {\n    pub manager: Signer,\n    #[account(has_one(manager))]\n    pub vault: &'info Account<Vault>,\n}\n";
    open(&client, IX_URI, ix);

    let diags = collect_diagnostics(&client, &Uri::from_str(IX_URI).unwrap());
    let codes: Vec<_> = diags
        .iter()
        .filter_map(|d| match d.code.as_ref()? {
            lsp_types::NumberOrString::String(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        codes.iter().any(|c| c.contains("has_one_missing_account_field")),
        "expected has_one missing-field diagnostic, got {:?}",
        codes
    );
}

#[test]
fn has_one_target_goto_definition_jumps_to_account_field() {
    let client = spawn();
    handshake(&client);
    open(&client, VAULT_URI, VAULT);
    let ix = "#[derive(Accounts)]\npub struct C<'info> {\n    pub authority: Signer,\n    #[account(has_one(authority))]\n    pub vault: &'info Account<Vault>,\n}\n";
    open(&client, IX_URI, ix);
    let _ = collect_diagnostics(&client, &Uri::from_str(IX_URI).unwrap());

    // Cursor on `authority` inside has_one(authority) — line 3 of ix.
    let col = column_of(ix, 3, "authority");
    send_request::<GotoDefinition>(
        &client,
        50,
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Uri::from_str(IX_URI).unwrap(),
                },
                position: Position { line: 3, character: col },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 50);
    let response: GotoDefinitionResponse =
        serde_json::from_value(resp.result.expect("goto result")).unwrap();
    let loc: Location = match response {
        GotoDefinitionResponse::Scalar(l) => l,
        other => panic!("expected scalar location, got {:?}", other),
    };
    assert_eq!(loc.uri, Uri::from_str(VAULT_URI).unwrap());
    // Vault.authority is on line 2 (zero-indexed) of VAULT.
    assert_eq!(loc.range.start.line, 2, "should jump to Vault.authority field");
}

#[test]
fn add_field_code_action_targets_account_type() {
    let client = spawn();
    handshake(&client);
    open(&client, VAULT_URI, VAULT);
    let ix = "#[derive(Accounts)]\npub struct C<'info> {\n    pub manager: Signer,\n    #[account(has_one(manager))]\n    pub vault: &'info Account<Vault>,\n}\n";
    open(&client, IX_URI, ix);

    let diags = collect_diagnostics(&client, &Uri::from_str(IX_URI).unwrap());
    let missing = diags
        .iter()
        .find(|d| {
            matches!(&d.code, Some(lsp_types::NumberOrString::String(s))
                if s.contains("has_one_missing_account_field"))
        })
        .expect("missing-field diagnostic")
        .clone();

    send_request::<CodeActionRequest>(
        &client,
        60,
        CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str(IX_URI).unwrap(),
            },
            range: missing.range,
            context: CodeActionContext {
                diagnostics: vec![missing.clone()],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 60);
    let response: CodeActionResponse =
        serde_json::from_value(resp.result.expect("code action result")).unwrap();

    let action = response
        .iter()
        .find_map(|c| match c {
            CodeActionOrCommand::CodeAction(a) if a.title.contains("Add field `manager`") => {
                Some(a.clone())
            }
            _ => None,
        })
        .expect("add-field action");

    // The edit must target Vault's file (state.rs), not the instructions file.
    let edit = action.edit.expect("workspace edit");
    let changes = edit.changes.expect("changes");
    assert!(
        changes.contains_key(&Uri::from_str(VAULT_URI).unwrap()),
        "edit should target the account type's file (state.rs)"
    );
    let edits = &changes[&Uri::from_str(VAULT_URI).unwrap()];
    assert!(
        edits[0].new_text.contains("manager"),
        "inserted text should declare the manager field"
    );
}
