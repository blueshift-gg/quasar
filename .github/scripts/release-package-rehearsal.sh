#!/usr/bin/env bash
set -euo pipefail

readonly rehearsal_root="/rehearsal/projects"
readonly package_root="/opt/quasar-release-rehearsal"
readonly source_fingerprint_before="/tmp/quasar-package-sources.before"
readonly source_fingerprint_after="/tmp/quasar-package-sources.after"

fail() {
  echo "release package rehearsal: $*" >&2
  exit 1
}

assert_no_credentials() {
  local variable
  for variable in CARGO_REGISTRY_TOKEN CARGO_TOKEN GH_TOKEN GITHUB_TOKEN; do
    if [[ -n "${!variable:-}" ]]; then
      fail "$variable must not be present"
    fi
  done
  [[ ! -e "$CARGO_HOME/credentials" ]] || fail "Cargo credentials file is present"
  [[ ! -e "$CARGO_HOME/credentials.toml" ]] || fail "Cargo credentials file is present"
}

fingerprint_package_sources() {
  local output="$1"
  find "$package_root" -type f -exec sha256sum {} + | LC_ALL=C sort -k2 >"$output"
}

wait_for_profiler() {
  local attempt
  for attempt in {1..100}; do
    if curl --fail --silent http://127.0.0.1:7777/ \
      >/tmp/quasar-profiler-index.html 2>/dev/null; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

stop_background_profiler() {
  local pids
  pids="$(pgrep -x quasar || true)"
  [[ -n "$pids" ]] || fail "background profiler process was not found"
  # The container runs no other Quasar process after the foreground command exits.
  kill $pids

  local attempt
  for attempt in {1..100}; do
    if ! curl --fail --silent http://127.0.0.1:7777/ >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.1
  done
  fail "background profiler did not stop"
}

verify_starter() {
  local template="$1"
  local name="quasar-release-$template"
  local project="$rehearsal_root/$name"
  local manifest_snapshot="/tmp/$name.Cargo.toml"

  cd "$rehearsal_root"
  quasar init "$name" \
    --yes \
    --no-git \
    --test-language rust \
    --rust-framework quasar-svm \
    --template "$template" \
    --toolchain solana

  cd "$project"
  cp Cargo.toml "$manifest_snapshot"
  grep -Fx 'quasar-lang = "=0.1.0"' Cargo.toml >/dev/null \
    || fail "$template starter does not use the packaged quasar-lang version"
  if grep -Eq 'quasar-lang = .*\b(path|git|branch)\b' Cargo.toml; then
    fail "$template starter contains a source override"
  fi

  quasar lint --strict --no-diff
  quasar build
  quasar test --no-build
  cmp "$manifest_snapshot" Cargo.toml \
    || fail "$template starter manifest changed during lint/build/test"
}

[[ ! -e /workspace/quasar ]] || fail "source checkout is present in the runtime image"
[[ "$(find "$package_root/archives" -type f -name '*.crate' | wc -l | tr -d ' ')" -eq 10 ]] \
  || fail "expected ten package archives"
[[ "$(find "$package_root/packages" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" -eq 10 ]] \
  || fail "expected ten unpacked packages"
if find "$package_root" -perm -u=w -print -quit | grep -q .; then
  fail "packaged sources are writable"
fi
if find "$CARGO_HOME" -mindepth 1 -print -quit | grep -q .; then
  fail "runtime Cargo cache is not empty"
fi
assert_no_credentials
fingerprint_package_sources "$source_fingerprint_before"

rm -rf "$rehearsal_root"/*
verify_starter minimal
verify_starter full

cd "$rehearsal_root/quasar-release-minimal"
readonly manifest_snapshot="/tmp/quasar-release-minimal.Cargo.toml"

# Build one real debug profile, verify its detached server, then create a second
# snapshot from the emitted ELF so the blocking diff view has real inputs.
quasar profile
wait_for_profiler || fail "profiler server did not become ready"
curl --fail --silent --show-error http://127.0.0.1:7777/profiles \
  >/tmp/quasar-profiler-profiles.json
stop_background_profiler

profile_elf="$(find target/profile -maxdepth 1 -type f -name '*.so' -print -quit)"
[[ -n "$profile_elf" ]] || fail "profile build did not emit an ELF"
quasar profile "$profile_elf"
wait_for_profiler || fail "second profiler server did not become ready"
stop_background_profiler

mapfile -t profiles < <(find target/profile/profiles -type f -name '*.profile.json' | LC_ALL=C sort)
[[ "${#profiles[@]}" -eq 2 ]] || fail "expected two real profile snapshots"
program="$(basename "${profiles[0]}")"
program="${program%%__*}"

set +e
timeout 5s quasar profile --diff "$program" >/tmp/quasar-profiler-diff.log 2>&1 &
diff_pid=$!
set -e
wait_for_profiler || fail "profiler diff server did not become ready"
curl --fail --silent --show-error \
  "http://127.0.0.1:7777/?program=$program&view=diff" \
  >/tmp/quasar-profiler-diff.html
set +e
wait "$diff_pid"
diff_status=$?
set -e
[[ "$diff_status" -eq 124 ]] || fail "profiler diff did not remain active until timeout"
grep -F "?program=$program&view=diff" /tmp/quasar-profiler-diff.log >/dev/null \
  || fail "profiler diff did not select the generated program"

quasar clean
for removed in target/profile target/idl target/client; do
  [[ ! -e "$removed" ]] || fail "quasar clean left $removed behind"
done
if find target/deploy -mindepth 1 ! -name '*-keypair.json' -print -quit | grep -q .; then
  fail "quasar clean left non-keypair deploy artifacts behind"
fi
cmp "$manifest_snapshot" Cargo.toml \
  || fail "minimal starter manifest changed during profile/clean"

fingerprint_package_sources "$source_fingerprint_after"
cmp "$source_fingerprint_before" "$source_fingerprint_after" \
  || fail "read-only packaged sources changed during the rehearsal"
assert_no_credentials

echo "release package rehearsal passed"
