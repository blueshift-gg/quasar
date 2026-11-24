# Token-2022 on Quasar

A native source port of the Token-2022 program at
[`e8ca680ca8aeb12f9598524b003c776c6a4168f2`](https://github.com/solana-program/token-2022/tree/e8ca680ca8aeb12f9598524b003c776c6a4168f2).
The core processor, batch instructions, and every extension processor from that
revision are present. Original instruction discriminators, packed account
layouts, TLV extensions, authority checks, and error codes are retained.

## Run

From the Quasar repository root:

```sh
make token-2022
```

Or from this directory:

```sh
make check
```

This builds the program and a transfer-hook test fixture, then runs formatting,
Clippy, the adapted upstream processor tests, and SBF extension tests. The root
target also runs Quasar's shared-account unit tests. The tools version is pinned
to `v1.56` and the architecture to `v0`, compatible with the repository's Agave
3.1 test runtime. The program artifact is `target/deploy/quasar_token_2022.so`
under this directory, or `$CARGO_TARGET_DIR/deploy` when set.

This example is an independent Cargo workspace because the pinned upstream
interfaces require newer minor versions than the root workspace's fixture
dependencies. Its lockfile is committed. Root `make test` includes this example.

## Implementation

- `src/entrypoint.rs` uses Quasar's entrypoint and cursor. The upstream dispatcher
  handles variable instruction lengths, batch instructions, and the separate
  metadata/group discriminators. Ordinary fixed-layout `#[program]` handlers
  would change this ABI.
- `src/processor.rs` and `src/extension/` contain the token rules. Account access
  uses Quasar's `SharedAccount`; all system and transfer-hook CPIs use Quasar.
- `src/runtime/` ports account-dependent allocation, sysvar, transfer-hook
  resolution, and proof-context extraction helpers. Cryptographic arithmetic
  and packed interface types use the pinned upstream libraries.
  Native-mint creation uses signed transfer/allocate/assign system instructions
  to support Agave 3.1, which predates upstream's system instruction 13.
- Quasar's opt-in `shared-accounts` feature provides dynamically checked aliasing,
  a 32 KiB SBF heap, and dynamically sized CPIs. Ordinary Quasar programs retain
  exclusive accounts and their allocation-free runtime. Shared handlers flush
  cached balances and owners on success; CPIs flush and refresh them automatically.

The extension set includes transfer fees, confidential transfers and fees,
confidential mint/burn, immutable ownership, nontransferability, permanent
delegates, mint-close authority, default account state, memo transfers, CPI
guards, transfer hooks, interest-bearing and scaled UI amounts, metadata and
its pointer, token groups and their pointers, pausing, and permissioned burns.

Use the pinned `spl-token-2022-interface` and the metadata/group interfaces to
construct instructions. The matching [upstream IDL](https://github.com/solana-program/token-2022/blob/e8ca680ca8aeb12f9598524b003c776c6a4168f2/idl.json)
describes the existing wire protocol; this raw dispatcher does not generate a
Quasar IDL. Off-chain clients and the separate ElGamal registry program remain
upstream components. As upstream does, confidential operations rely on Solana's
ZK proof program or accounts owned by that verifier.

## Verification and limits

Tests cover the upstream core regressions, every mint extension's initialization,
fees and withdrawals, metadata growth/shrinkage with aliased mint accounts,
group membership, pause/resume, rent-funded resizing, signed native-mint
creation, confidential proof-account ownership, and transfer-hook privileges,
transferring flags, and failure rollback. The new shared-account borrowing,
resizing, and close behavior also has Miri regression tests.

This is an unaudited example. The complete upstream client/integration suite and
all combinations of confidential proofs have not been run. Compute usage and
heap consumption differ from upstream; the 32 KiB bump allocator does not
reclaim allocations during an instruction.

The canonical `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` ID is retained for
local compatibility testing. Deployment at another ID requires coordinated
changes to the interface's program-ID checks and native-mint PDA derivation.
No network deployment is part of this example.

## Provenance

Adapted Token-2022 source and tests are Apache-2.0 licensed; see `LICENSE`.
The account-facing helpers in `src/runtime/extensions.rs` and `proof.rs` derive
from the same upstream revision. Transfer-hook resolution follows
`pinocchio-transfer-hook-interface` 0.1.0, also Apache-2.0.

`vendor/zk-elgamal-proof-interface` contains version 1.0.0 of the Apache-2.0
Solana SDK interface. Its Rust source is unchanged. Its manifest selects
`solana-instruction` v3 to match the pinned Token-2022 interfaces; the published
manifest's v4 dependency otherwise causes instruction-type mismatches in those
interfaces. The package also inherits this workspace's lints. No cryptographic
implementation is modified by this dependency patch.
