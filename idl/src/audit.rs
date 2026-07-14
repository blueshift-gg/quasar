//! Access to Quasar compiler validation plans stored in IDL extensions.

use crate::types::{
    Idl, IdlValidationPlan, VALIDATION_EXTENSION_KEY, VALIDATION_EXTENSION_VERSION,
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
    Ok(plan)
}
