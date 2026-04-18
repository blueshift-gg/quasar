use {
    super::{FieldSemantics, FieldShape},
    syn::Ident,
};

struct AccountsIndex {
    payer_default: Option<Ident>,
    system_program: Option<Ident>,
    token_program_candidates: Vec<Ident>,
    token_program_default: Option<Ident>,
    associated_token_program: Option<Ident>,
    rent_sysvar: Option<Ident>,
}

pub(super) fn resolve_supports(semantics: &mut [FieldSemantics]) -> syn::Result<()> {
    let index = build_accounts_index(semantics);

    for sem in semantics.iter_mut() {
        let explicit_payer = sem.init.as_ref().and_then(|init| init.payer.clone());
        let explicit_realloc_payer = sem.realloc.as_ref().and_then(|rc| rc.payer.clone());
        let explicit_token_program = sem
            .token
            .as_ref()
            .and_then(|tc| tc.token_program.clone())
            .or_else(|| sem.ata.as_ref().and_then(|ac| ac.token_program.clone()))
            .or_else(|| sem.mint.as_ref().and_then(|mc| mc.token_program.clone()));

        if sem.init.is_some() {
            sem.support.payer = explicit_payer
                .clone()
                .or_else(|| index.payer_default.clone());
            sem.support.system_program = index.system_program.clone();
            sem.support.rent_sysvar = index.rent_sysvar.clone();
        }

        if sem.realloc.is_some() {
            sem.support.realloc_payer = explicit_realloc_payer
                .or_else(|| sem.support.payer.clone())
                .or_else(|| index.payer_default.clone());
            sem.support.rent_sysvar = index.rent_sysvar.clone();
        }

        if uses_token_program(sem) {
            if explicit_token_program.is_none() && index.token_program_candidates.len() > 1 {
                return Err(syn::Error::new_spanned(
                    &sem.core.field,
                    "multiple token program fields found; specify `token::token_program`, \
                     `associated_token::token_program`, or `mint::token_program` explicitly",
                ));
            }
            sem.support.token_program =
                explicit_token_program.or_else(|| index.token_program_default.clone());
        }

        if sem.ata.is_some() {
            sem.support.associated_token_program = index.associated_token_program.clone();
            sem.support.system_program = index.system_program.clone();
        }
    }

    Ok(())
}

fn build_accounts_index(semantics: &[FieldSemantics]) -> AccountsIndex {
    let mut system_program = None;
    let mut token_program_candidates = Vec::new();
    let mut associated_token_program = None;
    let mut rent_sysvar = None;

    for sem in semantics {
        let ident = &sem.core.ident;

        match &sem.core.shape {
            FieldShape::Program { .. } => {
                if let Some(name) = sem.core.shape.inner_base_name() {
                    match name.to_string().as_str() {
                        "System" if system_program.is_none() => {
                            system_program = Some(ident.clone())
                        }
                        "Token" | "Token2022" => {
                            token_program_candidates.push(ident.clone());
                        }
                        "AssociatedTokenProgram" if associated_token_program.is_none() => {
                            associated_token_program = Some(ident.clone());
                        }
                        _ => {}
                    }
                }
            }
            FieldShape::Interface { .. }
                if sem.core.shape.inner_name_matches(&["TokenInterface"]) =>
            {
                token_program_candidates.push(ident.clone());
            }
            FieldShape::Sysvar { .. }
                if sem.core.shape.inner_name_matches(&["Rent"]) && rent_sysvar.is_none() =>
            {
                rent_sysvar = Some(ident.clone());
            }
            FieldShape::Account { .. } | FieldShape::InterfaceAccount { .. } => {}
            FieldShape::Composite => {}
            _ => {}
        }
    }

    let token_program_default = if token_program_candidates.len() == 1 {
        token_program_candidates.first().cloned()
    } else {
        None
    };

    AccountsIndex {
        payer_default: semantics
            .iter()
            .find(|sem| sem.core.ident == "payer")
            .map(|sem| sem.core.ident.clone()),
        system_program,
        token_program_candidates,
        token_program_default,
        associated_token_program,
        rent_sysvar,
    }
}

fn uses_token_program(sem: &FieldSemantics) -> bool {
    sem.token.is_some()
        || sem.ata.is_some()
        || sem.mint.is_some()
        || sem.has_sweep()
        || (sem.has_close() && (sem.token.is_some() || sem.ata.is_some()))
}
