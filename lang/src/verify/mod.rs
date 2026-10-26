//! Verification with Kani, compiled only under Kani: symbolic accounts for
//! property harnesses, and the models of the runtime they run against.

pub(crate) mod models;
mod world;

pub use world::World;

/// An account flag as the runtime serializes it: 0 or 1.
pub(crate) fn any_flag() -> u8 {
    let flag: u8 = kani::any();
    kani::assume(flag <= 1);
    flag
}
