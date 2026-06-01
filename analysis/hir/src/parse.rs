//! `parse_file` — the full per-file parse. Walks every `#[account]`-attributed
//! struct, parses its attribute body in recoverable mode via `quasar-syntax`,
//! and accumulates diagnostics.

use {
    crate::{
        db::Db,
        diagnostic::HirDiagnostic,
        input::File,
        items::{ByteRange, FieldDecl, ItemHead, ItemKind},
    },
    quasar_syntax::{
        account::{
            parse_discriminator_bytes, parse_recoverable, validate_recoverable, AccountAttrAst,
        },
        diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity},
        LineIndex,
    },
    syn::{
        parse::{ParseStream, Parser as _},
        Attribute, Item,
    },
};

#[salsa::tracked(debug)]
pub struct ParsedFile<'db> {
    #[returns(ref)]
    pub items: Vec<ItemHead>,
    #[returns(ref)]
    pub diagnostics: Vec<HirDiagnostic>,
    /// Every plain (non-`#[account]`, non-`Accounts`) named struct in the file,
    /// as `(name, fields)`. Feeds the workspace data-struct index so a
    /// `define_account!` type's `: Data` clause can resolve its fields even
    /// when that struct lives in a different module.
    #[returns(ref)]
    pub data_structs: Vec<(String, Vec<FieldDecl>)>,
}

#[salsa::tracked]
pub fn parse_file<'db>(db: &'db dyn Db, file: File) -> ParsedFile<'db> {
    let text = file.text(db);
    let (items, diagnostics, data_structs) = analyze(text.as_ref());
    ParsedFile::new(db, items, diagnostics, data_structs)
}

/// Builds a [`LineIndex`] for a file's current source text. Not memoized —
/// constructing the index is a single pass over the text and the result is
/// only used by the LSP-emit layer to translate byte offsets into
/// line/column positions.
pub fn line_index_for(db: &dyn Db, file: File) -> LineIndex {
    LineIndex::new(file.text(db).as_ref())
}

type Analysis = (
    Vec<ItemHead>,
    Vec<HirDiagnostic>,
    Vec<(String, Vec<FieldDecl>)>,
);

fn analyze(text: &str) -> Analysis {
    let mut items = Vec::new();
    let mut data_structs: Vec<(String, Vec<FieldDecl>)> = Vec::new();
    let mut sink = Diagnostics::new();

    let tree = match syn::parse_file(text) {
        Ok(t) => t,
        Err(err) => {
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::AccountAttrMalformedDirective,
                message: err.to_string(),
                primary: err.span(),
                labels: vec![],
                fixes: vec![],
            });
            return (items, lower_diagnostics(sink), data_structs);
        }
    };

    for item in &tree.items {
        match item {
            Item::Struct(item_struct) => {
                let (fields, body_insert) = struct_fields(item_struct);
                let is_accounts = item_struct.attrs.iter().any(crate::scope::derives_accounts);

                if let Some(attr) = find_account_attr(&item_struct.attrs) {
                    let discriminator = parse_account_attr_into(attr, &mut sink);
                    items.push(ItemHead {
                        name: item_struct.ident.to_string(),
                        kind: ItemKind::AccountType,
                        range: ByteRange::from_span(item_struct.ident.span()),
                        discriminator,
                        fields,
                        fields_known: true,
                        data_type: None,
                        body_insert,
                    });
                } else if is_accounts {
                    items.push(ItemHead {
                        name: item_struct.ident.to_string(),
                        kind: ItemKind::AccountsStruct,
                        range: ByteRange::from_span(item_struct.ident.span()),
                        discriminator: None,
                        fields,
                        fields_known: true,
                        data_type: None,
                        body_insert,
                    });
                } else if !fields.is_empty() {
                    // A plain data struct: a candidate target for a
                    // `define_account!` type's `: Data` clause.
                    data_structs.push((item_struct.ident.to_string(), fields));
                }
            }
            // `define_account!(pub struct Mint => […]: MintData)` — an account
            // type declared via macro. Its fields live on the `MintData` data
            // struct, resolved workspace-wide (it may live in another module),
            // so the field list here is intentionally empty and not-known.
            Item::Macro(m) if crate::scope::is_define_account(m) => {
                for da in parse_define_account(m.mac.tokens.clone()) {
                    items.push(ItemHead {
                        name: da.name.to_string(),
                        kind: ItemKind::AccountType,
                        range: ByteRange::from_span(da.name.span()),
                        discriminator: None,
                        fields: Vec::new(),
                        fields_known: false,
                        data_type: da.data_type,
                        body_insert: None,
                    });
                }
            }
            _ => {}
        }
    }

    (items, lower_diagnostics(sink), data_structs)
}

