//! Security audit for Quasar programs.
//!
//! Statically analyzes program source files to detect common Solana security
//! vulnerabilities based on the Sealevel Attacks taxonomy:
//!
//! 1. Missing signer checks
//! 2. Untyped / unchecked accounts (owner check bypass)
//! 3. Data matching (missing has_one / PDA binding)
//! 4. Duplicate mutable accounts
//! 5. Reinitialization (risky `init_if_needed`)
//! 6. Revival attacks (manual close without `close` attribute)
//! 7. Arbitrary CPI (untyped program accounts)
//! 8. Type cosplay (discriminator collisions)
//! 9. PDA sharing (insufficient seed specificity)

use {
    module_resolver::ResolvedFile,
    quasar_idl::parser::{self, helpers, module_resolver},
    std::{
        collections::{HashMap, HashSet},
        path::Path,
    },
    syn::{Fields, Item},
};

pub fn audit_program(crate_root: &Path) -> AuditReport {
    let files = module_resolver::resolve_crate(crate_root);
    let parsed = parser::parse_program_from_files(crate_root, &files);

    let mut accounts_structs = Vec::new();
    for file in &files {
        accounts_structs.extend(extract_audit_accounts(&file.file));
    }

    let mut findings = Vec::new();

    check_discriminator_collisions(&parsed, &mut findings);

    let program_has_drain_path = files.iter().any(|rf| file_contains_drain_pattern(&rf.file));

    for ix in &parsed.instructions {
        let accounts = accounts_structs
            .iter()
            .find(|a| a.name == ix.accounts_type_name);

        if let Some(accounts) = accounts {
            check_missing_signer(ix, accounts, &mut findings);
            check_untyped_accounts(ix, accounts, &mut findings);
            check_data_matching(ix, accounts, &mut findings);
            check_duplicate_mutable(ix, accounts, &mut findings);
            check_pda_specificity(ix, accounts, &mut findings);
            check_arbitrary_cpi(ix, accounts, &mut findings);
            check_init_if_needed(ix, accounts, program_has_drain_path, &mut findings);
            check_revival_attack(ix, accounts, &files, &mut findings);
        }
    }

    AuditReport { findings }
}

pub struct AuditReport {
    pub findings: Vec<Finding>,
}

pub struct Finding {
    pub severity: Severity,
    pub rule: &'static str,
    pub instruction: Option<String>,
    pub field: Option<String>,
    pub message: String,
    pub learn_url: Option<&'static str>,
}

/// Ordered by display priority: `Critical < Warning < Info` so that
/// `sort_by_key(|f| f.severity)` prints the most severe findings first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

struct AuditAccountsStruct {
    name: String,
    fields: Vec<AuditField>,
}

struct AuditField {
    name: String,
    type_name: String,
    type_inner: Option<String>,
    writable: bool,
    signer: bool,
    #[allow(dead_code)]
    has_init: bool,
    has_init_if_needed: bool,
    has_close: bool,
    has_one: Vec<String>,
    has_constraint: bool,
    has_address: bool,
    has_owner: bool,
    pda_seed_count: usize,
    pda_has_account_ref: bool,
    pda_seed_refs: Vec<String>,
}

fn extract_audit_accounts(file: &syn::File) -> Vec<AuditAccountsStruct> {
    let mut result = Vec::new();
    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            if !has_derive_accounts(&item_struct.attrs) {
                continue;
            }

            let name = item_struct.ident.to_string();
            let sibling_names: Vec<String> = match &item_struct.fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
                    .collect(),
                _ => vec![],
            };

            let fields = match &item_struct.fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| parse_audit_field(f, &sibling_names))
                    .collect(),
                _ => continue,
            };

            result.push(AuditAccountsStruct { name, fields });
        }
    }
    result
}

fn has_derive_accounts(attrs: &[syn::Attribute]) -> bool {
    // Note: `contains("Accounts")` may match derives like `BorshAccountsCoder`,
    // which is acceptable for this framework where `Accounts` is the canonical derive.
    attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .meta
                .require_list()
                .ok()
                .is_some_and(|l| l.tokens.to_string().contains("Accounts"))
    })
}

