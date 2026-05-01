//! Preflight (single-build) upgrade-safety rules.
//!
//! These rules look at one parsed program and flag patterns that make
//! future schema upgrades materially harder. They don't compare two
//! builds — that's the diff family in [`super::comparative`]. Run them
//! once per `quasar build` to get a "is this safe to deploy and evolve"
//! verdict before the first mainnet push.
//!
//! Rule set: L010 (missing version field), L011 (missing reserved
//! padding), L012 (account name collision with reserved Solana types).

use {
    super::types::{Diagnostic, LintRule, TypeRegistry},
    crate::parser::{state::RawStateAccount, ParsedProgram},
};

/// Names a `#[account(discriminator)]` struct should not take. Each one
/// either collides with a Quasar / SPL primitive (making tooling output
/// ambiguous) or matches a Solana primitive type (making docs and error
/// messages confusing).
const RESERVED_ACCOUNT_NAMES: &[&str] = &[
    "Account",
    "AccountInfo",
    "Address",
    "Mint",
    "Pubkey",
    "Signer",
    "System",
    "SystemAccount",
    "Token",
    "TokenAccount",
    "UncheckedAccount",
];

/// Run every preflight rule against `parsed` and append findings to
/// `diagnostics`. Single-pass — none of the rules look at each other.
pub fn run_all(parsed: &ParsedProgram, diagnostics: &mut Vec<Diagnostic>) {
    for account in &parsed.state_accounts {
        check_l010_missing_version_field(account, diagnostics);
        check_l011_missing_reserved_padding(account, diagnostics);
        check_l012_reserved_name_collision(account, diagnostics);
    }
    let _ = TypeRegistry::from_parsed; // reserved for future cross-rule context
}

// ---------------------------------------------------------------------------
// L010 — account-missing-version-field
// ---------------------------------------------------------------------------
//
// Borsh layouts have no in-band version tag. Without a leading `version: u8`
// (or u16) field, a future layout change forces every reader to guess which
// variant it's looking at by inference — usually they can't, and the program
// has to migrate every account on its first read after the upgrade. Adding
// a one-byte version up front is cheap at init and converts the worst-case
// upgrade from "rewrite every account" to "match on version, deserialize".

fn check_l010_missing_version_field(account: &RawStateAccount, diagnostics: &mut Vec<Diagnostic>) {
    let Some((first_name, _)) = account.fields.first() else {
        return;
    };
    if first_name == "version" {
        return;
    }
    diagnostics.push(Diagnostic {
        rule: LintRule::L010,
        severity: LintRule::L010.default_severity(),
        accounts_struct: account.name.clone(),
        field: Some(first_name.clone()),
        message: format!(
            "account `{}` has no leading `version: u8` field; future schema changes can't branch \
             on layout version at deserialize time",
            account.name
        ),
        suggestion: Some(format!(
            "Add `pub version: u8` (or u16) as the first field of `{}`. Initialise it to 1 on \
             creation and bump on every layout change so `try_deserialize` can route old vs new \
             bytes.",
            account.name
        )),
    });
}

// ---------------------------------------------------------------------------
// L011 — account-missing-reserved-padding
// ---------------------------------------------------------------------------
//
// Adding a field to a Borsh-serialized account changes its on-chain size,
// so every existing account has to be reallocated (paid for, signed for)
// before the new binary can read it. Trailing `_reserved: [u8; N]` padding
// pre-pays that cost: the next field that fits inside the reserved bytes
// is just a layout reinterpretation, no realloc needed. Without it, every
// future field append becomes an Unsafe upgrade.

fn check_l011_missing_reserved_padding(
    account: &RawStateAccount,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((last_name, last_ty)) = account.fields.last() else {
        return;
    };
    if last_name == "_reserved" && is_fixed_byte_array(last_ty) {
        return;
    }
    diagnostics.push(Diagnostic {
        rule: LintRule::L011,
        severity: LintRule::L011.default_severity(),
        accounts_struct: account.name.clone(),
        field: Some(last_name.clone()),
        message: format!(
            "account `{}` has no trailing `_reserved: [u8; N]` padding; every future field append \
             needs a realloc or migration",
            account.name
        ),
        suggestion: Some(format!(
            "Add `pub _reserved: [u8; 64]` (or whatever fits the headroom you want) as the last \
             field of `{}`. Cheap at init time, converts future appends from Unsafe to Additive.",
            account.name
        )),
    });
}

fn is_fixed_byte_array(ty: &syn::Type) -> bool {
    let syn::Type::Array(arr) = ty else {
        return false;
    };
    let syn::Type::Path(elem) = arr.elem.as_ref() else {
        return false;
    };
    elem.path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "u8")
}

