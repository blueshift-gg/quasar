# Vault benchmark

A system-owned vault, written in each framework with the same account checks.
The vault is a system-owned PDA, which anyone can fund at the address of any
bump, so the frameworks that check it prove its bump canonical: each candidate
from 255 down is hashed and curve-checked until one is off the curve. Quasar,
Quasar master and the Pinocchio version, which calls `find_program_address`,
do; anchor-next accepts any bump that gives the address.
Agave 3.1.14 through Mollusk 0.10.3, every feature enabled; `bench/rivals.sh` pins
the other revisions. Compute units include the System Program call. Every run
uses the same user, whose vault's canonical bump is 253, so a search from 255
tries 3 bumps.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
| Quasar, this repository | 2189 | 2216 | 5608 |
| Quasar master (b0de7db) | 2198 | 2215 | 6440 |
| anchor-next v2 (dbb5bae) | 2280 | 2331 | 9584 |
| Pinocchio 0.11, by hand | 5877 | 5890 | 6344 |

## A vault the program owns

Quasar master's vault example, unmodified, and the same vault written here
(`examples/lamport-vault`): deposits go through the System Program into a vault
the program owns, and withdrawals take lamports straight out, with no call.

| Implementation | deposit (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|
| Quasar, this repository | 2187 | 1035 | 4920 |
| Quasar master (b0de7db) | 2198 | 1035 | 5520 |

## Escrow

Quasar master's escrow example, unmodified, and the same escrow written here
(`examples/escrow`), on the SPL Token program: make, then take; and make, then
refund. Both log the same events; the rewrite also accepts Token-2022 accounts.
The maker's token account
for mint B and the vault are created by `make`; the taker's account for mint
A by `take`.

| Implementation | make (CU) | take (CU) | refund (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 20983 | 22846 | 10582 | 28048 |
| the same, built with `debug` | 22105 | 24040 | 11196 | 99776 |
| Quasar master (b0de7db) | 20990 | 22849 | 10556 | 44064 |

## Multisig

Quasar master's multisig example, unmodified, and the same multisig written
here (`examples/multisig`): three members, a threshold of two. The rewrite
sizes the configuration to its content.

| Implementation | create (CU) | deposit (CU) | execute (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 2027 | 1960 | 2385 | 25760 |
| Quasar master (b0de7db) | 3370 | 1948 | 2455 | 26536 |

## AMM

`examples/amm`, a constant-product pool on the SPL Token program; no other
framework here has one. Initialize creates the pool, its two vaults and its
LP mint; the deposit is the first.

| Implementation | initialize (CU) | deposit (CU) | swap (CU) | withdraw (CU) | binary (bytes) |
|---|---:|---:|---:|---:|---:|
| Quasar, this repository | 20044 | 17724 | 12116 | 17950 | 43112 |
| the same, built with `debug` | 21282 | 19058 | 13031 | 19226 | 120696 |

## On p-token

The escrow above with p-token, built from a pinned revision by
`bench/rivals.sh`, as the Token program. Initializing the accounts an
instruction creates in one call (p-token's `Batch`) was measured at 620
compute units less for `make` and declined: Token-2022 has no `Batch`, nor
do associated token accounts go through it.

| Implementation | make (CU) | take (CU) | refund (CU) | binary (bytes) |
|---|---:|---:|---:|---:|
| Quasar, this repository | 8512 | 6860 | 3116 | 28048 |
| Quasar master (b0de7db) | 8519 | 6863 | 3090 | 44064 |

What each instruction's compute units go to: the programs it calls, the
calls themselves at 946 each, the System Program at 150 an
instruction, and what is left: the program's own code and its syscalls.

| Instruction | total | called programs | calls | calls (CU) | System | left |
|---|---:|---:|---:|---:|---:|---:|
| Quasar, this repository, make | 8512 | 550 | 6 | 5676 | 450 | 1836 |
| Quasar, this repository, take | 6860 | 507 | 5 | 4730 | 150 | 1473 |
| Quasar, this repository, refund | 3116 | 194 | 2 | 1892 | 0 | 1030 |
| Quasar master (b0de7db), make | 8519 | 550 | 6 | 5676 | 450 | 1843 |
| Quasar master (b0de7db), take | 6863 | 507 | 5 | 4730 | 150 | 1476 |
| Quasar master (b0de7db), refund | 3090 | 194 | 2 | 1892 | 0 | 1004 |
