#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: check-proc-macro-baselines.sh <check|bless> <baseline-dir> <public-api-baseline>" >&2
  exit 2
}

[[ "$#" -eq 3 ]] || usage

mode="$1"
baseline_dir="$2"
public_api_baseline="$3"
profiles="$baseline_dir/profiles.tsv"
snapshot_source="derive/src/snapshot_tests.rs"

if [[ "$mode" != "check" && "$mode" != "bless" ]]; then
  usage
fi
if [[ ! -f "$profiles" ]]; then
  echo "missing proc-macro profile inventory: $profiles" >&2
  exit 1
fi
if [[ ! -f "$public_api_baseline" ]]; then
  echo "missing proc-macro public API baseline: $public_api_baseline" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

while IFS=$'\t' read -r family fixture file extra; do
  [[ -z "$family" || "$family" == \#* ]] && continue
  if [[ -n "${extra:-}" || -z "$fixture" || -z "$file" ]]; then
    echo "invalid proc-macro profile row: $family $fixture $file ${extra:-}" >&2
    exit 1
  fi
  if [[ ! "$family" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
    echo "invalid proc-macro family: $family" >&2
    exit 1
  fi
  if [[ ! "$fixture" =~ ^[a-z][a-z0-9_]*$ ]]; then
    echo "invalid proc-macro fixture: $fixture" >&2
    exit 1
  fi
  if [[ "$file" != "expansions/$fixture.rs" ]]; then
    echo "baseline file must match fixture $fixture: $file" >&2
    exit 1
  fi
  printf '%s\n' "$family" >>"$tmp/profile-families"
  printf '%s\n' "$fixture" >>"$tmp/profile-fixtures"
  printf '%s\n' "$file" >>"$tmp/expected-files"
done <"$profiles"

if [[ ! -s "$tmp/profile-families" ]]; then
  echo "proc-macro profile inventory is empty" >&2
  exit 1
fi

for field in profile-fixtures expected-files; do
  LC_ALL=C sort "$tmp/$field" -o "$tmp/$field"
  if [[ -n "$(uniq -d "$tmp/$field")" ]]; then
    echo "proc-macro ${field#*-} must be unique" >&2
    exit 1
  fi
done

LC_ALL=C sort -u "$tmp/profile-families" >"$tmp/profile-family-set"
awk '
  /^#\[proc_macro\]$/ || /^#\[proc_macro_attribute\]$/ {
    pending = "function-name"
    next
  }
  /^#\[proc_macro_derive\(/ {
    name = $0
    sub(/^#\[proc_macro_derive\(/, "", name)
    sub(/[,)].*$/, "", name)
    print name
    pending = "derive"
    next
  }
  pending == "function-name" && /^pub fn / {
    name = $0
    sub(/^pub fn /, "", name)
    sub(/\(.*/, "", name)
    print name
    pending = ""
    next
  }
  pending == "derive" && /^pub fn / {
    pending = ""
  }
' derive/src/lib.rs | LC_ALL=C sort -u >"$tmp/current-family-set"
if ! diff -u "$tmp/current-family-set" "$tmp/profile-family-set"; then
  echo "proc-macro profiles must cover every current public macro family" >&2
  exit 1
fi

awk '$1 == "proc-macro" {
  name = $3
  sub(/^.*::/, "", name)
  print name
}' "$public_api_baseline" | LC_ALL=C sort -u >"$tmp/public-family-set"
comm -23 "$tmp/public-family-set" "$tmp/profile-family-set" >"$tmp/missing-published-families"
if [[ -s "$tmp/missing-published-families" ]]; then
  echo "proc-macro profiles omit published macro families:" >&2
  sed 's/^/  - /' "$tmp/missing-published-families" >&2
  exit 1
fi

while IFS= read -r fixture; do
  if [[ "$(grep -Fxc "fn $fixture() {" "$snapshot_source")" -ne 1 ]]; then
    echo "expected exactly one snapshot test named $fixture" >&2
    exit 1
  fi
done <"$tmp/profile-fixtures"

sed -n 's/.*expect_test::expect_file!\["\(expansions\/[^" ]*\.rs\)"\].*/\1/p' \
  "$snapshot_source" | LC_ALL=C sort >"$tmp/source-files"
if ! diff -u "$tmp/expected-files" "$tmp/source-files"; then
  echo "snapshot test baseline references do not match profiles.tsv" >&2
  exit 1
fi

if [[ "$mode" == "bless" ]]; then
  mkdir -p "$baseline_dir/expansions"
else
  find "$baseline_dir/expansions" -maxdepth 1 -type f -name '*.rs' \
    -exec basename {} \; | sed 's|^|expansions/|' | LC_ALL=C sort >"$tmp/actual-files"
  if ! diff -u "$tmp/expected-files" "$tmp/actual-files"; then
    echo "proc-macro expansion files do not match profiles.tsv" >&2
    exit 1
  fi
fi

baseline_dir="$(cd "$baseline_dir" && pwd)"

if [[ "$mode" == "bless" ]]; then
  QUASAR_PROC_MACRO_BASELINE_DIR="$baseline_dir" UPDATE_EXPECT=1 \
    cargo test -p quasar-derive --all-features snapshot_tests:: -- --test-threads=1
else
  QUASAR_PROC_MACRO_BASELINE_DIR="$baseline_dir" \
    cargo test -p quasar-derive --all-features snapshot_tests:: -- --test-threads=1
fi

find "$baseline_dir/expansions" -maxdepth 1 -type f -name '*.rs' \
  -exec basename {} \; | sed 's|^|expansions/|' | LC_ALL=C sort >"$tmp/actual-files"
if ! diff -u "$tmp/expected-files" "$tmp/actual-files"; then
  echo "proc-macro expansion files do not match profiles.tsv" >&2
  exit 1
fi
