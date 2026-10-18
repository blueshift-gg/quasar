//! The System Program: transfers, account creation, and the `init` step of an
//! accounts struct.

use crate::{
    Address, ProgramError, Result,
    account::{Account, Kind, capacity, holds, len},
    cpi::{Cpi, CpiAccount},
    input,
    pda::{Seeds, SignerSeeds},
    sysvar::RentSource,
    wrapper::{Program, Signer, SystemProgram, UncheckedAccount, Wrapper},
};

/// `SystemInstruction` variants: a little-endian `u32` index, then the fields.
const CREATE_ACCOUNT: u32 = 0;
const ASSIGN: u32 = 1;
const TRANSFER: u32 = 2;
const ALLOCATE: u32 = 8;

impl Program<SystemProgram> {
    /// Moves lamports out of an account the System Program owns; `from` signs.
    pub fn transfer<'a, F: Wrapper, T: Wrapper>(
        &'a self,
        from: &'a mut F,
        to: &'a mut T,
        lamports: u64,
    ) -> Cpi<'a, SystemProgram, 2, 12> {
        // A transfer changes only lamports.
        let accounts = [CpiAccount::kept(from).signer(), CpiAccount::kept(to)];
        Cpi::new(self, accounts, transfer_data(lamports))
    }
}

#[inline(always)]
pub(crate) fn transfer_data(lamports: u64) -> [u8; 12] {
    let mut data = [0; 12];
    data[..4].copy_from_slice(&TRANSFER.to_le_bytes());
    data[4..].copy_from_slice(&lamports.to_le_bytes());
    data
}

fn create_account_data(lamports: u64, space: u64, owner: &Address) -> [u8; 52] {
    let mut data = [0; 52];
    data[..4].copy_from_slice(&CREATE_ACCOUNT.to_le_bytes());
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data[12..20].copy_from_slice(&space.to_le_bytes());
    data[20..].copy_from_slice(owner.as_ref());
    data
}

fn allocate_data(space: u64) -> [u8; 12] {
    let mut data = [0; 12];
    data[..4].copy_from_slice(&ALLOCATE.to_le_bytes());
    data[4..].copy_from_slice(&space.to_le_bytes());
    data
}

fn assign_data(owner: &Address) -> [u8; 36] {
    let mut data = [0; 36];
    data[..4].copy_from_slice(&ASSIGN.to_le_bytes());
    data[4..].copy_from_slice(owner.as_ref());
    data
}

/// Creates `account`, rent exempt for `space` bytes and owned by `owner`,
/// signing with `signer` if it is a PDA. `CreateAccount` fails on an account
/// that holds lamports, which anyone can send to any address, so such an
/// account is topped up, allocated and assigned instead, as Anchor does.
pub fn create<P: Wrapper, W: Wrapper, const S: usize>(
    system: &Program<SystemProgram>,
    payer: &P,
    account: &mut W,
    space: usize,
    owner: &Address,
    signer: Option<&SignerSeeds<'_, S>>,
    rent: &RentSource,
) -> Result {
    let rent = rent.get()?.minimum_balance(space);
    let held = account.lamports();
    if held == 0 {
        // SAFETY: creating an account changes only the payer's lamports.
        let payer = unsafe { CpiAccount::paying(payer) };
        let accounts = [payer, CpiAccount::writable(account).signer()];
        let data = create_account_data(rent, space as u64, owner);
        return invoke(Cpi::new(system, accounts, data), signer);
    }
    if let Some(top_up) = rent.checked_sub(held).filter(|&top_up| top_up > 0) {
        // SAFETY: a transfer changes only the payer's lamports.
        let payer = unsafe { CpiAccount::paying(payer) };
        let accounts = [payer, CpiAccount::writable(account)];
        invoke(Cpi::new(system, accounts, transfer_data(top_up)), signer)?;
    }
    let accounts = [CpiAccount::writable(account).signer()];
    invoke(Cpi::new(system, accounts, allocate_data(space as u64)), signer)?;
    let accounts = [CpiAccount::writable(account).signer()];
    invoke(Cpi::new(system, accounts, assign_data(owner)), signer)
}

fn invoke<const N: usize, const D: usize, const S: usize>(
    cpi: Cpi<'_, SystemProgram, N, D>,
    signer: Option<&SignerSeeds<'_, S>>,
) -> Result {
    match signer {
        Some(signer) => cpi.invoke_signed(signer),
        None => cpi.invoke(),
    }
}

