//! Server state and main loop.
//!
//! Threading model:
//!   - Main thread runs the JSON-RPC I/O loop and owns the [`Database`]
//!     + [`Vfs`]. All writes (didOpen/didChange/didClose) happen here,
//!     synchronously, which bumps Salsa's revision.
//!   - A [`ThreadPool`] sized to `num_cpus` handles reads — diagnostic
//!     recomputation and request handlers. Workers clone the database
//!     (cheap, Arc-internal) and may be implicitly cancelled by Salsa
//!     when the revision moves underneath them.
//!   - A dedicated background pool (size 1) services low-priority work
//!     like `cargo metadata` so workspace discovery doesn't compete with
//!     interactive request latency.
//!
//! Background workers communicate back via a private channel; the main
//! loop selects between the LSP receiver and the internal channel so
//! workspace-loaded events arrive in the same single-threaded state
//! mutation point as didChange / didOpen.

use crate::cargo_workspace::{self, WorkspaceConfig};
use crate::diagnostics::convert;
use crate::handlers;
use crate::snapshot::Snapshot;
use crate::vfs::Vfs;
use crossbeam_channel::{select, Receiver, Sender};
use lsp_server::{
    Connection, ErrorCode, Message, Notification as LspNotification, Request, Response,
    ResponseError,
};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification as _,
    Progress, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, CodeLensRequest, Completion, DocumentHighlightRequest,
    DocumentSymbolRequest, GotoDefinition, HoverRequest, InlayHintRequest, References,
    Request as _, SemanticTokensFullRequest, WorkspaceSymbolRequest,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    NumberOrString, ProgressParams, ProgressParamsValue, PublishDiagnosticsParams, Uri,
    WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressReport,
};
use quasar_hir::{line_index_for, parse_file, resolve_account_refs, Database, File, Workspace};
use salsa::Setter;
use serde::{de::DeserializeOwned, Serialize};
use std::error::Error;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use threadpool::ThreadPool;

const CONTENT_MODIFIED: i32 = -32801;
const WORKSPACE_LOAD_TOKEN: &str = "quasar/workspace-load";

enum InternalEvent {
    WorkspaceLoaded(Result<WorkspaceConfig, String>),
}

pub struct Server {
    db: Database,
    vfs: Vfs,
    workspace: Workspace,
    workspace_config: Option<WorkspaceConfig>,
    pool: ThreadPool,
    background: ThreadPool,
    sender: Sender<Message>,
    internal_sender: Sender<InternalEvent>,
    internal_receiver: Receiver<InternalEvent>,
}

impl Server {
    pub fn new(sender: Sender<Message>) -> Self {
        Self::with_workspace_root(sender, None)
    }

    /// Test helper: skip the background cargo-metadata load and install a
    /// pre-built [`WorkspaceConfig`] directly.
    pub fn for_test_with_config(sender: Sender<Message>, config: WorkspaceConfig) -> Self {
        let mut server = Self::with_workspace_root(sender, None);
        server.workspace_config = Some(config);
        server
    }

    /// Construct a server with an optional workspace root from the
    /// `initialize` handshake. When provided, `cargo metadata` runs in the
    /// background and populates [`WorkspaceConfig`] before any activation
    /// gating kicks in.
    pub fn with_workspace_root(sender: Sender<Message>, root: Option<PathBuf>) -> Self {
        let mut db = Database::default();
        let workspace = Workspace::new(&db, Vec::new());
        let _ = &mut db;
        let worker_count = num_cpus::get().max(1);
        let (internal_sender, internal_receiver) = crossbeam_channel::unbounded();
        let server = Self {
            db,
            vfs: Vfs::default(),
            workspace,
            workspace_config: None,
            pool: ThreadPool::with_name("quasar-lsp-worker".into(), worker_count),
            background: ThreadPool::with_name("quasar-lsp-bg".into(), 1),
            sender,
            internal_sender,
            internal_receiver,
        };
        if let Some(root) = root {
            server.schedule_workspace_load(root);
        }
        server
    }

