# Testing

This document defines what **tested** means in this repository: the layers of
the test suite, the per-feature contract every framework feature must satisfy,
the assertion rules that make a test count, and the policy that keeps all of it
true as the framework grows.

Quasar validates untrusted input on behalf of programs that hold funds. The
standard is therefore adversarial: a feature is tested when the suite proves it
**rejects** every documented hostile input with the exact expected error — not
when a happy path passes.

Two numbers summarize assurance, and both are enforced in CI:

- **The feature matrix has no empty required cells.** Every shipped feature
  declares its required test kinds in [`tests/feature-matrix.tsv`](tests/feature-matrix.tsv);
  `make check-test-matrix` fails if any required cell has no tests.
- **The mutation baseline only shrinks.** `cargo mutants` runs nightly against
  [`.ci/mutants-baseline.txt`](.ci/mutants-baseline.txt); a code mutation that
  no test catches — and that is not already recorded in the baseline — fails
  the run. Line coverage can look complete while asserting nothing; a mutation
  score cannot.

## The layers

Each layer catches a failure class the others cannot. A change is tested at
the layer where its failure would occur.

| Layer | Catches | Lives in | Run locally |
|---|---|---|---|
| Host unit + integration | logic bugs in host-runnable code | `#[cfg(test)]` modules, `lang/tests/`, `derive/`, `idl/`, `cli/tests/` | `make test-host` |
| SVM integration (Mollusk) | runtime validation behavior, end to end through real SBF binaries | `tests/programs/*` (fixture programs) + `tests/suite/` (assertions) | `make build-sbf && make test-sbf-host` |
| Compiler diagnostics (trybuild) | macro error-message drift | `*/tests/compile_fail/` + `.stderr` goldens | part of `make test-host`; regen: `make test-bless` |
| Macro-expansion snapshots | codegen drift | `compatibility-baselines/*/proc-macros/` | `make check-proc-macro-baselines` |
| IDL golden + wire baselines | IDL contract drift | `cli/tests/goldens/`, `compatibility-baselines/*/idl-wire/` | `make check-idl-wire-baselines` |
| Generated-client baselines | client codegen drift (Rust/TS/Py/Go/C) | `compatibility-baselines/*/generated-clients/` | `make check-generated-client-baselines` |
| Public API baseline | accidental API breaks | `api-baselines/` | `make check-public-api` |
| Miri | undefined behavior in unsafe code | `*/tests/miri.rs` | `make test-miri` / `make test-miri-strict` |
| Kani proofs | exhaustive verification of parsing/unsafe cores | `lang/kani/`, `spl/`, `metadata/` | `make kani` |
| Fuzzing | decoder crashes on arbitrary bytes | `lang/fuzz/` | `make test-fuzz-build`; run: `cd lang && cargo +$(make nightly-version) fuzz run <target>` |
| CU + size benchmarks | performance regressions in tracked programs | `examples/*/src/tests.rs` + `scripts/bench-tracked-programs.sh` | `make bench-tracked && make compare-tracked` |
| Mutation testing | tests that execute code without constraining it | `.cargo/mutants.toml` + `.ci/mutants-baseline.txt` | `make mutants` |
| Feature matrix | features shipped without their required tests | `tests/feature-matrix.tsv` + `scripts/test-matrix.py` | `make check-test-matrix` |

The full gate is `make test`. CI runs three tiers:

- **PR tier** (`ci.yml`, required): everything in the layer table above except
  Kani-full/mutation/fuzz, plus the matrix, silence, and oracle policies via
  the workspace-invariants job — and an informational `mutants_in_diff` job
  that mutates only the PR's changed lines and reports survivors in the job
  summary.
- **Nightly tier** (`nightly.yml`): full mutation runs judged against the
  shrink-only baseline, the host-coverage artifact, and unconditional Kani on
  all three proof crates (closing the path-gating hole where a lang-adjacent
  PR skips proofs). Fuzzing runs nightly at 300s per target (`fuzz.yml`).
- **Weekly**: an 1800s-per-target fuzz soak (Saturday schedule in `fuzz.yml`).

`make test-host-inventory` proves every host `#[test]` maps to a Cargo target
that required CI actually runs.

Bless/regen commands (`make test-bless`, `bless-proc-macro-baselines`,
`bless-idl-wire-baselines`, `bless-generated-client-baselines`,
`bless-public-api`, `UPDATE_GOLDEN=1` for the CLI IDL golden) are documented in
[CONTRIBUTING.md](CONTRIBUTING.md); the rule everywhere is the same —
**a regenerated golden is reviewed like code, never blessed blindly.**

## The per-feature test contract

A framework feature is anything a user program can invoke or declare: an
account wrapper, an attribute or constraint, a derive macro, an op, a sysvar
accessor, a CPI builder, a dispatch mode. Every feature MUST have:

1. **A happy-path SVM test that asserts state, not status.** Decode the
   resulting account bytes and assert field values (`resulting_accounts`,
   `get_account`). `is_ok()` alone proves nothing about what was written.
