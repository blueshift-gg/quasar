use {
    crate::type_map::{classify_field, FieldKind},
    proc_macro2::TokenStream,
    quote::quote,
    syn::{DeriveInput, Fields},
};

pub struct Schema {
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub fields: Vec<SchemaField>,
    pub is_compact: bool,
}

pub struct SchemaField {
    pub name: syn::Ident,
    pub ty: syn::Type,
    pub kind: FieldKind,
    pub vis: syn::Visibility,
}

impl Schema {
    pub fn parse(input: &DeriveInput) -> Result<Schema, TokenStream> {
        // Only structs with named fields.
        let named_fields = match &input.data {
            syn::Data::Struct(data) => match &data.fields {
                Fields::Named(named) => &named.named,
                _ => {
                    return Err(
                        quote! { compile_error!("ZeroPod only supports structs with named fields"); },
                    );
                }
            },
            _ => {
                return Err(
                    quote! { compile_error!("ZeroPod Schema::parse only accepts structs"); },
                );
            }
        };

        // Check for #[zeropod(compact)] attribute.
        let is_compact = input.attrs.iter().any(|attr| {
            if !attr.path().is_ident("zeropod") {
                return false;
            }
            let mut found = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("compact") {
                    found = true;
                }
                Ok(())
            });
            found
        });

        // Classify fields.
        let fields: Vec<SchemaField> = named_fields
            .iter()
            .map(|f| {
                let name = f.ident.clone().expect("named field must have ident");
                let ty = f.ty.clone();
                let kind = classify_field(&ty);
                SchemaField {
                    name,
                    ty,
                    kind,
                    vis: f.vis.clone(),
                }
            })
            .collect();

        // Enforce suffix-only rule for compact mode:
        // once a tail field appears, no inline fields may follow.
        if is_compact {
            let mut seen_tail = false;
            let mut first_tail_name: Option<&syn::Ident> = None;
            for f in &fields {
                match &f.kind {
                    FieldKind::Tail(_) => {
                        if !seen_tail {
                            seen_tail = true;
                            first_tail_name = Some(&f.name);
                        }
                    }
                    FieldKind::Inline => {
                        if seen_tail {
                            let inline_name = &f.name;
                            let tail_name = first_tail_name.unwrap();
                            let msg = format!(
                                "inline field `{inline_name}` cannot come after tail field \
                                 `{tail_name}` in compact mode"
                            );
                            return Err(quote! { compile_error!(#msg); });
                        }
                    }
                }
            }
        }

        Ok(Schema {
            name: input.ident.clone(),
            generics: input.generics.clone(),
            fields,
            is_compact,
        })
    }

    pub fn inline_fields(&self) -> impl Iterator<Item = &SchemaField> {
        self.fields
            .iter()
            .filter(|f| matches!(f.kind, FieldKind::Inline))
    }

    pub fn tail_fields(&self) -> impl Iterator<Item = &SchemaField> {
        self.fields
            .iter()
            .filter(|f| matches!(f.kind, FieldKind::Tail(_)))
    }
}
