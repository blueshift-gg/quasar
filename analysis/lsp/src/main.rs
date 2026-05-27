use lsp_server::{Connection, ErrorCode, Message, Response, ResponseError};
use lsp_types::{InitializeParams, ServerCapabilities};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    quasar_lsp::init_logging();
    tracing::info!("quasar-lsp starting (scaffold build, no capabilities registered)");

    let (connection, io_threads) = Connection::stdio();
    let server_capabilities = serde_json::to_value(ServerCapabilities::default())?;
    let init_params = connection.initialize(server_capabilities)?;
    let _params: InitializeParams = serde_json::from_value(init_params)?;

    main_loop(connection)?;
    io_threads.join()?;

    tracing::info!("quasar-lsp shutting down");
    Ok(())
}

fn main_loop(connection: Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                let resp = Response {
                    id: req.id,
                    result: None,
                    error: Some(ResponseError {
                        code: ErrorCode::MethodNotFound as i32,
                        message: format!("method not implemented: {}", req.method),
                        data: None,
                    }),
                };
                connection.sender.send(Message::Response(resp))?;
            }
            Message::Notification(_) | Message::Response(_) => {}
        }
    }
    Ok(())
}
