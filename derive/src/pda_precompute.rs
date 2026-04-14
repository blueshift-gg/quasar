//! Compile-time PDA precomputation.
//!
//! When `#[account(seeds = [b"counter"], bump)]` has seeds that are ALL byte
//! literals, we run `derive_program_address` on the host during macro expansion
//! and emit the canonical bump + address as constants. The runtime then uses
//! `keys_eq` (~10 CU) instead of `verify_program_address` (~200 CU) or
//! `find_bump_for_address` (~300 CU).
//!
//! Program ID discovery reads `CARGO_MANIFEST_DIR/src/lib.rs` looking for a
//! `declare_id!("...")` invocation and base58-decodes the literal. The result
//! is cached in a thread-local keyed by `CARGO_MANIFEST_DIR`.
//!
//! This is a *soft* optimization: if any seed is non-literal, or the program
//! ID can't be discovered, we silently fall through to runtime paths.

use std::cell::RefCell;

thread_local! {
    /// Cache: `(manifest_dir, discovery_result)`.
    /// `None` outer = not yet attempted for this dir.
    static CACHED_PROGRAM_ID: RefCell<Option<(String, Option<[u8; 32]>)>> = const { RefCell::new(None) };
}

/// Discover the program ID declared in the current crate's `src/lib.rs`.
///
/// Returns `None` on any failure (file missing, parse error, no `declare_id!`,
/// bad base58). Cached per `CARGO_MANIFEST_DIR`.
pub(crate) fn discover_program_id() -> Option<[u8; 32]> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;

    CACHED_PROGRAM_ID.with(|cell| {
        let borrow = cell.borrow();
        if let Some((ref cached_dir, ref result)) = *borrow {
            if cached_dir == &manifest_dir {
                return *result;
            }
        }
        drop(borrow);

        let id = try_discover_program_id(&manifest_dir);
        *cell.borrow_mut() = Some((manifest_dir, id));
        id
    })
}

fn try_discover_program_id(manifest_dir: &str) -> Option<[u8; 32]> {
    let lib_rs = std::path::PathBuf::from(manifest_dir)
        .join("src")
        .join("lib.rs");
    let source = std::fs::read_to_string(&lib_rs).ok()?;
    let file = syn::parse_file(&source).ok()?;

    for item in &file.items {
        if let syn::Item::Macro(item_macro) = item {
            let last = item_macro.mac.path.segments.last()?;
            if last.ident != "declare_id" {
                continue;
            }
            let lit: syn::LitStr = syn::parse2(item_macro.mac.tokens.clone()).ok()?;
            let decoded = bs58::decode(lit.value()).into_vec().ok()?;
            if decoded.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&decoded);
                return Some(arr);
            }
        }
    }
    None
}

/// Check if every seed expression is a byte string literal (`b"..."`).
///
/// Returns `None` if any seed is not a literal (field reference, method call,
/// path expression, etc.), signalling that precomputation can't apply.
pub(crate) fn seeds_as_byte_literals(seeds: &[syn::Expr]) -> Option<Vec<Vec<u8>>> {
    seeds
        .iter()
        .map(|expr| match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::ByteStr(b),
                ..
            }) => Some(b.value()),
            _ => None,
        })
        .collect()
}

/// Host-side PDA derivation using `const_crypto`.
///
/// Returns `(bump, pda_address)`. Always succeeds — the probability of
/// all 256 bumps landing on-curve is cryptographically negligible (~2^-256).
pub(crate) fn precompute_pda(seeds: &[&[u8]], program_id: &[u8; 32]) -> (u8, [u8; 32]) {
    // const_crypto::ed25519::derive_program_address is a const fn that also
    // works at runtime. It iterates bumps 255→0 and returns the first
    // off-curve hash — identical to Solana's find_program_address.
    let (addr, bump) = const_crypto::ed25519::derive_program_address(seeds, program_id);
    (bump, addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_as_byte_literals_all_literals() {
        let seeds: Vec<syn::Expr> = vec![
            syn::parse_str(r#"b"foo""#).unwrap(),
            syn::parse_str(r#"b"bar""#).unwrap(),
        ];
        let result = seeds_as_byte_literals(&seeds).unwrap();
        assert_eq!(result, vec![b"foo".to_vec(), b"bar".to_vec()]);
    }

    #[test]
    fn seeds_as_byte_literals_rejects_non_literal() {
        let seeds: Vec<syn::Expr> = vec![
            syn::parse_str(r#"b"foo""#).unwrap(),
            syn::parse_str("wallet").unwrap(),
        ];
        assert!(seeds_as_byte_literals(&seeds).is_none());
    }

    #[test]
    fn precompute_pda_known_value() {
        // Use a known program ID and verify the result is deterministic.
        let program_id = [1u8; 32];
        let seeds: Vec<&[u8]> = vec![b"test"];
        let (bump, addr) = precompute_pda(&seeds, &program_id);
        // Re-derive to confirm determinism.
        let (bump2, addr2) = precompute_pda(&seeds, &program_id);
        assert_eq!(bump, bump2);
        assert_eq!(addr, addr2);
        // Bump is u8 — this assert documents the intent (always true).
        #[allow(unused_comparisons, clippy::absurd_extreme_comparisons)]
        {
            assert!(bump <= 255);
        }
        // Address should not be all zeros.
        assert!(addr.iter().any(|b| *b != 0));
    }
}
