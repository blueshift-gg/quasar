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
    data_struct_index, line_index_for, parse_file, resolve_account_refs, resolve_has_one,
    workspace_symbol_index, AccountRef, AccountRefResolution, Database, File, ItemHead, ItemKind,
    Workspace,
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
        AccountRefResolution::ResolvedExternal => format!(
            "**Account type `{}`**\n\nProvided by a dependency.",
            account_ref.name
        ),
        AccountRefResolution::Unknown => format!(
            "**Account type `{}`**\n\nNot found in the workspace or its dependencies.",
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

    // `Account<T>` reference → the account type declaration.
    if let Some((account_ref, AccountRefResolution::Resolved { defining_file })) =
        ref_at(snapshot, file, position)
    {
        let parsed = parse_file(&snapshot.db, defining_file);
        if let Some(item) = parsed
            .items(&snapshot.db)
            .iter()
            .find(|i| i.name == account_ref.name && i.kind == ItemKind::AccountType)
        {
            let def_text = defining_file.text(&snapshot.db).clone();
            let def_line_index = line_index_for(&snapshot.db, defining_file);
            let def_range = byte_range_to_lsp_range(
                &def_line_index,
                &def_text,
                item.range.start,
                item.range.end,
            );
            if let Some(def_uri) = snapshot.uri_for(defining_file).cloned() {
                return Some(GotoDefinitionResponse::Scalar(Location {
                    uri: def_uri,
                    range: def_range,
                }));
            }
        }
    }

    // `has_one(target)` → the account type's field declaration.
    if let Some(location) = has_one_target_definition(snapshot, file, position) {
        return Some(GotoDefinitionResponse::Scalar(location));
    }

    None
}

fn has_one_target_definition(
    snapshot: &Snapshot,
    file: File,
    position: Position,
) -> Option<Location> {
    let text = file.text(&snapshot.db).clone();
    let byte_offset = lsp_position_to_byte_offset(&text, position);

    let resolved = resolve_has_one(&snapshot.db, snapshot.workspace, file);
    let r = resolved
        .refs(&snapshot.db)
        .iter()
        .find(|r| r.range.start <= byte_offset && byte_offset < r.range.end)
        .cloned()?;

    let account_type = r.account_type.as_ref()?;
    let index = workspace_symbol_index(&snapshot.db, snapshot.workspace);
    let entry = index.lookup(account_type)?;
    if entry.kind != ItemKind::AccountType {
        return None;
    }
    let parsed = parse_file(&snapshot.db, entry.file);
    let item = parsed
        .items(&snapshot.db)
        .iter()
        .find(|i| i.name == *account_type && i.kind == ItemKind::AccountType)?
        .clone();

    // The field declaration lives either inline on a directly-parsed struct (in
    // `entry.file`) or, for a `define_account!` type, on its data struct —
    // which may be in a different file. Resolve both the defining file and the
    // field's range accordingly.
    let (def_file, field_range) = if item.fields_known {
        let field = item.fields.iter().find(|f| f.name == r.target)?;
        (entry.file, field.range)
    } else {
        let data_type = item.data_type.as_ref()?;
        let data_index = data_struct_index(&snapshot.db, snapshot.workspace);
        let ds = data_index.get(data_type)?;
        let field = ds.fields.iter().find(|f| f.name == r.target)?;
        (ds.file, field.range)
    };

    let def_text = def_file.text(&snapshot.db).clone();
    let def_line_index = line_index_for(&snapshot.db, def_file);
    let range =
        byte_range_to_lsp_range(&def_line_index, &def_text, field_range.start, field_range.end);
    let def_uri = snapshot.uri_for(def_file)?.clone();
    Some(Location {
        uri: def_uri,
        range,
    })
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
        // Skip just this ref when it has no discriminator (external/unknown
        // types, `unsafe_no_disc`, `define_account!`); a `?` here would abort
        // hints for the whole file.
        let Some(bytes) = disc_cache
            .entry(account_ref.name.clone())
            .or_insert_with(|| lookup_discriminator(&snapshot.db, &index, &account_ref.name))
            .clone()
        else {
            continue;
        };

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
    let line_index = line_index_for(&snapshot.db, file);

    let mut actions: Vec<CodeActionOrCommand> = Vec::new();

    // Diagnostic-driven quickfixes.
    for diag in &params.context.diagnostics {
        let Some(code) = diag_code_str(diag) else {
            continue;
        };
        match code {
            "quasar::account_attr_missing_discriminator_or_unsafe" => {
                let diag_byte = lsp_position_to_byte_offset(&text, diag.range.start);
                if let Some((insert_byte, body_empty)) =
                    account_attr_body_insertion(&text, diag_byte)
                {
                    let pos = position_for(&line_index, &text, insert_byte);
                    let disc = if body_empty {
                        "discriminator = 1".to_string()
                    } else {
                        "discriminator = 1, ".to_string()
                    };
                    let unsafe_flag = if body_empty {
                        "unsafe_no_disc".to_string()
                    } else {
                        "unsafe_no_disc, ".to_string()
                    };
                    actions.push(insertion_action(
                        "Add `discriminator = 1`",
                        &uri,
                        pos,
                        disc,
                        Some(diag.clone()),
                    ));
                    actions.push(insertion_action(
                        "Add `unsafe_no_disc`",
                        &uri,
                        pos,
                        unsafe_flag,
                        Some(diag.clone()),
                    ));
                }
            }
            "quasar::unknown_account_type" => {
                let diag_byte = lsp_position_to_byte_offset(&text, diag.range.start);
                if let Some(name) =
                    find_account_type_name(&snapshot.db, snapshot.workspace, file, diag_byte)
                {
                    let end_pos = position_for(&line_index, &text, text.len() as u32);
                    let stub = format!(
                        "\n#[account(discriminator = 1)]\npub struct {} {{\n}}\n",
                        name
                    );
                    actions.push(insertion_action(
                        &format!("Create `#[account] struct {}`", name),
                        &uri,
                        end_pos,
                        stub,
                        Some(diag.clone()),
                    ));
                }
            }
            "quasar::has_one_missing_account_field" => {
                if let Some(action) =
                    add_field_to_account_type_action(snapshot, file, &text, diag)
                {
                    actions.push(action);
                }
            }
            "quasar::accounts_constraint_violation" if diag.message.contains("requires `mut`") => {
                if let Some(action) = add_mut_action(&text, &line_index, &uri, diag) {
                    actions.push(action);
                }
            }
            _ => {}
        }
    }

    // Position-based refactor: insert #[account] on a bare struct under the
    // cursor (not driven by a diagnostic).
    if let Some(action) =
        bare_struct_insert_action(&snapshot.db, file, &text, &line_index, &uri, params.range.start)
    {
        actions.push(action);
    }

    Some(actions)
}

