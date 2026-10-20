//! Symbolic accounts for `#[property]` harnesses.

use alloc::{vec, vec::Vec};
use core::{marker::PhantomData, slice};

use crate::{
    Address,
    context::{Accounts, Ctx},
    input::{Cursor, Header, NOT_DUPLICATE, entry_size},
    verify::any_flag,
};

/// Every account an accounts struct declares, with any flags, key, owner,
/// balance and data. Harnesses explore only the paths where the struct's
/// checks pass. Accounts without a fixed length are modelled with no data.
///
/// The accounts `before`, `ctx` and `after` return point into this world;
/// nothing ties them to its lifetime, so a harness keeps the world alive.
pub struct World<A, D> {
    input: Vec<u64>,
    before: Vec<u64>,
    accounts: PhantomData<(A, D)>,
}

impl<A: Accounts<D>, D> World<A, D> {
    pub fn any() -> Self {
        let lengths =
            A::FIELDS.iter().filter(|field| !field.rest).map(|field| field.len.unwrap_or(0));
        let size: usize = lengths.clone().map(entry_size).sum();
        let mut input = vec![0u64; 1 + size / 8];
        input[0] = lengths.clone().count() as u64;
        let bytes = input.as_mut_ptr().cast::<u8>();
        let mut at = size_of::<u64>();
        for len in lengths {
            let header = Header {
                dup: NOT_DUPLICATE,
                is_signer: any_flag(),
                is_writable: any_flag(),
                executable: any_flag(),
                resize_delta: 0,
                key: Address::new_from_array(kani::any()),
                owner: Address::new_from_array(kani::any()),
                lamports: kani::any(),
                data_len: len as u64,
            };
            // SAFETY: `at` is where this entry starts inside `input`, which
            // has room for the header, `len` bytes of data and the reserve.
            unsafe { bytes.add(at).cast::<Header>().write(header) };
            // SAFETY: the data follows the header, inside `input`.
            let data =
                unsafe { slice::from_raw_parts_mut(bytes.add(at + size_of::<Header>()), len) };
            data.fill_with(kani::any);
            at += entry_size(len);
        }
        let before = input.clone();
        Self { input, before, accounts: PhantomData }
    }

    /// What the handler receives: the accounts, checked, and `data`.
    pub fn ctx<V: crate::cpi::Vouches>(&mut self, data: D) -> Ctx<A, D> {
        let parsed = A::from_cursor::<V>(&mut cursor(&mut self.input), &data);
        kani::assume(parsed.is_ok());
        let accounts = parsed.unwrap_or_else(|_| unreachable!());
        Ctx { accounts, data }
    }

    /// The accounts as they were before the instruction.
    pub fn before(&mut self) -> A {
        let loaded = A::load_accounts(&mut cursor(&mut self.before));
        kani::assume(loaded.is_ok());
        loaded.unwrap_or_else(|_| unreachable!())
    }

    /// The accounts after the instruction, which must still pass their
    /// wrappers' checks.
    pub fn after(&mut self) -> A {
        let loaded = A::load_accounts(&mut cursor(&mut self.input));
        assert!(loaded.is_ok(), "the accounts no longer pass their checks after the instruction");
        loaded.unwrap_or_else(|_| unreachable!())
    }
}

fn cursor(input: &mut [u64]) -> Cursor {
    // SAFETY: `input` is laid out like the runtime's input.
    unsafe {
        Cursor::new(
            input.as_mut_ptr().cast(),
            &<crate::wrapper::SystemProgram as crate::wrapper::ProgramId>::ID,
        )
    }
}
