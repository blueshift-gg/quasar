//! Cross-build (diff) upgrade-safety rules.
//!
//! These rules need *two* builds to compare: the surface as it shipped
//! to mainnet (the "old" snapshot, loaded from `quasar.lock.json`)
//! versus the candidate the developer is about to release (the "new"
//! snapshot, derived from the current parse). Findings are written
//! into the same [`super::types::LintReport`] used by the single-build
//! rules; downstream output / suppression / CI integration is shared.
//!
//! ## Scope in this module
//!
//! Initial set of three representative rules:
//! - **L013** — account field reorder (Borsh layout corruption)
//! - **L018** — account discriminator change (orphans every existing account)
//! - **L019** — instruction removed (breaks every existing client)
//!
//! The full set (L013–L026) lands in follow-on PRs; the entry point
//! and snapshot infrastructure here are designed to absorb each new
//! rule as a single function appended to [`run_all`] without touching
//! the plumbing around it.
//!
//! ## What's intentionally not here
//!
//! - **R011 / R012 (enum variant rules)** — Quasar's `IdlTypeDefKind` is
//!   `Struct`-only at the schema layer. Adding enum support is an upstream
//!   change tracked separately; until then the equivalent rules in this catalog
//!   are unimplementable.
//! - **R013 PDA seed rule with full struct-field-path precision** — Quasar's
//!   `RawSeed::AccountRef` carries an account name but not a nested field path.
//!   The corresponding L023 (when added) will ship with reduced precision and
//!   document the limitation.

use super::{
    snapshot::ProgramSnapshot,
    types::{Diagnostic, LintRule},
};

/// Run every cross-build rule and return the resulting findings.
/// Caller folds these into the single-build report (or treats them as
/// a standalone report for `quasar lint --diff`-only invocations).
pub fn run_all(old: &ProgramSnapshot, new: &ProgramSnapshot) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    check_l013_account_field_reorder(old, new, &mut diagnostics);
    check_l018_account_discriminator_change(old, new, &mut diagnostics);
    check_l019_instruction_removed(old, new, &mut diagnostics);
    diagnostics
}

// ---------------------------------------------------------------------------
// L013 — account-field-reorder
// ---------------------------------------------------------------------------
//
// Borsh serializes account fields in source declaration order. Reordering
// them on a deployed program means every byte already on chain now
// deserializes against the wrong type at the wrong offset — silent data
// corruption with no error. The fix is almost always "put the order back";
// if the new order is semantically required, ship a one-shot migration
// instruction that rewrites every account in place.
//
// Detected when: the same account name exists in both snapshots, the set
// of field names is identical, but the order differs. A pure rename or
// retype is L014/L015's job, not this one.

