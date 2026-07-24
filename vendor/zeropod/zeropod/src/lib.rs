#![no_std]

pub mod error;
pub mod pod;
pub mod traits;

pub use {
    error::ZeroPodError,
    traits::{
        LayoutKind, ZcElem, ZcField, ZcValidate, ZeroPodCompact, ZeroPodFixed, ZeroPodSchema,
    },
    zeropod_derive::ZeroPod,
};

// Schema-friendly aliases to pod storage types.
// These are NOT a separate abstraction layer — they ARE PodString/PodVec
// with default prefix sizes.
pub type String<const N: usize> = pod::PodString<N, 1>;

/// Schema-friendly Vec alias. Maps native types to their pod companions
/// via `ZcField`, so `Vec<u64, 8>` becomes `PodVec<PodU64, 8, 2>`.
#[allow(type_alias_bounds)]
pub type Vec<T: ZcField<Pod: ZcElem>, const N: usize> = pod::PodVec<<T as ZcField>::Pod, N, 2>;
