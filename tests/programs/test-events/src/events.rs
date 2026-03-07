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

#[event(discriminator = 5)]
pub struct EmptyEvent {}

#[event(discriminator = 6)]
pub struct LargeEvent {
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
    pub e: Address,
    pub f: Address,
    pub g: u128,
    pub h: u128,
}

#[event(discriminator = 7)]
pub struct SecondSimpleEvent {
    pub value: u64,
}
