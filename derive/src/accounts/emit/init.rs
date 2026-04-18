use {
    super::super::{
        resolve::{
            FieldSemantics, FieldShape, InitMode, LifecycleConstraint, PdaConstraint,
            ReallocConstraint,
        },
        syntax::SeedRenderContext,
    },
    quote::{format_ident, quote},
};

pub(super) fn require_ident(
    ident: Option<syn::Ident>,
    field: &syn::Ident,
    message: &str,
) -> syn::Result<syn::Ident> {
    ident.ok_or_else(|| syn::Error::new(field.span(), message))
}

pub(super) fn emit_init_stmts(
    semantics: &[FieldSemantics],
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let mut stmts = Vec::new();

    for sem in semantics {
        if sem.init.is_some() {
            stmts.push(emit_one_init(sem, semantics)?);
        }
    }

    Ok(stmts)
}

pub(super) fn emit_realloc_steps(
    semantics: &[FieldSemantics],
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    semantics
        .iter()
        .filter_map(|sem| sem.realloc.as_ref().map(|rc| (sem, rc)))
        .map(|(sem, rc)| emit_one_realloc(sem, rc))
        .collect()
}

pub(super) fn emit_epilogue(semantics: &[FieldSemantics]) -> syn::Result<proc_macro2::TokenStream> {
    let mut sweep_stmts = Vec::new();
    let mut close_stmts = Vec::new();

    for sem in semantics {
        let field = &sem.core.ident;
        for lifecycle in &sem.lifecycle {
            match lifecycle {
                LifecycleConstraint::Sweep { receiver } => {
                    let authority = token_authority(sem).cloned().ok_or_else(|| {
                        syn::Error::new(field.span(), "sweep requires token::authority")
                    })?;
                    let mint = token_mint(sem).cloned().ok_or_else(|| {
                        syn::Error::new(field.span(), "sweep requires token::mint")
                    })?;
                    let token_program = token_program(sem).ok_or_else(|| {
                        syn::Error::new(field.span(), "sweep requires a token program field")
                    })?;
                    sweep_stmts.push(quote! {
                        quasar_spl::sweep_token_account(
                            self.#token_program.to_account_view(),
                            self.#field.to_account_view(),
                            self.#mint.to_account_view(),
                            self.#receiver.to_account_view(),
                            self.#authority.to_account_view(),
                        )?;
                    });
                }
                LifecycleConstraint::Close { destination } => {
                    if let (Some(authority), Some(token_program)) =
                        (token_authority(sem).cloned(), token_program(sem))
                    {
                        close_stmts.push(quote! {
                            quasar_spl::close_token_account(
                                self.#token_program.to_account_view(),
                                self.#field.to_account_view(),
                                self.#destination.to_account_view(),
                                self.#authority.to_account_view(),
                            )?;
                        });
                    } else {
                        match &sem.core.shape {
                            FieldShape::Account { .. }
                            | FieldShape::InterfaceAccount { .. }
                            | FieldShape::SystemAccount
                            | FieldShape::Other => {
                                close_stmts.push(quote! {
                                    self.#field.close(self.#destination.to_account_view())?;
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    if sweep_stmts.is_empty() && close_stmts.is_empty() {
        return Ok(quote! {});
    }

    Ok(quote! {
        #[inline(always)]
        fn epilogue(&mut self) -> Result<(), ProgramError> {
            #(#sweep_stmts)*
            #(#close_stmts)*
            Ok(())
        }
    })
}

pub(super) fn emit_non_init_check(sem: &FieldSemantics) -> Option<proc_macro2::TokenStream> {
    let field = &sem.core.ident;

    if let Some(tc) = &sem.token {
        let ty = &sem.core.effective_ty;
        let mint = &tc.mint;
        let auth = &tc.authority;
        let token_program = token_program_expr(sem);
        return Some(quote! {
            {
                let mut __params = <#ty as quasar_lang::account_load::AccountLoad>::Params::default();
                __params.mint = Some(*#mint.to_account_view().address());
                __params.authority = Some(*#auth.to_account_view().address());
                __params.token_program = Some(*#token_program);
                quasar_lang::account_load::AccountLoad::validate(#field, &__params)?;
            }
        });
    }

    if let Some(ac) = &sem.ata {
        let wallet = &ac.authority;
        let mint = &ac.mint;
        let token_program = token_program_expr(sem);
        return Some(quote! {
            quasar_spl::validate_ata(
                #field.to_account_view(),
                #wallet.to_account_view().address(),
                #mint.to_account_view().address(),
                #token_program,
            )?;
        });
    }

    sem.mint.as_ref().map(|mc| {
        let ty = &sem.core.effective_ty;
        let decimals = &mc.decimals;
        let auth = &mc.authority;
        let freeze_expr = mint_freeze_load_expr(&mc.freeze_authority);
        let token_program = token_program_expr(sem);
        quote! {
            {
                let mut __params = <#ty as quasar_lang::account_load::AccountLoad>::Params::default();
                __params.authority = Some(*#auth.to_account_view().address());
                __params.decimals = Some((#decimals) as u8);
                __params.freeze_authority = #freeze_expr;
                __params.token_program = Some(*#token_program);
                quasar_lang::account_load::AccountLoad::validate(#field, &__params)?;
            }
        }
    })
}

pub(super) fn token_authority(sem: &FieldSemantics) -> Option<&syn::Ident> {
    sem.token
        .as_ref()
        .map(|tc| &tc.authority)
        .or_else(|| sem.ata.as_ref().map(|ac| &ac.authority))
}

pub(super) fn token_mint(sem: &FieldSemantics) -> Option<&syn::Ident> {
    sem.token
        .as_ref()
        .map(|tc| &tc.mint)
        .or_else(|| sem.ata.as_ref().map(|ac| &ac.mint))
}

pub(super) fn token_program(sem: &FieldSemantics) -> Option<&syn::Ident> {
    sem.support.token_program.as_ref()
}

fn emit_one_init(
    sem: &FieldSemantics,
    all_semantics: &[FieldSemantics],
) -> syn::Result<proc_macro2::TokenStream> {
    let field = &sem.core.ident;
    let init = sem.init.as_ref().expect("checked by caller");
    let guard = matches!(init.mode, InitMode::InitIfNeeded);
    let payer = require_ident(
        sem.support.payer.clone(),
        field,
        "init requires a payer field",
    )?;

    let (signers_setup, signers_ref) = emit_signers(field, sem.pda.as_ref(), all_semantics);

    if let Some(token_init) = emit_token_init(sem, guard, &payer, &signers_setup, &signers_ref)? {
        return Ok(token_init);
    }

    let inner_ty = match &sem.core.shape {
        FieldShape::Account { inner_ty } | FieldShape::InterfaceAccount { inner_ty } => inner_ty,
        _ => &sem.core.effective_ty,
    };
    let inner_base = crate::helpers::strip_generics(inner_ty);
    let space_expr = if let Some(space) = &init.space {
        quote! { (#space) as u64 }
    } else {
        quote! { <#inner_base as quasar_lang::traits::Space>::SPACE as u64 }
    };
    let cpi_body = quote! {
        #signers_setup
        quasar_lang::account_init::init_account(
            #payer, #field, #space_expr,
            __program_id, #signers_ref, &__shared_rent,
            <#inner_base as quasar_lang::traits::Discriminator>::DISCRIMINATOR,
        )?;
    };
    let validate = if guard {
        Some(quote! {
            <#inner_base as quasar_lang::traits::CheckOwner>::check_owner(#field.to_account_view())?;
            <#inner_base as quasar_lang::traits::AccountCheck>::check(#field.to_account_view())?;
        })
    } else {
        None
    };
    Ok(wrap_init_guard(field, guard, cpi_body, validate))
}

fn emit_token_init(
    sem: &FieldSemantics,
    guard: bool,
    payer: &syn::Ident,
    signers_setup: &proc_macro2::TokenStream,
    signers_ref: &proc_macro2::TokenStream,
) -> syn::Result<Option<proc_macro2::TokenStream>> {
    let field = &sem.core.ident;

    if let Some(ac) = &sem.ata {
        let authority = &ac.authority;
        let mint = &ac.mint;
        let ata_program = require_ident(
            sem.support.associated_token_program.as_ref().cloned(),
            field,
            "#[account(init, associated_token::...)] requires an AssociatedTokenProgram field",
        )?;
        let token_program = require_ident(
            sem.support.token_program.as_ref().cloned(),
            field,
            "ATA init requires a token program field",
        )?;
        let system_program = require_ident(
            sem.support.system_program.as_ref().cloned(),
            field,
            "ATA init requires a System program field",
        )?;

        let cpi_body = quote! {
            quasar_spl::init_ata(
                #ata_program, #payer, #field, #authority, #mint,
                #system_program, #token_program, #guard,
            )?;
        };
        let validate = quote! {
            quasar_spl::validate_ata(
                #field.to_account_view(),
                #authority.to_account_view().address(),
                #mint.to_account_view().address(),
                #token_program.address(),
            )?;
        };
        return Ok(Some(wrap_init_guard(
            field,
            guard,
            cpi_body,
            Some(validate),
        )));
    }

    if let Some(tc) = &sem.token {
        let mint = &tc.mint;
        let authority = &tc.authority;
        let token_program = require_ident(
            sem.support.token_program.as_ref().cloned(),
            field,
            "Token init requires a token program field",
        )?;
        let cpi_body = quote! {
            #signers_setup
            quasar_spl::init_token_account(
                #payer, #field, #token_program, #mint,
                #authority.address(), #signers_ref, &__shared_rent,
            )?;
        };
        let validate = quote! {
            quasar_spl::validate_token_account(
                #field.to_account_view(),
                #mint.to_account_view().address(),
                #authority.to_account_view().address(),
                #token_program.address(),
            )?;
        };
        return Ok(Some(wrap_init_guard(
            field,
            guard,
            cpi_body,
            Some(validate),
        )));
    }

    let Some(mc) = &sem.mint else {
        return Ok(None);
    };

    let decimals = &mc.decimals;
    let authority = &mc.authority;
    let token_program = require_ident(
        sem.support.token_program.as_ref().cloned(),
        field,
        "Mint init requires a token program field",
    )?;
    let freeze_init = mint_freeze_address_expr(&mc.freeze_authority);
    let freeze_validate = mint_freeze_validate_expr(&mc.freeze_authority);
    let cpi_body = quote! {
        #signers_setup
        quasar_spl::init_mint_account(
            #payer, #field, #token_program,
            (#decimals) as u8, #authority.address(), #freeze_init,
            #signers_ref, &__shared_rent,
        )?;
    };
    let validate = quote! {
        quasar_spl::validate_mint(
            #field.to_account_view(),
            #authority.to_account_view().address(),
            (#decimals) as u8,
            #freeze_validate,
            #token_program.address(),
        )?;
    };
    Ok(Some(wrap_init_guard(
        field,
        guard,
        cpi_body,
        Some(validate),
    )))
}

fn emit_signers(
    field: &syn::Ident,
    pda: Option<&PdaConstraint>,
    all_semantics: &[FieldSemantics],
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let Some(pda) = pda else {
        return (quote! {}, quote! { &[] });
    };

    let bump_var = format_ident!("__bumps_{}", field);
    let bindings = super::parse::emit_seed_bindings(
        field,
        pda,
        all_semantics,
        SeedRenderContext::Init,
        "init_seed",
    );
    let seed_lets = bindings.seed_lets;
    let seed_idents = bindings.seed_idents;
    let seed_array_name = format_ident!("__init_seed_refs_{}", field);
    let explicit_bump_name = format_ident!("__init_bump_{}", field);
    let pda_assign = super::parse::emit_pda_bump_assignment(
        field,
        pda,
        &seed_idents,
        super::parse::PdaBumpAssignment {
            bump_var: &bump_var,
            addr_expr: &quote! { #field.address() },
            seed_array_name: &seed_array_name,
            explicit_bump_name: &explicit_bump_name,
            bare_mode: super::parse::PdaBareMode::DeriveExpected,
            log_failure: false,
        },
    );

    (
        quote! {
            #(#seed_lets)*
            #pda_assign
            let __init_bump_ref: &[u8] = &[#bump_var];
            let __init_signer_seeds = [#(quasar_lang::cpi::Seed::from(#seed_idents),)* quasar_lang::cpi::Seed::from(__init_bump_ref)];
            let __init_signers = [quasar_lang::cpi::Signer::from(&__init_signer_seeds[..])];
        },
        quote! { &__init_signers },
    )
}

fn emit_one_realloc(
    sem: &FieldSemantics,
    rc: &ReallocConstraint,
) -> syn::Result<proc_macro2::TokenStream> {
    let field = &sem.core.ident;
    let space = &rc.space_expr;
    let payer = sem
        .support
        .realloc_payer
        .clone()
        .ok_or_else(|| syn::Error::new(field.span(), "realloc requires a payer field"))?;

    Ok(quote! {
        {
            let __realloc_space = (#space) as usize;
            quasar_lang::accounts::realloc_account(
                #field, __realloc_space, #payer, Some(&__shared_rent)
            )?;
        }
    })
}

fn token_program_expr(sem: &FieldSemantics) -> syn::Expr {
    match token_program(sem) {
        Some(token_program) => syn::parse_quote!(#token_program.to_account_view().address()),
        None => syn::parse_quote!(&quasar_spl::SPL_TOKEN_ID),
    }
}

fn mint_freeze_address_expr(freeze_authority: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    match freeze_authority {
        Some(freeze_authority) => quote! { Some(#freeze_authority.address()) },
        None => quote! { None },
    }
}

fn mint_freeze_load_expr(freeze_authority: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    match freeze_authority {
        Some(freeze_authority) => quote! { Some(*#freeze_authority.to_account_view().address()) },
        None => quote! { None },
    }
}

fn mint_freeze_validate_expr(freeze_authority: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    match freeze_authority {
        Some(freeze_authority) => quote! { Some(#freeze_authority.to_account_view().address()) },
        None => quote! { None },
    }
}

pub(super) fn wrap_init_guard(
    field: &syn::Ident,
    idempotent: bool,
    cpi_body: proc_macro2::TokenStream,
    validate_existing: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    if idempotent {
        let validate = validate_existing.unwrap_or_default();
        quote! {
            {
                if quasar_lang::is_system_program(#field.owner()) {
                    #cpi_body
                } else {
                    #validate
                }
            }
        }
    } else {
        quote! {
            {
                if !quasar_lang::is_system_program(#field.owner()) {
                    return Err(ProgramError::AccountAlreadyInitialized);
                }
                #cpi_body
            }
        }
    }
}