fn parse_audit_field(field: &syn::Field, sibling_names: &[String]) -> AuditField {
    let name = field
        .ident
        .as_ref()
        .expect("named field must have ident")
        .to_string();
    let type_name = helpers::type_base_name(&field.ty).unwrap_or_default();
    let type_inner = helpers::type_inner_name(&field.ty);
    let signer = helpers::is_signer_type(&field.ty);

    let mut writable = helpers::is_mut_ref(&field.ty);
    let mut has_init = false;
    let mut has_init_if_needed = false;
    let mut has_close = false;
    let mut has_one = Vec::new();
    let mut has_constraint = false;
    let mut has_address = false;
    let mut has_owner = false;
    let mut pda_seed_count = 0;
    let mut pda_has_account_ref = false;
    let mut pda_seed_refs = Vec::new();

    for attr in &field.attrs {
        if !attr.path().is_ident("account") {
            continue;
        }
        let tokens_str = match attr.meta.require_list() {
            Ok(list) => list.tokens.to_string(),
            Err(_) => continue,
        };

        for d in split_directives(&tokens_str) {
            if d == "mut" {
                writable = true;
            } else if d == "init" {
                has_init = true;
                writable = true;
            } else if d == "init_if_needed" {
                has_init_if_needed = true;
                writable = true;
            } else if d.starts_with("close") {
                has_close = true;
                writable = true;
            } else if d.starts_with("has_one") {
                if let Some(val) = d
                    .strip_prefix("has_one")
                    .and_then(|s| s.trim().strip_prefix('='))
                {
                    has_one.push(val.trim().to_string());
                }
            } else if d.starts_with("constraint") {
                has_constraint = true;
            } else if d.starts_with("address") {
                has_address = true;
            } else if d.starts_with("owner") {
                has_owner = true;
            } else if d.starts_with("seeds") {
                let (count, has_ref, refs) = count_seeds(d, sibling_names);
                pda_seed_count = count;
                pda_has_account_ref = has_ref;
                pda_seed_refs = refs;
            }
        }
    }

    if type_name == "SystemProgram" || type_name == "Sysvar" {
        has_address = true;
    }

    AuditField {
        name,
        type_name,
        type_inner,
        writable,
        signer,
        has_init,
        has_init_if_needed,
        has_close,
        has_one,
        has_constraint,
        has_address,
        has_owner,
        pda_seed_count,
        pda_has_account_ref,
        pda_seed_refs,
    }
}

fn split_directives(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0u32;
    let mut in_string = false;

    for (i, c) in s.char_indices() {
        match c {
            '"' => in_string = !in_string,
            '[' | '(' if !in_string => depth += 1,
            ']' | ')' if !in_string => depth = depth.saturating_sub(1),
            ',' if depth == 0 && !in_string => {
                let trimmed = s[start..i].trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
                start = i + 1;
            }
            _ => {}
        }
    }

    let trimmed = s[start..].trim();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }

    parts
}

fn count_seeds(seeds_directive: &str, sibling_names: &[String]) -> (usize, bool, Vec<String>) {
    let eq_pos = match seeds_directive.find('=') {
        Some(idx) => idx,
        None => return (0, false, vec![]),
    };
    let after_eq = seeds_directive[eq_pos + 1..].trim();

    let start = match after_eq.find('[') {
        Some(idx) => idx,
        None => return (0, false, vec![]),
    };
    let mut depth = 0;
    let mut end = None;
    for (i, c) in after_eq[start..].chars().enumerate() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = match end {
        Some(idx) => idx,
        None => return (0, false, vec![]),
    };

    let inner = &after_eq[start + 1..end];
    let seeds: Vec<&str> = inner
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let count = seeds.len();
    let refs: Vec<String> = seeds
        .iter()
        .filter_map(|s| {
            let s = s.trim();
            if !s.starts_with("b\"")
                && s.chars().all(|c| c.is_alphanumeric() || c == '_')
                && sibling_names.iter().any(|n| n == s)
            {
                Some(s.to_string())
            } else {
                None
            }
        })
        .collect();
    let has_ref = !refs.is_empty();

    (count, has_ref, refs)
}

fn check_discriminator_collisions(parsed: &parser::ParsedProgram, findings: &mut Vec<Finding>) {
    for msg in parser::find_discriminator_collisions(parsed) {
        findings.push(Finding {
            severity: Severity::Critical,
            rule: "discriminator-collision",
            instruction: None,
            field: None,
            message: msg.trim().to_string(),
            learn_url: Some("https://learn.blueshift.gg/en/courses/program-security/type-cosplay"),
        });
    }
}