2. **One SVM rejection test per documented failure mode**, each asserting the
   **exact** error via `assert_error(...)` with a named constant (see Oracles
   below). If a failure mode has no rejection test, the check guarding it can
   be deleted without CI noticing.
3. **Adversarial variants** wherever the check guards funds:
   - single-bit corruption of the compared value (model:
     `has_one_single_bit_diff` in `tests/suite/src/constraints.rs`) — proves
     the comparison is not truncated;
   - account substitution (valid layout, wrong address/owner/mint);
   - duplicated accounts where distinct ones are required;
   - wrong signer / writability / owner on the account header;
   - data truncated one byte short of the fixed size — proves the length
     check, not just the parse, rejects.
4. **A trybuild compile-fail case** if the feature has macro-surface misuse
   that must be rejected at compile time.
5. **A macro-expansion snapshot** if it changes what the derives emit.
6. **A Kani proof** if it touches `unsafe`, pointer arithmetic, or byte
   parsing (the proof harness is the spec of the trust boundary).
7. **A CU benchmark entry** if it executes on the instruction hot path of a
   tracked program.
8. **A row in `tests/feature-matrix.tsv`** declaring which of the above apply.
   `make check-test-matrix` enforces the declaration; the row is the feature's
   testing spec, reviewed with the feature.

Fixture instructions live in `tests/programs/*`; assertions live in
`tests/suite/src/`. When a rejection test needs an instruction variant the
fixture program lacks, extend the fixture program — a missing fixture is never
a reason to skip the rejection test.

## Oracles

- **Failure tests assert the exact error.** Use the framework error enum, not
  magic numbers:

  ```rust
  use quasar_lang::prelude::QuasarError;

  result.assert_error(ProgramError::Custom(QuasarError::HasOneMismatch as u32));
  ```

  A bare `is_err()` cannot distinguish "the has_one check fired" from "an
  earlier owner check masked a broken has_one" — the regression it exists to
  catch. `make check-suite-oracles` enforces this: every suite test that
  asserts `is_err()` must also pin the exact error in the same test
  (`assert_error`, `ProgramResult::Failure`, or a raw `InstructionError`
  comparison). The allowlist in `scripts/check-suite-oracles.py` names the
  exempt modules with their reasons and only shrinks.
- **Success tests assert resulting state.** Decode and compare fields; for
  lamport-moving instructions, assert both sides of the transfer.
- **A new rejection test must fail when its assertion is inverted.** If you
  can flip the expected error and the test still passes, the test asserts
  nothing (self-check: this is what the mutation tier automates).

## Naming and noise

- Rejection tests are named `<feature>_rejects_<attack>` (or `_fails_` where
  the actor is the framework, e.g. `migrate_rejects_wrong_discriminator`).
  The matrix counts negative coverage by oracle presence, not by name — but
  reviewers read by name.
- **Tests are silent on success.** No `println!` — anything worth printing is
  worth asserting; progress decoration belongs to the harness. CU
  measurements are written to `target/cu-bench/*.jsonl` through
  `examples/cu_bench.rs`, never printed for a human to eyeball.
  `make check-test-silence` enforces this.

## Adding a new framework feature — definition of done

- [ ] Fixture instruction(s) in the owning `tests/programs/*` crate
- [ ] Suite module (or extension) satisfying the contract above
- [ ] Row in `tests/feature-matrix.tsv`; `make check-test-matrix` passes
- [ ] trybuild / snapshot / Kani / bench cells filled where the contract
      requires them
- [ ] `make test` green; goldens regenerated deliberately and reviewed
- [ ] Nightly tiers stay green (mutation baseline did not grow)

## Maintenance policy

Mechanical checks do the enforcement; this section defines the human rules.

- **Ratchets only shrink.** `.ci/mutants-baseline.txt` and the
  `check-suite-oracles` allowlist record accepted debt. Removing entries is
  always welcome and needs no justification. Adding one requires the PR to
  explain why the mutant is unkillable (equivalent mutation) or why the error
  cannot be pinned — "the check obviously works" is not an argument; the
  baseline is the argument.
- **Test code is reviewed like runtime code.** A success-only module is
  rejected in review. A blessed golden diff without line-by-line review is
  rejected. A weakened assertion is a semantic change, not a cleanup.
- **Every bug fix ships its regression test** — a test that fails before the
  fix and references the issue in its name or a comment. No exceptions: the
  bug proves the suite had a hole; the fix must close the hole, not just the
  bug.
- **Cadence.** Quarterly, or before each release, whichever comes first:
  re-run the deep tier, triage every mutation-baseline entry and oracle
  allowlist entry, prune tests that no longer assert anything real, and
  re-baseline the CU budgets (`accepted_*_delta`) against current numbers.
- **Failure routing.** A nightly deep-tier failure is triaged by the author
  of the change that introduced it, or by the release owner when no single
  change is at fault. A fuzz crash or a rejection test that *passes* against
  hostile input it should reject is a **security finding**: stop, minimize
  (`cargo fuzz tmin` for fuzz), and report before merging anything on top.
