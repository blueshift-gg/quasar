//! `declare_program!`: generates a typed CPI module from a program's IDL JSON.
//!
//! Produces account types, CPI helper functions (both free and method
//! variants), and optional custom struct definitions for cross-program
//! interaction without runtime IDL parsing.
//!
//! Uses canonical schema types from `quasar_idl_schema`: the
//! `quasar-idl/1.0.0` contract.

use {
    crate::helpers::pascal_to_snake,
    proc_macro::TokenStream,
    proc_macro2::{Ident, Span, TokenStream as TokenStream2},
    quasar_idl_schema::{
        AccountFlag, Idl, IdlArg, IdlFieldDef, IdlInstruction, IdlType, IdlTypeDef, IdlTypeDefKind,
    },
    quote::{format_ident, quote},
    std::collections::{HashMap, HashSet},
    syn::{
        parse::{Parse, ParseStream},
        parse_macro_input, LitStr, Token,
    },
};

struct DeclareProgramInput {
    mod_name: Ident,
    idl_path: LitStr,
}

impl Parse for DeclareProgramInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mod_name = input.parse()?;
        input.parse::<Token![,]>()?;
        let idl_path = input.parse()?;
        Ok(Self { mod_name, idl_path })
    }
}

/// Turn an IDL-supplied name into a valid Rust identifier.
///
/// `Ident::new` panics on reserved words (`type`, `match`, `move`, ...), so
/// every IDL string must pass through here. Keywords become raw identifiers
/// (`type` -> `r#type`); the few that cannot be raw (`crate`/`self`/`super`/
/// `Self`) get a trailing underscore. Anything still unusable is a spanned
/// error pointing at the macro invocation rather than a proc-macro panic.
fn sanitize_ident(name: &str, span: Span) -> syn::Result<Ident> {
    if let Ok(id) = syn::parse_str::<Ident>(name) {
        return Ok(id);
    }
    if let Ok(id) = syn::parse_str::<Ident>(&format!("r#{name}")) {
        return Ok(id);
    }
    if let Ok(id) = syn::parse_str::<Ident>(&format!("{name}_")) {
        return Ok(id);
    }
    Err(syn::Error::new(
        span,
        format!("IDL name `{name}` is not a valid Rust identifier"),
    ))
}

/// Compute byte sizes for all custom struct types in the IDL.
/// Returns an error if any type contains dynamic fields, circular references,
/// or non-struct kinds.
fn build_type_sizes(types: &[IdlTypeDef]) -> Result<HashMap<String, usize>, String> {
    let type_map: HashMap<&str, &[IdlFieldDef]> = types
        .iter()
        .filter(|td| td.kind == IdlTypeDefKind::Struct)
        .map(|td| (td.name.as_str(), td.fields.as_slice()))
        .collect();

    // Validate all types are structs (enum kind would produce wrong sizes).
    for td in types {
        if td.kind != IdlTypeDefKind::Struct {
            return Err(format!(
                "type '{}' has kind '{:?}': only structs are supported in CPI",
                td.name, td.kind
            ));
        }
    }

    let mut sizes: HashMap<String, usize> = HashMap::new();
    let mut resolving: HashSet<&str> = HashSet::new();

    for td in types {
        resolve_size(td.name.as_str(), &type_map, &mut sizes, &mut resolving)?;
    }
    Ok(sizes)
}

fn resolve_size<'a>(
    name: &'a str,
    type_map: &HashMap<&'a str, &'a [IdlFieldDef]>,
    sizes: &mut HashMap<String, usize>,
    resolving: &mut HashSet<&'a str>,
) -> Result<usize, String> {
    if let Some(&size) = sizes.get(name) {
        return Ok(size);
    }
    if !resolving.insert(name) {
        return Err(format!("circular type reference: '{name}'"));
    }
    let fields = type_map
        .get(name)
        .ok_or_else(|| format!("undefined type '{name}'"))?;
    let mut total: usize = 0;
    for field in *fields {
        let field_size = field_byte_size(&field.ty, type_map, sizes, resolving)?;
        total = total
            .checked_add(field_size)
            .ok_or_else(|| format!("type '{name}' byte size overflows usize"))?;
    }
    resolving.remove(name);
    sizes.insert(name.to_string(), total);
    Ok(total)
}

