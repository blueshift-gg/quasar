//! `#[account]`, `#[derive(Seeds)]`, `#[event]` and `#[error_code]`.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Error, Expr, ExprArray, Fields, Ident, ItemStruct, LitInt, Path, Result, Token,
    Type,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

/// `discriminator = 1` or `discriminator = [1, 2]`: the bytes, as written.
pub fn parse_discriminator(input: ParseStream) -> Result<Vec<u8>> {
    let bytes: Vec<LitInt> = if input.peek(syn::token::Bracket) {
        let array: ExprArray = input.parse()?;
        array
            .elems
            .iter()
            .map(|elem| match elem {
                Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int), .. }) => Ok(int.clone()),
                other => Err(Error::new_spanned(other, "discriminator bytes are integer literals")),
            })
            .collect::<Result<_>>()?
    } else {
        vec![input.parse()?]
    };
    let bytes = bytes.iter().map(LitInt::base10_parse::<u8>).collect::<Result<Vec<_>>>()?;
    if bytes.is_empty() {
        return Err(input.error("the discriminator needs at least one byte"));
    }
    Ok(bytes)
}

struct Args {
    discriminator: Vec<u8>,
    owner: Option<Path>,
}

fn parse_args(tokens: TokenStream, attribute: &str) -> Result<Args> {
    let mut args = Args { discriminator: Vec::new(), owner: None };
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("discriminator") {
            args.discriminator = parse_discriminator(meta.value()?)?;
        } else if meta.path.is_ident("owner") {
            args.owner = Some(meta.value()?.parse()?);
        } else {
            return Err(meta.error("expected `discriminator` or `owner`"));
        }
        Ok(())
    });
    syn::parse::Parser::parse2(parser, tokens)?;
    if args.discriminator.is_empty() {
        return Err(Error::new(
            Span::call_site(),
            format!("`{attribute}` needs `discriminator = ..`"),
        ));
    }
    Ok(args)
}

