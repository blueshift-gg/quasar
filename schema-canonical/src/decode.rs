//! Decoder for the canonical IDL binary format.
//!
//! See `SPEC.md` for the normative spec. Every reader here mirrors the matching
//! writer in `encode.rs` — if one changes, both must change.

use {
    crate::{error::DecodeError, wire::*},
    quasar_schema::{
        Idl, IdlAccountDef, IdlAccountItem, IdlDynString, IdlDynVec, IdlError, IdlEventDef,
        IdlField, IdlInstruction, IdlMetadata, IdlPda, IdlSeed, IdlType, IdlTypeDef,
        IdlTypeDefKind, IdlTypeDefType,
    },
    sha2::{Digest, Sha256},
};

/// Decode a full canonical blob. Verifies magic, version, length, and digest.
pub fn decode(bytes: &[u8]) -> Result<Idl, DecodeError> {
    if bytes.len() < HEADER_LEN {
        return Err(DecodeError::TruncatedHeader);
    }

    if bytes[0..3] != MAGIC {
        return Err(DecodeError::BadMagic);
    }

    let version = bytes[3];
    if version != VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }

    let body_len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let expected_digest: [u8; 32] = bytes[8..40]
        .try_into()
        .expect("slice of 32 bytes is always convertible to [u8; 32]");

    let body = &bytes[HEADER_LEN..];
    if body.len() != body_len as usize {
        return Err(DecodeError::BodyLengthMismatch {
            expected: body_len,
            actual: body.len(),
        });
    }

    let actual_digest = Sha256::digest(body);
    if actual_digest.as_slice() != expected_digest {
        return Err(DecodeError::BadDigest);
    }

    decode_body(body)
}

/// Decode just the body (no header verification). Useful when the header is
/// consumed separately (e.g. by an on-chain handler that validates the digest
/// via `sol_sha256` before invoking higher-level logic).
pub fn decode_body(bytes: &[u8]) -> Result<Idl, DecodeError> {
    let mut r = Reader::new(bytes);
    let idl = read_idl(&mut r)?;
    Ok(idl)
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::TruncatedBody { position: self.pos });
        }
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32, DecodeError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn read_len(&mut self) -> Result<usize, DecodeError> {
        let n = self.read_u32()? as usize;
        // Bounds-check: a length larger than the remaining buffer is invalid,
        // and we prefer to fail fast rather than Vec::with_capacity(4 GiB).
        if n > self.remaining() {
            return Err(DecodeError::TruncatedBody { position: self.pos });
        }
        Ok(n)
    }

    fn read_bool(&mut self) -> Result<bool, DecodeError> {
        match self.read_u8()? {
            0x00 => Ok(false),
            0x01 => Ok(true),
            other => Err(DecodeError::InvalidBool(other)),
        }
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>, DecodeError> {
        let n = self.read_len()?;
        Ok(self.take(n)?.to_vec())
    }

    fn read_string(&mut self) -> Result<String, DecodeError> {
        let bytes = self.read_bytes()?;
        Ok(String::from_utf8(bytes)?)
    }
}

fn read_option<T>(
    r: &mut Reader,
    read_inner: impl FnOnce(&mut Reader) -> Result<T, DecodeError>,
) -> Result<Option<T>, DecodeError> {
    match r.read_u8()? {
        0x00 => Ok(None),
        0x01 => Ok(Some(read_inner(r)?)),
        other => Err(DecodeError::InvalidTag {
            location: "Option discriminant",
            tag: other,
        }),
    }
}