/// Sealevel #1: instruction has writable accounts but no signer at all.
fn check_missing_signer(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    findings: &mut Vec<Finding>,
) {
    let has_any_signer = accounts.fields.iter().any(|f| f.signer);
    let has_writable = accounts.fields.iter().any(|f| f.writable);

    if has_writable && !has_any_signer {
        findings.push(Finding {
            severity: Severity::Critical,
            rule: "missing-signer",
            instruction: Some(ix.name.clone()),
            field: None,
            message: "instruction modifies accounts but has no signer — anyone can invoke it"
                .to_string(),
            learn_url: Some("https://learn.blueshift.gg/en/courses/program-security/signer-checks"),
        });
    }
}

/// Sealevel #2: account field uses an unchecked / untyped wrapper.
fn check_untyped_accounts(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    findings: &mut Vec<Finding>,
) {
    let unchecked_types = ["UncheckedAccount", "AccountInfo", "AccountView"];

    for field in &accounts.fields {
        if !unchecked_types.iter().any(|t| field.type_name == *t) {
            continue;
        }

        // Suppress if the developer explicitly constrains ownership
        if field.has_owner || field.has_constraint || field.has_address {
            continue;
        }

        findings.push(Finding {
            severity: Severity::Warning,
            rule: "untyped-account",
            instruction: Some(ix.name.clone()),
            field: Some(field.name.clone()),
            message: format!(
                "`{}` uses unchecked type `{}` — no owner or discriminator validation",
                field.name, field.type_name
            ),
            learn_url: Some("https://learn.blueshift.gg/en/courses/program-security/owner-checks"),
        });
    }
}

/// Sealevel #4: multiple writable accounts of the same type without uniqueness
/// check.
fn check_duplicate_mutable(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    findings: &mut Vec<Finding>,
) {
    let writable_typed: Vec<&AuditField> = accounts
        .fields
        .iter()
        .filter(|f| f.writable && f.type_name == "Account")
        .collect();

    let mut by_type: HashMap<&str, Vec<&AuditField>> = HashMap::new();
    for field in &writable_typed {
        let key = field.type_inner.as_deref().unwrap_or("unknown");
        by_type.entry(key).or_default().push(field);
    }

    for (type_name, fields) in &by_type {
        if fields.len() < 2 {
            continue;
        }

        let has_uniqueness_check = fields.iter().any(|f| f.has_constraint);

        let all_pda = fields.iter().all(|f| f.pda_seed_count > 0);

        if !has_uniqueness_check && !all_pda {
            let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            findings.push(Finding {
                severity: Severity::Warning,
                rule: "duplicate-mutable",
                instruction: Some(ix.name.clone()),
                field: None,
                message: format!(
                    "multiple writable `Account<{}>` fields ({}) without a constraint checking key uniqueness",
                    type_name,
                    names.join(", "),
                ),
                learn_url: Some("https://learn.blueshift.gg/en/courses/program-security/duplicate-mutable-accounts"),
            });
        }
    }
}

/// Sealevel #8: PDA derived without a signer-specific seed.
fn check_pda_specificity(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    findings: &mut Vec<Finding>,
) {
    let signer_names: Vec<&str> = accounts
        .fields
        .iter()
        .filter(|f| f.signer)
        .map(|f| f.name.as_str())
        .collect();

    for field in &accounts.fields {
        if field.pda_seed_count == 0 {
            continue;
        }

        let seeds_ref_signer = field
            .pda_seed_refs
            .iter()
            .any(|r| signer_names.contains(&r.as_str()));

        if seeds_ref_signer {
            continue;
        }

        let (severity, detail) = if field.pda_has_account_ref {
            (
                Severity::Info,
                format!(
                    "`{}` PDA seeds reference account(s) ({}) but none are signers — the PDA may \
                     be shared across users if those accounts are not user-specific. Consider \
                     adding a signer-derived seed.",
                    field.name,
                    field.pda_seed_refs.join(", "),
                ),
            )
        } else {
            // Only constant seeds
            (
                Severity::Warning,
                format!(
                    "`{}` PDA seeds contain only constants — the same PDA is shared across all \
                     users. Consider adding a user-specific seed to prevent PDA sharing attacks.",
                    field.name,
                ),
            )
        };

        findings.push(Finding {
            severity,
            rule: "pda-sharing",
            instruction: Some(ix.name.clone()),
            field: Some(field.name.clone()),
            message: detail,
            learn_url: Some("https://learn.blueshift.gg/en/courses/program-security/pda-sharing"),
        });
    }
}

