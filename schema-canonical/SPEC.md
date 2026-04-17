# Quasar Canonical IDL Binary Format — v1

This document is the **normative** specification for the canonical binary
encoding of a `quasar_schema::Idl`. The implementation in `src/` is expected to
match this spec byte-for-byte.

The goals of the format are:

1. **Compact** — small enough to fit in a rent-funded Solana account.
2. **Deterministic** — `encode(idl)` is a pure function; the same input always
   produces the same bytes (and therefore the same digest).
3. **Hash-verified** — readers can authenticate the payload with a single
   `sha256` call, matching the Solana `sol_sha256` syscall for cheap on-chain
   verification in later PRs.
4. **Simple** — no varints, no field tags, no hash-map iteration. Fixed-width
   length prefixes and declared field order, so any language can implement a
   decoder.

## 1. Blob layout

```
+--------+--------+----------------------+
| Header | Body   | (nothing else)       |
| 40 B   | N B    |                      |
+--------+--------+----------------------+
```

### 1.1 Header (40 bytes)

| Offset | Size | Field         | Value                                         |
|--------|------|---------------|-----------------------------------------------|
| 0..3   | 3    | `magic`       | `0xC1 0xDE 0x1C`                              |
| 3      | 1    | `version`     | `0x01` (this spec)                            |
| 4..8   | 4    | `body_len`    | `u32` LE; length of body in bytes             |
| 8..40  | 32   | `body_sha256` | `sha256(body)` over exactly `body_len` bytes  |

The header is **not** covered by the digest. This keeps the digest stable
across future header-only additions (e.g. new version bits) that do not change
the body.

A decoder MUST:
- verify the magic bytes exactly (error: `BadMagic`);
- verify `version == 0x01` (error: `UnsupportedVersion(v)`);
- ensure at least 40 bytes are present before indexing (error:
  `TruncatedHeader`);
- ensure `body_len as usize == bytes.len() - 40` (error:
  `BodyLengthMismatch { expected, actual }`);
- compute `sha256(body)` and compare with `body_sha256` (error: `BadDigest`).

### 1.2 Why SHA-256

Solana exposes `sol_sha256` as a native syscall costing ~100 CU. PR-1b will
compute the digest on-chain in one call, no hashing code in the program. BLAKE3
would be a few hundred bytes smaller but cost tens of thousands of CU.

## 2. Primitive encodings

All multi-byte integers are **little-endian**.

| Type            | Encoding                                                  |
|-----------------|-----------------------------------------------------------|
| `u8`            | 1 byte                                                    |
| `u16`           | 2 bytes, LE                                               |
| `u32`           | 4 bytes, LE                                               |
| `bool`          | `0x00` (false) or `0x01` (true); any other byte is an error |
| `String`        | `[len: u32 LE][utf8 bytes]`                               |
| `Vec<u8>`       | `[len: u32 LE][raw bytes]`                                |
| `Vec<T>`        | `[len: u32 LE][T_0][T_1]...[T_{len-1}]`                   |
| `Option<T>`     | `0x00` (None), or `0x01` followed by encoded `T` (Some)   |

All lengths are `u32` LE (max 4 GiB). No varints — the few bytes saved are not
worth the cross-language implementation complexity. Length bounds are checked
at decode time: a length that would over-run the remaining body triggers
`TruncatedBody { position }`.

## 3. Enum tags

### 3.1 `IdlType`

| Tag    | Variant       | Payload                                         |
|--------|---------------|-------------------------------------------------|
| `0x01` | `Primitive`   | `String` — primitive name (e.g. `"u64"`)        |
| `0x02` | `Option`      | `IdlType` (recursive)                           |
| `0x03` | `Defined`     | `String` — defined type name                    |
| `0x04` | `DynString`   | `max_length: u32 LE`, `prefix_bytes: u8`        |
| `0x05` | `DynVec`      | `IdlType`, `max_length: u32 LE`, `prefix_bytes: u8` |

`max_length` is encoded as `u32` even though the Rust field is `usize`. The
encoder returns `EncodeError::MaxLengthOverflow` if the value exceeds
`u32::MAX`; in practice values are orders of magnitude smaller than that.

`prefix_bytes` is constrained to `{1, 2, 4, 8}`. Any other value is a decode
error (`InvalidPrefixBytes`). The encoder does **not** validate this on the
way out — garbage in, garbage out — so the decoder is the source of truth.

