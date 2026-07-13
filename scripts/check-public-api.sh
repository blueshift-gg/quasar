#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: check-public-api.sh <check|bless> <baseline-dir> <nightly> <tool-version> <target> <package>...
EOF
  exit 2
}

[[ "$#" -ge 6 ]] || usage

mode="$1"
baseline_dir="$2"
nightly="$3"
expected_tool_version="$4"
target="$5"
shift 5
packages=("$@")

if [[ "$mode" != "check" && "$mode" != "bless" ]]; then
  usage
fi

actual_tool_version="$(cargo public-api --version 2>/dev/null | awk '{print $2}' || true)"
if [[ "$actual_tool_version" != "$expected_tool_version" ]]; then
  echo "unexpected cargo-public-api version: ${actual_tool_version:-not installed}" >&2
  echo "expected: $expected_tool_version" >&2
  exit 1
fi

if ! rustc "+$nightly" --version >/dev/null 2>&1; then
  echo "Rust toolchain $nightly is not installed" >&2
  exit 1
fi

profiles="$baseline_dir/profiles.tsv"
if [[ ! -f "$profiles" ]]; then
  echo "missing public API profile inventory: $profiles" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

printf '%s\n' "${packages[@]}" | LC_ALL=C sort -u >"$tmp/expected-packages"

