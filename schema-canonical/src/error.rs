//! Error types for the canonical encoder/decoder.
//!
//! Every decoder rejection has a distinct variant so callers (and tests) can
//! tell failure modes apart without string matching. Encoding is currently
//! infallible; `EncodeError` is reserved for future spec revisions.

use std::{fmt, string::FromUtf8Error};

/// Errors returned by [`crate::decode`] and [`crate::decode_body`].
#[derive(Debug)]
pub enum DecodeError {
    /// First three bytes did not match [`crate::wire::MAGIC`].
    BadMagic,
    /// Header present but version byte is not supported by this reader.
    UnsupportedVersion(u8),
    /// Fewer than 40 bytes available — no header to parse.
    TruncatedHeader,
    /// `body_len` field disagrees with the actual trailing slice length.
    BodyLengthMismatch { expected: u32, actual: usize },
    /// SHA-256 of the body did not match the digest in the header.
    BadDigest,
    /// Unknown enum tag at the named site.
    InvalidTag { location: &'static str, tag: u8 },
    /// A length prefix or multi-byte field ran off the end of the body.
    TruncatedBody { position: usize },
    /// A `String` field was not valid UTF-8.
    InvalidUtf8(FromUtf8Error),
    /// `prefix_bytes` value was not in `{1, 2, 4, 8}`.
    InvalidPrefixBytes(u8),
    /// A `bool` byte was outside `{0, 1}`.
    InvalidBool(u8),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "bad magic bytes (not a quasar canonical IDL blob)"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported canonical IDL version: 0x{v:02X}")
            }
            Self::TruncatedHeader => write!(f, "truncated header (need at least 40 bytes)"),
            Self::BodyLengthMismatch { expected, actual } => write!(
                f,
                "body length mismatch: header declares {expected} bytes, slice has {actual}"
            ),
            Self::BadDigest => write!(f, "body SHA-256 digest did not match header"),
            Self::InvalidTag { location, tag } => {
                write!(f, "invalid tag 0x{tag:02X} at {location}")
            }
            Self::TruncatedBody { position } => {
                write!(f, "truncated body at offset {position}")
            }
            Self::InvalidUtf8(err) => write!(f, "invalid utf-8 in string field: {err}"),
            Self::InvalidPrefixBytes(v) => {
                write!(f, "invalid prefix_bytes {v}; expected one of 1, 2, 4, 8")
            }
            Self::InvalidBool(v) => write!(f, "invalid bool byte 0x{v:02X}; expected 0x00 or 0x01"),
        }
    }
}

impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8(err) => Some(err),
            _ => None,
        }
    }
}

impl From<FromUtf8Error> for DecodeError {
    fn from(err: FromUtf8Error) -> Self {
        Self::InvalidUtf8(err)
    }
}

/// Errors returned by [`crate::encode`] and [`crate::encode_body`].
///
/// Encoding is currently infallible; no variants exist. The type is reserved
/// so a future spec revision (e.g. validating `prefix_bytes` on the way out)
/// can add fallibility without breaking the public signature shape.
#[derive(Debug)]
#[non_exhaustive]
pub enum EncodeError {}

impl fmt::Display for EncodeError {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `EncodeError` has no variants, so this match is exhaustive.
        match *self {}
    }
}

impl std::error::Error for EncodeError {}

/// Unified error for callers that want one type for both directions.
#[derive(Debug)]
pub enum CanonicalError {
    Encode(EncodeError),
    Decode(DecodeError),
}

impl fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "{e}"),
            Self::Decode(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CanonicalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Decode(e) => Some(e),
        }
    }
}

impl From<EncodeError> for CanonicalError {
    fn from(err: EncodeError) -> Self {
        Self::Encode(err)
    }
}

impl From<DecodeError> for CanonicalError {
    fn from(err: DecodeError) -> Self {
        Self::Decode(err)
    }
}
