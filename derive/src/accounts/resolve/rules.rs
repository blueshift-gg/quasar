use super::{FieldSemantics, FieldShape, PdaConstraint, PdaSource, SeedNode};

pub(super) fn validate_semantics(semantics: &[FieldSemantics]) -> syn::Result<()> {
    for sem in semantics {
        validate_field(sem)?;
    }
    Ok(())
}

fn validate_field(sem: &FieldSemantics) -> syn::Result<()> {
    let span = &sem.core.field;

    reject_field_rules(
        span,
        &[
            (
                sem.has_init() && sem.has_close(),
                "#[account(init)] and #[account(close)] cannot be used on the same field",
            ),
            (
                sem.has_realloc() && sem.has_init(),
                "#[account(realloc)] and #[account(init)] cannot be used on the same field",
            ),
            (
                sem.token.is_some() && sem.ata.is_some(),
                "`token::*` and `associated_token::*` cannot be used on the same field",
            ),
            (
                sem.pda.is_some() && sem.ata.is_some(),
                "`seeds` and `associated_token::*` cannot be used on the same field",
            ),
            (
                sem.has_sweep() && sem.token.is_none(),
                "#[account(sweep)] requires `token::mint` and `token::authority`",
            ),
            (
                sem.has_realloc() && !matches!(sem.core.shape, FieldShape::Account { .. }),
                "#[account(realloc)] is only valid on Account<T> fields",
            ),
            (
                sem.has_realloc() && sem.core.optional,
                "#[account(realloc)] cannot be used on Option<Account<T>> fields",
            ),
            (
                sem.has_realloc() && sem.core.shape.is_token_or_mint(),
                "#[account(realloc)] cannot be used on token or mint accounts — their size is \
                 fixed by the token program",
            ),
            (
                sem.has_sweep() && !sem.core.shape.is_token_account(),
                "#[account(sweep)] is only valid on token accounts, not mint accounts",
            ),
            (
                sem.has_close() && sem.core.shape.is_mint(),
                "#[account(close)] cannot be used on mint accounts. Mint closing is not supported \
                 through the token-account close path.",
            ),
        ],
    )?;

    if sem.core.dup {
        let has_doc = sem
            .core
            .field
            .attrs
            .iter()
            .any(|a| a.path().is_ident("doc"));
        if !has_doc {
            return Err(syn::Error::new_spanned(
                span,
                "#[account(dup)] requires a /// CHECK: <reason> doc comment explaining why this \
                 account is safe to use as a duplicate.",
            ));
        }
    }

    reject_field_rules(
        span,
        &[(
            sem.has_raw_pda()
                && matches!(sem.core.shape, FieldShape::Account { .. })
                && !sem.core.shape.is_token_or_mint()
                && !sem.has_init(),
            "Raw `seeds` are not allowed on `Account<T>` fields; use `typed_seeds` instead",
        )],
    )?;

    if let Some(pda) = &sem.pda {
        reject_field_rules(
            span,
            &[
                (
                    pda.bump.is_none(),
                    "PDA constraint requires a `bump` attribute",
                ),
                (
                    sem.has_init() && pda_references_field(pda, &sem.core.ident),
                    "PDA seeds for an `init` field cannot reference the account being initialized",
                ),
            ],
        )?;
    }

    if sem.has_init() {
        reject_field_rules(
            span,
            &[
                (
                    sem.support.payer.is_none(),
                    "`init` requires a payer. Add a `payer` field or specify `payer = <field>` on \
                     the init constraint.",
                ),
                (
                    sem.support.system_program.is_none(),
                    "`init` requires a `system_program` field of type `Program<System>`",
                ),
            ],
        )?;
    }

    Ok(())
}

fn reject_field_rules(span: &syn::Field, rules: &[(bool, &str)]) -> syn::Result<()> {
    for (reject, message) in rules {
        if *reject {
            return Err(syn::Error::new_spanned(span, *message));
        }
    }
    Ok(())
}

fn pda_references_field(pda: &PdaConstraint, field: &syn::Ident) -> bool {
    let seeds = match &pda.source {
        PdaSource::Raw { seeds } => seeds,
        PdaSource::Typed { args, .. } => args,
    };
    seeds.iter().any(|seed| seed_references_field(seed, field))
}

fn seed_references_field(seed: &SeedNode, field: &syn::Ident) -> bool {
    match seed {
        SeedNode::AccountAddress { field: seed_field } => seed_field == field,
        SeedNode::FieldBytes { root, .. } | SeedNode::FieldRootedExpr { root, .. } => root == field,
        SeedNode::Literal(_) | SeedNode::InstructionArg { .. } | SeedNode::OpaqueExpr(_) => false,
    }
}
