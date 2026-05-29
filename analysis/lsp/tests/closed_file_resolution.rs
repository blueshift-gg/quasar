//! Cross-file resolution into *closed* workspace members.
//!
//! The defining file (`state.rs`, declaring `#[account] Escrow`) is never
//! opened in the editor — it is only registered from disk via
//! [`WorkspaceConfig::indexed_source_files`]. The instructions file
//! (`make.rs`) that references `Account<Escrow>` is the only open buffer.
//! These tests assert that has_one field checks, goto-definition, and
//! references all resolve into the closed file, which they could not before
//! member sources were indexed (the symbol index held open buffers only).

// `Uri` map keys are safe: fluent-uri's interior cache doesn't affect Hash/Eq.
#![allow(clippy::mutable_key_type)]

use {
    lsp_server::{Connection, Message, Notification, Request, RequestId, Response},
    lsp_types::{
        notification::{DidOpenTextDocument, Initialized, Notification as _, PublishDiagnostics},
        request::{GotoDefinition, Initialize, References, Request as _},
        DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
        InitializedParams, Location, PartialResultParams, Position, PublishDiagnosticsParams,
        ReferenceContext, ReferenceParams, TextDocumentIdentifier, TextDocumentItem,
        TextDocumentPositionParams, Uri, WorkDoneProgressParams,
    },
    quasar_lsp::{capabilities::server_capabilities, Server, WorkspaceConfig},
    std::{
        collections::HashMap,
        path::PathBuf,
        str::FromStr,
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    },
};

const ESCROW: &str = "\
#[account(discriminator = 1)]
pub struct Escrow {
    pub maker: Address,
    pub amount: u64,
}
";

// `escrow` carries `has_one(maker)` (maker IS an Escrow field → resolves);
// `other` carries `has_one(taker)` (taker is a sibling binding but NOT an
// Escrow field → missing-field diagnostic). Both reference `Account<Escrow>`,
// whose definition lives in the unopened state.rs.
const MAKE: &str = "\
#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: Signer,
    pub taker: Signer,
    #[account(has_one(maker))]
    pub escrow: &'info Account<Escrow>,
    #[account(has_one(taker))]
    pub other: &'info Account<Escrow>,
}
";

/// A throwaway on-disk crate root with `src/state.rs` + `src/make.rs`,
/// removed on drop.
struct TempWs {
    dir: PathBuf,
    state_uri: Uri,
    make_uri: Uri,
}

impl Drop for TempWs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn unique_temp_ws() -> TempWs {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("quasar-lsp-closed-{}-{}", std::process::id(), n));
    let src = dir.join("src");
    std::fs::create_dir_all(&src).expect("create temp src dir");
    let state_path = src.join("state.rs");
    let make_path = src.join("make.rs");
    std::fs::write(&state_path, ESCROW).expect("write state.rs");
    std::fs::write(&make_path, MAKE).expect("write make.rs");
    TempWs {
        state_uri: path_uri(&state_path),
        make_uri: path_uri(&make_path),
        dir,
    }
}

fn path_uri(path: &std::path::Path) -> Uri {
    Uri::from_str(&format!("file://{}", path.to_str().unwrap())).unwrap()
}

/// Spawns a server whose only indexed account-type definition is the *closed*
/// state.rs registered through `indexed_source_files`. `known_account_types`
/// is deliberately empty so resolution can only succeed via member indexing.
fn spawn(ws: &TempWs) -> Connection {
    let state_path = uri_path(&ws.state_uri);
    let config = WorkspaceConfig {
        workspace_roots: vec![ws.dir.clone()],
        quasar_crate_roots: vec![ws.dir.clone()],
        known_account_types: Vec::new(),
        indexed_source_files: vec![state_path],
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

fn uri_path(uri: &Uri) -> PathBuf {
    PathBuf::from(uri.as_str().strip_prefix("file://").unwrap())
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

fn open(conn: &Connection, uri: &Uri, text: &str) {
    conn.sender
        .send(Message::Notification(Notification {
            method: DidOpenTextDocument::METHOD.to_string(),
            params: serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "rust".into(),
                    version: 1,
                    text: text.into(),
                },
            })
            .unwrap(),
        }))
        .unwrap();
}

