//! Synthetic round-trip tests for the canonical IDL encoding.
//!
//! Every test hand-builds an [`Idl`] or a malformed blob that exercises exactly
//! one encoder/decoder concern. See `SPEC.md` for the wire-format contract
//! being verified here.

use {
    quasar_schema::{
        Idl, IdlAccountDef, IdlAccountItem, IdlDynString, IdlDynVec, IdlError, IdlEventDef,
        IdlField, IdlInstruction, IdlMetadata, IdlPda, IdlSeed, IdlType, IdlTypeDef,
        IdlTypeDefKind, IdlTypeDefType,
    },
    quasar_schema_canonical::{decode, encode, encode_body, wire, DecodeError, MAGIC, VERSION},
    sha2::{Digest, Sha256},
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn empty_metadata() -> IdlMetadata {
    IdlMetadata {
        name: "test_prog".to_string(),
        crate_name: "test-prog".to_string(),
        version: "0.1.0".to_string(),
        spec: "0.1.0".to_string(),
    }
}

fn minimal_idl() -> Idl {
    Idl {
        address: "11111111111111111111111111111111".to_string(),
        metadata: empty_metadata(),
        instructions: vec![],
        accounts: vec![],
        events: vec![],
        types: vec![],
        errors: vec![],
    }
}

/// Encode → decode → assert equal. Also asserts idempotence of re-encoding.
fn assert_round_trip(idl: &Idl) {
    let blob = encode(idl);
    let decoded = decode(&blob).expect("decode should succeed");
    assert_eq!(idl, &decoded, "decoded idl must equal original");

    let reencoded = encode(&decoded);
    assert_eq!(
        blob, reencoded,
        "re-encoding decoded blob must produce identical bytes"
    );
}

// ---------------------------------------------------------------------------
// Positive round-trips
// ---------------------------------------------------------------------------

#[test]
fn encode_decode_empty_idl() {
    assert_round_trip(&minimal_idl());
}

#[test]
fn encode_decode_primitives() {
    // One instruction per primitive type, no accounts.
    let prims = [
        "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "bool", "pubkey",
    ];
    let instructions = prims
        .iter()
        .enumerate()
        .map(|(i, p)| IdlInstruction {
            name: format!("take_{p}"),
            discriminator: vec![i as u8],
            accounts: vec![],
            args: vec![IdlField {
                name: "arg".to_string(),
                ty: IdlType::Primitive((*p).to_string()),
            }],
            has_remaining: false,
            args_layout: None,
        })
        .collect();

    let idl = Idl {
        instructions,
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_option_nested() {
    // Option<Option<u64>> — recursion depth 2.
    let ty = IdlType::Option {
        option: Box::new(IdlType::Option {
            option: Box::new(IdlType::Primitive("u64".to_string())),
        }),
    };
    let idl = Idl {
        instructions: vec![IdlInstruction {
            name: "nested".to_string(),
            discriminator: vec![0xAA],
            accounts: vec![],
            args: vec![IdlField {
                name: "maybe".to_string(),
                ty,
            }],
            has_remaining: false,
            args_layout: None,
        }],
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_dyn_string_every_prefix() {
    let widths = [1usize, 2, 4, 8];
    let args: Vec<IdlField> = widths
        .iter()
        .map(|w| IdlField {
            name: format!("s_p{w}"),
            ty: IdlType::DynString {
                string: IdlDynString {
                    max_length: 64,
                    prefix_bytes: *w,
                },
            },
        })
        .collect();

    let idl = Idl {
        instructions: vec![IdlInstruction {
            name: "strings".to_string(),
            discriminator: vec![1],
            accounts: vec![],
            args,
            has_remaining: false,
            args_layout: None,
        }],
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_dyn_vec_of_defined() {
    // Vec<CustomStruct, 64, u16>
    let ty = IdlType::DynVec {
        vec: IdlDynVec {
            items: Box::new(IdlType::Defined {
                defined: "CustomStruct".to_string(),
            }),
            max_length: 64,
            prefix_bytes: 2,
        },
    };
    let idl = Idl {
        types: vec![IdlTypeDef {
            name: "CustomStruct".to_string(),
            ty: IdlTypeDefType {
                kind: IdlTypeDefKind::Struct,
                fields: vec![IdlField {
                    name: "a".to_string(),
                    ty: IdlType::Primitive("u64".to_string()),
                }],
            },
        }],
        instructions: vec![IdlInstruction {
            name: "bulk".to_string(),
            discriminator: vec![2],
            accounts: vec![],
            args: vec![IdlField {
                name: "items".to_string(),
                ty,
            }],
            has_remaining: false,
            args_layout: None,
        }],
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_every_seed_kind() {
    let pda = IdlPda {
        seeds: vec![
            IdlSeed::Const {
                value: b"prefix".to_vec(),
            },
            IdlSeed::Account {
                path: "authority".to_string(),
            },
            IdlSeed::Arg {
                path: "nonce".to_string(),
            },
        ],
    };

    let idl = Idl {
        instructions: vec![IdlInstruction {
            name: "derive".to_string(),
            discriminator: vec![3],
            accounts: vec![IdlAccountItem {
                name: "pda_account".to_string(),
                writable: true,
                signer: false,
                pda: Some(pda),
                address: None,
            }],
            args: vec![],
            has_remaining: false,
            args_layout: None,
        }],
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_account_item_with_address() {
    let idl = Idl {
        instructions: vec![IdlInstruction {
            name: "fixed_addr".to_string(),
            discriminator: vec![4],
            accounts: vec![IdlAccountItem {
                name: "system_program".to_string(),
                writable: false,
                signer: false,
                pda: None,
                address: Some("11111111111111111111111111111111".to_string()),
            }],
            args: vec![],
            has_remaining: false,
            args_layout: None,
        }],
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_errors_with_and_without_msg() {
    let idl = Idl {
        errors: vec![
            IdlError {
                code: 6000,
                name: "WithMessage".to_string(),
                msg: Some("boom".to_string()),
            },
            IdlError {
                code: 6001,
                name: "NoMessage".to_string(),
                msg: None,
            },
        ],
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_accounts_and_events() {
    let idl = Idl {
        accounts: vec![IdlAccountDef {
            name: "MyState".to_string(),
            discriminator: vec![1, 2, 3, 4, 5, 6, 7, 8],
        }],
        events: vec![IdlEventDef {
            name: "SomethingHappened".to_string(),
            discriminator: vec![0xEE; 8],
        }],
        ..minimal_idl()
    };
    assert_round_trip(&idl);
}

#[test]
fn encode_decode_idempotent() {
    // A richer IDL that stresses more of the encoder at once.
    let idl = Idl {
        address: "SoMeProg1111111111111111111111111111111111".to_string(),
        metadata: empty_metadata(),
        instructions: vec![IdlInstruction {
            name: "complexThing".to_string(),
            discriminator: vec![9, 9, 9, 9, 9, 9, 9, 9],
            accounts: vec![
                IdlAccountItem {
                    name: "payer".to_string(),
                    writable: true,
                    signer: true,
                    pda: None,
                    address: None,
                },
                IdlAccountItem {
                    name: "vault".to_string(),
                    writable: true,
                    signer: false,
                    pda: Some(IdlPda {
                        seeds: vec![IdlSeed::Const {
                            value: b"v".to_vec(),
                        }],
                    }),
                    address: None,
                },
            ],
            args: vec![IdlField {
                name: "amount".to_string(),
                ty: IdlType::Option {
                    option: Box::new(IdlType::Primitive("u64".to_string())),
                },
            }],
            has_remaining: true,
            args_layout: None,
        }],
        accounts: vec![IdlAccountDef {
            name: "Vault".to_string(),
            discriminator: vec![10, 20, 30, 40, 50, 60, 70, 80],
        }],
        events: vec![IdlEventDef {
            name: "Evt".to_string(),
            discriminator: vec![0; 8],
        }],
        types: vec![IdlTypeDef {
            name: "Vault".to_string(),
            ty: IdlTypeDefType {
                kind: IdlTypeDefKind::Struct,
                fields: vec![IdlField {
                    name: "owner".to_string(),
                    ty: IdlType::Primitive("pubkey".to_string()),
                }],
            },
        }],
        errors: vec![IdlError {
            code: 42,
            name: "NotAllowed".to_string(),
            msg: None,
        }],
    };

    let first = encode(&idl);
    let decoded = decode(&first).unwrap();
    let second = encode(&decoded);
    assert_eq!(first, second, "encode is deterministic");

    // Decode twice; must still equal.
    let decoded2 = decode(&second).unwrap();
    assert_eq!(decoded, decoded2);
}

// ---------------------------------------------------------------------------
// Negative cases — every DecodeError variant gets at least one test.
// ---------------------------------------------------------------------------

#[test]
fn decode_rejects_bad_magic() {
    let mut blob = encode(&minimal_idl());
    blob[0] = 0x00; // corrupt magic
    match decode(&blob) {
        Err(DecodeError::BadMagic) => {}
        other => panic!("expected BadMagic, got {other:?}"),
    }
}

#[test]
fn decode_rejects_bad_version() {
    let mut blob = encode(&minimal_idl());
    blob[3] = 0xFF; // unknown version
    match decode(&blob) {
        Err(DecodeError::UnsupportedVersion(0xFF)) => {}
        other => panic!("expected UnsupportedVersion(0xFF), got {other:?}"),
    }
}

#[test]
fn decode_rejects_truncated_header() {
    let short = [0xC1, 0xDE, 0x1C, 0x01];
    match decode(&short) {
        Err(DecodeError::TruncatedHeader) => {}
        other => panic!("expected TruncatedHeader, got {other:?}"),
    }
}

#[test]
fn decode_rejects_body_length_mismatch() {
    let mut blob = encode(&minimal_idl());
    // Bump the declared body_len by 1 so it no longer matches the slice.
    let current = u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]);
    let bumped = current + 1;
    blob[4..8].copy_from_slice(&bumped.to_le_bytes());
    match decode(&blob) {
        Err(DecodeError::BodyLengthMismatch { expected, actual }) => {
            assert_eq!(expected, bumped);
            assert_eq!(actual, current as usize);
        }
        other => panic!("expected BodyLengthMismatch, got {other:?}"),
    }
}

#[test]
fn decode_rejects_bad_digest() {
    let mut blob = encode(&minimal_idl());
    // Flip a bit inside the body — digest no longer matches.
    let body_start = wire::HEADER_LEN;
    blob[body_start] ^= 0xFF;
    match decode(&blob) {
        Err(DecodeError::BadDigest) => {}
        other => panic!("expected BadDigest, got {other:?}"),
    }
}

#[test]
fn decode_rejects_truncated_body() {
    // A body that declares a 1,000-byte string but has only a few bytes after.
    let mut body = Vec::new();
    // address: String with declared length 1000, but no payload.
    body.extend_from_slice(&1000u32.to_le_bytes());
    // No bytes follow — read_len should catch it via the remaining() check.

    // Assemble a valid-looking header so we reach the body reader.
    let digest = Sha256::digest(&body);
    let mut blob = Vec::new();
    blob.extend_from_slice(&MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&(body.len() as u32).to_le_bytes());
    blob.extend_from_slice(digest.as_slice());
    blob.extend_from_slice(&body);

    match decode(&blob) {
        Err(DecodeError::TruncatedBody { .. }) => {}
        other => panic!("expected TruncatedBody, got {other:?}"),
    }
}

#[test]
fn decode_rejects_invalid_utf8() {
    // Build a body where the address string contains invalid UTF-8.
    let mut body = Vec::new();
    // 2-byte "string": 0xFF 0xFE — not valid UTF-8.
    body.extend_from_slice(&2u32.to_le_bytes());
    body.extend_from_slice(&[0xFF, 0xFE]);

    let digest = Sha256::digest(&body);
    let mut blob = Vec::new();
    blob.extend_from_slice(&MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&(body.len() as u32).to_le_bytes());
    blob.extend_from_slice(digest.as_slice());
    blob.extend_from_slice(&body);

    match decode(&blob) {
        Err(DecodeError::InvalidUtf8(_)) => {}
        other => panic!("expected InvalidUtf8, got {other:?}"),
    }
}

#[test]
fn decode_rejects_invalid_prefix_bytes() {
    // Hand-construct a body that encodes a DynString with prefix_bytes=3.
    // Layout so the DynString tag is reached: we put it as the first instruction's
    // first argument type.
    //
    // Building the simplest valid prefix:
    //   address: ""                    (u32 0)
    //   metadata: 4 empty strings      (4 * u32 0)
    //   instructions: len=1
    //     name: ""
    //     discriminator: []
    //     has_remaining: 0
    //     accounts: len=0
    //     args: len=1
    //       name: ""
    //       ty: tag=DYN_STRING, max_length=0, prefix_bytes=3  <-- invalid
    //   accounts/events/types/errors: each len=0

    let mut body = Vec::new();
    // address
    body.extend_from_slice(&0u32.to_le_bytes());
    // metadata: name, crate_name, version, spec
    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    // instructions: 1
    body.extend_from_slice(&1u32.to_le_bytes());
    // instruction[0]
    body.extend_from_slice(&0u32.to_le_bytes()); // name ""
    body.extend_from_slice(&0u32.to_le_bytes()); // discriminator []
    body.push(0); // has_remaining=false
    body.extend_from_slice(&0u32.to_le_bytes()); // accounts: 0
    body.extend_from_slice(&1u32.to_le_bytes()); // args: 1
    body.extend_from_slice(&0u32.to_le_bytes()); // arg name ""
    body.push(wire::TAG_TYPE_DYN_STRING);
    body.extend_from_slice(&0u32.to_le_bytes()); // max_length
    body.push(3); // prefix_bytes = 3 (invalid)
                  // accounts/events/types/errors
    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    }

    let digest = Sha256::digest(&body);
    let mut blob = Vec::new();
    blob.extend_from_slice(&MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&(body.len() as u32).to_le_bytes());
    blob.extend_from_slice(digest.as_slice());
    blob.extend_from_slice(&body);

    match decode(&blob) {
        Err(DecodeError::InvalidPrefixBytes(3)) => {}
        other => panic!("expected InvalidPrefixBytes(3), got {other:?}"),
    }
}

#[test]
fn decode_rejects_invalid_type_tag() {
    // Same skeleton as the prefix_bytes test, but the IdlType tag is garbage.
    let mut body = Vec::new();
    body.extend_from_slice(&0u32.to_le_bytes()); // address
    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    } // metadata
    body.extend_from_slice(&1u32.to_le_bytes()); // instructions=1
    body.extend_from_slice(&0u32.to_le_bytes()); // name
    body.extend_from_slice(&0u32.to_le_bytes()); // disc
    body.push(0); // has_remaining
    body.extend_from_slice(&0u32.to_le_bytes()); // accounts=0
    body.extend_from_slice(&1u32.to_le_bytes()); // args=1
    body.extend_from_slice(&0u32.to_le_bytes()); // arg name
    body.push(0x7F); // invalid IdlType tag
    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    } // accounts/events/types/errors

    let digest = Sha256::digest(&body);
    let mut blob = Vec::new();
    blob.extend_from_slice(&MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&(body.len() as u32).to_le_bytes());
    blob.extend_from_slice(digest.as_slice());
    blob.extend_from_slice(&body);

    match decode(&blob) {
        Err(DecodeError::InvalidTag {
            location: "IdlType",
            tag: 0x7F,
        }) => {}
        other => panic!("expected InvalidTag at IdlType, got {other:?}"),
    }
}

#[test]
fn decode_rejects_invalid_bool() {
    // Body where an instruction's has_remaining byte is 0x02.
    let mut body = Vec::new();
    body.extend_from_slice(&0u32.to_le_bytes()); // address
    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    } // metadata
    body.extend_from_slice(&1u32.to_le_bytes()); // instructions=1
    body.extend_from_slice(&0u32.to_le_bytes()); // name
    body.extend_from_slice(&0u32.to_le_bytes()); // disc
    body.push(0x02); // has_remaining: INVALID
    body.extend_from_slice(&0u32.to_le_bytes()); // accounts=0
    body.extend_from_slice(&0u32.to_le_bytes()); // args=0
    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    }

    let digest = Sha256::digest(&body);
    let mut blob = Vec::new();
    blob.extend_from_slice(&MAGIC);
    blob.push(VERSION);
    blob.extend_from_slice(&(body.len() as u32).to_le_bytes());
    blob.extend_from_slice(digest.as_slice());
    blob.extend_from_slice(&body);

    match decode(&blob) {
        Err(DecodeError::InvalidBool(0x02)) => {}
        other => panic!("expected InvalidBool(0x02), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Determinism: two independent encode calls yield the same bytes.
// ---------------------------------------------------------------------------

#[test]
fn encode_is_deterministic_across_calls() {
    let idl = Idl {
        instructions: vec![IdlInstruction {
            name: "x".to_string(),
            discriminator: vec![1],
            accounts: vec![],
            args: vec![IdlField {
                name: "a".to_string(),
                ty: IdlType::Primitive("u8".to_string()),
            }],
            has_remaining: false,
            args_layout: None,
        }],
        ..minimal_idl()
    };

    let a = encode(&idl);
    let b = encode(&idl);
    assert_eq!(a, b);

    let body_a = encode_body(&idl);
    let body_b = encode_body(&idl);
    assert_eq!(body_a, body_b);
}
