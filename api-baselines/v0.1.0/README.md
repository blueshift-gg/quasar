# Rust public API baseline

These snapshots record the v0.1.0 Rust API for every published library target.
The gate uses `x86_64-unknown-linux-gnu` as the canonical host target and the
repository's pinned nightly. It captures each ordinary library's default
feature profile. `quasar-lang` also has an all-features snapshot because it is
the only package whose exported item set changes when all features are enabled.
For the proc-macro-only `quasar-derive`, it records every exported macro's name
and kind plus the helper attributes accepted by derive macros.

The snapshots include compiler-derived trait implementations and auto traits
such as `Send` and `Sync`. They omit blanket implementations inherited from
other crates. Function parameter names stay out because renaming a parameter
does not break Rust callers.

The comparison permits new public items. It fails when a baseline line is
missing, which covers removed items and changed signatures, and prints the
package, feature profile, and exact affected lines.

## Surface audit

- All ten publishable packages contain a library target and have a primary
  snapshot, including `quasar-cli`, the proc-macro-only `quasar-derive`, and the
  host-empty `solana-compiler-builtins` library.
- `quasar-cli` publishes its library to share the binary and integration-test
  implementation. The baseline treats its visible library items as supported
  v0.1 API.
- `quasar-idl` exposes its code generators, validation model, lint API, and IDL
  schema re-exports. The baseline treats those modules as supported v0.1 API.
- `quasar-lang` marks the proc-macro protocol under `__internal` and its private
  dependency re-exports with `#[doc(hidden)]`; rustdoc excludes them from the
  public snapshot. The release train pins that protocol through exact internal
  crate versions.
- `solana-compiler-builtins::memcmp` exists only for the BPF target and exports
  a C ABI symbol. Its C contract belongs to runtime verification rather than
  this host Rust API snapshot.

Run `make check-public-api` to compare the working tree with this baseline.
Run `make bless-public-api` only for a reviewed API addition or when capturing
the next published 0.1.z baseline. After publishing a 0.1.z release, add its
baseline directory and update `PUBLIC_API_BASELINE_VERSION` so the next
candidate compares with the latest published API.
