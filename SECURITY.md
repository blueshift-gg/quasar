# Security Policy

> **Quasar has not been audited.** Do not use it in production with real funds until an audit is complete. There is no bug bounty program at this time.

## Reporting a Vulnerability

Since Quasar is unaudited and should not be used with real funds, **report vulnerabilities publicly** by [opening a bug report](https://github.com/blueshift-gg/quasar/issues/new?template=bug.yml). Public disclosure helps everyone and gets bugs fixed faster.

Once Quasar is audited and in production use, we'll switch to private disclosure with a bug bounty program.

## Scope

This policy covers vulnerabilities in Quasar-owned source and automation that
is published in, or used to produce, the v0.1.0 release.

### Published packages

- `quasar-lang`, `quasar-derive`, and `solana-compiler-builtins`: program
  runtime primitives, zero-copy access, validation code generation, CPI and
  syscall handling, and compiler runtime behavior.
- `quasar-spl` and `quasar-metadata`: parsing, validation, zero-copy account
  views, and CPI integrations for SPL Token and Metaplex Token Metadata.
- `quasar-schema`, `quasar-idl-schema`, and `quasar-idl`: schema and IDL
  parsing, serialization, hashing, validation, and generated interface data.
- `quasar-cli`: project and client generation, configuration parsing, command
  construction, deploy inputs, and program keypair or other secret-file
  reading, generation, replacement, and permissions.
- `quasar-profile`: SBF parsing, profile data generation, snapshot handling,
  and the local profiler server.

### Release supply chain

The policy also covers repository-owned workflows, Dockerfiles, scripts, and
configuration that verify, package, publish, or create Quasar releases. This
includes dependency integrity, workflow permissions, release credential
exposure, protected publishing boundaries, and mismatches between a tag and
its published crates or GitHub release artifacts.

### Outside this policy

- Vulnerabilities that exist solely in an upstream dependency, the Solana or
  Agave toolchain, GitHub, or crates.io should be reported to that project or
  service. A Quasar integration, dependency pin, or reachable use that exposes
  users to the vulnerability remains in scope here.
- Other repositories, including `blueshift-gg/quasar-docs` and
  `blueshift-gg/quasar-svm`, use their own reporting and support boundaries.
- Examples, test programs, benchmarks, and test-only clients in this repository
  are not published v0.1.0 packages. A defect confined to those fixtures is out
  of scope; a fixture that demonstrates a vulnerability in a published package
  or release process is in scope.

## Unsafe Code and Verification

Quasar uses `unsafe` for zero-copy access, CPI syscalls, and pointer casts. The
required CI job runs `make test-miri-strict`, which executes the dedicated Miri
integration suites for `quasar-lang`, `quasar-spl`, and `quasar-metadata` with:

- `-Zmiri-tree-borrows`
- `-Zmiri-symbolic-alignment-check`
- `-Zmiri-strict-provenance`

Those suites exercise the unsafe paths represented by their tests. A passing
run is evidence for those paths, not a proof that every unsafe block or every
published package is sound.

The tag-triggered release workflow separately runs the Kani 0.67.0 proof
harnesses in `quasar-lang`, `quasar-spl`, and `quasar-metadata`. Kani checks the
properties encoded by those individual harnesses under their stated bounds and
assumptions; it does not prove an entire crate or the complete Quasar system.

Miri cannot execute the generated SBF `extern "C"` program entrypoint or other
SBF-only syscall and FFI paths. Host and on-chain integration tests cover
additional behavior, but they are not substitutes for an audit or a complete
undefined behavior proof.

An unsafe operation that lacks an adequate safety argument, violates its stated
contract, or can be triggered to produce undefined behavior qualifies as a
security vulnerability.
