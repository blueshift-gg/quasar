# Releasing Quasar 0.1

The release is the product journey, exercised from immutable archives:

```text
install -> init -> build -> test -> deploy -> verify -> debug
```

Cargo metadata is the only package inventory. Do not add crate lists, archive
counts, or publication tiers to scripts or workflows.

## Prepare the exact head

Start from a clean `0.1.0-release` head. Confirm the workspace version and
derived dependency graph:

```bash
cargo run --locked -p quasar-release-tool -- graph --json
```

The graph command rejects version disagreement, inexact internal pins,
unpublished internal dependencies, and cycles.

Create and inspect every archive through the same graph:

```bash
cargo run --locked -p quasar-release-tool -- package --output target/release-packages
make package-rehearsal
```

The rehearsal image contains the packaged CLI and libraries, but no repository
checkout, credentials, or writable package source. It creates the canonical
starter, builds and tests every packaged crate, executes representative Rust,
Kit, and Web3 client contracts, profiles the starter, deploys to a local
validator, verifies positive and negative cases, and confirms generation and
cleanup are deterministic.

## Required gates

Run these commands on the exact commit that will be tagged:

```bash
cargo fmt --all -- --check
make msrv-check
make check-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
make doc-check
make test
make contracts
make miri
make kani
make fuzz-build
make bench
make package-rehearsal
actionlint
zizmor .github/workflows
```

Also confirm the stable JavaScript dependency floors exist and do not resolve
to prereleases:

```bash
npm view @solana/kit@7.0.0 version
npm view @solana/web3.js@3.0.0 version
```

The final Web3.js command must succeed before creating the tag. Its absence
blocks publication, but it does not prevent review of an otherwise complete
pruning change.

## Publish

Create a signed `v<VERSION>` tag only after the exact-head workflow and package
rehearsal are green. The release workflow validates the tag against the
Cargo-derived version, transfers the exact rehearsed archives to the
credentialed job, and publishes in derived dependency order:

```bash
cargo run -p quasar-release-tool -- publish --version <VERSION>
```

The helper reproduces each archive byte-for-byte before Cargo uploads it, then
requires the crates.io checksum to match the rehearsal manifest. It waits for
each dependency tier to become available before publishing its consumers and
refuses a dirty tree, mismatched tag, missing archive, checksum mismatch, or
release-graph drift.
