//! Database trait and concrete in-memory implementation.

#[salsa::db]
pub trait Db: salsa::Database {}

/// In-memory Salsa database used by tests and (currently) by `quasar-lsp`.
#[salsa::db]
#[derive(Clone, Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for Database {}

#[salsa::db]
impl Db for Database {}