declare -a entry_packages=()
declare -a entry_profiles=()
declare -a entry_files=()
while IFS=$'\t' read -r package profile file extra; do
  [[ -z "$package" || "$package" == \#* ]] && continue
  if [[ -n "${extra:-}" || -z "$profile" || -z "$file" ]]; then
    echo "invalid public API profile row: $package $profile $file ${extra:-}" >&2
    exit 1
  fi
  if ! grep -Fxq "$package" "$tmp/expected-packages"; then
    echo "public API profile references unknown package: $package" >&2
    exit 1
  fi
  if [[ "$profile" != "default" \
    && "$profile" != "all-features" \
    && "$profile" != "proc-macro" ]]; then
    echo "unsupported public API profile for $package: $profile" >&2
    exit 1
  fi
  if [[ "$file" == */* || "$file" != *.txt ]]; then
    echo "invalid public API baseline filename: $file" >&2
    exit 1
  fi
  entry_packages+=("$package")
  entry_profiles+=("$profile")
  entry_files+=("$file")
  printf '%s\t%s\n' "$package" "$profile" >>"$tmp/profile-keys"
done <"$profiles"

if [[ "${#entry_packages[@]}" -eq 0 ]]; then
  echo "public API profile inventory is empty" >&2
  exit 1
fi

LC_ALL=C sort "$tmp/profile-keys" -o "$tmp/profile-keys"
if [[ -n "$(uniq -d "$tmp/profile-keys")" ]]; then
  echo "public API package/profile pairs must be unique" >&2
  exit 1
fi

for index in "${!entry_packages[@]}"; do
  if [[ "${entry_profiles[$index]}" == "default" \
    || "${entry_profiles[$index]}" == "proc-macro" ]]; then
    printf '%s\n' "${entry_packages[$index]}" >>"$tmp/primary-packages-raw"
  fi
  printf '%s\n' "${entry_files[$index]}" >>"$tmp/expected-files"
done

LC_ALL=C sort "$tmp/primary-packages-raw" >"$tmp/primary-packages"
LC_ALL=C sort "$tmp/expected-files" -o "$tmp/expected-files"

if [[ -n "$(uniq -d "$tmp/primary-packages")" ]] \
  || ! diff -u "$tmp/expected-packages" "$tmp/primary-packages"; then
  echo "every publishable library needs exactly one default or proc-macro public API profile" >&2
  exit 1
fi

if [[ "$(wc -l <"$tmp/expected-files" | tr -d ' ')" -ne "${#entry_files[@]}" ]] \
  || [[ "$(uniq -d "$tmp/expected-files" | wc -l | tr -d ' ')" -ne 0 ]]; then
  echo "public API baseline filenames must be unique" >&2
  exit 1
fi

if [[ "$mode" == "bless" ]]; then
  mkdir -p "$baseline_dir"
else
  find "$baseline_dir" -maxdepth 1 -type f -name '*.txt' -exec basename {} \; \
    | LC_ALL=C sort >"$tmp/actual-files"
  if ! diff -u "$tmp/expected-files" "$tmp/actual-files"; then
    echo "public API baseline files do not match profiles.tsv" >&2
    exit 1
  fi
fi

failed=0
for index in "${!entry_packages[@]}"; do
  package="${entry_packages[$index]}"
  profile="${entry_profiles[$index]}"
  file="${entry_files[$index]}"
  candidate="$tmp/$file"
  echo "Checking $package public API ($profile)" >&2
  if [[ "$profile" == "proc-macro" ]]; then
    crate_name="$(
      cargo metadata --no-deps --format-version 1 \
        | jq -r --arg package "$package" '
            .packages[]
            | select(.name == $package)
            | .targets[]
            | select(.kind | index("proc-macro"))
            | .name
          '
    )"
    if [[ -z "$crate_name" || "$crate_name" == *$'\n'* ]]; then
      echo "$package must contain exactly one proc-macro target" >&2
      exit 1
    fi

    profile_target_dir="$tmp/proc-macro-$index"
    CARGO_TARGET_DIR="$profile_target_dir" \
      cargo "+$nightly" rustdoc -p "$package" --target "$target" -- \
        -Z unstable-options --output-format json
    rustdoc_json="$(find "$profile_target_dir" -type f -name "$crate_name.json" -print)"
    if [[ -z "$rustdoc_json" || "$rustdoc_json" == *$'\n'* ]]; then
      echo "expected exactly one rustdoc JSON file for $package" >&2
      exit 1
    fi
    jq -r --arg crate "$crate_name" '
      .index[]
      | select(.visibility == "public" and .inner.proc_macro != null)
      | .inner.proc_macro as $macro
      | "proc-macro \($macro.kind) \($crate)::\(.name)"
        + if ($macro.helpers | length) > 0 then
            " helpers=[" + ($macro.helpers | sort | join(",")) + "]"
          else
            ""
          end
    ' "$rustdoc_json" | LC_ALL=C sort -u >"$candidate"
    if [[ ! -s "$candidate" ]]; then
      echo "$package exposes no public procedural macros" >&2
      exit 1
    fi
  else
    args=(
      "+$nightly"
      public-api
      -p "$package"
      --target "$target"
      -s
      --color never
    )
    if [[ "$profile" == "all-features" ]]; then
      args+=(--all-features)
    fi
    cargo "${args[@]}" >"$candidate"
  fi

  if [[ "$mode" == "bless" ]]; then
    cp "$candidate" "$baseline_dir/$file"
    continue
  fi

  baseline="$baseline_dir/$file"
  LC_ALL=C sort -u "$baseline" >"$tmp/baseline-sorted"
  LC_ALL=C sort -u "$candidate" >"$tmp/candidate-sorted"
  comm -23 "$tmp/baseline-sorted" "$tmp/candidate-sorted" >"$tmp/missing"
  comm -13 "$tmp/baseline-sorted" "$tmp/candidate-sorted" >"$tmp/added"

  if [[ -s "$tmp/missing" ]]; then
    echo "::error title=Public API regression ($package, $profile)::Baseline items were removed or changed" >&2
    echo "$package ($profile) removed or changed public API items:" >&2
    sed 's/^/  - /' "$tmp/missing" >&2
    if [[ -s "$tmp/added" ]]; then
      echo "$package ($profile) candidate-only items, including possible replacements:" >&2
      sed 's/^/  + /' "$tmp/added" >&2
    fi
    failed=1
  elif [[ -s "$tmp/added" ]]; then
    count="$(wc -l <"$tmp/added" | tr -d ' ')"
    echo "$package ($profile) adds $count public API item(s); additions are allowed" >&2
  fi
done

find "$baseline_dir" -maxdepth 1 -type f -name '*.txt' -exec basename {} \; \
  | LC_ALL=C sort >"$tmp/actual-files"
if ! diff -u "$tmp/expected-files" "$tmp/actual-files"; then
  echo "public API baseline files do not match profiles.tsv" >&2
  exit 1
fi

if [[ "$failed" -ne 0 ]]; then
  exit 1
fi
