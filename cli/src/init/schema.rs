use {crate::config::CommandSpec, serde::Serialize};

#[derive(Serialize)]
pub(super) struct QuasarToml {
    pub(super) project: QuasarProject,
    pub(super) toolchain: QuasarToolchain,
    pub(super) testing: QuasarTesting,
    pub(super) clients: QuasarClients,
}

#[derive(Serialize)]
pub(super) struct QuasarProject {
    pub(super) name: String,
}

#[derive(Serialize)]
pub(super) struct QuasarToolchain {
    #[serde(rename = "type")]
    pub(super) toolchain_type: String,
}

#[derive(Serialize)]
pub(super) struct QuasarTesting {
    pub(super) language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) rust: Option<QuasarRustTesting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) typescript: Option<QuasarTypeScriptTesting>,
}

#[derive(Serialize)]
pub(super) struct QuasarRustTesting {
    pub(super) framework: String,
    pub(super) test: CommandSpec,
}

#[derive(Serialize)]
pub(super) struct QuasarTypeScriptTesting {
    pub(super) framework: String,
    pub(super) sdk: String,
    pub(super) install: CommandSpec,
    pub(super) test: CommandSpec,
}

#[derive(Serialize)]
pub(super) struct QuasarClients {
    pub(super) path: String,
    pub(super) languages: Vec<String>,
}
