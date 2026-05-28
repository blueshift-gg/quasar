use lsp_server::Connection;
use lsp_types::InitializeParams;
use quasar_lsp::{capabilities::server_capabilities, init_logging, Server};
use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    init_logging();
    tracing::info!("quasar-lsp starting");

    let (connection, io_threads) = Connection::stdio();
    let caps =
        serde_json::to_value(server_capabilities()).expect("server capabilities serialise");
    let init_value = connection.initialize(caps)?;
    let params: InitializeParams = serde_json::from_value(init_value)?;
    let root = extract_workspace_root(&params);

    let server = Server::with_workspace_root(connection.sender.clone(), root);
    server.run(&connection)?;

    io_threads.join()?;
    tracing::info!("quasar-lsp shutting down");
    Ok(())
}

fn extract_workspace_root(params: &InitializeParams) -> Option<PathBuf> {
    if let Some(folders) = &params.workspace_folders {
        if let Some(folder) = folders.first() {
            let s = folder.uri.as_str();
            if let Some(rest) = s.strip_prefix("file://") {
                return Some(PathBuf::from(rest));
            }
        }
    }
    #[allow(deprecated)]
    if let Some(root_uri) = &params.root_uri {
        let s = root_uri.as_str();
        if let Some(rest) = s.strip_prefix("file://") {
            return Some(PathBuf::from(rest));
        }
    }
    None
}
