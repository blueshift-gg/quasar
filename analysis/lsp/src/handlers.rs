//! Request handlers. Each takes a [`Snapshot`] and the request params, returns
//! the response value. The handlers are pure functions over the snapshot —
//! they don't touch live server state.

use crate::capabilities::{TOK_KEYWORD, TOK_NAMESPACE, TOK_PROPERTY};
use crate::diagnostics::{byte_range_to_lsp_range, lsp_position_to_byte_offset, position_for};
use crate::snapshot::Snapshot;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    CodeLens, CodeLensParams, CompletionItem, CompletionItemKind, CompletionParams,
    CompletionResponse, DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams,
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, InlayHint, InlayHintKind,
    InlayHintLabel, InlayHintParams, Location, MarkupContent, MarkupKind, Position,
    ReferenceParams, SemanticToken, SemanticTokens, SemanticTokensParams, SemanticTokensResult,
    SymbolInformation, SymbolKind, TextEdit, WorkspaceEdit, WorkspaceSymbolParams,
    WorkspaceSymbolResponse,
};
use quasar_hir::{
    line_index_for, parse_file, resolve_account_refs, workspace_symbol_index, AccountRef,
    AccountRefResolution, Database, File, ItemHead, ItemKind, Workspace,
};
use quasar_syntax::LineIndex;
use std::collections::HashMap;
use syn::{Attribute, GenericArgument, Item, PathArguments, Type};

pub fn handle_hover(snapshot: &Snapshot, params: HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let file = snapshot.file_for(uri)?;
    let (account_ref, resolution) = ref_at(snapshot, file, position)?;

    let text = file.text(&snapshot.db).clone();
    let line_index = line_index_for(&snapshot.db, file);
    let range = byte_range_to_lsp_range(
        &line_index,
        &text,
        account_ref.range.start,
        account_ref.range.end,
    );

    let value = match resolution {
        AccountRefResolution::Resolved { defining_file } => {
            let path = defining_file.path(&snapshot.db).clone();
            format!(
                "**Account type `{}`**\n\nDefined in `{}`.",
                account_ref.name, path
            )
        }
        AccountRefResolution::Unknown => format!(
            "**Account type `{}`**\n\nNot found in the current workspace.",
            account_ref.name
        ),
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(range),
    })
}

pub fn handle_definition(
    snapshot: &Snapshot,
    params: GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let file = snapshot.file_for(uri)?;
    let (account_ref, resolution) = ref_at(snapshot, file, position)?;

    let AccountRefResolution::Resolved { defining_file } = resolution else {
        return None;
    };

    let parsed = parse_file(&snapshot.db, defining_file);
    let items = parsed.items(&snapshot.db);
    let item = items
        .iter()
        .find(|i| i.name == account_ref.name && i.kind == ItemKind::AccountType)?;

    let def_text = defining_file.text(&snapshot.db).clone();
    let def_line_index = line_index_for(&snapshot.db, defining_file);
    let def_range =
        byte_range_to_lsp_range(&def_line_index, &def_text, item.range.start, item.range.end);
    let def_uri = snapshot.uri_for(defining_file)?.clone();

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: def_uri,
        range: def_range,
    }))
}

pub fn handle_completion(
    snapshot: &Snapshot,
    params: CompletionParams,
) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let file = snapshot.file_for(uri)?;
    let text = file.text(&snapshot.db).clone();
    let byte_offset = lsp_position_to_byte_offset(&text, position);

    if !cursor_in_account_type_arg(&text, byte_offset) {
        return None;
    }

    let index = workspace_symbol_index(&snapshot.db, snapshot.workspace);
    let mut items: Vec<CompletionItem> = index
        .names()
        .filter_map(|name| {
            let entry = index.lookup(name)?;
            if entry.kind != ItemKind::AccountType {
                return None;
            }
            Some(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::STRUCT),
                detail: Some("Quasar account type".to_string()),
                ..Default::default()
            })
        })
        .collect();
    items.sort_by(|a, b| a.label.cmp(&b.label));

    Some(CompletionResponse::Array(items))
}

