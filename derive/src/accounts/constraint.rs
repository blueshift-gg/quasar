//! Constraint types and completeness verification.
//!
//! `Constraint` is the canonical representation of a single `#[account(...)]`
//! directive. It is a type alias for `AccountDirective` — the same enum used
//! for parsing — exposed here as a semantic-level concept.
//!
//! The key safety mechanism is [`verify_directives_handled`]: an exhaustive
//! match over every `Constraint` variant that maps each one to the subsystem
//! responsible for handling it. Adding a new variant to `AccountDirective`
//! forces a compile error here, ensuring no constraint is silently ignored.

use super::attrs::AccountDirective;

/// A single `#[account(...)]` constraint directive.
///
/// This is a semantic alias for [`AccountDirective`]. The variants are
/// identical — the alias exists so that downstream code reads as "processing
/// constraints" rather than "processing parse output."
pub(crate) type Constraint = AccountDirective;

/// Subsystem responsible for handling a constraint.
///
/// Used by [`verify_directives_handled`] to document and enforce the
/// mapping from each constraint to the code that processes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handler {
    /// Encoded in the u32 header bitmask via `FieldFlags::compute`.
    HeaderFlags,
    /// Processed by `process_fields` in the per-field check loop.
    FieldCheck,
    /// Processed by `init::gen_init_block` for account creation.
    Init,
    /// Processed by the PDA seed/bump verification codegen.
    Pda,
    /// Processed by the close/sweep epilogue codegen.
    Lifecycle,
    /// Processed by `validate_token_account` / `validate_ata` / `validate_mint`.
    TokenValidation,
    /// Processed by realloc codegen.
    Realloc,
    /// Processed by metadata/master-edition init codegen.
    MetaplexInit,
    /// Processed by dup detection in the buffer walker (`mod.rs`).
    BufferWalker,
}

/// Exhaustive mapping from every `Constraint` variant to its handler.
///
/// **This is the completeness guarantee.** Adding a new variant to
/// `AccountDirective` without adding a match arm here causes a compile
/// error. An auditor reviews this single function to verify every
/// constraint is accounted for.
///
/// The returned `Handler` documents WHERE each constraint is processed,
/// not HOW — the actual codegen lives in the respective subsystem.
pub(crate) fn directive_handler(constraint: &Constraint) -> Handler {
    match constraint {
        // -- Header flags (bitmask in buffer walker) --
        Constraint::Mut => Handler::HeaderFlags,

        // -- Lifecycle (init, close, sweep) --
        Constraint::Init => Handler::Init,
        Constraint::InitIfNeeded => Handler::Init,
        Constraint::Close(_) => Handler::Lifecycle,
        Constraint::Sweep(_) => Handler::Lifecycle,

        // -- Init parameters (consumed by init codegen) --
        Constraint::Payer(_) => Handler::Init,
        Constraint::Space(_) => Handler::Init,

        // -- Field-level validation checks --
        Constraint::HasOne(_, _) => Handler::FieldCheck,
        Constraint::Constraint(_, _) => Handler::FieldCheck,
        Constraint::Address(_, _) => Handler::FieldCheck,

        // -- PDA seed verification --
        Constraint::Seeds(_) => Handler::Pda,
        Constraint::TypedSeeds(_) => Handler::Pda,
        Constraint::Bump(_) => Handler::Pda,

        // -- Token account validation --
        Constraint::TokenMint(_) => Handler::TokenValidation,
        Constraint::TokenAuthority(_) => Handler::TokenValidation,
        Constraint::TokenTokenProgram(_) => Handler::TokenValidation,

        // -- Associated token account validation --
        Constraint::AssociatedTokenMint(_) => Handler::TokenValidation,
        Constraint::AssociatedTokenAuthority(_) => Handler::TokenValidation,
        Constraint::AssociatedTokenTokenProgram(_) => Handler::TokenValidation,

        // -- Realloc --
        Constraint::Realloc(_) => Handler::Realloc,
        Constraint::ReallocPayer(_) => Handler::Realloc,

        // -- Metaplex metadata init --
        Constraint::MetadataName(_) => Handler::MetaplexInit,
        Constraint::MetadataSymbol(_) => Handler::MetaplexInit,
        Constraint::MetadataUri(_) => Handler::MetaplexInit,
        Constraint::MetadataSellerFeeBasisPoints(_) => Handler::MetaplexInit,
        Constraint::MetadataIsMutable(_) => Handler::MetaplexInit,

        // -- Master edition init --
        Constraint::MasterEditionMaxSupply(_) => Handler::MetaplexInit,

        // -- Mint init / validation --
        Constraint::MintDecimals(_) => Handler::TokenValidation,
        Constraint::MintInitAuthority(_) => Handler::TokenValidation,
        Constraint::MintFreezeAuthority(_) => Handler::TokenValidation,
        Constraint::MintTokenProgram(_) => Handler::TokenValidation,

        // -- Buffer walker (dup detection) --
        Constraint::Dup => Handler::BufferWalker,
    }
}

/// Verify that every directive in the set has a known handler.
///
/// This is called at the end of `process_fields` for each field as a
/// runtime (macro-expansion-time) assertion. It is deliberately cheap —
/// the real safety comes from the exhaustive match in [`directive_handler`]
/// which the compiler enforces at framework compile time.
pub(crate) fn verify_all_directives_mapped(directives: &[Constraint]) {
    for d in directives {
        // Force the exhaustive match to run. If a variant is unhandled,
        // this won't compile.
        let _ = directive_handler(d);
    }
}
