//! `parse_file` — the full per-file parse. Walks every `#[account]`-attributed
//! struct, parses its attribute body in recoverable mode via `quasar-syntax`,
//! and accumulates diagnostics.

use crate::db::Db;
use crate::diagnostic::HirDiagnostic;
use crate::input::File;
use crate::items::{ByteRange, ItemHead, ItemKind};
use quasar_syntax::account::{
    parse_discriminator_bytes, parse_recoverable, validate_recoverable, AccountAttrAst,
};
use quasar_syntax::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity};
use quasar_syntax::LineIndex;
use syn::parse::{Parser as _, ParseStream};
use syn::{Attribute, Item};

#[salsa::tracked(debug)]
pub struct ParsedFile<'db> {
    #[returns(ref)]
    pub items: Vec<ItemHead>,
    #[returns(ref)]
    pub diagnostics: Vec<HirDiagnostic>,
}

#[salsa::tracked]
pub fn parse_file<'db>(db: &'db dyn Db, file: File) -> ParsedFile<'db> {
    let text = file.text(db);
    let (items, diagnostics) = analyze(text.as_ref());
    ParsedFile::new(db, items, diagnostics)
}

/// Builds a [`LineIndex`] for a file's current source text. Not memoized —
/// constructing the index is a single pass over the text and the result is
/// only used by the LSP-emit layer to translate byte offsets into
/// line/column positions.
pub fn line_index_for(db: &dyn Db, file: File) -> LineIndex {
    LineIndex::new(file.text(db).as_ref())
}

fn analyze(text: &str) -> (Vec<ItemHead>, Vec<HirDiagnostic>) {
    let mut items = Vec::new();
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
            return (items, lower_diagnostics(sink));
        }
    };

    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };

        if let Some(attr) = find_account_attr(&item_struct.attrs) {
            let discriminator = parse_account_attr_into(attr, &mut sink);
            items.push(ItemHead {
                name: item_struct.ident.to_string(),
                kind: ItemKind::AccountType,
                range: ByteRange::from_span(item_struct.ident.span()),
                discriminator,
            });
            continue;
        }

        if item_struct.attrs.iter().any(crate::scope::derives_accounts) {
            items.push(ItemHead {
                name: item_struct.ident.to_string(),
                kind: ItemKind::AccountsStruct,
                range: ByteRange::from_span(item_struct.ident.span()),
                discriminator: None,
            });
        }
    }

    (items, lower_diagnostics(sink))
}

fn find_account_attr(attrs: &[Attribute]) -> Option<&Attribute> {
    attrs.iter().find(|a| a.path().is_ident("account"))
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
    sink.into_items().into_iter().map(HirDiagnostic::lower).collect()
}
