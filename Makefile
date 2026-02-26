SHELL := /usr/bin/env bash
NIGHTLY_TOOLCHAIN := nightly

# Quasar example programs that produce SBF binaries
SBF_EXAMPLES := examples/vault examples/escrow

.PHONY: format format-fix clippy clippy-fix check-features \
	build build-sbf test bench-cu all-checks nightly-version

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

test:
	@$(MAKE) build
	@cargo test -p quasar-core -p quasar-derive -p quasar-spl -p quasar-vault -p quasar-escrow --all-features

bench-cu:
	@$(MAKE) build-sbf
	@echo "Running vault CU benchmark..."
	@cargo test -p quasar-vault -- --nocapture 2>&1 | grep -E '(DEPOSIT|WITHDRAW) CU:'

# Run all checks in sequence
all-checks:
	@echo "Running all checks..."
	@$(MAKE) format
	@$(MAKE) clippy
	@$(MAKE) check-features
	@$(MAKE) build-sbf
	@$(MAKE) test
	@echo "All checks passed!"
