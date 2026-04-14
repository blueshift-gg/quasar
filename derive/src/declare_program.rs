//! `declare_program!` — generates a typed CPI module from a program's IDL JSON.
//!
//! Produces account types, CPI helper functions (both free and method
//! variants), and optional custom struct definitions for cross-program
//! interaction without runtime IDL parsing.
//!
//! Uses canonical IDL types from `quasar_idl::types` — no duplicate
//! definitions.

use {
    crate::helpers::pascal_to_snake,
    proc_macro::TokenStream,
    proc_macro2::{Ident, Span, TokenStream as TokenStream2},
    quasar_idl::types::{Idl, IdlField, IdlType, IdlTypeDef},
    quote::{format_ident, quote},
    std::collections::{HashMap, HashSet},
};

// ---------------------------------------------------------------------------
// Type sizing — recursive computation with cycle detection
// ---------------------------------------------------------------------------

/// Compute byte sizes for all custom struct types in the IDL.
/// Returns an error if any type contains dynamic fields, circular references,
/// or non-struct kinds.
fn build_type_sizes(types: &[IdlTypeDef]) -> Result<HashMap<String, usize>, String> {
    let type_map: HashMap<&str, &[IdlField]> = types
        .iter()
        .map(|td| (td.name.as_str(), td.ty.fields.as_slice()))
        .collect();

    // Validate all types are structs (enum kind would produce wrong sizes).
    for td in types {
        if td.ty.kind != "struct" {
            return Err(format!(
                "type '{}' has kind '{}' — only structs are supported in CPI",
                td.name, td.ty.kind
            ));
        }
    }

    let mut sizes: HashMap<String, usize> = HashMap::new();
    let mut resolving: HashSet<String> = HashSet::new();

    for td in types {
        resolve_size(&td.name, &type_map, &mut sizes, &mut resolving)?;
    }
    Ok(sizes)
}

fn resolve_size(
    name: &str,
    type_map: &HashMap<&str, &[IdlField]>,
    sizes: &mut HashMap<String, usize>,
    resolving: &mut HashSet<String>,
) -> Result<usize, String> {
    if let Some(&size) = sizes.get(name) {
        return Ok(size);
    }
    if !resolving.insert(name.to_string()) {
        return Err(format!("circular type reference: '{name}'"));
    }
    let fields = type_map
        .get(name)
        .ok_or_else(|| format!("undefined type '{name}'"))?;
    let mut total = 0;
    for field in *fields {
        total += field_byte_size(&field.ty, type_map, sizes, resolving)?;
    }
    resolving.remove(name);
    sizes.insert(name.to_string(), total);
    Ok(total)
}

fn field_byte_size(
    ty: &IdlType,
    type_map: &HashMap<&str, &[IdlField]>,
    sizes: &mut HashMap<String, usize>,
    resolving: &mut HashSet<String>,
) -> Result<usize, String> {
    match ty {
        IdlType::Primitive(p) => primitive_size(p),
        IdlType::Defined { defined } => resolve_size(defined, type_map, sizes, resolving),
        IdlType::DynString { .. } => {
            Err("dynamic string not supported in CPI — only fixed-size types allowed".into())
        }
        IdlType::DynVec { .. } => {
            Err("dynamic vec not supported in CPI — only fixed-size types allowed".into())
        }
    }
}

fn primitive_size(name: &str) -> Result<usize, String> {
    match name {
        "u8" | "i8" | "bool" => Ok(1),
        "u16" | "i16" => Ok(2),
        "u32" | "i32" => Ok(4),
        "u64" | "i64" => Ok(8),
        "u128" | "i128" => Ok(16),
        "publicKey" => Ok(32),
        other => Err(format!("unsupported primitive type '{other}'")),
    }
}

// ---------------------------------------------------------------------------
// Type mapping — converts IDL types to Rust token streams
// ---------------------------------------------------------------------------

struct TypeInfo {
    /// Rust type for function parameters (pubkey → &Address).
    param_type: TokenStream2,
    /// Rust type for struct field definitions (pubkey → Address).
    field_type: TokenStream2,
}