    pub fn run(mut self, connection: &Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
        tracing::info!(
            workers = self.pool.max_count(),
            "quasar-lsp ready"
        );

        let lsp_rx = connection.receiver.clone();
        let internal_rx = self.internal_receiver.clone();

        loop {
            select! {
                recv(lsp_rx) -> msg => {
                    let Ok(msg) = msg else { return Ok(()) };
                    match msg {
                        Message::Request(req) => {
                            if connection.handle_shutdown(&req)? {
                                tracing::info!("shutdown received");
                                return Ok(());
                            }
                            self.handle_request(req);
                        }
                        Message::Notification(notif) => {
                            self.handle_notification(notif);
                        }
                        Message::Response(_) => {}
                    }
                }
                recv(internal_rx) -> evt => {
                    let Ok(evt) = evt else { continue };
                    self.handle_internal(evt);
                }
            }
        }
    }

    fn handle_request(&self, req: Request) {
        match req.method.as_str() {
            HoverRequest::METHOD => {
                self.dispatch::<HoverRequest>(req, handlers::handle_hover);
            }
            GotoDefinition::METHOD => {
                self.dispatch::<GotoDefinition>(req, handlers::handle_definition);
            }
            Completion::METHOD => {
                self.dispatch::<Completion>(req, handlers::handle_completion);
            }
            References::METHOD => {
                self.dispatch::<References>(req, handlers::handle_references);
            }
            DocumentSymbolRequest::METHOD => {
                self.dispatch::<DocumentSymbolRequest>(req, handlers::handle_document_symbol);
            }
            SemanticTokensFullRequest::METHOD => {
                self.dispatch::<SemanticTokensFullRequest>(
                    req,
                    handlers::handle_semantic_tokens_full,
                );
            }
            InlayHintRequest::METHOD => {
                self.dispatch::<InlayHintRequest>(req, handlers::handle_inlay_hint);
            }
            CodeActionRequest::METHOD => {
                self.dispatch::<CodeActionRequest>(req, handlers::handle_code_action);
            }
            CodeLensRequest::METHOD => {
                self.dispatch::<CodeLensRequest>(req, handlers::handle_code_lens);
            }
            WorkspaceSymbolRequest::METHOD => {
                self.dispatch::<WorkspaceSymbolRequest>(req, handlers::handle_workspace_symbol);
            }
            DocumentHighlightRequest::METHOD => {
                self.dispatch::<DocumentHighlightRequest>(req, handlers::handle_document_highlight);
            }
            _ => self.send_method_not_found(req),
        }
    }

    fn dispatch<R>(
        &self,
        req: Request,
        handler: fn(&Snapshot, R::Params) -> R::Result,
    ) where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + Send + 'static,
        R::Result: Serialize + Send + 'static,
    {
        let snapshot = self.snapshot();
        let sender = self.sender.clone();
        let id = req.id.clone();
        let params_value = req.params;

        self.pool.execute(move || {
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let params: R::Params = match serde_json::from_value(params_value) {
                    Ok(p) => p,
                    Err(err) => {
                        return Err(ResponseError {
                            code: ErrorCode::InvalidParams as i32,
                            message: err.to_string(),
                            data: None,
                        });
                    }
                };
                Ok(handler(&snapshot, params))
            }));

