use quasar_core::prelude::*;

#[event(discriminator = 1)]
pub struct SimpleEvent {
    pub value: u64,
}

#[event(discriminator = 2)]
pub struct AddressEvent {
    pub addr: Address,
    pub value: u64,
}

#[event(discriminator = 3)]
pub struct BoolEvent {
    pub flag: bool,
}

#[event(discriminator = 4)]
pub struct MultiEvent {
    pub a: u64,
    pub b: u64,
    pub c: Address,
}
