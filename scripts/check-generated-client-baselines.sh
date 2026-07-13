#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: check-generated-client-baselines.sh <check|bless> <baseline-dir>" >&2
  exit 2
}

[[ "$#" -eq 2 ]] || usage

mode="$1"
baseline_dir="$2"
profiles="$baseline_dir/profiles.tsv"

if [[ "$mode" != "check" && "$mode" != "bless" ]]; then
  usage
fi
if [[ ! -f "$profiles" ]]; then
  echo "missing generated-client profile inventory: $profiles" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

while IFS=$'\t' read -r fixture crate_path extra; do
  [[ -z "$fixture" || "$fixture" == \#* ]] && continue
  if [[ -n "${extra:-}" || -z "$crate_path" ]]; then
    echo "invalid generated-client profile row: $fixture $crate_path ${extra:-}" >&2
    exit 1
  fi
  if [[ ! "$fixture" =~ ^[a-z][a-z0-9_-]*$ ]]; then
    echo "invalid generated-client fixture: $fixture" >&2
    exit 1
  fi
  if [[ "$crate_path" == /* || "$crate_path" == *..* \
    || ! -f "$crate_path/Cargo.toml" ]]; then
    echo "invalid generated-client program crate: $crate_path" >&2
    exit 1
  fi
  printf '%s\n' "$fixture" >>"$tmp/fixtures"
  printf '%s\n' "$crate_path" >>"$tmp/crate-paths"
done <"$profiles"

if [[ ! -s "$tmp/fixtures" ]]; then
  echo "generated-client profile inventory is empty" >&2
  exit 1
fi

for field in fixtures crate-paths; do
  LC_ALL=C sort "$tmp/$field" -o "$tmp/$field"
  if [[ -n "$(uniq -d "$tmp/$field")" ]]; then
    echo "generated-client ${field} must be unique" >&2
    exit 1
  fi
done

if [[ "$mode" == "bless" ]]; then
  mkdir -p "$baseline_dir/outputs"
  while IFS= read -r fixture; do
    mkdir -p "$baseline_dir/outputs/$fixture"
  done <"$tmp/fixtures"
elif [[ ! -d "$baseline_dir/outputs" ]]; then
  echo "missing generated-client baseline outputs: $baseline_dir/outputs" >&2
  exit 1
fi

find "$baseline_dir/outputs" -mindepth 1 -maxdepth 1 -type d \
  -exec basename {} \; | LC_ALL=C sort >"$tmp/actual-fixtures"
if ! diff -u "$tmp/fixtures" "$tmp/actual-fixtures"; then
  echo "generated-client baseline directories do not match profiles.tsv" >&2
  exit 1
fi

baseline_dir="$(cd "$baseline_dir" && pwd)"
if [[ "$mode" == "bless" ]]; then
  QUASAR_GENERATED_CLIENT_BASELINE_DIR="$baseline_dir" UPDATE_EXPECT=1 \
    cargo test -p quasar-cli --test generated_client_baseline -- --test-threads=1
else
  QUASAR_GENERATED_CLIENT_BASELINE_DIR="$baseline_dir" \
    cargo test -p quasar-cli --test generated_client_baseline -- --test-threads=1
fi
