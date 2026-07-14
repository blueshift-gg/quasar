use {
    super::{
        report::{Diagnostic, LintConfig, LintReport, RuleCode},
        surface::{
            AccountMetaSurface, AccountSurface, FieldSurface, InstructionSurface, ProgramSurface,
            TypeSurface,
        },
    },
    std::collections::{BTreeMap, BTreeSet, VecDeque},
};

const RESERVED_ACCOUNT_NAMES: &[&str] = &[
    "Account",
    "AccountInfo",
    "Address",
    "Mint",
    "Program",
    "Pubkey",
    "Signer",
    "System",
    "SystemAccount",
    "Token",
    "TokenAccount",
    "UncheckedAccount",
];

pub fn preflight(surface: &ProgramSurface, config: &LintConfig) -> LintReport {
    let mut report = LintReport::default();
    preflight_accounts(surface, &mut report);
    preflight_instructions(surface, config, &mut report);
    preflight_discriminators(surface, &mut report);
    graph_checks(surface, &mut report);
    report
}

pub fn diff(old: &ProgramSurface, new: &ProgramSurface) -> LintReport {
    let mut report = LintReport::default();
    diff_accounts(old, new, &mut report);
    diff_instructions(old, new, &mut report);
    diff_enums(old, new, &mut report);
    diff_events(old, new, &mut report);
    report
}

fn preflight_accounts(surface: &ProgramSurface, report: &mut LintReport) {
    for account in &surface.accounts {
        if !has_leading_version(account) {
            report.push(Diagnostic::new(
                RuleCode::P001,
                account.name.clone(),
                format!(
                    "account `{}` has no leading `version: u8` or `version: u16` field",
                    account.name
                ),
                "add a leading version field before first deployment so future layouts can branch \
                 explicitly",
            ));
        }

        if !has_trailing_reserved_padding(account) {
            report.push(Diagnostic::new(
                RuleCode::P002,
                account.name.clone(),
                format!(
                    "account `{}` has no trailing `_reserved: [u8; N]` padding",
                    account.name
                ),
                "reserve trailing bytes before deployment to make small future appends \
                 migration-free",
            ));
        }

        if RESERVED_ACCOUNT_NAMES.contains(&account.name.as_str()) {
            report.push(Diagnostic::new(
                RuleCode::P005,
                account.name.clone(),
                format!(
                    "account `{}` collides with a well-known Solana or Quasar type name",
                    account.name
                ),
                "rename the account to a program-specific type before clients depend on the IDL \
                 name",
            ));
        }
    }
}

fn has_leading_version(account: &AccountSurface) -> bool {
    matches!(
        account.fields.first(),
        Some(field) if field.name == "version" && matches!(field.ty.as_str(), "u8" | "u16")
    )
}

fn has_trailing_reserved_padding(account: &AccountSurface) -> bool {
    matches!(
        account.fields.last(),
        Some(field)
            if field.name == "_reserved"
                && field.ty.starts_with("[u8; ")
                && field.ty.ends_with(']')
    )
}

fn preflight_instructions(surface: &ProgramSurface, config: &LintConfig, report: &mut LintReport) {
    for instruction in &surface.instructions {
        if instruction.discriminator_source.as_deref() == Some("auto") && !config.lockfile_present {
            report.push(Diagnostic::new(
                RuleCode::P008,
                instruction.name.clone(),
                format!(
                    "instruction `{}` uses an auto discriminator without a lockfile",
                    instruction.name
                ),
                "run `quasar lint --update-lock` before deployment so future reorders cannot \
                 silently move instruction discriminators",
            ));
        }

        if !instruction_has_signer(instruction) {
            report.push(Diagnostic::new(
                RuleCode::P006,
                instruction.name.clone(),
                format!(
                    "instruction `{}` declares no signer account or signer remaining-account \
                     policy",
                    instruction.name
                ),
                "require an authority signer unless the instruction is intentionally \
                 permissionless",
            ));
        }

        if instruction
            .remaining_accounts
            .as_deref()
            .is_some_and(|remaining| remaining.contains("\"max\":null"))
        {
            report.push(Diagnostic::new(
                RuleCode::P007,
                instruction.name.clone(),
                format!(
                    "instruction `{}` accepts remaining accounts without an upper bound",
                    instruction.name
                ),
                "set a bounded Remaining<T, N> shape so audits can reason about account count and \
                 cost",
            ));
        }
    }
}

