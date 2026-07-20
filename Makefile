SHELL := /usr/bin/env bash
# Keep rustfmt, Clippy, and Miri deterministic across local and CI runs.
NIGHTLY_TOOLCHAIN := nightly-2026-06-24
KANI_VERSION := 0.67.0
CARGO_FUZZ_VERSION := 0.13.2
CARGO_AUDIT_VERSION := 0.22.1
CARGO_PUBLIC_API_VERSION := 0.52.0
LICENSE_EXPRESSION := Apache-2.0 OR MIT
PUBLIC_API_BASELINE_VERSION := v0.1.0
PUBLIC_API_BASELINE_DIR := api-baselines/$(PUBLIC_API_BASELINE_VERSION)
PUBLIC_API_TARGET := x86_64-unknown-linux-gnu
STABLE_API_PACKAGES := quasar-lang quasar-spl quasar-test
PROGRAM_MSRV := 1.89.0
# platform-tools v1.52 ships Cargo 1.89 which supports Cargo.lock v4.
# v1.51 ships Cargo 1.84 which does not, causing "duplicate lang item" errors.
PLATFORM_TOOLS := v1.52

# Test programs that produce SBF binaries
SBF_TEST_PROGRAMS := tests/programs/test-misc tests/programs/test-errors \
	tests/programs/test-events tests/programs/test-pda \
	tests/programs/test-token-cpi tests/programs/test-token-init \
	tests/programs/test-token-validate tests/programs/test-sysvar \
	tests/programs/test-one-of tests/programs/test-migrate \
	tests/programs/test-raw

# Example programs that produce SBF binaries
SBF_EXAMPLES := examples/vault examples/escrow examples/multisig

# All SBF programs
SBF_ALL := $(SBF_EXAMPLES) $(SBF_TEST_PROGRAMS)

# Host-side tests that consume freshly built SBF artifacts.
SBF_HOST_TEST_PACKAGES := quasar-vault quasar-escrow quasar-multisig quasar-test-suite
# Program packages are discovered from Cargo's cdylib targets so a new SBF
# target does not need to be copied into a second host-test inventory.
SBF_PROGRAM_PACKAGES := $(shell cargo metadata --locked --no-deps --format-version 1 2>/dev/null | \
	jq -r '.packages[] | select(any(.targets[]?; (.crate_types // []) | index("cdylib"))) | .name')
HOST_TEST_EXCLUDES := $(sort $(SBF_HOST_TEST_PACKAGES) $(SBF_PROGRAM_PACKAGES))

PACKAGE_REHEARSAL_ROOT ?= target/release-rehearsal

.PHONY: format format-fix clippy clippy-fix check-features \
	check-workspace-invariants check-license-policy \
	check-package-metadata check-release-train \
	build build-sbf test test-bless \
	test-host test-sbf-host test-quasar-test-standalone \
	bench-cu bench-tracked compare-tracked test-benchmark-policy doc-check \
	miri test-miri test-miri-strict test-all \
	nightly-version cargo-fuzz-version cargo-audit-version cargo-public-api-version \
	fuzz-build test-fuzz-build check-public-api bless-public-api contracts \
	check-proc-macro-baselines bless-proc-macro-baselines \
	coverage kani help-kani check-kani kani-lang kani-spl msrv-check \
	bench package-check package-rehearsal audit

# Print the nightly toolchain version for CI
nightly-version:
	@echo $(NIGHTLY_TOOLCHAIN)

cargo-fuzz-version:
	@echo $(CARGO_FUZZ_VERSION)

cargo-audit-version:
	@echo $(CARGO_AUDIT_VERSION)

cargo-public-api-version:
	@echo $(CARGO_PUBLIC_API_VERSION)

check-public-api:
	@scripts/check-public-api.sh check "$(PUBLIC_API_BASELINE_DIR)" \
		"$(NIGHTLY_TOOLCHAIN)" "$(CARGO_PUBLIC_API_VERSION)" \
		"$(PUBLIC_API_TARGET)" $(STABLE_API_PACKAGES)

bless-public-api:
	@scripts/check-public-api.sh bless "$(PUBLIC_API_BASELINE_DIR)" \
		"$(NIGHTLY_TOOLCHAIN)" "$(CARGO_PUBLIC_API_VERSION)" \
		"$(PUBLIC_API_TARGET)" $(STABLE_API_PACKAGES)

check-proc-macro-baselines:
	@cargo test -p quasar-derive --all-features snapshot_tests:: -- --test-threads=1

bless-proc-macro-baselines:
	@UPDATE_EXPECT=1 cargo test -p quasar-derive --all-features snapshot_tests:: -- --test-threads=1

fuzz-build: test-fuzz-build
test-fuzz-build:
	@cd lang && cargo +$(NIGHTLY_TOOLCHAIN) fuzz build

contracts: check-public-api check-proc-macro-baselines
	@cargo test -p quasar-idl --all-features

doc-check:
	@RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked

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

check-workspace-invariants: check-license-policy check-package-metadata check-release-train

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

