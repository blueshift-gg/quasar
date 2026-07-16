#!/usr/bin/env bash
# Mutation testing driver (TESTING.md: "Mutation testing").
#
# Runs cargo-mutants per package with that package's oracle flags, then
# compares the union of missed mutants against the committed baseline
# .ci/mutants-baseline.txt. The baseline is a shrink-only ratchet: a missed
# mutant not recorded there fails the run; killing a recorded one is always
# welcome (re-bless to shrink the file).
#
# Baseline entries are normalized by stripping line:column so ordinary code
# motion does not churn the file; a mutant is identified by its file,
# function, and mutation description.
#
# Usage:
#   scripts/mutants.sh run <package>     Run one package, leave results in target/mutants/<package>.
#   scripts/mutants.sh run-all           Run every configured package.
#   scripts/mutants.sh check-baseline    Fail if any missed mutant is not in the baseline.
#   scripts/mutants.sh bless-baseline    Rewrite the baseline from the last run's results.
#   scripts/mutants.sh packages          List configured packages (for CI matrix fan-out).
set -euo pipefail

CARGO_MUTANTS_VERSION_EXPECTED="27"
BASELINE_FILE=".ci/mutants-baseline.txt"
OUT_ROOT="target/mutants"

# Configured packages. Scope and oracle per package:
# - quasar-lang: mutate the host-testable validation core; unit tests in the
#   lib (lang/src/checks/tests.rs and friends) are the oracle. The SBF-only
#   runtime is excluded in .cargo/mutants.toml and covered by the SVM suite,
#   Miri, and Kani instead.
# - quasar-derive: whole crate; the expansion/plan-IR snapshot tests in the
#   lib are the oracle (a codegen mutant shows up as a snapshot diff).
# - quasar-idl / quasar-schema / quasar-idl-schema: whole crate; their host
#   test suites are the oracle.
# Not yet configured (documented debt, extend deliberately): quasar-cli
# (integration tests shell out to toolchains), quasar-spl / quasar-metadata
# (host-side oracle is thin; assurance is Kani + the SVM validate suites).
PACKAGES=(quasar-lang quasar-schema quasar-idl-schema quasar-derive quasar-idl)

mutants_args_for() {
  local package="$1"
  case "$package" in
    quasar-lang)
      printf '%s\n' "-f" "lang/src/checks/*.rs" "-f" "lang/src/error.rs" \
        "--test-package" "quasar-lang" "--" "--lib"
      ;;
    quasar-derive)
      printf '%s\n' "--test-package" "quasar-derive" "--" "--lib"
      ;;
    quasar-idl | quasar-schema | quasar-idl-schema)
      printf '%s\n' "--test-package" "$package"
      ;;
    *)
      echo "unknown package: $package" >&2
      return 1
      ;;
  esac
}

check_tool() {
  if ! command -v cargo-mutants >/dev/null 2>&1; then
    echo "cargo-mutants is not installed; run: cargo install cargo-mutants --locked" >&2
    exit 1
  fi
  local version
  version="$(cargo mutants --version | awk '{print $2}')"
  if [[ "${version%%.*}" != "$CARGO_MUTANTS_VERSION_EXPECTED" ]]; then
    echo "unexpected cargo-mutants major version: $version (expected ${CARGO_MUTANTS_VERSION_EXPECTED}.x)" >&2
    exit 1
  fi
}

# Strip line:col so code motion does not churn baselines:
# "lang/src/checks/address.rs:8:9: replace X" -> "lang/src/checks/address.rs: replace X"
normalize() {
  sed -E 's/:[0-9]+:[0-9]+: / /' | LC_ALL=C sort -u
}

run_one() {
  local package="$1"
  local out_dir="$OUT_ROOT/$package"
  local args=()
  while IFS= read -r arg; do
    args+=("$arg")
  done < <(mutants_args_for "$package")

  mkdir -p "$out_dir"
  echo "=== cargo mutants: $package ==="
  # cargo-mutants exits non-zero when mutants are missed; missed mutants are
  # judged against the baseline in check_baseline, not here. MUTANTS_JOBS
  # trades local wall-clock for memory; CI shards instead.
  cargo mutants -p "$package" -j "${MUTANTS_JOBS:-1}" --no-shuffle \
    --output "$out_dir" "${args[@]}" || true

  if [[ ! -f "$out_dir/mutants.out/outcomes.json" ]]; then
    echo "$package: cargo-mutants produced no outcomes (baseline build/test failed?)" >&2
    exit 1
  fi
}

collect_missed() {
  local package missed
  for package in "${PACKAGES[@]}"; do
    missed="$OUT_ROOT/$package/mutants.out/missed.txt"
    if [[ -f "$missed" ]]; then
      cat "$missed"
    fi
  done | normalize
}

check_baseline() {
  if [[ ! -f "$BASELINE_FILE" ]]; then
    echo "missing baseline: $BASELINE_FILE (run: scripts/mutants.sh bless-baseline)" >&2
    exit 1
  fi
  local new_missed
  new_missed="$(comm -23 <(collect_missed) <(normalize <"$BASELINE_FILE"))"
  if [[ -n "$new_missed" ]]; then
    echo "new missed mutants (not in $BASELINE_FILE):" >&2
    echo "$new_missed" >&2
    echo >&2
    echo "Either add a test that kills the mutant, or — only for a provably" >&2
    echo "equivalent mutation — add it to the baseline with justification" >&2
    echo "in the PR (TESTING.md: ratchets only shrink)." >&2
    exit 1
  fi
  local fixed
  fixed="$(comm -13 <(collect_missed) <(normalize <"$BASELINE_FILE"))"
  if [[ -n "$fixed" ]]; then
    echo "baseline entries now killed (shrink the baseline with bless-baseline):"
    echo "$fixed"
  fi
  echo "mutation baseline check passed"
}

bless_baseline() {
  mkdir -p "$(dirname "$BASELINE_FILE")"
  {
    echo "# Missed mutants accepted as debt (normalized: no line:col)."
    echo "# Shrink-only: additions require justification in the PR (TESTING.md)."
    collect_missed
  } >"$BASELINE_FILE"
  echo "wrote $(grep -vc '^#' "$BASELINE_FILE") entries to $BASELINE_FILE"
}

main() {
  if (($# < 1)); then
    sed -n '2,20p' "$0" >&2
    exit 1
  fi
  case "$1" in
    run)
      (($# == 2)) || { echo "usage: scripts/mutants.sh run <package>" >&2; exit 1; }
      check_tool
      run_one "$2"
      ;;
    run-all)
      check_tool
      local package
      for package in "${PACKAGES[@]}"; do
        run_one "$package"
      done
      ;;
    check-baseline) check_baseline ;;
    bless-baseline) bless_baseline ;;
    packages) printf '%s\n' "${PACKAGES[@]}" ;;
    *)
      echo "unknown command: $1" >&2
      exit 1
      ;;
  esac
}

main "$@"
