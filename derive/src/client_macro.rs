//! Client instruction macro generation for `#[derive(Accounts)]` structs.

use {
    proc_macro2::TokenStream,
    quasar_schema::pascal_to_snake,
    quote::{format_ident, quote},
};

/// Internal account descriptor for client macro generation.
struct AccountDescriptor {
    name: syn::Ident,
    writable: bool,
    signer: TokenStream,
    /// Const address expression for `Program<T>`/`Sysvar<T>` fields. These
    /// accounts have exactly one canonical address, so the client fills it
    /// in and the instruction struct drops the field.
    fixed_address: Option<TokenStream>,
    /// Synthetic typed inputs replacing a derived field whose seeds read
    /// stored account data: `(input ident, definition-site type tokens)`.
    seed_inputs: Vec<(syn::Ident, TokenStream)>,
}

pub fn generate_accounts_macro(
    name: &syn::Ident,
    generics: &syn::Generics,
    plan: &crate::accounts::resolve::specs::AccountsPlanTyped,
) -> TokenStream {
    let krate = crate::krate::lang_path();
    let descriptors = describe_accounts(name, generics, plan);
    let macro_name = format_ident!("__{}_instruction", pascal_to_snake(&name.to_string()));
    let module_name = format_ident!("__{}_client_macro", pascal_to_snake(&name.to_string()));
    // Two derived fields may share a stored-data seed root (a chained field
    // inherits its base's inputs); the input appears once, at first use.
    let mut seen_inputs: Vec<syn::Ident> = Vec::new();
    let mut descriptors = descriptors;
    for descriptor in &mut descriptors {
        descriptor
            .seed_inputs
            .retain(|(input, _)| !seen_inputs.contains(input));
        for (input, _) in &descriptor.seed_inputs {
            seen_inputs.push(input.clone());
        }
    }
    let descriptors = descriptors;
    let account_fields: Vec<_> = descriptors
        .iter()
        .map(|descriptor| emit_account_field(name, descriptor))
        .collect();
    let account_metas: Vec<_> = descriptors.iter().map(emit_account_meta).collect();
    let seed_input_aliases: Vec<_> = descriptors
        .iter()
        .flat_map(|descriptor| {
            descriptor.seed_inputs.iter().map(|(input, alias)| {
                let realias = seed_input_realias(name, input);
                quote! {
                    #[doc(hidden)]
                    #[allow(unexpected_cfgs)]
                    #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
                    pub type #realias = #alias;
                }
            })
        })
        .collect();

    quote! {
        #(#seed_input_aliases)*

        #[doc(hidden)]
        #[allow(unexpected_cfgs)]
        mod #module_name {
            #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
            #[macro_export]
            macro_rules! #macro_name {
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                }

                impl From<$struct_name> for #krate::client::Instruction {
                    #[allow(unused_variables)]
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::SerializeArg>::serialize_arg(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, compact) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                }

                impl From<$struct_name> for #krate::client::Instruction {
                    #[allow(unused_variables)]
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_header(&ix.$arg_name)
                                );
                            )*
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_tail(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, remaining) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                    pub remaining_accounts: ::alloc::vec::Vec<#krate::client::AccountMeta>,
                }

                impl From<$struct_name> for #krate::client::Instruction {
                    #[allow(unused_variables)]
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let mut accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        accounts.extend(ix.remaining_accounts);
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::SerializeArg>::serialize_arg(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
            ($struct_name:ident, [$($disc:expr),*], {$($arg_name:ident : $arg_ty:ty),*}, compact, remaining) => {
                pub struct $struct_name {
                    #(#account_fields)*
                    $(pub $arg_name: $arg_ty,)*
                    pub remaining_accounts: ::alloc::vec::Vec<#krate::client::AccountMeta>,
                }

                impl From<$struct_name> for #krate::client::Instruction {
                    #[allow(unused_variables)]
                    fn from(ix: $struct_name) -> #krate::client::Instruction {
                        let mut accounts = ::alloc::vec![
                            #(#account_metas)*
                        ];
                        accounts.extend(ix.remaining_accounts);
                        let data = {
                            let mut _data = ::alloc::vec![$($disc),*];
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_header(&ix.$arg_name)
                                );
                            )*
                            $(
                                _data.extend_from_slice(
                                    &<$arg_ty as #krate::client::CompactSerializeArg>::compact_tail(&ix.$arg_name)
                                );
                            )*
                            _data
                        };
                        #krate::client::Instruction {
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }
                    }
                }
            };
            }
        }
    }
}

