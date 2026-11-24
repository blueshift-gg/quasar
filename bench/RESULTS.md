# Vault benchmark

A system-owned vault, written in each framework with the same account checks.
The vault is a system-owned PDA, which anyone can fund at the address of any
bump, so the frameworks that check it prove its bump canonical: each candidate
from 255 down is hashed and curve-checked until one is off the curve. Quasar,
Quasar master and the Pinocchio version, which calls `find_program_address`,
do; anchor-next accepts any bump that gives the address.
Agave 4.3.2 through Mollusk 0.16.2, every feature enabled; `bench/rivals.sh` pins
the other revisions. Programs use platform-tools v1.57 and SBPFv3 except the
pinned Quasar master multisig, which requires SBPFv0. Compute units include the
System Program call. Every run
uses the same user, whose vault's canonical bump is 253, so a search from 255
tries 3 bumps.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
| Quasar, this repository | 2187 | 2225 | 4712 |
| Quasar master (b0de7db) | 2198 | 2219 | 5680 |
| anchor-next v2 (2cdb58b) | 2344 | 2391 | 6720 |
| Pinocchio 0.12.0, by hand | 5758 | 5771 | 4600 |

## A vault the program owns

Quasar master's vault example, unmodified, and the same vault written here
(`examples/lamport-vault`): deposits go through the System Program into a vault
the program owns, and withdrawals take lamports straight out, with no call.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
| Quasar, this repository | 2186 | 1035 | 3880 |
| Quasar master (b0de7db) | 2196 | 1035 | 4800 |

## Escrow

Quasar master's escrow example, unmodified, and the same escrow written here
(`examples/escrow`), on classic SPL Token (the pinned ELF in `test/programs`):
make, then take; and make, then refund. Both log the same events; the rewrite
also accepts Token-2022 accounts.
The maker's token account
for mint B and the vault are created by `make`; the taker's account for mint
A by `take`.

| Implementation | make (CU) | take (CU) | refund (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 21010 | 22892 | 10611 | 28632 |
| the same, built with `debug` | 22067 | 24000 | 11179 | 91040 |
| Quasar master (b0de7db) | 21002 | 22872 | 10574 | 41584 |

## Multisig

Quasar master's multisig example, unmodified, and the same multisig written
here (`examples/multisig`): three members, a threshold of two. The rewrite
sizes the configuration to its content.

| Implementation | create (CU) | deposit (CU) | execute (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 2029 | 1960 | 2380 | 24000 |
| Quasar master (b0de7db) | 3347 | 1948 | 2436 | 26440 |

## AMM

`examples/amm`, a constant-product pool on the same classic SPL Token ELF;
no other framework here has one. Initialize creates the pool, its two vaults and its
LP mint; the deposit is the first.

| Implementation | initialize (CU) | deposit (CU) | swap (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|---:|---:|
| Quasar, this repository | 20028 | 17733 | 12073 | 17921 | 42624 |
| the same, built with `debug` | 21246 | 19015 | 12962 | 19157 | 112432 |

## On p-token

The escrow above with p-token, built from a pinned revision by
`bench/rivals.sh`, as the Token program. Initializing the accounts an
instruction creates in one call (p-token's `Batch`) was measured at 620
compute units less for `make` and declined: Token-2022 has no `Batch`, nor
do associated token accounts go through it.

| Implementation | make (CU) | take (CU) | refund (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 8565 | 6927 | 3152 | 28632 |
| Quasar master (b0de7db) | 8557 | 6907 | 3115 | 41584 |

What each instruction's compute units go to: the programs it calls, the
calls themselves at 946 each, the System Program at 150 an
instruction, and what is left: the program's own code and its syscalls.

| Instruction | total | called programs | calls | calls (CU) | System | left |
|---|---:|---:|---:|---:|---:|---:|
| Quasar, this repository, make | 8565 | 576 | 6 | 5676 | 450 | 1863 |
| Quasar, this repository, take | 6927 | 528 | 5 | 4730 | 150 | 1519 |
| Quasar, this repository, refund | 3152 | 201 | 2 | 1892 | 0 | 1059 |
| Quasar master (b0de7db), make | 8557 | 576 | 6 | 5676 | 450 | 1855 |
| Quasar master (b0de7db), take | 6907 | 528 | 5 | 4730 | 150 | 1499 |
| Quasar master (b0de7db), refund | 3115 | 201 | 2 | 1892 | 0 | 1022 |
