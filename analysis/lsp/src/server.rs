//! Server state and main loop.
//!
//! Threading model:
//!   - Main thread runs the JSON-RPC I/O loop and owns the [`Database`] and
//!     [`Vfs`]. All writes (didOpen/didChange/didClose) happen here,
//!     synchronously, which bumps Salsa's revision.
//!   - A [`ThreadPool`] sized to `num_cpus` handles reads — diagnostic
//!     recomputation and request handlers. Workers clone the database (cheap,
//!     Arc-internal) and may be implicitly cancelled by Salsa when the revision
//!     moves underneath them.
//!   - A dedicated background pool (size 1) services low-priority work like
//!     `cargo metadata` so workspace discovery doesn't compete with interactive
//!     request latency.
//!
//! Background workers communicate back via a private channel; the main
//! loop selects between the LSP receiver and the internal channel so
//! workspace-loaded events arrive in the same single-threaded state
//! mutation point as didChange / didOpen.

use {
    crate::{
        cargo_workspace::{self, WorkspaceConfig},
        diagnostics::convert,
        handlers,
        snapshot::Snapshot,
        vfs::Vfs,
    },
    crossbeam_channel::{select, Receiver, Sender},
    lsp_server::{
        Connection, ErrorCode, Message, Notification as LspNotification, Request, RequestId,
        Response, ResponseError,
    },
    lsp_types::{
        notification::{
            DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
            DidOpenTextDocument, Notification as _, Progress, PublishDiagnostics,
        },
        request::{
            CodeActionRequest, CodeLensRequest, Completion, DocumentHighlightRequest,
            DocumentSymbolRequest, GotoDefinition, HoverRequest, InlayHintRequest, References,
            RegisterCapability, Request as _, SemanticTokensFullRequest, WorkspaceSymbolRequest,
        },
        DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
        DidChangeWatchedFilesRegistrationOptions, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, FileChangeType, FileEvent, FileSystemWatcher, GlobPattern,
        NumberOrString, ProgressParams, ProgressParamsValue, PublishDiagnosticsParams,
        Registration, RegistrationParams, Uri, WorkDoneProgress, WorkDoneProgressBegin,
        WorkDoneProgressEnd, WorkDoneProgressReport,
    },
    quasar_hir::{
        line_index_for, parse_file, resolve_account_refs, resolve_has_one, validate_accounts,
        Database, File, Workspace,
    },
    salsa::Setter,
    serde::{de::DeserializeOwned, Serialize},
    std::{
        error::Error,
        panic::AssertUnwindSafe,
        path::{Path, PathBuf},
        sync::Arc,
    },
    threadpool::ThreadPool,
};

const CONTENT_MODIFIED: i32 = -32801;
const WORKSPACE_LOAD_TOKEN: &str = "quasar/workspace-load";

enum InternalEvent {
    WorkspaceLoaded(Option<WorkspaceConfig>),
}

pub struct Server {
    db: Database,
    vfs: Vfs,
    workspace: Workspace,
    workspace_config: Option<WorkspaceConfig>,
    workspace_roots: Vec<PathBuf>,
    supports_file_watching: bool,
    next_server_request_id: i32,
    pool: ThreadPool,
    background: ThreadPool,
    sender: Sender<Message>,
    internal_sender: Sender<InternalEvent>,
    internal_receiver: Receiver<InternalEvent>,
}

impl Server {
    pub fn new(sender: Sender<Message>) -> Self {
        Self::build(sender, Vec::new(), false)
    }

    /// Test helper: skip the background cargo-metadata load and install a
    /// pre-built [`WorkspaceConfig`] directly.
    pub fn for_test_with_config(sender: Sender<Message>, config: WorkspaceConfig) -> Self {
        let mut server = Self::build(sender, Vec::new(), false);
        server.apply_loaded_config(config);
        server
    }

    /// Construct a server with workspace roots from the `initialize`
    /// handshake. `cargo metadata` runs in the background for each root and
    /// populates [`WorkspaceConfig`] before activation gating kicks in.
    /// `supports_file_watching` reflects the client's dynamic-registration
    /// capability for `workspace/didChangeWatchedFiles`.
    pub fn with_workspace_roots(
        sender: Sender<Message>,
        roots: Vec<PathBuf>,
        supports_file_watching: bool,
    ) -> Self {
        let server = Self::build(sender, roots.clone(), supports_file_watching);
        if !roots.is_empty() {
            server.schedule_workspace_load(roots);
        }
        server
    }

    fn build(
        sender: Sender<Message>,
        workspace_roots: Vec<PathBuf>,
        supports_file_watching: bool,
    ) -> Self {
        let mut db = Database::default();
        let workspace = Workspace::new(&db, Vec::new(), Vec::new());
        let _ = &mut db;
        let worker_count = num_cpus::get().max(1);
        let (internal_sender, internal_receiver) = crossbeam_channel::unbounded();
        Self {
            db,
            vfs: Vfs::default(),
            workspace,
            workspace_config: None,
            workspace_roots,
            supports_file_watching,
            next_server_request_id: 1,
            pool: ThreadPool::with_name("quasar-lsp-worker".into(), worker_count),
            background: ThreadPool::with_name("quasar-lsp-bg".into(), 1),
            sender,
            internal_sender,
            internal_receiver,
        }
    }

