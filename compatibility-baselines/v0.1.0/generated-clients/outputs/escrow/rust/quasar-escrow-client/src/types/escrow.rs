use wincode::{SchemaWrite, SchemaRead};
use solana_address::Address;

#[derive(SchemaWrite, SchemaRead)]
pub struct Escrow {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
    pub bump: u8,
}
