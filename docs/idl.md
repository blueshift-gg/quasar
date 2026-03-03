# IDL Generator

The `quasar-idl` crate is a standalone binary that parses a Quasar program crate's Rust source and produces three outputs:

1. **JSON IDL** (`target/idl/<program>.idl.json`) -- a machine-readable description of instructions, accounts, events, errors, and types
2. **TypeScript client** (`target/idl/<program>.ts`) -- a complete client with instruction builders, account decoders, event decoders, and codec helpers
3. **Rust client module** (`<crate>/src/client.rs`) -- instruction builder structs with `From<...> for Instruction` impls, injected into the program crate behind a `client` feature flag

## Usage

```bash
cargo run -p quasar-idl -- path/to/program-crate
```

Example:

```bash
cargo run -p quasar-idl -- examples/escrow
```

This writes:

```
target/idl/escrow.idl.json
target/idl/escrow.ts
examples/escrow/src/client.rs
```

If `lib.rs` does not already contain `mod client`, the generator injects:

```rust
#[cfg(feature = "client")]
extern crate alloc;
#[cfg(feature = "client")]
pub mod client;
```

The injection point is immediately after the first line (typically `#![no_std]`).

## Pipeline

The generator runs in four phases, all in a single `main()` invocation (`idl/src/main.rs`):

```
Source files  -->  Parser  -->  Collision check  -->  Codegen
                                                      ├─ JSON IDL
                                                      ├─ TypeScript client
                                                      └─ Rust client module
```

### Phase 1: Module Resolution

The entry point is `parser::parse_program(crate_root)` (`idl/src/parser/mod.rs:26`). Before any macro parsing happens, the generator resolves all source files in the crate.

`module_resolver::resolve_crate` (`idl/src/parser/module_resolver.rs:10`) starts at `src/lib.rs` and recursively follows `mod foo;` declarations. For each external module declaration (no inline body), it resolves the file path using standard Rust module resolution rules:

- If the current file is `lib.rs` or `mod.rs`, look in the same directory: `<dir>/<mod_name>.rs` or `<dir>/<mod_name>/mod.rs`
- Otherwise, look in a subdirectory named after the current file: `<dir>/<stem>/<mod_name>.rs`

`#[cfg(test)]` modules are skipped. Each resolved file is parsed into a `syn::File` AST.

```rust
pub struct ResolvedFile {
    pub path: PathBuf,
    pub file: syn::File,
}
```

### Phase 2: Parsing

With the full set of resolved files, the parser extracts five categories of data by walking the AST of every file:

**Program ID** (`parser/program.rs:14`): Finds the `declare_id!("...")` macro invocation in `lib.rs` and extracts the base58 address string.

**Instructions** (`parser/program.rs:27`): Finds the `#[program]` module in `lib.rs`, then extracts each `#[instruction(discriminator = N)]` function. For each function, it captures:
- The function name (converted to camelCase in the IDL)
- The discriminator bytes (single integer or byte array)
- The `Ctx<T>` type parameter (used to look up the accounts struct)
- Additional parameters after `ctx` (instruction arguments, with their `syn::Type`)

**Accounts structs** (`parser/accounts.rs:30`): Finds all `#[derive(Accounts)]` structs across all files. For each field, it determines:
- Mutability: `&'a mut T` sets `writable: true`
- Signer: type base name is `Signer`
- PDA seeds: parsed from `#[account(seeds = [...], bump)]` attributes. Seeds are classified as `ByteString` (literal `b"..."`) or `AccountRef` (identifiers matching sibling field names)
- Known addresses: `SystemProgram`, `Sysvar<Rent>`, `Sysvar<Clock>` are automatically annotated with their well-known addresses

**State accounts** (`parser/state.rs:14`): Finds all `#[account(discriminator = N)]` structs. These appear in both the `accounts` array (name + discriminator) and the `types` array (name + field definitions) in the final IDL.

**Events** (`parser/events.rs:14`): Finds all `#[event(discriminator = N)]` structs. Same dual-entry pattern as state accounts.

**Errors** (`parser/errors.rs:6`): Finds all `#[error_code]` enums. Variants with explicit discriminants set the base code; subsequent variants auto-increment:

```rust
#[error_code]
pub enum MyError {
    InsufficientFunds = 6000,  // code 6000
    InvalidAuthority,          // code 6001
    AccountExpired,            // code 6002
}
```

### Phase 3: Discriminator Collision Detection

Before generating output, `build_idl` calls `check_discriminator_collisions` (`idl/src/parser/mod.rs:150`). This checks for collisions **within the same kind** -- two instructions sharing a discriminator is an error, but an instruction and an account sharing one is allowed (they occupy separate dispatch namespaces).

The collision check collects all entries:

```rust
struct DiscEntry {
    kind: &'static str,   // "instruction", "account", or "event"
    name: String,
    discriminator: Vec<u8>,
}
```

Then performs O(n^2) pairwise comparison within each kind. On collision, the generator prints all collisions and exits with code 1.

