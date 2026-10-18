# CI runs `make check` and the two Miri targets in separate jobs.

KANI_VERSION := 0.67.0
MIRI_TOOLCHAIN := nightly-2026-03-27
MIRI_FLAGS := -Zmiri-tree-borrows -Zmiri-symbolic-alignment-check

# Compute units and sizes depend on the compiler, so every SBF build uses one.
SBF_TOOLS := --tools-version v1.56
export SBF_TOOLS

PROGRAMS := $(patsubst %/Cargo.toml,%,$(wildcard tests/*/Cargo.toml examples/*/Cargo.toml))
# Built a second time with `quasar-lang/debug`: making every check
# themselves and saying which fails, to compare with the build that leaves
# checks to the calls that make them.
DEBUG := $(PROGRAMS)

.PHONY: check fmt clippy check-features sbf rivals test kani bench bench-check idl expansion profile publish-check
.PHONY: test-miri test-miri-strict miri-version
.PHONY: format format-fix clippy-fix build build-sbf generated-client-smoke
.PHONY: bench-cu bench-tracked compare-tracked test-all nightly-version
.PHONY: help-kani check-kani kani-lang kani-metadata

check: fmt clippy check-features sbf rivals test kani bench-check expansion publish-check

fmt:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

check-features:
	cargo hack --workspace --feature-powerset --no-dev-deps check

sbf:
	@for program in $(PROGRAMS); do \
		RUSTFLAGS="-D warnings" cargo build-sbf $(SBF_TOOLS) --manifest-path $$program/Cargo.toml || exit 1; \
	done
	@for program in $(DEBUG); do \
		CARGO_TARGET_DIR=$(CURDIR)/target/debug-checks RUSTFLAGS="-D warnings" \
			cargo build-sbf $(SBF_TOOLS) --features quasar-lang/debug \
			--manifest-path $$program/Cargo.toml || exit 1; \
	done

# Programs built from pinned revisions, for the oracle tests and the benchmark.
rivals:
	bench/rivals.sh

test: sbf rivals
	RUST_LOG=error cargo test --workspace

kani: kani-lang kani-metadata

kani-lang: check-kani
	cargo kani -p quasar-lang --output-format terse

kani-metadata: check-kani
	cargo kani -p quasar-metadata --output-format terse

check-kani:
	@version="$$(cargo kani --version 2>/dev/null)"; \
	if [ "$$version" != "cargo-kani $(KANI_VERSION)" ]; then \
		echo "Expected cargo-kani $(KANI_VERSION); install with cargo install --locked kani-verifier@$(KANI_VERSION) && cargo kani setup" >&2; \
		exit 1; \
	fi

help-kani:
	@echo "Install: cargo install --locked kani-verifier@$(KANI_VERSION) && cargo kani setup"
	@echo "Verify: make check-kani; run proofs: make kani, kani-lang, or kani-metadata"

miri-version:
	@echo $(MIRI_TOOLCHAIN)

test-miri:
	MIRIFLAGS="$(MIRI_FLAGS)" cargo +$(MIRI_TOOLCHAIN) miri test --locked -p quasar-lang --lib miri::
	MIRIFLAGS="$(MIRI_FLAGS)" cargo +$(MIRI_TOOLCHAIN) miri test --locked -p quasar-spl --test miri
	MIRIFLAGS="$(MIRI_FLAGS)" cargo +$(MIRI_TOOLCHAIN) miri test --locked -p quasar-metadata --lib --test accounts --test wire

test-miri-strict:
	$(MAKE) test-miri MIRI_FLAGS="$(MIRI_FLAGS) -Zmiri-strict-provenance"

bench: sbf rivals
	RUST_LOG=error cargo run -q -p bench > bench/RESULTS.md

# Where the escrow's own compute units go, by function: built with debug
# information, which leaves its code as it is, and run with every
# instruction recorded.
profile: rivals
	CARGO_PROFILE_RELEASE_DEBUG=2 CARGO_TARGET_DIR=$(CURDIR)/target/profile \
		cargo build-sbf $(SBF_TOOLS) --manifest-path examples/escrow/Cargo.toml
	RUST_LOG=error cargo run -q -p bench --features profile --bin profile

# Fails when compute units or binary sizes differ from bench/RESULTS.md.
bench-check: sbf rivals
	RUST_LOG=error cargo run -q -p bench | diff -u bench/RESULTS.md -

# Rewrites each example's committed `idl.json` from the program.
idl:
	QUASAR_BLESS=1 cargo test -q --workspace _the_committed_one

# What the macros generate for `examples/lamport-vault`, committed as a
# reading aid: any change to generated code shows up in review.
expansion:
	cargo expand -p lamport-vault --lib > docs/lamport-vault.expanded.rs

# Verify the registry packages together without uploading or requiring a clean tree.
publish-check:
	python3 .github/scripts/test-publish-crates.py
	cargo publish --dry-run --locked --allow-dirty -p quasar-derive -p quasar-lang -p quasar-spl -p quasar-metadata -p quasar-test -p quasar-schema -p quasar-idl-schema -p quasar-idl -p quasar-profile -p quasar-cli

# Familiar developer commands, using the rewrite's toolchains and gates.
format: fmt

format-fix:
	cargo fmt --all

clippy-fix:
	cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged -- -D warnings

build:
	cargo build --workspace

build-sbf: sbf

nightly-version: miri-version

generated-client-smoke:
	cargo test -p quasar-cli --test generated_clients_smoke -- --nocapture

# Print the current report; `make bench` is the explicit baseline update.
bench-cu bench-tracked: sbf rivals
	RUST_LOG=error cargo run -q -p bench

compare-tracked: bench-check

test-all:
	$(MAKE) -j1 check test-miri test-miri-strict
