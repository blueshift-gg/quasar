use wincode::{SchemaWrite, SchemaRead};
use solana_address::Address;
use quasar_lang::client::{DynString, DynVec};

#[derive(SchemaWrite, SchemaRead)]
pub struct MultisigConfig {
    pub creator: Address,
    pub threshold: u8,
    pub bump: u8,
    pub label: DynString<u8>,
    pub signers: DynVec<Address, u16>,
}