/// One account type declared by a `define_account!` invocation: its name (with
/// span) and the optional associated data-struct name from the `: Data` clause.
struct DefineAccount {
    name: proc_macro2::Ident,
    data_type: Option<String>,
}

/// Parses the token body of a `define_account!` call into its declared account
/// type(s). The data type, when present, is the path after the bracketed check
/// list (`pub struct Mint => [checks]: MintData`); we take its last segment.
fn parse_define_account(tokens: proc_macro2::TokenStream) -> Vec<DefineAccount> {
    use proc_macro2::{Delimiter, TokenTree};

    let names = crate::scope::struct_idents_in_tokens(tokens.clone());
    let tts: Vec<TokenTree> = tokens.into_iter().collect();
    // The check list is the last top-level bracket group; the data type, if
    // any, is the last identifier following it.
    let data_type = tts
        .iter()
        .rposition(|tt| matches!(tt, TokenTree::Group(g) if g.delimiter() == Delimiter::Bracket))
        .and_then(|i| {
            tts[i + 1..].iter().rev().find_map(|tt| match tt {
                TokenTree::Ident(id) => Some(id.to_string()),
                _ => None,
            })
        });

    names
        .into_iter()
        .map(|name| DefineAccount {
            name,
            data_type: data_type.clone(),
        })
        .collect()
}

fn find_account_attr(attrs: &[Attribute]) -> Option<&Attribute> {
    attrs.iter().find(|a| a.path().is_ident("account"))
}

/// Extracts named field declarations and the byte offset just inside the
/// opening brace (for "add field" code actions).
fn struct_fields(item: &syn::ItemStruct) -> (Vec<crate::items::FieldDecl>, Option<u32>) {
    let syn::Fields::Named(named) = &item.fields else {
        return (Vec::new(), None);
    };
    let fields = named
        .named
        .iter()
        .filter_map(|f| {
            let ident = f.ident.as_ref()?;
            Some(crate::items::FieldDecl {
                name: ident.to_string(),
                range: ByteRange::from_span(ident.span()),
            })
        })
        .collect();
    let body_insert = Some(named.brace_token.span.open().byte_range().end as u32);
    (fields, body_insert)
}

fn parse_account_attr_into(attr: &Attribute, sink: &mut Diagnostics) -> Option<Vec<u8>> {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        _ => return None,
    };

    let mut ast_holder: Option<AccountAttrAst> = None;
    let parser = |input: ParseStream| -> syn::Result<()> {
        let fallback = input.span();
        let ast = parse_recoverable(input, sink);
        validate_recoverable(&ast, sink, fallback);
        ast_holder = Some(ast);
        Ok(())
    };

    let _ = parser.parse2(tokens);

    let clause = ast_holder.as_ref()?.discriminator.as_ref()?;
    let mut throwaway = Diagnostics::new();
    let bytes = parse_discriminator_bytes(clause, &mut throwaway);
    if throwaway.is_empty() {
        Some(bytes)
    } else {
        None
    }
}

fn lower_diagnostics(mut sink: Diagnostics) -> Vec<HirDiagnostic> {
    sink.dedup_subsume_narrower();
    sink.into_items()
        .into_iter()
        .map(HirDiagnostic::lower)
        .collect()
}
