# Rust public API baseline

This baseline enforces the [compatibility and versioning
policy](../../VERSIONING.md) for published Rust items.

These snapshots record the v0.1.0 Rust API of the three stable Rust libraries:
`quasar-lang`, `quasar-spl`, and `quasar-test`. The gate uses
`x86_64-unknown-linux-gnu` as the canonical host target and the repository's
pinned nightly. It captures each library's default feature profile.
`quasar-lang` also has an all-features snapshot because its exported item set
changes when all features are enabled.

The snapshots include compiler-derived trait implementations and auto traits
such as `Send` and `Sync`. They omit blanket implementations inherited from
other crates. Function parameter names stay out because renaming a parameter
does not break Rust callers.

The comparison permits new public items. It fails when a baseline line is
missing, which covers removed items and changed signatures, and prints the
package, feature profile, and exact affected lines.

Supporting crates are published implementation machinery, not additional
stable Rust products. Their intentional contracts live with their owners:
derive expansion fixtures, IDL wire and client fixtures, testing macros, and
compiler-runtime behavior. CLI behavior is versioned through binary tests, not
through its internal Rust module graph.

Run `make check-public-api` to compare the working tree with this baseline.
Run `make bless-public-api` only for a reviewed API addition or when capturing
the next published 0.1.z baseline. After publishing a 0.1.z release, add its
baseline directory and update `PUBLIC_API_BASELINE_VERSION` so the next
candidate compares with the latest published API.