    pub fn run(mut self, connection: &Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
        tracing::info!(workers = self.pool.max_count(), "quasar-lsp ready");

        // `lsp-server`'s `initialize()` consumes the `Initialized` notification
        // before `run()` is reached, so register watchers here at startup
        // rather than on receiving it.
        self.register_file_watchers();

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

    fn dispatch<R>(&self, req: Request, handler: fn(&Snapshot, R::Params) -> R::Result)
    where
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
            DidChangeWatchedFiles::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<DidChangeWatchedFilesParams>(notif.params)
                {
                    self.on_watched_files_changed(params);
                }
            }
            _ => {}
        }
    }

    /// Registers client-side watchers for `Cargo.toml` / `Cargo.lock` (the
    /// dependency graph) and `*.rs` (closed member-source content) so the
    /// client notifies us via `workspace/didChangeWatchedFiles` when either
    /// changes. No-op when the client doesn't support dynamic registration or
    /// no workspace roots were provided.
    fn register_file_watchers(&mut self) {
        if !self.supports_file_watching || self.workspace_roots.is_empty() {
            return;
        }
        let watchers = vec![
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/Cargo.toml".to_string()),
                kind: None,
            },
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/Cargo.lock".to_string()),
                kind: None,
            },
            // Closed member sources are indexed from disk; watch `.rs` so edits
            // made outside the editor (or in unopened files) refresh the index.
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/*.rs".to_string()),
                kind: None,
            },
        ];
        let registration = Registration {
            id: "quasar-cargo-watcher".to_string(),
            method: DidChangeWatchedFiles::METHOD.to_string(),
            register_options: Some(
                serde_json::to_value(DidChangeWatchedFilesRegistrationOptions { watchers })
                    .expect("registration options serialise"),
            ),
        };
        let params = RegistrationParams {
            registrations: vec![registration],
        };
        let id = self.next_server_request_id;
        self.next_server_request_id += 1;
        let req = Request {
            id: RequestId::from(id),
            method: RegisterCapability::METHOD.to_string(),
            params: serde_json::to_value(params).expect("registration params serialise"),
        };
        let _ = self.sender.send(Message::Request(req));
        tracing::info!("registered Cargo.toml / Cargo.lock / *.rs watchers");
    }

    fn on_watched_files_changed(&mut self, params: DidChangeWatchedFilesParams) {
        if self.workspace_roots.is_empty() {
            return;
        }
        // A manifest change can reshape the dependency graph, so it triggers a
        // full reload (which re-scans and re-registers member sources). A bare
        // `.rs` change only needs that one closed file's content refreshed.
        let mut cargo_changed = false;
        let mut sources_refreshed = false;
        for change in &params.changes {
            let Some(path) = crate::paths::uri_to_path(&change.uri) else {
                continue;
            };
            if is_cargo_manifest(&path) {
                cargo_changed = true;
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources_refreshed |= self.refresh_watched_rs(change, &path);
            }
        }

        if cargo_changed {
            tracing::info!("watched Cargo files changed; reloading workspace");
            self.schedule_workspace_load(self.workspace_roots.clone());
        } else if sources_refreshed {
            self.update_workspace();
            self.schedule_all_open_diagnostics();
        }
    }

    /// Refreshes (or drops) a single closed member source after a disk change.
    /// Returns whether the workspace index needs recomputing. Files outside
    /// the Quasar crate roots and files currently open in the editor (the
    /// overlay is authoritative) are ignored.
    fn refresh_watched_rs(&mut self, change: &FileEvent, path: &Path) -> bool {
        let covered = self
            .workspace_config
            .as_ref()
            .is_some_and(|cfg| cfg.covers(path));
        if !covered || self.vfs.is_open(&change.uri) {
            return false;
        }
        if change.typ == FileChangeType::DELETED {
            self.vfs.remove_workspace_member(&change.uri);
            return true;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let text: Arc<str> = Arc::from(text.as_str());
                self.vfs
                    .register_workspace_member(&mut self.db, change.uri.clone(), text);
                true
            }
            Err(_) => false,
        }
    }

    fn handle_internal(&mut self, evt: InternalEvent) {
        match evt {
            InternalEvent::WorkspaceLoaded(Some(config)) => {
                tracing::info!(
                    crates = config.quasar_crate_roots.len(),
                    account_types = config.known_account_types.len(),
                    indexed_files = config.indexed_source_files.len(),
                    "workspace loaded"
                );
                self.apply_loaded_config(config);
                self.send_progress_end(WORKSPACE_LOAD_TOKEN, "loaded");
            }
            InternalEvent::WorkspaceLoaded(None) => {
                tracing::warn!("workspace load failed for all folders; serving all open files");
                self.workspace_config = None;
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
        // The file may already be interned as a closed disk-backed member; the
        // editor's buffer is authoritative on open, so overwrite its content.
        if self
            .vfs
            .set_text(&mut self.db, &uri, text.clone())
            .is_none()
        {
            self.vfs.intern(&mut self.db, uri.clone(), text);
        }
        self.vfs.mark_open(uri);
        self.update_workspace();
        // Opening a file changes workspace membership, so every open file's
        // cross-file diagnostics may shift. Schedule them all against the
        // post-write snapshot; any worker cancelled by a later mutation is
        // superseded by that mutation's own all-files pass.
        self.schedule_all_open_diagnostics();
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
        if self
            .vfs
            .set_text(&mut self.db, &uri, text.clone())
            .is_none()
        {
            self.vfs.intern(&mut self.db, uri.clone(), text);
        }
        // Editing a definition shifts cross-file resolution for every other
        // open file (their refs read the workspace-wide index), so recompute
        // all open files — not just this one. Salsa early-cutoff keeps this
        // cheap when the edit didn't change cross-file results.
        self.schedule_all_open_diagnostics();
    }

    fn on_did_close(&mut self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.vfs.mark_closed(&uri);
        self.update_workspace();
        publish(&self.sender, uri, vec![]);
        // Remaining files' cross-file diagnostics may shift when a file leaves
        // the workspace.
        self.schedule_all_open_diagnostics();
    }

    fn schedule_all_open_diagnostics(&self) {
        let map = self.vfs.uri_to_file();
        for uri in self.vfs.open_uris() {
            if let Some(file) = map.get(&uri).copied() {
                self.schedule_diagnostics(uri, file);
            }
        }
    }

    fn update_workspace(&mut self) {
        let files = self.vfs.active_files();
        self.workspace.set_files(&mut self.db).to(files);
    }

    /// Installs a freshly-loaded [`WorkspaceConfig`]: publishes the known
    /// account-type names into the Salsa workspace, registers every indexed
    /// source file as a closed disk-backed file, drops any open files that fell
    /// outside the discovered crate roots, and recomputes diagnostics for what
    /// remains open. Shared by the background-load path and test setup.
    fn apply_loaded_config(&mut self, config: WorkspaceConfig) {
        self.workspace
            .set_known_account_types(&mut self.db)
            .to(config.known_account_types.clone());
        self.workspace_config = Some(config);
        self.register_indexed_sources();
        // Closes open files outside the crate roots and refreshes
        // `Workspace.files` to include the newly registered members.
        self.refresh_workspace_membership();
        // The first didOpen diagnostics ran before this scan finished, so
        // already-open files may carry stale results — types flagged as unknown,
        // or cross-file refs that now resolve into a freshly indexed file.
        // Recompute them now.
        self.schedule_all_open_diagnostics();
    }

    /// Reads every indexed Quasar source file (member + dependency) from disk
    /// and registers it as a closed, `File`-backed workspace member, so
    /// cross-file analysis covers definitions that aren't open in the editor —
    /// including account types declared in dependency crates. Open overlays
    /// keep precedence on content.
    fn register_indexed_sources(&mut self) {
        let Some(config) = self.workspace_config.clone() else {
            return;
        };
        for path in &config.indexed_source_files {
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            let Some(uri) = crate::paths::path_to_uri(path) else {
                continue;
            };
            let text: Arc<str> = Arc::from(text.as_str());
            self.vfs.register_workspace_member(&mut self.db, uri, text);
        }
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
                // Salsa cancelled this worker because the revision moved. The
                // mutation that caused the cancellation runs its own
                // all-open-files pass afterwards, so we simply drop here.
                tracing::debug!("diagnostic worker cancelled for {}", uri.as_str());
            }
        });
    }

    fn schedule_workspace_load(&self, roots: Vec<PathBuf>) {
        self.send_progress_begin(WORKSPACE_LOAD_TOKEN, "Loading Quasar workspace");
        let internal = self.internal_sender.clone();
        self.background.execute(move || {
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                cargo_workspace::load_workspaces(&roots)
            }));
            let config = outcome.unwrap_or(None);
            let _ = internal.send(InternalEvent::WorkspaceLoaded(config));
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
    let Some(path) = crate::paths::uri_to_path(uri) else {
        return false;
    };
    cfg.covers(&path)
}

/// True for `Cargo.toml` / `Cargo.lock`, whose changes can reshape the
/// dependency graph and warrant a full workspace reload.
fn is_cargo_manifest(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("Cargo.toml") | Some("Cargo.lock")
    )
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
    let has_one = resolve_has_one(db, workspace, file);

    let line_index = line_index_for(db, file);
    let text = file.text(db).clone();

    let mut diagnostics: Vec<lsp_types::Diagnostic> = Vec::new();
    for d in parsed.diagnostics(db).iter() {
        diagnostics.push(convert(d, &text, &line_index, &uri));
    }
    for d in resolved.diagnostics(db).iter() {
        diagnostics.push(convert(d, &text, &line_index, &uri));
    }
    for d in has_one.diagnostics(db).iter() {
        diagnostics.push(convert(d, &text, &line_index, &uri));
    }
    for d in validate_accounts(db, file).diagnostics(db).iter() {
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