pub fn handle_references(snapshot: &Snapshot, params: ReferenceParams) -> Option<Vec<Location>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let file = snapshot.file_for(uri)?;
    let text = file.text(&snapshot.db).clone();
    let byte_offset = lsp_position_to_byte_offset(&text, position);

    let name = find_account_type_name(&snapshot.db, snapshot.workspace, file, byte_offset)?;

    let mut locations = Vec::new();
    for &workspace_file in snapshot.workspace.files(&snapshot.db) {
        let Some(file_uri) = snapshot.uri_for(workspace_file).cloned() else {
            continue;
        };
        let resolved = resolve_account_refs(&snapshot.db, snapshot.workspace, workspace_file);
        let file_text = workspace_file.text(&snapshot.db).clone();
        let line_index = line_index_for(&snapshot.db, workspace_file);
        for (account_ref, _) in resolved.refs(&snapshot.db).iter() {
            if account_ref.name == name {
                let range = byte_range_to_lsp_range(
                    &line_index,
                    &file_text,
                    account_ref.range.start,
                    account_ref.range.end,
                );
                locations.push(Location {
                    uri: file_uri.clone(),
                    range,
                });
            }
        }
    }

    if params.context.include_declaration {
        let index = workspace_symbol_index(&snapshot.db, snapshot.workspace);
        if let Some(entry) = index.lookup(&name) {
            if entry.kind == ItemKind::AccountType {
                let parsed = parse_file(&snapshot.db, entry.file);
                if let Some(item) = parsed
                    .items(&snapshot.db)
                    .iter()
                    .find(|i| i.name == name && i.kind == ItemKind::AccountType)
                {
                    let def_text = entry.file.text(&snapshot.db).clone();
                    let def_line_index = line_index_for(&snapshot.db, entry.file);
                    let range = byte_range_to_lsp_range(
                        &def_line_index,
                        &def_text,
                        item.range.start,
                        item.range.end,
                    );
                    if let Some(def_uri) = snapshot.uri_for(entry.file).cloned() {
                        locations.push(Location {
                            uri: def_uri,
                            range,
                        });
                    }
                }
            }
        }
    }

    Some(locations)
}

pub fn handle_document_symbol(
    snapshot: &Snapshot,
    params: DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let uri = &params.text_document.uri;
    let file = snapshot.file_for(uri)?;
    let text = file.text(&snapshot.db).clone();
    let line_index = line_index_for(&snapshot.db, file);
    let parsed = parse_file(&snapshot.db, file);

    let symbols: Vec<DocumentSymbol> = parsed
        .items(&snapshot.db)
        .iter()
        .map(|item| {
            let range = byte_range_to_lsp_range(
                &line_index,
                &text,
                item.range.start,
                item.range.end,
            );
            let detail = Some(match item.kind {
                ItemKind::AccountType => "#[account]".to_string(),
                ItemKind::AccountsStruct => "#[derive(Accounts)]".to_string(),
            });
            #[allow(deprecated)]
            DocumentSymbol {
                name: item.name.clone(),
                detail,
                kind: SymbolKind::STRUCT,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            }
        })
        .collect();

    Some(DocumentSymbolResponse::Nested(symbols))
}

pub fn handle_semantic_tokens_full(
    snapshot: &Snapshot,
    params: SemanticTokensParams,
) -> Option<SemanticTokensResult> {
    let uri = &params.text_document.uri;
    let file = snapshot.file_for(uri)?;
    let text = file.text(&snapshot.db).clone();
    let line_index = line_index_for(&snapshot.db, file);

    let mut raw = collect_semantic_tokens(&text);
    raw.sort_by_key(|t| (t.line, t.start_col_utf16));

    let mut data = Vec::with_capacity(raw.len());
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;
    for tok in raw {
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line == 0 {
            tok.start_col_utf16 - prev_col
        } else {
            tok.start_col_utf16
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.length_utf16,
            token_type: tok.token_type,
            token_modifiers_bitset: 0,
        });
        prev_line = tok.line;
        prev_col = tok.start_col_utf16;
    }
    let _ = (&line_index, &text);

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    }))
}

