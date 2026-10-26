# Architecture

Quasar turns the input the Solana runtime hands a program into typed
accounts, runs the handler, and writes nothing back: every account is read
and written in place. This page is the map of the code.

## Crates

| Crate | What it holds |
|---|---|
| `lang` (`quasar-lang`) | the runtime: input, wrappers, account kinds, PDAs, calls, effects; `no_std` |
| `derive` (`quasar-derive`) | `#[derive(Accounts)]`, `#[derive(InstructionData)]`, `#[instruction(n)]`, `#[program]`, `#[account]`, `#[derive(Seeds)]`, `#[event]`, `#[error_code]`, `#[property]` |
| `spl` (`quasar-spl`) | SPL Token and Token-2022 accounts, mints, calls and the `token(..)` and `mint(..)` constraints |
| `metadata` (`quasar-metadata`) | Metaplex metadata/master-edition prefixes, PDA constraints, creation helpers and CPI calls |
| `test` (`quasar-test`) | the program test harness over Mollusk; never a program dependency |
| `cli` (`quasar-cli`) | project scaffolding, build/test/deploy commands, keys, IDL and client generation, linting, profiling and disassembly |
| `idl` / `idl/schema` | the public client schema, adapter for the runtime IDL, client generators and interface compatibility checks |
| `schema` (`quasar-schema`) | shared naming utilities used by the CLI and client generators |
| `profile` (`quasar-profile`) | the CLI's static SBF profiler and local viewer; separate from the execution-trace benchmark profiler |

`examples/` holds programs compared with other frameworks; `tests/features`
uses every feature once, and `tests/callee` is the program it calls.

## One instruction, start to end

1. **Entrypoint** (`context::entrypoint`, emitted by `#[program]`). The runtime
   passes the input in r1 and the instruction data in r2. The
   discriminator is split off, and `dispatch` matches on each
   handler's `DISCRIMINATOR`, which `#[instruction(n)]` declares.
2. **Instruction data** (`data`). The handler's `D` in `Ctx<A, D>` decodes
   field by field, each of one size; the length must match exactly.
3. **Accounts** (`input::Cursor`, `Accounts::from_cursor`, emitted by the
   derive). One line per field, through the `field::Field` trait, so the
   derive never reads a type's name: `cursor.load::<W>(mut)` checks the
   entry's first four bytes, not a duplicate plus the required signer,
   writable and executable flags, in one `u32` compare, then its length, then
   what `W` promises. A check runs as soon as every field it names has
   loaded, in the order written; `seeds(..)` turns the account into its
   `Pda`, and `init` and `migrate` run after every load.
4. **Handler**, an ordinary function over `Ctx<A, D>`, inlined into the
   dispatch.
5. **Deferred effects**, only if the handler succeeded: `close(to = ..)`, and
   settling rent for a variable kind that `init` created or that declares
   `realloc(payer = ..)`.

`docs/lamport-vault.expanded.rs` shows steps 1 to 5 as generated code.

## What keeps it sound

- **One handle per account.** The cursor rejects duplicate entries, so two
  `Entry`s never alias; `Entry` is not `Clone`.
- **Only checks make wrappers.** `Wrapper::from_entry` and `Wrapper::kept`
  need a `Checked` token that only this crate can make: the cursor, a
  recheck after a call, and `init`.
- **Each check once.** What the program has just written (an account `init`
  created, a layout `migrate` wrote) or what the owning program has just
  guaranteed (a token account the Token program initialized or kept through a
  transfer) is not checked again; what nothing guarantees is checked exactly
  once. The rent is read once per instruction, from its `Sysvar<Rent>` if it
  has one.
- **Lengths.** `Account<K>` requires the length of a fixed `K`, so a
  truncated account cannot be read past its end; a variable `K` is at least
  its fixed part and passes ZeroPod's validation, and its field lengths sit
  outside what `DerefMut` reaches, so only its setters change them. Kinds
  have explicit discriminators, never all zeros, and a program's kinds never
  share one (a compile error).
- **A call may change an account under its wrapper** (`cpi`). Wrappers are
  views over the input, so after a call they read the callee's bytes. A
  call says what it leaves of each account (`cpi::After`, in its `ENFORCES`):
  the Token program keeps a token account through a transfer, and its
  wrapper stays usable. Passed writable to a call that could change it and
  does not say so, a wrapper is unusable until `reload()` checks the account
  again: its handle is moved to an address the runtime maps nothing at
  (`Entry::poison`), so a read without `reload()` ends the program with an
  access violation, and the reads that are allowed carry no test. Which
  wrappers that is, is decided when compiling; an account passed read-only
  cannot change. A variable kind is still checked at once after a call to
  this program itself, since its length is read after the handler.
