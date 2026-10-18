//! `#[instruction(n)]` on a handler, and `#[program]` on the module of them.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, GenericArgument, Ident, Item, ItemFn, ItemMod, PathArguments, Result, Type,
    Visibility,
};

/// `#[instruction(0)]` on `fn make(ctx: Ctx<Make, MakeData>) -> Result<..>`:
/// the handler as written, and beside it a module of the same name with its
/// discriminator, its builder for clients, and its call for other programs.
pub fn instruction(args: TokenStream, handler: ItemFn) -> Result<TokenStream> {
    let discriminator = syn::parse::Parser::parse2(crate::account::parse_discriminator, args)?;
    let (accounts, data) = context_types(&handler)?;
    let name = &handler.sig.ident;
    let vis = &handler.vis;
    let width = discriminator.len();
    let size = quote!({ #width + <#data as ::quasar_lang::data::InstructionData>::SIZE });
    let doc = format!("The `{name}` instruction: its discriminator, and how to build or call it.");
    let program = quote!(crate::__Program);
    let vouches = crate::vouch::expand(&handler);

    Ok(quote! {
        // Inlined into the dispatch: each instruction's code runs in one
        // function, which lets the optimizer see across the loads and the
        // handler (measured on the benchmark).
        #[inline(always)]
        #handler

        #[doc = #doc]
        #vis mod #name {
            use super::*;

            pub const DISCRIMINATOR: [u8; #width] = [#(#discriminator),*];

            #vouches

            /// Its accounts' fields, for `#[program]` to check.
            pub const FIELDS: &[::quasar_lang::field::FieldInfo] =
                <#accounts as ::quasar_lang::context::Accounts<#data>>::FIELDS;

            /// Calls it from another program, with the accounts in the order
            /// its accounts struct declares them.
            pub fn cpi<'a, const N: usize>(
                program: &'a ::quasar_lang::wrapper::Program<#program>,
                accounts: [::quasar_lang::cpi::CpiAccount<'a>; N],
                data: #data,
            ) -> ::quasar_lang::cpi::Cpi<'a, #program, N, #size> {
                let data = ::quasar_lang::data::encode(&DISCRIMINATOR, &data);
                ::quasar_lang::cpi::Cpi::new(program, accounts, data)
            }

            ::quasar_lang::host_only! {
                /// The instruction, for a client or a test.
                pub fn instruction(
                    accounts: <#accounts as ::quasar_lang::client::ClientAccounts>::Keys,
                    data: #data,
                ) -> ::quasar_lang::client::Instruction {
                    let bytes: [u8; #size] = ::quasar_lang::data::encode(&DISCRIMINATOR, &data);
                    ::quasar_lang::client::Instruction {
                        program_id: crate::ID,
                        accounts: <#accounts as ::quasar_lang::client::ClientAccounts>::metas(accounts, crate::ID),
                        data: ::quasar_lang::client::Vec::from(bytes),
                    }
                }

                pub fn idl() -> ::quasar_lang::client::IdlInstruction {
                    ::quasar_lang::client::IdlInstruction {
                        name: stringify!(#name),
                        discriminator: &DISCRIMINATOR,
                        accounts: <#accounts as ::quasar_lang::client::ClientAccounts>::idl_accounts(),
                        args: ::quasar_lang::client::Vec::from(
                            <#data as ::quasar_lang::data::InstructionData>::IDL,
                        ),
                    }
                }

                /// The checks loading its accounts leaves to its calls.
                pub fn elided() -> ::quasar_lang::client::Vec<::quasar_lang::client::String> {
                    <#accounts>::elided::<Vouched>()
                }

                pub fn idl_kinds() -> ::quasar_lang::client::Vec<::quasar_lang::client::IdlKind> {
                    <#accounts as ::quasar_lang::client::ClientAccounts>::idl_kinds()
                }
            }
        }
    })
}

/// The `A` and `D` of a handler's `ctx: Ctx<A>` or `ctx: Ctx<A, D>`.
fn context_types(handler: &ItemFn) -> Result<(Type, Type)> {
    let wrong = || {
        Error::new_spanned(
            &handler.sig,
            "an instruction takes one argument, `Ctx<Accounts>` or `Ctx<Accounts, Data>`",
        )
    };
    let mut inputs = handler.sig.inputs.iter();
    let (Some(FnArg::Typed(ctx)), None) = (inputs.next(), inputs.next()) else {
        return Err(wrong());
    };
    let Type::Path(path) = &*ctx.ty else { return Err(wrong()) };
    let segment =
        path.path.segments.last().filter(|segment| segment.ident == "Ctx").ok_or_else(wrong)?;
    let PathArguments::AngleBracketed(generics) = &segment.arguments else { return Err(wrong()) };
    let mut types = generics.args.iter().map(|arg| match arg {
        GenericArgument::Type(ty) => Ok(ty.clone()),
        _ => Err(wrong()),
    });
    match (types.next(), types.next(), types.next()) {
        (Some(accounts), None, None) => Ok((accounts?, syn::parse_quote!(()))),
        (Some(accounts), Some(data), None) => Ok((accounts?, data?)),
        _ => Err(wrong()),
    }
}

/// `#[program] mod name { .. }`: the module as written, its items at the
/// crate root, and for every `#[instruction(n)]` handler in it the entrypoint
/// arm; the program's type for callers, and its IDL.
pub fn program(args: TokenStream, module: ItemMod) -> Result<TokenStream> {
    if !args.is_empty() {
        return Err(Error::new_spanned(args, "`#[program]` takes no arguments"));
    }
    let Some((_, items)) = &module.content else {
        return Err(Error::new_spanned(
            &module,
            "`#[program]` needs a module with a body: `mod name { .. }`",
        ));
    };
    let mut handlers = Vec::new();
    // Each discriminator, and the handler it belongs to.
    let mut seen: Vec<(Vec<u8>, &Ident)> = Vec::new();
    for item in items {
        let Item::Fn(handler) = item else { continue };
        let Some(attr) = handler.attrs.iter().find(|attr| attr.path().is_ident("instruction"))
        else {
            continue;
        };
        let discriminator = attr.parse_args_with(crate::account::parse_discriminator)?;
        let name = &handler.sig.ident;
        if let Some((first, _)) = seen.first()
            && first.len() != discriminator.len()
        {
            return Err(Error::new_spanned(
                attr,
                "every instruction's discriminator has the same number of bytes",
            ));
        }
        if let Some((_, other)) = seen.iter().find(|(bytes, _)| *bytes == discriminator) {
            return Err(Error::new_spanned(
                attr,
                format!("discriminator {discriminator:?} is already `{other}`'s"),
            ));
        }
        seen.push((discriminator, name));
        if !matches!(handler.vis, Visibility::Public(_)) {
            return Err(Error::new_spanned(
                &handler.sig.ident,
                "a handler is `pub`, so clients and other programs can build its instruction",
            ));
        }
        handlers.push(&handler.sig.ident);
    }
    let Some(first) = handlers.first() else {
        return Err(Error::new_spanned(
            &module.ident,
            "a program has at least one `#[instruction(n)]` handler",
        ));
    };
    let name = &module.ident;
    let program = format_ident!("{}", camel_case(&name.to_string()));
    let names: Vec<String> = handlers.iter().map(|handler| handler.to_string()).collect();
    let handlers: Vec<TokenStream> =
        handlers.iter().map(|handler| quote!(#name::#handler)).collect();
    let first = quote!(#name::#first);
    let cursor = Ident::new("cursor", Span::mixed_site());
    let data = Ident::new("data", Span::mixed_site());

    Ok(quote! {
        #module

        pub use #name::*;

        /// This program, for `Program<program::Name>` in a program calling it.
        pub mod program {
            pub type #program = crate::__Program;
        }

        const _: () = ::quasar_lang::account::one_width(&[#(#handlers::FIELDS),*]);

        ::quasar_lang::host_only! {
            /// This program's interface, from the constants it dispatches on.
            pub fn idl() -> ::quasar_lang::client::Idl {
                let mut kinds = ::quasar_lang::client::Vec::new();
                for kind in [#(#handlers::idl_kinds()),*].into_iter().flatten() {
                    if !kinds.contains(&kind) {
                        kinds.push(kind);
                    }
                }
                ::quasar_lang::client::Idl {
                    address: crate::ID,
                    name: stringify!(#name),
                    instructions: ::quasar_lang::client::Vec::from([#(#handlers::idl()),*]),
                    kinds,
                }
            }
        }

        ::quasar_lang::host_only! {
            #[test]
            fn __quasar_emit_idl() {
                extern crate std;
                std::println!("__QUASAR_IDL_JSON_BEGIN__\n{}__QUASAR_IDL_JSON_END__", idl().to_json());
            }
        }

        ::quasar_lang::host_only! {
            /// Each check an instruction leaves to a call it always makes,
            /// one per line: `instruction.account: check`.
            pub fn elisions() -> ::quasar_lang::client::String {
                let mut lines = ::quasar_lang::client::String::new();
                #(
                    for elided in #handlers::elided() {
                        lines.push_str(#names);
                        lines.push('.');
                        lines.push_str(&elided);
                        lines.push('\n');
                    }
                )*
                lines
            }
        }

        // A program that calls this one depends on it with the `no-entrypoint`
        // feature, so it links only one entrypoint. The feature need not be
        // declared by programs nobody calls.
        #[allow(unexpected_cfgs)]
        const _: () = {
            #[cfg(not(feature = "no-entrypoint"))]
            fn dispatch(
                discriminator: [u8; #first::DISCRIMINATOR.len()],
                #data: &[u8],
                #cursor: ::quasar_lang::input::Cursor,
            ) -> ::quasar_lang::Result {
                match discriminator {
                    #(#handlers::DISCRIMINATOR => ::quasar_lang::context::run::<_, _, _, #handlers::Vouched>(#cursor, #data, #handlers),)*
                    _ => ::core::result::Result::Err(::quasar_lang::ProgramError::InvalidInstructionData),
                }
            }

            ::quasar_lang::export_entrypoint! {
                /// # Safety
                /// Called only by the runtime, with its serialized input and, in r2,
                /// the instruction data.
                #[cfg(not(feature = "no-entrypoint"))]
                pub unsafe extern "C" fn entrypoint(input: *mut u8, data: *const u8) -> u64 {
                    // SAFETY: forwarded from this function's contract.
                    unsafe { ::quasar_lang::context::entrypoint(input, data, &crate::ID, dispatch) }
                }
            }
        };
    })
}

/// `callee` to `Callee`, `quasar_vault` to `QuasarVault`.
fn camel_case(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
        })
        .collect::<Option<String>>()
        .unwrap_or_default()
}
