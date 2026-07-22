#!/usr/bin/env bash
set -euo pipefail

package_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
consumer_dir="$(mktemp -d "${TMPDIR:-/tmp}/quasar-test-consumer.XXXXXX")"
trap 'rm -rf "$consumer_dir"' EXIT

npm pack \
  --prefix "$package_root" \
  --ignore-scripts \
  --pack-destination "$consumer_dir" \
  >/dev/null

mkdir -p "$consumer_dir/consumer" "$consumer_dir/fixtures/vault"
cp -R "$package_root/tests/consumer/." "$consumer_dir/consumer/"
cp -R "$package_root/tests/fixtures/vault/clients" "$consumer_dir/fixtures/vault/"

runtime_source="$(
  node -p \
    "require('$package_root/package.json').devDependencies['@blueshift-gg/quasar-svm']"
)"
web3_version="$(
  node -p \
    "require('$package_root/package.json').devDependencies['@solana/web3.js']"
)"
typescript_version="$(
  node -p \
    "require('$package_root/package.json').devDependencies.typescript"
)"
node_types_version="$(
  node -p \
    "require('$package_root/package.json').devDependencies['@types/node']"
)"
package_tarball="$(find "$consumer_dir" -maxdepth 1 -name '*.tgz' -print -quit)"
test -n "$package_tarball"

npm install \
  --prefix "$consumer_dir" \
  --ignore-scripts \
  --legacy-peer-deps \
  --no-save \
  "$package_tarball" \
  "$runtime_source" \
  "@solana/web3.js@$web3_version" \
  "@types/node@$node_types_version" \
  "typescript@$typescript_version" \
  >/dev/null

npm exec --prefix "$consumer_dir" -- tsc --noEmit -p "$consumer_dir/consumer/tsconfig.json"
(
  cd "$consumer_dir"
  node --input-type=module -e \
    'import.meta.resolve("@blueshift-gg/quasar-test/kit"); import.meta.resolve("@blueshift-gg/quasar-test/web3.js");'
)
