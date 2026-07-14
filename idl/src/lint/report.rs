use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleCode {
    L001,
    P001,
    P002,
    P005,
    P006,
    P007,
    P008,
    P009,
    P010,
    P011,
    R001,
    R002,
    R003,
    R004,
    R005,
    R006,
    R007,
    R008,
    R009,
    R010,
    R011,
    R012,
    R013,
    R014,
    R015,
    R016,
}

impl RuleCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L001 => "L001",
            Self::P001 => "P001",
            Self::P002 => "P002",
            Self::P005 => "P005",
            Self::P006 => "P006",
            Self::P007 => "P007",
            Self::P008 => "P008",
            Self::P009 => "P009",
            Self::P010 => "P010",
            Self::P011 => "P011",
            Self::R001 => "R001",
            Self::R002 => "R002",
            Self::R003 => "R003",
            Self::R004 => "R004",
            Self::R005 => "R005",
            Self::R006 => "R006",
            Self::R007 => "R007",
            Self::R008 => "R008",
            Self::R009 => "R009",
            Self::R010 => "R010",
            Self::R011 => "R011",
            Self::R012 => "R012",
            Self::R013 => "R013",
            Self::R014 => "R014",
            Self::R015 => "R015",
            Self::R016 => "R016",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::L001 => "disconnected account graph",
            Self::P001 => "account missing version field",
            Self::P002 => "account missing reserved padding",
            Self::P005 => "account name collision",
            Self::P006 => "instruction missing signer",
            Self::P007 => "unbounded remaining accounts",
            Self::P008 => "auto instruction discriminators without lockfile",
            Self::P009 => "account discriminator collision",
            Self::P010 => "event discriminator collision",
            Self::P011 => "error code collision",
            Self::R001 => "account field reorder",
            Self::R002 => "account field retype",
            Self::R003 => "account field removed",
            Self::R004 => "account field inserted in the middle",
            Self::R005 => "account field appended",
            Self::R006 => "account discriminator changed",
            Self::R007 => "instruction removed",
            Self::R008 => "instruction argument changed",
            Self::R009 => "instruction account list changed",
            Self::R010 => "instruction account flags changed",
            Self::R011 => "enum variant removed or inserted",
            Self::R012 => "enum variant appended",
            Self::R013 => "PDA seed changed",
            Self::R014 => "instruction discriminator changed",
            Self::R015 => "account removed",
            Self::R016 => "event discriminator changed",
        }
    }

    pub fn default_severity(self) -> Severity {
        match self {
            Self::R001
            | Self::R002
            | Self::R003
            | Self::R004
            | Self::R006
            | Self::R007
            | Self::R008
            | Self::R009
            | Self::R010
            | Self::R011
            | Self::R013
            | Self::R014
            | Self::R015
            | Self::R016
            // Account discriminator collisions are silent type confusion at
            // runtime (the discriminator check is a prefix compare and every
            // `#[account]` shares `OWNER = crate::ID`), so they are a hard error
            // — mirrored by the `build_idl` collision panic.
            | Self::P009 => Severity::Error,
            Self::R005
            | Self::L001
            | Self::P001
            | Self::P002
            | Self::P005
            | Self::P006
            | Self::P007
            | Self::P008
            | Self::P010
            | Self::P011 => Severity::Warning,
            Self::R012 => Severity::Info,
        }
    }
}

impl fmt::Display for RuleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub rule: RuleCode,
    pub severity: Severity,
    pub target: String,
    pub message: String,
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn new(
        rule: RuleCode,
        target: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            rule,
            severity: rule.default_severity(),
            target: target.into(),
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LintReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl LintReport {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn extend(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn contains(&self, rule: RuleCode) -> bool {
        self.diagnostics.iter().any(|diag| diag.rule == rule)
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diag| diag.severity == Severity::Error)
    }

    pub fn should_fail(&self, config: &LintConfig) -> bool {
        self.diagnostics.iter().any(|diag| {
            diag.severity == Severity::Error
                || (config.strict && matches!(diag.severity, Severity::Warning | Severity::Info))
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    /// Treat warnings and info findings as failures. Intended for audit/CI.
    pub strict: bool,
    /// Whether the current program surface is protected by `quasar.lock.json`.
    pub lockfile_present: bool,
}
