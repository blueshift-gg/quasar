use {
    crate::{
        parser::helpers,
        types::{IdlEventDef, IdlField, IdlTypeDef, IdlTypeDefType},
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

        return parse_discriminator_value(&tokens);
    }
    None
}

fn parse_discriminator_value(tokens_str: &str) -> Option<Vec<u8>> {
    let eq_pos = tokens_str.find('=')?;
    let value_str = tokens_str[eq_pos + 1..].trim();

    if value_str.starts_with('[') {
        let inner = value_str.trim_start_matches('[').trim_end_matches(']');
        let bytes: Vec<u8> = inner
            .split(',')
            .filter_map(|s| s.trim().parse::<u8>().ok())
            .collect();
        if bytes.is_empty() {
            None
        } else {
            Some(bytes)
        }
    } else {
        let byte: u8 = value_str
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?;
        Some(vec![byte])
    }
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
            kind: "struct".to_string(),
            fields,
        },
    }
}
