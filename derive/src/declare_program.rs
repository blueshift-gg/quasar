//! `declare_program!` — generates a typed client module from a program's IDL
//! JSON. Produces account types, instruction builders, event types, and error
//! enums for cross-program interaction without runtime IDL parsing.

use {
    crate::helpers::{pascal_to_snake, snake_to_pascal},
    proc_macro::TokenStream,
    proc_macro2::{Ident, Span, TokenStream as TokenStream2},
    quote::{format_ident, quote},
};

// ---------------------------------------------------------------------------
// Minimal IDL types for declare_program! macro.
// Source: idl/src/types.rs — keep in sync.
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct Idl {
    address: String,
    instructions: Vec<IdlInstruction>,
}

#[derive(serde::Deserialize)]
struct IdlInstruction {
    name: String,
    discriminator: Vec<u8>,
    accounts: Vec<IdlAccountItem>,
    args: Vec<IdlField>,
}

#[derive(serde::Deserialize)]
struct IdlAccountItem {
    name: String,
    #[serde(default)]
    writable: bool,
    #[serde(default)]
    signer: bool,
}

#[derive(serde::Deserialize)]
struct IdlField {
    name: String,
    #[serde(rename = "type")]
    ty: IdlType,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum IdlType {
    Primitive(String),
    Defined {
        #[allow(dead_code)]
        defined: String,
    },
}

// ---------------------------------------------------------------------------
// Type mapping
// ---------------------------------------------------------------------------

struct TypeInfo {
    rust_type: TokenStream2,
    byte_size: usize,
    is_reference: bool,
}

fn map_idl_type(ty: &IdlType) -> Result<TypeInfo, String> {
    match ty {
        IdlType::Primitive(s) => {
            let (rust_type, byte_size) = match s.as_str() {
                "u8" => (quote! { u8 }, 1),
                "i8" => (quote! { i8 }, 1),
                "bool" => (quote! { bool }, 1),
                "u16" => (quote! { u16 }, 2),
                "i16" => (quote! { i16 }, 2),
                "u32" => (quote! { u32 }, 4),
                "i32" => (quote! { i32 }, 4),
                "u64" => (quote! { u64 }, 8),
                "i64" => (quote! { i64 }, 8),
                "u128" => (quote! { u128 }, 16),
                "i128" => (quote! { i128 }, 16),
                "pubkey" => {
                    return Ok(TypeInfo {
                        rust_type: quote! { &quasar_core::prelude::Address },
                        byte_size: 32,
                        is_reference: true,
                    });
                }
                other => return Err(format!("unsupported primitive type '{other}'")),
            };
            Ok(TypeInfo {
                rust_type,
                byte_size,
                is_reference: false,
            })
        }
        IdlType::Defined { defined } => Err(format!(
            "defined type '{defined}' is not supported — CPI helpers require fixed-size args"
        )),
    }
}

// ---------------------------------------------------------------------------
// Code generation helpers
// ---------------------------------------------------------------------------

fn generate_data_write(args: &[IdlField], disc: &[u8]) -> Result<(TokenStream2, usize), String> {
    let disc_len = disc.len();
    let mut offset = disc_len;
    let mut write_stmts = Vec::new();

    for (i, &byte) in disc.iter().enumerate() {
        let byte_lit = proc_macro2::Literal::u8_suffixed(byte);
        write_stmts.push(quote! {
            core::ptr::write(__ptr.add(#i), #byte_lit);
        });
    }

    for field in args {
        let info = map_idl_type(&field.ty)?;
        let fname = Ident::new(&pascal_to_snake(&field.name), Span::call_site());
        let size = info.byte_size;

        if info.is_reference {
            write_stmts.push(quote! {
                core::ptr::copy_nonoverlapping(
                    #fname.as_ref().as_ptr(),
                    __ptr.add(#offset),
                    #size,
                );
            });
        } else if size == 1 {
            let rust_type = &info.rust_type;
            write_stmts.push(quote! {
                core::ptr::write(__ptr.add(#offset), #fname as #rust_type as u8);
            });
        } else {
            write_stmts.push(quote! {
                core::ptr::copy_nonoverlapping(
                    #fname.to_le_bytes().as_ptr(),
                    __ptr.add(#offset),
                    #size,
                );
            });
        }

        offset += size;
    }

    let total_size = offset;
    let block = quote! {
        unsafe {
            let mut __buf = core::mem::MaybeUninit::<[u8; #total_size]>::uninit();
            let __ptr = __buf.as_mut_ptr() as *mut u8;
            #(#write_stmts)*
            __buf.assume_init()
        }
    };

    Ok((block, total_size))
}

/// Build an InstructionAccount constructor call for the given account flags.
fn ia_constructor(writable: bool, signer: bool) -> &'static str {
    match (writable, signer) {
        (true, true) => "writable_signer",
        (true, false) => "writable",
        (false, true) => "readonly_signer",
        (false, false) => "readonly",
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn declare_program(input: TokenStream) -> TokenStream {
    let input2 = proc_macro2::TokenStream::from(input);
    let mut iter = input2.into_iter();

    let mod_name = match iter.next() {
        Some(proc_macro2::TokenTree::Ident(id)) => id,
        _ => {
            return syn::Error::new(Span::call_site(), "expected module name as first argument")
                .to_compile_error()
                .into();
        }
    };

    match iter.next() {
        Some(proc_macro2::TokenTree::Punct(p)) if p.as_char() == ',' => {}
        _ => {
            return syn::Error::new(Span::call_site(), "expected comma after module name")
                .to_compile_error()
                .into();
        }
    };

    let idl_path = match iter.next() {
        Some(proc_macro2::TokenTree::Literal(lit)) => {
            let s = lit.to_string();
            if s.starts_with('"') && s.ends_with('"') {
                s[1..s.len() - 1].to_string()
            } else {
                return syn::Error::new(Span::call_site(), "expected string literal for IDL path")
                    .to_compile_error()
                    .into();
            }
        }
        _ => {
            return syn::Error::new(Span::call_site(), "expected string literal for IDL path")
                .to_compile_error()
                .into();
        }
    };

    let idl_json = match std::fs::read_to_string(&idl_path) {
        Ok(json) => json,
        Err(_) => {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let full_path = std::path::Path::new(&manifest_dir).join(&idl_path);
            match std::fs::read_to_string(&full_path) {
                Ok(json) => json,
                Err(e) => {
                    let msg = format!(
                        "could not read IDL file '{}' (also tried '{}'): {}",
                        idl_path,
                        full_path.display(),
                        e,
                    );
                    return syn::Error::new(Span::call_site(), msg)
                        .to_compile_error()
                        .into();
                }
            }
        }
    };

    let idl: Idl = match serde_json::from_str(&idl_json) {
        Ok(idl) => idl,
        Err(e) => {
            let msg = format!("failed to parse IDL JSON: {e}");
            return syn::Error::new(Span::call_site(), msg)
                .to_compile_error()
                .into();
        }
    };

    // Validate all arg types up front
    for ix in &idl.instructions {
        for arg in &ix.args {
            if let Err(msg) = map_idl_type(&arg.ty) {
                let full_msg = format!("in instruction '{}', arg '{}': {}", ix.name, arg.name, msg);
                return syn::Error::new(Span::call_site(), full_msg)
                    .to_compile_error()
                    .into();
            }
        }
    }

    let program_type_name = format_ident!("{}", snake_to_pascal(&mod_name.to_string()));
    let address_str = &idl.address;
    let address_tokens = quote! { quasar_core::prelude::address!(#address_str) };

    let mut free_functions = Vec::new();
    let mut method_impls = Vec::new();

    for ix in &idl.instructions {
        let fn_name = Ident::new(&pascal_to_snake(&ix.name), Span::call_site());
        let acct_count = ix.accounts.len();

        // Pre-compute per-account identifiers once
        let acct_idents: Vec<Ident> = ix
            .accounts
            .iter()
            .map(|a| Ident::new(&pascal_to_snake(&a.name), Span::call_site()))
            .collect();

        let ia_entries: Vec<TokenStream2> = ix
            .accounts
            .iter()
            .zip(&acct_idents)
            .map(|(a, name)| {
                let method = Ident::new(ia_constructor(a.writable, a.signer), Span::call_site());
                quote! { quasar_core::cpi::InstructionAccount::#method(#name.address()) }
            })
            .collect();

        let arg_params: Vec<TokenStream2> = ix
            .args
            .iter()
            .map(|a| {
                let info = map_idl_type(&a.ty).unwrap();
                let name = Ident::new(&pascal_to_snake(&a.name), Span::call_site());
                let ty = &info.rust_type;
                quote! { #name: #ty }
            })
            .collect();

        let (data_write, data_size) = generate_data_write(&ix.args, &ix.discriminator).unwrap();

        // Free function: accounts as &'a AccountView
        let free_acct_params: Vec<TokenStream2> = acct_idents
            .iter()
            .map(|name| quote! { #name: &'a quasar_core::prelude::AccountView })
            .collect();

        free_functions.push(quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                __program: &'a quasar_core::prelude::AccountView,
                #(#free_acct_params,)*
                #(#arg_params,)*
            ) -> quasar_core::cpi::CpiCall<'a, #acct_count, #data_size> {
                let __data = #data_write;
                quasar_core::cpi::CpiCall::new(
                    __program.address(),
                    [#(#ia_entries),*],
                    [#(#acct_idents),*],
                    __data,
                )
            }
        });

        // Method variant: accounts as &'a impl AsAccountView
        let method_acct_params: Vec<TokenStream2> = acct_idents
            .iter()
            .map(|name| quote! { #name: &'a impl quasar_core::traits::AsAccountView })
            .collect();

        let method_acct_conversions: Vec<TokenStream2> = acct_idents
            .iter()
            .map(|name| quote! { #name.to_account_view() })
            .collect();

        let arg_names: Vec<Ident> = ix
            .args
            .iter()
            .map(|a| Ident::new(&pascal_to_snake(&a.name), Span::call_site()))
            .collect();

        method_impls.push(quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                &'a self,
                #(#method_acct_params,)*
                #(#arg_params,)*
            ) -> quasar_core::cpi::CpiCall<'a, #acct_count, #data_size> {
                #fn_name(
                    self.to_account_view(),
                    #(#method_acct_conversions,)*
                    #(#arg_names,)*
                )
            }
        });
    }

    quote! {
        pub mod #mod_name {
            pub const ID: quasar_core::prelude::Address = #address_tokens;

            quasar_core::define_account!(
                pub struct #program_type_name =>
                    [quasar_core::checks::Executable, quasar_core::checks::Address]
            );

            impl quasar_core::traits::Id for #program_type_name {
                const ID: quasar_core::prelude::Address = ID;
            }

            #(#free_functions)*

            impl #program_type_name {
                #(#method_impls)*
            }
        }
    }
    .into()
}
