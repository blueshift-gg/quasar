use crate::impl_sysvar_get;
use crate::pod::{PodI64, PodU64};
use {
    crate::sysvars::Sysvar,
    core::mem::{align_of, size_of},
    solana_address::Address,
    solana_program_error::ProgramError,
};

const CLOCK_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Clock {
    pub slot: PodU64,
    pub epoch_start_timestamp: PodI64,
    pub epoch: PodU64,
    pub leader_schedule_epoch: PodU64,
    pub unix_timestamp: PodI64,
}

const _ASSERT_STRUCT_LEN: () = assert!(size_of::<Clock>() == 40);
const _ASSERT_STRUCT_ALIGN: () = assert!(align_of::<Clock>() == 1);

impl Sysvar for Clock {
    impl_sysvar_get!(CLOCK_ID, 0);
}
