SHELL := /usr/bin/env bash
# Keep rustfmt, Clippy, and Miri deterministic across local and CI runs.
NIGHTLY_TOOLCHAIN := nightly-2026-03-27
KANI_VERSION := 0.67.0
CARGO_FUZZ_VERSION := 0.13.2
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
	tests/programs/test-raw tests/programs/test-metadata-validate

# Example programs that produce SBF binaries
SBF_EXAMPLES := examples/vault examples/escrow examples/multisig examples/upstream-vault

# All SBF programs
SBF_ALL := $(SBF_EXAMPLES) $(SBF_TEST_PROGRAMS)

# Public crates in dependency order. Keep this list aligned with the release
# workflow; `package-check` proves the complete publication graph packages.
PUBLISH_PACKAGES := quasar-schema quasar-idl-schema quasar-profile \
	solana-compiler-builtins quasar-derive quasar-idl quasar-lang \
	quasar-spl quasar-metadata quasar-cli

# Publishable crates whose ordinary host tests can run without the generated
# client toolchains. The CLI smoke target is delegated below because it needs
# pinned Node, Python, Go, Clang, and Caravel dependencies.
HOST_TEST_PACKAGES := $(filter-out quasar-cli,$(PUBLISH_PACKAGES))

# Host-side tests that consume freshly built SBF artifacts.
SBF_HOST_TEST_PACKAGES := quasar-vault quasar-escrow quasar-multisig \
	upstream-vault quasar-test-suite

# Resolve first-release internal dependencies while checking package manifests.
# These patches are command-local and never enter the published archives.
PACKAGE_PATCHES := \
	--config 'patch.crates-io.quasar-schema.path="schema"' \
	--config 'patch.crates-io.quasar-idl-schema.path="idl/schema"' \
	--config 'patch.crates-io.quasar-profile.path="profile"' \
	--config 'patch.crates-io.solana-compiler-builtins.path="solana-compiler-builtins"' \
	--config 'patch.crates-io.quasar-derive.path="derive"' \
	--config 'patch.crates-io.quasar-idl.path="idl"' \
	--config 'patch.crates-io.quasar-lang.path="lang"' \
	--config 'patch.crates-io.quasar-spl.path="spl"' \
	--config 'patch.crates-io.quasar-metadata.path="metadata"'

.PHONY: format format-fix clippy clippy-fix check-features check-workspace-lints \
	check-runtime-panics check-workspace-invariants build build-sbf test test-bless \
	test-host-inventory test-host test-sbf-host \
	bench-cu bench-tracked compare-tracked test-miri test-miri-strict test-all \
	nightly-version cargo-fuzz-version test-fuzz-build generated-client-smoke \
	kani help-kani check-kani kani-lang \
	kani-spl kani-metadata msrv-check package-check audit

# Print the nightly toolchain version for CI
nightly-version:
	@echo $(NIGHTLY_TOOLCHAIN)

cargo-fuzz-version:
	@echo $(CARGO_FUZZ_VERSION)

test-fuzz-build:
	@cd lang && cargo +$(NIGHTLY_TOOLCHAIN) fuzz build

msrv-check:
	@cargo +$(PROGRAM_MSRV) check \
		$(foreach package,$(PUBLISH_PACKAGES),-p $(package)) \
		--all-features --locked

help-kani:
	@echo "Local Kani verification is optional."
	@echo "CI installs and runs Kani automatically."
	@echo ""
	@echo "Expected local version: kani $(KANI_VERSION)"
	@echo "Check version:         kani --version"
	@echo "Run all proofs:        make kani"
	@echo "Run one crate:         make kani-lang | make kani-spl | make kani-metadata"

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
	@cargo +$(NIGHTLY_TOOLCHAIN) fmt --all -- --check

format-fix:
	@cargo +$(NIGHTLY_TOOLCHAIN) fmt --all

clippy:
	@cargo +$(NIGHTLY_TOOLCHAIN) clippy --all --all-features --all-targets -- -D warnings

clippy-fix:
	@cargo +$(NIGHTLY_TOOLCHAIN) clippy --all --all-features --all-targets --fix --allow-dirty --allow-staged -- -D warnings

check-features:
	@cargo hack --feature-powerset --no-dev-deps check

check-workspace-lints:
	@missing=0; \
	while IFS= read -r manifest; do \
	  if ! rg -q '^\[lints\]$$' "$$manifest" || ! rg -q '^workspace = true$$' "$$manifest"; then \
	    echo "missing workspace lint opt-in: $$manifest" >&2; \
	    missing=1; \
	  fi; \
	done < <( \
	  cargo metadata --no-deps --format-version 1 \
	    | rg -o '"manifest_path":"[^"]+"' \
	    | sed 's/"manifest_path":"//; s/"$$//' \
	); \
	if [[ "$$missing" -ne 0 ]]; then exit 1; fi