pub fn handle_inlay_hint(
    snapshot: &Snapshot,
    params: InlayHintParams,
) -> Option<Vec<InlayHint>> {
    let uri = &params.text_document.uri;
    let file = snapshot.file_for(uri)?;
    let text = file.text(&snapshot.db).clone();
    let line_index = line_index_for(&snapshot.db, file);

    let range_start = lsp_position_to_byte_offset(&text, params.range.start);
    let range_end = lsp_position_to_byte_offset(&text, params.range.end);

    let index = workspace_symbol_index(&snapshot.db, snapshot.workspace);
    let resolved = resolve_account_refs(&snapshot.db, snapshot.workspace, file);

    // Cache parsed-file lookups across multiple refs of the same type.
    let mut disc_cache: HashMap<String, Option<Vec<u8>>> = HashMap::new();

    let mut hints = Vec::new();
    for (account_ref, _) in resolved.refs(&snapshot.db).iter() {
        if account_ref.range.end < range_start || account_ref.range.start > range_end {
            continue;
        }
        let bytes = disc_cache
            .entry(account_ref.name.clone())
            .or_insert_with(|| lookup_discriminator(&snapshot.db, &index, &account_ref.name))
            .clone()?;

        let position = position_for(&line_index, &text, account_ref.range.end);
        hints.push(InlayHint {
            position,
            label: InlayHintLabel::String(format!(" {}", format_discriminator(&bytes))),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        });
    }

    Some(hints)
}

pub fn handle_code_action(
    snapshot: &Snapshot,
    params: CodeActionParams,
) -> Option<CodeActionResponse> {
    let uri = params.text_document.uri.clone();
    let file = snapshot.file_for(&uri)?;
    let text = file.text(&snapshot.db).clone();
    let byte_offset = lsp_position_to_byte_offset(&text, params.range.start);

    let tree = syn::parse_file(&text).ok()?;
    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        let item_span = item_struct.ident.span();
        let item_range = item_span.byte_range();
        if (item_range.start as u32) > byte_offset || (item_range.end as u32) < byte_offset {
            // Allow cursor anywhere on the struct definition line; widen by
            // checking enclosing struct token span as well.
            let full_span = item_struct
                .struct_token
                .span
                .byte_range();
            if (full_span.start as u32) > byte_offset || (item_range.end as u32) < byte_offset {
                continue;
            }
        }

        if item_struct
            .attrs
            .iter()
            .any(|a| a.path().is_ident("account"))
        {
            continue;
        }
        if item_struct.attrs.iter().any(derives_accounts) {
            continue;
        }

        // Insert at the byte offset where the struct keyword starts, including
        // any leading `pub` visibility.
        let insertion_byte = leading_attr_anchor(&item_struct);
        let insertion_position = position_for(
            &line_index_for(&snapshot.db, file),
            &text,
            insertion_byte,
        );

        let edit = TextEdit {
            range: lsp_types::Range {
                start: insertion_position,
                end: insertion_position,
            },
            new_text: "#[account(discriminator = 1)]\n".to_string(),
        };
        let workspace_edit = WorkspaceEdit {
            changes: Some([(uri.clone(), vec![edit])].into_iter().collect()),
            document_changes: None,
            change_annotations: None,
        };
        let action = CodeAction {
            title: "Insert #[account(discriminator = 1)]".to_string(),
            kind: Some(CodeActionKind::REFACTOR_REWRITE),
            diagnostics: None,
            edit: Some(workspace_edit),
            command: None,
            is_preferred: None,
            disabled: None,
            data: None,
        };
        return Some(vec![CodeActionOrCommand::CodeAction(action)]);
    }

    Some(vec![])
}

