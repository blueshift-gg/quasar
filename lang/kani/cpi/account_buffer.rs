use solana_account_view::AccountView;

/// Stack-allocated account buffer for constructing `AccountView` instances
/// in tests and Kani proofs. Uses `#[repr(align(8))]` for RuntimeAccount
/// alignment. `N` is buffer size in bytes; use `MIN_ACCOUNT_BUF` for a
/// zero-data account.
#[repr(C, align(8))]
pub(crate) struct AccountBuffer<const N: usize> {
    inner: [u8; N],
}

/// Minimum buffer size for a zero-data AccountView.
pub(crate) const MIN_ACCOUNT_BUF: usize =
    core::mem::size_of::<solana_account_view::RuntimeAccount>() + 8;

impl<const N: usize> AccountBuffer<N> {
    pub(crate) fn new() -> Self {
        Self { inner: [0u8; N] }
    }

    fn raw(&mut self) -> *mut solana_account_view::RuntimeAccount {
        self.inner.as_mut_ptr() as *mut solana_account_view::RuntimeAccount
    }

    pub(crate) fn init(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        data_len: usize,
        is_signer: bool,
        is_writable: bool,
        executable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = solana_account_view::NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = executable as u8;
            (*raw).padding = [0u8; 4];
            (*raw).address = solana_address::Address::new_from_array(address);
            (*raw).owner = solana_address::Address::new_from_array(owner);
            (*raw).lamports = 100;
            (*raw).data_len = data_len as u64;
        }
    }

    pub(crate) unsafe fn view(&mut self) -> AccountView {
        // SAFETY: Proof harnesses call `init` before constructing a view.
        unsafe { AccountView::new_unchecked(self.raw()) }
    }
}
