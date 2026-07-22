SHELL := /usr/bin/env bash
# Keep rustfmt, Clippy, and Miri deterministic across local and CI runs.
NIGHTLY_TOOLCHAIN := nightly-2026-06-24
KANI_VERSION := 0.67.0
CARGO_FUZZ_VERSION := 0.13.2
CARGO_AUDIT_VERSION := 0.22.1
LICENSE_EXPRESSION := Apache-2.0 OR MIT
PROGRAM_MSRV := 1.89.0
# platform-tools v1.52 ships Cargo 1.89 which supports Cargo.lock v4.
# v1.51 ships Cargo 1.84 which does not, causing "duplicate lang item" errors.
PLATFORM_TOOLS := v1.52

# Native test runners that consume freshly built SBF artifacts. Their
# development dependency on quasar-test or quasar-svm is the owning manifest's
# declaration of that requirement; the Makefile keeps no parallel inventory.
SBF_TEST_RUNNERS := $(shell cargo metadata --locked --no-deps --format-version 1 2>/dev/null | \
	jq -r '.packages[] | select(any(.dependencies[]?; .kind == "dev" and (.name == "quasar-test" or .name == "quasar-svm"))) | .name')
# Cargo owns the SBF program inventory. Each program manifest owns its default
# build features, so adding a cdylib target needs no Makefile update.
SBF_PROGRAM_PACKAGES := $(shell cargo metadata --locked --no-deps --format-version 1 2>/dev/null | \
	jq -r '.packages[] | select(any(.targets[]?; (.crate_types // []) | index("cdylib"))) | .name')
HOST_TEST_EXCLUDES := $(sort $(SBF_TEST_RUNNERS) $(SBF_PROGRAM_PACKAGES))

.PHONY: format format-fix clippy clippy-fix check-features \
	check-workspace-invariants check-license-policy \
	build build-sbf test test-bless \
	test-host test-sbf-host test-quasar-test-standalone \
	doc-check \
	miri test-miri test-miri-strict test-all \
	nightly-version cargo-fuzz-version cargo-audit-version \
	fuzz-build test-fuzz-build contracts \
	check-proc-macro-baselines check-test-clients bless-proc-macro-baselines \
	kani help-kani check-kani kani-lang kani-spl msrv-check \
	package-check audit

# Print the nightly toolchain version for CI
nightly-version:
	@echo $(NIGHTLY_TOOLCHAIN)

cargo-fuzz-version:
	@echo $(CARGO_FUZZ_VERSION)

cargo-audit-version:
	@echo $(CARGO_AUDIT_VERSION)

check-proc-macro-baselines:
	@cargo test -p quasar-derive --all-features snapshot_tests:: -- --test-threads=1

bless-proc-macro-baselines:
	@UPDATE_EXPECT=1 cargo test -p quasar-derive --all-features snapshot_tests:: -- --test-threads=1

fuzz-build: test-fuzz-build
test-fuzz-build:
	@cd lang && cargo +$(NIGHTLY_TOOLCHAIN) fuzz build

check-test-clients:
	@scripts/check-test-clients.sh

contracts: check-proc-macro-baselines check-test-clients
	@cargo test -p quasar-idl --all-features
	@idl/tests/client-conformance/run.sh

doc-check:
	@RUSTDOCFLAGS="-D warnings" cargo +$(PROGRAM_MSRV) doc \
		--workspace --all-features --no-deps --locked

msrv-check:
	@cargo +$(PROGRAM_MSRV) check --workspace --all-features --locked

help-kani:
	@echo "Local Kani verification is optional."
	@echo "CI installs and runs Kani automatically."
	@echo ""
	@echo "Expected local version: kani $(KANI_VERSION)"
	@echo "Check version:         kani --version"
	@echo "Run all proofs:        make kani"
	@echo "Run one crate:         make kani-lang | make kani-spl"

check-kani:
	@command -v kani >/dev/null 2>&1 || { \
		echo "kani is not installed."; \
		echo "Normal builds/tests do not require Kani."; \
		echo "To run proof harnesses locally, install kani $(KANI_VERSION) and re-run."; \
		echo "Then verify with: kani --version"; \
		exit 1; \
	}
	@version="$$(kani --version 2>/dev/null | awk '{print $$2}')"; \
	if [[ "$$version" != "$(KANI_VERSION)" ]]; then \
		echo "unexpected kani version: $$version"; \
		echo "expected: $(KANI_VERSION)"; \
		echo "CI uses Kani $(KANI_VERSION); local verification should match."; \
		exit 1; \
	fi

format:
	@cargo fmt --all -- --check

format-fix:
	@cargo fmt --all

clippy:
	@cargo clippy --workspace --all-features --all-targets -- -D warnings

clippy-fix:
	@cargo clippy --workspace --all-features --all-targets --fix --allow-dirty --allow-staged -- -D warnings

check-features:
	@cargo hack --feature-powerset --no-dev-deps check

