use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// The derive pins `Zc = u8` for the whole type, so a generic enum has
// no single discriminant layout to serialize and must be rejected.
#[repr(u8)]
#[derive(QuasarSerialize)]
pub enum Wrapper<T> {
    None,
    _Phantom(core::marker::PhantomData<T>),
}

fn main() {}
