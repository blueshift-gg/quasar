//! Canonical, hash-verified binary encoding of a [`quasar_schema::Idl`].
//!
//! This crate defines the wire format documented in [`SPEC.md`] and provides
//! encoder/decoder pairs that are byte-for-byte deterministic. The SHA-256
//! digest in the header lets later PRs (on-chain handler, client SDK) verify
//! integrity with a single `sol_sha256` syscall — no parsing required to
//! authenticate.
//!
//! # Stability
//!
//! The wire format is versioned (`wire::VERSION`). Any change that is not
//! backwards-compatible must bump the version byte; encoders continue to emit
//! the new version, decoders reject unknown versions.
//!
//! # Example
//!
//! ```no_run
//! use quasar_schema::Idl;
//! # fn idl() -> Idl { unimplemented!() }
//! let idl: Idl = idl();
//! let blob = quasar_schema_canonical::encode(&idl);
//! let roundtripped = quasar_schema_canonical::decode(&blob).unwrap();
//! assert_eq!(idl, roundtripped);
//! ```
//!
//! [`SPEC.md`]: https://github.com/blueshift-gg/quasar/blob/master/schema-canonical/SPEC.md

mod decode;
mod encode;
mod error;
pub mod wire;

pub use {
    decode::{decode, decode_body},
    encode::{encode, encode_body},
    error::{CanonicalError, DecodeError, EncodeError},
    wire::{MAGIC, VERSION},
};
