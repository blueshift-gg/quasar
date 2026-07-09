#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

// `AccountLoad` is sealed behind the `StaticView` supertrait: a type that is
// not `#[repr(transparent)]` over `AccountView` cannot implement `StaticView`,
// and therefore cannot implement `AccountLoad`. Implementing `AccountLoad`
// without `StaticView` must fail to compile — otherwise the pointer-cast
// constructors would reach UB from safe code.
struct NotTransparent {
    _data: u64,
}

impl AsAccountView for NotTransparent {
    fn to_account_view(&self) -> &AccountView {
        unreachable!()
    }
}

impl AccountLoad for NotTransparent {
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
