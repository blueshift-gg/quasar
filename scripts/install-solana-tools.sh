#!/usr/bin/env bash
set -euo pipefail

version="${1:?Solana release version is required}"
expected_sha256="${2:?Solana release SHA-256 is required}"
install_root="${3:-${HOME}/.local/share/solana/install}"
target="x86_64-unknown-linux-gnu"

if [[ ! "${version}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid Solana release version: ${version}" >&2
  exit 1
fi
if [[ ! "${expected_sha256}" =~ ^[0-9a-f]{64}$ ]]; then
  echo "invalid Solana release SHA-256: ${expected_sha256}" >&2
  exit 1
fi

temporary_directory="$(mktemp -d)"
trap 'rm -rf "${temporary_directory}"' EXIT

cache_root="${XDG_CACHE_HOME:-${HOME}/.cache}/quasar/solana"
archive="${cache_root}/${version}-${target}.tar.bz2"
checksum_file="${temporary_directory}/SHA256SUM"
url="https://github.com/anza-xyz/agave/releases/download/${version}/solana-release-${target}.tar.bz2"
mkdir -p "${cache_root}"
if [[ ! -f "${archive}" ]]; then
  download="$(mktemp "${cache_root}/.download.XXXXXX")"
  trap 'rm -rf "${temporary_directory}"; rm -f "${download:-}"' EXIT
  curl --proto '=https' --tlsv1.2 --location --fail --silent --show-error \
    --output "${download}" "${url}"
  printf '%s  %s\n' "${expected_sha256}" "${download}" > "${checksum_file}"
  sha256sum -c "${checksum_file}"
  mv "${download}" "${archive}"
fi
printf '%s  %s\n' "${expected_sha256}" "${archive}" > "${checksum_file}"
sha256sum -c "${checksum_file}"

tar --extract --bzip2 --file "${archive}" --directory "${temporary_directory}"
extracted="${temporary_directory}/solana-release"
if [[ ! -x "${extracted}/bin/cargo-build-sbf" ]]; then
  echo "Solana release archive does not contain bin/cargo-build-sbf" >&2
  exit 1
fi

mkdir -p "${install_root}"
active_release="${install_root}/active_release"
rm -rf "${active_release}"
mv "${extracted}" "${active_release}"
"${active_release}/bin/solana" --version
"${active_release}/bin/cargo-build-sbf" --version
