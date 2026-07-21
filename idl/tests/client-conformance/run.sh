#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)"
fixture="$repo_root/idl/tests/fixtures/programs/client-conformance.idl.json"
contracts="$repo_root/idl/tests/client-conformance"
project="$(mktemp -d "${TMPDIR:-/tmp}/quasar-client-contracts.XXXXXX")"
trap 'rm -rf "$project"' EXIT

cat >"$project/Quasar.toml" <<'EOF'
[project]
name = "client-conformance"

[clients]
path = "target/client"
EOF

generate() {
  local target="$1"
  (
    cd "$project"
    cargo run --quiet --manifest-path "$repo_root/cli/Cargo.toml" -- \
      client "$fixture" --target "$target"
  )
}

check_client() {
  local target="$1"
  local package="$2"
  local range="$3"
  local major="$4"
  local client
  client="$(find "$project/target/client/$target" -mindepth 1 -maxdepth 1 -type d -print -quit)"
  [[ -n "$client" ]] || { echo "$target client was not generated" >&2; exit 1; }

  jq -e --arg package "$package" --arg range "$range" \
    '.dependencies[$package] == $range' "$client/package.json" >/dev/null
  cp "$contracts/$target.ts" "$client/conformance.ts"
  (
    cd "$client"
    npm install --ignore-scripts --no-audit --no-fund
    npm install --ignore-scripts --no-audit --no-fund --no-save \
      typescript@5.9.3 tsx@4.20.6
    npx tsc --noEmit --target ES2022 --module NodeNext \
      --moduleResolution NodeNext --skipLibCheck --strict client.ts conformance.ts
    npx tsx conformance.ts
    resolved="$(node -p "require('./node_modules/$package/package.json').version")"
    [[ "$resolved" == "$major".* && "$resolved" != *-* ]] || {
      echo "$target resolved unexpected $package version: $resolved" >&2
      exit 1
    }
  )
}

generate kit
check_client kit '@solana/kit' '^7.0.0' 7

generate web3
if npm view '@solana/web3.js@3.0.0' version 2>/dev/null | grep -Fx '3.0.0' >/dev/null; then
  check_client web3 '@solana/web3.js' '^3.0.0' 3
else
  echo 'final @solana/web3.js@3.0.0 unavailable; generated manifest checked, runtime conformance deferred' >&2
fi
