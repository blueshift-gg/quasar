mod checks;
mod helpers;
mod parsers;

use {
    core::fmt::{Display, Formatter, Result},
    quasar_idl::parser::{self, module_resolver},
    std::{collections::HashMap, path::Path},
};

pub fn audit_program(crate_root: &Path) -> Vec<Finding> {
    let files = module_resolver::resolve_crate(crate_root);
    let parsed = parser::parse_program_from_files(crate_root, &files);

    let sources: HashMap<String, &str> = files
        .iter()
        .map(|rf| (rf.path.display().to_string(), rf.source.as_str()))
        .collect();

    let mut accounts_structs = Vec::new();
    for file in &files {
        let path = file.path.display().to_string();
        accounts_structs.extend(parsers::extract_audit_accounts(&file.file, &path));
    }

    let mut findings = Vec::new();

    checks::check_discriminator_collisions(&parsed, &mut findings);
    let program_has_drain_path = files
        .iter()
        .any(|rf| helpers::file_contains_drain_pattern(&rf.file));

    for ix in &parsed.instructions {
        let accounts = accounts_structs
            .iter()
            .find(|a| a.name == ix.accounts_type_name);

        if let Some(accounts) = accounts {
            let src = sources
                .get(&accounts.file_path)
                .copied()
                .unwrap_or_default();
            checks::check_missing_signer(ix, accounts, src, &mut findings);
            checks::check_untyped_accounts(ix, accounts, src, &mut findings);
            checks::check_data_matching(ix, accounts, src, &mut findings);
            checks::check_duplicate_mutable(ix, accounts, src, &mut findings);
            checks::check_pda_specificity(ix, accounts, src, &mut findings);
            checks::check_arbitrary_cpi(ix, accounts, src, &mut findings);
            checks::check_init_if_needed(ix, accounts, program_has_drain_path, src, &mut findings);
            checks::check_revival_attack(ix, accounts, &files, src, &mut findings);
        }
    }

    findings
}

pub struct Finding {
    pub severity: Severity,
    pub rule: Rule,
    pub location: String,
    pub source_file: String,
    pub source_line: usize,
    pub snippet: String,
    pub message: String,
}

pub(crate) fn extract_snippet(source: &str, line: usize) -> String {
    if line == 0 {
        return String::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let start = line.saturating_sub(2);
    let end = (line + 1).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let num = start + i + 1;
            let marker = if num == line { ">" } else { " " };
            format!("{marker} {num:>4} | {l}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

impl Display for Severity {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    DiscriminatorCollision,
    MissingSigner,
    UntypedAccount,
    DuplicateMutable,
    PdaSharing,
    ArbitraryCpi,
    InitIfNeeded,
    DataMatching,
    RevivalAttack,
}

impl Rule {
    pub fn learn_url(self) -> &'static str {
        match self {
            Rule::DiscriminatorCollision => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/type-cosplay"
            ),
            Rule::MissingSigner => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/signer-checks"
            ),
            Rule::UntypedAccount => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/owner-checks"
            ),
            Rule::DuplicateMutable => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/duplicate-mutable-accounts"
            ),
            Rule::PdaSharing => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/pda-sharing"
            ),
            Rule::ArbitraryCpi => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/arbitrary-cpi"
            ),
            Rule::InitIfNeeded => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/reinitialization-attacks"
            ),
            Rule::DataMatching => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/data-matching"
            ),
            Rule::RevivalAttack => concat!(
                "https://learn.blueshift.gg/en/courses/program-security",
                "/revival-attacks"
            ),
        }
    }
}

impl Display for Rule {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Rule::DiscriminatorCollision => write!(f, "discriminator-collision"),
            Rule::MissingSigner => write!(f, "missing-signer"),
            Rule::UntypedAccount => write!(f, "untyped-account"),
            Rule::DuplicateMutable => write!(f, "duplicate-mutable"),
            Rule::PdaSharing => write!(f, "pda-sharing"),
            Rule::ArbitraryCpi => write!(f, "arbitrary-cpi"),
            Rule::InitIfNeeded => write!(f, "init-if-needed"),
            Rule::DataMatching => write!(f, "data-matching"),
            Rule::RevivalAttack => write!(f, "revival-attack"),
        }
    }
}
