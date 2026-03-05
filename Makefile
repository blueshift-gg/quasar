SHELL := /usr/bin/env bash
NIGHTLY_TOOLCHAIN := nightly

# Test programs that produce SBF binaries
SBF_TEST_PROGRAMS := tests/programs/test-misc tests/programs/test-errors \
	tests/programs/test-events tests/programs/test-pda \
	tests/programs/test-token-cpi tests/programs/test-sysvar

# Example programs that produce SBF binaries
SBF_EXAMPLES := examples/vault examples/escrow examples/multisig

# All SBF programs
SBF_ALL := $(SBF_EXAMPLES) $(SBF_TEST_PROGRAMS)

.PHONY: format format-fix clippy clippy-fix check-features \
	build build-sbf test bench-cu test-miri test-all nightly-version

# Print the nightly toolchain version for CI
nightly-version:
	@echo $(NIGHTLY_TOOLCHAIN)

format:
	@cargo +$(NIGHTLY_TOOLCHAIN) fmt --all -- --check

format-fix:
	@cargo +$(NIGHTLY_TOOLCHAIN) fmt --all

clippy:
	@cargo +$(NIGHTLY_TOOLCHAIN) clippy --all --all-features --all-targets -- -D warnings

clippy-fix:
	@cargo +$(NIGHTLY_TOOLCHAIN) clippy --all --all-features --all-targets --fix --allow-dirty --allow-staged -- -D warnings

check-features:
	@cargo hack --feature-powerset --no-dev-deps check

build:
	@cargo build

build-sbf:
	@for dir in $(SBF_EXAMPLES); do \
		echo "Building $$dir"; \
		cargo build-sbf --manifest-path "$$dir/Cargo.toml"; \
	done
	@for dir in $(SBF_TEST_PROGRAMS); do \
		echo "Building $$dir (with debug)"; \
		cargo build-sbf --manifest-path "$$dir/Cargo.toml" --features debug,alloc; \
	done

test:
	@$(MAKE) build
	@$(MAKE) build-sbf
	@cargo test -p quasar-core -p quasar-derive -p quasar-spl -p quasar-pod \
		-p quasar-vault -p quasar-escrow -p quasar-multisig \
		-p quasar-test-suite \
		--all-features

bench-cu:
	@$(MAKE) build-sbf
	@echo "Running vault CU benchmark..."
	@cargo test -p quasar-vault -- --nocapture 2>&1 | grep -E '(DEPOSIT|WITHDRAW) CU:'
	@echo "Running escrow CU benchmark..."
	@cargo test -p quasar-escrow -- --nocapture 2>&1 | grep -E '(MAKE|TAKE|REFUND) CU:'

test-miri:
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +nightly miri test -p quasar-core --test miri

# Run all checks in sequence
test-all:
	@echo "Running all checks..."
	@$(MAKE) format
	@$(MAKE) clippy
	@$(MAKE) build-sbf
	@$(MAKE) test
	@$(MAKE) test-miri
	@echo "All checks passed!"