- **PDAs** (`pda`). A `Pda<W, S>` loads only through `seeds(..)`, whose
  values go to `S::seeds`, the seeds its kind declares. Existing accounts are
  checked by hashing, one `sol_sha256` per bump; `init` finds the canonical
  bump with the curve syscall; literal seeds are found while compiling.

## Checks left to calls

A check an account's load or constraint makes is redundant when a call the
handler makes checks the same and every successful run makes that call: a
failed call fails the transaction, so nothing a wrongly loaded account led to
survives. Leaving such checks out is redundancy elimination over three
facts, each a constant:

- **What a check is**: a name (`cpi::Check`), such as `SIGNER_CHECK`, a
  library's `TOKEN_ACCOUNT_CHECK`, or `has_one.maker`. A wrapper names the
  check its load makes (`Wrapper::CHECK`); a constraint key names its own
  (`token::Check::KEYS`). A check relating two accounts is about the other.
- **What a call makes**: `ENFORCES`, per argument, beside the function that
  builds the call: `token::transfer::ENFORCES`, written from the Token
  programs' sources, and `Take::ENFORCES`, which `#[derive(Accounts)]`
  generates from the checks the struct itself makes, so a program calling a
  Quasar program leaves the same checks to it.
- **Which calls a handler always makes**: `#[instruction(n)]` reads the
  handler (`derive/src/vouch.rs`) and lists, per field, the calls that are
  top-level statements no earlier statement can return success past, with
  the field passed by a name bound once and handed to nothing before. A
  `let (a, b) = if .. { (x, y) } else { (y, x) }` is followed down both
  branches, and a field counts only if both vouch for it. What the macro
  cannot tell keeps its check.

`cpi::vouched` joins the three in a `const` block at each check, which
compiles to nothing where it holds. What stays whatever a call makes: the
duplicate and privilege compare, the length a view reads, and any kind
whose view cannot read arbitrary bytes, since a handler may read an account
before the call that checks it.

One check is not left to a call although the runtime makes it: a PDA's
seeds, when the handler signs a call with them. The runtime lets an account
sign a call if it is at the address the caller's seeds derive, but also if it
signed the caller's instruction, so the call proves the seeds only for an
account known not to have signed. Measured at about 200 compute units an
instruction and declined: the hash stays.

Two things keep it honest. Each program's `elisions()` lists what was left
out, from the same constants, and the examples commit it (`elisions.txt`).
And the `debug` feature builds the program making every check itself, and
logging which account failed which check before it fails:
`make sbf` builds both, tests run them on the same instructions, including
accounts only the left-out checks would reject, and require the same
outcomes and state; the benchmark shows what the difference costs.

## Where to change what

| To change | Look in |
|---|---|
| how an account is checked | its wrapper in `wrapper.rs`, `account.rs` or `sysvar.rs` |
| the constraint grammar | `derive/src/accounts.rs`, `parse_constraints` and `checks` |
| what a call does | `cpi.rs`; System calls in `system.rs`; Token calls in `spl/src/token.rs` |
| generated client code and the IDL | `client.rs`, `field.rs` and `derive/src/program.rs` |

## Measured, not assumed

`make check` runs formatting, feature-powerset checks without dev dependencies, clippy
with warnings denied, the SBF builds, Mollusk tests on those builds,
compile-fail tests, Kani proofs of the framework's own code, the benchmark (which must equal
`bench/RESULTS.md`), and the committed expansion. Oracle tests run the
examples against the programs they reimplement, built from pinned revisions
by `bench/rivals.sh`, and require the same outcomes and, where the two store
data alike, the same account state.
CI also runs `make test-miri` and `make test-miri-strict` for the runtime,
SPL and Metaplex integrations. Metaplex instruction encodings and account
metas are compared with the pinned official SDK; its account constraints run
under Mollusk in normal and debug builds.
Optimizations are kept only when `make bench` shows them paying.

## Verification

Under `cfg(kani)`, `verify::World` builds symbolic accounts that passed the
real checks, and `#[property]` turns a predicate over the accounts before and
after an instruction into a Kani harness that runs the real handler. The
runtime's syscalls are replaced by the models in `verify/models.rs`.
