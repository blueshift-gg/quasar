#!/bin/sh
# Builds the programs the benchmark and the oracle tests compare against into
# target/rivals, from pinned revisions.
set -eu

ANCHOR_REPO=https://github.com/otter-sec/anchor
ANCHOR_REV=dbb5bae417bfb7dd2ba67c1d1f4091c5216d7f4c
BLUESHIFT_REPO=https://github.com/blueshift-gg/quasar
BLUESHIFT_REV=b0de7db4cd271654a2dcf78807dd865e98e0b339
# p-token, which replaces the Token program (SIMD-0266).
TOKEN_REPO=https://github.com/solana-program/token
TOKEN_REV=8185db13640f0df038266c7cf4306c212d81380a

root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/target/rivals"
mkdir -p "$out/src"

checkout() {
    dir="$out/src/$1"
    [ -d "$dir" ] || git clone --quiet "$2" "$dir"
    git -C "$dir" checkout --quiet "$3"
}

build() {
    rm -f "$out/$2.so"
    # Make supplies SBF_TOOLS as a list of command-line arguments.
    # shellcheck disable=SC2086
    cargo build-sbf ${SBF_TOOLS:?run through make} --manifest-path "$1" --sbf-out-dir "$out" >"$out/$2.log" 2>&1 ||
        { cat "$out/$2.log" >&2; exit 1; }
}

checkout anchor-next "$ANCHOR_REPO" "$ANCHOR_REV"
checkout blueshift "$BLUESHIFT_REPO" "$BLUESHIFT_REV"
checkout token "$TOKEN_REPO" "$TOKEN_REV"
build "$root/bench/rivals/pinocchio-vault/Cargo.toml" pinocchio_vault
build "$root/bench/rivals/anchor-next-vault/Cargo.toml" anchor_next_vault
build "$root/bench/rivals/blueshift-vault/Cargo.toml" blueshift_vault
build "$out/src/blueshift/examples/vault/Cargo.toml" quasar_vault
build "$out/src/blueshift/examples/escrow/Cargo.toml" quasar_escrow
# shortcut: this pinned multisig cannot link SBPFv3; remove after updating its revision.
# shellcheck disable=SC2086
multisig_tools=$(printf '%s ' $SBF_TOOLS | sed -E 's/(^|[[:space:]])--arch(=|[[:space:]]+)[^[:space:]]+//g')
(SBF_TOOLS="$multisig_tools --arch v0" \
    build "$out/src/blueshift/examples/multisig/Cargo.toml" quasar_multisig)
build "$out/src/token/pinocchio/program/Cargo.toml" pinocchio_token_program
