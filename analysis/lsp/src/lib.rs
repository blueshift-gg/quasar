//! Quasar language server.

// The VFS keys files by `lsp_types::Uri`. fluent-uri's `Uri` holds an internal
// `Cell` parse-cache, but that cache doesn't participate in `Hash`/`Eq` (the
// URI string does), so it's safe to use as a map key.
#![allow(clippy::mutable_key_type)]

pub mod capabilities;
pub mod cargo_workspace;
pub mod diagnostics;
pub mod handlers;
pub mod paths;
pub mod server;
pub mod snapshot;
pub mod vfs;

use tracing_subscriber::EnvFilter;
pub use {
    cargo_workspace::{identify_quasar_crates, load_workspace, load_workspaces, WorkspaceConfig},
    server::Server,
};

pub fn init_logging() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("quasar=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
}