fn collect_diagnostics(conn: &Connection, uri: &Uri) -> Vec<lsp_types::Diagnostic> {
    let mut latest: HashMap<Uri, Vec<lsp_types::Diagnostic>> = HashMap::new();
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

fn diag_codes(diags: &[lsp_types::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter_map(|d| match d.code.as_ref()? {
            lsp_types::NumberOrString::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn column_of(text: &str, line: u32, needle: &str) -> u32 {
    let line_text = text.lines().nth(line as usize).expect("line");
    let byte_col = line_text.find(needle).expect("needle");
    line_text[..byte_col].encode_utf16().count() as u32
}

#[test]
fn has_one_field_check_fires_with_defining_file_closed() {
    let ws = unique_temp_ws();
    let client = spawn(&ws);
    handshake(&client);
    // Only make.rs is opened; state.rs (defining Escrow) stays closed.
    open(&client, &ws.make_uri, MAKE);

    let diags = collect_diagnostics(&client, &ws.make_uri);
    let codes = diag_codes(&diags);

    assert!(
        codes
            .iter()
            .any(|c| c.contains("has_one_missing_account_field")),
        "has_one(taker) should be flagged as a missing field on the closed Escrow, got {:?}",
        codes
    );
    // Escrow itself must resolve (via the closed member), so it is NOT flagged
    // as an unknown account type.
    assert!(
        !codes.iter().any(|c| c.contains("unknown_account_type")),
        "Escrow resolves through the indexed closed file; should not be unknown, got {:?}",
        codes
    );
    // Exactly one missing-field diagnostic — has_one(maker) resolves cleanly.
    let missing = codes
        .iter()
        .filter(|c| c.contains("has_one_missing_account_field"))
        .count();
    assert_eq!(
        missing, 1,
        "only has_one(taker) should miss, got {:?}",
        codes
    );
}

#[test]
fn goto_definition_resolves_into_closed_file() {
    let ws = unique_temp_ws();
    let client = spawn(&ws);
    handshake(&client);
    open(&client, &ws.make_uri, MAKE);
    let _ = collect_diagnostics(&client, &ws.make_uri);

    // Cursor on `Escrow` inside `Account<Escrow>` on the `escrow` field (line 5).
    let col = column_of(MAKE, 5, "Escrow");
    send_request::<GotoDefinition>(
        &client,
        50,
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: ws.make_uri.clone(),
                },
                position: Position {
                    line: 5,
                    character: col,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    let resp = await_response(&client, 50);
    let response: GotoDefinitionResponse =
        serde_json::from_value(resp.result.expect("goto result")).unwrap();
    let loc = match response {
        GotoDefinitionResponse::Scalar(l) => l,
        other => panic!("expected scalar location into closed file, got {:?}", other),
    };
    assert_eq!(
        loc.uri, ws.state_uri,
        "goto should land in the closed state.rs"
    );
    // `pub struct Escrow` is on line 1 (zero-indexed) of ESCROW.
    assert_eq!(
        loc.range.start.line, 1,
        "should point at the Escrow declaration"
    );
}

#[test]
fn references_include_closed_file_declaration() {
    let ws = unique_temp_ws();
    let client = spawn(&ws);
    handshake(&client);
    open(&client, &ws.make_uri, MAKE);
    let _ = collect_diagnostics(&client, &ws.make_uri);

    let col = column_of(MAKE, 5, "Escrow");
    send_request::<References>(
        &client,
        70,
        ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: ws.make_uri.clone(),
                },
                position: Position {
                    line: 5,
                    character: col,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: true,
            },
        },
    );
    let resp = await_response(&client, 70);
    let locations: Vec<Location> =
        serde_json::from_value(resp.result.expect("references result")).unwrap();

    // Two `Account<Escrow>` uses in the open file, plus the declaration in the
    // closed file.
    let in_make = locations.iter().filter(|l| l.uri == ws.make_uri).count();
    assert_eq!(
        in_make, 2,
        "both Account<Escrow> uses in make.rs, got {:?}",
        locations
    );
    assert!(
        locations.iter().any(|l| l.uri == ws.state_uri),
        "the declaration in the closed state.rs should be included, got {:?}",
        locations
    );
}