pub fn handle_code_lens(snapshot: &Snapshot, params: CodeLensParams) -> Option<Vec<CodeLens>> {
    let uri = &params.text_document.uri;
    let file = snapshot.file_for(uri)?;
    let text = file.text(&snapshot.db).clone();
    let line_index = line_index_for(&snapshot.db, file);
    let parsed = parse_file(&snapshot.db, file);

    let mut lenses = Vec::new();
    for item in parsed.items(&snapshot.db).iter() {
        if item.kind != ItemKind::AccountType {
            continue;
        }
        let count = count_references(&snapshot.db, snapshot.workspace, &item.name);
        let range = byte_range_to_lsp_range(&line_index, &text, item.range.start, item.range.end);
        let label = if count == 1 {
            "1 reference".to_string()
        } else {
            format!("{} references", count)
        };
        lenses.push(CodeLens {
            range,
            command: Some(lsp_types::Command {
                title: label,
                command: "quasar.showReferences".to_string(),
                arguments: None,
            }),
            data: None,
        });
    }
    Some(lenses)
}

pub fn handle_workspace_symbol(
    snapshot: &Snapshot,
    params: WorkspaceSymbolParams,
) -> Option<WorkspaceSymbolResponse> {
    let query = params.query.to_lowercase();
    let mut out: Vec<SymbolInformation> = Vec::new();

    for &workspace_file in snapshot.workspace.files(&snapshot.db) {
        let Some(file_uri) = snapshot.uri_for(workspace_file).cloned() else {
            continue;
        };
        let parsed = parse_file(&snapshot.db, workspace_file);
        let file_text = workspace_file.text(&snapshot.db).clone();
        let line_index = line_index_for(&snapshot.db, workspace_file);
        for item in parsed.items(&snapshot.db).iter() {
            if !item.name.to_lowercase().contains(&query) {
                continue;
            }
            let range = byte_range_to_lsp_range(
                &line_index,
                &file_text,
                item.range.start,
                item.range.end,
            );
            #[allow(deprecated)]
            out.push(SymbolInformation {
                name: item.name.clone(),
                kind: SymbolKind::STRUCT,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: file_uri.clone(),
                    range,
                },
                container_name: Some(match item.kind {
                    ItemKind::AccountType => "#[account]".to_string(),
                    ItemKind::AccountsStruct => "#[derive(Accounts)]".to_string(),
                }),
            });
        }
    }

    Some(WorkspaceSymbolResponse::Flat(out))
}

pub fn handle_document_highlight(
    snapshot: &Snapshot,
    params: DocumentHighlightParams,
) -> Option<Vec<DocumentHighlight>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let file = snapshot.file_for(uri)?;
    let text = file.text(&snapshot.db).clone();
    let byte_offset = lsp_position_to_byte_offset(&text, position);

    let name = find_account_type_name(&snapshot.db, snapshot.workspace, file, byte_offset)?;

    let line_index = line_index_for(&snapshot.db, file);
    let resolved = resolve_account_refs(&snapshot.db, snapshot.workspace, file);

    let mut highlights = Vec::new();
    for (account_ref, _) in resolved.refs(&snapshot.db).iter() {
        if account_ref.name != name {
            continue;
        }
        let range = byte_range_to_lsp_range(
            &line_index,
            &text,
            account_ref.range.start,
            account_ref.range.end,
        );
        highlights.push(DocumentHighlight {
            range,
            kind: Some(DocumentHighlightKind::READ),
        });
    }

    // Also highlight the definition if it lives in the same file.
    let parsed = parse_file(&snapshot.db, file);
    for item in parsed.items(&snapshot.db).iter() {
        if item.kind == ItemKind::AccountType && item.name == name {
            let range = byte_range_to_lsp_range(
                &line_index,
                &text,
                item.range.start,
                item.range.end,
            );
            highlights.push(DocumentHighlight {
                range,
                kind: Some(DocumentHighlightKind::WRITE),
            });
        }
    }

    Some(highlights)
}

// ---- shared helpers -------------------------------------------------------