fn emit_account_field(name: &syn::Ident, descriptor: &AccountDescriptor) -> TokenStream {
    if descriptor.fixed_address.is_some() {
        // A derived field whose seeds read stored account data is replaced by
        // typed inputs carrying those values (via definition-site re-aliases,
        // so the type resolves inside the cpi module).
        let inputs = descriptor.seed_inputs.iter().map(|(input, _)| {
            let realias = seed_input_realias(name, input);
            quote! { pub #input: #realias, }
        });
        return quote! { #(#inputs)* };
    }
    let krate = crate::krate::lang_path();
    let ident = &descriptor.name;
    quote! { pub #ident: #krate::prelude::Address, }
}

/// The definition-site re-alias for one synthetic seed input, scoped by the
/// accounts struct so sibling structs with identical fields don't collide in
/// the program module's glob imports.
fn seed_input_realias(accounts_struct: &syn::Ident, input: &syn::Ident) -> syn::Ident {
    format_ident!(
        "__QuasarSeedInput{}{}",
        accounts_struct,
        crate::helpers::snake_to_camel(&input.to_string())
    )
}

fn emit_account_meta(descriptor: &AccountDescriptor) -> TokenStream {
    let krate = crate::krate::lang_path();
    let ident = &descriptor.name;
    let signer = &descriptor.signer;
    let address = match &descriptor.fixed_address {
        Some(fixed) => fixed.clone(),
        None => quote! { ix.#ident },
    };
    if descriptor.writable {
        quote! {
            #krate::client::AccountMeta::new(#address, #signer),
        }
    } else {
        quote! {
            #krate::client::AccountMeta::new_readonly(#address, #signer),
        }
    }
}

fn describe_accounts(
    name: &syn::Ident,
    generics: &syn::Generics,
    plan: &crate::accounts::resolve::specs::AccountsPlanTyped,
) -> Vec<AccountDescriptor> {
    let static_lifetimes = generics.lifetimes().map(|_| quote! { 'static });
    // The macro only expands inside the generated `cpi` module, whose parent
    // is the `#[program]` module. `super::` reaches the accounts struct even
    // when a client struct in `cpi` shadows its name.
    let account_type = if generics.lifetimes().next().is_some() {
        quote! { super::#name::<#(#static_lifetimes),*> }
    } else {
        quote! { super::#name }
    };

    plan.fields
        .iter()
        .enumerate()
        .map(|(index, fp)| {
            let mut seed_inputs: Vec<(syn::Ident, TokenStream)> = Vec::new();
            let fixed_address = if fixed_address_expr(fp).is_some() {
                let const_ident = fixed_address_const(&fp.ident);
                Some(quote! { #account_type::#const_ident })
            } else if let Some(derivation) = field_derivation(plan, fp, &mut Vec::new()) {
                let fn_ident = pda_address_fn(&fp.ident);
                let roots = derivation_roots(plan, &derivation);
                seed_inputs = roots
                    .iter()
                    .filter_map(|root| match root {
                        DeriveRoot::SeedInput { input, alias } => {
                            Some((input.clone(), alias.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                let args = roots.iter().map(|root| match root {
                    DeriveRoot::Account(i) | DeriveRoot::ArgRef(i) => quote! { &ix.#i },
                    DeriveRoot::ArgValue(i, _) => quote! { ix.#i },
                    DeriveRoot::SeedInput { input, .. } => quote! { ix.#input },
                });
                Some(quote! { #account_type::#fn_ident(#(#args,)* &$crate::ID) })
            } else {
                None
            };
            AccountDescriptor {
                name: fp.ident.clone(),
                writable: fp.writable,
                signer: if fp.behavior_init_signer {
                    quote! { #account_type::__QUASAR_ACCOUNT_SIGNERS[#index] }
                } else {
                    let signer = fp.signer;
                    quote! { #signer }
                },
                fixed_address,
                seed_inputs,
            }
        })
        .collect()
}

/// How one input to a client-side address derivation resolves.
pub(crate) enum SeedSource<'p> {
    /// A plain account field that stays in the instruction struct: a
    /// `&Address` fn parameter, `&ix.field` at the call site.
    PlainAccount(&'p syn::Ident),
    /// Another derived field: a `let` local inside the derivation fn.
    DerivedAccount(&'p syn::Ident),
    /// An `Address`-typed instruction arg: `&Address` parameter, `&ix.name`.
    ArgRef(&'p syn::Ident),
    /// A by-value primitive instruction arg: `ty` parameter, `ix.name`.
    ArgValue(&'p syn::Ident, &'p syn::Type),
    /// A constant expression, resolvable at the definition site.
    Const(&'p syn::Expr),
    /// A value read from another account's stored data on-chain; the client
    /// takes it as a typed input field and derives with it (the same
    /// convention as the standalone IDL clients).
    FieldValue {
        /// The synthetic instruction-struct input, `{base}_{path}_seed`.
        input: syn::Ident,
        /// Type tokens naming the parameter's owned form through
        /// `SeedParam<INDEX>`, valid wherever the account type resolves.
        alias: TokenStream,
    },
}

/// A field's client-side address derivation, when one exists.
///
/// The derive stays protocol-neutral on-chain; both forms are client-codegen
/// conventions. Typed-seeds PDAs derive from `address = T::seeds(..)`.
/// Associated token accounts derive when a behavior group whose path ends in
/// `associated_token` maps `authority` + `mint` to resolvable fields, through
/// the behavior module's `client_address` fn; `token_program` joins when it
/// maps to a `Program<T>` field (its canonical const) and defaults inside the
/// behavior otherwise. Derivations chain: a seed may itself be a derived
/// field, so `vault` seeded by the derived `config` still resolves down to
/// `config`'s own plain roots.
pub(crate) enum FieldDerivation<'p> {
    Pda {
        account_ty: &'p syn::Path,
        seeds: Vec<SeedSource<'p>>,
    },
    Ata {
        behavior_path: &'p syn::Path,
        authority: SeedSource<'p>,
        mint: SeedSource<'p>,
        token_program: BehaviorProgramArg<'p>,
    },
}

/// Where an ATA derivation's token program comes from.
pub(crate) enum BehaviorProgramArg<'p> {
    /// A `Program<T>` field: its canonical const.
    Fixed(&'p syn::Ident),
    /// A plain field (e.g. `Interface<TokenInterface>`): the caller-supplied
    /// value, read off the instruction struct at build time.
    Field(&'p syn::Ident),
    /// No mapping: the behavior's default (SPL Token).
    Default,
}

pub(crate) fn field_derivation<'p>(
    plan: &'p crate::accounts::resolve::specs::AccountsPlanTyped,
    fp: &'p crate::accounts::resolve::specs::FieldPlan,
    stack: &mut Vec<syn::Ident>,
) -> Option<FieldDerivation<'p>> {
    use crate::accounts::resolve::specs::{IdlResolverPlan, IdlSeedPlan};
    if stack.contains(&fp.ident) {
        return None;
    }
    stack.push(fp.ident.clone());
    let derivation = (|| {
        if let Some(IdlResolverPlan::Pda { account_ty, seeds }) = fp.idl_resolver.as_ref() {
            let mut classified = Vec::with_capacity(seeds.len());
            for (index, seed) in seeds.iter().enumerate() {
                classified.push(match seed {
                    IdlSeedPlan::AccountAddr { base } => account_source(plan, base, stack)?,
                    IdlSeedPlan::Const { expr } => SeedSource::Const(expr),
                    IdlSeedPlan::IxArg { name, ty } => {
                        if is_address_type(ty) {
                            SeedSource::ArgRef(name)
                        } else if is_value_seed_type(ty) {
                            SeedSource::ArgValue(name, ty)
                        } else {
                            return None;
                        }
                    }
                    IdlSeedPlan::AccountField { base, field, .. } => SeedSource::FieldValue {
                        input: seed_input_ident(base, field),
                        alias: seed_alias_path(account_ty, index),
                    },
                });
            }
            // `find_address` (the owned-value path FieldValue requires) cannot
            // take a Const expr of unknown ownedness.
            let has_field_value = classified
                .iter()
                .any(|seed| matches!(seed, SeedSource::FieldValue { .. }));
            if has_field_value
                && classified
                    .iter()
                    .any(|seed| matches!(seed, SeedSource::Const(_)))
            {
                return None;
            }
            return Some(FieldDerivation::Pda {
                account_ty,
                seeds: classified,
            });
        }
        if fp.idl_resolver.is_some() {
            return None;
        }
        let group = fp
            .behaviors
            .iter()
            .find(|group| group.name.ends_with("associated_token"))?;
        let arg = |key: &str| {
            group
                .idl_account_args
                .iter()
                .find(|arg| arg.key == key)
                .map(|arg| &arg.field_ident)
        };
        let authority = account_source(plan, arg("authority")?, stack)?;
        let mint = account_source(plan, arg("mint")?, stack)?;
        let token_program = match arg("token_program") {
            Some(field)
                if find_field(plan, field).is_some_and(|f| fixed_address_expr(f).is_some()) =>
            {
                BehaviorProgramArg::Fixed(field)
            }
            // An interface or otherwise caller-chosen token program stays an
            // input field; the derivation reads its value at build time.
            Some(field)
                if matches!(
                    account_source(plan, field, stack)?,
                    SeedSource::PlainAccount(_)
                ) =>
            {
                BehaviorProgramArg::Field(field)
            }
            Some(_) => return None,
            None => BehaviorProgramArg::Default,
        };
        Some(FieldDerivation::Ata {
            behavior_path: &group.path,
            authority,
            mint,
            token_program,
        })
    })();
    stack.pop();
    derivation
}

/// The synthetic client input carrying a stored-data seed value.
fn seed_input_ident(base: &syn::Ident, path: &str) -> syn::Ident {
    let sanitized: String = path
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    format_ident!("{}_{}_seed", base, sanitized)
}

/// The owned seed-parameter type, named through the `SeedParam` trait so it
/// resolves wherever the account type does.
fn seed_alias_path(account_ty: &syn::Path, index: usize) -> TokenStream {
    let krate = crate::krate::lang_path();
    quote! { <#account_ty as #krate::traits::SeedParam<#index>>::Ty }
}

fn find_field<'p>(
    plan: &'p crate::accounts::resolve::specs::AccountsPlanTyped,
    ident: &syn::Ident,
) -> Option<&'p crate::accounts::resolve::specs::FieldPlan> {
    plan.fields.iter().find(|field| field.ident == *ident)
}

/// Resolve an account-field reference used as a derivation input.
fn account_source<'p>(
    plan: &'p crate::accounts::resolve::specs::AccountsPlanTyped,
    ident: &'p syn::Ident,
    stack: &mut Vec<syn::Ident>,
) -> Option<SeedSource<'p>> {
    let field = find_field(plan, ident)?;
    if fixed_address_expr(field).is_some() {
        return None;
    }
    if field_derivation(plan, field, stack).is_some() {
        return Some(SeedSource::DerivedAccount(ident));
    }
    Some(SeedSource::PlainAccount(ident))
}

/// A derivation's transitive inputs: the fn parameters at the definition
/// site and the `ix.*` arguments at the call site, deduplicated in
/// first-use order.
pub(crate) enum DeriveRoot<'p> {
    Account(&'p syn::Ident),
    ArgRef(&'p syn::Ident),
    ArgValue(&'p syn::Ident, &'p syn::Type),
    /// A stored-data seed value: a synthetic owned input field.
    SeedInput {
        input: syn::Ident,
        alias: TokenStream,
    },
}

impl<'p> DeriveRoot<'p> {
    pub(crate) fn ident(&self) -> &syn::Ident {
        match self {
            DeriveRoot::Account(i) | DeriveRoot::ArgRef(i) | DeriveRoot::ArgValue(i, _) => i,
            DeriveRoot::SeedInput { input, .. } => input,
        }
    }
}

pub(crate) fn derivation_roots<'p>(
    plan: &'p crate::accounts::resolve::specs::AccountsPlanTyped,
    derivation: &FieldDerivation<'p>,
) -> Vec<DeriveRoot<'p>> {
    let mut roots: Vec<DeriveRoot<'p>> = Vec::new();
    collect_roots(plan, derivation, &mut roots);
    roots
}

fn collect_roots<'p>(
    plan: &'p crate::accounts::resolve::specs::AccountsPlanTyped,
    derivation: &FieldDerivation<'p>,
    roots: &mut Vec<DeriveRoot<'p>>,
) {
    let source = |source: &SeedSource<'p>, roots: &mut Vec<DeriveRoot<'p>>| match source {
        SeedSource::PlainAccount(ident) => {
            if !roots.iter().any(|seen| seen.ident() == *ident) {
                roots.push(DeriveRoot::Account(ident));
            }
        }
        SeedSource::ArgRef(name) => {
            if !roots.iter().any(|seen| seen.ident() == *name) {
                roots.push(DeriveRoot::ArgRef(name));
            }
        }
        SeedSource::ArgValue(name, ty) => {
            if !roots.iter().any(|seen| seen.ident() == *name) {
                roots.push(DeriveRoot::ArgValue(name, ty));
            }
        }
        SeedSource::DerivedAccount(ident) => {
            let field = find_field(plan, ident).expect("derived field exists");
            let nested =
                field_derivation(plan, field, &mut Vec::new()).expect("derived field re-resolves");
            collect_roots(plan, &nested, roots);
        }
        SeedSource::FieldValue { input, alias } => {
            if !roots.iter().any(|seen| seen.ident() == input) {
                roots.push(DeriveRoot::SeedInput {
                    input: input.clone(),
                    alias: alias.clone(),
                });
            }
        }
        SeedSource::Const(_) => {}
    };
    match derivation {
        FieldDerivation::Pda { seeds, .. } => {
            for seed in seeds {
                source(seed, roots);
            }
        }
        FieldDerivation::Ata {
            authority,
            mint,
            token_program,
            ..
        } => {
            source(authority, roots);
            source(mint, roots);
            if let BehaviorProgramArg::Field(ident) = token_program {
                if !roots.iter().any(|seen| seen.ident() == *ident) {
                    roots.push(DeriveRoot::Account(ident));
                }
            }
        }
    }
}

/// The derived fields a derivation reads directly, in first-use order.
pub(crate) fn direct_derived_deps<'p>(derivation: &FieldDerivation<'p>) -> Vec<&'p syn::Ident> {
    let mut deps: Vec<&'p syn::Ident> = Vec::new();
    let mut visit = |source: &SeedSource<'p>| {
        if let SeedSource::DerivedAccount(ident) = source {
            if !deps.contains(ident) {
                deps.push(ident);
            }
        }
    };
    match derivation {
        FieldDerivation::Pda { seeds, .. } => seeds.iter().for_each(&mut visit),
        FieldDerivation::Ata {
            authority, mint, ..
        } => {
            visit(authority);
            visit(mint);
        }
    }
    deps
}

fn is_address_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(p) if p.path.is_ident("Address"))
}

/// Seed arg types `T::seeds` takes by value: the integer set and `[u8; N]`.
fn is_value_seed_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(p) => ["u8", "u16", "u32", "u64"]
            .iter()
            .any(|name| p.path.is_ident(name)),
        syn::Type::Array(array) => {
            matches!(array.elem.as_ref(), syn::Type::Path(p) if p.path.is_ident("u8"))
        }
        _ => false,
    }
}

/// The hidden associated fn deriving one field's address.
pub(crate) fn pda_address_fn(field: &syn::Ident) -> syn::Ident {
    format_ident!("__quasar_pda_{}", field)
}

/// The canonical-address expression for a `Program<T>`/`Sysvar<T>` field,
/// valid where the accounts struct (and `T`) is in scope.
pub(crate) fn fixed_address_expr(
    fp: &crate::accounts::resolve::specs::FieldPlan,
) -> Option<TokenStream> {
    use crate::accounts::resolve::specs::{FixedAddressSource, IdlResolverPlan};
    let krate = crate::krate::lang_path();
    match fp.idl_resolver.as_ref()? {
        IdlResolverPlan::FixedAddress { inner_ty, source } => Some(match source {
            FixedAddressSource::Program => quote! { <#inner_ty as #krate::traits::Id>::ID },
            FixedAddressSource::Sysvar => quote! { <#inner_ty as #krate::sysvars::Sysvar>::ID },
        }),
        IdlResolverPlan::Pda { .. } => None,
    }
}

/// The hidden associated const carrying one field's canonical address.
pub(crate) fn fixed_address_const(field: &syn::Ident) -> syn::Ident {
    format_ident!("__QUASAR_FIXED_ADDRESS_{}", field)
}
