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
pub type Vec<T, const N: usize> = pod::PodVec<T, N, 2>;
