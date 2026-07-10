#!/usr/bin/env bash
set -euo pipefail

crate="${1:?crate name is required}"
version="${2:?crate version is required}"
attempts="${3:-30}"

for ((attempt = 1; attempt <= attempts; attempt++)); do
  if cargo info --registry crates-io "${crate}@${version}" >/dev/null 2>&1; then
    echo "${crate}@${version} is available from crates.io"
    exit 0
  fi
  echo "Waiting for ${crate}@${version} (${attempt}/${attempts})"
  sleep 10
done

echo "Timed out waiting for ${crate}@${version} to reach crates.io" >&2
exit 1