check-package-metadata:
	@metadata="$$(cargo metadata --locked --no-deps --format-version 1)"; \
	allowed_categories='["api-bindings","command-line-utilities","data-structures","development-tools","development-tools::procedural-macro-helpers","development-tools::profiling","development-tools::testing","embedded","no-std","rust-patterns"]'; \
	errors="$$(jq -r \
	  --arg homepage 'https://quasar-lang.com' \
	  --arg repository 'https://github.com/blueshift-gg/quasar' \
	  --arg docs 'https://docs.rs' \
	  --argjson allowed "$$allowed_categories" \
	  --from-file scripts/check-package-metadata.jq <<<"$$metadata")"; \
	if [[ -n "$$errors" ]]; then \
	  echo "incomplete crates.io metadata:" >&2; \
	  echo "$$errors" >&2; \
	  exit 1; \
	fi; \
	while IFS= read -r readme; do \
	  if [[ ! -s "$$readme" ]]; then \
	    echo "missing package README: $$readme" >&2; \
	    exit 1; \
	  fi; \
	done < <(jq -r \
	  '.packages[] | select(.publish != []) | (.manifest_path | sub("/Cargo.toml$$"; "")) + "/" + .readme' \
	  <<<"$$metadata")

check-release-train:
	@cargo run --locked -p quasar-release-tool -- graph --json >/dev/null

build:
	@cargo build

build-sbf:
	@for dir in $(SBF_EXAMPLES); do \
		echo "Building $$dir"; \
		cargo build-sbf --tools-version $(PLATFORM_TOOLS) --manifest-path "$$dir/Cargo.toml"; \
	done
	@for dir in $(SBF_TEST_PROGRAMS); do \
		echo "Building $$dir (with debug)"; \
		cargo build-sbf --tools-version $(PLATFORM_TOOLS) --manifest-path "$$dir/Cargo.toml" --features debug,alloc; \
	done
	@echo "Building test-heap (alloc only, no debug — tests alloc trap)"
	cargo build-sbf --tools-version $(PLATFORM_TOOLS) --manifest-path tests/programs/test-heap/Cargo.toml --features alloc

# Cargo owns test-target discovery: adding a legitimate target needs no
# secondary inventory update.
test-host:
	@CARGO_INCREMENTAL=0 cargo test --workspace \
		$(foreach package,$(HOST_TEST_EXCLUDES),--exclude $(package)) \
		--all-features

test-sbf-host:
	@CARGO_INCREMENTAL=0 cargo test \
		$(foreach package,$(SBF_HOST_TEST_PACKAGES),-p $(package)) \
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

package-check: check-package-metadata
	@rm -rf target/release-packages
	@cargo run --locked -p quasar-release-tool -- package \
		--output target/release-packages

package-rehearsal: package-check
	@rm -rf "$(PACKAGE_REHEARSAL_ROOT)"
	@scripts/prepare-package-rehearsal.sh \
		target/release-packages "$(PACKAGE_REHEARSAL_ROOT)"

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

bench-cu:
	@$(MAKE) build-sbf
	@rm -f target/cu-bench/quasar-vault.jsonl target/cu-bench/quasar-escrow.jsonl
	@echo "Running vault CU benchmark..."
	@cargo test -p quasar-vault -- --test-threads=1
	@jq -r '"  \(.instruction) CU: \(.cu)"' target/cu-bench/quasar-vault.jsonl
	@echo "Running escrow CU benchmark..."
	@cargo test -p quasar-escrow -- --test-threads=1
	@jq -r '"  \(.instruction) CU: \(.cu)"' target/cu-bench/quasar-escrow.jsonl

bench-tracked:
	@bash scripts/bench-tracked-programs.sh capture target/tracked-metrics.env
	@cat target/tracked-metrics.env

test-benchmark-policy:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_bench_tracked_programs.py

compare-tracked:
	@bash scripts/bench-tracked-programs.sh compare

miri: test-miri

# Ordinary semantic cases in lang/tests/miri.rs and spl/tests/miri.rs run in
# the fast host suite. Miri selects only tests with a unique unsafe failure
# story: generated parsing, provenance, aliasing, initialization, and exact
# pointer boundaries. Extension points run under both borrow models.
test-miri:
	@MIRIFLAGS="-Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@for test in \
		aliasing_write_then_read_original_view \
		aliasing_duplicate_accounts_4_deref_mut_to_same_data \
		bounds_account_view_exact_size_sweep \
		bounds_remaining_boundary_pointer_subtraction \
		uninit_maybeuninit_account_view_array \
		dynamic_memmove_1byte_grow_1byte_tail \
		instruction_zc_cast_exact_length \
		tail_str_exact_boundary \
		cpi_aliasing_interleaved_write_cpi_cycles; do \
		MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
			cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri "$$test" -- --exact; \
	done
	@for test in \
		token_deref_mut_aliasing_stress \
		mint_exact_size_buffer \
		interface_account_aliasing \
		zero_copy_deref_from_exact_boundary \
		maybeunit_init_then_read_every_byte_initialize_mint; do \
		MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
			cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-spl --test miri "$$test" -- --exact; \
	done

# Strict provenance is reserved for the small downstream extension-point suite.
test-miri-strict:
	@MIRIFLAGS="-Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions

# Host-side line coverage is informational only; SBF-executed code is
# invisible here by design.
coverage:
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { \
		echo "cargo-llvm-cov is not installed; run: cargo install cargo-llvm-cov --locked"; \
		exit 1; \
	}
	@cargo llvm-cov --workspace \
		$(foreach package,$(SBF_HOST_TEST_PACKAGES),--exclude $(package)) \
		--all-features --html
	@echo "HTML report: target/llvm-cov/html/index.html"

kani-lang: check-kani
	@cargo kani -p quasar-lang

kani-spl: check-kani
	@cargo kani -p quasar-spl

kani: kani-lang kani-spl

bench: test-benchmark-policy bench-tracked compare-tracked

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
	@$(MAKE) test-benchmark-policy
	@$(MAKE) doc-check
	@$(MAKE) fuzz-build
	@$(MAKE) miri
	@echo "All checks passed!"
