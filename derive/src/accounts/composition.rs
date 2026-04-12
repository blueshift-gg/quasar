//! Constraint composition rules as a static, auditable table.
//!
//! [`COMPOSITION_RULES`] is a declarative rule table that an auditor reviews
//! as a single, flat specification of which constraints are incompatible,
//! which require others, and which are restricted to certain field kinds.

use {
    super::{attrs::AccountFieldAttrs, field_kind::FieldKind},
    crate::helpers::extract_generic_inner_type,
    syn::Ident,
};

/// Predicate over the constraint set for a single field.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Dup, AnySeeds exist for future rule-table completeness
enum Pred {
    Init,
    InitIfNeeded,
    AnyInit,
    Close,
    Sweep,
    Dup,
    Payer,
    Space,
    Seeds,
    AnySeeds,
    Realloc,
    ReallocPayer,
    TokenMint,
    TokenAuthority,
    TokenTokenProgram,
    AtaMint,
    AtaAuthority,
    AtaTokenProgram,
    MintDecimals,
    MintTokenProgram,
    MetadataAny,
    MetadataName,
    MetadataSymbol,
    MetadataUri,
    MasterEditionMaxSupply,
}

impl Pred {
    fn matches(self, attrs: &AccountFieldAttrs) -> bool {
        match self {
            Pred::Init => attrs.is_init,
            Pred::InitIfNeeded => attrs.init_if_needed,
            Pred::AnyInit => attrs.is_init || attrs.init_if_needed,
            Pred::Close => attrs.close.is_some(),
            Pred::Sweep => attrs.sweep.is_some(),
            Pred::Dup => attrs.dup,
            Pred::Payer => attrs.payer.is_some(),
            Pred::Space => attrs.space.is_some(),
            Pred::Seeds => attrs.seeds.is_some(),
            Pred::AnySeeds => attrs.seeds.is_some() || attrs.typed_seeds.is_some(),
            Pred::Realloc => attrs.realloc.is_some(),
            Pred::ReallocPayer => attrs.realloc_payer.is_some(),
            Pred::TokenMint => attrs.token_mint.is_some(),
            Pred::TokenAuthority => attrs.token_authority.is_some(),
            Pred::TokenTokenProgram => attrs.token_token_program.is_some(),
            Pred::AtaMint => attrs.associated_token_mint.is_some(),
            Pred::AtaAuthority => attrs.associated_token_authority.is_some(),
            Pred::AtaTokenProgram => attrs.associated_token_token_program.is_some(),
            Pred::MintDecimals => attrs.mint_decimals.is_some(),
            Pred::MintTokenProgram => attrs.mint_token_program.is_some(),
            Pred::MetadataAny => {
                attrs.metadata_name.is_some()
                    || attrs.metadata_symbol.is_some()
                    || attrs.metadata_uri.is_some()
                    || attrs.metadata_seller_fee_basis_points.is_some()
                    || attrs.metadata_is_mutable.is_some()
            }
            Pred::MetadataName => attrs.metadata_name.is_some(),
            Pred::MetadataSymbol => attrs.metadata_symbol.is_some(),
            Pred::MetadataUri => attrs.metadata_uri.is_some(),
            Pred::MasterEditionMaxSupply => attrs.master_edition_max_supply.is_some(),
        }
    }
}

/// Predicate over the field kind.
#[derive(Debug, Clone, Copy)]
enum KindPred {
    Account,
    TokenAccount,
    NotMint,
    NotOptional,
}

impl KindPred {
    fn matches(self, kind: &FieldKind, field: &syn::Field) -> bool {
        match self {
            KindPred::Account => matches!(kind, FieldKind::Account { .. }),
            KindPred::TokenAccount => kind.is_token_account(),
            KindPred::NotMint => !kind.inner_name_matches(&["Mint", "Mint2022"]),
            KindPred::NotOptional => extract_generic_inner_type(&field.ty, "Option").is_none(),
        }
    }
}

