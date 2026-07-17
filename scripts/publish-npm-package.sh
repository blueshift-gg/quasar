#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <package-directory> <version>" >&2
  exit 2
fi

package_dir="$1"
expected_version="$2"
package_name="$(jq -r '.name' "$package_dir/package.json")"
package_version="$(jq -r '.version' "$package_dir/package.json")"

if [[ "$package_version" != "$expected_version" ]]; then
  echo "$package_name package version $package_version does not match $expected_version" >&2
  exit 1
fi

local_integrity="$(
  cd "$package_dir"
  npm pack --dry-run --json \
    | jq -r '.[0].integrity'
)"
published_integrity="$(
  npm view "$package_name@$expected_version" dist.integrity --json 2>/dev/null \
    | jq -r 'select(type == "string")' \
    || true
)"

if [[ -n "$published_integrity" ]]; then
  if [[ "$published_integrity" != "$local_integrity" ]]; then
    echo "$package_name@$expected_version is already published with different contents" >&2
    echo "local:     $local_integrity" >&2
    echo "published: $published_integrity" >&2
    exit 1
  fi
  echo "$package_name@$expected_version is already published with matching contents; skipping"
  exit 0
fi

(
  cd "$package_dir"
  npm publish --access public
)
