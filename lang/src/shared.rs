//! Opt-in dynamically borrowed accounts for programs whose wire format allows aliases.

use alloc::{rc::Rc, vec::Vec};
use core::cell::{Cell, Ref, RefCell, RefMut};

use crate::{Address, ProgramError, Result, input::Entry};

/// A duplicate account shares data borrows, balances, and ownership with its original.
#[derive(Clone)]
pub struct SharedAccount(Rc<Storage>);

struct Storage {
    entry: RefCell<Entry>,
    address: Address,
    owner: Cell<Address>,
    lamports: Cell<u64>,
    len: Cell<usize>,
    signer: bool,
    writable: bool,
}

impl SharedAccount {
    pub(crate) fn new(entry: Entry) -> Self {
        Self(Rc::new(Storage {
            address: *entry.key(),
            owner: Cell::new(*entry.owner()),
            lamports: Cell::new(entry.lamports()),
            len: Cell::new(entry.data().len()),
            signer: entry.is_signer(),
            writable: entry.is_writable(),
            entry: RefCell::new(entry),
        }))
    }

    pub fn address(&self) -> &Address {
        &self.0.address
    }
    pub fn owner(&self) -> Address {
        self.0.owner.get()
    }
    pub fn lamports(&self) -> u64 {
        self.0.lamports.get()
    }
    pub fn data_len(&self) -> usize {
        self.0.len.get()
    }
    pub fn is_signer(&self) -> bool {
        self.0.signer
    }
    pub fn is_writable(&self) -> bool {
        self.0.writable
    }

    /// Balance changes are committed after data borrows end, or before a CPI.
    pub fn set_lamports(&mut self, value: u64) {
        self.0.lamports.set(value);
    }

    pub fn try_borrow(&self) -> Result<Ref<'_, [u8]>> {
        let entry = self.0.entry.try_borrow().map_err(|_| ProgramError::AccountBorrowFailed)?;
        Ok(Ref::map(entry, Entry::data))
    }

    pub fn try_borrow_mut(&mut self) -> Result<RefMut<'_, [u8]>> {
        let entry = self.entry_mut()?;
        Ok(RefMut::map(entry, Entry::data_mut))
    }

    pub fn resize(&mut self, len: usize) -> Result {
        let mut entry = self.entry_mut()?;
        entry.resize(len)?;
        self.0.len.set(len);
        Ok(())
    }

    pub fn close(&mut self) -> Result {
        self.resize(0)?;
        self.0.owner.set(Address::new_from_array([0; 32]));
        self.0.lamports.set(0);
        Ok(())
    }

    /// Flush before returning successfully from a raw-account handler.
    pub fn flush(&self) -> Result {
        let mut entry = self.entry_mut()?;
        entry.set_lamports(self.lamports());
        entry.set_owner(self.owner());
        Ok(())
    }

    fn entry_mut(&self) -> Result<RefMut<'_, Entry>> {
        self.0.entry.try_borrow_mut().map_err(|_| ProgramError::AccountBorrowFailed)
    }

    fn refresh(&self, entry: &Entry) {
        self.0.owner.set(*entry.owner());
        self.0.lamports.set(entry.lamports());
        self.0.len.set(entry.data().len());
    }
}

/// An instruction's account privileges, independently of the supplied account infos.
pub struct AccountMeta {
    pub address: Address,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// Invokes a dynamically sized instruction, borrowing each unique account once.
/// Runtime privilege checks and ownership checks still apply to every write.
pub fn invoke_signed(
    program: &Address,
    metas: &[AccountMeta],
    data: &[u8],
    accounts: &[&SharedAccount],
    signers: &[&[&[u8]]],
) -> Result {
    let mut unique: Vec<&SharedAccount> = Vec::with_capacity(accounts.len());
    for account in accounts {
        if !unique.iter().any(|other| Rc::ptr_eq(&account.0, &other.0)) {
            unique.push(account);
        }
    }
    for account in &unique {
        account.flush()?;
    }
    let mut entries =
        unique.iter().map(|account| account.entry_mut()).collect::<Result<Vec<_>>>()?;
    crate::cpi::invoke_shared(program, metas, data, &mut entries, signers);
    for (account, entry) in unique.iter().zip(&entries) {
        account.refresh(entry);
    }
    Ok(())
}

#[cfg(target_os = "solana")]
#[global_allocator]
static HEAP: Heap = Heap;

#[cfg(target_os = "solana")]
struct Heap;

// SAFETY: SBF execution is single-threaded; allocations are disjoint aligned
// slices of the runtime's 32 KiB heap, and are never reused within an invocation.
#[cfg(target_os = "solana")]
unsafe impl alloc::alloc::GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        const START: usize = 0x300000000;
        const END: usize = START + 32 * 1024;
        let cursor = START as *mut usize;
        // SAFETY: the runtime maps a zero-initialized heap at START.
        let current = unsafe { cursor.read() };
        let current = if current == 0 { END } else { current };
        let next = current.saturating_sub(layout.size()) & !(layout.align() - 1);
        if next < START + core::mem::size_of::<usize>() {
            return core::ptr::null_mut();
        }
        // SAFETY: the first word is reserved for this allocator's cursor.
        unsafe {
            cursor.write(next);
        }
        next as *mut u8
    }
    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{miri::with_accounts, wrapper::UncheckedAccount};

    #[test]
    fn aliases_share_borrows_balances_resizes_and_close() {
        with_accounts(&[(1, &[1, 2, 3])], &[0], |cursor| {
            let mut accounts = cursor.take_rest().into_shared().unwrap();
            let mut alias = accounts.pop().unwrap();
            let account = &mut accounts[0];
            let reader = account.clone();
            let data = account.try_borrow_mut().unwrap();
            assert_eq!(alias.try_borrow().unwrap_err(), ProgramError::AccountBorrowFailed);
            assert_eq!(alias.try_borrow_mut().unwrap_err(), ProgramError::AccountBorrowFailed);
            alias.set_lamports(42);
            assert_eq!(reader.lamports(), 42);
            assert_eq!(alias.flush(), Err(ProgramError::AccountBorrowFailed));
            drop(data);
            alias.resize(5).unwrap();
            assert_eq!(&*account.try_borrow().unwrap(), &[1, 2, 3, 0, 0]);
            assert_eq!(account.data_len(), 5);
            assert_eq!(alias.resize(10244), Err(ProgramError::InvalidRealloc));
            alias.flush().unwrap();
            assert_eq!(account.0.entry.borrow().lamports(), 42);
            alias.close().unwrap();
            account.flush().unwrap();
            assert_eq!(account.data_len(), 0);
            assert_eq!(account.lamports(), 0);
            assert_eq!(account.0.entry.borrow().owner(), &Address::new_from_array([0; 32]));
        });
    }

    #[test]
    fn shared_loading_cannot_alias_an_exclusive_wrapper() {
        with_accounts(&[(1, &[])], &[0], |cursor| {
            let _exclusive = cursor.load::<UncheckedAccount>(false).unwrap();
            assert!(matches!(cursor.take_rest().into_shared(), Err(ProgramError::InvalidArgument)));
        });
        with_accounts(&[], &[0], |cursor| {
            assert!(matches!(cursor.take_rest().into_shared(), Err(ProgramError::InvalidArgument)));
        });
    }
}
