# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Quasar, **please report it privately** instead of opening a public issue.

**Email:** [leo@blueshift.gg](mailto:leo@blueshift.gg)

Include:
- A description of the vulnerability
- Steps to reproduce (if applicable)
- The affected crate(s) and version(s)
- Any suggested fix or mitigation

We will acknowledge receipt within 48 hours and aim to release a patch within 7 days for critical issues.

## Scope

This policy covers the Quasar framework crates:

- `quasar` (facade)
- `quasar-core` (framework primitives)
- `quasar-derive` (proc macros)
- `quasar-pod` (Pod integer types)
- `quasar-spl` (SPL Token integration)
- `quasar-idl` (IDL generator)

## Unsafe Code

Quasar uses `unsafe` extensively for zero-copy access, CPI syscalls, and pointer casts. Every `unsafe` block has a documented soundness invariant and is validated by Miri under Tree Borrows with symbolic alignment checking.

If you find an `unsafe` block that lacks a soundness argument or can be triggered to produce undefined behavior, that qualifies as a security vulnerability.
