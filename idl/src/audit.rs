//! Access to Quasar compiler validation plans stored in IDL extensions.

use {
    crate::types::{
        Idl, IdlValidationPlan, VALIDATION_EXTENSION_KEY, VALIDATION_EXTENSION_VERSION,
    },
    std::collections::BTreeSet,
};

/// Parse and version-check the compiler validation-plan extension.
pub fn validation_plan(idl: &Idl) -> Result<IdlValidationPlan, String> {
    let extensions = idl
        .extensions
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            "IDL has no Quasar validation plan; rebuild it with this version of `quasar build`"
                .to_string()
        })?;
    let value = extensions.get(VALIDATION_EXTENSION_KEY).ok_or_else(|| {
        format!(
            "IDL extensions do not contain `{VALIDATION_EXTENSION_KEY}`; rebuild the program IDL"
        )
    })?;
    let plan: IdlValidationPlan = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid Quasar validation plan: {error}"))?;
    if plan.version != VALIDATION_EXTENSION_VERSION {
        return Err(format!(
            "unsupported validation-plan version {}; expected {VALIDATION_EXTENSION_VERSION}",
            plan.version
        ));
    }

    let instruction_names: BTreeSet<&str> = idl
        .instructions
        .iter()
        .map(|instruction| instruction.name.as_str())
        .collect();
    if let Some(unknown) = plan
        .instructions
        .keys()
        .find(|name| !instruction_names.contains(name.as_str()))
    {
        return Err(format!(
            "validation plan contains unknown instruction `{unknown}`"
        ));
    }
    if let Some(missing) = idl.instructions.iter().find(|instruction| {
        !instruction.accounts.is_empty() && !plan.instructions.contains_key(&instruction.name)
    }) {
        return Err(format!(
            "validation plan is missing instruction `{}` with declared accounts",
            missing.name
        ));
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use {super::validation_plan, crate::types::Idl, serde_json::json};

    fn idl(plan_instructions: serde_json::Value) -> Idl {
        serde_json::from_value(json!({
            "spec": "quasar-idl/1.0.0",
            "name": "audit_demo",
            "version": "0.1.0",
            "address": "11111111111111111111111111111111",
            "instructions": [{
                "name": "write",
                "discriminator": [1],
                "accounts": [{
                    "name": "state",
                    "writable": true,
                    "resolver": { "kind": "input" }
                }],
                "args": []
            }],
            "extensions": {
                "quasar:validationPlan": {
                    "version": 1,
                    "instructions": plan_instructions
                }
            }
        }))
        .expect("valid audit fixture")
    }

    #[test]
    fn rejects_missing_instruction_plan() {
        let error = validation_plan(&idl(json!({}))).unwrap_err();
        assert!(error.contains("missing instruction `write`"), "{error}");
    }

    #[test]
    fn rejects_unknown_instruction_plan() {
        let error = validation_plan(&idl(json!({
            "unknown": { "rent": "NotNeeded", "accounts": [] }
        })))
        .unwrap_err();
        assert!(error.contains("unknown instruction `unknown`"), "{error}");
    }
}
