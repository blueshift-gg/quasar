# Security policy

> **Quasar has not been audited.** Do not use it in production with real funds.
> There is no bug bounty program at this time.

## Reporting a vulnerability

Report vulnerabilities by
[opening a security bug](https://github.com/blueshift-gg/quasar/issues/new?template=bug.yml).
Quasar currently uses public reporting because it is unaudited and not
recommended for production funds. This policy will move to private disclosure
before a production recommendation or bounty program.

## Scope

This policy covers Quasar-owned source and automation published in, or used to
produce, the 0.1.0 release.

The primary product scope is:

- `quasar-lang`, including zero-copy access, validation, CPI and syscall
  handling, and generated program behavior;
- `quasar-spl`, including SPL parsing, validation, account views, and CPI
  helpers;
- Rust and TypeScript `quasar-test`, including macros, fixtures, and assertions; and
- `quasar-cli`, including configuration and command construction, project and
  client generation, deployment and verification, profiling, and keypair or
  secret-file handling.

Supporting proc-macro, IDL, schema, and testing-derive crates
are in scope where they participate in those products. Stable Rust, Kit 7, and
Web3.js 3 generated clients and their wire behavior are in scope. Preview
Python, Go, C, validation-inspection, assembly-inspection, and profiler-server
code remains security-relevant even though it has no patch-level compatibility
promise.

Repository workflows and package manifests are also in scope. This includes
dependency integrity, workflow permissions, credential exposure, and package
contents. Credentialed publication is operated outside this repository.

Examples and test programs are not themselves published product
surfaces. A fixture demonstrating a vulnerability in a shipped product or
release process remains in scope.

Vulnerabilities solely in an upstream dependency or service should be reported
upstream. A reachable Quasar use, unsafe integration, or dependency choice that
exposes users remains in scope here.

## Unsafe code and assurance

Quasar uses `unsafe` for zero-copy account access, pointer walking, CPI
syscalls, and the SBF compiler runtime shim. Required checks focus Miri on
provenance, aliasing, initialization, exact boundaries, duplicate account
regions, macro-generated decoders, and adversarial extension points.

Kani verifies bounded properties in `quasar-lang` and `quasar-spl`. Fuzzing
searches arbitrary parsing and account-region inputs. Host and real SBF tests
cover semantic and on-chain behavior. These checks are evidence for their
encoded paths and assumptions; they are not a complete soundness proof or a
substitute for an audit.

An unsafe operation without an adequate safety argument, a violation of its
documented contract, or user-controlled undefined behavior qualifies as a
security vulnerability.
