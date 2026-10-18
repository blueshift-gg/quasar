# Security Policy

> **Quasar has not been audited.** Do not use it in production with real funds until an audit is complete. There is no bug bounty program at this time.

## Reporting a Vulnerability

Since Quasar is unaudited and should not be used with real funds, **report vulnerabilities publicly** by [opening a bug report](https://github.com/blueshift-gg/quasar/issues/new?template=bug.yml). Public disclosure helps everyone and gets bugs fixed faster.

Once Quasar is audited and in production use, we'll switch to private disclosure with a bug bounty program.

## Scope

This policy covers:

- `quasar-lang` — framework primitives, zero-copy access, CPI builder
- `quasar-derive` — proc macros
- `quasar-spl` — SPL Token integration

## Unsafe Code

Quasar uses `unsafe` for zero-copy access, CPI syscalls, and pointer casts. Every `unsafe` block has a documented soundness invariant. The rewrite describes its invariants in [ARCHITECTURE.md](ARCHITECTURE.md), and `make check` runs its Kani proof harnesses.

If you find an `unsafe` block that lacks a soundness argument or can be triggered to produce undefined behavior, that qualifies as a security vulnerability.
