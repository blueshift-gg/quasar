PLATFORM_TOOLS_VERSION ?= v1.56
TARGET_DIR := $(if $(CARGO_TARGET_DIR),$(abspath $(CARGO_TARGET_DIR)),$(CURDIR)/target)
export CARGO_TARGET_DIR := $(TARGET_DIR)
export SBF_OUT_DIR := $(TARGET_DIR)/deploy
export RUST_LOG := error

.PHONY: build test check fmt clippy

# Agave 3.1 in quasar-test uses the v0 ELF format. Recent build-sbf defaults to v3.
build:
	cargo build-sbf --tools-version $(PLATFORM_TOOLS_VERSION) --arch v0 --manifest-path Cargo.toml --sbf-out-dir $(SBF_OUT_DIR) -- --locked
	cargo build-sbf --tools-version $(PLATFORM_TOOLS_VERSION) --arch v0 --manifest-path fixtures/hook/Cargo.toml --sbf-out-dir $(SBF_OUT_DIR) -- --locked

test: build
	cargo test --locked

fmt:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets --locked -- -D warnings

check: fmt clippy test
