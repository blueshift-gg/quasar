use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};

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
        IdlType::Primitive(s) => match s.as_str() {
            "u8" => Ok(TypeInfo {
                rust_type: quote! { u8 },
                byte_size: 1,
                is_reference: false,
            }),
            "u16" => Ok(TypeInfo {
                rust_type: quote! { u16 },
                byte_size: 2,
                is_reference: false,
            }),
            "u32" => Ok(TypeInfo {
                rust_type: quote! { u32 },
                byte_size: 4,
                is_reference: false,
            }),
            "u64" => Ok(TypeInfo {
                rust_type: quote! { u64 },
                byte_size: 8,
                is_reference: false,
            }),
            "u128" => Ok(TypeInfo {
                rust_type: quote! { u128 },
                byte_size: 16,
                is_reference: false,
            }),
            "i8" => Ok(TypeInfo {
                rust_type: quote! { i8 },
                byte_size: 1,
                is_reference: false,
            }),
            "i16" => Ok(TypeInfo {
                rust_type: quote! { i16 },
                byte_size: 2,
                is_reference: false,
            }),
            "i32" => Ok(TypeInfo {
                rust_type: quote! { i32 },
                byte_size: 4,
                is_reference: false,
            }),
            "i64" => Ok(TypeInfo {
                rust_type: quote! { i64 },
                byte_size: 8,
                is_reference: false,
            }),
            "i128" => Ok(TypeInfo {
                rust_type: quote! { i128 },
                byte_size: 16,
                is_reference: false,
            }),
            "bool" => Ok(TypeInfo {
                rust_type: quote! { bool },
                byte_size: 1,
                is_reference: false,
            }),
            "pubkey" => Ok(TypeInfo {
                rust_type: quote! { &solana_address::Address },
                byte_size: 32,
                is_reference: true,
            }),
            other => Err(format!("unsupported primitive type '{other}'")),
        },
        IdlType::Defined { defined } => Err(format!(
            "defined type '{defined}' is not supported — CPI helpers require fixed-size args"
        )),
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