check-runtime-panics:
	@# Panic-style macros in production runtime/derive code. Each file is scanned
	@# only up to its first #[cfg(test)] (test modules trail the file by
	@# convention), so inline test-module panics are excluded — the previous
	@# `tests/`-glob only excluded whole test directories. Allowlisted: the
	@# intentional lib.rs abort, the whole idl_build.rs, and the ice.rs helper.
	@viol=""; \
	while IFS= read -r f; do \
	  hits="$$(awk '/#\[cfg\(test\)\]/{exit} /^[[:space:]]*\/\//{next} /panic!|unreachable!|todo!|unimplemented!/{print FILENAME":"FNR": "$$0}' "$$f")"; \
	  [[ -n "$$hits" ]] && viol+="$$hits"$$'\n'; \
	done < <(find lang/src spl/src derive/src -name '*.rs'); \
	viol="$$(printf '%s' "$$viol" | grep -v 'lang/src/idl_build.rs:' | grep -vF 'panic!("program aborted")' | grep -v 'derive/src/ice.rs:')"; \
	if [[ -n "$$viol" ]]; then \
	  echo "unexpected panic-style macro in runtime/derive code:" >&2; \
	  echo "$$viol" >&2; \
	  exit 1; \
	fi
	@# No bare unwrap/expect in derive/src production code: front-end invariants
	@# panic through ice!() instead. Two sites are allowlisted by message: the
	@# quote!-generated IDL serializer (runs in the user crate) and the
	@# sibling-owned rent-plan invariant in emit/parse.rs. Test modules (scanned
	@# only up to the first #[cfg(test)]) and test-only files are excluded.
	@uw=""; \
	while IFS= read -r f; do \
	  if [[ "$$f" == */snapshot_tests.rs || "$$f" == */plan_snapshots.rs || "$$f" == */snapshots/* || "$$f" == */dump.rs ]]; then continue; fi; \
	  hits="$$(awk '/#\[cfg\(test\)\]/{exit} /^[[:space:]]*\/\//{next} /\.unwrap\(\)|\.expect\(/{print FILENAME":"FNR": "$$0}' "$$f")"; \
	  [[ -n "$$hits" ]] && uw+="$$hits"$$'\n'; \
	done < <(find derive/src -name '*.rs'); \
	uw="$$(printf '%s' "$$uw" | grep -vF 'generated IDL should serialize' | grep -vF 'rent plan field should exist in account semantics')"; \
	if [[ -n "$$uw" ]]; then \
	  echo "unexpected bare unwrap/expect in derive/src production code (use ice!() or extend the allowlist with justification):" >&2; \
	  echo "$$uw" >&2; \
	  exit 1; \
	fi

check-workspace-invariants:
	@check_allowed() { \
	  local desc="$$1" pattern="$$2"; shift 2; \
	  local allowed=("$$@") matches; \
	  matches="$$(rg -n "$$pattern" cli/src || true)"; \
	  while IFS= read -r entry; do \
	    [[ -z "$$entry" ]] && continue; \
	    local ok=0; \
	    for prefix in "$${allowed[@]}"; do \
	      if [[ "$$entry" == "$$prefix"* ]]; then ok=1; break; fi; \
	    done; \
	    if [[ "$$ok" -eq 0 ]]; then \
	      echo "unexpected $${desc}: $$entry" >&2; \
	      exit 1; \
	    fi; \
	  done <<<"$$matches"; \
	}; \
	if [[ ! -x scripts/bench-tracked-programs.sh ]]; then \
	  echo "expected executable script: scripts/bench-tracked-programs.sh" >&2; \
	  exit 1; \
	fi; \
	for script in scripts/publish-crate.sh scripts/wait-for-crate.sh; do \
	  if [[ ! -x "$$script" ]]; then \
	    echo "expected executable script: $$script" >&2; \
	    exit 1; \
	  fi; \
	done; \
	check_allowed "process::exit" 'std::process::exit|process::exit' \
	  'cli/src/main.rs:' 'cli/src/init/banner.rs:'; \
	check_allowed "polling watch loop sleep" \
	  'std::thread::sleep\(std::time::Duration::from_secs\(1\)\)' \
	  'cli/src/build_watch.rs:'; \
	if rg -n 'split_whitespace\(' cli/src >/dev/null; then \
	  echo "cli command parsing must not use split_whitespace()" >&2; \
	  rg -n 'split_whitespace\(' cli/src >&2; \
	  exit 1; \
	fi

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