fn instruction_has_signer(instruction: &InstructionSurface) -> bool {
    instruction
        .accounts
        .iter()
        .any(AccountMetaSurface::signer_required)
        || instruction
            .remaining_accounts
            .as_deref()
            .is_some_and(|remaining| remaining.contains("\"signer\":true"))
        || instruction
            .remaining_accounts
            .as_deref()
            .is_some_and(|remaining| remaining.contains("dynamic"))
}

/// Whole-program discriminator/code collision analysis.
///
/// - Account discriminators collide when equal OR when one is a prefix of the
///   other: the runtime discriminator check reads only the declared-length
///   prefix (`checks/discriminator.rs`), and every `#[account]` shares `OWNER =
///   crate::ID`, so a prefix relation is silent type confusion. Empty
///   discriminators (`unsafe_no_disc`) are an explicit opt-out and are skipped.
/// - Event discriminators collide on exact duplication.
/// - Error codes collide on exact duplication (codes are 6000-based).
fn preflight_discriminators(surface: &ProgramSurface, report: &mut LintReport) {
    for (i, account) in surface.accounts.iter().enumerate() {
        for other in &surface.accounts[i + 1..] {
            if account_discriminators_collide(&account.discriminator, &other.discriminator) {
                report.push(Diagnostic::new(
                    RuleCode::P009,
                    account.name.clone(),
                    format!(
                        "accounts `{}` and `{}` have colliding discriminators ({:?} vs {:?}); the \
                         runtime check is a prefix compare, so one can be decoded as the other",
                        account.name, other.name, account.discriminator, other.discriminator
                    ),
                    "give every account type a distinct discriminator that is not a prefix of \
                     another",
                ));
            }
        }
    }

    for (i, event) in surface.events.iter().enumerate() {
        for other in &surface.events[i + 1..] {
            if event.discriminator == other.discriminator {
                report.push(Diagnostic::new(
                    RuleCode::P010,
                    event.name.clone(),
                    format!(
                        "events `{}` and `{}` share discriminator {:?}; off-chain indexers cannot \
                         tell their logs apart",
                        event.name, other.name, event.discriminator
                    ),
                    "give every event a distinct discriminator",
                ));
            }
        }
    }

    for (i, error) in surface.errors.iter().enumerate() {
        for other in &surface.errors[i + 1..] {
            if error.code == other.code {
                report.push(Diagnostic::new(
                    RuleCode::P011,
                    error.name.clone(),
                    format!(
                        "errors `{}` and `{}` share code {}; clients cannot distinguish the two \
                         failures",
                        error.name, other.name, error.code
                    ),
                    "give every error variant a distinct code",
                ));
            }
        }
    }
}

/// Two account discriminators collide when either is a prefix of the other
/// (which includes exact equality). Empty discriminators are skipped: they mark
/// an explicit `unsafe_no_disc` opt-out with no discriminator to compare.
fn account_discriminators_collide(a: &[u8], b: &[u8]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let shared = a.len().min(b.len());
    a[..shared] == b[..shared]
}

fn graph_checks(surface: &ProgramSurface, report: &mut LintReport) {
    for instruction in &surface.instructions {
        let components = connected_components(instruction);
        if components.len() > 1 {
            let rendered = components
                .iter()
                .map(|component| format!("{{{}}}", component.join(", ")))
                .collect::<Vec<_>>()
                .join(", ");
            report.push(Diagnostic::new(
                RuleCode::L001,
                instruction.name.clone(),
                format!(
                    "instruction `{}` has disconnected account groups: {rendered}",
                    instruction.name
                ),
                "connect account groups through PDA seeds, account-field resolvers, or explicit \
                 signer authority paths",
            ));
        }
    }
}

