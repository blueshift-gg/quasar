//! Uri ↔ Salsa [`File`] mapping. Open buffers (overlays) take precedence over
//! disk for any path they cover until `didClose`.

use lsp_types::Uri;
use quasar_hir::{Database, File};
use salsa::Setter;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct Vfs {
    by_url: HashMap<Uri, File>,
    open: HashMap<Uri, ()>,
}

impl Vfs {
    /// Returns the existing [`File`] for `url`, creating it if absent.
    pub fn intern(&mut self, db: &mut Database, url: Uri, initial_text: Arc<str>) -> File {
        if let Some(file) = self.by_url.get(&url) {
            return *file;
        }
        let path = url.as_str().to_string();
        let file = File::new(db, initial_text, path);
        self.by_url.insert(url, file);
        file
    }

    pub fn get(&self, url: &Uri) -> Option<File> {
        self.by_url.get(url).copied()
    }

    pub fn set_text(&self, db: &mut Database, url: &Uri, text: Arc<str>) -> Option<File> {
        let file = *self.by_url.get(url)?;
        file.set_text(db).to(text);
        Some(file)
    }

    pub fn mark_open(&mut self, url: Uri) {
        self.open.insert(url, ());
    }

    pub fn mark_closed(&mut self, url: &Uri) {
        self.open.remove(url);
    }

    pub fn is_open(&self, url: &Uri) -> bool {
        self.open.contains_key(url)
    }

    /// All currently open files. Used to construct or update the
    /// [`Workspace`](quasar_hir::Workspace) on each VFS change.
    pub fn open_files(&self) -> Vec<File> {
        self.open
            .keys()
            .filter_map(|u| self.by_url.get(u).copied())
            .collect()
    }

    /// Snapshot copy of the Uri → File map for use on worker threads.
    pub fn uri_to_file(&self) -> HashMap<Uri, File> {
        self.by_url.clone()
    }
}
