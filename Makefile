SHELL := /usr/bin/env bash
# Keep rustfmt, Clippy, and Miri deterministic across local and CI runs.
NIGHTLY_TOOLCHAIN := nightly-2026-03-27
KANI_VERSION := 0.67.0
CARGO_FUZZ_VERSION := 0.13.2
CARGO_AUDIT_VERSION := 0.22.1
CARGO_PUBLIC_API_VERSION := 0.52.0
LICENSE_EXPRESSION := Apache-2.0 OR MIT
PUBLIC_API_BASELINE_VERSION := v0.1.0
PUBLIC_API_BASELINE_DIR := api-baselines/$(PUBLIC_API_BASELINE_VERSION)
PUBLIC_API_TARGET := x86_64-unknown-linux-gnu
PROC_MACRO_BASELINE_VERSION := v0.1.0
PROC_MACRO_BASELINE_DIR := compatibility-baselines/$(PROC_MACRO_BASELINE_VERSION)/proc-macros
IDL_WIRE_BASELINE_VERSION := v0.1.0
IDL_WIRE_BASELINE_DIR := compatibility-baselines/$(IDL_WIRE_BASELINE_VERSION)/idl-wire
GENERATED_CLIENT_BASELINE_VERSION := v0.1.0
GENERATED_CLIENT_BASELINE_DIR := compatibility-baselines/$(GENERATED_CLIENT_BASELINE_VERSION)/generated-clients
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

# Non-release targets may select the current publishable workspace without
# maintaining an inventory. Publication order and packaging come exclusively
# from quasar-release-tool.
PUBLISHABLE_PACKAGES = $(shell cargo metadata --locked --no-deps --format-version 1 \
	| jq -r '.packages[] | select(.publish != []) | .name')

# Publishable crates whose ordinary host tests can run without the generated
# client toolchains. The CLI smoke target is delegated below because it needs
# pinned Node, Python, Go, Clang, and Caravel dependencies.
HOST_TEST_PACKAGES = $(filter-out quasar-cli,$(PUBLISHABLE_PACKAGES))

# Host-side tests that consume freshly built SBF artifacts.
SBF_HOST_TEST_PACKAGES := quasar-vault quasar-escrow quasar-multisig quasar-test-suite

PACKAGE_REHEARSAL_ROOT ?= target/release-rehearsal

.PHONY: format format-fix clippy clippy-fix check-features check-workspace-lints \
	check-runtime-panics check-workspace-invariants check-test-silence \
	check-suite-oracles check-unsafe-policy check-license-policy \
	check-package-metadata check-readme-crate-inventory check-release-train \
	build build-sbf test test-bless \
	test-host-inventory test-host test-sbf-host test-quasar-test-standalone \
	bench-cu bench-tracked compare-tracked test-benchmark-policy doc-check \
	test-miri test-miri-strict test-all \
	nightly-version cargo-fuzz-version cargo-audit-version cargo-public-api-version \
	test-fuzz-build check-public-api bless-public-api \
	check-proc-macro-baselines bless-proc-macro-baselines \
	check-idl-wire-baselines bless-idl-wire-baselines \
	check-generated-client-baselines bless-generated-client-baselines \
	test-audit-policy generated-client-smoke \
	check-release-dependencies test-release-dependency-policy \
	check-release-permissions test-release-permission-policy \
	test-matrix check-test-matrix coverage \
	mutants mutants-bless kani help-kani check-kani kani-lang \
	kani-spl msrv-check package-check package-rehearsal audit

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
		"$(PUBLIC_API_TARGET)" $(PUBLISHABLE_PACKAGES)

bless-public-api:
	@scripts/check-public-api.sh bless "$(PUBLIC_API_BASELINE_DIR)" \
		"$(NIGHTLY_TOOLCHAIN)" "$(CARGO_PUBLIC_API_VERSION)" \
		"$(PUBLIC_API_TARGET)" $(PUBLISHABLE_PACKAGES)

check-proc-macro-baselines:
	@scripts/check-proc-macro-baselines.sh check "$(PROC_MACRO_BASELINE_DIR)" \
		"$(PUBLIC_API_BASELINE_DIR)/quasar-derive.txt"

bless-proc-macro-baselines:
	@scripts/check-proc-macro-baselines.sh bless "$(PROC_MACRO_BASELINE_DIR)" \
		"$(PUBLIC_API_BASELINE_DIR)/quasar-derive.txt"

check-idl-wire-baselines:
	@scripts/check-idl-wire-baselines.sh check "$(IDL_WIRE_BASELINE_DIR)"

bless-idl-wire-baselines:
	@scripts/check-idl-wire-baselines.sh bless "$(IDL_WIRE_BASELINE_DIR)"

check-generated-client-baselines:
	@scripts/check-generated-client-baselines.sh check "$(GENERATED_CLIENT_BASELINE_DIR)"

bless-generated-client-baselines:
	@scripts/check-generated-client-baselines.sh bless "$(GENERATED_CLIENT_BASELINE_DIR)"

