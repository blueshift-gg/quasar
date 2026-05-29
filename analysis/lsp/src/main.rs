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
    let roots = extract_workspace_roots(&params);
    let supports_watching = supports_file_watching(&params);

    let server =
        Server::with_workspace_roots(connection.sender.clone(), roots, supports_watching);
    server.run(&connection)?;

    io_threads.join()?;
    tracing::info!("quasar-lsp shutting down");
    Ok(())
}

fn extract_workspace_roots(params: &InitializeParams) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(folders) = &params.workspace_folders {
        for folder in folders {
            if let Some(p) = quasar_lsp::paths::file_str_to_path(folder.uri.as_str()) {
                roots.push(p);
            }
        }
    }
    if roots.is_empty() {
        #[allow(deprecated)]
        if let Some(root_uri) = &params.root_uri {
            if let Some(p) = quasar_lsp::paths::file_str_to_path(root_uri.as_str()) {
                roots.push(p);
            }
        }
    }
    roots
}

fn supports_file_watching(params: &InitializeParams) -> bool {
    params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|w| w.did_change_watched_files.as_ref())
        .and_then(|d| d.dynamic_registration)
        .unwrap_or(false)
}