fn check_l013_account_field_reorder(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_acc in &old.accounts {
        let Some(new_acc) = new.accounts.iter().find(|a| a.name == old_acc.name) else {
            continue;
        };

        let old_names: Vec<&str> = old_acc.fields.iter().map(|f| f.name.as_str()).collect();
        let new_names: Vec<&str> = new_acc.fields.iter().map(|f| f.name.as_str()).collect();

        // Same set of names but different order = reorder. If the sets
        // differ at all, that's add/remove territory (other rules' job).
        if old_names == new_names {
            continue;
        }
        let mut sorted_old = old_names.clone();
        let mut sorted_new = new_names.clone();
        sorted_old.sort_unstable();
        sorted_new.sort_unstable();
        if sorted_old != sorted_new {
            continue;
        }

        diagnostics.push(Diagnostic {
            rule: LintRule::L013,
            severity: LintRule::L013.default_severity(),
            accounts_struct: old_acc.name.clone(),
            field: None,
            message: format!(
                "fields reordered in account `{}`: {:?} → {:?}",
                old_acc.name, old_names, new_names
            ),
            suggestion: Some(
                "Borsh lays fields out in declaration order. Put them back in the original order, \
                 or write a one-shot migration instruction that rewrites every account."
                    .to_string(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// L018 — account-discriminator-change
// ---------------------------------------------------------------------------
//
// The first N bytes of every account on chain are the discriminator —
// Quasar's account loader uses them to confirm the account it's reading
// is what it expects. Changing the discriminator on a deployed account
// type means every existing account of that type fails the check; from
// the program's perspective they no longer exist.
//
// Detected when: an account name is present in both snapshots and the
// discriminator bytes differ. Renames are L025 (account-removed) +
// implicit add elsewhere — not this rule.

fn check_l018_account_discriminator_change(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_acc in &old.accounts {
        let Some(new_acc) = new.accounts.iter().find(|a| a.name == old_acc.name) else {
            continue;
        };
        if old_acc.discriminator == new_acc.discriminator {
            continue;
        }
        diagnostics.push(Diagnostic {
            rule: LintRule::L018,
            severity: LintRule::L018.default_severity(),
            accounts_struct: old_acc.name.clone(),
            field: None,
            message: format!(
                "discriminator of account `{}` changed: {:?} → {:?}",
                old_acc.name, old_acc.discriminator, new_acc.discriminator
            ),
            suggestion: Some(
                "If this is intentional (e.g. struct rename) and you've migrated existing \
                 accounts, suppress with `#[allow(quasar::account_discriminator_change)]`. \
                 Otherwise restore the original discriminator bytes."
                    .to_string(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// L019 — instruction-removed
// ---------------------------------------------------------------------------
//
// Removing an instruction breaks every off-chain caller that still routes
// to it — clients send the instruction data, the program can't dispatch,
// the transaction fails. Even if no client *should* be calling it, you
// usually don't have visibility into who is.
//
// Detected when: an instruction name in the old snapshot is missing from
// the new one. The standard fix is to keep the instruction declared but
// have its body return a specific error (or no-op) for one release cycle
// before deleting outright.

fn check_l019_instruction_removed(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_ix in &old.instructions {
        if new.instructions.iter().any(|n| n.name == old_ix.name) {
            continue;
        }
        diagnostics.push(Diagnostic {
            rule: LintRule::L019,
            severity: LintRule::L019.default_severity(),
            accounts_struct: old_ix.accounts_type_name.clone(),
            field: None,
            message: format!(
                "instruction `{}` was removed; any client that still calls it will fail",
                old_ix.name
            ),
            suggestion: Some(format!(
                "Keep the instruction declared but make its body return a specific error (or \
                 no-op) for one release before removing entirely. Or suppress with \
                 `#[allow(quasar::instruction_removed)]` once you're confident no client routes \
                 to `{}`.",
                old_ix.name
            )),
        });
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::lint::snapshot::{
            AccountSnapshot, InstructionSnapshot, NamedTypeSnapshot, ProgramSnapshot,
            SNAPSHOT_VERSION,
        },
    };

    fn empty_snap() -> ProgramSnapshot {
        ProgramSnapshot {
            version: SNAPSHOT_VERSION,
            program_id: "11111111111111111111111111111111".to_string(),
            program_name: "test".to_string(),
            accounts: vec![],
            instructions: vec![],
            events: vec![],
            accounts_structs: vec![],
        }
    }

    fn account(name: &str, disc: Vec<u8>, fields: &[(&str, &str)]) -> AccountSnapshot {
        AccountSnapshot {
            name: name.to_string(),
            discriminator: disc,
            fields: fields
                .iter()
                .map(|(n, t)| NamedTypeSnapshot {
                    name: n.to_string(),
                    ty: t.to_string(),
                })
                .collect(),
        }
    }

    fn instruction(name: &str, disc: Vec<u8>) -> InstructionSnapshot {
        InstructionSnapshot {
            name: name.to_string(),
            discriminator: disc,
            args: vec![],
            accounts_type_name: format!("{name}Accounts"),
        }
    }

    // L013 tests --------------------------------------------------------------

    #[test]
    fn l013_fires_on_field_reorder_with_same_names() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("a", "u8"), ("b", "u8")])];
        new.accounts = vec![account("X", vec![1], &[("b", "u8"), ("a", "u8")])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L013));
    }

    #[test]
    fn l013_silent_on_identical_order() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("a", "u8"), ("b", "u8")])];
        new.accounts = vec![account("X", vec![1], &[("a", "u8"), ("b", "u8")])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L013));
    }

    #[test]
    fn l013_silent_when_field_set_differs() {
        // a → b is a remove + add, not a reorder. Other rules cover it.
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("a", "u8"), ("b", "u8")])];
        new.accounts = vec![account("X", vec![1], &[("b", "u8"), ("c", "u8")])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L013));
    }

    // L018 tests --------------------------------------------------------------

    #[test]
    fn l018_fires_when_discriminator_changes() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![42], &[("a", "u8")])];
        new.accounts = vec![account("X", vec![99], &[("a", "u8")])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L018));
    }

    #[test]
    fn l018_silent_when_discriminator_unchanged() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![42], &[("a", "u8")])];
        new.accounts = vec![account("X", vec![42], &[("a", "u8")])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L018));
    }

    // L019 tests --------------------------------------------------------------

    #[test]
    fn l019_fires_when_instruction_removed() {
        let mut old = empty_snap();
        let new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L019));
    }

    #[test]
    fn l019_silent_when_instruction_kept() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        new.instructions = vec![instruction("make", vec![0])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L019));
    }

    #[test]
    fn l019_silent_when_unrelated_instruction_added() {
        // Adding a new instruction is additive, not breaking — old
        // callers keep working.
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        new.instructions = vec![instruction("make", vec![0]), instruction("take", vec![1])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L019));
    }
}
