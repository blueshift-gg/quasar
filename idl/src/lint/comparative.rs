//! Cross-build (diff) upgrade-safety rules.
//!
//! These rules need *two* builds to compare: the surface as it shipped
//! to mainnet (the "old" snapshot, loaded from `quasar.lock.json`)
//! versus the candidate the developer is about to release (the "new"
//! snapshot, derived from the current parse). Findings are written
//! into the same [`super::types::LintReport`] used by the single-build
//! rules; downstream output / suppression / CI integration is shared.
//!
//! ## Scope
//!
//! Account-shape diffs:
//! - **L013** — field reorder (Borsh layout corruption)
//! - **L014** — field retype (offset / interpretation mismatch)
//! - **L015** — field removed (downstream offsets corrupt)
//! - **L016** — mid-list insert (every later offset shifts)
//! - **L017** — append (Warning — needs realloc, not silently corrupt)
//! - **L018** — discriminator change (orphans every existing account)
//! - **L025** — account struct removed (existing accounts orphaned)
//!
//! Instruction-shape diffs:
//! - **L019** — instruction removed (breaks every existing client)
//! - **L020** — argument signature change (clients send misread bytes)
//! - **L021** — account list added/removed/reordered (positional indices)
//! - **L022** — `is_signer` / `is_writable` flip (wire-format mismatch)
//! - **L024** — instruction discriminator change (clients route wrong)
//!
//! PDA + event diffs:
//! - **L023** — PDA seed change (every existing PDA orphaned; reduced precision
//!   — see rule docs)
//! - **L026** — event discriminator change (off-chain indexers go silent)
//!
//! Adding a rule means writing one function over `(old, new)` and
//! appending it to [`run_all`]; the snapshot, lock-file IO, and CLI
//! plumbing don't move.
//!
//! ## What's intentionally not here
//!
//! - **R011 / R012 (enum variant rules)** — Quasar's `IdlTypeDefKind` is
//!   `Struct`-only at the schema layer. Adding enum support is an upstream
//!   change tracked separately; until then the equivalent rules in this catalog
//!   are unimplementable.
//! - **L023 PDA seed rule with full struct-field-path precision** — Quasar's
//!   `RawSeed::AccountRef` carries an account name but not a nested
//!   struct-field path. L023 ships in reduced-precision mode: it catches
//!   account-level seed swaps but can miss seed changes that only swap a field
//!   path within the same account. Tracked as a future-work parser enhancement.

use super::{
    snapshot::ProgramSnapshot,
    types::{Diagnostic, LintRule},
};

/// Run every cross-build rule and return the resulting findings.
/// Caller folds these into the single-build report (or treats them as
/// a standalone report for `quasar lint --diff`-only invocations).
pub fn run_all(old: &ProgramSnapshot, new: &ProgramSnapshot) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // Account-shape diffs.
    check_l013_account_field_reorder(old, new, &mut diagnostics);
    check_l014_account_field_retype(old, new, &mut diagnostics);
    check_l015_account_field_removed(old, new, &mut diagnostics);
    check_l016_account_field_insert_middle(old, new, &mut diagnostics);
    check_l017_account_field_append(old, new, &mut diagnostics);
    check_l018_account_discriminator_change(old, new, &mut diagnostics);
    check_l025_account_removed(old, new, &mut diagnostics);
    // Instruction-shape diffs.
    check_l019_instruction_removed(old, new, &mut diagnostics);
    check_l020_instruction_arg_change(old, new, &mut diagnostics);
    check_l021_instruction_account_list_change(old, new, &mut diagnostics);
    check_l022_instruction_signer_writable_flip(old, new, &mut diagnostics);
    check_l024_instruction_discriminator_change(old, new, &mut diagnostics);
    // PDA + event diffs.
    check_l023_pda_seed_change(old, new, &mut diagnostics);
    check_l026_event_discriminator_change(old, new, &mut diagnostics);
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

