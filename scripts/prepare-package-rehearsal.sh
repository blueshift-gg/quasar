#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

archive_dir="${1:?package archive directory is required}"
output_root="${2:?rehearsal output directory is required}"
shift 2
packages=("$@")

if [[ "${#packages[@]}" -ne 11 ]]; then
  echo "expected the eleven v0.1.0 release packages, got ${#packages[@]}" >&2
  exit 1
fi
if [[ ! -d "$archive_dir" ]]; then
  echo "package archive directory does not exist: $archive_dir" >&2
  exit 1
fi
if [[ -e "$output_root" ]]; then
  echo "rehearsal output already exists: $output_root" >&2
  exit 1
fi

mkdir -p "$output_root/archives" "$output_root/packages"
output_root="$(cd "$output_root" && pwd -P)"
archive_dir="$(cd "$archive_dir" && pwd -P)"
config="$output_root/cargo-config.toml"
inventory="$output_root/packages.tsv"

printf '[patch.crates-io]\n' >"$config"
printf 'package\tversion\tarchive\n' >"$inventory"

for package in "${packages[@]}"; do
  archives=()
  for candidate in "$archive_dir/$package-"*.crate; do
    suffix="$(basename "${candidate#"$archive_dir/$package-"}")"
    if [[ "$suffix" =~ ^[0-9] ]]; then
      archives+=("$candidate")
    fi
  done
  if [[ "${#archives[@]}" -ne 1 ]]; then
    echo "expected exactly one archive for $package under $archive_dir" >&2
    exit 1
  fi

  archive="${archives[0]}"
  archive_name="$(basename "$archive")"
  archive_root="$(tar -tzf "$archive" | awk -F/ 'NR == 1 { print $1 }')"
  if [[ "$archive_root" != "$package-"* ]]; then
    echo "$archive_name has unexpected archive root: $archive_root" >&2
    exit 1
  fi
  version="${archive_root#"$package-"}"
  if [[ -z "$version" ]]; then
    echo "$archive_name does not encode a package version" >&2
    exit 1
  fi

  cp "$archive" "$output_root/archives/$archive_name"
  tar -xzf "$archive" -C "$output_root/packages"
  package_dir="$output_root/packages/$archive_root"
  if [[ ! -f "$package_dir/Cargo.toml" || ! -f "$package_dir/Cargo.lock" ]]; then
    echo "$archive_name is missing its normalized manifest or lockfile" >&2
    exit 1
  fi

  printf '%s = { path = "%s" }\n' "$package" "$package_dir" >>"$config"
  printf '%s\t%s\t%s\n' "$package" "$version" "$archive_name" >>"$inventory"
done

archive_count="$(find "$output_root/archives" -type f -name '*.crate' | wc -l | tr -d ' ')"
package_count="$(find "$output_root/packages" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')"
if [[ "$archive_count" -ne 11 || "$package_count" -ne 11 ]]; then
  echo "rehearsal must contain exactly eleven archives and eleven unpacked packages" >&2
  exit 1
fi