/// Fix for a `requires \`mut\`` constraint violation: insert `mut` into the
/// offending field's `#[account(...)]` attribute.
fn add_mut_action(
    text: &str,
    line_index: &LineIndex,
    uri: &lsp_types::Uri,
    diag: &lsp_types::Diagnostic,
) -> Option<CodeActionOrCommand> {
    use syn::spanned::Spanned;
    let diag_byte = lsp_position_to_byte_offset(text, diag.range.start);
    let tree = syn::parse_file(text).ok()?;
    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        if !item_struct.attrs.iter().any(derives_accounts) {
            continue;
        }
        for field in &item_struct.fields {
            let fr = field.span().byte_range();
            if (fr.start as u32) > diag_byte || (fr.end as u32) < diag_byte {
                continue;
            }
            for attr in &field.attrs {
                if !attr.path().is_ident("account") {
                    continue;
                }
                let syn::Meta::List(list) = &attr.meta else {
                    continue;
                };
                let (open, _) = delim_open_close(&list.delimiter);
                let pos = position_for(line_index, text, open.byte_range().end as u32);
                let new_text = if list.tokens.is_empty() {
                    "mut".to_string()
                } else {
                    "mut, ".to_string()
                };
                return Some(insertion_action(
                    "Add `mut`",
                    uri,
                    pos,
                    new_text,
                    Some(diag.clone()),
                ));
            }
        }
    }
    None
}

