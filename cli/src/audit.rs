use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        AuditCommand,
    },
    quasar_idl::types::{Idl, IdlAccountValidation, IdlValidationPlan},
    std::{fmt::Write, path::Path},
};

pub fn run(command: AuditCommand) -> CliResult {
    let idl = match command.idl_path {
        Some(path) => load_idl(&path)?,
        None => {
            let config = QuasarConfig::load()?;
            crate::idl::load_generated(&config)?.1
        }
    };
    let plan = quasar_idl::audit::validation_plan(&idl).map_err(CliError::message)?;
    if command.json {
        let json = serde_json::to_string_pretty(&plan)
            .map_err(|error| CliError::json_serialize("validation plan", error))?;
        println!("{json}");
    } else {
        print!("{}", render_human(&idl, &plan));
    }
    Ok(())
}

fn load_idl(path: &Path) -> Result<Idl, CliError> {
    let json =
        std::fs::read_to_string(path).map_err(|error| CliError::io_path("read", path, error))?;
    quasar_idl::types::check_spec(&json).map_err(CliError::message)?;
    serde_json::from_str(&json)
        .map_err(|error| CliError::json_parse(format!("IDL file {}", path.display()), error))
}

fn render_human(idl: &Idl, plan: &IdlValidationPlan) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "{} validation plan", idl.name);
    for instruction in &idl.instructions {
        let Some(validation) = plan.instructions.get(&instruction.name) else {
            continue;
        };
        let _ = writeln!(
            output,
            "\n{} (discriminator {:?})",
            instruction.name, instruction.discriminator
        );
        let _ = writeln!(output, "  rent: {}", validation.rent);
        for account in &validation.accounts {
            render_account(&mut output, account);
        }
    }
    output
}

fn render_account(output: &mut String, account: &IdlAccountValidation) {
    let mut access = Vec::new();
    if account.writable {
        access.push("writable");
    } else {
        access.push("readonly");
    }
    if account.signer {
        access.push("signer");
    }
    if account.optional {
        access.push("optional");
    }
    if account.allow_duplicate {
        access.push("duplicates allowed");
    }
    let _ = writeln!(
        output,
        "  {}  [{}]  {} ({})",
        account.name,
        access.join(", "),
        account.wrapper,
        account.account_type
    );
    let _ = writeln!(output, "    load: {}", account.load);
    render_phase(output, "pre-load", &account.pre_load);
    render_phase(output, "post-load", &account.post_load);
    render_phase(output, "epilogue", &account.epilogue);
}

fn render_phase(output: &mut String, label: &str, steps: &[String]) {
    if steps.is_empty() {
        return;
    }
    let _ = writeln!(output, "    {label}:");
    for step in steps {
        let _ = writeln!(output, "      - {step}");
    }
}

#[cfg(test)]
mod tests {
    use {
        super::render_human,
        quasar_idl::types::{
            Idl, IdlAccountValidation, IdlAccountsValidation, IdlInstruction, IdlMetadata,
            IdlValidationPlan, CURRENT_SPEC, VALIDATION_EXTENSION_VERSION,
        },
        std::collections::BTreeMap,
    };

    #[test]
    fn human_output_is_deterministic_and_phase_ordered() {
        let idl = Idl {
            spec: CURRENT_SPEC.to_string(),
            name: "vault".to_string(),
            version: "0.1.0".to_string(),
            address: "11111111111111111111111111111111".to_string(),
            metadata: IdlMetadata::default(),
            docs: vec![],
            instructions: vec![IdlInstruction {
                name: "initialize".to_string(),
                discriminator: vec![0],
                docs: vec![],
                accounts: vec![],
                args: vec![],
                layout: None,
                remaining_accounts: None,
            }],
            accounts: vec![],
            types: vec![],
            events: vec![],
            errors: vec![],
            extensions: None,
            hashes: None,
        };
        let account = IdlAccountValidation {
            name: "state".to_string(),
            account_type: "Account < 'a , State >".to_string(),
            wrapper: "Account".to_string(),
            writable: true,
            signer: false,
            optional: false,
            allow_duplicate: false,
            load: "Fixed(validates=[])".to_string(),
            pre_load: vec!["Init::Program(payer=payer)".to_string()],
            post_load: vec!["UserCheck(HasOne targets=[authority])".to_string()],
            epilogue: vec!["ProgramClose(destination_field=payer)".to_string()],
        };
        let plan = IdlValidationPlan {
            version: VALIDATION_EXTENSION_VERSION,
            instructions: BTreeMap::from([(
                "initialize".to_string(),
                IdlAccountsValidation {
                    rent: "FetchOnce".to_string(),
                    accounts: vec![account],
                },
            )]),
        };

        let expected = concat!(
            "vault validation plan\n\n",
            "initialize (discriminator [0])\n",
            "  rent: FetchOnce\n",
            "  state  [writable]  Account (Account < 'a , State >)\n",
            "    load: Fixed(validates=[])\n",
            "    pre-load:\n",
            "      - Init::Program(payer=payer)\n",
            "    post-load:\n",
            "      - UserCheck(HasOne targets=[authority])\n",
            "    epilogue:\n",
            "      - ProgramClose(destination_field=payer)\n",
        );
        assert_eq!(render_human(&idl, &plan), expected);
        // Determinism: a second render over the same plan is byte-identical.
        assert_eq!(render_human(&idl, &plan), expected);
    }
}