fn generate_data_write(args: &[IdlField], disc: &[u8]) -> Result<(TokenStream2, usize), String> {
    let disc_len = disc.len();
    let mut offset = disc_len;
    let mut write_stmts = Vec::new();

    // Write discriminator bytes
    for (i, &byte) in disc.iter().enumerate() {
        let byte_lit = proc_macro2::Literal::u8_suffixed(byte);
        let idx = i;
        write_stmts.push(quote! {
            core::ptr::write(__ptr.add(#idx), #byte_lit);
        });
    }

    // Write each arg
    for field in args {
        let info = map_idl_type(&field.ty)?;
        let fname = Ident::new(&to_snake_case(&field.name), Span::call_site());
        let size = info.byte_size;

        if info.is_reference {
            // &Address → copy 32 bytes
            write_stmts.push(quote! {
                core::ptr::copy_nonoverlapping(
                    #fname.as_ref().as_ptr(),
                    __ptr.add(#offset),
                    #size,
                );
            });
        } else if size == 1 {
            // Single byte types (u8, i8, bool)
            let rust_type = &info.rust_type;
            write_stmts.push(quote! {
                core::ptr::write(__ptr.add(#offset), #fname as #rust_type as u8);
            });
        } else {
            // Multi-byte types → to_le_bytes
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

fn generate_instruction(ix: &IdlInstruction) -> Result<TokenStream2, String> {
    let fn_name = Ident::new(&to_snake_case(&ix.name), Span::call_site());
    let acct_count = ix.accounts.len();

    // Build account parameter list (for free function: all &'a AccountView)
    let free_acct_params: Vec<TokenStream2> = ix
        .accounts
        .iter()
        .map(|a| {
            let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
            quote! { #name: &'a solana_account_view::AccountView }
        })
        .collect();

    // Build InstructionAccount array entries
    let ia_entries: Vec<TokenStream2> = ix
        .accounts
        .iter()
        .map(|a| {
            let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
            match (a.writable, a.signer) {
                (true, true) => {
                    quote! { quasar_core::cpi::InstructionAccount::writable_signer(#name.address()) }
                }
                (true, false) => {
                    quote! { quasar_core::cpi::InstructionAccount::writable(#name.address()) }
                }
                (false, true) => {
                    quote! { quasar_core::cpi::InstructionAccount::readonly_signer(#name.address()) }
                }
                (false, false) => {
                    quote! { quasar_core::cpi::InstructionAccount::readonly(#name.address()) }
                }
            }
        })
        .collect();

    // Build views array entries
    let view_entries: Vec<TokenStream2> = ix
        .accounts
        .iter()
        .map(|a| {
            let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
            quote! { #name }
        })
        .collect();

    // Build arg parameters for the free function
    let arg_params: Vec<TokenStream2> = ix
        .args
        .iter()
        .map(|a| {
            let info = map_idl_type(&a.ty).expect("type already validated");
            let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
            let ty = &info.rust_type;
            quote! { #name: #ty }
        })
        .collect();

    // Build the data buffer
    let (data_write, data_size) = generate_data_write(&ix.args, &ix.discriminator)?;

    // --- Free function ---
    let free_fn = quote! {
        #[inline(always)]
        pub fn #fn_name<'a>(
            __program: &'a solana_account_view::AccountView,
            #(#free_acct_params,)*
            #(#arg_params,)*
        ) -> quasar_core::cpi::CpiCall<'a, #acct_count, #data_size> {
            let __data = #data_write;

            quasar_core::cpi::CpiCall::new(
                __program.address(),
                [#(#ia_entries),*],
                [#(#view_entries),*],
                __data,
            )
        }
    };

    // --- Method on program type ---
    // Method params use &'a impl AsAccountView instead of &'a AccountView
    let method_acct_params: Vec<TokenStream2> = ix
        .accounts
        .iter()
        .map(|a| {
            let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
            quote! { #name: &'a impl quasar_core::traits::AsAccountView }
        })
        .collect();

    // Method body: convert to account_view then delegate to free function
    let method_acct_conversions: Vec<TokenStream2> = ix
        .accounts
        .iter()
        .map(|a| {
            let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
            quote! { #name.to_account_view() }
        })
        .collect();

    let arg_names: Vec<Ident> = ix
        .args
        .iter()
        .map(|a| Ident::new(&to_snake_case(&a.name), Span::call_site()))
        .collect();

    let method_fn = quote! {
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
    };

    Ok(quote! {
        #free_fn
        __methods! { #method_fn }
    })
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let mut s = c.to_uppercase().to_string();
                    s.push_str(&chars.collect::<String>());
                    s
                }
                None => String::new(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn declare_program(input: TokenStream) -> TokenStream {
    let input2 = proc_macro2::TokenStream::from(input);
    let mut iter = input2.into_iter();

    // Parse: module_name, "path/to/idl.json"
    let mod_name = match iter.next() {
        Some(proc_macro2::TokenTree::Ident(id)) => id,
        _ => {
            return syn::Error::new(Span::call_site(), "expected module name as first argument")
                .to_compile_error()
                .into();
        }
    };

    // Skip comma
    match iter.next() {
        Some(proc_macro2::TokenTree::Punct(p)) if p.as_char() == ',' => {}
        _ => {
            return syn::Error::new(Span::call_site(), "expected comma after module name")
                .to_compile_error()
                .into();
        }
    };

    // Parse string literal (IDL path)
    let idl_path = match iter.next() {
        Some(proc_macro2::TokenTree::Literal(lit)) => {
            let s = lit.to_string();
            // Strip surrounding quotes
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

    // Parse IDL JSON at compile time
    let idl_json = match std::fs::read_to_string(&idl_path) {
        Ok(json) => json,
        Err(_) => {
            // Fall back to CARGO_MANIFEST_DIR-relative path
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

    // Generate program type name from module name
    let program_type_name = format_ident!("{}", to_pascal_case(&mod_name.to_string()));

    // Generate address bytes from IDL
    let address_str = &idl.address;
    let address_tokens = quote! {
        solana_address::address!(#address_str)
    };

    // Generate instruction code (free functions + method fragments)
    let mut free_functions = Vec::new();
    let mut method_impls = Vec::new();

    for ix in &idl.instructions {
        match generate_instruction(ix) {
            Ok(tokens) => {
                // We need to split free functions from methods.
                // The generate_instruction uses a __methods! sentinel.
                // We'll generate them separately instead.
                let _ = tokens; // discard combined
            }
            Err(msg) => {
                return syn::Error::new(Span::call_site(), msg)
                    .to_compile_error()
                    .into();
            }
        }

        // Generate free function directly
        let fn_name = Ident::new(&to_snake_case(&ix.name), Span::call_site());
        let acct_count = ix.accounts.len();

        let free_acct_params: Vec<TokenStream2> = ix
            .accounts
            .iter()
            .map(|a| {
                let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
                quote! { #name: &'a solana_account_view::AccountView }
            })
            .collect();

        let ia_entries: Vec<TokenStream2> = ix
            .accounts
            .iter()
            .map(|a| {
                let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
                match (a.writable, a.signer) {
                    (true, true) => quote! { quasar_core::cpi::InstructionAccount::writable_signer(#name.address()) },
                    (true, false) => quote! { quasar_core::cpi::InstructionAccount::writable(#name.address()) },
                    (false, true) => quote! { quasar_core::cpi::InstructionAccount::readonly_signer(#name.address()) },
                    (false, false) => quote! { quasar_core::cpi::InstructionAccount::readonly(#name.address()) },
                }
            })
            .collect();

        let view_entries: Vec<TokenStream2> = ix
            .accounts
            .iter()
            .map(|a| {
                let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
                quote! { #name }
            })
            .collect();

        let arg_params: Vec<TokenStream2> = ix
            .args
            .iter()
            .map(|a| {
                let info = map_idl_type(&a.ty).unwrap();
                let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
                let ty = &info.rust_type;
                quote! { #name: #ty }
            })
            .collect();

        let (data_write, data_size) = generate_data_write(&ix.args, &ix.discriminator).unwrap();

        free_functions.push(quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                __program: &'a solana_account_view::AccountView,
                #(#free_acct_params,)*
                #(#arg_params,)*
            ) -> quasar_core::cpi::CpiCall<'a, #acct_count, #data_size> {
                let __data = #data_write;

                quasar_core::cpi::CpiCall::new(
                    __program.address(),
                    [#(#ia_entries),*],
                    [#(#view_entries),*],
                    __data,
                )
            }
        });

        // Method variant
        let method_acct_params: Vec<TokenStream2> = ix
            .accounts
            .iter()
            .map(|a| {
                let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
                quote! { #name: &'a impl quasar_core::traits::AsAccountView }
            })
            .collect();

        let method_acct_conversions: Vec<TokenStream2> = ix
            .accounts
            .iter()
            .map(|a| {
                let name = Ident::new(&to_snake_case(&a.name), Span::call_site());
                quote! { #name.to_account_view() }
            })
            .collect();

        let arg_names: Vec<Ident> = ix
            .args
            .iter()
            .map(|a| Ident::new(&to_snake_case(&a.name), Span::call_site()))
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

    let output = quote! {
        pub mod #mod_name {
            pub const ID: solana_address::Address = #address_tokens;

            quasar_core::define_account!(
                pub struct #program_type_name =>
                    [quasar_core::checks::Executable, quasar_core::checks::Address]
            );

            impl quasar_core::traits::Id for #program_type_name {
                const ID: solana_address::Address = ID;
            }

            #(#free_functions)*

            impl #program_type_name {
                #(#method_impls)*
            }
        }
    };

    output.into()
}
