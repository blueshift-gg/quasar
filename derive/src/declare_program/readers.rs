use {
    super::{collect_type_refs as collect_nested_type_refs, primitive_size, sanitize_ident},
    crate::helpers::pascal_to_snake,
    proc_macro2::{Span, TokenStream as TokenStream2},
    quasar_idl_schema::{IdlAccountDef, IdlLayout, IdlType, IdlTypeDef},
    quote::quote,
    std::collections::{HashMap, HashSet},
};

/// Collect account types and every custom type they contain.
pub(super) fn collect_account_type_refs(
    accounts: &[IdlAccountDef],
    idl_types: &[IdlTypeDef],
    out: &mut HashSet<String>,
) -> Result<(), String> {
    for account in accounts {
        let type_def = account_type(account, idl_types)?;
        if out.insert(type_def.name.clone()) {
            for field in &type_def.fields {
                collect_nested_type_refs(&field.ty, idl_types, out);
            }
        }
    }
    Ok(())
}

/// Emit owned, read-only decoders for fixed-layout foreign accounts.
pub(super) fn emit(
    accounts: &[IdlAccountDef],
    idl_types: &[IdlTypeDef],
    type_sizes: &HashMap<String, usize>,
) -> Result<Vec<TokenStream2>, String> {
    let krate = crate::krate::lang_path();
    let mut readers = Vec::new();
    let mut emitted = HashSet::new();

    for account in accounts {
        if !emitted.insert(account.name.as_str()) {
            return Err(format!("duplicate account definition '{}'", account.name));
        }
        let type_def = account_type(account, idl_types)?;
        if matches!(type_def.layout, Some(IdlLayout::Compact { .. })) {
            return Err(format!(
                "account '{}' uses a dynamic compact layout; fixed-layout account readers \
                 cannot decode it",
                account.name
            ));
        }
        let payload_size = type_sizes
            .get(&account.name)
            .copied()
            .ok_or_else(|| format!("missing fixed byte size for account '{}'", account.name))?;
        let decoded_len = account
            .discriminator
            .len()
            .checked_add(payload_size)
            .ok_or_else(|| format!("account '{}' data length overflows usize", account.name))?;
        let data_len = account_data_len(account, decoded_len)?;
        let name = sanitize_ident(&account.name, Span::call_site()).map_err(|e| e.to_string())?;
        let discriminator: Vec<_> = account
            .discriminator
            .iter()
            .copied()
            .map(proc_macro2::Literal::u8_suffixed)
            .collect();
        let discriminator_checks: Vec<_> = account
            .discriminator
            .iter()
            .copied()
            .enumerate()
            .map(|(index, byte)| quote! { __data[#index] != #byte })
            .collect();
        let discriminator_check = if discriminator_checks.is_empty() {
            quote! {}
        } else {
            quote! {
                if #(#discriminator_checks)||* {
                    return Err(#krate::prelude::ProgramError::InvalidAccountData);
                }
            }
        };

        let mut offset = account.discriminator.len();
        let fields: Vec<TokenStream2> = type_def
            .fields
            .iter()
            .map(|field| {
                let field_name = sanitize_ident(&pascal_to_snake(&field.name), Span::call_site())
                    .map_err(|error| error.to_string())?;
                let value = emit_field_read(&field.ty, &mut offset, idl_types)?;
                Ok(quote! { #field_name: #value })
            })
            .collect::<Result<_, String>>()?;
        if offset != decoded_len {
            return Err(format!(
                "internal reader layout mismatch for account '{}': decoded {offset} bytes, \
                 expected {decoded_len}",
                account.name
            ));
        }

        readers.push(quote! {
            impl #name {
                /// Foreign account discriminator from the declared IDL.
                pub const ACCOUNT_DISCRIMINATOR: &'static [u8] = &[#(#discriminator),*];
                /// Minimum fixed account-data length, including the discriminator.
                pub const ACCOUNT_DATA_LEN: usize = #data_len;

                /// Validate and decode an owned snapshot of this foreign account.
                #[inline(always)]
                pub fn read_account(
                    view: &#krate::prelude::AccountView,
                ) -> Result<Self, #krate::prelude::ProgramError> {
                    if !view.owned_by(&ID) {
                        return Err(#krate::prelude::ProgramError::IllegalOwner);
                    }
                    if view.data_len() < Self::ACCOUNT_DATA_LEN {
                        return Err(#krate::prelude::ProgramError::AccountDataTooSmall);
                    }
                    let __data = view.try_borrow()?;
                    #discriminator_check
                    Ok(Self { #(#fields,)* })
                }
            }
        });
    }

    Ok(readers)
}

fn account_type<'a>(
    account: &IdlAccountDef,
    idl_types: &'a [IdlTypeDef],
) -> Result<&'a IdlTypeDef, String> {
    idl_types
        .iter()
        .find(|type_def| type_def.name == account.name)
        .ok_or_else(|| {
            format!(
                "account '{}' has no matching type definition for its reader",
                account.name
            )
        })
}

fn account_data_len(account: &IdlAccountDef, decoded_len: usize) -> Result<usize, String> {
    let Some(space) = &account.space else {
        return Ok(decoded_len);
    };
    if space
        .discriminator
        .is_some_and(|length| length != account.discriminator.len())
    {
        return Err(format!(
            "account '{}' space discriminator length disagrees with its discriminator",
            account.name
        ));
    }
    let minimum = usize::try_from(space.min).map_err(|_| {
        format!(
            "account '{}' minimum space does not fit usize",
            account.name
        )
    })?;
    if minimum < decoded_len {
        return Err(format!(
            "account '{}' minimum space {minimum} is smaller than its fixed layout length \
             {decoded_len}",
            account.name
        ));
    }
    Ok(minimum)
}

fn emit_field_read(
    ty: &IdlType,
    offset: &mut usize,
    idl_types: &[IdlTypeDef],
) -> Result<TokenStream2, String> {
    match ty {
        IdlType::Primitive(name) => emit_primitive_read(name, offset),
        IdlType::Defined { defined } => {
            let type_def = idl_types
                .iter()
                .find(|type_def| type_def.name == defined.name)
                .ok_or_else(|| format!("undefined type '{}'", defined.name))?;
            let name =
                sanitize_ident(&defined.name, Span::call_site()).map_err(|e| e.to_string())?;
            let fields: Vec<TokenStream2> = type_def
                .fields
                .iter()
                .map(|field| {
                    let field_name =
                        sanitize_ident(&pascal_to_snake(&field.name), Span::call_site())
                            .map_err(|error| error.to_string())?;
                    let value = emit_field_read(&field.ty, offset, idl_types)?;
                    Ok(quote! { #field_name: #value })
                })
                .collect::<Result<_, String>>()?;
            Ok(quote! { #name { #(#fields,)* } })
        }
        IdlType::Array { array } => {
            let elements: Vec<TokenStream2> = (0..array.1)
                .map(|_| emit_field_read(&array.0, offset, idl_types))
                .collect::<Result<_, String>>()?;
            Ok(quote! { [#(#elements),*] })
        }
        IdlType::Option { .. } => Err(
            "option uses a dynamic/tagged layout and is unsupported in fixed account readers"
                .into(),
        ),
        IdlType::Vec { .. } => {
            Err("vec uses a dynamic layout and is unsupported in fixed account readers".into())
        }
        IdlType::Generic { .. } => {
            Err("generic types are unsupported in fixed account readers".into())
        }
    }
}

fn emit_primitive_read(name: &str, offset: &mut usize) -> Result<TokenStream2, String> {
    let krate = crate::krate::lang_path();
    let size = primitive_size(name)?;
    let start = *offset;
    *offset = start
        .checked_add(size)
        .ok_or_else(|| "account reader offset overflows usize".to_string())?;

    match name {
        "u8" => Ok(quote! { __data[#start] }),
        "i8" => Ok(quote! { __data[#start] as i8 }),
        "bool" => Ok(quote! {
            match __data[#start] {
                0 => false,
                1 => true,
                _ => return Err(#krate::prelude::ProgramError::InvalidAccountData),
            }
        }),
        "pubkey" => {
            let indexes = start..*offset;
            Ok(quote! {
                #krate::prelude::Address::new_from_array([
                    #(__data[#indexes]),*
                ])
            })
        }
        "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "u128" | "i128" | "f32" | "f64" => {
            let rust_type = sanitize_ident(name, Span::call_site()).map_err(|e| e.to_string())?;
            let indexes = start..*offset;
            Ok(quote! { #rust_type::from_le_bytes([#(__data[#indexes]),*]) })
        }
        other => Err(format!(
            "primitive type '{other}' is unsupported in fixed account readers"
        )),
    }
}
