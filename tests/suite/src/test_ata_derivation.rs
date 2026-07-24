use {
    crate::compat::Pubkey,
    crate::helpers::*,
    quasar_spl::{
        get_associated_token_address_const, get_associated_token_address_with_program_const,
    },
};

#[test]
fn different_wallets() {
    let wallet1 = Pubkey::new_unique();
    let wallet2 = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let (addr1, _) = get_associated_token_address_const(&wallet1, &mint);
    let (addr2, _) = get_associated_token_address_const(&wallet2, &mint);
    assert_ne!(addr1, addr2);
}

#[test]
fn different_mints() {
    let wallet = Pubkey::new_unique();
    let mint1 = Pubkey::new_unique();
    let mint2 = Pubkey::new_unique();
    let (addr1, _) = get_associated_token_address_const(&wallet, &mint1);
    let (addr2, _) = get_associated_token_address_const(&wallet, &mint2);
    assert_ne!(addr1, addr2);
}

#[test]
fn different_programs() {
    let wallet = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let (addr1, _) =
        get_associated_token_address_with_program_const(&wallet, &mint, &spl_token_program_id());
    let (addr2, _) =
        get_associated_token_address_with_program_const(&wallet, &mint, &token_2022_program_id());
    assert_ne!(addr1, addr2);
}

#[test]
fn const_matches_with_program_spl() {
    let wallet = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let (addr1, bump1) = get_associated_token_address_const(&wallet, &mint);
    let (addr2, bump2) =
        get_associated_token_address_with_program_const(&wallet, &mint, &spl_token_program_id());
    assert_eq!(addr1, addr2);
    assert_eq!(bump1, bump2);
}
