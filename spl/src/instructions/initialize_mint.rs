use quasar_core::{
    cpi::{CpiCall, InstructionAccount},
    prelude::*,
};

/// Initialize a mint (InitializeMint2 — opcode 20).
///
/// Free function variant for generated code that works with raw `AccountView`
/// references during parse-time init. Equivalent to
/// [`super::TokenCpi::initialize_mint2`].
///
/// Unlike InitializeMint, this variant does not require the Rent sysvar
/// account, saving one account in the CPI. The account must already be
/// allocated with the correct size (82 bytes).
///
/// ### Accounts:
///   0. `[WRITE]` Mint account to initialize
///
/// ### Instruction data (67 bytes):
/// ```text
/// [0    ] discriminator    (20)
/// [1    ] decimals         (u8)
/// [2..34 ] mint_authority   (32-byte address)
/// [34   ] has_freeze_auth  (u8, 0 or 1)
/// [35..67] freeze_authority (32-byte address, zeroed if absent)
/// ```
#[inline(always)]
#[allow(dead_code)]
pub fn initialize_mint2<'a>(
    token_program: &'a AccountView,
    mint: &'a AccountView,
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
) -> CpiCall<'a, 1, 67> {
    // SAFETY: All 67 bytes written before `assume_init`.
    let data = unsafe {
        let mut buf = core::mem::MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 20);
        core::ptr::write(ptr.add(1), decimals);
        core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
        if let Some(fa) = freeze_authority {
            core::ptr::write(ptr.add(34), 1u8);
            core::ptr::copy_nonoverlapping(fa.as_ref().as_ptr(), ptr.add(35), 32);
        } else {
            core::ptr::write_bytes(ptr.add(34), 0, 33);
        }
        buf.assume_init()
    };

    CpiCall::new(
        token_program.address(),
        [InstructionAccount::writable(mint.address())],
        [mint],
        data,
    )
}
