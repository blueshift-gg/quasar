//! `#[derive(Accounts)]`: one load or check per line, in field order. What a
//! field is, a wrapper, `Option`, `Remaining` or `Pda`, the `Field` and
//! `Seeded` traits say; the derive never reads a type's name.

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{quote, quote_spanned};
use syn::{
    Data, DeriveInput, Error, Expr, Field, Fields, Ident, Path, Result, Token, Type,
    parse::ParseStream, punctuated::Punctuated, spanned::Spanned,
};

/// What `#[account(..)]` declares on one field.
#[derive(Default)]
struct Constraints {
    writable: bool,
    init: Option<Init>,
    /// In the order written, which is the order they run.
    checks: Vec<Check>,
    /// Where each check is written, for the `debug` build to show.
    spans: Vec<Span>,
    /// Where the library's constraint is written.
    library_span: Option<Span>,
    /// A library's constraint, `token(mint = .., ..)`: checked, or with
    /// `init` how the account is created.
    library: Option<(Path, Vec<Key>)>,
    close: Option<Ident>,
    realloc: Option<Ident>,
}

/// One key of a library's constraint: its value, and the field the value
/// names if it names one.
type Key = (Ident, TokenStream, Option<Ident>);

enum Init {
    Create { payer: Ident, idempotent: bool },
    Migrate { from: Box<Type>, payer: Ident },
}

impl Init {
    fn payer(&self) -> &Ident {
        match self {
            Self::Create { payer, .. } | Self::Migrate { payer, .. } => payer,
        }
    }
}

enum Check {
    Address(Expr),
    HasOne(Ident, Option<Expr>),
    Holds(Box<Expr>, Option<Expr>),
    /// `seeds(values.., bump = ..)`: the field is a `Pda`.
    Seeds(Seeds),
}

struct Seeds {
    values: Vec<Expr>,
    bump: Option<Expr>,
    /// `seeds` without values: each is the account's field of the seed's
    /// name, or else the account of that name.
    by_name: bool,
}

impl Constraints {
    fn seeds(&self) -> Option<&Seeds> {
        self.checks.iter().find_map(|check| match check {
            Check::Seeds(seeds) => Some(seeds),
            _ => None,
        })
    }
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(input, "`Accounts` is derived on a struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new_spanned(input, "`Accounts` needs named fields"));
    };
    let name = &input.ident;
    let cursor = Ident::new("cursor", Span::mixed_site());
    let names: Vec<&Ident> = fields.named.iter().filter_map(|field| field.ident.as_ref()).collect();
    let types: Vec<&Type> = fields.named.iter().map(|field| &field.ty).collect();
    let data_type = data_type(input)?;
    // Programs a constraint calls are found by field name, as in Anchor.
    let system = names.iter().copied().find(|name| *name == "system_program");

