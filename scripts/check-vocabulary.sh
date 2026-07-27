#!/usr/bin/env bash
# Tripwire for vocabulary that died in an API round: stale grammar in docs or
# code is a bug. Extend the pattern list whenever a rename retires a name.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dead='has_lamports|has_tokens|has_supply|cu_at_most|owned_by|is_closed\(|hasLamports|hasTokens|hasSupply|cuAtMost|ownedBy|isClosed\(|CuBudget|Lamports::|Tokens::|Supply::|Owner::eq|Changes::|Changes\.|::state\(|with::<|Assert::|\.sendAll|sendWith|\.send\(|send_all|test\.send|simulate_all|simulateAll|succeeds\(|fails_with|fails\(|SucceededTransaction|FailedTransaction|&mut Test|Test::builder|TestBuilder'
if grep -rnE "$dead" "$root/test/README.md" "$root/test/src" 2>/dev/null; then
  echo "stale vocabulary found" >&2
  exit 1
fi
echo "vocabulary clean"
