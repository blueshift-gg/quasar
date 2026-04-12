//! Constraint types and completeness verification.
//!
//! `Constraint` is the canonical representation of a single `#[account(...)]`
//! directive. It is a type alias for `AccountDirective` — the same enum used
//! for parsing — exposed here as a semantic-level concept.
//!
//! The key safety mechanism is [`verify_all_directives_mapped`]: an exhaustive
//! match over every `Constraint` variant that forces a compile error when a
//! new variant is added without a handler mapping.

use super::attrs::AccountDirective;

/// A single `#[account(...)]` constraint directive.
///
/// This is a semantic alias for [`AccountDirective`]. The variants are
/// identical — the alias exists so that downstream code reads as "processing
/// constraints" rather than "processing parse output."
pub(crate) type Constraint = AccountDirective;

/// Exhaustive mapping that documents WHERE each constraint is processed.
///
/// **This is the completeness guarantee.** Adding a new variant to
/// `AccountDirective` without adding a match arm here causes a compile
/// error. An auditor reviews this single function to verify every
/// constraint is accounted for.
fn assert_directive_handled(constraint: &Constraint) {
    match constraint {
        // Header flags (bitmask in buffer walker)
        Constraint::Mut => {}

        // Lifecycle (init, close, sweep)
        Constraint::Init
        | Constraint::InitIfNeeded
        | Constraint::Close(_)
        | Constraint::Sweep(_)
        | Constraint::Payer(_)
        | Constraint::Space(_) => {}

        // Field-level validation checks
        Constraint::HasOne(_, _) | Constraint::Constraint(_, _) | Constraint::Address(_, _) => {}

        // PDA seed verification
        Constraint::Seeds(_) | Constraint::TypedSeeds(_) | Constraint::Bump(_) => {}

        // Token account / ATA / mint validation
        Constraint::TokenMint(_)
        | Constraint::TokenAuthority(_)
        | Constraint::TokenTokenProgram(_)
        | Constraint::AssociatedTokenMint(_)
        | Constraint::AssociatedTokenAuthority(_)
        | Constraint::AssociatedTokenTokenProgram(_)
        | Constraint::MintDecimals(_)
        | Constraint::MintInitAuthority(_)
        | Constraint::MintFreezeAuthority(_)
        | Constraint::MintTokenProgram(_) => {}

        // Realloc
        Constraint::Realloc(_) | Constraint::ReallocPayer(_) => {}

        // Metaplex metadata / master edition init
        Constraint::MetadataName(_)
        | Constraint::MetadataSymbol(_)
        | Constraint::MetadataUri(_)
        | Constraint::MetadataSellerFeeBasisPoints(_)
        | Constraint::MetadataIsMutable(_)
        | Constraint::MasterEditionMaxSupply(_) => {}

        // Buffer walker (dup detection)
        Constraint::Dup => {}
    }
}

/// Verify that every directive in the set has a known handler.
///
/// This is called at the end of `process_fields` for each field as a
/// runtime (macro-expansion-time) assertion. It is deliberately cheap —
/// the real safety comes from the exhaustive match in
/// [`assert_directive_handled`] which the compiler enforces at framework
/// compile time.
pub(crate) fn verify_all_directives_mapped(directives: &[Constraint]) {
    for d in directives {
        assert_directive_handled(d);
    }
}
