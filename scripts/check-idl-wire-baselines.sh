#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: check-idl-wire-baselines.sh <check|bless> <baseline-dir>" >&2
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
  echo "missing IDL wire profile inventory: $profiles" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

while IFS=$'\t' read -r fixture crate_path file extra; do
  [[ -z "$fixture" || "$fixture" == \#* ]] && continue
  if [[ -n "${extra:-}" || -z "$crate_path" || -z "$file" ]]; then
    echo "invalid IDL wire profile row: $fixture $crate_path $file ${extra:-}" >&2
    exit 1
  fi
  if [[ ! "$fixture" =~ ^[a-z][a-z0-9_-]*$ ]]; then
    echo "invalid IDL wire fixture: $fixture" >&2
    exit 1
  fi
  if [[ "$crate_path" == /* || "$crate_path" == *..* \
    || ! -f "$crate_path/Cargo.toml" ]]; then
    echo "invalid IDL wire program crate: $crate_path" >&2
    exit 1
  fi
  if [[ "$file" != "programs/$fixture.abi.json" ]]; then
    echo "baseline file must match fixture $fixture: $file" >&2
    exit 1
  fi
  printf '%s\n' "$fixture" >>"$tmp/fixtures"
  printf '%s\n' "$crate_path" >>"$tmp/crate-paths"
  printf '%s\n' "$file" >>"$tmp/expected-files"
done <"$profiles"

if [[ ! -s "$tmp/fixtures" ]]; then
  echo "IDL wire profile inventory is empty" >&2
  exit 1
fi

for field in fixtures crate-paths expected-files; do
  LC_ALL=C sort "$tmp/$field" -o "$tmp/$field"
  if [[ -n "$(uniq -d "$tmp/$field")" ]]; then
    echo "IDL wire ${field} must be unique" >&2
    exit 1
  fi
done

if [[ "$mode" == "bless" ]]; then
  mkdir -p "$baseline_dir/programs"
else
  find "$baseline_dir/programs" -maxdepth 1 -type f -name '*.abi.json' \
    -exec basename {} \; | sed 's|^|programs/|' | LC_ALL=C sort >"$tmp/actual-files"
  if ! diff -u "$tmp/expected-files" "$tmp/actual-files"; then
    echo "IDL wire baseline files do not match profiles.tsv" >&2
    exit 1
  fi
fi

baseline_dir="$(cd "$baseline_dir" && pwd)"
if [[ "$mode" == "bless" ]]; then
  QUASAR_IDL_WIRE_BASELINE_DIR="$baseline_dir" UPDATE_EXPECT=1 \
    cargo test -p quasar-cli --test idl_wire_baseline -- --test-threads=1
else
  QUASAR_IDL_WIRE_BASELINE_DIR="$baseline_dir" \
    cargo test -p quasar-cli --test idl_wire_baseline -- --test-threads=1
fi

find "$baseline_dir/programs" -maxdepth 1 -type f -name '*.abi.json' \
  -exec basename {} \; | sed 's|^|programs/|' | LC_ALL=C sort >"$tmp/actual-files"
if ! diff -u "$tmp/expected-files" "$tmp/actual-files"; then
  echo "IDL wire baseline files do not match profiles.tsv" >&2
  exit 1
fi

while IFS= read -r file; do
  path="$baseline_dir/$file"
  jq -e '
    (.name | type == "string")
    and (.address | type == "string")
    and (.instructions | type == "array")
    and (.accounts | type == "array")
    and (.types | type == "array")
    and (.events | type == "array")
    and (.errors | type == "array")
  ' "$path" >/dev/null
  if rg -n '"(spec|version|metadata|docs|msg|formula|semantics|extensions|hashes)"[[:space:]]*:' "$path"; then
    echo "non-ABI fields leaked into $file" >&2
    exit 1
  fi
done <"$tmp/expected-files"
