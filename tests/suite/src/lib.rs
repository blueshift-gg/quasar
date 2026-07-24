//! SVM integration suite: every module asserts the behavior of one framework
//! feature area against the fixture programs in `tests/programs/*`, executed
//! as real SBF binaries under Mollusk. Each test owns an exact state or error
//! oracle for the behavior named by its module.

#[cfg(test)]
mod dynamic;
#[cfg(test)]
mod events;
#[cfg(test)]
mod header_tests;
#[cfg(test)]
mod pda;
#[cfg(test)]
mod remaining;
#[cfg(test)]
mod sysvar;
#[cfg(test)]
mod token_state;
#[cfg(test)]
mod two_dyn;

// Core account lifecycle
#[cfg(test)]
mod close;
#[cfg(test)]
mod discriminator;
#[cfg(test)]
mod init;
#[cfg(test)]
mod init_if_needed;
#[cfg(test)]
mod optional_accounts;
#[cfg(test)]
mod realloc;

// Validation & constraints
#[cfg(test)]
mod account_flags;
#[cfg(test)]
mod account_validation;
#[cfg(test)]
mod constraints;

// CPI & errors
#[cfg(test)]
mod cpi_pointer_safety;
#[cfg(test)]
mod cpi_return;
#[cfg(test)]
mod cpi_system;
#[cfg(test)]
mod errors;

// Suite-local SVM compat layer (mollusk-backed) + shared helpers
#[cfg(test)]
mod compat;
#[cfg(test)]
mod helpers;
#[cfg(test)]
mod test_ata_derivation;
#[cfg(test)]
mod test_close_attr;
#[cfg(test)]
mod test_cpi_approve_revoke;
#[cfg(test)]
mod test_cpi_close;
#[cfg(test)]
mod test_cpi_mint_burn;
#[cfg(test)]
mod test_cpi_transfer;
#[cfg(test)]
mod test_init_ata;
#[cfg(test)]
mod test_init_interface;
#[cfg(test)]
mod test_init_mint;
#[cfg(test)]
mod test_init_mint_pda;
#[cfg(test)]
mod test_init_token;
#[cfg(test)]
mod test_init_token_pda;
#[cfg(test)]
mod test_sweep;
#[cfg(test)]
mod test_validate_ata;
#[cfg(test)]
mod test_validate_mint;
#[cfg(test)]
mod test_validate_token;

// Option<T> instruction args
#[cfg(test)]
mod optional_args;

// InterfaceAccount custom Owners
#[cfg(test)]
mod test_interface_migration;

// Heap opt-in
#[cfg(test)]
mod test_heap;

// Polymorphic accounts (one_of)
#[cfg(test)]
mod test_one_of;

// Account migration (migrate)
#[cfg(test)]
mod test_migrate;

// Raw instruction escape hatch
#[cfg(test)]
mod test_raw;
