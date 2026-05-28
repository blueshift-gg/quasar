//! Quasar language server.

pub mod capabilities;
pub mod cargo_workspace;
pub mod diagnostics;
pub mod handlers;
pub mod server;
pub mod snapshot;
pub mod vfs;

pub use cargo_workspace::{identify_quasar_crates, load_workspace, WorkspaceConfig};
pub use server::Server;

use tracing_subscriber::EnvFilter;

pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("quasar=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
}
