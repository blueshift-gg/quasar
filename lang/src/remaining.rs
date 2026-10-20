//! The accounts after the ones an accounts struct declares, as its last field
//! `Remaining<T>`. Nothing is copied: iterating walks the input and checks
//! each account as it is reached.

use core::marker::PhantomData;

use crate::{Result, input::Cursor, wrapper::Wrapper};

/// Every account left in the input, each checked as a `W` when it is read.
/// It is iterated once, by value: a second pass would hand out two handles to
/// the same account. Keep what you need, such as addresses, from the first.
pub struct Remaining<W> {
    cursor: Cursor,
    /// Declared `mut`: each account must be writable.
    writable: bool,
    accounts: PhantomData<W>,
}

impl<W: Wrapper> Remaining<W> {
    /// Takes every account the cursor has not loaded.
    pub fn new(cursor: &mut Cursor, writable: bool) -> Self {
        Self { cursor: cursor.take_rest(), writable, accounts: PhantomData }
    }

    /// Accounts not yet read.
    pub fn len(&self) -> usize {
        self.cursor.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cursor.is_empty()
    }
}

impl<W: Wrapper> Iterator for Remaining<W> {
    type Item = Result<W>;

    fn next(&mut self) -> Option<Result<W>> {
        (!self.cursor.is_empty()).then(|| self.cursor.load(self.writable))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