/// Sealevel #9: program account not using typed `Program<T>`.
fn check_arbitrary_cpi(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    findings: &mut Vec<Finding>,
) {
    let unchecked_types = ["UncheckedAccount", "AccountInfo", "AccountView"];

    for field in &accounts.fields {
        let name_lower = field.name.to_lowercase();
        let looks_like_program = name_lower.contains("program");
        let is_unchecked = unchecked_types.iter().any(|t| field.type_name == *t);

        if looks_like_program
            && is_unchecked
            && field.type_name != "Program"
            && field.type_name != "SystemProgram"
            && !field.has_address
            && !field.has_constraint
            && !field.has_owner
        {
            findings.push(Finding {
                severity: Severity::Critical,
                rule: "arbitrary-cpi",
                instruction: Some(ix.name.clone()),
                field: Some(field.name.clone()),
                message: format!(
                    "`{}` looks like a program account but uses `{}` instead of typed \
                     `Program<T>` — attacker can substitute a malicious program",
                    field.name, field.type_name,
                ),
                learn_url: Some(
                    "https://learn.blueshift.gg/en/courses/program-security/arbitrary-cpi",
                ),
            });
        }
    }
}

/// Sealevel #5: `init_if_needed` without additional validation.
fn check_init_if_needed(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    program_has_drain_path: bool,
    findings: &mut Vec<Finding>,
) {
    for field in &accounts.fields {
        if !field.has_init_if_needed || field.has_constraint || !field.has_one.is_empty() {
            continue;
        }

        let (severity, qualifier) = if program_has_drain_path {
            (
                Severity::Critical,
                "the program contains a lamport-drain path that could make it reinitializable",
            )
        } else {
            (
                Severity::Info,
                "no drain path detected, but verify no future instruction introduces one",
            )
        };

        findings.push(Finding {
            severity,
            rule: "init-if-needed",
            instruction: Some(ix.name.clone()),
            field: Some(field.name.clone()),
            message: format!(
                "`{}` uses `init_if_needed` without additional constraints — {} . Consider adding \
                 a `has_one` or `constraint` check.",
                field.name, qualifier,
            ),
            learn_url: Some(
                "https://learn.blueshift.gg/en/courses/program-security/reinitialization-attacks",
            ),
        });
    }
}

/// Sealevel #3: writable program-owned account without `has_one`, `constraint`,
fn check_data_matching(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    findings: &mut Vec<Finding>,
) {
    let has_signer = accounts.fields.iter().any(|f| f.signer);
    if !has_signer {
        return;
    }

    let signer_names: Vec<&str> = accounts
        .fields
        .iter()
        .filter(|f| f.signer)
        .map(|f| f.name.as_str())
        .collect();

    for field in &accounts.fields {
        if !field.writable || field.signer || field.type_name != "Account" || field.has_address {
            continue;
        }

        if field.pda_seed_count > 0 && field.pda_has_account_ref {
            continue;
        }

        let has_one_reaches_signer = field
            .has_one
            .iter()
            .any(|h| reaches_signer(h, accounts, &signer_names));

        // Check if there's a custom constraint (we can't inspect its content,
        // but its presence suggests the developer added validation)
        if has_one_reaches_signer || field.has_constraint {
            continue;
        }

        findings.push(Finding {
            severity: Severity::Warning,
            rule: "data-matching",
            instruction: Some(ix.name.clone()),
            field: Some(field.name.clone()),
            message: format!(
                "`{}` is a writable `Account<{}>` with no `has_one`, `constraint`, or PDA seeds \
                 binding it to the signer — an attacker could pass someone else's account if the \
                 stored data isn't validated against the signer",
                field.name,
                field.type_inner.as_deref().unwrap_or("?"),
            ),
            learn_url: Some("https://learn.blueshift.gg/en/courses/program-security/data-matching"),
        });
    }
}

fn reaches_signer(name: &str, accounts: &AuditAccountsStruct, signer_names: &[&str]) -> bool {
    let mut visited = HashSet::new();
    let mut stack = vec![name];

    while let Some(current) = stack.pop() {
        if signer_names.contains(&current) {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }

        if let Some(field) = accounts.fields.iter().find(|f| f.name == current) {
            for seed_ref in &field.pda_seed_refs {
                stack.push(seed_ref.as_str());
            }

            for target in &field.has_one {
                stack.push(target.as_str());
            }
        }
    }

    false
}

