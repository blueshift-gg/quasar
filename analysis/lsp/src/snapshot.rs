//! Worker-thread-friendly snapshot of the server's state.
//!
//! Workers don't touch the live [`Vfs`](crate::vfs::Vfs); they receive a
//! [`Snapshot`] that captures everything needed to compute one read at a
//! consistent point in time. The Salsa database itself is internally Arc'd,
//! so cloning is cheap.

use {
    lsp_types::Uri,
    quasar_hir::{Database, File, Workspace},
    std::collections::HashMap,
};

pub struct Snapshot {
    pub db: Database,
    pub workspace: Workspace,
    pub uri_to_file: HashMap<Uri, File>,
}

impl Snapshot {
    pub fn file_for(&self, uri: &Uri) -> Option<File> {
        self.uri_to_file.get(uri).copied()
    }

    pub fn uri_for(&self, file: File) -> Option<&Uri> {
        self.uri_to_file
            .iter()
            .find_map(|(uri, f)| if *f == file { Some(uri) } else { None })
    }
}