# Generates the exact Cargo test-target inventory used by required CI. The
# checker also maps every tracked #[test] back to an enabled Cargo target and
# fails on disabled, unassigned, or newly unclassified tests.
test-host-inventory:
	@mkdir -p target
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_host_test_inventory.py
	@python3 scripts/host-test-inventory.py \
		$(foreach package,$(PUBLISH_PACKAGES),--tested-package $(package)) \
		$(foreach package,$(SBF_HOST_TEST_PACKAGES),--sbf-package $(package)) \
		> target/host-test-inventory.json
	@cat target/host-test-inventory.json

# Runs every ordinary host target in every publishable crate. CLI integration
# targets are derived from Cargo metadata so a new target cannot silently fall
# out of this command. generated_clients_smoke remains in its pinned toolchain
# job and is recorded as delegated in the inventory.
test-host: test-host-inventory
	@CARGO_INCREMENTAL=0 cargo test \
		$(foreach package,$(HOST_TEST_PACKAGES),-p $(package)) \
		--all-features
	@CARGO_INCREMENTAL=0 cargo test -p quasar-cli --all-features \
		$$(python3 scripts/host-test-inventory.py --cli-host-args)

test-sbf-host:
	@CARGO_INCREMENTAL=0 cargo test \
		$(foreach package,$(SBF_HOST_TEST_PACKAGES),-p $(package)) \
		--all-features

# Asserts committed trybuild .stderr goldens (trybuild default mode). A stale
# golden fails the build — that is the gate. Regenerate with `make test-bless`.
test:
	@$(MAKE) build
	@$(MAKE) build-sbf
	@$(MAKE) test-host
	@$(MAKE) test-sbf-host

# Regenerates trybuild .stderr goldens (TRYBUILD=overwrite). Use only when a
# diagnostic change is intended; review the regenerated diffs like code before
# committing. `make test` (and CI) run in assert mode and never set TRYBUILD.
test-bless:
	@$(MAKE) build
	@$(MAKE) build-sbf
	@CARGO_INCREMENTAL=0 TRYBUILD=overwrite cargo test \
		$(foreach package,$(HOST_TEST_PACKAGES),-p $(package)) \
		$(foreach package,$(SBF_HOST_TEST_PACKAGES),-p $(package)) \
		--all-features

generated-client-smoke:
	@cargo test -p quasar-cli --test generated_clients_smoke -- --nocapture --test-threads=1

package-check:
	@# First-release internal dependencies are not on crates.io yet. `msrv-check`
	@# compiles the source graph; #283 rehearses the packaged graph locally.
	@cargo package --quiet $(foreach package,$(PUBLISH_PACKAGES),-p $(package)) \
		--locked --allow-dirty --no-verify $(PACKAGE_PATCHES)

audit:
	@command -v cargo-audit >/dev/null 2>&1 || { \
		echo "cargo-audit is not installed; run: cargo install cargo-audit --locked"; \
		exit 1; \
	}
	@cargo audit

bench-cu:
	@$(MAKE) build-sbf
	@echo "Running vault CU benchmark..."
	@cargo test -p quasar-vault -- --nocapture --test-threads=1 2>&1 | grep -E '(DEPOSIT|WITHDRAW) CU:'
	@echo "Running escrow CU benchmark..."
	@cargo test -p quasar-escrow -- --nocapture --test-threads=1 2>&1 | grep -E '(MAKE|TAKE|REFUND) CU:'

bench-tracked:
	@bash scripts/bench-tracked-programs.sh capture target/tracked-metrics.env
	@cat target/tracked-metrics.env

compare-tracked:
	@bash scripts/bench-tracked-programs.sh compare

test-miri:
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-spl --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-metadata --test miri

test-miri-strict:
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-spl --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-metadata --test miri

kani-lang: check-kani
	@cargo kani -p quasar-lang

kani-spl: check-kani
	@cargo kani -p quasar-spl

kani-metadata: check-kani
	@cargo kani -p quasar-metadata

kani: kani-lang kani-spl kani-metadata

# Run all checks in sequence
test-all:
	@echo "Running all checks..."
	@$(MAKE) format
	@$(MAKE) clippy
	@$(MAKE) check-workspace-lints
	@$(MAKE) check-runtime-panics
	@$(MAKE) check-workspace-invariants
	@$(MAKE) test
	@$(MAKE) generated-client-smoke
	@$(MAKE) package-check
	@$(MAKE) audit
	@$(MAKE) test-fuzz-build
	@$(MAKE) test-miri-strict
	@echo "All checks passed!"
