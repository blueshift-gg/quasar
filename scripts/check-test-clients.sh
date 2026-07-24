#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo run --quiet --locked --manifest-path "$root/Cargo.toml" \
  -p quasar-cli -- idl "$root/examples/vault"
(
  cd "$root/examples/vault"
  "$root/target/debug/quasar" client "$root/target/idl/quasar_vault.json"
)

# The vault example's generated Kit/Web3.js clients are a drift fixture for
# quasar's own IDL->TS codegen (`quasar client`). Parallax keeps a synced copy
# under its typescript/tests/fixtures for its harness parity tests; when this
# output changes intentionally, re-sync that copy there.
generated="tests/fixtures/vault/clients"
if test -n "$(git -C "$root" status --porcelain --untracked-files=all -- "$generated")"; then
  git -C "$root" diff -- "$generated"
  git -C "$root" status --short --untracked-files=all -- "$generated"
  echo "generated test clients are stale; run make check-test-clients and commit the result" >&2
  exit 1
fi