            let response = match outcome {
                Ok(Ok(result)) => Response {
                    id,
                    result: Some(serde_json::to_value(result).expect("result serialise")),
                    error: None,
                },
                Ok(Err(error)) => Response {
                    id,
                    result: None,
                    error: Some(error),
                },
                Err(_) => Response {
                    id,
                    result: None,
                    error: Some(ResponseError {
                        code: CONTENT_MODIFIED,
                        message: "content modified during request".into(),
                        data: None,
                    }),
                },
            };
            let _ = sender.send(Message::Response(response));
        });
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            db: self.db.clone(),
            workspace: self.workspace,
            uri_to_file: self.vfs.uri_to_file(),
        }
    }

    fn send_method_not_found(&self, req: Request) {
        let resp = Response {
            id: req.id,
            result: None,
            error: Some(ResponseError {
                code: ErrorCode::MethodNotFound as i32,
                message: format!("method not implemented: {}", req.method),
                data: None,
            }),
        };
        let _ = self.sender.send(Message::Response(resp));
    }

    fn handle_notification(&mut self, notif: LspNotification) {
        match notif.method.as_str() {
            DidOpenTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidOpenTextDocumentParams>(notif.params)
                {
                    self.on_did_open(params);
                }
            }
            DidChangeTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidChangeTextDocumentParams>(notif.params)
                {
                    self.on_did_change(params);
                }
            }
            DidCloseTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidCloseTextDocumentParams>(notif.params)
                {
                    self.on_did_close(params);
                }
            }
            _ => {}
        }
    }

    fn handle_internal(&mut self, evt: InternalEvent) {
        match evt {
            InternalEvent::WorkspaceLoaded(Ok(config)) => {
                let crate_count = config.quasar_crate_roots.len();
                tracing::info!(crates = crate_count, "workspace loaded");
                self.workspace_config = Some(config);
                self.refresh_workspace_membership();
                self.send_progress_end(WORKSPACE_LOAD_TOKEN, "loaded");
            }
            InternalEvent::WorkspaceLoaded(Err(err)) => {
                tracing::warn!(error = %err, "workspace load failed; serving all open files");
                self.send_progress_end(WORKSPACE_LOAD_TOKEN, "load failed; degraded mode");
            }
        }
    }

    fn on_did_open(&mut self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        if !self.is_quasar_file(&uri) {
            // Make sure stale diagnostics on this URI are cleared.
            publish(&self.sender, uri, vec![]);
            return;
        }
        let text: Arc<str> = Arc::from(params.text_document.text.as_str());
        let file = self.vfs.intern(&mut self.db, uri.clone(), text);
        self.vfs.mark_open(uri.clone());
        self.update_workspace();
        self.schedule_diagnostics(uri, file);
    }

    fn on_did_change(&mut self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if !self.is_quasar_file(&uri) {
            return;
        }
        let Some(full) = params.content_changes.into_iter().last() else {
            return;
        };
        let text: Arc<str> = Arc::from(full.text.as_str());
        let file = match self.vfs.set_text(&mut self.db, &uri, text.clone()) {
            Some(file) => file,
            None => self.vfs.intern(&mut self.db, uri.clone(), text),
        };
        self.schedule_diagnostics(uri, file);
    }

    fn on_did_close(&mut self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.vfs.mark_closed(&uri);
        self.update_workspace();
        publish(&self.sender, uri, vec![]);
    }

    fn update_workspace(&mut self) {
        let files = self.vfs.open_files();
        self.workspace.set_files(&mut self.db).to(files);
    }

    /// Called after cargo metadata lands. Any open file outside the
    /// discovered Quasar crate roots is dropped from the active set;
    /// previously-emitted diagnostics on those URIs are cleared.
    fn refresh_workspace_membership(&mut self) {
        let Some(config) = self.workspace_config.clone() else {
            return;
        };
        let snapshot = self.vfs.uri_to_file();
        let mut to_close: Vec<Uri> = Vec::new();
        for (uri, _file) in snapshot.iter() {
            if !uri_covered_by(&config, uri) {
                to_close.push(uri.clone());
            }
        }
        for uri in to_close {
            self.vfs.mark_closed(&uri);
            publish(&self.sender, uri, vec![]);
        }
        self.update_workspace();
    }

    fn schedule_diagnostics(&self, uri: Uri, file: File) {
        let snapshot = self.db.clone();
        let workspace = self.workspace;
        let sender = self.sender.clone();
        self.pool.execute(move || {
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                compute_and_publish(&snapshot, workspace, file, uri.clone(), &sender);
            }));
            if result.is_err() {
                tracing::debug!("diagnostic worker cancelled or panicked for {}", uri.as_str());
            }
        });
    }

    fn schedule_workspace_load(&self, root: PathBuf) {
        self.send_progress_begin(WORKSPACE_LOAD_TOKEN, "Loading Quasar workspace");
        let internal = self.internal_sender.clone();
        self.background.execute(move || {
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                cargo_workspace::load_workspace(&root)
            }));
            let result = match outcome {
                Ok(Ok(cfg)) => Ok(cfg),
                Ok(Err(err)) => Err(err.to_string()),
                Err(_) => Err("cargo metadata panicked".to_string()),
            };
            let _ = internal.send(InternalEvent::WorkspaceLoaded(result));
        });
    }

    fn is_quasar_file(&self, uri: &Uri) -> bool {
        match &self.workspace_config {
            Some(cfg) => uri_covered_by(cfg, uri),
            None => true,
        }
    }

    fn send_progress_begin(&self, token: &str, title: &str) {
        let notif = LspNotification {
            method: Progress::METHOD.to_string(),
            params: serde_json::to_value(ProgressParams {
                token: NumberOrString::String(token.to_string()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: title.to_string(),
                        cancellable: Some(false),
                        message: None,
                        percentage: None,
                    },
                )),
            })
            .expect("progress params"),
        };
        let _ = self.sender.send(Message::Notification(notif));
    }

    #[allow(dead_code)]
    fn send_progress_report(&self, token: &str, message: &str) {
        let notif = LspNotification {
            method: Progress::METHOD.to_string(),
            params: serde_json::to_value(ProgressParams {
                token: NumberOrString::String(token.to_string()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                    WorkDoneProgressReport {
                        cancellable: Some(false),
                        message: Some(message.to_string()),
                        percentage: None,
                    },
                )),
            })
            .expect("progress params"),
        };
        let _ = self.sender.send(Message::Notification(notif));
    }

    fn send_progress_end(&self, token: &str, message: &str) {
        let notif = LspNotification {
            method: Progress::METHOD.to_string(),
            params: serde_json::to_value(ProgressParams {
                token: NumberOrString::String(token.to_string()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                    message: Some(message.to_string()),
                })),
            })
            .expect("progress params"),
        };
        let _ = self.sender.send(Message::Notification(notif));
    }
}

