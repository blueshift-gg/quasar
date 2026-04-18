SHELL := /usr/bin/env bash
NIGHTLY_TOOLCHAIN := nightly
KANI_VERSION := 0.67.0
# platform-tools v1.52 ships Cargo 1.89 which supports Cargo.lock v4.
# v1.51 ships Cargo 1.84 which does not, causing "duplicate lang item" errors.
PLATFORM_TOOLS := v1.52

# Test programs that produce SBF binaries
SBF_TEST_PROGRAMS := tests/programs/test-misc tests/programs/test-errors \
	tests/programs/test-events tests/programs/test-pda \
	tests/programs/test-token-cpi tests/programs/test-token-init \
	tests/programs/test-token-validate tests/programs/test-sysvar

# Example programs that produce SBF binaries
SBF_EXAMPLES := examples/vault examples/escrow examples/multisig

# All SBF programs
SBF_ALL := $(SBF_EXAMPLES) $(SBF_TEST_PROGRAMS)

.PHONY: format format-fix clippy clippy-fix check-features check-workspace-lints \
	check-runtime-panics check-workspace-invariants build build-sbf test bench-cu \
	bench-tracked compare-tracked test-miri test-miri-strict test-all nightly-version \
	kani help-kani check-kani kani-lang kani-spl

# Print the nightly toolchain version for CI
nightly-version:
	@echo $(NIGHTLY_TOOLCHAIN)

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
	@matches="$$( \
	  rg -n 'panic!|unreachable!|todo!|unimplemented!' \
	    lang/src spl/src derive/src \
	    --glob '!**/tests/**' || true \
	)"; \
	violations=(); \
	while IFS= read -r entry; do \
	  [[ -z "$$entry" ]] && continue; \
	  code="$${entry#*:*:}"; \
	  if [[ "$$code" =~ ^[[:space:]]*// ]]; then continue; fi; \
	  case "$$entry" in \
	    *'lang/src/lib.rs:'*'panic!("program aborted")'*) continue ;; \
	    *'derive/src/accounts/evidence.rs:'*) continue ;; \
	  esac; \
	  violations+=("$$entry"); \
	done <<<"$$matches"; \
	if (($${#violations[@]} > 0)); then \
	  echo "unexpected panic-style macro in runtime/derive code:" >&2; \
	  printf '  %s\n' "$${violations[@]}" >&2; \
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
	for script in scripts/bench-tracked-programs.sh scripts/setup-branch-protection.sh; do \
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

test:
	@$(MAKE) build
	@$(MAKE) build-sbf
	@cargo test -p quasar-lang -p quasar-derive -p quasar-spl \
		-p quasar-vault -p quasar-escrow -p quasar-multisig \
		-p quasar-test-suite \
		--all-features

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

test-miri-strict:
	@MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-symbolic-alignment-check -Zmiri-strict-provenance" \
		cargo +$(NIGHTLY_TOOLCHAIN) miri test -p quasar-lang --test miri -- --skip remaining
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
	@$(MAKE) check-workspace-lints
	@$(MAKE) check-runtime-panics
	@$(MAKE) check-workspace-invariants
	@$(MAKE) build-sbf
	@$(MAKE) test
	@$(MAKE) test-miri
	@echo "All checks passed!"