fn ref_at(
    snapshot: &Snapshot,
    file: File,
    position: Position,
) -> Option<(AccountRef, AccountRefResolution)> {
    let text = file.text(&snapshot.db).clone();
    let byte_offset = lsp_position_to_byte_offset(&text, position);

    let resolved = resolve_account_refs(&snapshot.db, snapshot.workspace, file);
    let refs = resolved.refs(&snapshot.db);
    refs.iter()
        .find(|(r, _)| r.range.start <= byte_offset && byte_offset < r.range.end)
        .cloned()
}

fn find_account_type_name(
    db: &Database,
    workspace: Workspace,
    file: File,
    byte_offset: u32,
) -> Option<String> {
    let resolved = resolve_account_refs(db, workspace, file);
    for (account_ref, _) in resolved.refs(db).iter() {
        if account_ref.range.start <= byte_offset && byte_offset < account_ref.range.end {
            return Some(account_ref.name.clone());
        }
    }

    let parsed = parse_file(db, file);
    for item in parsed.items(db).iter() {
        if item.kind == ItemKind::AccountType
            && item.range.start <= byte_offset
            && byte_offset < item.range.end
        {
            return Some(item.name.clone());
        }
    }
    None
}

fn cursor_in_account_type_arg(text: &str, offset: u32) -> bool {
    let Ok(tree) = syn::parse_file(text) else {
        return false;
    };
    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        if !item_struct.attrs.iter().any(derives_accounts) {
            continue;
        }
        for field in &item_struct.fields {
            if field_type_contains_account_arg_at(&field.ty, offset) {
                return true;
            }
        }
    }
    false
}

fn field_type_contains_account_arg_at(ty: &Type, offset: u32) -> bool {
    let inner = unwrap_to_inner_type(ty);
    let Type::Path(tp) = inner else { return false };
    let Some(last) = tp.path.segments.last() else {
        return false;
    };
    if last.ident != "Account" && last.ident != "InterfaceAccount" {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return false;
    };
    let lt = args.lt_token.span.byte_range().start as u32;
    let gt = args.gt_token.span.byte_range().end as u32;
    lt < offset && offset <= gt
}

fn unwrap_to_inner_type(ty: &Type) -> &Type {
    let mut current = ty;
    loop {
        match current {
            Type::Reference(r) => current = &r.elem,
            Type::Path(tp) => {
                if let Some(last) = tp.path.segments.last() {
                    if last.ident == "Option" {
                        if let PathArguments::AngleBracketed(args) = &last.arguments {
                            if let Some(GenericArgument::Type(inner)) = args.args.first() {
                                current = inner;
                                continue;
                            }
                        }
                    }
                }
                return current;
            }
            other => return other,
        }
    }
}

fn derives_accounts(attr: &Attribute) -> bool {
    if !attr.path().is_ident("derive") {
        return false;
    }
    let mut found = false;
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("Accounts") {
            found = true;
        }
        Ok(())
    });
    found
}

fn count_references(db: &Database, workspace: Workspace, name: &str) -> usize {
    let mut count = 0;
    for &file in workspace.files(db) {
        let resolved = resolve_account_refs(db, workspace, file);
        for (account_ref, _) in resolved.refs(db).iter() {
            if account_ref.name == name {
                count += 1;
            }
        }
    }
    count
}

fn lookup_discriminator(
    db: &Database,
    index: &quasar_hir::SymbolIndex,
    name: &str,
) -> Option<Vec<u8>> {
    let entry = index.lookup(name)?;
    if entry.kind != ItemKind::AccountType {
        return None;
    }
    let parsed = parse_file(db, entry.file);
    parsed
        .items(db)
        .iter()
        .find(|i| i.name == name && i.kind == ItemKind::AccountType)
        .and_then(|i| i.discriminator.clone())
}

fn format_discriminator(bytes: &[u8]) -> String {
    let parts: Vec<String> = bytes.iter().map(|b| b.to_string()).collect();
    format!("[{}]", parts.join(", "))
}

fn leading_attr_anchor(item_struct: &syn::ItemStruct) -> u32 {
    if let Some(first_attr) = item_struct.attrs.first() {
        return first_attr.pound_token.span.byte_range().start as u32;
    }
    if let Some(vis_span) = visibility_span(&item_struct.vis) {
        return vis_span as u32;
    }
    item_struct.struct_token.span.byte_range().start as u32
}

