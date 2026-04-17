//! Encoder for the canonical IDL binary format.
//!
//! See `SPEC.md` for the normative spec. Every helper here maps directly to a
//! named field or enum variant in that document; keep the code and the spec in
//! sync.

use {
    crate::wire::*,
    quasar_schema::{
        Idl, IdlAccountDef, IdlAccountItem, IdlError, IdlEventDef, IdlField, IdlInstruction,
        IdlMetadata, IdlPda, IdlSeed, IdlType, IdlTypeDef, IdlTypeDefKind,
    },
    sha2::{Digest, Sha256},
};

/// Encode a full IDL blob: header + body + digest.
pub fn encode(idl: &Idl) -> Vec<u8> {
    let body = encode_body(idl);
    let digest = Sha256::digest(&body);

    // Total size known up-front: header + body length.
    let mut out = Vec::with_capacity(HEADER_LEN + body.len());
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    // body_len is u32 LE. Panic if the body exceeds 4 GiB — a real IDL is
    // tens of kilobytes at most, so this is unreachable in practice.
    let body_len: u32 = body
        .len()
        .try_into()
        .expect("canonical IDL body length exceeds u32::MAX");
    out.extend_from_slice(&body_len.to_le_bytes());
    out.extend_from_slice(digest.as_slice());
    out.extend_from_slice(&body);

    debug_assert_eq!(out.len(), HEADER_LEN + body.len());
    out
}

/// Encode just the body (no header). Useful when the header will be assembled
/// elsewhere — for example, a build-time const that a program handler hashes
/// on-chain.
pub fn encode_body(idl: &Idl) -> Vec<u8> {
    let mut buf = Vec::new();
    write_idl(&mut buf, idl);
    buf
}

// ---------------------------------------------------------------------------
// Primitive writers
// ---------------------------------------------------------------------------

#[inline]
fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

#[inline]
fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

#[inline]
fn write_len(buf: &mut Vec<u8>, len: usize) {
    let n: u32 = len
        .try_into()
        .expect("canonical IDL length exceeds u32::MAX");
    write_u32(buf, n);
}

#[inline]
fn write_bool(buf: &mut Vec<u8>, b: bool) {
    buf.push(u8::from(b));
}

fn write_bytes(buf: &mut Vec<u8>, bytes: &[u8]) {
    write_len(buf, bytes.len());
    buf.extend_from_slice(bytes);
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_bytes(buf, s.as_bytes());
}

fn write_option<T>(
    buf: &mut Vec<u8>,
    value: Option<&T>,
    write_inner: impl FnOnce(&mut Vec<u8>, &T),
) {
    match value {
        None => buf.push(0x00),
        Some(v) => {
            buf.push(0x01);
            write_inner(buf, v);
        }
    }
}

fn write_vec<T>(buf: &mut Vec<u8>, items: &[T], mut write_item: impl FnMut(&mut Vec<u8>, &T)) {
    write_len(buf, items.len());
    for item in items {
        write_item(buf, item);
    }
}

// ---------------------------------------------------------------------------
// IdlType + IdlSeed
// ---------------------------------------------------------------------------

fn write_idl_type(buf: &mut Vec<u8>, ty: &IdlType) {
    match ty {
        IdlType::Primitive(name) => {
            write_u8(buf, TAG_TYPE_PRIMITIVE);
            write_string(buf, name);
        }
        IdlType::Option { option } => {
            write_u8(buf, TAG_TYPE_OPTION);
            write_idl_type(buf, option);
        }
        IdlType::Defined { defined } => {
            write_u8(buf, TAG_TYPE_DEFINED);
            write_string(buf, defined);
        }
        IdlType::DynString { string } => {
            write_u8(buf, TAG_TYPE_DYN_STRING);
            write_len(buf, string.max_length);
            write_u8(buf, string.prefix_bytes as u8);
        }
        IdlType::DynVec { vec } => {
            write_u8(buf, TAG_TYPE_DYN_VEC);
            write_idl_type(buf, &vec.items);
            write_len(buf, vec.max_length);
            write_u8(buf, vec.prefix_bytes as u8);
        }
    }
}