/// A `String` or `Vec<T>` field's `#[max_len(..)]`, which it must have, and
/// how the IDL names it (`String<32>`, `Vec<Address, 10>`); `None` for any
/// other field, which must not have one.
fn bounded(field: &syn::Field) -> Result<Option<String>> {
    let max_len = field.attrs.iter().find(|attr| attr.path().is_ident("max_len"));
    let segment = match &field.ty {
        Type::Path(path) => path.path.segments.last(),
        _ => None,
    };
    let collection = segment.filter(|segment| segment.ident == "String" || segment.ident == "Vec");
    match (collection, max_len) {
        (None, None) => Ok(None),
        (None, Some(attr)) => {
            Err(Error::new_spanned(attr, "`#[max_len]` is for `String` and `Vec`"))
        }
        (Some(segment), None) => Err(Error::new_spanned(
            &field.ty,
            format!("`{}` needs `#[max_len(N)]`: an account has a capacity", segment.ident),
        )),
        (Some(_), Some(attr)) => {
            let bounds = attr.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;
            let bounds: Vec<String> =
                bounds.iter().map(|bound| quote!(#bound).to_string()).collect();
            let named = match generic_of(&field.ty, "Vec") {
                Some(item) => format!("Vec<{}, {}>", type_name(item), bounds.join(", ")),
                None => format!("String<{}>", bounds.join(", ")),
            };
            Ok(Some(named))
        }
    }
}

/// `#[account(discriminator = .., owner = ..)]`, with an optional
/// `#[seeds(..)]` beside it: the struct as written, stored in zeropod's
/// encoding. `Account<Name>` derefs to its view, a method per field.
pub fn expand_account(args: TokenStream, mut item: ItemStruct) -> Result<TokenStream> {
    let args = parse_args(args, "#[account]")?;
    if args.discriminator.len() > 16 {
        return Err(Error::new(Span::call_site(), "an account discriminator is at most 16 bytes"));
    }
    // `Account<K>` asserts this too, for kinds written by hand; here the error
    // points at the attribute.
    if args.discriminator.iter().all(|&byte| byte == 0) {
        return Err(Error::new(
            Span::call_site(),
            "an account discriminator cannot be all zeros: anyone can create a zeroed account \
             this program owns, and it would pass as this kind",
        ));
    }
    let seeds = take_seeds(&mut item.attrs)?;
    if !item.generics.params.is_empty() {
        return Err(Error::new_spanned(&item.generics, "accounts cannot be generic"));
    }
    let Fields::Named(fields) = &item.fields else {
        return Err(Error::new_spanned(&item, "accounts need named fields"));
    };
    let idl_fields = fields
        .named
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().expect("named").to_string();
            let ty = &field.ty;
            let ty = bounded(field)?.unwrap_or_else(|| type_name(ty));
            Ok(quote!(::quasar_lang::client::IdlField { name: #name, ty: #ty }))
        })
        .collect::<Result<Vec<_>>>()?;
    let name = &item.ident;
    let discriminator = &args.discriminator;
    let (owner, program_owned) = match &args.owner {
        Some(owner) => (quote!(#owner), false),
        None => (quote!(crate::ID), true),
    };
    // Two kinds this program owns with one discriminator would be two
    // identical impls, which do not compile.
    let unique = program_owned.then(|| {
        let len = discriminator.len();
        let value = discriminator.iter().fold(0u128, |value, &byte| value << 8 | u128::from(byte));
        quote! {
            impl ::quasar_lang::account::UniqueDiscriminator<#len, #value> for crate::__Kinds {}
        }
    });
    let field_names =
        fields.named.iter().filter_map(|field| Some(field.ident.as_ref()?.to_string()));
    let stored: Vec<(&Ident, &Type)> =
        fields.named.iter().filter_map(|field| Some((field.ident.as_ref()?, &field.ty))).collect();
    let view = quote!(<#name as ::quasar_lang::zeropod::Layout>::View);
    let seeds = seeds.map(|seeds| {
        let (by_name, own) = (seeds_impl(name, &seeds), own_seeds(name, &seeds, &stored, view));
        quote!(#by_name #own)
    });

    Ok(quote! {
        #[derive(::quasar_lang::zeropod::ZeroPod)]
        #[zeropod(crate = ::quasar_lang::zeropod)]
        #item

        impl ::quasar_lang::account::Kind for #name {
            const OWNER: ::quasar_lang::Address = #owner;
            const DISCRIMINATOR: &'static [u8] = &[#(#discriminator),*];
            const PROGRAM_OWNED: bool = #program_owned;
            const FIELDS: &'static [&'static str] = &[#(#field_names),*];
            const CHECK: ::core::option::Option<::quasar_lang::cpi::Named> =
                ::quasar_lang::cpi::Named::plain::<Self>(
                    concat!(module_path!(), "::", stringify!(#name)),
                );

            ::quasar_lang::host_only! {
                const IDL: ::core::option::Option<::quasar_lang::client::IdlKind> =
                    ::core::option::Option::Some(::quasar_lang::client::IdlKind {
                        name: stringify!(#name),
                        discriminator: &[#(#discriminator),*],
                        size: ::quasar_lang::account::len::<#name>(),
                        fields: &[#(#idl_fields),*],
                    });
            }
        }

        #unique
        #seeds
    })
}

/// One `#[seeds(..)]` entry: a byte-string literal, or a named value of a
/// type `pda::Seed` takes: `Address`, `[u8; N]`, an integer.
enum Seed {
    Literal(syn::LitByteStr),
    Param(Ident, TokenStream),
}

impl Parse for Seed {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(syn::LitByteStr) {
            let literal: syn::LitByteStr = input.parse()?;
            if literal.value().len() > solana_address::MAX_SEED_LEN {
                return Err(Error::new_spanned(literal, "a seed is at most 32 bytes"));
            }
            return Ok(Self::Literal(literal));
        }
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(Self::Param(name, quote!(#ty)))
    }
}

fn take_seeds(attrs: &mut Vec<Attribute>) -> Result<Option<Vec<Seed>>> {
    let Some(index) = attrs.iter().position(|attr| attr.path().is_ident("seeds")) else {
        return Ok(None);
    };
    let attr = attrs.remove(index);
    let seeds = attr.parse_args_with(Punctuated::<Seed, Token![,]>::parse_terminated)?;
    Ok(Some(seeds.into_iter().collect()))
}

/// `Name::seeds(..)`, returning the seeds of the PDA. Seeds without
/// parameters are found at compile time.
fn seeds_impl(name: &Ident, seeds: &[Seed]) -> TokenStream {
    let count = seeds.len();
    let params: Vec<TokenStream> = seeds
        .iter()
        .filter_map(|seed| match seed {
            Seed::Param(name, ty) => Some(quote!(#name: &'a impl ::quasar_lang::pda::Seed<#ty>)),
            Seed::Literal(_) => None,
        })
        .collect();
    let values = seeds.iter().map(|seed| match seed {
        Seed::Literal(bytes) => quote!(#bytes as &[u8]),
        Seed::Param(name, _) => quote!(::quasar_lang::pda::Seed::seed(#name)),
    });
    if params.is_empty() {
        quote! {
            impl #name {
                pub const fn seeds() -> ::quasar_lang::pda::Seeds<'static, #count> {
                    const SEEDS: [&[u8]; #count] = [#(#values),*];
                    // A `const` item, so the search runs when compiling, never on chain.
                    const FOUND: (::quasar_lang::Address, u8) =
                        ::quasar_lang::pda::find(&SEEDS, &crate::ID);
                    ::quasar_lang::pda::Seeds::found(SEEDS, FOUND)
                }
            }
        }
    } else {
        quote! {
            impl #name {
                pub fn seeds<'a>(
                    #(#params),*
                ) -> ::quasar_lang::pda::Seeds<'a, #count> {
                    ::quasar_lang::pda::Seeds::new([#(#values),*])
                }
            }
        }
    }
}

/// `Name::own_seeds(account, accounts)`, for `seeds` written without values:
/// each seed from the field of its name in `account`, with the stored `bump`
/// if there is one, or else from the account of its name in `accounts`.
/// Nothing for seeds that neither gives: an integer, say.
fn own_seeds(
    name: &Ident,
    seeds: &[Seed],
    stored: &[(&Ident, &Type)],
    view: TokenStream,
) -> Option<TokenStream> {
    // A stored value is borrowed in place, which an address or bytes can be.
    let borrowed = |ty: &TokenStream| {
        let ty = ty.to_string();
        ty == "Address" || ty.starts_with('[')
    };
    let mut named = Vec::new();
    let values = seeds
        .iter()
        .filter_map(|seed| match seed {
            Seed::Literal(_) => None,
            Seed::Param(param, ty) => Some(
                if stored.iter().any(|(field, _)| *field == param) && borrowed(ty) {
                    Some(quote!(account.#param()))
                } else if ty.to_string() == "Address" {
                    let at = param.to_string();
                    named.push(quote!(::quasar_lang::pda::Named<{ ::quasar_lang::pda::name(#at) }>));
                    Some(quote! {
                        <__Named as ::quasar_lang::pda::Named<{ ::quasar_lang::pda::name(#at) }>>::named(accounts)
                    })
                } else {
                    None
                },
            ),
        })
        .collect::<Option<Vec<_>>>()?;
    let bump = stored.iter().any(|(field, ty)| *field == "bump" && quote!(#ty).to_string() == "u8");
    let (seeds_ty, with_bump) = match bump {
        true => (quote!(SignerSeeds), Some(quote!(.with_bump(account.bump())))),
        false => (quote!(Seeds), None),
    };
    let count = seeds.len();
    // An account that stores every seed and its bump signs for itself.
    let has_signer_field = stored.iter().any(|(field, _)| *field == "signer");
    let signer = (bump && named.is_empty() && !stored.is_empty() && !has_signer_field).then(|| {
        let view = quote::format_ident!("{name}View");
        quote! {
            impl #view {
                /// The seeds and bump of this account's address, which it
                /// stores, to sign a call for it.
                pub fn signer(&self) -> ::quasar_lang::pda::SignerSeeds<'_, #count> {
                    #name::own_seeds(self, &())
                }
            }
        }
    });
    Some(quote! {
        #signer

        impl #name {
            /// The seeds of `account`'s address, each from its field of that
            /// name or else the account of that name.
            pub fn own_seeds<'a, __Named: #(#named +)* ?Sized>(
                account: &'a #view,
                accounts: &'a __Named,
            ) -> ::quasar_lang::pda::#seeds_ty<'a, #count> {
                let _ = (account, accounts);
                Self::seeds(#(#values),*) #with_bump
            }
        }
    })
}

/// `#[derive(Seeds)]` with `#[seeds(..)]`, for a PDA that holds no data.
pub fn expand_seeds(mut item: syn::DeriveInput) -> Result<TokenStream> {
    let seeds = take_seeds(&mut item.attrs)?
        .ok_or_else(|| Error::new_spanned(&item.ident, "`Seeds` needs `#[seeds(..)]`"))?;
    let by_name = seeds_impl(&item.ident, &seeds);
    let own = own_seeds(&item.ident, &seeds, &[], quote!(impl ?Sized));
    Ok(quote!(#by_name #own))
}

/// `#[event(discriminator = ..)]`: the fields as written, logged
/// little-endian after the discriminator as one `Program data:` entry.
pub fn expand_event(args: TokenStream, item: ItemStruct) -> Result<TokenStream> {
    let args = parse_args(args, "#[event]")?;
    if args.owner.is_some() {
        return Err(Error::new(Span::call_site(), "`#[event]` takes only `discriminator = ..`"));
    }
    let Fields::Named(fields) = &item.fields else {
        return Err(Error::new_spanned(&item, "events need named fields"));
    };
    let name = &item.ident;
    let discriminator = &args.discriminator;
    let width = discriminator.len();
    let names = fields.named.iter().map(|field| &field.ident);
    let types: Vec<_> = fields.named.iter().map(|field| &field.ty).collect();
    let data = Ident::new("data", Span::mixed_site());
    let at = Ident::new("at", Span::mixed_site());

    Ok(quote! {
        #item

        impl ::quasar_lang::effects::Event for #name {
            fn emit(&self) {
                let mut #data = [0u8; #width #(+ ::quasar_lang::data::field_size::<#types>())*];
                #data[..#width].copy_from_slice(&[#(#discriminator),*]);
                let mut #at = #width;
                #(::quasar_lang::data::put(&self.#names, &mut #data, &mut #at);)*
                ::quasar_lang::effects::log_data(&#data);
            }
        }
    })
}

/// A type as the IDL names it: as written, without the spaces `quote` adds.
pub(crate) fn type_name(ty: &Type) -> String {
    let compact: String = quote!(#ty).to_string().split_whitespace().collect();
    compact.replace(',', ", ").replace(';', "; ")
}

/// `#[error_code]`: each variant becomes `ProgramError::Custom(6000 + index)`,
/// as in Anchor, and its `#[msg(..)]` becomes its documentation.
pub fn expand_error_code(mut item: syn::ItemEnum) -> Result<TokenStream> {
    for variant in &mut item.variants {
        if !matches!(variant.fields, Fields::Unit) || variant.discriminant.is_some() {
            return Err(Error::new_spanned(variant, "error variants are plain names"));
        }
        for attr in &mut variant.attrs {
            if attr.path().is_ident("msg") {
                let message: syn::LitStr = attr.parse_args()?;
                *attr = syn::parse_quote!(#[doc = #message]);
            }
        }
    }
    let name = &item.ident;
    Ok(quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u32)]
        #item

        impl ::core::convert::From<#name> for ::quasar_lang::ProgramError {
            fn from(error: #name) -> Self {
                ::quasar_lang::ProgramError::Custom(6000 + error as u32)
            }
        }
    })
}

/// The `T` in a type written `Name<T>`.
fn generic_of<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last().filter(|segment| segment.ident == name)?;
    let syn::PathArguments::AngleBracketed(generics) = &segment.arguments else { return None };
    match generics.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}
