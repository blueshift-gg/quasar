use quasar_lang::{Address, ProgramError};
use quasar_metadata::{METADATA_PROGRAM_ID, pda};
use quasar_test::Test;

#[test]
fn metaplex_constraints_run_on_sbf() {
    for debug in [false, true] {
        let mut t = if debug {
            Test::debug(metaplex::ID, "metaplex")
        } else {
            Test::new(metaplex::ID, "metaplex")
        };
        let mint = t.wallet(1_000_000);
        let authority = t.wallet(1_000_000);
        let metadata = pda::metadata_address(&mint).unwrap().0;
        let edition = pda::master_edition_address(&mint).unwrap().0;
        let mut metadata_data = vec![0; 679];
        metadata_data[0] = 4;
        metadata_data[1..33].copy_from_slice(authority.as_ref());
        metadata_data[33..65].copy_from_slice(mint.as_ref());
        let mut edition_data = vec![0; 282];
        edition_data[0] = 6;
        edition_data[1..9].copy_from_slice(&42u64.to_le_bytes());
        t.set(metadata, METADATA_PROGRAM_ID, t.rent(679), metadata_data.clone());
        t.set(edition, METADATA_PROGRAM_ID, t.rent(282), edition_data.clone());
        let ix = || metaplex::read::instruction((mint, authority, metadata, edition), ());
        assert_eq!(t.run(ix()).succeeds().return_data(), 42u64.to_le_bytes());

        t.set(metadata, Address::default(), t.rent(679), metadata_data.clone());
        t.run(ix()).fails(ProgramError::IllegalOwner);
        metadata_data[33..65].fill(0);
        t.set(metadata, METADATA_PROGRAM_ID, t.rent(679), metadata_data.clone());
        t.run(ix()).fails(ProgramError::InvalidAccountData);
        metadata_data[33..65].copy_from_slice(mint.as_ref());
        t.set(metadata, METADATA_PROGRAM_ID, t.rent(679), metadata_data);
        edition_data[9] = 2;
        t.set(edition, METADATA_PROGRAM_ID, t.rent(282), edition_data);
        t.run(ix()).fails(ProgramError::InvalidAccountData);
    }
}
