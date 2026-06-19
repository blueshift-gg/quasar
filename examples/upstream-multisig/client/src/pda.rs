use solana_address::Address;

/// Seeds: [b"multisig", creator]
pub fn find_config_address(creator: &Address, program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"multisig", creator.as_ref()], program_id)
}

/// Seeds: [b"vault", config]
pub fn find_vault_address(config: &Address, program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"vault", config.as_ref()], program_id)
}
