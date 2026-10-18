//! What a handler receives, and the entrypoint that builds it.

use zeropod::ZeroPod;

use crate::{
    Address, ProgramError, Result,
    cpi::Vouches,
    data::{InstructionData, set_return},
    field::FieldInfo,
    input::{Cursor, instruction_data},
};

/// An instruction's accounts, read from the cursor in declaration order.
/// Implemented by `#[derive(Accounts)]`: for any data `D`, or for the one
/// its constraints read, named by `#[data(..)]`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not take `{D}` as its instruction data",
    label = "its constraints read the data named by its `#[data(..)]`"
)]
pub trait Accounts<D = ()>: Sized {
    /// What runs once the handler has succeeded: a `Close` or `Settle` per
    /// `close`, `realloc` and `init` field, in declaration order.
    type Deferred;

    /// Each field, in declaration order.
    const FIELDS: &'static [FieldInfo];

    /// `V` is what the handler's calls vouch for: those checks are left out.
    fn from_cursor<V: Vouches>(cursor: &mut Cursor, data: &D) -> Result<Self>;

    /// Each account with its own wrapper's checks, but none of the relations
    /// between them: a property's snapshots before and after the instruction,
    /// which only read the accounts.
    #[cfg(kani)]
    fn load_accounts(cursor: &mut Cursor) -> Result<Self>;

    /// Taken before the handler runs, and run by `run_deferred` after it
    /// succeeds.
    fn deferred(&self) -> Self::Deferred;

    fn run_deferred(deferred: Self::Deferred) -> Result;
}

/// What a handler receives: its accounts, checked, and its instruction data.
pub struct Ctx<A, D = ()> {
    pub accounts: A,
    pub data: D,
}

/// Decodes `D`, loads `A`, runs `handler`, runs what `A` defers, and sets
/// the handler's result as return data.
#[inline(always)]
pub fn run<A: Accounts<D>, D: InstructionData, T: ZeroPod, V: Vouches>(
    mut cursor: Cursor,
    bytes: &[u8],
    handler: impl FnOnce(Ctx<A, D>) -> Result<T>,
) -> Result {
    let data = D::decode(bytes)?;
    let accounts = A::from_cursor::<V>(&mut cursor, &data)?;
    let deferred = accounts.deferred();
    let value = handler(Ctx { accounts, data })?;
    A::run_deferred(deferred)?;
    set_return(&value);
    Ok(())
}

/// Splits off the discriminator and calls `dispatch`. Returns the value the
/// entrypoint must return to the runtime. The program id the runtime passes
/// is not compared with `id`: a copy deployed at another address can neither
/// write nor sign for what `id` owns.
///
/// # Safety
/// `input` and `data` are the pointers the runtime passed to the entrypoint.
pub unsafe fn entrypoint<const D: usize>(
    input: *mut u8,
    data: *const u8,
    id: &'static Address,
    dispatch: impl FnOnce([u8; D], &[u8], Cursor) -> Result,
) -> u64 {
    // SAFETY: forwarded from this function's contract.
    let data = unsafe { instruction_data(data) };
    // SAFETY: as above; this is the only cursor over `input`.
    let cursor = unsafe { Cursor::new(input, id) };
    let result = if let Some((discriminator, args)) = data.split_first_chunk::<D>() {
        dispatch(*discriminator, args, cursor)
    } else {
        Err(ProgramError::InvalidInstructionData)
    };
    match result {
        Ok(()) => 0,
        Err(error) => code(error),
    }
}

/// Kept out of line: inlined, the conversion's jump table is copied into
/// every path that can fail.
#[cold]
#[inline(never)]
fn code(error: ProgramError) -> u64 {
    error.into()
}