This is a build-time safety net. The `#[program]` proc macro also catches instruction discriminator collisions at compile time (including the `0xFF` reservation for self-CPI events), but the IDL generator provides a second layer that covers accounts and events as well.

## JSON IDL Format

The output conforms to the `Idl` struct defined in `idl/src/types.rs`:

```json
{
  "address": "22222222222222222222222222222222222222222222",
  "metadata": {
    "name": "escrow",
    "version": "0.1.0",
    "spec": "0.1.0"
  },
  "instructions": [
    {
      "name": "make",
      "discriminator": [0],
      "accounts": [
        { "name": "maker", "writable": true, "signer": true },
        { "name": "escrow", "writable": true, "pda": {
          "seeds": [
            { "kind": "const", "value": [101, 115, 99, 114, 111, 119] },
            { "kind": "account", "path": "maker" }
          ]
        }},
        { "name": "systemProgram", "address": "11111111111111111111111111111111" }
      ],
      "args": [
        { "name": "deposit", "type": "u64" },
        { "name": "receive", "type": "u64" }
      ]
    }
  ],
  "accounts": [
    { "name": "EscrowAccount", "discriminator": [1] }
  ],
  "events": [
    { "name": "MakeEvent", "discriminator": [0] }
  ],
  "types": [
    {
      "name": "EscrowAccount",
      "type": {
        "kind": "struct",
        "fields": [
          { "name": "maker", "type": "publicKey" },
          { "name": "receive", "type": "u64" },
          { "name": "bump", "type": "u8" }
        ]
      }
    }
  ],
  "errors": [
    { "code": 6000, "name": "InsufficientFunds" }
  ]
}
```

### Type Mapping

The parser maps Rust types to IDL types as follows (`parser/helpers.rs:22`):

| Rust Type | IDL Type |
|-----------|----------|
| `Address`, `Pubkey` | `"publicKey"` |
| `u8`, `u16`, `u32`, `u64`, `u128` | `"u8"`, `"u16"`, etc. |
| `i8` through `i128` | `"i8"` through `"i128"` |
| `bool` | `"bool"` |
| `String<'a, N>` or `String<N>` | `{ "string": { "maxLength": N } }` |
| `Vec<'a, T, N>` or `Vec<T, N>` | `{ "vec": { "items": <T>, "maxLength": N } }` |
| Other named types | `{ "defined": "<TypeName>" }` |

Dynamic types (`String<N>`, `Vec<T, N>`) are detected by inspecting the `syn::Type` AST for angle-bracketed arguments with const generics. The parser handles both lifetimed (`String<'a, 32>`) and non-lifetimed (`String<32>`) variants.

### PDA Seeds

Seeds are serialized in the IDL as:

- `{ "kind": "const", "value": [bytes...] }` for byte string literals like `b"escrow"` (stored as raw UTF-8 bytes)
- `{ "kind": "account", "path": "fieldName" }` for account references (identifiers that match sibling field names in the same `#[derive(Accounts)]` struct)

The parser determines whether an identifier is an account reference by checking it against the list of sibling field names (`parser/accounts.rs:73`).

### Known Addresses

The parser auto-detects certain types and annotates them with their well-known program addresses (`parser/accounts.rs:98`):

| Type | Address |
|------|---------|
| `SystemProgram` | `11111111111111111111111111111111` |
| `Sysvar<Rent>` | `SysvarRent111111111111111111111111111111111` |
| `Sysvar<Clock>` | `SysvarC1ock11111111111111111111111111111111` |

These accounts appear in the IDL with an `address` field, signaling to client generators that they should not be passed as user-provided arguments.

## TypeScript Client

The TypeScript codegen (`idl/src/codegen_ts.rs`) produces a self-contained client file with the following sections:

### Imports

```typescript
import { PublicKey as Address, TransactionInstruction } from "@solana/web3.js";
import { getStructCodec, getU64Codec, ... } from "@solana/codecs";
```

Only the codecs actually used by the IDL's types are imported. The generator collects used codecs by visiting all type fields and instruction args before emitting imports.

### Constants

```typescript
export const PROGRAM_ADDRESS = new Address("22222...");
export const ESCROW_ACCOUNT_DISCRIMINATOR = new Uint8Array([1]);
export const MAKE_EVENT_DISCRIMINATOR = new Uint8Array([0]);
export const MAKE_INSTRUCTION_DISCRIMINATOR = new Uint8Array([0]);
```

### Interfaces and Codecs

For each type (state accounts and events), an interface and a codec are generated:

```typescript
export interface EscrowAccount {
  maker: Address;
  receive: bigint;
  bump: number;
}

export const EscrowAccountCodec = getStructCodec([
  ["maker", getPublicKeyCodec()],
  ["receive", getU64Codec()],
  ["bump", getU8Codec()],
]);
```

Instruction arguments get their own interfaces:

```typescript
export interface MakeInstructionArgs {
  deposit: bigint;
  receive: bigint;
}
```

### Enums

The generator creates discriminated union types for events and instructions:

