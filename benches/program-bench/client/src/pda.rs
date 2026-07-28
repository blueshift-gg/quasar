use solana_address::Address;

/// Seeds: [b"pda"]
pub fn find_account_address(program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"pda"], program_id)
}

/// Seeds: [b"clock"]
pub fn find_snapshot_address(program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"clock"], program_id)
}
