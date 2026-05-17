use {
    crate::{
        impl_sysvar_get,
        pod::{PodI64, PodU64},
        sysvars::Sysvar,
    },
    solana_address::Address,
};

const CLOCK_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);

/// Clock sysvar: slot, epoch, and timestamps.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Clock {
    pub slot: PodU64,
    pub epoch_start_timestamp: PodI64,
    pub epoch: PodU64,
    pub leader_schedule_epoch: PodU64,
    pub unix_timestamp: PodI64,
}

const _: () = assert!(core::mem::size_of::<Clock>() == 40);
const _: () = assert!(core::mem::align_of::<Clock>() == 1);

impl Sysvar for Clock {
    impl_sysvar_get!(CLOCK_ID, 0);
}

#[cfg(kani)]
#[path = "../../kani/sysvars/clock.rs"]
mod kani_proofs;