// ---------------------------------------------------------------------------
// L012 — reserved-name-collision
// ---------------------------------------------------------------------------
//
// Naming a `#[account(...)]` struct after a well-known Solana / Quasar
// primitive (Mint, Token, Pubkey, AccountInfo, etc.) makes tooling output
// ambiguous — every `quasar lint` line, every error trace, every doc has
// to disambiguate "which `Token` do you mean". Cheap to rename now,
// painful to rename after the program is shipped and clients depend on
// the type name.

fn check_l012_reserved_name_collision(
    account: &RawStateAccount,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !RESERVED_ACCOUNT_NAMES.contains(&account.name.as_str()) {
        return;
    }
    diagnostics.push(Diagnostic {
        rule: LintRule::L012,
        severity: LintRule::L012.default_severity(),
        accounts_struct: account.name.clone(),
        field: None,
        message: format!(
            "account struct named `{}` collides with a well-known Solana / Quasar type",
            account.name
        ),
        suggestion: Some(format!(
            "Rename `{}` to something program-specific (e.g. `My{}`). Cheaper to do before \
             clients consume the type name.",
            account.name, account.name
        )),
    });
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::parser::{program::RawInstruction, state::RawStateAccount},
    };

    fn ty(s: &str) -> syn::Type {
        syn::parse_str(s).unwrap()
    }

    fn account(name: &str, fields: &[(&str, &str)]) -> RawStateAccount {
        RawStateAccount {
            name: name.to_string(),
            discriminator: vec![1, 2, 3, 4, 5, 6, 7, 8],
            fields: fields.iter().map(|(n, t)| (n.to_string(), ty(t))).collect(),
            seeds: None,
        }
    }

    fn parsed(state_accounts: Vec<RawStateAccount>) -> ParsedProgram {
        ParsedProgram {
            program_id: "11111111111111111111111111111111".to_string(),
            program_name: "test".to_string(),
            crate_name: "test".to_string(),
            version: "0.1.0".to_string(),
            instructions: Vec::<RawInstruction>::new(),
            accounts_structs: vec![],
            state_accounts,
            events: vec![],
            errors: vec![],
            data_structs: vec![],
        }
    }

    #[test]
    fn l010_fires_when_first_field_is_not_version() {
        let p = parsed(vec![account(
            "Escrow",
            &[("maker", "Pubkey"), ("amount", "u64")],
        )]);
        let mut diags = Vec::new();
        run_all(&p, &mut diags);
        assert!(diags.iter().any(|d| d.rule == LintRule::L010));
    }

    #[test]
    fn l010_silent_when_first_field_is_version() {
        let p = parsed(vec![account(
            "Escrow",
            &[
                ("version", "u8"),
                ("maker", "Pubkey"),
                ("_reserved", "[u8; 64]"),
            ],
        )]);
        let mut diags = Vec::new();
        run_all(&p, &mut diags);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L010));
    }

    #[test]
    fn l011_fires_when_no_reserved_padding() {
        let p = parsed(vec![account(
            "Escrow",
            &[("version", "u8"), ("amount", "u64")],
        )]);
        let mut diags = Vec::new();
        run_all(&p, &mut diags);
        assert!(diags.iter().any(|d| d.rule == LintRule::L011));
    }

    #[test]
    fn l011_silent_when_reserved_byte_array_present() {
        let p = parsed(vec![account(
            "Escrow",
            &[
                ("version", "u8"),
                ("amount", "u64"),
                ("_reserved", "[u8; 64]"),
            ],
        )]);
        let mut diags = Vec::new();
        run_all(&p, &mut diags);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L011));
    }

    #[test]
    fn l011_fires_when_reserved_field_is_wrong_type() {
        // `_reserved` is a Pubkey, not a `[u8; N]` — doesn't pre-pay realloc cost.
        let p = parsed(vec![account(
            "Escrow",
            &[("amount", "u64"), ("_reserved", "Pubkey")],
        )]);
        let mut diags = Vec::new();
        run_all(&p, &mut diags);
        assert!(diags.iter().any(|d| d.rule == LintRule::L011));
    }

    #[test]
    fn l012_fires_for_reserved_name() {
        let p = parsed(vec![account("Mint", &[("amount", "u64")])]);
        let mut diags = Vec::new();
        run_all(&p, &mut diags);
        assert!(diags.iter().any(|d| d.rule == LintRule::L012));
    }

    #[test]
    fn l012_silent_for_program_specific_name() {
        let p = parsed(vec![account("MyEscrow", &[("amount", "u64")])]);
        let mut diags = Vec::new();
        run_all(&p, &mut diags);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L012));
    }
}