fn write_idl_seed(buf: &mut Vec<u8>, seed: &IdlSeed) {
    match seed {
        IdlSeed::Const { value } => {
            write_u8(buf, TAG_SEED_CONST);
            write_bytes(buf, value);
        }
        IdlSeed::Account { path } => {
            write_u8(buf, TAG_SEED_ACCOUNT);
            write_string(buf, path);
        }
        IdlSeed::Arg { path } => {
            write_u8(buf, TAG_SEED_ARG);
            write_string(buf, path);
        }
    }
}

// ---------------------------------------------------------------------------
// Leaf structs
// ---------------------------------------------------------------------------

fn write_idl_field(buf: &mut Vec<u8>, field: &IdlField) {
    write_string(buf, &field.name);
    write_idl_type(buf, &field.ty);
}

fn write_idl_pda(buf: &mut Vec<u8>, pda: &IdlPda) {
    write_vec(buf, &pda.seeds, write_idl_seed);
}

fn write_idl_account_item(buf: &mut Vec<u8>, item: &IdlAccountItem) {
    write_string(buf, &item.name);
    write_bool(buf, item.writable);
    write_bool(buf, item.signer);
    write_option(buf, item.pda.as_ref(), write_idl_pda);
    write_option(buf, item.address.as_ref(), |buf, s| write_string(buf, s));
}

fn write_idl_instruction(buf: &mut Vec<u8>, ix: &IdlInstruction) {
    write_string(buf, &ix.name);
    write_bytes(buf, &ix.discriminator);
    write_bool(buf, ix.has_remaining);
    write_vec(buf, &ix.accounts, write_idl_account_item);
    write_vec(buf, &ix.args, write_idl_field);
    write_option(buf, ix.args_layout.as_ref(), |buf, s| write_string(buf, s));
}

fn write_idl_account_def(buf: &mut Vec<u8>, def: &IdlAccountDef) {
    write_string(buf, &def.name);
    write_bytes(buf, &def.discriminator);
}

fn write_idl_event_def(buf: &mut Vec<u8>, def: &IdlEventDef) {
    write_string(buf, &def.name);
    write_bytes(buf, &def.discriminator);
}

fn write_idl_type_def(buf: &mut Vec<u8>, td: &IdlTypeDef) {
    write_string(buf, &td.name);
    // Flatten IdlTypeDefType on the wire — see SPEC.md §4.9.
    let kind_tag: u8 = match td.ty.kind {
        IdlTypeDefKind::Struct => TAG_TYPEDEF_STRUCT,
    };
    write_u8(buf, kind_tag);
    write_vec(buf, &td.ty.fields, write_idl_field);
}

fn write_idl_error(buf: &mut Vec<u8>, err: &IdlError) {
    write_u32(buf, err.code);
    write_string(buf, &err.name);
    write_option(buf, err.msg.as_ref(), |buf, s| write_string(buf, s));
}

fn write_idl_metadata(buf: &mut Vec<u8>, md: &IdlMetadata) {
    write_string(buf, &md.name);
    write_string(buf, &md.crate_name);
    write_string(buf, &md.version);
    write_string(buf, &md.spec);
}

fn write_idl(buf: &mut Vec<u8>, idl: &Idl) {
    write_string(buf, &idl.address);
    write_idl_metadata(buf, &idl.metadata);
    write_vec(buf, &idl.instructions, write_idl_instruction);
    write_vec(buf, &idl.accounts, write_idl_account_def);
    write_vec(buf, &idl.events, write_idl_event_def);
    write_vec(buf, &idl.types, write_idl_type_def);
    write_vec(buf, &idl.errors, write_idl_error);
}
