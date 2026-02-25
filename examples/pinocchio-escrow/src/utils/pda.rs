use {
    crate::state::EscrowAccount,
    pinocchio::{cpi::Seed, Address},
};

#[inline(always)]
pub fn escrow_pda(maker: &[u8]) -> (Address, u8) {
    Address::find_program_address(&[EscrowAccount::SEEDS_PREFIX, maker], &crate::ID)
}

#[inline(always)]
pub fn escrow_seeds<'a>(maker: &'a [u8], bump: &'a [u8]) -> [Seed<'a>; 3] {
    [
        Seed::from(EscrowAccount::SEEDS_PREFIX),
        Seed::from(maker),
        Seed::from(bump),
    ]
}
