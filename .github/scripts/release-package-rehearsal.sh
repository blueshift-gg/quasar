#!/usr/bin/env bash
set -euo pipefail

readonly rehearsal_root="/rehearsal/projects"
readonly package_root="/opt/quasar-release-rehearsal"
readonly conformance_root="/opt/quasar-client-conformance"
readonly project="$rehearsal_root/canonical"
readonly source_fingerprint_before="/tmp/quasar-package-sources.before"
readonly source_fingerprint_after="/tmp/quasar-package-sources.after"

fail() {
  echo "release package rehearsal: $*" >&2
  exit 1
}

assert_no_credentials() {
  local variable
  for variable in CARGO_REGISTRY_TOKEN CARGO_TOKEN GH_TOKEN GITHUB_TOKEN; do
    [[ -z "${!variable:-}" ]] || fail "$variable must not be present"
  done
  [[ ! -e "$CARGO_HOME/credentials" ]] || fail "Cargo credentials file is present"
  [[ ! -e "$CARGO_HOME/credentials.toml" ]] || fail "Cargo credentials file is present"
}

fingerprint_package_sources() {
  find "$package_root" -type f -exec sha256sum {} + | LC_ALL=C sort -k2
}

wait_for_validator() {
  local attempt
  for attempt in {1..120}; do
    if solana cluster-version --url http://127.0.0.1:8899 >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  fail "local validator did not become ready"
}

assert_exact_config() {
  cat >/tmp/expected-Quasar.toml <<'EOF'
[project]
name = "canonical"

[testing]
command = { program = "cargo", args = ["test", "tests::"] }

[clients]
path = "target/client"
targets = ["rust", "kit", "web3"]
EOF
  cmp /tmp/expected-Quasar.toml Quasar.toml \
    || fail "starter Quasar.toml does not match the canonical typed schema"
}

typecheck_typescript_client() {
  local clients_root="$1"
  local target="$2"
  local expected_package="$3"
  local expected_range="$4"
  local expected_major="$5"
  local contract="$6"
  local client_dir
  client_dir="$(find "$clients_root/$target" -mindepth 1 -maxdepth 1 -type d -print -quit)"
  [[ -n "$client_dir" ]] || fail "$target client was not generated"
  jq -e --arg package "$expected_package" --arg range "$expected_range" \
    '.dependencies[$package] == $range' "$client_dir/package.json" >/dev/null \
    || fail "$target manifest does not require $expected_package $expected_range"

  (
    cd "$client_dir"
    cp "$contract" conformance.ts
    npm install --ignore-scripts --no-audit --no-fund
    npm install --ignore-scripts --no-audit --no-fund --no-save \
      typescript@5.9.3 tsx@4.20.6
    npx tsc --noEmit --target ES2022 --module NodeNext \
      --moduleResolution NodeNext --skipLibCheck --strict client.ts conformance.ts
    npx tsx conformance.ts
    resolved="$(node -p "require('./node_modules/$expected_package/package.json').version")"
    [[ "$resolved" != *-* ]] || fail "$target resolved a prerelease dependency: $resolved"
    case "$resolved" in
      "$expected_major".*) ;;
      *) fail "$target resolved unexpected $expected_package version: $resolved" ;;
    esac
  )
}

[[ ! -e /workspace/quasar ]] || fail "source checkout is present in the runtime image"
expected_packages="$(jq -r '.packages | length' "$package_root/manifest.json")"
[[ "$expected_packages" -gt 0 ]] || fail "release manifest is empty"
[[ "$(find "$package_root/archives" -type f -name '*.crate' | wc -l | tr -d ' ')" -eq "$expected_packages" ]] \
  || fail "archive inventory does not match the release manifest"
[[ "$(find "$package_root/packages" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" -eq "$expected_packages" ]] \
  || fail "unpacked inventory does not match the release manifest"
if find "$package_root" -perm -u=w -print -quit | grep -q .; then
  fail "packaged sources are writable"
fi
if find "$CARGO_HOME" -mindepth 1 -print -quit | grep -q .; then
  fail "runtime Cargo cache is not empty"
fi
assert_no_credentials
fingerprint_package_sources >"$source_fingerprint_before"

