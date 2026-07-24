#!/usr/bin/env bash
set -euo pipefail

readonly package_name="quasar-test"
invocation_dir="$(pwd -P)"
program_artifact="${1:?compiled fixture program path is required}"
if [[ "$program_artifact" != /* ]]; then
  program_artifact="$invocation_dir/$program_artifact"
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

fail() {
  echo "quasar-test standalone: $*" >&2
  exit 1
}

if [[ ! -f "$program_artifact" ]]; then
  fail "fixture program does not exist: $program_artifact (run `make build-sbf` first)"
fi
program_artifact="$(cd "$(dirname "$program_artifact")" && pwd -P)/$(basename "$program_artifact")"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/quasar-test-standalone.XXXXXX")"
tmp="$(cd "$tmp" && pwd -P)"
trap 'rm -rf "$tmp"' EXIT

package_dir="$repo_root/test"
mkdir -p "$tmp/consumer/src"

cat >"$tmp/consumer/Cargo.toml" <<EOF
[package]
name = "quasar-test-external-consumer"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
qt = { package = "quasar-test", path = "$package_dir" }

# TEMPORARY: mirrors the workspace zeropod patch until zeropod >=0.3.4
# (solana-address <3, wincode 0.5) is published.
[patch.crates-io]
zeropod = { path = "$repo_root/vendor/zeropod/zeropod" }
zeropod-derive = { path = "$repo_root/vendor/zeropod/zeropod-derive" }
EOF

cat >"$tmp/consumer/src/lib.rs" <<'EOF'
#[cfg(test)]
mod tests {
    use {qt::{prelude::*, PROGRAM_PATH_ENV}, std::{env, fs, str::FromStr}};

    const PROGRAM_ID: &str = "33333333333333333333333333333333333333333333";
    const USER_LAMPORTS: u64 = 10_000_000_000;
    const DEPOSIT: u64 = 1_000_000_000;

    #[derive(Clone, Copy)]
    #[repr(u32)]
    enum VaultError {
        InvalidPda = 3002,
    }

    impl From<VaultError> for u32 {
        fn from(error: VaultError) -> Self {
            error as u32
        }
    }

    fn deposit_instruction(program_id: Pubkey, user: Pubkey, vault: Pubkey) -> Instruction {
        let mut data = vec![0];
        data.extend_from_slice(&DEPOSIT.to_le_bytes());
        Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(user, true),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data,
        }
    }

    #[quasar_test(program_id = Pubkey::from_str(PROGRAM_ID).expect("valid fixture program id"))]
    fn external_consumer_executes_a_real_program(q: &mut Test) -> Result<(), Box<dyn std::error::Error>> {
        let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
        let user = Pubkey::new_from_array([1; 32]);
        let (vault, _) = Pubkey::find_program_address(&[b"vault", user.as_ref()], &program_id);
        q.add(Wallet::new().at(user).fund(USER_LAMPORTS));

        if let Some(expected) = env::var_os(PROGRAM_PATH_ENV) {
            assert_eq!(
                fs::canonicalize(q.program_path()).expect("canonical loaded artifact"),
                fs::canonicalize(expected).expect("canonical configured artifact"),
            );
        }

        q.send(deposit_instruction(program_id, user, vault))
            .succeeds()
            .cu_at_most(5_000)
            .has_lamports(vault, DEPOSIT)
            .has_lamports(user, USER_LAMPORTS - DEPOSIT);

        let wrong_vault = Pubkey::new_unique();
        q.send(deposit_instruction(program_id, user, wrong_vault))
            .fails_with(VaultError::InvalidPda);
        assert!(q.account(wrong_vault).is_none(), "failed init left a placeholder account");
        Ok(())
    }
}
EOF

# Fresh standalone resolution drifts onto the wincode-0.6 line as new
# solana-* patch releases land; pin the proven single-wincode-0.5 set
# (mirrors the workspace lock and litesvm's own lockfile).
cargo generate-lockfile --manifest-path "$tmp/consumer/Cargo.toml"
for pin in solana-address@2.6.1 solana-hash@4.5.0 solana-message@4.3.0 \
           solana-pubkey@4.2.0 solana-instruction@3.4.0 solana-account@4.3.1 \
           solana-short-vec@3.2.2 solana-clock@3.1.1 \
           solana-last-restart-slot@3.1.0 solana-slot-history@3.1.0; do
  crate="${pin%@*}"; version="${pin#*@}"
  cargo update --manifest-path "$tmp/consumer/Cargo.toml" \
    -p "$crate" --precise "$version" 2>/dev/null || true
done

metadata="$tmp/consumer-metadata.json"
cargo metadata \
  --manifest-path "$tmp/consumer/Cargo.toml" \
  --format-version 1 \
  >"$metadata"
actual_manifest="$(jq -r --arg name "$package_name" \
  '.packages[] | select(.name == $name) | .manifest_path' "$metadata")"
[[ "$actual_manifest" == "$package_dir/Cargo.toml" ]] \
  || fail "consumer did not resolve quasar-test from the requested source"
if jq -e '.packages[] | select(.name | test("^quasar-(cli|idl|spl)$"))' \
  "$metadata" >/dev/null; then
  fail "quasar-test pulled an unrelated product into its dependency graph"
fi

export CARGO_TARGET_DIR="$tmp/consumer/target"
cd "$tmp/consumer"

QUASAR_PROGRAM_PATH="$program_artifact" cargo test --locked

mkdir -p target/deploy
cp "$program_artifact" target/deploy/quasar_vault.so
env -u QUASAR_PROGRAM_PATH cargo test --locked

cp "$program_artifact" target/deploy/decoy.so
if env -u QUASAR_PROGRAM_PATH cargo test --locked \
  >"$tmp/ambiguous.log" 2>&1; then
  fail "direct discovery accepted an ambiguous target/deploy directory"
fi
grep -F "found multiple program artifacts" "$tmp/ambiguous.log" >/dev/null \
  || fail "ambiguous discovery did not return its actionable diagnostic"
rm target/deploy/decoy.so

if QUASAR_PROGRAM_PATH="$tmp/missing.so" cargo test --locked \
  >"$tmp/missing.log" 2>&1; then
  fail "configured discovery accepted a missing artifact"
fi
grep -F "QUASAR_PROGRAM_PATH points to missing program artifact" "$tmp/missing.log" >/dev/null \
  || fail "missing configured artifact did not return its actionable diagnostic"

echo "quasar-test standalone harness passed"
