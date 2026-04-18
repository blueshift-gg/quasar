//! Parses `#[event]` structs for IDL generation.

use {
    crate::{
        parser::helpers,
        types::{IdlEventDef, IdlField, IdlTypeDef, IdlTypeDefKind, IdlTypeDefType},
    },
    syn::{Fields, Item},
};

/// Raw parsed data for a `#[event(discriminator = N)]` struct.
pub struct RawEvent {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub fields: Vec<(String, syn::Type)>,
}

/// Extract all `#[event(discriminator = N)]` structs from a parsed file.
pub fn extract_events(file: &syn::File) -> Vec<RawEvent> {
    let mut result = Vec::new();
    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            if let Some(disc) = get_event_discriminator(&item_struct.attrs) {
                let name = item_struct.ident.to_string();
                let fields = match &item_struct.fields {
                    Fields::Named(named) => named
                        .named
                        .iter()
                        .map(|f| {
                            let field_name = f.ident.as_ref().unwrap().to_string();
                            (field_name, f.ty.clone())
                        })
                        .collect(),
                    _ => vec![],
                };

                result.push(RawEvent {
                    name,
                    discriminator: disc,
                    fields,
                });
            }
        }
    }
    result
}

fn get_event_discriminator(attrs: &[syn::Attribute]) -> Option<Vec<u8>> {
    for attr in attrs {
        if !attr.path().is_ident("event") {
            continue;
        }

        let tokens = match attr.meta.require_list() {
            Ok(list) => list.tokens.to_string(),
            Err(_) => continue,
        };

        if !tokens.contains("discriminator") {
            continue;
        }

        return helpers::parse_discriminator_value(&tokens);
    }
    None
}

pub fn to_idl_event_def(raw: &RawEvent) -> IdlEventDef {
    IdlEventDef {
        name: raw.name.clone(),
        discriminator: raw.discriminator.clone(),
    }
}

pub fn to_idl_type_def(raw: &RawEvent) -> IdlTypeDef {
    let fields = raw
        .fields
        .iter()
        .map(|(name, ty)| IdlField {
            name: helpers::to_camel_case(name),
            ty: helpers::map_type_from_syn(ty),
        })
        .collect();

    IdlTypeDef {
        name: raw.name.clone(),
        ty: IdlTypeDefType {
            kind: IdlTypeDefKind::Struct,
            fields,
        },
    }
}