rm -rf "$rehearsal_root"/*
while IFS=$'\t' read -r package version archive; do
  [[ "$package" != "package" ]] || continue
  package_dir="$package_root/packages/$package-$version"
  CARGO_TARGET_DIR="$rehearsal_root/archive-check-target" \
    cargo check --manifest-path "$package_dir/Cargo.toml" \
      --locked --all-features --all-targets \
    || fail "packaged crate did not compile: $archive"
done <"$package_root/packages.tsv"
rm -rf "$rehearsal_root/archive-check-target"

cd "$rehearsal_root"
quasar init canonical --no-git
cd "$project"
cp Cargo.toml /tmp/canonical.Cargo.toml
assert_exact_config
test -f src/lib.rs
test -f src/tests.rs
test ! -e package.json
grep -Fx 'quasar-lang = "=0.1.0"' Cargo.toml >/dev/null
grep -Fx 'quasar-test = "=0.1.0"' Cargo.toml >/dev/null
if grep -Eq '\b(path|git|branch)\b' Cargo.toml; then
  fail "starter manifest contains a source override"
fi

quasar lint --strict
quasar build --debug
quasar test --no-build

idl="$(find target/idl -maxdepth 1 -type f -name '*.json' -print -quit)"
[[ -n "$idl" ]] || fail "build did not generate an IDL"
quasar idl verify "$idl"
quasar client "$idl"
test -d target/client/rust
test -d target/client/kit
test -d target/client/web3
test ! -d target/client/python
test ! -d target/client/go
test ! -d target/client/c

find target/client -type f -exec sha256sum {} + | LC_ALL=C sort -k2 \
  >/tmp/generated-clients.before
quasar idl .
quasar client "$idl"
find target/client -type f -exec sha256sum {} + | LC_ALL=C sort -k2 \
  >/tmp/generated-clients.after
cmp /tmp/generated-clients.before /tmp/generated-clients.after \
  || fail "repeated client generation was not byte-identical"

rust_client="$(find target/client/rust -name Cargo.toml -print -quit)"
[[ -n "$rust_client" ]] || fail "Rust client manifest was not generated"
cargo check --manifest-path "$rust_client" --locked

conformance_project="$rehearsal_root/client-conformance"
mkdir -p "$conformance_project"
cat >"$conformance_project/Quasar.toml" <<'EOF'
[project]
name = "client-conformance"

[clients]
path = "target/client"
EOF
(
  cd "$conformance_project"
  quasar client "$conformance_root/program.idl.json"
)
conformance_clients="$conformance_project/target/client"

npm view '@solana/kit@7.0.0' version | grep -Fx '7.0.0' >/dev/null
typecheck_typescript_client \
  "$conformance_clients" kit '@solana/kit' '^7.0.0' 7 "$conformance_root/kit.ts"
if npm view '@solana/web3.js@3.0.0' version 2>/dev/null | grep -Fx '3.0.0' >/dev/null; then
  typecheck_typescript_client \
    "$conformance_clients" web3 '@solana/web3.js' '^3.0.0' 3 "$conformance_root/web3.ts"
elif [[ "${ALLOW_MISSING_FINAL_WEB3:-0}" == "1" ]]; then
  echo "final @solana/web3.js@3.0.0 is unavailable; Web3 installation skipped for PR review" >&2
else
  fail "final @solana/web3.js@3.0.0 is not available; the release tag is blocked"
fi

quasar profile --json >/tmp/profile.json
jq -e '.version == 1 and .measurement.program == "canonical"' /tmp/profile.json >/dev/null
quasar profile --write-budget --headroom 5 --json >/tmp/profile-budget-write.json
quasar profile --assert-budget --json >/tmp/profile-budget-assert.json
jq -e '.budget.status == "passed"' /tmp/profile-budget-assert.json >/dev/null

solana-keygen new --no-bip39-passphrase --silent --force --outfile /tmp/payer.json
solana-keygen new --no-bip39-passphrase --silent --force --outfile /tmp/wrong-authority.json
solana-test-validator --reset --quiet >/tmp/validator.log 2>&1 &
validator_pid=$!
trap 'kill "$validator_pid" >/dev/null 2>&1 || true' EXIT
wait_for_validator
solana airdrop 100 --url http://127.0.0.1:8899 --keypair /tmp/payer.json >/dev/null

quasar deploy --skip-build \
  --url http://127.0.0.1:8899 \
  --keypair /tmp/payer.json \
  --upgrade-authority /tmp/payer.json
quasar verify --url http://127.0.0.1:8899 --upgrade-authority /tmp/payer.json

elf="$(find target/deploy -maxdepth 1 -type f -name '*.so' -print -quit)"
cp "$elf" /tmp/mismatched.so
printf '\0' >>/tmp/mismatched.so
if quasar verify --url http://127.0.0.1:8899 \
  --elf-path /tmp/mismatched.so >/tmp/mismatched-elf.log 2>&1; then
  fail "verification accepted a mismatched ELF"
fi
grep -F 'deployed ELF mismatch' /tmp/mismatched-elf.log >/dev/null
if quasar verify --url http://127.0.0.1:8899 \
  --upgrade-authority /tmp/wrong-authority.json >/tmp/mismatched-authority.log 2>&1; then
  fail "verification accepted a mismatched authority"
fi
grep -F 'upgrade authority mismatch' /tmp/mismatched-authority.log >/dev/null

quasar clean
test ! -e target/idl
test ! -e target/client
test ! -e target/profile
cmp /tmp/canonical.Cargo.toml Cargo.toml \
  || fail "starter manifest changed during the journey"

fingerprint_package_sources >"$source_fingerprint_after"
cmp "$source_fingerprint_before" "$source_fingerprint_after" \
  || fail "read-only packaged sources changed during the rehearsal"
assert_no_credentials

echo "release package rehearsal passed"