/// `init` on a `Pda` field with `seeds(..)`: creates the account at
/// the canonical PDA of `seeds`, rent exempt at its layout's length, and returns it
/// with its bump. With `idempotent`, an account that already holds a `K` is
/// loaded instead.
pub fn init_pda<K: Kind, P: Wrapper, const S: usize>(
    system: &Program<SystemProgram>,
    payer: &P,
    mut account: UncheckedAccount,
    seeds: Seeds<'_, S>,
    idempotent: bool,
    rent: &RentSource,
) -> Result<(Account<K>, u8)> {
    const { assert!(K::PROGRAM_OWNED, "`init` creates accounts this program owns") };
    let (address, bump) = seeds.canonical(&K::OWNER)?;
    if *account.address() != address {
        return Err(ProgramError::InvalidSeeds);
    }
    if idempotent {
        match input::existing(account) {
            Ok(existing) => return Ok((existing, bump)),
            Err(new) => account = new,
        }
    }
    let signer = seeds.with_bump(bump);
    create(system, payer, &mut account, capacity::<K>(), &K::OWNER, Some(&signer), rent)?;
    write_discriminator::<K>(&mut account);
    Ok((Account::created(account.into_entry())?, bump))
}

/// `init` on a field without seeds: the new account signs for itself.
pub fn init<K: Kind, P: Wrapper>(
    system: &Program<SystemProgram>,
    payer: &P,
    mut account: Signer,
    idempotent: bool,
    rent: &RentSource,
) -> Result<Account<K>> {
    const { assert!(K::PROGRAM_OWNED, "`init` creates accounts this program owns") };
    if idempotent {
        match input::existing(account) {
            Ok(existing) => return Ok(existing),
            Err(new) => account = new,
        }
    }
    let signer = None::<&SignerSeeds<0>>;
    create(system, payer, &mut account, capacity::<K>(), &K::OWNER, signer, rent)?;
    write_discriminator::<K>(&mut account);
    Account::created(account.into_entry())
}

/// `migrate(from = O), payer = ..` on an `Account<N>` field: an account still
/// holding `O` becomes an `N`, converted by `N::from`, rewritten in place and
/// kept rent exempt with the payer's lamports. An account already holding
/// `N` loads as it is.
pub fn migrate<O: Kind, N: Kind + From<O>, P: Wrapper>(
    system: &Program<SystemProgram>,
    payer: &P,
    mut account: UncheckedAccount,
    rent: &RentSource,
) -> Result<Account<N>> {
    const { assert!(O::PROGRAM_OWNED && N::PROGRAM_OWNED, "only this program's kinds migrate") };
    let fits = len::<O>().is_none_or(|len| account.data().len() == len);
    if !(fits && holds::<O>(account.entry())) {
        // Already an `N`, or neither. The privileges were checked as the
        // account loaded, and cannot change.
        return input::finish(account.into_entry());
    }
    // SAFETY: `holds` found a valid `O` after the discriminator.
    let old = unsafe { O::read(account.data().get_unchecked(O::DISCRIMINATOR.len()..)) };
    let new = N::from(O::own(old));
    let len = N::DISCRIMINATOR.len() + N::encoded_len(&new.input(), &[])?;
    let rent = rent.get()?.minimum_balance(len);
    if let Some(top_up) = rent.checked_sub(account.lamports()).filter(|&top_up| top_up > 0) {
        // SAFETY: a transfer changes only the payer's lamports.
        let payer = unsafe { CpiAccount::paying(payer) };
        let accounts = [payer, CpiAccount::writable(&mut account)];
        Cpi::new(system, accounts, transfer_data(top_up)).invoke()?;
    }
    account.resize(len)?;
    let data = account.data_mut();
    data[..N::DISCRIMINATOR.len()].copy_from_slice(N::DISCRIMINATOR);
    zeropod::write(&new, &mut data[N::DISCRIMINATOR.len()..])?;
    // SAFETY: this program owns the account, which now holds `N`'s
    // discriminator, then a valid `N` exactly as long as the data.
    Ok(unsafe { Account::new_unchecked(account.into_entry()) })
}

// A loop, not a sliced copy: the slice's bounds check brings in the panic
// path, about 4.8 KB (measured).
fn write_discriminator<K: Kind>(account: &mut impl Wrapper) {
    let data = account.entry_mut().data_mut();
    for (byte, value) in data.iter_mut().zip(K::DISCRIMINATOR) {
        *byte = *value;
    }
}