fn uri_covered_by(cfg: &WorkspaceConfig, uri: &Uri) -> bool {
    let Some(path) = uri_to_file_path(uri) else {
        return false;
    };
    cfg.covers(&path)
}

/// Lossy file URI → filesystem path. Supports `file://` URIs on Unix-style
/// paths; Windows drive-letter parsing is intentionally minimal and may need
/// hardening with a real URL parser later.
pub(crate) fn uri_to_file_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str();
    let stripped = s.strip_prefix("file://")?;
    Some(PathBuf::from(stripped))
}

fn compute_and_publish(
    db: &Database,
    workspace: Workspace,
    file: File,
    uri: Uri,
    sender: &Sender<Message>,
) {
    let parsed = parse_file(db, file);
    let resolved = resolve_account_refs(db, workspace, file);

    let line_index = line_index_for(db, file);
    let text = file.text(db).clone();

    let mut diagnostics: Vec<lsp_types::Diagnostic> = Vec::new();
    for d in parsed.diagnostics(db).iter() {
        diagnostics.push(convert(d, &text, &line_index, &uri));
    }
    for d in resolved.diagnostics(db).iter() {
        diagnostics.push(convert(d, &text, &line_index, &uri));
    }

    publish(sender, uri, diagnostics);
}

fn publish(sender: &Sender<Message>, uri: Uri, diagnostics: Vec<lsp_types::Diagnostic>) {
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    let notif = LspNotification {
        method: PublishDiagnostics::METHOD.to_string(),
        params: serde_json::to_value(params).expect("params serializable"),
    };
    let _ = sender.send(Message::Notification(notif));
}
