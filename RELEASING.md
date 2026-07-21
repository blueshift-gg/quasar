# Releasing Quasar 0.1

Quasar keeps product verification in this repository and credentialed
publication outside it. Cargo owns package selection, archive verification,
dependency ordering, and registry availability; do not reproduce those rules
in a Quasar release tool or workflow.

## Verify the exact head

Run the same gates required by CI on the commit that will be tagged:

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
make package-check
actionlint
zizmor .github/workflows
```

`make package-check` delegates to Cargo's real publication path in dry-run
mode. It packages and verifies every publishable workspace member while
skipping examples and other `publish = false` members.

Confirm the stable JavaScript dependency floors exist and do not resolve to
prereleases:

```bash
npm view @solana/kit@7.0.0 version
npm view @solana/web3.js@3.0.0 version
```

Final Web3.js 3 is required for the tag, but its absence does not block review
of the pruning change.

## Publish outside the repository

After exact-head CI is green, create the signed `v<VERSION>` tag and let its CI
run finish. From the separately controlled release environment, verify that the
tag and workspace version agree, then publish with Cargo:

```bash
cargo publish --workspace --locked
```

The external release environment owns crates.io credentials and GitHub release
creation. Quasar contains no publisher, registry polling code, Docker rehearsal,
archive handoff, or release secret.
