#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo run --quiet --locked --manifest-path "$root/Cargo.toml" \
  -p quasar-cli -- idl "$root/examples/vault"
(
  cd "$root/examples/vault"
  "$root/target/debug/quasar" client "$root/target/idl/quasar_vault.json"
)

generated="test/typescript/tests/fixtures/vault/clients"
if test -n "$(git -C "$root" status --porcelain --untracked-files=all -- "$generated")"; then
  git -C "$root" diff -- "$generated"
  git -C "$root" status --short --untracked-files=all -- "$generated"
  echo "generated test clients are stale; run make check-test-clients and commit the result" >&2
  exit 1
fi