fn connected_components(instruction: &InstructionSurface) -> Vec<Vec<String>> {
    let names: BTreeSet<&str> = instruction
        .accounts
        .iter()
        .filter(|account| is_graph_relevant(account))
        .map(|account| account.name.as_str())
        .collect();
    if names.len() < 2 {
        return Vec::new();
    }

    let mut edges: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for account in &instruction.accounts {
        if !is_graph_relevant(account) {
            continue;
        }
        edges.entry(account.name.as_str()).or_default();
        for reference in &account.resolver_refs {
            if names.contains(reference.as_str()) {
                edges
                    .entry(account.name.as_str())
                    .or_default()
                    .insert(reference.as_str());
                edges
                    .entry(reference.as_str())
                    .or_default()
                    .insert(account.name.as_str());
            }
        }
    }

    let mut seen = BTreeSet::new();
    let mut components = Vec::new();
    for name in names {
        if seen.contains(name) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([name]);
        seen.insert(name);
        while let Some(current) = queue.pop_front() {
            component.push(current.to_owned());
            if let Some(neighbors) = edges.get(current) {
                for neighbor in neighbors {
                    if seen.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        components.push(component);
    }

    if components.len() > 1 {
        components
    } else {
        Vec::new()
    }
}

fn is_graph_relevant(account: &AccountMetaSurface) -> bool {
    account.graph_relevant
}

fn diff_accounts(old: &ProgramSurface, new: &ProgramSurface, report: &mut LintReport) {
    for old_account in &old.accounts {
        let Some(new_account) = new
            .accounts
            .iter()
            .find(|account| account.name == old_account.name)
        else {
            report.push(Diagnostic::new(
                RuleCode::R015,
                old_account.name.clone(),
                format!(
                    "account `{}` was removed; existing accounts of that type become unreadable",
                    old_account.name
                ),
                "keep the account type for at least one release or ship an explicit migration \
                 before removal",
            ));
            continue;
        };

        if old_account.discriminator != new_account.discriminator {
            report.push(Diagnostic::new(
                RuleCode::R006,
                old_account.name.clone(),
                format!(
                    "account `{}` discriminator changed from {:?} to {:?}",
                    old_account.name, old_account.discriminator, new_account.discriminator
                ),
                "restore the old discriminator or migrate every existing account before releasing",
            ));
        }

        diff_account_fields(old_account, new_account, report);
    }
}

fn diff_account_fields(
    old_account: &AccountSurface,
    new_account: &AccountSurface,
    report: &mut LintReport,
) {
    let old_names = old_account
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    let new_names = new_account
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    if old_names != new_names && same_set(&old_names, &new_names) {
        report.push(Diagnostic::new(
            RuleCode::R001,
            old_account.name.clone(),
            format!(
                "fields in account `{}` were reordered from {:?} to {:?}",
                old_account.name, old_names, new_names
            ),
            "restore the original field order; deployed bytes are laid out in declaration order",
        ));
    }

    for old_field in &old_account.fields {
        match new_account
            .fields
            .iter()
            .find(|field| field.name == old_field.name)
        {
            Some(new_field) if field_shape(old_field) != field_shape(new_field) => {
                report.push(Diagnostic::new(
                    RuleCode::R002,
                    format!("{}.{}", old_account.name, old_field.name),
                    format!(
                        "field `{}.{}` changed type from `{}` to `{}`",
                        old_account.name, old_field.name, old_field.ty, new_field.ty
                    ),
                    "keep the old field shape and add a new field, or migrate every existing \
                     account",
                ));
            }
            Some(_) => {}
            None => report.push(Diagnostic::new(
                RuleCode::R003,
                format!("{}.{}", old_account.name, old_field.name),
                format!(
                    "field `{}.{}` was removed from the account layout",
                    old_account.name, old_field.name
                ),
                "keep deprecated fields in place unless a migration rewrites every existing \
                 account",
            )),
        }
    }

    for (index, new_field) in new_account.fields.iter().enumerate() {
        if old_names.contains(&new_field.name.as_str()) {
            continue;
        }

        let inserted_before_existing = new_account.fields[index + 1..]
            .iter()
            .any(|field| old_names.contains(&field.name.as_str()));
        if inserted_before_existing {
            report.push(Diagnostic::new(
                RuleCode::R004,
                format!("{}.{}", old_account.name, new_field.name),
                format!(
                    "field `{}.{}` was inserted before existing fields",
                    old_account.name, new_field.name
                ),
                "append new fields at the tail or migrate existing accounts before changing \
                 offsets",
            ));
        } else {
            report.push(Diagnostic::new(
                RuleCode::R005,
                format!("{}.{}", old_account.name, new_field.name),
                format!(
                    "field `{}.{}` was appended and existing accounts need realloc or \
                     reserved-space handling",
                    old_account.name, new_field.name
                ),
                "confirm the field consumes reserved padding or ship a bounded realloc migration",
            ));
        }
    }
}

fn same_set(left: &[&str], right: &[&str]) -> bool {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

fn field_shape(field: &FieldSurface) -> (&str, Option<&str>) {
    (field.ty.as_str(), field.codec.as_deref())
}

fn diff_instructions(old: &ProgramSurface, new: &ProgramSurface, report: &mut LintReport) {
    for old_instruction in &old.instructions {
        let Some(new_instruction) = new
            .instructions
            .iter()
            .find(|instruction| instruction.name == old_instruction.name)
        else {
            report.push(Diagnostic::new(
                RuleCode::R007,
                old_instruction.name.clone(),
                format!("instruction `{}` was removed", old_instruction.name),
                "keep a deprecated instruction entry point for one release so existing clients \
                 fail deliberately",
            ));
            continue;
        };

        if old_instruction.args != new_instruction.args {
            report.push(Diagnostic::new(
                RuleCode::R008,
                old_instruction.name.clone(),
                format!(
                    "instruction `{}` arguments changed from `{}` to `{}`",
                    old_instruction.name,
                    render_fields(&old_instruction.args),
                    render_fields(&new_instruction.args)
                ),
                "preserve the old instruction signature and add a new instruction for the new \
                 wire format",
            ));
        }

        if old_instruction.account_names() != new_instruction.account_names()
            || old_instruction.remaining_accounts != new_instruction.remaining_accounts
        {
            report.push(Diagnostic::new(
                RuleCode::R009,
                old_instruction.name.clone(),
                format!(
                    "instruction `{}` account list changed from {:?} to {:?}",
                    old_instruction.name,
                    old_instruction.account_names(),
                    new_instruction.account_names()
                ),
                "preserve positional account order for existing clients or add a new instruction",
            ));
        }

        for old_account in &old_instruction.accounts {
            let Some(new_account) = new_instruction
                .accounts
                .iter()
                .find(|account| account.name == old_account.name)
            else {
                continue;
            };
            if old_account.optional != new_account.optional
                || old_account.signer != new_account.signer
                || old_account.writable != new_account.writable
            {
                report.push(Diagnostic::new(
                    RuleCode::R010,
                    format!("{}.{}", old_instruction.name, old_account.name),
                    format!(
                        "account flags for `{}.{}` changed from optional={}, signer={}, \
                         writable={} to optional={}, signer={}, writable={}",
                        old_instruction.name,
                        old_account.name,
                        old_account.optional,
                        old_account.signer,
                        old_account.writable,
                        new_account.optional,
                        new_account.signer,
                        new_account.writable
                    ),
                    "preserve account meta signer/writable flags for existing clients or add a \
                     new instruction",
                ));
            }
            if old_account.pda_seeds != new_account.pda_seeds {
                report.push(Diagnostic::new(
                    RuleCode::R013,
                    format!("{}.{}", old_instruction.name, old_account.name),
                    format!(
                        "PDA seeds for `{}.{}` changed",
                        old_instruction.name, old_account.name
                    ),
                    "preserve the old seed recipe or migrate every PDA derived from it",
                ));
            }
        }

        if old_instruction.discriminator != new_instruction.discriminator {
            report.push(Diagnostic::new(
                RuleCode::R014,
                old_instruction.name.clone(),
                format!(
                    "instruction `{}` discriminator changed from {:?} to {:?}",
                    old_instruction.name,
                    old_instruction.discriminator,
                    new_instruction.discriminator
                ),
                "restore the old discriminator or publish the change under a new instruction",
            ));
        }
    }
}

fn render_fields(fields: &[FieldSurface]) -> String {
    fields
        .iter()
        .map(|field| format!("{}: {}", field.name, field.ty))
        .collect::<Vec<_>>()
        .join(", ")
}

fn diff_enums(old: &ProgramSurface, new: &ProgramSurface, report: &mut LintReport) {
    for old_type in old.types.iter().filter(|ty| is_enum(ty)) {
        let Some(new_type) = new
            .types
            .iter()
            .find(|ty| ty.name == old_type.name && is_enum(ty))
        else {
            continue;
        };

        let old_variants = variant_keys(old_type);
        let new_variants = variant_keys(new_type);
        if old_variants == new_variants {
            continue;
        }

        if new_variants.starts_with(&old_variants) {
            report.push(Diagnostic::new(
                RuleCode::R012,
                old_type.name.clone(),
                format!(
                    "enum `{}` appended variants: {:?}",
                    old_type.name,
                    &new_variants[old_variants.len()..]
                ),
                "confirm clients and indexers tolerate the new variant before release",
            ));
        } else {
            report.push(Diagnostic::new(
                RuleCode::R011,
                old_type.name.clone(),
                format!(
                    "enum `{}` variants changed from {:?} to {:?}",
                    old_type.name, old_variants, new_variants
                ),
                "only append enum variants at the tail; do not remove, reorder, or insert \
                 variants in the middle",
            ));
        }
    }
}

fn is_enum(ty: &TypeSurface) -> bool {
    ty.kind == "Enum"
}

fn variant_keys(ty: &TypeSurface) -> Vec<String> {
    ty.variants
        .iter()
        .map(|variant| {
            format!(
                "{}={}:{}",
                variant.name,
                variant.value,
                render_fields(&variant.fields)
            )
        })
        .collect()
}

fn diff_events(old: &ProgramSurface, new: &ProgramSurface, report: &mut LintReport) {
    for old_event in &old.events {
        let Some(new_event) = new.events.iter().find(|event| event.name == old_event.name) else {
            continue;
        };
        if old_event.discriminator != new_event.discriminator {
            report.push(Diagnostic::new(
                RuleCode::R016,
                old_event.name.clone(),
                format!(
                    "event `{}` discriminator changed from {:?} to {:?}",
                    old_event.name, old_event.discriminator, new_event.discriminator
                ),
                "restore the old event discriminator so existing off-chain indexers keep matching \
                 logs",
            ));
        }
    }
}

#[cfg(test)]
mod collision_tests {
    use super::{
        super::surface::{ErrorSurface, EventSurface},
        preflight, AccountSurface, LintConfig, ProgramSurface, RuleCode,
    };

    fn account(name: &str, discriminator: Vec<u8>) -> AccountSurface {
        AccountSurface {
            name: name.to_owned(),
            discriminator,
            fields: Vec::new(),
            layout: None,
            space: None,
        }
    }

    fn event(name: &str, discriminator: Vec<u8>) -> EventSurface {
        EventSurface {
            name: name.to_owned(),
            discriminator,
            fields: Vec::new(),
        }
    }

    fn error(name: &str, code: u32) -> ErrorSurface {
        ErrorSurface {
            name: name.to_owned(),
            code,
        }
    }

    fn surface(
        accounts: Vec<AccountSurface>,
        events: Vec<EventSurface>,
        errors: Vec<ErrorSurface>,
    ) -> ProgramSurface {
        ProgramSurface {
            version: 2,
            spec: "quasar-idl/1.0.0".to_owned(),
            name: "collision_test".to_owned(),
            program_id: "11111111111111111111111111111111".to_owned(),
            accounts,
            instructions: Vec::new(),
            types: Vec::new(),
            events,
            errors,
        }
    }

    fn contains(surface: &ProgramSurface, rule: RuleCode) -> bool {
        preflight(surface, &LintConfig::default()).contains(rule)
    }

    #[test]
    fn account_exact_discriminator_pair_collides() {
        let s = surface(
            vec![account("A", vec![1]), account("B", vec![1])],
            vec![],
            vec![],
        );
        assert!(contains(&s, RuleCode::P009));
    }

    #[test]
    fn account_prefix_discriminator_pair_collides() {
        // `[1]` is a prefix of `[1, 2]`; the runtime prefix compare confuses them.
        let s = surface(
            vec![account("A", vec![1]), account("B", vec![1, 2])],
            vec![],
            vec![],
        );
        assert!(contains(&s, RuleCode::P009));
    }

    #[test]
    fn account_distinct_discriminators_are_clean() {
        let s = surface(
            vec![
                account("A", vec![1]),
                account("B", vec![2]),
                account("C", vec![3, 4]),
            ],
            vec![],
            vec![],
        );
        assert!(!contains(&s, RuleCode::P009));
    }

    #[test]
    fn account_empty_discriminator_opt_out_is_skipped() {
        let s = surface(
            vec![account("NoDisc", vec![]), account("B", vec![1])],
            vec![],
            vec![],
        );
        assert!(!contains(&s, RuleCode::P009));
    }

    #[test]
    fn event_duplicate_discriminator_collides() {
        let s = surface(
            vec![],
            vec![event("A", vec![5]), event("B", vec![5])],
            vec![],
        );
        assert!(contains(&s, RuleCode::P010));
    }

    #[test]
    fn event_distinct_discriminators_are_clean() {
        let s = surface(
            vec![],
            vec![event("A", vec![5]), event("B", vec![6])],
            vec![],
        );
        assert!(!contains(&s, RuleCode::P010));
    }

    #[test]
    fn error_duplicate_code_collides() {
        let s = surface(vec![], vec![], vec![error("A", 6000), error("B", 6000)]);
        assert!(contains(&s, RuleCode::P011));
    }

    #[test]
    fn error_distinct_codes_are_clean() {
        let s = surface(vec![], vec![], vec![error("A", 6000), error("B", 6001)]);
        assert!(!contains(&s, RuleCode::P011));
    }
}
