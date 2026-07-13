use wincode::{SchemaWrite, SchemaRead};
use solana_address::Address;

#[derive(SchemaWrite, SchemaRead)]
pub struct MakeEvent {
    pub escrow: Address,
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub deposit: u64,
    pub receive: u64,
}