    // Whether a field takes its seeds by name, from `__Named`.
    let mut by_name = false;
    let mut loads = Vec::new();
    // Each check a handler's calls can make instead, pushed if they do: what
    // the program's `elisions` lists.
    let mut remarks = Vec::new();
    // What a call to this instruction vouches for, per account.
    let mut enforces = Vec::new();
    let mut bare = Vec::new();
    let mut deferred = vec![Vec::new(); names.len()];
    let mut inits = Vec::new();
    let mut effects = Vec::new();
    let mut client = Vec::new();
    // Checks of the fields' types, evaluated where the struct is defined, so
    // an error points at the field whether or not an instruction uses it.
    let mut type_checks = Vec::new();
    // The fields a library's `init` takes, which it checks as it creates
    // the account: `token(mint = mint_a)` checks `mint_a` is a mint.
    let mut inits_take: Vec<(Ident, Path, String)> = Vec::new();
    for field in &fields.named {
        let constraints = parse_constraints(field, &names)?;
        if let (Some(Init::Create { .. }), Some((library, keys))) =
            (&constraints.init, &constraints.library)
        {
            for (key, _, of) in keys {
                if let Some(of) = of {
                    inits_take.push((of.clone(), library.clone(), key.to_string()));
                }
            }
        }
    }
    for (index, field) in fields.named.iter().enumerate() {
        let (name, ty) = (names[index], &field.ty);
        let constraints = parse_constraints(field, &names)?;
        let writable = constraints.writable || constraints.init.is_some();
        if data_type.is_none() && mentions_data(&constraints) {
            return Err(Error::new_spanned(
                name,
                "a constraint reads `data`: name its type with `#[data(..)]` on the struct",
            ));
        }
        if data_type.is_some() && *name == "data" {
            return Err(Error::new_spanned(name, "`data` is the instruction data here"));
        }

        // How the field loads: as declared, or as the account an `init`
        // creates or a `Pda` checks.
        let (load, signs) = match (&constraints.init, constraints.seeds()) {
            (Some(Init::Create { .. }), None) => (quote!(::quasar_lang::wrapper::Signer), true),
            (Some(_), _) => (quote!(::quasar_lang::wrapper::UncheckedAccount), false),
            (None, Some(_)) => (quote!(<#ty as ::quasar_lang::pda::Seeded>::Account), false),
            (None, None) => (quote!(#ty), false),
        };
        // Checked where the field's type is written, so an error points there.
        if constraints.seeds().is_none() && constraints.init.is_none() {
            type_checks.push(quote_spanned!(ty.span()=> ::quasar_lang::field::not_pda::<#ty>();));
        }
        if constraints.close.is_some() {
            type_checks
                .push(quote_spanned!(ty.span()=> ::quasar_lang::effects::closable::<#ty>();));
        }
        if constraints.realloc.is_some() {
            type_checks
                .push(quote_spanned!(ty.span()=> ::quasar_lang::effects::resizable::<#ty>();));
        }
        if has_constraints(&constraints) {
            type_checks
                .push(quote_spanned!(ty.span()=> ::quasar_lang::field::not_remaining::<#ty>();));
        }
        // Without the checks the handler's calls vouch for, decided when compiling.
        let field = name.to_string();
        let by_init =
            inits_take.iter().filter(|(taken, ..)| taken == name).map(|(_, library, key)| {
                quote! {
                    || ::quasar_lang::cpi::made_by_init::<#load>(
                        ::quasar_lang::cpi::key(<#library::Init>::KEYS, #key),
                    )
                }
            });
        // Where the field is declared, for the `debug` build to show when
        // the account does not load.
        let label = format!(
            "account #{index} of the instruction{}",
            if writable { ", which the program writes to" } else { "" }
        );
        let declared = located(&field, ty.span(), crate::account::type_name(ty).len(), &label);
        let waived = quote! {
            const {
                #[allow(unused_imports)]
                use ::quasar_lang::cpi::Unkeyed as _;
                ::quasar_lang::cpi::waived_field::<#load>(
                    <__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES,
                    #field,
                ) #(#by_init)*
            }
        };
        loads.push(quote! {
            let #name = <#load as ::quasar_lang::field::Field>::load_vouched(
                #cursor, #writable, #waived, #declared,
            )?;
        });
        remarks.push(quote! {
            if #waived {
                let check = <#load as ::quasar_lang::field::Field>::CHECK.unwrap_or_default();
                elided.push(::quasar_lang::client::format!("{}: {check}", #field));
            }
        });
        enforces.push(enforced(&load, writable, &constraints, &names));
        bare.push(match constraints.seeds() {
            Some(_) => quote! {
                let #name = <#ty as ::quasar_lang::pda::Seeded>::found(
                    <#load as ::quasar_lang::field::Field>::load(#cursor, #writable)?, 0,
                );
            },
            None => {
                quote!(let #name = <#ty as ::quasar_lang::field::Field>::load(#cursor, #writable)?;)
            }
        });

        // Checks: once every field they name has loaded, or after the `init`
        // that creates the account.
        by_name |= constraints.seeds().is_some_and(|seeds| seeds.by_name);
        let checks = checks(name, ty, &constraints, &names, &mut remarks);
        match &constraints.init {
            None => deferred[runs_after(&constraints, index, &names)?].extend(checks),
            Some(init) => {
                inits.push(init_step(name, ty, &constraints, init, system)?);
                inits.extend(checks);
            }
        }
        loads.append(&mut deferred[index]);

        effects.extend(effect(name, ty, &constraints, writable, system)?);
        client.push((name, if signs { load } else { quote!(#ty) }, writable));
    }

    let (impl_head, data_param) = match &data_type {
        Some(data) => {
            (quote!(impl ::quasar_lang::context::Accounts<#data> for #name), quote!(data: &#data))
        }
        None => (
            quote!(impl<__Data> ::quasar_lang::context::Accounts<__Data> for #name),
            quote!(_: &__Data),
        ),
    };
    let accounts = match &data_type {
        Some(data) => quote!(::quasar_lang::context::Accounts<#data>),
        None => quote!(::quasar_lang::context::Accounts<()>),
    };
    let effect_types = effects.iter().map(|effect| &effect.ty);
    let effect_values = effects.iter().map(|effect| &effect.value);
    let effect_index = (0..effects.len()).map(syn::Index::from);
    // An empty tuple is written as an empty body: `()` alone is a lint.
    let effect_values = (!effects.is_empty()).then(|| quote!((#(#effect_values,)*)));
    // The rent `init` and settling pay, from the instruction's `Sysvar<Rent>`
    // if it has one: no syscall then.
    let rent = rent_ident();
    let rent_source = |fields: Vec<TokenStream>| {
        let fields =
            fields.iter().map(|field| quote!(::quasar_lang::field::Field::rent_account(#field)));
        quote!(let #rent = ::quasar_lang::sysvar::RentSource::new(::core::option::Option::None #(.or(#fields))*);)
    };
    let rent_now =
        (!inits.is_empty()).then(|| rent_source(names.iter().map(|name| quote!(&#name)).collect()));
    let rent_later = effects
        .iter()
        .any(|effect| effect.pays_rent)
        .then(|| rent_source(names.iter().map(|name| quote!(&self.#name)).collect()));
    let named = by_name.then(|| named(&names));
    let cpi = cpi_accounts(name, &client, &enforces);
    let client = client_impl(name, &client);

    Ok(quote! {
        const _: () = {
            ::quasar_lang::field::required(&[#(<#types as ::quasar_lang::field::Field>::INFO),*]);
            #(#type_checks)*
        };

        #cpi

        #impl_head {
            type Deferred = (#(::core::option::Option<#effect_types>,)*);

            const FIELDS: &'static [::quasar_lang::field::FieldInfo] =
                &[#(<#types as ::quasar_lang::field::Field>::INFO),*];

            #[inline(always)]
            fn from_cursor<__Vouches: ::quasar_lang::cpi::Vouches>(
                #cursor: &mut ::quasar_lang::input::Cursor,
                #data_param,
            ) -> ::quasar_lang::Result<Self> {
                #named
                #cursor.require(const { ::quasar_lang::field::required(<Self as #accounts>::FIELDS) })?;
                #(#loads)*
                #rent_now
                #(#inits)*
                ::core::result::Result::Ok(Self { #(#names),* })
            }

            ::quasar_lang::kani_only! {
                fn load_accounts(
                    #cursor: &mut ::quasar_lang::input::Cursor,
                ) -> ::quasar_lang::Result<Self> {
                    #(#bare)*
                    ::core::result::Result::Ok(Self { #(#names),* })
                }
            }

            fn deferred(&self) -> Self::Deferred {
                #rent_later
                #effect_values
            }

            fn run_deferred(deferred: Self::Deferred) -> ::quasar_lang::Result {
                #(
                    if let ::core::option::Option::Some(effect) = deferred.#effect_index {
                        effect.run()?;
                    }
                )*
                ::core::result::Result::Ok(())
            }
        }

        #client

        ::quasar_lang::host_only! {
            impl #name {
                /// The checks the calls in `__Vouches` make, which loading
                /// these accounts for that handler leaves out.
                pub fn elided<__Vouches: ::quasar_lang::cpi::Vouches>()
                    -> ::quasar_lang::client::Vec<::quasar_lang::client::String>
                {
                    let mut elided = ::quasar_lang::client::Vec::new();
                    #(#remarks)*
                    elided
                }
            }
        }
    })
}

/// `__Named`, every field by reference, and for each that is an account its
/// `pda::Named` under the field's name.
fn named(names: &[&Ident]) -> TokenStream {
    let types: Vec<Ident> =
        (0..names.len()).map(|index| quote::format_ident!("T{index}")).collect();
    let impls = names.iter().enumerate().map(|(index, name)| {
        let (at, ty, member) = (name.to_string(), &types[index], syn::Index::from(index));
        quote! {
            impl<#(#types),*> ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name(#at) }>
                for __Named<'_, #(#types),*>
            where
                #ty: ::quasar_lang::wrapper::Wrapper,
            {
                fn named(&self) -> &::quasar_lang::Address {
                    ::quasar_lang::wrapper::Wrapper::address(self.#member)
                }
            }
        }
    });
    quote! {
        struct __Named<'a, #(#types),*>(#(&'a #types),*);
        #(#impls)*
    }
}

/// `#[data(T)]` on the struct: the instruction data its constraints read.
fn data_type(input: &DeriveInput) -> Result<Option<Type>> {
    let mut attrs = input.attrs.iter().filter(|attr| attr.path().is_ident("data"));
    let data = attrs.next().map(syn::Attribute::parse_args).transpose()?;
    if let Some(extra) = attrs.next() {
        return Err(Error::new_spanned(extra, "`#[data(..)]` is declared twice"));
    }
    Ok(data)
}

fn parse_constraints(field: &Field, names: &[&Ident]) -> Result<Constraints> {
    let mut constraints = Constraints::default();
    for attr in field.attrs.iter().filter(|attr| attr.path().is_ident("account")) {
        attr.parse_nested_meta(|meta| {
            let path = &meta.path;
            if path.is_ident("mut") {
                constraints.writable = true;
            } else if path.is_ident("init") {
                let (mut payer, mut idempotent) = (None, false);
                meta.parse_nested_meta(|inner| {
                    if inner.path.is_ident("payer") {
                        payer = Some(inner.value()?.parse()?);
                    } else if inner.path.is_ident("idempotent") {
                        idempotent = true;
                    } else {
                        return Err(inner.error("expected `payer = ..` or `idempotent`"));
                    }
                    Ok(())
                })?;
                let payer = payer.ok_or_else(|| meta.error("`init` needs `payer = ..`"))?;
                constraints.init = Some(Init::Create { payer, idempotent });
            } else if path.is_ident("migrate") {
                let (mut from, mut payer) = (None, None);
                meta.parse_nested_meta(|inner| {
                    if inner.path.is_ident("from") {
                        from = Some(inner.value()?.parse()?);
                    } else if inner.path.is_ident("payer") {
                        payer = Some(inner.value()?.parse()?);
                    } else {
                        return Err(inner.error("expected `from = ..` or `payer = ..`"));
                    }
                    Ok(())
                })?;
                let (Some(from), Some(payer)) = (from, payer) else {
                    return Err(meta.error("`migrate` needs `from = ..` and `payer = ..`"));
                };
                constraints.init = Some(Init::Migrate { from, payer });
            } else if path.is_ident("seeds") {
                if constraints.seeds().is_some() {
                    return Err(meta.error("`seeds` is declared twice"));
                }
                constraints.spans.push(path.span());
                constraints.checks.push(Check::Seeds(if meta.input.peek(syn::token::Paren) {
                    seeds(meta.input)?
                } else {
                    Seeds { values: Vec::new(), bump: None, by_name: true }
                }));
            } else if path.is_ident("address") {
                constraints.spans.push(path.span());
                constraints.checks.push(Check::Address(meta.value()?.parse()?));
            } else if path.is_ident("has_one") {
                let (target, error) = parenthesized(meta.input)?;
                constraints.spans.push(path.span());
                constraints.checks.push(Check::HasOne(target, error));
            } else if path.is_ident("check") {
                let (holds, error) = parenthesized(meta.input)?;
                constraints.spans.push(path.span());
                constraints.checks.push(Check::Holds(Box::new(holds), error));
            } else if path.is_ident("close") {
                constraints.close = Some(one_key(&meta, "to")?);
            } else if path.is_ident("realloc") {
                constraints.realloc = Some(one_key(&meta, "payer")?);
            } else if path.is_ident("space") {
                return Err(
                    meta.error("`init` sizes an account from its layout; there is no `space`")
                );
            } else if meta.input.peek(syn::token::Paren) {
                if constraints.library.is_some() {
                    return Err(meta.error("one field takes one library constraint"));
                }
                let mut keys = Vec::new();
                meta.parse_nested_meta(|inner| {
                    let key = inner.path.require_ident()?.clone();
                    let value: Expr = inner.value()?.parse()?;
                    let field = match &value {
                        Expr::Path(path) => {
                            path.path.get_ident().filter(|name| names.contains(name))
                        }
                        _ => None,
                    };
                    keys.push((key, entry_or_value(&value, names), field.cloned()));
                    Ok(())
                })?;
                constraints.library_span = Some(path.span());
                constraints.library = Some((path.clone(), keys));
            } else {
                return Err(meta.error(
                    "expected `mut`, `init(..)`, `migrate(..)`, `seeds(..)`, `address = ..`, \
                     `has_one(..)`, `check(..)`, `close(..)`, `realloc(..)` or a library's \
                     constraint such as `token(..)`",
                ));
            }
            Ok(())
        })?;
    }
    if constraints.seeds().is_some() && constraints.library.is_some() && constraints.init.is_some()
    {
        return Err(Error::new_spanned(
            &field.ident,
            "an `init` of a library's account takes no `seeds`",
        ));
    }
    if let (Some(_), Some(Seeds { bump: Some(bump), .. })) =
        (&constraints.init, constraints.seeds())
    {
        return Err(Error::new_spanned(bump, "`init` finds the canonical bump itself"));
    }
    if let (Some(_), Some(Seeds { by_name: true, .. })) = (&constraints.init, constraints.seeds()) {
        return Err(Error::new_spanned(
            &field.ident,
            "`init` creates the account, which stores nothing yet: write the values, `seeds(..)`",
        ));
    }
    Ok(constraints)
}

/// `(key = value)`.
fn one_key(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<Ident> {
    let mut value = None;
    meta.parse_nested_meta(|inner| {
        if inner.path.is_ident(key) {
            value = Some(inner.value()?.parse()?);
            Ok(())
        } else {
            Err(inner.error(format!("expected `{key} = ..`")))
        }
    })?;
    value.ok_or_else(|| meta.error(format!("expected `{key} = ..`")))
}

/// `(value, .., bump = expr)`.
fn seeds(input: ParseStream) -> Result<Seeds> {
    let content;
    syn::parenthesized!(content in input);
    let mut values = Vec::new();
    let mut bump = None;
    for expr in Punctuated::<Expr, Token![,]>::parse_terminated(&content)? {
        match expr {
            Expr::Assign(assign) if matches!(&*assign.left, Expr::Path(path) if path.path.is_ident("bump")) =>
            {
                bump = Some(*assign.right);
            }
            value => values.push(value),
        }
    }
    Ok(Seeds { values, bump, by_name: false })
}

/// `(value)` or `(value @ error)`.
fn parenthesized<T: syn::parse::Parse>(input: ParseStream) -> Result<(T, Option<Expr>)> {
    let content;
    syn::parenthesized!(content in input);
    let value = content.parse()?;
    let error = match content.parse::<Option<Token![@]>>()? {
        Some(_) => Some(content.parse()?),
        None => None,
    };
    Ok((value, error))
}

/// A library constraint's value: a field of the struct passes its entry.
fn entry_or_value(value: &Expr, names: &[&Ident]) -> TokenStream {
    match value {
        Expr::Path(path) if path.path.get_ident().is_some_and(|ident| names.contains(&ident)) => {
            quote!(::quasar_lang::wrapper::Wrapper::entry(&#value))
        }
        _ => quote!(#value),
    }
}

fn has_constraints(constraints: &Constraints) -> bool {
    let Constraints { init, checks, library, close, realloc, .. } = constraints;
    init.is_some()
        || !checks.is_empty()
        || library.is_some()
        || close.is_some()
        || realloc.is_some()
}

/// The field's checks, in the order written. A `Pda`'s address check turns
/// the loaded account into the `Pda`, unless `init` made it one.
fn checks(
    name: &Ident,
    ty: &Type,
    constraints: &Constraints,
    names: &[&Ident],
    remarks: &mut Vec<TokenStream>,
) -> Vec<TokenStream> {
    let field = name.to_string();
    let kind = quote!(<#ty as ::quasar_lang::field::Field>::CHECK);
    // Runs `check` unless a call the handler always makes makes `named`
    // about the field `of`.
    let mut unless_vouched = |check: TokenStream, named: TokenStream, of: String, shown: String| {
        let vouched = quote! {
            const {
                #[allow(unused_imports)]
                use ::quasar_lang::cpi::Unkeyed as _;
                ::quasar_lang::cpi::vouched(
                    <__Vouches as ::quasar_lang::cpi::Vouches>::VOUCHES,
                    #field,
                    #named,
                    #of,
                    #kind,
                )
            }
        };
        let shown = format!("{field}: {shown}");
        remarks.push(quote! {
            if #vouched {
                elided.push(::quasar_lang::client::String::from(#shown));
            }
        });
        (check, vouched)
    };
    let key = quote!(::quasar_lang::wrapper::Wrapper::address(&#name));
    // Where each check is written, for the `debug` build to show when it
    // fails: the constraint's name is underlined.
    let index = names.iter().position(|other| *other == name).unwrap_or_default();
    let label = format!("account #{index} of the instruction, `{name}`, does not satisfy this");
    let at = |span: Span, keyword: &str| located(&field, span, keyword.len(), &label);
    let spans = constraints.spans.iter().copied();
    let mut checks = Vec::new();
    for (check, span) in constraints.checks.iter().zip(spans) {
        let not_at_seeds = {
            let (error, at) = (
                format!(
                    "error[InvalidSeeds]: account `{name}` is not at the address its seeds derive"
                ),
                at(span, "seeds"),
            );
            quote! {
                #error,
                #at,
                "derive the address with `find_program_address` from the seeds its type declares in `#[seeds(..)]`, the values given here, and this program's id; a stored bump must be the one the account was created with"
            }
        };
        checks.push(match check {
            // `init` created the account at the seeds' address.
            Check::Seeds(_) if constraints.init.is_some() => continue,
            Check::Seeds(Seeds { by_name: true, .. }) => quote! {
                let #name = {
                    let bump = {
                        let named = __Named(#(&#names),*);
                        <<#ty as ::quasar_lang::pda::Seeded>::Seeds>::own_seeds(&#name, &named)
                            .bump_of(&#name, &crate::ID)
                    };
                    let bump = ::quasar_lang::error::explain(bump, #not_at_seeds)?;
                    <#ty as ::quasar_lang::pda::Seeded>::found(#name, bump)
                };
            },
            Check::Seeds(seeds) => {
                let seeds = seeds_value(ty, seeds);
                quote! {
                    let #name = {
                        let bump = ::quasar_lang::error::explain(
                            #seeds.bump_of(&#name, &crate::ID),
                            #not_at_seeds,
                        )?;
                        <#ty as ::quasar_lang::pda::Seeded>::found(#name, bump)
                    };
                }
            }
            Check::Address(address) => {
                let before = format!("error[Address]: account `{name}` is ");
                let at = at(span, "address");
                quote! {
                    ::quasar_lang::error::explain_same(
                        ::quasar_lang::relation::address(#key, &(#address)),
                        #before,
                        #key,
                        ", expected the address ",
                        &(#address),
                        #at,
                        "its field takes one fixed address: pass that account",
                    )?;
                }
            }
            Check::HasOne(target, error) => {
                let default = quote!(::quasar_lang::error::Error::HasOne);
                let error = error.as_ref().map_or(default, |error| quote!(#error));
                let at = at(span, "has_one");
                let before = format!("error[HasOne]: account `{name}` stores `{target}` = ");
                let between = format!(", but the account passed as `{target}` is ");
                let help = format!(
                    "`{name}` stores the address of its `{target}`: pass that account as \
                     `{target}`, or pass the `{name}` that belongs to this `{target}`"
                );
                let check = quote! {
                    ::quasar_lang::error::explain_same(
                        ::quasar_lang::relation::constraint(
                            #name.#target() == ::quasar_lang::wrapper::Wrapper::address(&#target),
                            #error,
                        ),
                        #before,
                        #name.#target(),
                        #between,
                        ::quasar_lang::wrapper::Wrapper::address(&#target),
                        #at,
                        #help,
                    )?;
                };
                let named =
                    quote!(::core::option::Option::Some(::quasar_lang::has_one_check!(#target)));
                let (check, vouched) =
                    unless_vouched(check, named, target.to_string(), format!("has_one({target})"));
                quote!(if !#vouched { #check })
            }
            Check::Holds(condition, error) => {
                let written = quote!(#condition).to_string();
                let failed =
                    format!("error[Constraint]: account `{name}`: `{written}` is false");
                let at = at(span, "check");
                let default = quote!(::quasar_lang::error::Error::Constraint);
                let error = error.as_ref().map_or(default, |error| quote!(#error));
                quote! {
                    ::quasar_lang::error::explain(
                        ::quasar_lang::relation::constraint(#condition, #error),
                        #failed,
                        #at,
                        "the accounts or the instruction data passed do not satisfy this `check(..)`: look at the values it reads",
                    )?;
                }
            }
        });
    }
    if let (Some((library, keys)), None) = (&constraints.library, &constraints.init) {
        // A key about another field is left out when a call the handler
        // always makes checks the same of the two.
        let values: Vec<TokenStream> = keys
            .iter()
            .map(|(key, value, of)| {
                let (key, of) =
                    (key.to_string(), of.as_ref().map(Ident::to_string).unwrap_or_default());
                let named = quote!(::quasar_lang::cpi::key(<#library::Check>::KEYS, #key));
                let shown = format!("{}({key} = {of})", quote!(#library));
                let (value, vouched) = unless_vouched(quote!(#value), named, of, shown);
                quote! {
                    if #vouched {
                        ::core::option::Option::None
                    } else {
                        ::core::option::Option::Some(#value)
                    }
                }
            })
            .collect();
        let keyword = quote!(#library).to_string();
        let at = at(constraints.library_span.unwrap_or_else(Span::call_site), &keyword);
        let keys = keys.iter().map(|(key, ..)| key);
        checks.push(quote! {{
            // A check's keys are all optional; `..Default` leaves those not written unchecked.
            #[allow(clippy::needless_update)]
            let check = #library::Check {
                #(#keys: #values,)*
                ..::core::default::Default::default()
            };
            ::quasar_lang::error::explain_with(
                check.check(&#name),
                || {
                    #[allow(unused_imports)]
                    use ::quasar_lang::error::Unexplained as _;
                    check.why(&#name, #field)
                },
                #at,
            )?;
        }});
    }
    checks
}

/// The checks a call to this instruction makes of the field, for
/// `ENFORCES`: its load's, its privileges, and each relation to another
/// field. A field `init` creates is not checked: it is made.
fn enforced(
    load: &TokenStream,
    writable: bool,
    constraints: &Constraints,
    names: &[&Ident],
) -> TokenStream {
    let field = quote!(::quasar_lang::field::Field);
    let (none, enforced) =
        (quote!(::core::option::Option::None), quote!(::quasar_lang::cpi::enforced));
    let mutable = match writable {
        true => quote!(::core::option::Option::Some(::quasar_lang::cpi::MUTABLE_CHECK)),
        false => none.clone(),
    };
    let mut checks = vec![
        quote!(#enforced(<#load as #field>::CHECK, #none)),
        quote!(#enforced(<#load as #field>::SIGNS, #none)),
        quote!(#enforced(#mutable, #none)),
    ];
    let index = |target: &Ident| names.iter().position(|name| *name == target);
    if constraints.init.is_none() {
        for check in &constraints.checks {
            if let Check::HasOne(target, _) = check
                && let Some(of) = index(target)
            {
                checks.push(quote! {
                    #enforced(
                        ::core::option::Option::Some(::quasar_lang::has_one_check!(#target)),
                        ::core::option::Option::Some(#of),
                    )
                });
            }
        }
        if let Some((library, keys)) = &constraints.library {
            for (key, _, of) in keys {
                if let Some(of) = of.as_ref().and_then(&index) {
                    let key = key.to_string();
                    checks.push(quote! {
                        #enforced(
                            {
                                #[allow(unused_imports)]
                                use ::quasar_lang::cpi::Unkeyed as _;
                                ::quasar_lang::cpi::key(<#library::Check>::KEYS, #key)
                            },
                            ::core::option::Option::Some(#of),
                        )
                    });
                }
            }
        }
    }
    // What the handler does with it afterwards is its own to say: only a
    // `close` is known here.
    match constraints.close {
        Some(_) => quote!(::quasar_lang::cpi::Argument::closed(&[#(#checks),*])),
        None => quote!(::quasar_lang::cpi::Argument::checked(&[#(#checks),*])),
    }
}

/// Where `span` is, as rustc shows a place in the source, for the account
/// `name`: the file, line and column, the line itself, and `len` carets under
/// the span with `label` beside them. Only the name where the span has no
/// place, as in a test of this crate.
fn located(name: &str, span: Span, len: usize, label: &str) -> TokenStream {
    let start = span.start();
    let source = span
        .local_file()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| text.lines().nth(start.line.wrapping_sub(1)).map(str::to_owned));
    let lines = match source {
        Some(source) if start.line > 0 => vec![
            format!("    --> {}:{}:{}", span.file(), start.line, start.column + 1),
            "     |".to_owned(),
            format!("{:>4} | {source}", start.line),
            format!("     | {}{} {label}", " ".repeat(start.column), "^".repeat(len.max(1))),
            "     |".to_owned(),
        ],
        _ => Vec::new(),
    };
    quote!(&::quasar_lang::error::At { name: #name, lines: &[#(#lines),*] })
}

/// `S::seeds(&value, ..)` for the `Pda`'s seeds type, with `.with_bump(..)`
/// when a stored bump is given.
fn seeds_value(ty: &Type, Seeds { values, bump, .. }: &Seeds) -> TokenStream {
    // Each value spanned as written, so a wrong one is where the error points.
    let values = values.iter().map(|value| quote_spanned!(value.span()=> &#value));
    let seeds = quote!(<<#ty as ::quasar_lang::pda::Seeded>::Seeds>::seeds(#(#values),*));
    match bump {
        Some(bump) => quote!(#seeds.with_bump(#bump)),
        None => seeds,
    }
}

/// Creates or converts the account, after every field has loaded.
fn init_step(
    name: &Ident,
    ty: &Type,
    constraints: &Constraints,
    init: &Init,
    system: Option<&Ident>,
) -> Result<TokenStream> {
    let system = system.ok_or_else(|| {
        Error::new_spanned(name, "`init` and `migrate` need a field named `system_program`")
    })?;
    let payer = init.payer();
    let rent = rent_ident();
    let idempotent = match init {
        Init::Create { idempotent, .. } => *idempotent,
        Init::Migrate { from, .. } => {
            return Ok(quote! {
                let #name = ::quasar_lang::system::migrate::<#from, _, _>(&#system, &#payer, #name, &#rent)?;
            });
        }
    };
    if let Some((library, keys)) = &constraints.library {
        // Libraries' programs are found by field name, as in Anchor.
        let program = Ident::new("token_program", Span::call_site());
        let (keys, values): (Vec<_>, Vec<_>) =
            keys.iter().map(|(key, value, _)| (key, value)).unzip();
        return Ok(quote! {
            let #name = #library::Init { #(#keys: #values),* }
                .init(&#system, &#payer, &#program, #name, #idempotent, &#rent)?;
        });
    }
    Ok(match constraints.seeds() {
        Some(seeds) => {
            let seeds = seeds_value(ty, seeds);
            quote! {
                let #name = {
                    let (account, bump) = ::quasar_lang::system::init_pda(
                        &#system, &#payer, #name, #seeds, #idempotent, &#rent,
                    )?;
                    <#ty as ::quasar_lang::pda::Seeded>::found(account, bump)
                };
            }
        }
        None => quote! {
            let #name = ::quasar_lang::system::init(&#system, &#payer, #name, #idempotent, &#rent)?;
        },
    })
}

/// What runs after the handler, for one field.
struct Effect {
    ty: TokenStream,
    value: TokenStream,
    /// Whether it settles rent, and so reads it.
    pays_rent: bool,
}

/// What runs after the handler: `close`, or settling a resized account's
/// length and, with a payer, its rent. `None` for a field with neither.
fn effect(
    name: &Ident,
    ty: &Type,
    constraints: &Constraints,
    writable: bool,
    system: Option<&Ident>,
) -> Result<Option<Effect>> {
    if let Some(destination) = &constraints.close {
        let close = quote!(::quasar_lang::effects::Close::new(&self.#name, &self.#destination));
        return Ok(Some(Effect {
            ty: quote!(::quasar_lang::effects::Close),
            value: quote!(::core::option::Option::Some(#close)),
            pays_rent: false,
        }));
    }
    if !writable {
        return Ok(None);
    }
    let settle = quote!(::quasar_lang::effects::Settle);
    let paid = |constructor: &str, payer: &Ident| -> Result<TokenStream> {
        system.ok_or_else(|| {
            Error::new_spanned(
                name,
                "settling rent calls the System Program: add a field named `system_program`",
            )
        })?;
        let (constructor, rent) = (Ident::new(constructor, Span::call_site()), rent_ident());
        Ok(quote!(#settle::#constructor(&self.#name, &self.#payer, &self.system_program, &#rent)))
    };
    let (value, pays_rent) = match (&constraints.realloc, &constraints.init) {
        (Some(payer), _) => (paid("realloc", payer)?, true),
        (None, Some(Init::Create { payer, .. })) if constraints.library.is_none() => {
            (paid("paid", payer)?, true)
        }
        _ => (quote!(<#ty as ::quasar_lang::field::Field>::settle(&self.#name)), false),
    };
    Ok(Some(Effect { ty: settle, value, pays_rent }))
}

/// The index of the field after whose load the checks of field `index` run:
/// the last field they name, so a check can name fields declared after its
/// own. `init` checks run after every field has loaded.
fn runs_after(constraints: &Constraints, index: usize, names: &[&Ident]) -> Result<usize> {
    for check in &constraints.checks {
        if let Check::HasOne(target, _) = check
            && !names.contains(&target)
        {
            return Err(Error::new_spanned(
                target,
                format!("`has_one` names `{target}`, which is not a field of this struct"),
            ));
        }
    }
    // Seeds taken by name may be any account: they wait for the last.
    if constraints.seeds().is_some_and(|seeds| seeds.by_name) {
        return Ok(names.len() - 1);
    }
    Ok(mentioned(constraints)
        .filter_map(|tokens| last_named(tokens, names))
        .fold(index, usize::max))
}

/// Every expression the field's constraints hold.
fn mentioned(constraints: &Constraints) -> impl Iterator<Item = TokenStream> + '_ {
    let checks = constraints.checks.iter().map(|check| match check {
        Check::Address(address) => quote!(#address),
        Check::HasOne(target, error) => quote!(#target #error),
        Check::Holds(condition, error) => quote!(#condition #error),
        Check::Seeds(Seeds { values, bump, .. }) => quote!(#(#values)* #bump),
    });
    let library = constraints.library.iter().map(|(_, keys)| {
        let values = keys.iter().map(|(_, value, _)| value);
        quote!(#(#values)*)
    });
    checks.chain(library)
}

fn mentions_data(constraints: &Constraints) -> bool {
    let data = Ident::new("data", Span::call_site());
    mentioned(constraints).any(|tokens| last_named(tokens, &[&data]).is_some())
}

/// The position in `names` of the last of them `tokens` mention.
fn last_named(tokens: TokenStream, names: &[&Ident]) -> Option<usize> {
    tokens
        .into_iter()
        .filter_map(|token| match token {
            TokenTree::Ident(ident) => names.iter().position(|name| **name == ident),
            TokenTree::Group(group) => last_named(group.stream(), names),
            _ => None,
        })
        .max()
}

/// Account metas and the IDL, on the host. A field `init` creates without
/// seeds signs for its own creation.
fn client_impl(name: &Ident, fields: &[(&Ident, TokenStream, bool)]) -> TokenStream {
    let program = Ident::new("program", Span::mixed_site());
    let metas = Ident::new("metas", Span::mixed_site());
    let keys = fields.iter().map(|(_, ty, _)| quote!(<#ty as ::quasar_lang::field::Field>::Key));
    let names: Vec<_> = fields.iter().map(|(name, _, _)| name).collect();
    let (keys, pattern) = match names.as_slice() {
        [name] => {
            let key = keys.clone().next();
            (quote!(#key), quote!(#name))
        }
        _ => (quote!((#(#keys,)*)), quote!((#(#names,)*))),
    };
    let pushes = fields.iter().map(|(name, ty, writable)| {
        quote!(<#ty as ::quasar_lang::field::Field>::metas(#name, #writable, #program, &mut #metas);)
    });
    let idl = fields.iter().map(|(name, ty, writable)| {
        let name = name.to_string();
        quote!(<#ty as ::quasar_lang::field::Field>::idl(#name, #writable))
    });
    let kinds =
        fields.iter().map(|(_, ty, _)| quote!(<#ty as ::quasar_lang::field::Field>::idl_kind()));
    quote! {
        ::quasar_lang::host_only! {
            impl ::quasar_lang::client::ClientAccounts for #name {
                type Keys = #keys;

                fn metas(
                    #pattern: Self::Keys,
                    #program: ::quasar_lang::Address,
                ) -> ::quasar_lang::client::Vec<::quasar_lang::client::AccountMeta> {
                    let mut #metas = ::quasar_lang::client::Vec::new();
                    #(#pushes)*
                    #metas
                }

                fn idl_accounts() -> ::quasar_lang::client::Vec<::quasar_lang::client::IdlAccount> {
                    ::quasar_lang::client::Vec::from([#(#idl),*])
                }

                fn idl_kinds() -> ::quasar_lang::client::Vec<::quasar_lang::client::IdlKind> {
                    [#(#kinds),*].into_iter().flatten().collect()
                }
            }
        }
    }
}

/// `Name::cpi_accounts(..)`: this instruction's accounts for a program that
/// calls it, writable and signing as the struct declares them.
fn cpi_accounts(
    name: &Ident,
    fields: &[(&Ident, TokenStream, bool)],
    enforces: &[TokenStream],
) -> TokenStream {
    let count = fields.len();
    let params = fields.iter().map(|(name, _, writable)| match writable {
        true => quote!(#name: &'a mut impl ::quasar_lang::wrapper::Wrapper),
        false => quote!(#name: &'a impl ::quasar_lang::wrapper::Wrapper),
    });
    let accounts = fields.iter().map(|(name, ty, writable)| match writable {
        true => quote!(::quasar_lang::cpi::CpiAccount::writable_field::<#ty, _>(#name, &crate::ID)),
        false => quote!(::quasar_lang::cpi::CpiAccount::readonly_field::<#ty, _>(#name)),
    });
    quote! {
        impl #name {
            /// What the instruction checks of each account `cpi_accounts`
            /// takes, so a handler that always calls it need not.
            pub const ENFORCES: ::quasar_lang::cpi::Enforces = &[#(#enforces),*];

            /// This instruction's accounts, for a program that calls it:
            /// writable and signing as the instruction declares them.
            // One parameter per account, however many the instruction has.
            #[allow(clippy::too_many_arguments)]
            pub fn cpi_accounts<'a>(
                #(#params),*
            ) -> [::quasar_lang::cpi::CpiAccount<'a>; #count] {
                [#(#accounts),*]
            }
        }
    }
}

/// The local holding the rent, which a user's constraints cannot name.
fn rent_ident() -> Ident {
    Ident::new("rent", Span::mixed_site())
}
