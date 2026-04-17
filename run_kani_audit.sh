#!/usr/bin/env bash
# Run Kani proofs across all crates and generate KANI_HARNESS_AUDIT.md.
# Usage: bash run_kani_audit.sh
set -euo pipefail

CRATES="quasar-lang quasar-spl"
TMPDIR=$(mktemp -d)
OUTFILE="KANI_HARNESS_AUDIT.md"

total_pass=0
total_fail=0

for crate in $CRATES; do
  echo "=== Running Kani for $crate ===" >&2
  logfile="$TMPDIR/$crate.log"
  set +e
  cargo kani -p "$crate" 2>&1 | tee "$logfile" >&2
  kani_exit=$?
  set -e
  echo "" >&2
  if [ $kani_exit -ne 0 ]; then
    echo "WARNING: cargo kani failed for $crate (exit $kani_exit)" >&2
  fi

  # Parse: pair "Checking harness <name>..." with next "VERIFICATION:- SUCCESSFUL|FAILED"
  awk '
    /^Checking harness / {
      name = $0
      sub(/^Checking harness /, "", name)
      sub(/\.\.\.$/, "", name)
      gsub(/[[:space:]]+$/, "", name)
    }
    /^VERIFICATION:- SUCCESSFUL/ && name != "" {
      print name "\tPASS"
      name = ""
    }
    /^VERIFICATION:- FAILED/ && name != "" {
      print name "\tFAIL"
      name = ""
    }
  ' "$logfile" > "$TMPDIR/$crate.results"

  pass=$(grep -c 'PASS$' "$TMPDIR/$crate.results" 2>/dev/null) || true
  fail=$(grep -c 'FAIL$' "$TMPDIR/$crate.results" 2>/dev/null) || true
  pass=${pass:-0}
  fail=${fail:-0}
  echo "$crate $pass $fail $kani_exit" >> "$TMPDIR/summary.txt"
  total_pass=$((total_pass + pass))
  total_fail=$((total_fail + fail))
  echo "=== $crate: $pass passed, $fail failed ===" >&2
done

# Generate markdown
{
  echo "# Kani Harness Audit"
  echo ""
  echo "Verification of all \`#[cfg(kani)]\` proof harnesses via \`cargo kani\`."
  echo ""
  echo "**Kani version:** $(kani --version 2>/dev/null | awk '{print $2}' || echo 'unknown')"
  echo "**Total:** $((total_pass + total_fail)) harnesses across 2 crates — $total_pass passed, $total_fail failed"
  echo ""
  echo "## Summary"
  echo ""
  echo "| Crate | Harnesses | Passed | Failed |"
  echo "|---|---|---|---|"
  while read -r crate pass fail exit_code; do
    if [ "$exit_code" -ne 0 ]; then
      echo "| $crate | — | — | **cargo kani failed** |"
    else
      echo "| $crate | $((pass + fail)) | $pass | $fail |"
    fi
  done < "$TMPDIR/summary.txt"
  echo ""
  echo "## Full Results"
  echo ""
  echo "| Crate | Harness | Result |"
  echo "|---|---|---|"
  for crate in $CRATES; do
    short="${crate#quasar-}"
    while IFS=$(printf '\t') read -r name result; do
      if [ "$result" = "PASS" ]; then
        echo "| $short | \`$name\` | PASS |"
      else
        echo "| $short | \`$name\` | **FAIL** |"
      fi
    done < "$TMPDIR/$crate.results"
  done
} > "$OUTFILE"

echo "" >&2
echo "Wrote $OUTFILE ($((total_pass + total_fail)) harnesses)" >&2
rm -rf "$TMPDIR"
exit $((total_fail > 0 ? 1 : 0))
