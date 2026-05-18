//! Off-chain coverage for [`quasar_lang::cpi::CpiDynamic`] instruction data paths.
//!
//! The safe serialization entrypoint is [`CpiDynamic::set_data`]. Zero-copy writes use
//! [`CpiDynamic::data_mut`] and then [`CpiDynamic::set_data_len`], which is `unsafe`
//! because the caller must initialize every byte in the active range.

use quasar_lang::cpi::CpiDynamic;
use solana_address::Address;

#[test]
fn cpi_dynamic_set_data_round_trip() {
    let program_id = Address::new_from_array([0u8; 32]);
    let mut cpi = CpiDynamic::<0, 8>::new(&program_id);
    cpi.set_data(&[1, 2, 3, 4]).unwrap();
    assert_eq!(cpi.instruction_data(), &[1, 2, 3, 4]);
}