fn field_byte_size<'a>(
    ty: &'a IdlType,
    type_map: &HashMap<&'a str, &'a [IdlFieldDef]>,
    sizes: &mut HashMap<String, usize>,
    resolving: &mut HashSet<&'a str>,
) -> Result<usize, String> {
    match ty {
        IdlType::Primitive(p) => primitive_size(p),
        IdlType::Defined { defined } => resolve_size(&defined.name, type_map, sizes, resolving),
        IdlType::Option { .. } => {
            Err("option not supported in CPI: only fixed-size types allowed".into())
        }
        IdlType::Vec { .. } => {
            Err("dynamic vec not supported in CPI: only fixed-size types allowed".into())
        }
        IdlType::Array { .. } => Err("fixed array not yet supported in CPI".into()),
        IdlType::Generic { .. } => Err("generic types not supported in CPI".into()),
    }
}

fn primitive_size(name: &str) -> Result<usize, String> {
    match name {
        "u8" | "i8" | "bool" => Ok(1),
        "u16" | "i16" => Ok(2),
        "u32" | "i32" | "f32" => Ok(4),
        "u64" | "i64" | "f64" => Ok(8),
        "u128" | "i128" => Ok(16),
        "pubkey" => Ok(32),
        "string" => {
            Err("dynamic string not supported in CPI: only fixed-size types allowed".into())
        }
        "bytes" => Err("dynamic bytes not supported in CPI: only fixed-size types allowed".into()),
        other => Err(format!("unsupported primitive type '{other}'")),
    }
}

struct TypeInfo {
    /// Rust type for function parameters (pubkey -> &Address).
    param_type: TokenStream2,
    /// Rust type for struct field definitions (pubkey -> Address).
    field_type: TokenStream2,
}

