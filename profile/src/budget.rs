//! Checked-in ceilings for static CU estimates and program binary size.

use {
    crate::aggregate::ProfileResult,
    serde::{Deserialize, Serialize},
    std::{collections::BTreeMap, fs, path::Path},
};

pub(crate) const BUDGET_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BudgetFile {
    pub(crate) version: u32,
    #[serde(default)]
    pub(crate) program: BTreeMap<String, ProgramBudget>,
}

impl Default for BudgetFile {
    fn default() -> Self {
        Self {
            version: BUDGET_VERSION,
            program: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProgramBudget {
    pub(crate) binary_size: u64,
    pub(crate) total_cu: u64,
    #[serde(default)]
    pub(crate) functions: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Measurement {
    pub(crate) program: String,
    pub(crate) binary_size: u64,
    pub(crate) total_cu: u64,
    pub(crate) functions: BTreeMap<String, u64>,
}

impl Measurement {
    pub(crate) fn from_profile(
        result: &ProfileResult,
        program: impl Into<String>,
        binary_size: u64,
    ) -> Self {
        Self {
            program: program.into(),
            binary_size,
            total_cu: result.total_cus,
            functions: result.function_cus.iter().cloned().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Violation {
    pub(crate) metric: String,
    pub(crate) actual: u64,
    pub(crate) maximum: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MachineReport<'a> {
    pub(crate) version: u32,
    #[serde(flatten)]
    pub(crate) measurement: &'a Measurement,
    pub(crate) budget: Option<BudgetStatus<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BudgetStatus<'a> {
    pub(crate) path: &'a Path,
    pub(crate) status: &'static str,
    pub(crate) violations: &'a [Violation],
}

pub(crate) fn load(path: &Path) -> Result<BudgetFile, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read budget {}: {error}", path.display()))?;
    let budget: BudgetFile = toml::from_str(&source)
        .map_err(|error| format!("failed to parse budget {}: {error}", path.display()))?;
    validate_version(&budget, path)?;
    Ok(budget)
}

pub(crate) fn write(
    path: &Path,
    measurement: &Measurement,
    headroom_percent: u32,
) -> Result<(), String> {
    let mut budget = if path.exists() {
        load(path)?
    } else {
        BudgetFile::default()
    };
    budget.program.insert(
        measurement.program.clone(),
        ProgramBudget {
            binary_size: ceiling_with_headroom(measurement.binary_size, headroom_percent),
            total_cu: ceiling_with_headroom(measurement.total_cu, headroom_percent),
            functions: measurement
                .functions
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        ceiling_with_headroom(*value, headroom_percent),
                    )
                })
                .collect(),
        },
    );
    let output = toml::to_string_pretty(&budget)
        .map_err(|error| format!("failed to serialize budget {}: {error}", path.display()))?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create budget directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(path, output)
        .map_err(|error| format!("failed to write budget {}: {error}", path.display()))
}

pub(crate) fn assert(path: &Path, measurement: &Measurement) -> Result<Vec<Violation>, String> {
    let budget = load(path)?;
    let program = budget.program.get(&measurement.program).ok_or_else(|| {
        format!(
            "budget {} has no [program.{}] entry; run `quasar profile --write-budget`",
            path.display(),
            measurement.program
        )
    })?;
    Ok(compare(program, measurement))
}

fn compare(budget: &ProgramBudget, measurement: &Measurement) -> Vec<Violation> {
    let mut violations = Vec::new();
    push_violation(
        &mut violations,
        "binary_size",
        measurement.binary_size,
        budget.binary_size,
    );
    push_violation(
        &mut violations,
        "total_cu",
        measurement.total_cu,
        budget.total_cu,
    );
    for (name, maximum) in &budget.functions {
        let actual = measurement.functions.get(name).copied().unwrap_or(0);
        push_violation(
            &mut violations,
            format!("functions.{name}"),
            actual,
            *maximum,
        );
    }
    violations
}

fn push_violation(
    violations: &mut Vec<Violation>,
    metric: impl Into<String>,
    actual: u64,
    maximum: u64,
) {
    if actual > maximum {
        violations.push(Violation {
            metric: metric.into(),
            actual,
            maximum,
        });
    }
}

fn validate_version(budget: &BudgetFile, path: &Path) -> Result<(), String> {
    if budget.version == BUDGET_VERSION {
        Ok(())
    } else {
        Err(format!(
            "unsupported budget version {} in {}; expected {BUDGET_VERSION}",
            budget.version,
            path.display()
        ))
    }
}

fn ceiling_with_headroom(value: u64, percent: u32) -> u64 {
    let numerator = u128::from(value) * u128::from(100 + percent);
    let ceiling = numerator.div_ceil(100);
    u64::try_from(ceiling).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use {
        super::{
            assert, ceiling_with_headroom, load, write, MachineReport, Measurement, BUDGET_VERSION,
        },
        std::collections::BTreeMap,
        tempfile::tempdir,
    };

    fn measurement() -> Measurement {
        Measurement {
            program: "vault".to_string(),
            binary_size: 1_001,
            total_cu: 2_001,
            functions: BTreeMap::from([
                ("a::hot".to_string(), 1_500),
                ("b::cold".to_string(), 501),
            ]),
        }
    }

    #[test]
    fn headroom_rounds_up_without_floating_point() {
        assert_eq!(ceiling_with_headroom(1_001, 5), 1_052);
        assert_eq!(ceiling_with_headroom(0, 5), 0);
    }

    #[test]
    fn write_round_trips_and_preserves_other_programs() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("quasar-budget.toml");
        write(&path, &measurement(), 5).unwrap();

        let mut other = measurement();
        other.program = "escrow".to_string();
        write(&path, &other, 0).unwrap();

        let budget = load(&path).unwrap();
        assert_eq!(budget.program["vault"].binary_size, 1_052);
        assert_eq!(budget.program["vault"].total_cu, 2_102);
        assert_eq!(budget.program["vault"].functions["a::hot"], 1_575);
        assert!(budget.program.contains_key("escrow"));
    }

    #[test]
    fn assertion_reports_every_violation_in_stable_order() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("quasar-budget.toml");
        write(&path, &measurement(), 0).unwrap();

        let mut regressed = measurement();
        regressed.binary_size += 1;
        regressed.total_cu += 2;
        *regressed.functions.get_mut("a::hot").unwrap() += 3;
        *regressed.functions.get_mut("b::cold").unwrap() += 4;

        let violations = assert(&path, &regressed).unwrap();
        assert_eq!(
            violations
                .iter()
                .map(|violation| violation.metric.as_str())
                .collect::<Vec<_>>(),
            [
                "binary_size",
                "total_cu",
                "functions.a::hot",
                "functions.b::cold"
            ]
        );
    }

    #[test]
    fn malformed_unknown_and_missing_program_budgets_fail_clearly() {
        let temp = tempdir().unwrap();
        let malformed = temp.path().join("malformed.toml");
        std::fs::write(&malformed, "version = nope").unwrap();
        assert!(load(&malformed)
            .unwrap_err()
            .contains("failed to parse budget"));

        let unknown = temp.path().join("unknown.toml");
        std::fs::write(&unknown, "version = 2").unwrap();
        assert!(load(&unknown)
            .unwrap_err()
            .contains("unsupported budget version 2"));

        let empty = temp.path().join("empty.toml");
        std::fs::write(&empty, "version = 1").unwrap();
        assert!(assert(&empty, &measurement())
            .unwrap_err()
            .contains("has no [program.vault] entry"));
    }

    #[test]
    fn machine_report_is_deterministic_and_camel_cased() {
        let measurement = measurement();
        let report = MachineReport {
            version: BUDGET_VERSION,
            measurement: &measurement,
            budget: None,
        };
        assert_eq!(
            serde_json::to_string_pretty(&report).unwrap(),
            concat!(
                "{\n",
                "  \"version\": 1,\n",
                "  \"program\": \"vault\",\n",
                "  \"binarySize\": 1001,\n",
                "  \"totalCu\": 2001,\n",
                "  \"functions\": {\n",
                "    \"a::hot\": 1500,\n",
                "    \"b::cold\": 501\n",
                "  },\n",
                "  \"budget\": null\n",
                "}"
            )
        );
    }
}