/// Sealevel #6: manual lamport-drain closure without `close` attribute.
fn check_revival_attack(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    files: &[ResolvedFile],
    findings: &mut Vec<Finding>,
) {
    let writable_without_close: Vec<&AuditField> = accounts
        .fields
        .iter()
        .filter(|f| f.writable && !f.has_close)
        .collect();

    if writable_without_close.is_empty() {
        return;
    }

    let has_drain_to_zero = files
        .iter()
        .any(|rf| impl_block_contains_zero_drain(&rf.file, &accounts.name));

    if !has_drain_to_zero {
        return;
    }

    let uses_close_method = files
        .iter()
        .any(|rf| impl_block_contains_call(&rf.file, &accounts.name, "close"));

    if uses_close_method {
        return;
    }

    for field in &writable_without_close {
        let is_closeable = field.type_name == "Account"
            || field.type_name == "UncheckedAccount"
            || field.type_name == "AccountView";

        if !is_closeable {
            continue;
        }

        findings.push(Finding {
            severity: Severity::Warning,
            rule: "revival-attack",
            instruction: Some(ix.name.clone()),
            field: Some(field.name.clone()),
            message: format!(
                "`{}` is writable and the instruction uses `set_lamports` (manual lamport \
                 transfer) without the `close` attribute — if lamports are drained to zero the \
                 account can be revived within the same transaction because the discriminator is \
                 not zeroed. Use `#[account(close = destination)]` instead.",
                field.name,
            ),
            learn_url: Some(
                "https://learn.blueshift.gg/en/courses/program-security/revival-attacks",
            ),
        });
    }
}

fn file_contains_drain_pattern(file: &syn::File) -> bool {
    use syn::visit::Visit;

    const DRAIN_NAMES: &[&str] = &["set_lamports", "sub_lamports", "assign"];

    struct DrainFinder {
        found: bool,
    }

    impl<'ast> Visit<'ast> for DrainFinder {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if self.found {
                return;
            }
            if DRAIN_NAMES.iter().any(|n| node.method == n) {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_method_call(self, node);
        }

        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if self.found {
                return;
            }
            if let syn::Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    if DRAIN_NAMES.iter().any(|n| seg.ident == n) {
                        self.found = true;
                        return;
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    let mut finder = DrainFinder { found: false };
    finder.visit_file(file);
    finder.found
}

fn impl_blocks_for<'a>(
    file: &'a syn::File,
    struct_name: &str,
) -> impl Iterator<Item = &'a syn::ItemImpl> {
    let struct_name = struct_name.to_string();
    file.items.iter().filter_map(move |item| match item {
        Item::Impl(impl_block) => {
            let matches = matches!(
                impl_block.self_ty.as_ref(),
                syn::Type::Path(tp) if tp.path.segments.iter().any(|s| s.ident == struct_name)
            );
            matches.then_some(impl_block)
        }
        _ => None,
    })
}

fn impl_block_contains_zero_drain(file: &syn::File, struct_name: &str) -> bool {
    use syn::visit::Visit;

    struct ZeroDrainFinder {
        found: bool,
    }

    fn has_zero_literal(args: &syn::punctuated::Punctuated<syn::Expr, syn::token::Comma>) -> bool {
        args.iter().any(|arg| {
            if let syn::Expr::Lit(lit) = arg {
                if let syn::Lit::Int(int) = &lit.lit {
                    return int.base10_digits() == "0";
                }
            }
            false
        })
    }

    impl<'ast> Visit<'ast> for ZeroDrainFinder {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if self.found {
                return;
            }
            if node.method == "set_lamports" && has_zero_literal(&node.args) {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_method_call(self, node);
        }

        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if self.found {
                return;
            }
            if let syn::Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    if seg.ident == "set_lamports" && has_zero_literal(&node.args) {
                        self.found = true;
                        return;
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }
    }

    impl_blocks_for(file, struct_name).any(|impl_block| {
        let mut finder = ZeroDrainFinder { found: false };
        finder.visit_item_impl(impl_block);
        finder.found
    })
}

fn impl_block_contains_call(file: &syn::File, struct_name: &str, fn_name: &str) -> bool {
    use syn::visit::Visit;

    struct CallFinder<'a> {
        target: &'a str,
        found: bool,
    }

    impl<'a, 'ast> Visit<'ast> for CallFinder<'a> {
        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if self.found {
                return;
            }
            if let syn::Expr::Path(p) = &*node.func {
                if let Some(seg) = p.path.segments.last() {
                    if seg.ident == self.target {
                        self.found = true;
                        return;
                    }
                }
            }
            syn::visit::visit_expr_call(self, node);
        }

        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if self.found {
                return;
            }
            if node.method == self.target {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_method_call(self, node);
        }
    }

    impl_blocks_for(file, struct_name).any(|impl_block| {
        let mut finder = CallFinder {
            target: fn_name,
            found: false,
        };
        finder.visit_item_impl(impl_block);
        finder.found
    })
}