test-fuzz-build:
	@cd lang && cargo +$(NIGHTLY_TOOLCHAIN) fuzz build

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

# Every unsafe site carries its soundness argument (STYLE.md): SAFETY
# comments name preconditions, unsafe fns carry # Safety contracts.
check-unsafe-policy:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_check_unsafe_policy.py
	@python3 scripts/check-unsafe-policy.py

# Rejection tests pin the exact error (TESTING.md): bare is_err() cannot
# distinguish "the right check fired" from "an earlier check masked a broken
# one". Allowlist (with reasons) lives in the script.
check-suite-oracles:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_check_suite_oracles.py
	@python3 scripts/check-suite-oracles.py

# Tests are silent on success (TESTING.md): anything worth printing is worth
# asserting. Benchmark CU goes to target/cu-bench/*.jsonl via
# examples/cu_bench.rs, never to stdout. Covers the SVM suite, the example
# benches, every crate's host test directory, and the quasar-test harness.
check-test-silence:
	@viol="$$(rg -n 'println!|eprintln!' tests/suite/src \
	  examples/vault/src/tests.rs examples/escrow/src/tests.rs \
	  examples/multisig/src/tests.rs \
	  lang/tests derive/tests spl/tests idl/tests cli/tests \
	  testing/src \
	  -g '!compile_fail/**' -g '!compile_pass/**' \
	  || true)"; \
	if [[ -n "$$viol" ]]; then \
	  echo "test code must not print (TESTING.md: assert, don't print):" >&2; \
	  echo "$$viol" >&2; \
	  exit 1; \
	fi

check-workspace-invariants: check-license-policy check-package-metadata \
	check-readme-crate-inventory check-release-train check-test-silence \
	check-suite-oracles check-unsafe-policy
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
	for script in scripts/install-solana-tools.sh; do \
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

check-readme-crate-inventory:
	@python3 scripts/check-readme-crate-inventory.py

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

# Generates the exact Cargo test-target inventory used by required CI. The
# checker also maps every tracked #[test] back to an enabled Cargo target and
# fails on disabled, unassigned, or newly unclassified tests.
test-host-inventory:
	@mkdir -p target
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_host_test_inventory.py
	@python3 scripts/host-test-inventory.py \
		$(foreach package,$(PUBLISHABLE_PACKAGES),--tested-package $(package)) \
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
	@CARGO_INCREMENTAL=0 TRYBUILD=overwrite cargo test \
		$(foreach package,$(HOST_TEST_PACKAGES),-p $(package)) \
		$(foreach package,$(SBF_HOST_TEST_PACKAGES),-p $(package)) \
		--all-features

generated-client-smoke:
	@cargo test -p quasar-cli --test generated_clients_smoke -- --nocapture --test-threads=1

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
	@$(MAKE) test-audit-policy
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/audit-release-reachability.py

test-audit-policy:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_audit_release_reachability.py

check-release-dependencies: test-release-dependency-policy
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-release-dependencies.py

test-release-dependency-policy:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_release_dependency_policy.py

check-release-permissions: test-release-permission-policy
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-release-permissions.py

test-release-permission-policy:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_release_permission_policy.py

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

test-miri:
	@MIRIFLAGS="-Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-spl --test miri

test-miri-strict:
	@MIRIFLAGS="-Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri_extensions
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-spl --test miri

# Feature -> test traceability (TESTING.md): the grid of what is tested and
# how. check mode fails on an empty required cell, manifest drift, or a suite
# module no feature claims.
test-matrix:
	@python3 scripts/test-matrix.py

check-test-matrix:
	@PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/test_test_matrix.py
	@python3 scripts/test-matrix.py --check

# Host-side line coverage, informational only: code executed under
# SBF/Mollusk is invisible here (TESTING.md) — the matrix above and the
# mutation baseline are the assurance metrics, never a coverage percentage.
coverage:
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { \
		echo "cargo-llvm-cov is not installed; run: cargo install cargo-llvm-cov --locked"; \
		exit 1; \
	}
	@cargo llvm-cov --workspace \
		$(foreach package,$(SBF_HOST_TEST_PACKAGES),--exclude $(package)) \
		--all-features --html
	@echo "HTML report: target/llvm-cov/html/index.html"

# Mutation testing (TESTING.md): per-package cargo-mutants runs, then the
# missed set is judged against the shrink-only baseline in
# .ci/mutants-baseline.txt. Deep-tier: nightly CI, not the PR gate.
mutants:
	@scripts/mutants.sh run-all
	@scripts/mutants.sh check-baseline

mutants-bless:
	@scripts/mutants.sh bless-baseline

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
	@$(MAKE) check-workspace-lints
	@$(MAKE) check-runtime-panics
	@$(MAKE) check-workspace-invariants
	@$(MAKE) test
	@$(MAKE) generated-client-smoke
	@$(MAKE) package-check
	@$(MAKE) audit
	@$(MAKE) test-benchmark-policy
	@$(MAKE) doc-check
	@$(MAKE) test-fuzz-build
	@$(MAKE) test-miri-strict
	@echo "All checks passed!"