check-workspace-invariants: check-license-policy

check-license-policy:
	@expected='$(LICENSE_EXPRESSION)'; \
	metadata="$$(cargo metadata --locked --no-deps --format-version 1)"; \
	unexpected="$$(jq -r --arg expected "$$expected" \
	  '.packages[] | select(.publish != []) | select(.license != $$expected) | [.name, (.license // "missing")] | @tsv' \
	  <<<"$$metadata")"; \
	if [[ -n "$$unexpected" ]]; then \
	  echo "publishable crates must use SPDX license expression $$expected:" >&2; \
	  echo "$$unexpected" >&2; \
	  exit 1; \
	fi; \
	for file in LICENSE-APACHE LICENSE-MIT; do \
	  if [[ ! -s "$$file" ]]; then \
	    echo "missing license grant: $$file" >&2; \
	    exit 1; \
	  fi; \
	done; \
	if ! grep -Fq '[Apache License, Version 2.0](LICENSE-APACHE)' README.md \
	  || ! grep -Fq '[MIT License](LICENSE-MIT)' README.md; then \
	  echo "README license grant does not match $$expected" >&2; \
	  exit 1; \
	fi

build:
	@cargo build

build-sbf:
	@while IFS= read -r manifest; do \
		echo "Building $$manifest"; \
		cargo build-sbf --tools-version $(PLATFORM_TOOLS) --manifest-path "$$manifest" || exit; \
	done < <(cargo metadata --locked --no-deps --format-version 1 \
		| jq -r '.packages[] | select(any(.targets[]?; (.crate_types // []) | index("cdylib"))) | .manifest_path')

# Cargo owns test-target discovery: adding a legitimate target needs no
# secondary inventory update.
test-host:
	@CARGO_INCREMENTAL=0 cargo test --workspace \
		$(foreach package,$(HOST_TEST_EXCLUDES),--exclude $(package)) \
		--all-features

test-sbf-host:
	@CARGO_INCREMENTAL=0 cargo test \
		$(foreach package,$(SBF_TEST_RUNNERS),-p $(package)) \
		--all-features

test-quasar-test-standalone:
	@scripts/test-quasar-test-standalone.sh target/deploy/quasar_vault.so

# Asserts committed trybuild .stderr goldens (trybuild default mode). A stale
# golden fails the build — that is the gate. Regenerate with `make test-bless`.
test:
	@$(MAKE) build
	@$(MAKE) build-sbf
	@$(MAKE) test-host
	@$(MAKE) test-quasar-test-standalone
	@$(MAKE) test-sbf-host

# Regenerates trybuild .stderr goldens (TRYBUILD=overwrite). Use only when a
# diagnostic change is intended; review the regenerated diffs like code before
# committing. `make test` (and CI) run in assert mode and never set TRYBUILD.
test-bless:
	@$(MAKE) build
	@$(MAKE) build-sbf
	@CARGO_INCREMENTAL=0 TRYBUILD=overwrite cargo test --workspace --all-features

# CI already compiles the workspace, docs, and consumer contracts. Keep this
# gate source-only; registry-sequenced verification belongs to the external
# publishing environment.
package-check: check-license-policy
	@cargo publish --workspace --dry-run --locked --no-verify

audit:
	@command -v cargo-audit >/dev/null 2>&1 || { \
		echo "cargo-audit is not installed; run: cargo install cargo-audit --version $(CARGO_AUDIT_VERSION) --locked"; \
		exit 1; \
	}
	@version="$$(cargo audit --version | awk '{print $$2}')"; \
	if [[ "$$version" != "$(CARGO_AUDIT_VERSION)" ]]; then \
		echo "unexpected cargo-audit version: $$version"; \
		echo "expected: $(CARGO_AUDIT_VERSION)"; \
		exit 1; \
	fi
	@cargo audit

miri: test-miri test-miri-strict

# The complete adversarial suites run under Tree Borrows. No test is removed
# from Miri merely because a nearby case exercises the same broad category:
# pruning requires a per-test unsafe-path and oracle equivalence argument.
# Generated downstream extension points additionally run under both supported
# borrow models.
test-miri:
	@MIRIFLAGS="-Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-spl --test miri

# Strict provenance covers the same complete unsafe surface.
test-miri-strict:
	@MIRIFLAGS="-Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-spl --test miri

kani-lang: check-kani
	@cargo kani -p quasar-lang

kani-spl: check-kani
	@cargo kani -p quasar-spl

kani: kani-lang kani-spl

# Run all checks in sequence
test-all:
	@echo "Running all checks..."
	@$(MAKE) format
	@$(MAKE) clippy
	@$(MAKE) check-workspace-invariants
	@$(MAKE) test
	@$(MAKE) contracts
	@$(MAKE) package-check
	@$(MAKE) audit
	@$(MAKE) doc-check
	@$(MAKE) fuzz-build
	@$(MAKE) miri
	@echo "All checks passed!"