fn read_vec<T>(
    r: &mut Reader,
    mut read_item: impl FnMut(&mut Reader) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    let n = r.read_len()?;
    // Deliberately avoid `Vec::with_capacity(n)` here. `read_len` only bounds
    // `n` by remaining *bytes*, but `T` can be larger than one byte on the
    // heap, so an adversarial length prefix could force a multi-GiB reserve
    // even on a small body. Amortised `push` growth is O(1) and caps waste
    // at 2× the real size.
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(read_item(r)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// IdlType + IdlSeed
// ---------------------------------------------------------------------------

fn validate_prefix_bytes(v: u8) -> Result<usize, DecodeError> {
    match v {
        1 | 2 | 4 | 8 => Ok(v as usize),
        other => Err(DecodeError::InvalidPrefixBytes(other)),
    }
}

fn read_idl_type(r: &mut Reader) -> Result<IdlType, DecodeError> {
    let tag = r.read_u8()?;
    match tag {
        TAG_TYPE_PRIMITIVE => Ok(IdlType::Primitive(r.read_string()?)),
        TAG_TYPE_OPTION => Ok(IdlType::Option {
            option: Box::new(read_idl_type(r)?),
        }),
        TAG_TYPE_DEFINED => Ok(IdlType::Defined {
            defined: r.read_string()?,
        }),
        TAG_TYPE_DYN_STRING => {
            let max_length = r.read_u32()? as usize;
            let prefix_bytes = validate_prefix_bytes(r.read_u8()?)?;
            Ok(IdlType::DynString {
                string: IdlDynString {
                    max_length,
                    prefix_bytes,
                },
            })
        }
        TAG_TYPE_DYN_VEC => {
            let items = Box::new(read_idl_type(r)?);
            let max_length = r.read_u32()? as usize;
            let prefix_bytes = validate_prefix_bytes(r.read_u8()?)?;
            Ok(IdlType::DynVec {
                vec: IdlDynVec {
                    items,
                    max_length,
                    prefix_bytes,
                },
            })
        }
        other => Err(DecodeError::InvalidTag {
            location: "IdlType",
            tag: other,
        }),
    }
}

fn read_idl_seed(r: &mut Reader) -> Result<IdlSeed, DecodeError> {
    let tag = r.read_u8()?;
    match tag {
        TAG_SEED_CONST => Ok(IdlSeed::Const {
            value: r.read_bytes()?,
        }),
        TAG_SEED_ACCOUNT => Ok(IdlSeed::Account {
            path: r.read_string()?,
        }),
        TAG_SEED_ARG => Ok(IdlSeed::Arg {
            path: r.read_string()?,
        }),
        other => Err(DecodeError::InvalidTag {
            location: "IdlSeed",
            tag: other,
        }),
    }
}

// ---------------------------------------------------------------------------
// Leaf structs
// ---------------------------------------------------------------------------

fn read_idl_field(r: &mut Reader) -> Result<IdlField, DecodeError> {
    let name = r.read_string()?;
    let ty = read_idl_type(r)?;
    Ok(IdlField { name, ty })
}

fn read_idl_pda(r: &mut Reader) -> Result<IdlPda, DecodeError> {
    let seeds = read_vec(r, read_idl_seed)?;
    Ok(IdlPda { seeds })
}

fn read_idl_account_item(r: &mut Reader) -> Result<IdlAccountItem, DecodeError> {
    let name = r.read_string()?;
    let writable = r.read_bool()?;
    let signer = r.read_bool()?;
    let pda = read_option(r, read_idl_pda)?;
    let address = read_option(r, |r| r.read_string())?;
    Ok(IdlAccountItem {
        name,
        writable,
        signer,
        pda,
        address,
    })
}

fn read_idl_instruction(r: &mut Reader) -> Result<IdlInstruction, DecodeError> {
    let name = r.read_string()?;
    let discriminator = r.read_bytes()?;
    let has_remaining = r.read_bool()?;
    let accounts = read_vec(r, read_idl_account_item)?;
    let args = read_vec(r, read_idl_field)?;
    let args_layout = read_option(r, |r| r.read_string())?;
    Ok(IdlInstruction {
        name,
        discriminator,
        accounts,
        args,
        has_remaining,
        args_layout,
    })
}

fn read_idl_account_def(r: &mut Reader) -> Result<IdlAccountDef, DecodeError> {
    let name = r.read_string()?;
    let discriminator = r.read_bytes()?;
    Ok(IdlAccountDef {
        name,
        discriminator,
    })
}

fn read_idl_event_def(r: &mut Reader) -> Result<IdlEventDef, DecodeError> {
    let name = r.read_string()?;
    let discriminator = r.read_bytes()?;
    Ok(IdlEventDef {
        name,
        discriminator,
    })
}

fn read_idl_type_def(r: &mut Reader) -> Result<IdlTypeDef, DecodeError> {
    let name = r.read_string()?;
    let kind_tag = r.read_u8()?;
    let kind = match kind_tag {
        TAG_TYPEDEF_STRUCT => IdlTypeDefKind::Struct,
        other => {
            return Err(DecodeError::InvalidTag {
                location: "IdlTypeDefKind",
                tag: other,
            })
        }
    };
    let fields = read_vec(r, read_idl_field)?;
    Ok(IdlTypeDef {
        name,
        ty: IdlTypeDefType { kind, fields },
    })
}

fn read_idl_error(r: &mut Reader) -> Result<IdlError, DecodeError> {
    let code = r.read_u32()?;
    let name = r.read_string()?;
    let msg = read_option(r, |r| r.read_string())?;
    Ok(IdlError { code, name, msg })
}

fn read_idl_metadata(r: &mut Reader) -> Result<IdlMetadata, DecodeError> {
    let name = r.read_string()?;
    let crate_name = r.read_string()?;
    let version = r.read_string()?;
    let spec = r.read_string()?;
    Ok(IdlMetadata {
        name,
        crate_name,
        version,
        spec,
    })
}

fn read_idl(r: &mut Reader) -> Result<Idl, DecodeError> {
    let address = r.read_string()?;
    let metadata = read_idl_metadata(r)?;
    let instructions = read_vec(r, read_idl_instruction)?;
    let accounts = read_vec(r, read_idl_account_def)?;
    let events = read_vec(r, read_idl_event_def)?;
    let types = read_vec(r, read_idl_type_def)?;
    let errors = read_vec(r, read_idl_error)?;
    Ok(Idl {
        address,
        metadata,
        instructions,
        accounts,
        events,
        types,
        errors,
    })
}
