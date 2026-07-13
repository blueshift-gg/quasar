use wincode::{SchemaWrite, SchemaRead};
use solana_address::Address;

#[derive(SchemaWrite, SchemaRead)]
pub struct RefundEvent {
    pub escrow: Address,
}