fn visibility_span(vis: &syn::Visibility) -> Option<usize> {
    match vis {
        syn::Visibility::Public(p) => Some(p.span.byte_range().start),
        syn::Visibility::Restricted(r) => Some(r.pub_token.span.byte_range().start),
        syn::Visibility::Inherited => None,
    }
}

// ---- semantic token collection -------------------------------------------

struct ProtoToken {
    line: u32,
    start_col_utf16: u32,
    length_utf16: u32,
    token_type: u32,
}

fn collect_semantic_tokens(text: &str) -> Vec<ProtoToken> {
    let line_index = LineIndex::new(text);
    let mut out = Vec::new();

    let Ok(tree) = syn::parse_file(text) else {
        return out;
    };

    for item in &tree.items {
        let attrs = match item {
            Item::Struct(s) => &s.attrs,
            Item::Mod(m) => &m.attrs,
            Item::Fn(f) => &f.attrs,
            Item::Enum(e) => &e.attrs,
            _ => continue,
        };
        for attr in attrs {
            if attr.path().is_ident("account") {
                emit_account_attr_tokens(attr, text, &line_index, &mut out);
            }
        }
        if let Item::Struct(item_struct) = item {
            for field in &item_struct.fields {
                for attr in &field.attrs {
                    if attr.path().is_ident("account") {
                        emit_account_attr_tokens(attr, text, &line_index, &mut out);
                    }
                }
            }
        }
    }

    out
}

fn emit_account_attr_tokens(
    attr: &Attribute,
    text: &str,
    line_index: &LineIndex,
    out: &mut Vec<ProtoToken>,
) {
    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        _ => return,
    };
    for tt in tokens {
        match tt {
            proc_macro2::TokenTree::Ident(ident) => {
                let span = ident.span();
                let r = span.byte_range();
                let token_type = match ident.to_string().as_str() {
                    // Quasar attribute keywords
                    "discriminator" | "unsafe_no_disc" | "set_inner" | "fixed_capacity"
                    | "one_of" | "implements" | "init" | "idempotent" | "dup" | "group"
                    | "payer" | "address" | "realloc" | "close" | "has_one" | "constraints"
                    | "dest" => TOK_KEYWORD,
                    _ => continue,
                };
                let (line, col) = position_of_byte_in_lsp(text, line_index, r.start as u32);
                let length = ident.to_string().encode_utf16().count() as u32;
                out.push(ProtoToken {
                    line,
                    start_col_utf16: col,
                    length_utf16: length,
                    token_type,
                });
            }
            proc_macro2::TokenTree::Group(group) => {
                // Recurse into nested groups (parens) to find more keywords.
                for inner in group.stream() {
                    if let proc_macro2::TokenTree::Ident(ident) = inner {
                        let name = ident.to_string();
                        let token_type = match name.as_str() {
                            "idempotent" | "dest" => TOK_KEYWORD,
                            // bare idents inside `has_one(a, b)` are field
                            // references; tag as property
                            _ => TOK_PROPERTY,
                        };
                        let r = ident.span().byte_range();
                        let (line, col) =
                            position_of_byte_in_lsp(text, line_index, r.start as u32);
                        let length = name.encode_utf16().count() as u32;
                        out.push(ProtoToken {
                            line,
                            start_col_utf16: col,
                            length_utf16: length,
                            token_type,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    // Avoid emitting namespace tokens on path segments for v1.
    let _ = TOK_NAMESPACE;
}

fn position_of_byte_in_lsp(text: &str, line_index: &LineIndex, byte_offset: u32) -> (u32, u32) {
    let pos = position_for(line_index, text, byte_offset);
    (pos.line, pos.character)
}

// Reference to keep `ItemHead` reachable without import warnings if all uses
// move behind cfg later.
#[allow(dead_code)]
fn _dummy_item_head(item: &ItemHead) -> &str {
    &item.name
}
