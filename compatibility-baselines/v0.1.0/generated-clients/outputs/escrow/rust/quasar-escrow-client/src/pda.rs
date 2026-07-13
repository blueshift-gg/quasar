use solana_address::Address;

/// Seeds: [b"escrow", maker]
pub fn find_escrow_address(maker: &Address, program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"escrow", maker.as_ref()], program_id)
}