### 3.2 `IdlSeed`

| Tag    | Variant   | Payload                 |
|--------|-----------|-------------------------|
| `0x01` | `Const`   | `Vec<u8>` seed bytes    |
| `0x02` | `Account` | `String` — field path   |
| `0x03` | `Arg`     | `String` — arg path     |

### 3.3 `IdlTypeDefKind`

| Tag    | Variant   |
|--------|-----------|
| `0x01` | `Struct`  |

Tag space `0x02..=0xFF` is reserved for future kinds (enums, unions) when they
land in the IDL surface.

## 4. Body layout

Fields are encoded **in the order listed below**. No field tags.

### 4.1 `Idl`

```
address              : String
metadata             : IdlMetadata
instructions         : Vec<IdlInstruction>
accounts             : Vec<IdlAccountDef>
events               : Vec<IdlEventDef>
types                : Vec<IdlTypeDef>
errors               : Vec<IdlError>
```

### 4.2 `IdlMetadata`

```
name                 : String
crate_name           : String
version              : String
spec                 : String
```

### 4.3 `IdlInstruction`

```
name                 : String
discriminator        : Vec<u8>
has_remaining        : bool
accounts             : Vec<IdlAccountItem>
args                 : Vec<IdlField>
args_layout          : Option<String>   # "fixed" | "compact" | absent
```

`args_layout` is `None` for legacy IDLs; a string tag ("fixed" / "compact")
when the source `#[instruction]` was compiled with the zeropod layout
distinction. Treated as an opaque string on the wire — the set of valid
values is governed by the IDL side, not this spec.

### 4.4 `IdlAccountItem`

```
name                 : String
writable             : bool
signer               : bool
pda                  : Option<IdlPda>
address              : Option<String>
```

### 4.5 `IdlPda`

```
seeds                : Vec<IdlSeed>
```

### 4.6 `IdlField`

```
name                 : String
ty                   : IdlType
```

### 4.7 `IdlAccountDef`

```
name                 : String
discriminator        : Vec<u8>
```

### 4.8 `IdlEventDef`

```
name                 : String
discriminator        : Vec<u8>
```

### 4.9 `IdlTypeDef`

```
name                 : String
kind                 : u8   # IdlTypeDefKind tag (0x01 = Struct)
fields               : Vec<IdlField>
```

Note: at the Rust level `IdlTypeDef` nests an `IdlTypeDefType { kind, fields }`;
on the wire the nesting is flattened for compactness.

### 4.10 `IdlError`

```
code                 : u32 LE
name                 : String
msg                  : Option<String>
```

## 5. Determinism guarantees

- No hash-map iteration. Every collection is a `Vec<_>` with declared order.
- No floats, no timestamps, no non-deterministic inputs.
- `encode(idl)` is a pure function: identical `Idl` → identical bytes →
  identical SHA-256 digest.
- Re-encoding a decoded blob yields the identical byte string
  (`encode(decode(b)) == b`), provided the input was produced by this encoder.

## 6. Error catalogue

Decoder errors (`DecodeError`):

- `BadMagic` — first 3 bytes ≠ `0xC1 0xDE 0x1C`.
- `UnsupportedVersion(u8)` — version byte not in this spec.
- `TruncatedHeader` — fewer than 40 bytes available.
- `BodyLengthMismatch { expected, actual }` — declared `body_len` ≠ trailing
  slice length.
- `BadDigest` — SHA-256 of body does not match header.
- `InvalidTag { location, tag }` — unknown variant tag at the named site.
- `TruncatedBody { position }` — a length prefix requests more bytes than
  remain.
- `InvalidUtf8` — a `String` field contained non-UTF-8 bytes.
- `InvalidPrefixBytes(u8)` — `prefix_bytes` outside `{1, 2, 4, 8}`.
- `InvalidBool(u8)` — a `bool` byte outside `{0, 1}`.

Encoder errors (`EncodeError`):

- `MaxLengthOverflow` — a `usize` length exceeded `u32::MAX` when packed.

In practice encoder errors are unreachable under normal inputs, but the API is
fallible so that future spec revisions can add validation without an
incompatible signature change.

## 7. Version history

- **v1** (this document) — initial release.