// ---------------------------------------------------------------------------
// L014 — account-field-retype
// ---------------------------------------------------------------------------
//
// A field's type changed without the name changing. Borsh reads bytes
// at the offset assigned by declaration order and the type at that
// offset — `u64 → u32` shifts every later offset by 4 bytes (and reads
// the wrong value for the field itself). Even type changes that don't
// shift offsets (`u8 → bool`) reinterpret existing bytes and almost
// always cause data corruption or panics.

fn check_l014_account_field_retype(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_acc in &old.accounts {
        let Some(new_acc) = new.accounts.iter().find(|a| a.name == old_acc.name) else {
            continue;
        };
        for old_field in &old_acc.fields {
            let Some(new_field) = new_acc.fields.iter().find(|f| f.name == old_field.name) else {
                continue;
            };
            if old_field.ty == new_field.ty {
                continue;
            }
            diagnostics.push(Diagnostic {
                rule: LintRule::L014,
                severity: LintRule::L014.default_severity(),
                accounts_struct: old_acc.name.clone(),
                field: Some(old_field.name.clone()),
                message: format!(
                    "field `{}.{}` type changed: `{}` → `{}`",
                    old_acc.name, old_field.name, old_field.ty, new_field.ty
                ),
                suggestion: Some(
                    "Borsh layout depends on the declared type at each offset. Restore the \
                     original type, or write a one-shot migration that rewrites every account \
                     under the new type."
                        .to_string(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// L015 — account-field-removed
// ---------------------------------------------------------------------------
//
// Removing a field by name leaves its on-chain bytes in place — the
// next field now reads from the wrong offset. The whole layout downstream
// of the removal is corrupted on every existing account.

fn check_l015_account_field_removed(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_acc in &old.accounts {
        let Some(new_acc) = new.accounts.iter().find(|a| a.name == old_acc.name) else {
            continue;
        };
        for old_field in &old_acc.fields {
            if new_acc.fields.iter().any(|f| f.name == old_field.name) {
                continue;
            }
            diagnostics.push(Diagnostic {
                rule: LintRule::L015,
                severity: LintRule::L015.default_severity(),
                accounts_struct: old_acc.name.clone(),
                field: Some(old_field.name.clone()),
                message: format!(
                    "field `{}.{}` was removed; the bytes are still on chain and now alias the \
                     next field",
                    old_acc.name, old_field.name
                ),
                suggestion: Some(
                    "Either keep the field declared (rename to `_deprecated_<name>` to mark \
                     intent) or write a one-shot migration that compacts the layout of every \
                     account."
                        .to_string(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// L016 — account-field-insert-middle
// ---------------------------------------------------------------------------
//
// A field added anywhere except the tail shifts every later field's
// offset. Existing on-chain accounts deserialize against the wrong
// offsets — silent corruption.
//
// Detected when: `new` has a field name not present in `old`, and the
// position of that name in the new field list is *before* at least one
// name that did exist in old. A pure append fires L017 instead.

fn check_l016_account_field_insert_middle(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_acc in &old.accounts {
        let Some(new_acc) = new.accounts.iter().find(|a| a.name == old_acc.name) else {
            continue;
        };
        let old_names: Vec<&str> = old_acc.fields.iter().map(|f| f.name.as_str()).collect();
        for (idx, new_field) in new_acc.fields.iter().enumerate() {
            if old_names.contains(&new_field.name.as_str()) {
                continue;
            }
            // Is there an existing-name field after this insertion point?
            // If yes, the insert shifts that field's offset → mid-list insert.
            let inserted_before_existing = new_acc.fields[idx + 1..]
                .iter()
                .any(|later| old_names.contains(&later.name.as_str()));
            if !inserted_before_existing {
                continue; // pure append, L017's job
            }
            diagnostics.push(Diagnostic {
                rule: LintRule::L016,
                severity: LintRule::L016.default_severity(),
                accounts_struct: old_acc.name.clone(),
                field: Some(new_field.name.clone()),
                message: format!(
                    "field `{}.{}` was inserted before existing fields; every later offset shifts \
                     and existing accounts deserialize to garbage",
                    old_acc.name, new_field.name
                ),
                suggestion: Some(
                    "Append new fields to the tail of the account (after `_reserved` if the \
                     account had reserved padding sized for it). Mid-list insertion requires a \
                     one-shot migration of every existing account."
                        .to_string(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// L017 — account-field-append
// ---------------------------------------------------------------------------
//
// Adding a field at the tail doesn't corrupt existing data, but it
// changes the account size — every existing account has to be
// reallocated (paid for, signed for) before the new binary can read it.
// If the account had `_reserved: [u8; N]` padding sized for the new
// field, this could be safely *replaced* with a typed field — but the
// snapshot can't tell whether the new field fits inside the prior
// padding budget without inspecting type sizes, so it warns and lets
// the dev confirm.
//
// Severity: Warning, not Error — the upgrade is recoverable with a
// realloc, and devs intentionally adding migration code shouldn't have
// the build fail on them.

fn check_l017_account_field_append(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_acc in &old.accounts {
        let Some(new_acc) = new.accounts.iter().find(|a| a.name == old_acc.name) else {
            continue;
        };
        let old_names: Vec<&str> = old_acc.fields.iter().map(|f| f.name.as_str()).collect();
        for (idx, new_field) in new_acc.fields.iter().enumerate() {
            if old_names.contains(&new_field.name.as_str()) {
                continue;
            }
            let inserted_before_existing = new_acc.fields[idx + 1..]
                .iter()
                .any(|later| old_names.contains(&later.name.as_str()));
            if inserted_before_existing {
                continue; // mid-list insert, L016's job
            }
            diagnostics.push(Diagnostic {
                rule: LintRule::L017,
                severity: LintRule::L017.default_severity(),
                accounts_struct: old_acc.name.clone(),
                field: Some(new_field.name.clone()),
                message: format!(
                    "field `{}.{}` was appended; every existing account needs a realloc before \
                     the new binary can read it",
                    old_acc.name, new_field.name
                ),
                suggestion: Some(
                    "If the account had `_reserved` padding sized for this field, replace the \
                     padding with the typed field instead — that's an additive change. Otherwise \
                     plan a one-shot realloc instruction."
                        .to_string(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// L020 — instruction-arg-change
// ---------------------------------------------------------------------------
//
// Instruction data is Borsh-serialized in declaration order, so any
// change to the arg list (rename, retype, reorder, add, remove) means
// existing clients send bytes the program will misread. Fires on any
// difference in the ordered (name, ty) sequence.

fn check_l020_instruction_arg_change(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_ix in &old.instructions {
        let Some(new_ix) = new.instructions.iter().find(|i| i.name == old_ix.name) else {
            continue;
        };
        if old_ix.args == new_ix.args {
            continue;
        }
        let render = |args: &[crate::lint::snapshot::NamedTypeSnapshot]| -> String {
            args.iter()
                .map(|a| format!("{}: {}", a.name, a.ty))
                .collect::<Vec<_>>()
                .join(", ")
        };
        diagnostics.push(Diagnostic {
            rule: LintRule::L020,
            severity: LintRule::L020.default_severity(),
            accounts_struct: old_ix.accounts_type_name.clone(),
            field: None,
            message: format!(
                "argument signature of `{}` changed: ({}) → ({})",
                old_ix.name,
                render(&old_ix.args),
                render(&new_ix.args)
            ),
            suggestion: Some(
                "Borsh serializes args in declaration order. Prefer adding a new instruction name \
                 rather than reshaping an existing one, or coordinate updates with every caller \
                 in the same release."
                    .to_string(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// L021 — instruction-account-list-change
// ---------------------------------------------------------------------------
//
// Instruction accounts are passed by *position* — clients construct an
// `AccountMeta` array indexed positionally. Adding, removing, or
// reordering account slots changes that indexing. Detected by comparing
// the ordered list of account-slot names per instruction's accounts
// struct.

fn check_l021_instruction_account_list_change(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_ix in &old.instructions {
        let Some(new_ix) = new.instructions.iter().find(|i| i.name == old_ix.name) else {
            continue;
        };
        let Some(old_struct) = old
            .accounts_structs
            .iter()
            .find(|s| s.name == old_ix.accounts_type_name)
        else {
            continue;
        };
        let Some(new_struct) = new
            .accounts_structs
            .iter()
            .find(|s| s.name == new_ix.accounts_type_name)
        else {
            continue;
        };
        let old_names: Vec<&str> = old_struct.fields.iter().map(|f| f.name.as_str()).collect();
        let new_names: Vec<&str> = new_struct.fields.iter().map(|f| f.name.as_str()).collect();
        if old_names == new_names {
            continue;
        }
        diagnostics.push(Diagnostic {
            rule: LintRule::L021,
            severity: LintRule::L021.default_severity(),
            accounts_struct: old_ix.accounts_type_name.clone(),
            field: None,
            message: format!(
                "instruction `{}` account list changed: {:?} → {:?}",
                old_ix.name, old_names, new_names
            ),
            suggestion: Some(
                "Account slots are positional. Append-only changes are still risky if existing \
                 clients pass the new slots as `remaining_accounts`. Prefer a new instruction \
                 name when the account shape needs to evolve."
                    .to_string(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// L022 — instruction-signer-writable-flip
// ---------------------------------------------------------------------------
//
// The `is_signer` and `is_writable` flags on each `AccountMeta` are
// part of the wire format. Flipping either on a slot that exists in
// both versions means existing callers send bytes the program rejects
// (or, worse, silently accepts when it shouldn't). Fires per slot, not
// per instruction.

fn check_l022_instruction_signer_writable_flip(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_ix in &old.instructions {
        let Some(new_ix) = new.instructions.iter().find(|i| i.name == old_ix.name) else {
            continue;
        };
        let Some(old_struct) = old
            .accounts_structs
            .iter()
            .find(|s| s.name == old_ix.accounts_type_name)
        else {
            continue;
        };
        let Some(new_struct) = new
            .accounts_structs
            .iter()
            .find(|s| s.name == new_ix.accounts_type_name)
        else {
            continue;
        };
        for old_slot in &old_struct.fields {
            let Some(new_slot) = new_struct.fields.iter().find(|s| s.name == old_slot.name) else {
                continue;
            };
            if old_slot.signer == new_slot.signer && old_slot.writable == new_slot.writable {
                continue;
            }
            diagnostics.push(Diagnostic {
                rule: LintRule::L022,
                severity: LintRule::L022.default_severity(),
                accounts_struct: old_ix.accounts_type_name.clone(),
                field: Some(old_slot.name.clone()),
                message: format!(
                    "slot `{}.{}` flags flipped: signer {} → {}, writable {} → {}",
                    old_ix.name,
                    old_slot.name,
                    old_slot.signer,
                    new_slot.signer,
                    old_slot.writable,
                    new_slot.writable
                ),
                suggestion: Some(
                    "Existing clients send `AccountMeta` with the old flags. Coordinate updates \
                     across every caller in the same release, or version the instruction by name."
                        .to_string(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// L023 — pda-seed-change
// ---------------------------------------------------------------------------
//
// A PDA's address is `derive(seeds, program_id)`. Change any seed and
// every existing account at the old address is orphaned — the program
// computes a new address and finds nothing there. Detected by comparing
// the ordered seed list per slot.
//
// **Precision limitation.** `RawSeed::AccountRef` carries an account
// name but not a nested struct-field path (e.g., `account.field.nested`
// — Quasar's parser doesn't currently extract the path). This rule
// catches account-level seed changes (different account referenced)
// but can miss seed changes that only swap a field path within the
// same account. Tracked as future-work parser enhancement.

fn check_l023_pda_seed_change(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_struct in &old.accounts_structs {
        let Some(new_struct) = new
            .accounts_structs
            .iter()
            .find(|s| s.name == old_struct.name)
        else {
            continue;
        };
        for old_slot in &old_struct.fields {
            let Some(new_slot) = new_struct.fields.iter().find(|s| s.name == old_slot.name) else {
                continue;
            };
            if old_slot.pda == new_slot.pda {
                continue;
            }
            diagnostics.push(Diagnostic {
                rule: LintRule::L023,
                severity: LintRule::L023.default_severity(),
                accounts_struct: old_struct.name.clone(),
                field: Some(old_slot.name.clone()),
                message: format!(
                    "PDA seeds for slot `{}.{}` changed; every existing account at the old \
                     address is now orphaned",
                    old_struct.name, old_slot.name
                ),
                suggestion: Some(
                    "Restore the original seed expression, or write a one-shot migration that \
                     transfers state from the old PDA to the new one for every existing account."
                        .to_string(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// L024 — instruction-discriminator-change
// ---------------------------------------------------------------------------
//
// The first N bytes of the instruction data identify which handler to
// dispatch. Changing the discriminator on an existing instruction
// means every existing caller routes to the wrong slot — or to none
// at all.

fn check_l024_instruction_discriminator_change(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_ix in &old.instructions {
        let Some(new_ix) = new.instructions.iter().find(|i| i.name == old_ix.name) else {
            continue;
        };
        if old_ix.discriminator == new_ix.discriminator {
            continue;
        }
        diagnostics.push(Diagnostic {
            rule: LintRule::L024,
            severity: LintRule::L024.default_severity(),
            accounts_struct: old_ix.accounts_type_name.clone(),
            field: None,
            message: format!(
                "discriminator of instruction `{}` changed: {:?} → {:?}",
                old_ix.name, old_ix.discriminator, new_ix.discriminator
            ),
            suggestion: Some(
                "Restore the original discriminator. If the rename is intentional, version the \
                 instruction by name and keep the original handler routed to its old \
                 discriminator for one release cycle."
                    .to_string(),
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// L025 — account-removed
// ---------------------------------------------------------------------------
//
// Removing an account struct from the program means every existing
// account of that type is orphaned — the program no longer has a
// loader for it, so even reads fail. Equivalent severity to a
// discriminator change for that type.

fn check_l025_account_removed(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_acc in &old.accounts {
        if new.accounts.iter().any(|a| a.name == old_acc.name) {
            continue;
        }
        diagnostics.push(Diagnostic {
            rule: LintRule::L025,
            severity: LintRule::L025.default_severity(),
            accounts_struct: old_acc.name.clone(),
            field: None,
            message: format!(
                "account struct `{}` was removed; existing accounts of that type are orphaned",
                old_acc.name
            ),
            suggestion: Some(format!(
                "Keep the struct declared (mark it `#[deprecated]` if needed) for one release \
                 cycle so existing accounts can still be read while clients migrate. Or suppress \
                 with `#[allow(quasar::account_removed)]` once you're confident no accounts of \
                 type `{}` remain on chain.",
                old_acc.name
            )),
        });
    }
}

// ---------------------------------------------------------------------------
// L026 — event-discriminator-change
// ---------------------------------------------------------------------------
//
// Event discriminators are how off-chain indexers filter logs. Change
// one and every listener filtering for the old value goes silent —
// usually without raising any error visible to the program team.

fn check_l026_event_discriminator_change(
    old: &ProgramSnapshot,
    new: &ProgramSnapshot,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for old_evt in &old.events {
        let Some(new_evt) = new.events.iter().find(|e| e.name == old_evt.name) else {
            continue;
        };
        if old_evt.discriminator == new_evt.discriminator {
            continue;
        }
        diagnostics.push(Diagnostic {
            rule: LintRule::L026,
            severity: LintRule::L026.default_severity(),
            accounts_struct: old_evt.name.clone(),
            field: None,
            message: format!(
                "discriminator of event `{}` changed: {:?} → {:?}",
                old_evt.name, old_evt.discriminator, new_evt.discriminator
            ),
            suggestion: Some(
                "Restore the original discriminator. Off-chain indexers filtering on the prior \
                 value go silent without raising any visible error."
                    .to_string(),
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::lint::snapshot::{
            AccountSlotSnapshot, AccountSnapshot, AccountsStructSnapshot, EventSnapshot,
            InstructionSnapshot, NamedTypeSnapshot, PdaSnapshot, ProgramSnapshot, SeedSnapshot,
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

    // --- shared helpers for the L014–L026 tests -----------------------------

    fn instruction_with_args(
        name: &str,
        disc: Vec<u8>,
        args: &[(&str, &str)],
    ) -> InstructionSnapshot {
        InstructionSnapshot {
            name: name.to_string(),
            discriminator: disc,
            args: args
                .iter()
                .map(|(n, t)| NamedTypeSnapshot {
                    name: n.to_string(),
                    ty: t.to_string(),
                })
                .collect(),
            accounts_type_name: format!("{name}Accounts"),
        }
    }

    fn slot(name: &str, signer: bool, writable: bool) -> AccountSlotSnapshot {
        AccountSlotSnapshot {
            name: name.to_string(),
            signer,
            writable,
            pda: None,
        }
    }

    fn slot_with_pda(name: &str, seeds: Vec<SeedSnapshot>) -> AccountSlotSnapshot {
        AccountSlotSnapshot {
            name: name.to_string(),
            signer: false,
            writable: true,
            pda: Some(PdaSnapshot { seeds }),
        }
    }

    fn accounts_struct(name: &str, slots: Vec<AccountSlotSnapshot>) -> AccountsStructSnapshot {
        AccountsStructSnapshot {
            name: name.to_string(),
            fields: slots,
        }
    }

    fn event(name: &str, disc: Vec<u8>) -> EventSnapshot {
        EventSnapshot {
            name: name.to_string(),
            discriminator: disc,
            fields: vec![],
        }
    }

    // L014 tests --------------------------------------------------------------

    #[test]
    fn l014_fires_on_field_retype() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("amount", "u64")])];
        new.accounts = vec![account("X", vec![1], &[("amount", "u32")])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L014));
    }

    #[test]
    fn l014_silent_when_type_unchanged() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("amount", "u64")])];
        new.accounts = vec![account("X", vec![1], &[("amount", "u64")])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L014));
    }

    // L015 tests --------------------------------------------------------------

    #[test]
    fn l015_fires_when_field_removed() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("a", "u64"), ("b", "u8")])];
        new.accounts = vec![account("X", vec![1], &[("a", "u64")])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L015));
    }

    // L016 tests --------------------------------------------------------------

    #[test]
    fn l016_fires_on_mid_list_insert() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("a", "u64"), ("b", "u8")])];
        // `c` inserted between `a` and `b` — `b`'s offset shifts.
        new.accounts = vec![account(
            "X",
            vec![1],
            &[("a", "u64"), ("c", "u8"), ("b", "u8")],
        )];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L016));
        // Should not also fire L017 — this is mid-list, not append.
        assert!(!diags.iter().any(|d| d.rule == LintRule::L017));
    }

    // L017 tests --------------------------------------------------------------

    #[test]
    fn l017_fires_on_append() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("a", "u64")])];
        new.accounts = vec![account("X", vec![1], &[("a", "u64"), ("b", "u8")])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L017));
        assert!(!diags.iter().any(|d| d.rule == LintRule::L016));
    }

    #[test]
    fn l017_severity_is_warning_not_error() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("X", vec![1], &[("a", "u64")])];
        new.accounts = vec![account("X", vec![1], &[("a", "u64"), ("b", "u8")])];
        let diags = run_all(&old, &new);
        let l017 = diags.iter().find(|d| d.rule == LintRule::L017).unwrap();
        assert_eq!(l017.severity, crate::lint::types::Severity::Warning);
    }

    // L020 tests --------------------------------------------------------------

    #[test]
    fn l020_fires_on_arg_retype() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction_with_args("make", vec![0], &[("amount", "u64")])];
        new.instructions = vec![instruction_with_args("make", vec![0], &[("amount", "u32")])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L020));
    }

    #[test]
    fn l020_fires_on_arg_added() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction_with_args("make", vec![0], &[("amount", "u64")])];
        new.instructions = vec![instruction_with_args(
            "make",
            vec![0],
            &[("amount", "u64"), ("memo", "String")],
        )];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L020));
    }

    #[test]
    fn l020_silent_when_args_identical() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction_with_args("make", vec![0], &[("amount", "u64")])];
        new.instructions = vec![instruction_with_args("make", vec![0], &[("amount", "u64")])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L020));
    }

    // L021 tests --------------------------------------------------------------

    #[test]
    fn l021_fires_when_account_slot_added() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        new.instructions = vec![instruction("make", vec![0])];
        old.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("maker", true, true)],
        )];
        new.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("maker", true, true), slot("vault", false, true)],
        )];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L021));
    }

    #[test]
    fn l021_silent_when_slots_unchanged() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        new.instructions = vec![instruction("make", vec![0])];
        old.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("maker", true, true)],
        )];
        new.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("maker", true, true)],
        )];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L021));
    }

    // L022 tests --------------------------------------------------------------

    #[test]
    fn l022_fires_when_writable_flips() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        new.instructions = vec![instruction("make", vec![0])];
        old.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("vault", false, false)],
        )];
        new.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("vault", false, true)],
        )];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L022));
    }

    #[test]
    fn l022_fires_when_signer_flips() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        new.instructions = vec![instruction("make", vec![0])];
        old.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("authority", false, false)],
        )];
        new.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot("authority", true, false)],
        )];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L022));
    }

    // L023 tests --------------------------------------------------------------

    #[test]
    fn l023_fires_on_seed_change() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot_with_pda(
                "escrow",
                vec![SeedSnapshot::Const {
                    bytes: b"escrow".to_vec(),
                }],
            )],
        )];
        new.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot_with_pda(
                "escrow",
                vec![SeedSnapshot::Const {
                    bytes: b"vault".to_vec(),
                }],
            )],
        )];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L023));
    }

    #[test]
    fn l023_silent_when_seeds_match() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        let seeds = vec![SeedSnapshot::Const {
            bytes: b"escrow".to_vec(),
        }];
        old.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot_with_pda("escrow", seeds.clone())],
        )];
        new.accounts_structs = vec![accounts_struct(
            "makeAccounts",
            vec![slot_with_pda("escrow", seeds)],
        )];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L023));
    }

    // L024 tests --------------------------------------------------------------

    #[test]
    fn l024_fires_when_instruction_discriminator_changes() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.instructions = vec![instruction("make", vec![0])];
        new.instructions = vec![instruction("make", vec![7])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L024));
    }

    // L025 tests --------------------------------------------------------------

    #[test]
    fn l025_fires_when_account_struct_removed() {
        let mut old = empty_snap();
        let new = empty_snap();
        old.accounts = vec![account("Escrow", vec![42], &[("a", "u64")])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L025));
    }

    #[test]
    fn l025_silent_when_account_kept() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.accounts = vec![account("Escrow", vec![42], &[("a", "u64")])];
        new.accounts = vec![account("Escrow", vec![42], &[("a", "u64")])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L025));
    }

    // L026 tests --------------------------------------------------------------

    #[test]
    fn l026_fires_when_event_discriminator_changes() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.events = vec![event("EscrowMade", vec![1])];
        new.events = vec![event("EscrowMade", vec![9])];
        let diags = run_all(&old, &new);
        assert!(diags.iter().any(|d| d.rule == LintRule::L026));
    }

    #[test]
    fn l026_silent_when_event_unchanged() {
        let mut old = empty_snap();
        let mut new = empty_snap();
        old.events = vec![event("EscrowMade", vec![1])];
        new.events = vec![event("EscrowMade", vec![1])];
        let diags = run_all(&old, &new);
        assert!(!diags.iter().any(|d| d.rule == LintRule::L026));
    }
}