fn map_idl_type(ty: &IdlType, type_sizes: &HashMap<String, usize>) -> Result<TypeInfo, String> {
    let krate = crate::krate::lang_path();
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
                "f32" => quote! { f32 },
                "f64" => quote! { f64 },
                "pubkey" => {
                    return Ok(TypeInfo {
                        param_type: quote! { &#krate::prelude::Address },
                        field_type: quote! { #krate::prelude::Address },
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
            if !type_sizes.contains_key(defined.name.as_str()) {
                return Err(format!("undefined type '{}'", defined.name));
            }
            let ident =
                sanitize_ident(&defined.name, Span::call_site()).map_err(|e| e.to_string())?;
            Ok(TypeInfo {
                param_type: quote! { #ident },
                field_type: quote! { #ident },
            })
        }
        IdlType::Option { .. } => {
            Err("option not supported in CPI: only fixed-size types allowed".into())
        }
        IdlType::Vec { .. } => {
            Err("dynamic vec not supported in CPI: only fixed-size types allowed".into())
        }
        IdlType::Array { .. } => Err("fixed array not yet supported in CPI".into()),
        IdlType::Generic { .. } => Err("generic types not supported in CPI".into()),
    }
}

/// Generate the data write block for instruction args, flattening struct fields
/// recursively into a packed byte buffer.
fn generate_data_write(
    args: &[IdlArg],
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
        let fname = sanitize_ident(&pascal_to_snake(&field.name), Span::call_site())
            .map_err(|e| e.to_string())?;
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
        {
            let mut __buf = core::mem::MaybeUninit::<[u8; #total_size]>::uninit();
            let __ptr = __buf.as_mut_ptr() as *mut u8;
            // SAFETY: generated offsets cover each byte in the fixed CPI buffer
            // exactly once before `assume_init`.
            unsafe {
                #(#write_stmts)*
                __buf.assume_init()
            }
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
            let field_offset = *offset;
            let next_offset = field_offset
                .checked_add(size)
                .ok_or_else(|| "CPI instruction data size overflows usize".to_string())?;
            if p == "pubkey" {
                stmts.push(quote! {
                    core::ptr::copy_nonoverlapping(
                        #access.as_ref().as_ptr(),
                        __ptr.add(#field_offset),
                        #size,
                    );
                });
            } else if size == 1 {
                stmts.push(quote! {
                    core::ptr::write(__ptr.add(#field_offset), #access as u8);
                });
            } else {
                stmts.push(quote! {
                    core::ptr::copy_nonoverlapping(
                        #access.to_le_bytes().as_ptr(),
                        __ptr.add(#field_offset),
                        #size,
                    );
                });
            }
            *offset = next_offset;
        }
        IdlType::Defined { defined } => {
            let td = idl_types
                .iter()
                .find(|t| t.name == defined.name)
                .ok_or_else(|| format!("undefined type '{}'", defined.name))?;
            for sub_field in &td.fields {
                let sub_name = sanitize_ident(&pascal_to_snake(&sub_field.name), Span::call_site())
                    .map_err(|e| e.to_string())?;
                let sub_access = quote! { #access.#sub_name };
                emit_field_write(stmts, offset, &sub_access, &sub_field.ty, idl_types)?;
            }
        }
        IdlType::Option { .. }
        | IdlType::Vec { .. }
        | IdlType::Array { .. }
        | IdlType::Generic { .. } => {
            return Err("dynamic types not supported in CPI".into());
        }
    }
    Ok(())
}

/// Build an InstructionAccount constructor call for the given account flags.
fn ia_constructor(writable: &AccountFlag, signer: &AccountFlag) -> &'static str {
    match (writable.is_true(), signer.is_true()) {
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
        let name = sanitize_ident(&td.name, Span::call_site()).map_err(|e| e.to_string())?;
        let fields: Vec<TokenStream2> = td
            .fields
            .iter()
            .map(|f| {
                let fname = sanitize_ident(&pascal_to_snake(&f.name), Span::call_site())
                    .map_err(|e| e.to_string())?;
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
    instructions: &[IdlInstruction],
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
        IdlType::Defined { defined } if out.insert(defined.name.clone()) => {
            if let Some(td) = idl_types.iter().find(|t| t.name == defined.name) {
                for field in &td.fields {
                    collect_type_refs(&field.ty, idl_types, out);
                }
            }
        }
        IdlType::Defined { .. } => {}
        IdlType::Option { option } => collect_type_refs(option, idl_types, out),
        IdlType::Vec { vec } => collect_type_refs(vec, idl_types, out),
        IdlType::Array { array } => collect_type_refs(&array.0, idl_types, out),
        IdlType::Primitive(_) | IdlType::Generic { .. } => {}
    }
}

pub fn declare_program(input: TokenStream) -> TokenStream {
    let krate = crate::krate::lang_path();
    let DeclareProgramInput { mod_name, idl_path } =
        parse_macro_input!(input as DeclareProgramInput);
    let idl_path = idl_path.value();

    if idl_path.is_empty() {
        return syn::Error::new(Span::call_site(), "IDL path cannot be empty")
            .to_compile_error()
            .into();
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

    // Spec-version gate: diagnose an incompatible schema up front so the caller
    // gets a clear message instead of a confusing field-level parse error.
    if let Err(msg) = quasar_idl_schema::check_spec(&idl_json) {
        return syn::Error::new(Span::call_site(), msg)
            .to_compile_error()
            .into();
    }

    let idl: Idl = match serde_json::from_str(&idl_json) {
        Ok(idl) => idl,
        Err(e) => {
            let msg = format!("failed to parse IDL JSON: {e}");
            return syn::Error::new(Span::call_site(), msg)
                .to_compile_error()
                .into();
        }
    };

    let type_sizes = match build_type_sizes(&idl.types) {
        Ok(sizes) => sizes,
        Err(msg) => {
            return syn::Error::new(Span::call_site(), msg)
                .to_compile_error()
                .into();
        }
    };

    // Instruction arg types are validated once, below, where `arg_params` calls
    // `map_idl_type` with the identical "in instruction '..', arg '..'" message.

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
    let address_tokens = quote! { #krate::prelude::address!(#address_str) };

    let mut free_functions = Vec::new();
    let mut method_impls = Vec::new();

    for ix in &idl.instructions {
        let fn_name = match sanitize_ident(&pascal_to_snake(&ix.name), Span::call_site()) {
            Ok(id) => id,
            Err(e) => return e.to_compile_error().into(),
        };
        let acct_count = ix.accounts.len();

        let acct_idents: Vec<Ident> = match ix
            .accounts
            .iter()
            .map(|a| sanitize_ident(&pascal_to_snake(&a.name), Span::call_site()))
            .collect::<syn::Result<Vec<_>>>()
        {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };

        let ia_entries: Vec<TokenStream2> = ix
            .accounts
            .iter()
            .zip(&acct_idents)
            .map(|(a, name)| {
                let method = Ident::new(ia_constructor(&a.writable, &a.signer), Span::call_site());
                quote! { #krate::cpi::InstructionAccount::#method(#name.address()) }
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
                let name = sanitize_ident(&pascal_to_snake(&a.name), Span::call_site())?;
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
            .map(|name| quote! { #name: &'a #krate::prelude::AccountView })
            .collect();

        free_functions.push(quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                __program: &'a #krate::prelude::AccountView,
                #(#free_acct_params,)*
                #(#arg_params,)*
            ) -> #krate::cpi::CpiCall<'a, #acct_count, #data_size> {
                let __data = #data_write;
                #krate::cpi::CpiCall::new(
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
            .map(|name| quote! { #name: &'a impl #krate::traits::AsAccountView })
            .collect();

        let method_acct_conversions: Vec<TokenStream2> = acct_idents
            .iter()
            .map(|name| quote! { #name.to_account_view() })
            .collect();

        let arg_names: Vec<Ident> = match ix
            .args
            .iter()
            .map(|a| sanitize_ident(&pascal_to_snake(&a.name), Span::call_site()))
            .collect::<syn::Result<Vec<_>>>()
        {
            Ok(v) => v,
            Err(e) => return e.to_compile_error().into(),
        };

        method_impls.push(quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                &'a self,
                #(#method_acct_params,)*
                #(#arg_params,)*
            ) -> #krate::cpi::CpiCall<'a, #acct_count, #data_size> {
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
            pub const ID: #krate::prelude::Address = #address_tokens;

            #krate::define_account!(
                pub struct #program_type_name =>
                    [#krate::checks::Executable, #krate::checks::Address]
            );

            impl #krate::traits::Id for #program_type_name {
                const ID: #krate::prelude::Address = ID;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_ident_keywords_become_raw() {
        let sp = Span::call_site();
        assert_eq!(sanitize_ident("type", sp).unwrap().to_string(), "r#type");
        assert_eq!(sanitize_ident("match", sp).unwrap().to_string(), "r#match");
        assert_eq!(sanitize_ident("move", sp).unwrap().to_string(), "r#move");
        assert_eq!(sanitize_ident("fn", sp).unwrap().to_string(), "r#fn");
    }

    #[test]
    fn sanitize_ident_non_raw_keywords_get_suffix() {
        let sp = Span::call_site();
        // crate/self/super/Self cannot be raw identifiers.
        assert_eq!(sanitize_ident("crate", sp).unwrap().to_string(), "crate_");
        assert_eq!(sanitize_ident("self", sp).unwrap().to_string(), "self_");
        assert_eq!(sanitize_ident("super", sp).unwrap().to_string(), "super_");
        assert_eq!(sanitize_ident("Self", sp).unwrap().to_string(), "Self_");
    }

    #[test]
    fn sanitize_ident_valid_passthrough() {
        let sp = Span::call_site();
        assert_eq!(
            sanitize_ident("my_account", sp).unwrap().to_string(),
            "my_account"
        );
    }

    #[test]
    fn sanitize_ident_unusable_errors() {
        let sp = Span::call_site();
        assert!(sanitize_ident("has spaces", sp).is_err());
        assert!(sanitize_ident("1leading", sp).is_err());
        assert!(sanitize_ident("", sp).is_err());
    }

    #[test]
    fn map_idl_type_rejects_unsupported_arg_types() {
        // The single kept arg-type validation still rejects dynamic/unsupported
        // types with the same diagnostic the deleted pre-pass emitted.
        let sizes = HashMap::new();
        let opt = IdlType::Option {
            option: Box::new(IdlType::Primitive("u8".to_owned())),
        };
        assert!(map_idl_type(&opt, &sizes).is_err());
        let v = IdlType::Vec {
            vec: Box::new(IdlType::Primitive("u8".to_owned())),
        };
        assert!(map_idl_type(&v, &sizes).is_err());
        // Supported fixed primitive still maps cleanly.
        assert!(map_idl_type(&IdlType::Primitive("u64".to_owned()), &sizes).is_ok());
    }
}