```typescript
export enum ProgramEvent {
  MakeEvent = "MakeEvent",
}

export type DecodedEvent =
  | { type: ProgramEvent.MakeEvent; data: MakeEvent };

export enum ProgramInstruction {
  Make = "Make",
  Take = "Take",
}

export type DecodedInstruction =
  | { type: ProgramInstruction.Make; args: MakeInstructionArgs }
  | { type: ProgramInstruction.Take; args: TakeInstructionArgs };
```

### Instruction Input Interfaces

For each instruction with user-provided accounts or arguments, the generator creates an `InstructionInput` interface that combines both:

```typescript
export interface MakeInstructionInput {
  maker: Address;       // user-provided account
  mintA: Address;       // user-provided account
  deposit: bigint;      // instruction argument
  receive: bigint;      // instruction argument
}
```

Accounts with fixed addresses (`SystemProgram`, sysvars) and PDA-derived accounts are excluded from the input -- they are resolved internally by the instruction builder.

### Client Class

The client class provides account decoders, event/instruction decoders, and instruction builders:

```typescript
export class EscrowClient {
  constructor(readonly programId: Address = PROGRAM_ADDRESS) {}

  // Account decoders -- validate discriminator, then decode fields
  decodeEscrowAccount(data: Uint8Array): EscrowAccount { ... }

  // Event decoder -- match discriminator, decode payload
  decodeEvent(data: Uint8Array): DecodedEvent | null { ... }

  // Instruction decoder -- match discriminator, decode args
  decodeInstruction(data: Uint8Array): DecodedInstruction | null { ... }

  // Instruction builders -- single input object
  createMakeInstruction(input: MakeInstructionInput): TransactionInstruction { ... }
}
```

Instruction builders take a single `input` parameter (the `XxxInstructionInput` interface) instead of positional arguments. If an instruction has no user-provided accounts and no arguments, the builder takes no parameters.

Builders handle three categories of accounts internally:

- **User-provided**: Read from `input.<accountName>`
- **Fixed-address**: Resolved once from known addresses (`new Address("11111...")`)
- **PDAs**: Derived via `Address.findProgramAddressSync` using the seed definitions from the IDL

Example usage:

```typescript
const client = new EscrowClient();
const ix = client.createMakeInstruction({
  maker: walletAddress,
  mintA: mintAAddress,
  mintB: mintBAddress,
  makerTaA: makerTokenAAddress,
  deposit: 1_000_000n,
  receive: 2_000_000n,
});
```

### Dynamic Field Codecs

When the IDL contains dynamic string or vec types, the generator includes helper codec functions:

- `getDynStringCodec(maxLength)`: Encodes/decodes a `PodU16` length prefix followed by UTF-8 bytes, padded to `2 + maxLength` fixed size
- `getDynVecCodec(itemCodec, maxLength)`: Encodes/decodes a `PodU16` count prefix followed by fixed-size elements, padded to `2 + maxLength * itemCodec.fixedSize`

These match the on-chain memory layout used by Quasar's dynamic fields: a fixed-size ZC header with `PodU16` length descriptors, followed by packed variable-length data.

## Rust Client Module

The Rust codegen (`idl/src/codegen.rs`) produces instruction builder structs:

```rust
use alloc::vec;
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

pub struct MakeInstruction {
    pub maker: Address,
    pub escrow: Address,
    pub system_program: Address,
    pub deposit: u64,
    pub receive: u64,
}

impl From<MakeInstruction> for Instruction {
    fn from(ix: MakeInstruction) -> Instruction {
        let accounts = vec![
            AccountMeta::new(ix.maker, true),       // writable + signer
            AccountMeta::new(ix.escrow, false),      // writable
            AccountMeta::new_readonly(ix.system_program, false),
        ];
        let mut data = vec![0];  // discriminator
        data.extend_from_slice(&ix.deposit.to_le_bytes());
        data.extend_from_slice(&ix.receive.to_le_bytes());
        Instruction {
            program_id: crate::ID,
            accounts,
            data,
        }
    }
}
```

The module is gated behind `#[cfg(feature = "client")]` and requires `alloc` (it uses `Vec` for instruction data construction). This keeps the on-chain binary zero-allocation while providing a convenient client API for tests and off-chain tooling.

## Limitations

- **Expression-based seeds are not supported**: Seeds like `&escrow.maker.to_bytes()` or `value.to_le_bytes()` are ignored. Only byte string literals (`b"escrow"`) and direct field name references (`maker`) are parsed.
- **No `#[path = "..."]` module resolution**: The module resolver follows standard Rust conventions only. Custom `#[path]` attributes on `mod` declarations are not handled.
- **Single-crate scope**: The parser only resolves files within a single crate. Cross-crate type references (e.g., importing an account type from a shared library crate) appear as `{ "defined": "TypeName" }` without field expansion.
- **Token program addresses**: `TokenProgram` and `TokenInterface` are not auto-annotated with known addresses (unlike `SystemProgram` and sysvars). The user must pass these explicitly.