/// Fix for `has_one_missing_account_field`: insert the missing field into the
/// referenced account type's struct body (possibly in another file).
fn add_field_to_account_type_action(
    snapshot: &Snapshot,
    file: File,
    text: &str,
    diag: &lsp_types::Diagnostic,
) -> Option<CodeActionOrCommand> {
    let diag_byte = lsp_position_to_byte_offset(text, diag.range.start);
    let resolved = resolve_has_one(&snapshot.db, snapshot.workspace, file);
    let r = resolved
        .refs(&snapshot.db)
        .iter()
        .find(|r| r.range.start <= diag_byte && diag_byte < r.range.end)
        .cloned()?;
    let account_type = r.account_type.as_ref()?;

    let index = workspace_symbol_index(&snapshot.db, snapshot.workspace);
    let entry = index.lookup(account_type)?;
    if entry.kind != ItemKind::AccountType {
        return None;
    }
    let parsed = parse_file(&snapshot.db, entry.file);
    let item = parsed
        .items(&snapshot.db)
        .iter()
        .find(|i| i.name == *account_type && i.kind == ItemKind::AccountType)?
        .clone();
    let insert_byte = item.body_insert?;

    let def_text = entry.file.text(&snapshot.db).clone();
    let def_line_index = line_index_for(&snapshot.db, entry.file);
    let pos = position_for(&def_line_index, &def_text, insert_byte);
    let def_uri = snapshot.uri_for(entry.file)?.clone();

    Some(insertion_action(
        &format!("Add field `{}` to `{}`", r.target, account_type),
        &def_uri,
        pos,
        format!("\n    pub {}: Address,", r.target),
        Some(diag.clone()),
    ))
}

fn bare_struct_insert_action(
    db: &Database,
    file: File,
    text: &str,
    line_index: &LineIndex,
    uri: &lsp_types::Uri,
    cursor: Position,
) -> Option<CodeActionOrCommand> {
    let _ = (db, file);
    let byte_offset = lsp_position_to_byte_offset(text, cursor);
    let tree = syn::parse_file(text).ok()?;
    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        let item_range = item_struct.ident.span().byte_range();
        let struct_kw = item_struct.struct_token.span.byte_range();
        let on_struct = (struct_kw.start as u32) <= byte_offset
            && byte_offset <= (item_range.end as u32);
        if !on_struct {
            continue;
        }
        if item_struct.attrs.iter().any(|a| a.path().is_ident("account")) {
            continue;
        }
        if item_struct.attrs.iter().any(derives_accounts) {
            continue;
        }

        let insertion_byte = leading_attr_anchor(item_struct);
        let pos = position_for(line_index, text, insertion_byte);
        return Some(insertion_action(
            "Insert #[account(discriminator = 1)]",
            uri,
            pos,
            "#[account(discriminator = 1)]\n".to_string(),
            None,
        ));
    }
    None
}

fn insertion_action(
    title: &str,
    uri: &lsp_types::Uri,
    at: Position,
    new_text: String,
    diagnostic: Option<lsp_types::Diagnostic>,
) -> CodeActionOrCommand {
    let edit = TextEdit {
        range: lsp_types::Range { start: at, end: at },
        new_text,
    };
    let workspace_edit = WorkspaceEdit {
        changes: Some([(uri.clone(), vec![edit])].into_iter().collect()),
        document_changes: None,
        change_annotations: None,
    };
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: diagnostic.map(|d| vec![d]),
        edit: Some(workspace_edit),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
    })
}

fn diag_code_str(diag: &lsp_types::Diagnostic) -> Option<&str> {
    match diag.code.as_ref()? {
        lsp_types::NumberOrString::String(s) => Some(s.as_str()),
        _ => None,
    }
}

/// Finds the `#[account(...)]` attribute whose parenthesised body contains
/// `near_byte`, and returns the byte offset just after the `(` plus whether
/// the body is currently empty.
fn account_attr_body_insertion(text: &str, near_byte: u32) -> Option<(u32, bool)> {
    let tree = syn::parse_file(text).ok()?;
    let mut found = None;
    for item in &tree.items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        for attr in &item_struct.attrs {
            check_account_attr(attr, near_byte, &mut found);
        }
        for field in &item_struct.fields {
            for attr in &field.attrs {
                check_account_attr(attr, near_byte, &mut found);
            }
        }
    }
    found
}

fn check_account_attr(attr: &Attribute, near_byte: u32, found: &mut Option<(u32, bool)>) {
    if !attr.path().is_ident("account") {
        return;
    }
    let syn::Meta::List(list) = &attr.meta else {
        return;
    };
    let (open, close) = delim_open_close(&list.delimiter);
    let open_r = open.byte_range();
    let close_r = close.byte_range();
    if (open_r.start as u32) <= near_byte && near_byte <= (close_r.end as u32) {
        *found = Some((open_r.end as u32, list.tokens.is_empty()));
    }
}

fn delim_open_close(d: &syn::MacroDelimiter) -> (proc_macro2::Span, proc_macro2::Span) {
    match d {
        syn::MacroDelimiter::Paren(p) => (p.span.open(), p.span.close()),
        syn::MacroDelimiter::Brace(b) => (b.span.open(), b.span.close()),
        syn::MacroDelimiter::Bracket(b) => (b.span.open(), b.span.close()),
    }
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
