use {
    crate::{
        extract_snippet,
        helpers::{impl_block_contains_call, impl_block_contains_zero_drain},
        parsers::{AuditAccountsStruct, AuditField},
        Finding, Rule, Severity,
    },
    module_resolver::ResolvedFile,
    quasar_idl::parser::{self, module_resolver},
    std::collections::{HashMap, HashSet},
};

const UNCHECKED_TYPES: &[&str] = &["UncheckedAccount", "AccountInfo", "AccountView"];

fn field_finding(
    severity: Severity,
    rule: Rule,
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    field: &AuditField,
    source: &str,
    message: String,
) -> Finding {
    Finding {
        severity,
        rule,
        location: format!("{} -> {}", ix.name, field.name),
        source_file: accounts.file_path.clone(),
        source_line: field.line,
        snippet: extract_snippet(source, field.line),
        message,
    }
}

pub fn check_discriminator_collisions(parsed: &parser::ParsedProgram, findings: &mut Vec<Finding>) {
    for msg in parser::find_discriminator_collisions(parsed) {
        findings.push(Finding {
            severity: Severity::Critical,
            rule: Rule::DiscriminatorCollision,
            location: "global".to_string(),
            source_file: String::new(),
            source_line: 0,
            snippet: String::new(),
            message: msg.trim().to_string(),
        });
    }
}

pub fn check_missing_signer(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    source: &str,
    findings: &mut Vec<Finding>,
) {
    let has_any_signer = accounts.fields.iter().any(|f| f.signer);
    let has_writable = accounts.fields.iter().any(|f| f.writable);

    if has_writable && !has_any_signer {
        let first_writable = accounts.fields.iter().find(|f| f.writable);
        let line = first_writable.map_or(0, |f| f.line);
        findings.push(Finding {
            severity: Severity::Critical,
            rule: Rule::MissingSigner,
            location: ix.name.clone(),
            source_file: accounts.file_path.clone(),
            source_line: line,
            snippet: extract_snippet(source, line),
            message: "instruction modifies accounts but has no signer — anyone can invoke it"
                .to_string(),
        });
    }
}

pub fn check_untyped_accounts(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    source: &str,
    findings: &mut Vec<Finding>,
) {
    for field in &accounts.fields {
        if !UNCHECKED_TYPES.iter().any(|t| field.type_name == *t) {
            continue;
        }

        if field.has_owner || field.has_constraint || field.has_address {
            continue;
        }

        let signer_names: Vec<&str> = accounts
            .fields
            .iter()
            .filter(|f| f.signer)
            .map(|f| f.name.as_str())
            .collect();
        if field.pda_seed_count > 0
            && field
                .pda_seed_refs
                .iter()
                .any(|r| signer_names.contains(&r.as_str()))
        {
            continue;
        }

        let validated_by_sibling = accounts.fields.iter().any(|sibling| {
            sibling.name != field.name && sibling.has_one.iter().any(|h| h == &field.name)
        });
        if validated_by_sibling {
            continue;
        }

        findings.push(field_finding(
            Severity::Warning,
            Rule::UntypedAccount,
            ix,
            accounts,
            field,
            source,
            format!(
                "`{}` uses unchecked type `{}` — no owner or discriminator validation",
                field.name, field.type_name
            ),
        ));
    }
}

pub fn check_duplicate_mutable(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    source: &str,
    findings: &mut Vec<Finding>,
) {
    let writable_typed: Vec<&AuditField> = accounts
        .fields
        .iter()
        .filter(|f| f.writable && f.type_name == "Account")
        .collect();

    let mut by_type: HashMap<&str, Vec<&AuditField>> = HashMap::new();
    for field in &writable_typed {
        let key = if field.type_inner.is_empty() {
            "unknown"
        } else {
            &field.type_inner
        };
        by_type.entry(key).or_default().push(field);
    }

    for (type_name, fields) in &by_type {
        if fields.len() < 2 {
            continue;
        }

        let has_uniqueness_check = fields
            .iter()
            .any(|f| f.has_constraint || f.has_token_constraint);

        let all_pda = fields.iter().all(|f| f.pda_seed_count > 0);

        if !has_uniqueness_check && !all_pda {
            let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            let first = fields[0];
            findings.push(field_finding(
                Severity::Warning,
                Rule::DuplicateMutable,
                ix,
                accounts,
                first,
                source,
                format!(
                    "multiple writable `Account<{}>` fields ({}) without a constraint checking \
                     key uniqueness",
                    type_name,
                    names.join(", "),
                ),
            ));
        }
    }
}

