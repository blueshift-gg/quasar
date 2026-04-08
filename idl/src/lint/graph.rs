//! Account relationship graph.
use super::types::TypeRegistry;
use crate::parser::accounts::RawAccountsStruct;

pub struct AccountGraph {
    pub struct_name: String,
}

impl AccountGraph {
    pub fn build(accounts: &RawAccountsStruct, _registry: &TypeRegistry) -> Self {
        Self {
            struct_name: accounts.name.clone(),
        }
    }
    pub fn expected_edge_count(&self) -> usize {
        0
    }
    pub fn constrained_edge_count(&self) -> usize {
        0
    }
}
