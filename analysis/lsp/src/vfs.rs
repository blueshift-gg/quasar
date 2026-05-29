//! Virtual file system to translate communication between document Uri and Salsa. 
//! Open buffers (overlays) take precedence over
//! disk for any path they cover until `didClose`.

use lsp_types::Uri;
use quasar_hir::{Database, File};
use salsa::Setter;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Default)]
pub struct Vfs {
    by_url: HashMap<Uri, File>,
    open: HashMap<Uri, ()>,
    /// Closed, disk-backed files that belong to a workspace-member Quasar
    /// crate. They participate in cross-file analysis (the symbol index,
    /// goto, has_one checks) without being open in the editor. An open
    /// overlay for the same URI always wins on content.
    workspace_members: HashSet<Uri>,
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

    /// Registers a closed, disk-backed workspace-member file. Interns it if
    /// new; otherwise refreshes its content from disk unless an editor overlay
    /// is currently open for the URI (the overlay is authoritative). Returns
    /// the file handle.
    pub fn register_workspace_member(
        &mut self,
        db: &mut Database,
        url: Uri,
        disk_text: Arc<str>,
    ) -> File {
        let file = match self.by_url.get(&url) {
            Some(&file) => {
                if !self.open.contains_key(&url) {
                    file.set_text(db).to(disk_text);
                }
                file
            }
            None => {
                let path = url.as_str().to_string();
                let file = File::new(db, disk_text, path);
                self.by_url.insert(url.clone(), file);
                file
            }
        };
        self.workspace_members.insert(url);
        file
    }

    /// Drops a workspace-member file (e.g. it was deleted on disk). No-op for
    /// files that aren't registered members. Open overlays are left intact.
    pub fn remove_workspace_member(&mut self, url: &Uri) {
        if self.workspace_members.remove(url) && !self.open.contains_key(url) {
            self.by_url.remove(url);
        }
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

    /// Every file visible to cross-file analysis: open editor buffers plus
    /// closed disk-backed workspace members, deduplicated by URI. Open buffers
    /// and members share a single [`File`] per URI, so the open overlay's
    /// content is automatically reflected.
    pub fn active_files(&self) -> Vec<File> {
        let mut uris: Vec<&Uri> = self.open.keys().collect();
        uris.extend(self.workspace_members.iter());
        // Sort + dedup for a deterministic order: this Vec is a Salsa input
        // (`Workspace::set_files`), and the indexes built from it resolve
        // duplicate names first-declaration-wins. A nondeterministic order
        // would make goto/hover/has_one flaky and defeat Salsa's early cutoff.
        uris.sort_unstable_by_key(|u| u.as_str());
        uris.dedup();
        uris.into_iter()
            .filter_map(|u| self.by_url.get(u).copied())
            .collect()
    }

    /// Snapshot copy of the Uri → File map for use on worker threads.
    pub fn uri_to_file(&self) -> HashMap<Uri, File> {
        self.by_url.clone()
    }

    /// URIs of all currently open files.
    pub fn open_uris(&self) -> Vec<Uri> {
        self.open.keys().cloned().collect()
    }
}
