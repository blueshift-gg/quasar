// Quasar Test Suite
//
// Integration tests for the Quasar Solana framework.
// Each module tests a specific test program via Mollusk SVM.

#[cfg(test)]
mod errors;
#[cfg(test)]
mod events;
#[cfg(test)]
mod header_tests;
#[cfg(test)]
mod accounts;
#[cfg(test)]
mod constraints;
#[cfg(test)]
mod cpi_system;
#[cfg(test)]
mod dynamic;
#[cfg(test)]
mod remaining;
#[cfg(test)]
mod pda;
#[cfg(test)]
mod sysvar;
#[cfg(test)]
mod token_cpi;
#[cfg(test)]
mod account_validation;
#[cfg(test)]
mod token_state;
