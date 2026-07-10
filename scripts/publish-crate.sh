#!/usr/bin/env bash
set -euo pipefail

crate="${1:?crate name is required}"
version="${2:?crate version is required}"

local_version="$(
  cargo metadata --no-deps --format-version 1 \
    | jq -er --arg crate "${crate}" '.packages[] | select(.name == $crate) | .version'
)"
if [[ "${local_version}" != "${version}" ]]; then
  echo "requested ${crate}@${version}, but the workspace contains ${crate}@${local_version}" >&2
  exit 1
fi

if ! cargo info --registry crates-io "${crate}@${version}" >/dev/null 2>&1; then
  cargo publish -p "${crate}" --locked
  exit 0
fi

# A tag workflow may be rerun after publishing only part of the dependency
# graph. Skip an existing crate only when it is byte-for-byte the package this
# checkout would publish; a real version collision must stop the release.
cargo package -p "${crate}" --locked --no-verify
archive="target/package/${crate}-${version}.crate"
if [[ ! -f "${archive}" ]]; then
  echo "cargo package did not produce ${archive}" >&2
  exit 1
fi

published_checksum="$(
  curl --proto '=https' --tlsv1.2 -fsSL \
    -A 'quasar-release-check/1.0' \
    "https://crates.io/api/v1/crates/${crate}/${version}" \
    | jq -er '.version.checksum'
)"
local_checksum="$(sha256sum "${archive}" | awk '{print $1}')"

if [[ "${local_checksum}" != "${published_checksum}" ]]; then
  echo "${crate}@${version} already exists with different package contents" >&2
  echo "local:     ${local_checksum}" >&2
  echo "published: ${published_checksum}" >&2
  exit 1
fi

echo "${crate}@${version} is already published with matching contents; skipping"