pub fn check_pda_specificity(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    source: &str,
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
            (
                Severity::Warning,
                format!(
                    "`{}` PDA seeds contain only constants — the same PDA is shared across all \
                     users. Consider adding a user-specific seed to prevent PDA sharing attacks.",
                    field.name,
                ),
            )
        };

        findings.push(field_finding(
            severity,
            Rule::PdaSharing,
            ix,
            accounts,
            field,
            source,
            detail,
        ));
    }
}

pub fn check_arbitrary_cpi(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    source: &str,
    findings: &mut Vec<Finding>,
) {
    for field in &accounts.fields {
        let name_lower = field.name.to_lowercase();
        let looks_like_program = name_lower.contains("program");
        let is_unchecked = UNCHECKED_TYPES.iter().any(|t| field.type_name == *t);

        if looks_like_program
            && is_unchecked
            && field.type_name != "Program"
            && field.type_name != "SystemProgram"
            && !field.has_address
            && !field.has_constraint
            && !field.has_owner
        {
            findings.push(field_finding(
                Severity::Critical,
                Rule::ArbitraryCpi,
                ix,
                accounts,
                field,
                source,
                format!(
                    "`{}` looks like a program account but uses `{}` instead of typed \
                     `Program<T>` — attacker can substitute a malicious program",
                    field.name, field.type_name,
                ),
            ));
        }
    }
}

pub fn check_init_if_needed(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    program_has_drain_path: bool,
    source: &str,
    findings: &mut Vec<Finding>,
) {
    for field in &accounts.fields {
        if !field.has_init_if_needed
            || field.has_constraint
            || field.has_token_constraint
            || !field.has_one.is_empty()
        {
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

        findings.push(field_finding(
            severity,
            Rule::InitIfNeeded,
            ix,
            accounts,
            field,
            source,
            format!(
                "`{}` uses `init_if_needed` without additional constraints — {} . Consider adding \
                 a `has_one` or `constraint` check.",
                field.name, qualifier,
            ),
        ));
    }
}

pub fn check_data_matching(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    source: &str,
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

        let token_auth_reaches_signer = !field.token_authority_ref.is_empty()
            && reaches_signer(&field.token_authority_ref, accounts, &signer_names);

        let sibling_has_one_validates = accounts.fields.iter().any(|sibling| {
            sibling.name != field.name
                && sibling.has_one.iter().any(|h| h == &field.name)
                && (sibling.has_constraint
                    || sibling.pda_seed_count > 0
                    || sibling
                        .has_one
                        .iter()
                        .any(|h| reaches_signer(h, accounts, &signer_names)))
        });

        if has_one_reaches_signer
            || token_auth_reaches_signer
            || sibling_has_one_validates
            || field.has_constraint
            || field.has_token_constraint
        {
            continue;
        }

        findings.push(field_finding(
            Severity::Warning,
            Rule::DataMatching,
            ix,
            accounts,
            field,
            source,
            format!(
                "`{}` is a writable `Account<{}>` with no `has_one`, `constraint`, or PDA seeds \
                 binding it to the signer — an attacker could pass someone else's account if the \
                 stored data isn't validated against the signer",
                field.name,
                if field.type_inner.is_empty() {
                    "?"
                } else {
                    &field.type_inner
                },
            ),
        ));
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

pub fn check_revival_attack(
    ix: &parser::program::RawInstruction,
    accounts: &AuditAccountsStruct,
    files: &[ResolvedFile],
    source: &str,
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
        let is_closeable =
            field.type_name == "Account" || UNCHECKED_TYPES.contains(&field.type_name.as_str());

        if !is_closeable {
            continue;
        }

        findings.push(field_finding(
            Severity::Warning,
            Rule::RevivalAttack,
            ix,
            accounts,
            field,
            source,
            format!(
                "`{}` is writable and the instruction uses `set_lamports` (manual lamport \
                 transfer) without the `close` attribute — if lamports are drained to zero the \
                 account can be revived within the same transaction because the discriminator is \
                 not zeroed. Use `#[account(close = destination)]` instead.",
                field.name,
            ),
        ));
    }
}