fn map_idl_type(ty: &IdlType, type_sizes: &HashMap<String, usize>) -> Result<TypeInfo, String> {
    match ty {
        IdlType::Primitive(s) => {
            let rust_type = match s.as_str() {
                "u8" => quote! { u8 },
                "i8" => quote! { i8 },
                "bool" => quote! { bool },
                "u16" => quote! { u16 },
                "i16" => quote! { i16 },
                "u32" => quote! { u32 },
                "i32" => quote! { i32 },
                "u64" => quote! { u64 },
                "i64" => quote! { i64 },
                "u128" => quote! { u128 },
                "i128" => quote! { i128 },
                "publicKey" => {
                    return Ok(TypeInfo {
                        param_type: quote! { &quasar_lang::prelude::Address },
                        field_type: quote! { quasar_lang::prelude::Address },
                    });
                }
                other => return Err(format!("unsupported primitive type '{other}'")),
            };
            Ok(TypeInfo {
                param_type: rust_type.clone(),
                field_type: rust_type,
            })
        }
        IdlType::Defined { defined } => {
            if !type_sizes.contains_key(defined.as_str()) {
                return Err(format!("undefined type '{defined}'"));
            }
            let ident = Ident::new(defined, Span::call_site());
            Ok(TypeInfo {
                param_type: quote! { #ident },
                field_type: quote! { #ident },
            })
        }
        IdlType::DynString { .. } => {
            Err("dynamic string not supported in CPI — only fixed-size types allowed".into())
        }
        IdlType::DynVec { .. } => {
            Err("dynamic vec not supported in CPI — only fixed-size types allowed".into())
        }
    }
}

// ---------------------------------------------------------------------------
// Code generation helpers
// ---------------------------------------------------------------------------

/// Generate the data write block for instruction args, flattening struct fields
/// recursively into a packed byte buffer.
fn generate_data_write(
    args: &[IdlField],
    disc: &[u8],
    idl_types: &[IdlTypeDef],
) -> Result<(TokenStream2, usize), String> {
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
        let fname = Ident::new(&pascal_to_snake(&field.name), Span::call_site());
        emit_field_write(
            &mut write_stmts,
            &mut offset,
            &quote! { #fname },
            &field.ty,
            idl_types,
        )?;
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

/// Emit write statements for a single field, recursing into struct sub-fields.
fn emit_field_write(
    stmts: &mut Vec<TokenStream2>,
    offset: &mut usize,
    access: &TokenStream2,
    ty: &IdlType,
    idl_types: &[IdlTypeDef],
) -> Result<(), String> {
    match ty {
        IdlType::Primitive(p) => {
            let size = primitive_size(p)?;
            if p == "publicKey" {
                stmts.push(quote! {
                    core::ptr::copy_nonoverlapping(
                        #access.as_ref().as_ptr(),
                        __ptr.add(#offset),
                        #size,
                    );
                });
            } else if size == 1 {
                stmts.push(quote! {
                    core::ptr::write(__ptr.add(#offset), #access as u8);
                });
            } else {
                stmts.push(quote! {
                    core::ptr::copy_nonoverlapping(
                        #access.to_le_bytes().as_ptr(),
                        __ptr.add(#offset),
                        #size,
                    );
                });
            }
            *offset += size;
        }
        IdlType::Defined { defined } => {
            // Recurse into the struct's fields
            let td = idl_types
                .iter()
                .find(|t| t.name == *defined)
                .ok_or_else(|| format!("undefined type '{defined}'"))?;
            for sub_field in &td.ty.fields {
                let sub_name = Ident::new(&pascal_to_snake(&sub_field.name), Span::call_site());
                let sub_access = quote! { #access.#sub_name };
                emit_field_write(stmts, offset, &sub_access, &sub_field.ty, idl_types)?;
            }
        }
        IdlType::DynString { .. } | IdlType::DynVec { .. } => {
            return Err("dynamic types not supported in CPI".into());
        }
    }
    Ok(())
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

/// Emit struct definitions for custom types referenced by instruction args.
fn emit_struct_defs(
    idl_types: &[IdlTypeDef],
    referenced: &HashSet<String>,
    type_sizes: &HashMap<String, usize>,
) -> Result<Vec<TokenStream2>, String> {
    let mut defs = Vec::new();

    for td in idl_types {
        if !referenced.contains(&td.name) {
            continue;
        }
        let name = Ident::new(&td.name, Span::call_site());
        let fields: Vec<TokenStream2> = td
            .ty
            .fields
            .iter()
            .map(|f| {
                let fname = Ident::new(&pascal_to_snake(&f.name), Span::call_site());
                let info = map_idl_type(&f.ty, type_sizes)?;
                let fty = &info.field_type;
                Ok(quote! { pub #fname: #fty })
            })
            .collect::<Result<Vec<_>, String>>()?;

        defs.push(quote! {
            #[derive(Clone, Copy)]
            pub struct #name {
                #(#fields,)*
            }
        });
    }

    Ok(defs)
}

/// Collect all Defined type names referenced (transitively) from instruction
/// args.
fn collect_referenced_types(
    instructions: &[quasar_idl::types::IdlInstruction],
    idl_types: &[IdlTypeDef],
) -> HashSet<String> {
    let mut referenced = HashSet::new();
    for ix in instructions {
        for arg in &ix.args {
            collect_type_refs(&arg.ty, idl_types, &mut referenced);
        }
    }
    referenced
}

fn collect_type_refs(ty: &IdlType, idl_types: &[IdlTypeDef], out: &mut HashSet<String>) {
    match ty {
        IdlType::Defined { defined } if out.insert(defined.clone()) => {
            // Recurse into nested types
            if let Some(td) = idl_types.iter().find(|t| t.name == *defined) {
                for field in &td.ty.fields {
                    collect_type_refs(&field.ty, idl_types, out);
                }
            }
        }
        IdlType::Defined { .. } => {}
        IdlType::DynVec { vec } => collect_type_refs(&vec.items, idl_types, out),
        IdlType::Primitive(_) | IdlType::DynString { .. } => {}
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

    // Build type sizes for custom struct support
    let type_sizes = match build_type_sizes(&idl.types) {
        Ok(sizes) => sizes,
        Err(msg) => {
            return syn::Error::new(Span::call_site(), msg)
                .to_compile_error()
                .into();
        }
    };

    // Validate all arg types up front
    for ix in &idl.instructions {
        for arg in &ix.args {
            if let Err(msg) = map_idl_type(&arg.ty, &type_sizes) {
                let full_msg = format!("in instruction '{}', arg '{}': {}", ix.name, arg.name, msg);
                return syn::Error::new(Span::call_site(), full_msg)
                    .to_compile_error()
                    .into();
            }
        }
    }

    // Collect and generate custom struct definitions
    let referenced = collect_referenced_types(&idl.instructions, &idl.types);
    let struct_defs = match emit_struct_defs(&idl.types, &referenced, &type_sizes) {
        Ok(defs) => defs,
        Err(msg) => {
            return syn::Error::new(Span::call_site(), msg)
                .to_compile_error()
                .into();
        }
    };

    let program_type_name =
        format_ident!("{}", crate::helpers::snake_to_pascal(&mod_name.to_string()));
    let address_str = &idl.address;
    let address_tokens = quote! { quasar_lang::prelude::address!(#address_str) };

    let mut free_functions = Vec::new();
    let mut method_impls = Vec::new();

    for ix in &idl.instructions {
        let fn_name = Ident::new(&pascal_to_snake(&ix.name), Span::call_site());
        let acct_count = ix.accounts.len();

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
                quote! { quasar_lang::cpi::InstructionAccount::#method(#name.address()) }
            })
            .collect();

        let arg_params: Vec<TokenStream2> = match ix
            .args
            .iter()
            .map(|a| {
                let info = map_idl_type(&a.ty, &type_sizes).map_err(|msg| {
                    syn::Error::new(
                        Span::call_site(),
                        format!("in instruction '{}', arg '{}': {}", ix.name, a.name, msg),
                    )
                })?;
                let name = Ident::new(&pascal_to_snake(&a.name), Span::call_site());
                let ty = &info.param_type;
                Ok(quote! { #name: #ty })
            })
            .collect::<Result<Vec<_>, syn::Error>>()
        {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };

        let (data_write, data_size) =
            match generate_data_write(&ix.args, &ix.discriminator, &idl.types) {
                Ok(v) => v,
                Err(msg) => {
                    return syn::Error::new(Span::call_site(), msg)
                        .to_compile_error()
                        .into()
                }
            };

        // Free function: accounts as &'a AccountView
        let free_acct_params: Vec<TokenStream2> = acct_idents
            .iter()
            .map(|name| quote! { #name: &'a quasar_lang::prelude::AccountView })
            .collect();

        free_functions.push(quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                __program: &'a quasar_lang::prelude::AccountView,
                #(#free_acct_params,)*
                #(#arg_params,)*
            ) -> quasar_lang::cpi::CpiCall<'a, #acct_count, #data_size> {
                let __data = #data_write;
                quasar_lang::cpi::CpiCall::new(
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
            .map(|name| quote! { #name: &'a impl quasar_lang::traits::AsAccountView })
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
            ) -> quasar_lang::cpi::CpiCall<'a, #acct_count, #data_size> {
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
            pub const ID: quasar_lang::prelude::Address = #address_tokens;

            quasar_lang::define_account!(
                pub struct #program_type_name =>
                    [quasar_lang::checks::Executable, quasar_lang::checks::Address]
            );

            impl quasar_lang::traits::Id for #program_type_name {
                const ID: quasar_lang::prelude::Address = ID;
            }

            #(#struct_defs)*

            #(#free_functions)*

            impl #program_type_name {
                #(#method_impls)*
            }
        }
    }
    .into()
}
