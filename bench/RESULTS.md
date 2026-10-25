# Vault benchmark

A system-owned vault, written in each framework with the same account checks.
The vault is a system-owned PDA, which anyone can fund at the address of any
bump, so the frameworks that check it prove its bump canonical: each candidate
from 255 down is hashed and curve-checked until one is off the curve. Quasar,
Quasar master and the Pinocchio version, which calls `find_program_address`,
do; anchor-next accepts any bump that gives the address.
Agave 4.3.2 through Mollusk 0.16.2, every feature enabled; `bench/rivals.sh` pins
the other revisions. Programs use SBPFv3 except the pinned Quasar master multisig,
which requires SBPFv0. Compute units include the System Program call. Every run
uses the same user, whose vault's canonical bump is 253, so a search from 255
tries 3 bumps.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
| Quasar, this repository | 2190 | 2218 | 4656 |
| Quasar master (b0de7db) | 2214 | 2231 | 6024 |
| anchor-next v2 (dbb5bae) | 2275 | 2336 | 8656 |
| Pinocchio 0.11, by hand | 5877 | 5890 | 5472 |

## A vault the program owns

Quasar master's vault example, unmodified, and the same vault written here
(`examples/lamport-vault`): deposits go through the System Program into a vault
the program owns, and withdrawals take lamports straight out, with no call.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
| Quasar, this repository | 2187 | 1035 | 3968 |
| Quasar master (b0de7db) | 2214 | 1052 | 5168 |

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
| Quasar, this repository | 20970 | 22837 | 10579 | 26600 |
| the same, built with `debug` | 22089 | 24029 | 11191 | 92760 |
| Quasar master (b0de7db) | 20981 | 22846 | 10556 | 42088 |

## Multisig

Quasar master's multisig example, unmodified, and the same multisig written
here (`examples/multisig`): three members, a threshold of two. The rewrite
sizes the configuration to its content.

| Implementation | create (CU) | deposit (CU) | execute (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 2021 | 1960 | 2385 | 24152 |
| Quasar master (b0de7db) | 3369 | 1948 | 2455 | 26536 |

## AMM

`examples/amm`, a constant-product pool on the same classic SPL Token ELF;
no other framework here has one. Initialize creates the pool, its two vaults and its
LP mint; the deposit is the first.

| Implementation | initialize (CU) | deposit (CU) | swap (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|---:|---:|
| Quasar, this repository | 20029 | 17724 | 12116 | 17950 | 41528 |
| the same, built with `debug` | 21255 | 19059 | 13031 | 19226 | 113768 |

## On p-token

The escrow above with p-token, built from a pinned revision by
`bench/rivals.sh`, as the Token program. Initializing the accounts an
instruction creates in one call (p-token's `Batch`) was measured at 620
compute units less for `make` and declined: Token-2022 has no `Batch`, nor
do associated token accounts go through it.

| Implementation | make (CU) | take (CU) | refund (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 8491 | 6847 | 3113 | 26600 |
| Quasar master (b0de7db) | 8502 | 6856 | 3090 | 42088 |

What each instruction's compute units go to: the programs it calls, the
calls themselves at 946 each, the System Program at 150 an
instruction, and what is left: the program's own code and its syscalls.

| Instruction | total | called programs | calls | calls (CU) | System | left |
|---|---:|---:|---:|---:|---:|---:|
| Quasar, this repository, make | 8491 | 542 | 6 | 5676 | 450 | 1823 |
| Quasar, this repository, take | 6847 | 503 | 5 | 4730 | 150 | 1464 |
| Quasar, this repository, refund | 3113 | 194 | 2 | 1892 | 0 | 1027 |
| Quasar master (b0de7db), make | 8502 | 542 | 6 | 5676 | 450 | 1834 |
| Quasar master (b0de7db), take | 6856 | 503 | 5 | 4730 | 150 | 1473 |
| Quasar master (b0de7db), refund | 3090 | 194 | 2 | 1892 | 0 | 1004 |