/// A single composition rule.
#[derive(Debug)]
enum Rule {
    /// `when` and `other` cannot both be present.
    Incompatible { a: Pred, b: Pred, msg: &'static str },
    /// If `when` is present, `requires` must also be present.
    Requires {
        when: Pred,
        requires: Pred,
        msg: &'static str,
    },
    /// If `when` is present, the field kind must match.
    RequiresKind {
        when: Pred,
        kind: KindPred,
        msg: &'static str,
    },
    /// If `when` is present, `other` must also be present (symmetric).
    Paired { a: Pred, b: Pred, msg: &'static str },
    /// If `when` is present but `unless` is NOT present, error.
    RequiresAny {
        when: Pred,
        any_of: &'static [Pred],
        msg: &'static str,
    },
}

/// The complete composition rule table. An auditor reviews this single
/// array to understand every constraint-combination restriction.
static COMPOSITION_RULES: &[Rule] = &[
    // --- Mutual exclusions ---
    Rule::Incompatible {
        a: Pred::AnyInit,
        b: Pred::Close,
        msg: "#[account(init)] and #[account(close)] cannot be used on the same field",
    },
    Rule::Incompatible {
        a: Pred::Init,
        b: Pred::InitIfNeeded,
        msg: "#[account(init)] and #[account(init_if_needed)] are mutually exclusive",
    },
    Rule::Incompatible {
        a: Pred::Realloc,
        b: Pred::AnyInit,
        msg: "#[account(realloc)] and #[account(init)] cannot be used on the same field",
    },
    Rule::Incompatible {
        a: Pred::TokenMint,
        b: Pred::AtaMint,
        msg: "`token::*` and `associated_token::*` cannot be used on the same field",
    },
    Rule::Incompatible {
        a: Pred::Seeds,
        b: Pred::AtaMint,
        msg: "`seeds` and `associated_token::*` cannot be used on the same field",
    },
    // --- Co-requirements ---
    Rule::Requires {
        when: Pred::Payer,
        requires: Pred::AnyInit,
        msg: "`payer` requires `init` or `init_if_needed`",
    },
    Rule::Requires {
        when: Pred::Space,
        requires: Pred::AnyInit,
        msg: "`space` requires `init` or `init_if_needed`",
    },
    Rule::Requires {
        when: Pred::ReallocPayer,
        requires: Pred::Realloc,
        msg: "`realloc::payer` requires `realloc`",
    },
    Rule::Requires {
        when: Pred::MetadataAny,
        requires: Pred::AnyInit,
        msg: "`metadata::*` attributes require `init` or `init_if_needed`",
    },
    Rule::Requires {
        when: Pred::MasterEditionMaxSupply,
        requires: Pred::AnyInit,
        msg: "`master_edition::max_supply` requires `init` or `init_if_needed`",
    },
    Rule::Requires {
        when: Pred::MasterEditionMaxSupply,
        requires: Pred::MetadataName,
        msg: "`master_edition::max_supply` requires `metadata::name`, `metadata::symbol`, and \
              `metadata::uri`",
    },
    // --- Paired attributes (both or neither) ---
    Rule::Paired {
        a: Pred::TokenMint,
        b: Pred::TokenAuthority,
        msg: "`token::mint` and `token::authority` must both be specified",
    },
    Rule::Paired {
        a: Pred::AtaMint,
        b: Pred::AtaAuthority,
        msg: "`associated_token::mint` and `associated_token::authority` must both be specified",
    },
    // --- Context requirements ---
    Rule::Requires {
        when: Pred::AtaTokenProgram,
        requires: Pred::AtaMint,
        msg: "`associated_token::token_program` requires `associated_token::mint` and \
              `associated_token::authority`",
    },
    Rule::RequiresAny {
        when: Pred::TokenTokenProgram,
        any_of: &[Pred::TokenMint, Pred::Sweep, Pred::Close],
        msg: "`token::token_program` requires `token::mint`/`token::authority`, `sweep`, or token \
              account `close`",
    },
    Rule::RequiresAny {
        when: Pred::MintTokenProgram,
        any_of: &[Pred::MintDecimals, Pred::MasterEditionMaxSupply],
        msg: "`mint::token_program` requires `mint::decimals` or `master_edition::max_supply`",
    },
    // --- Metadata completeness ---
    Rule::Requires {
        when: Pred::MetadataAny,
        requires: Pred::MetadataName,
        msg: "`metadata::name`, `metadata::symbol`, and `metadata::uri` must all be specified",
    },
    Rule::Requires {
        when: Pred::MetadataAny,
        requires: Pred::MetadataSymbol,
        msg: "`metadata::name`, `metadata::symbol`, and `metadata::uri` must all be specified",
    },
    Rule::Requires {
        when: Pred::MetadataAny,
        requires: Pred::MetadataUri,
        msg: "`metadata::name`, `metadata::symbol`, and `metadata::uri` must all be specified",
    },
    // --- Kind restrictions ---
    Rule::RequiresKind {
        when: Pred::Realloc,
        kind: KindPred::Account,
        msg: "#[account(realloc)] is only valid on Account<T> fields",
    },
    Rule::RequiresKind {
        when: Pred::Realloc,
        kind: KindPred::NotOptional,
        msg: "#[account(realloc)] cannot be used on Option<Account<T>> fields",
    },
    Rule::RequiresKind {
        when: Pred::Sweep,
        kind: KindPred::TokenAccount,
        msg: "#[account(sweep)] is only valid on token accounts, not mint accounts",
    },
    Rule::RequiresKind {
        when: Pred::Close,
        kind: KindPred::NotMint,
        msg: "#[account(close)] cannot be used on mint accounts. Mint closing is not supported \
              through the token-account close path.",
    },
];

/// Validate constraint composition rules for a single field.
///
/// Returns a compile error on the first violated rule. Each rule is
/// defined in the [`COMPOSITION_RULES`] table above.
pub(super) fn validate_composition(
    field: &syn::Field,
    field_name: &Ident,
    attrs: &AccountFieldAttrs,
    kind: &FieldKind,
) -> Result<(), proc_macro::TokenStream> {
    for rule in COMPOSITION_RULES {
        match rule {
            Rule::Incompatible { a, b, msg } => {
                if a.matches(attrs) && b.matches(attrs) {
                    return Err(syn::Error::new_spanned(field_name, *msg)
                        .to_compile_error()
                        .into());
                }
            }
            Rule::Requires {
                when,
                requires,
                msg,
            } => {
                if when.matches(attrs) && !requires.matches(attrs) {
                    return Err(syn::Error::new_spanned(field_name, *msg)
                        .to_compile_error()
                        .into());
                }
            }
            Rule::RequiresKind {
                when,
                kind: kp,
                msg,
            } => {
                if when.matches(attrs) && !kp.matches(kind, field) {
                    return Err(syn::Error::new_spanned(field_name, *msg)
                        .to_compile_error()
                        .into());
                }
            }
            Rule::Paired { a, b, msg } => {
                if a.matches(attrs) != b.matches(attrs) {
                    return Err(syn::Error::new_spanned(field_name, *msg)
                        .to_compile_error()
                        .into());
                }
            }
            Rule::RequiresAny { when, any_of, msg } => {
                if when.matches(attrs) && !any_of.iter().any(|p| p.matches(attrs)) {
                    return Err(syn::Error::new_spanned(field_name, *msg)
                        .to_compile_error()
                        .into());
                }
            }
        }
    }

    // The dup doc-comment check doesn't fit the table pattern (it reads
    // field attributes, not constraint attrs). Keep it as a direct check.
    if attrs.dup {
        let has_doc = field.attrs.iter().any(|a| a.path().is_ident("doc"));
        if !has_doc {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[account(dup)] requires a /// CHECK: <reason> doc comment explaining why this \
                 account is safe to use as a duplicate.",
            )
            .to_compile_error()
            .into());
        }
    }

    // Realloc on token/mint is not expressible via KindPred alone
    // (it's a property of the inner type, not the wrapper).
    if attrs.realloc.is_some() && kind.is_token_or_mint() {
        return Err(syn::Error::new_spanned(
            field_name,
            "#[account(realloc)] cannot be used on token or mint accounts — their size is fixed \
             by the token program",
        )
        .to_compile_error()
        .into());
    }

    Ok(())
}
