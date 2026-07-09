use {
    crate::state::{TwoDynArgsAccount, TwoDynArgsAccountInner},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

/// Two dynamic instruction args (`a`, `b`) declared at the struct level and
/// referenced in a constraint. Exercises A1: the `#[derive(Accounts)]`
/// extraction must decode the same compact wire layout the handler and the
/// generated client use. With two dynamic args the old interleaved walker read
/// `b`'s length prefix from `a`'s payload, so the constraint saw garbage (or
/// the instruction was rejected outright) while the handler decoded correctly.
#[derive(Accounts)]
#[instruction(tag: u64, a: String<8>, b: String<8>)]
pub struct TwoDyn {
    #[account(mut, constraints(tag != 0 && a.len() == b.len()))]
    pub account: Account<TwoDynArgsAccount>,
}

impl TwoDyn {
    #[inline(always)]
    pub fn handler(&mut self, tag: u64, a: &str, b: &str) -> Result<(), ProgramError> {
        let mut a_buf = [0u8; 8];
        a_buf[..a.len()].copy_from_slice(a.as_bytes());
        let mut b_buf = [0u8; 8];
        b_buf[..b.len()].copy_from_slice(b.as_bytes());
        self.account.set_inner(TwoDynArgsAccountInner {
            tag,
            a_len: a.len() as u8,
            a: u64::from_le_bytes(a_buf),
            b_len: b.len() as u8,
            b: u64::from_le_bytes(b_buf),
        });
        Ok(())
    }
}
