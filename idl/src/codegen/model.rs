use {
    crate::types::{Idl, IdlType},
    quasar_schema::{camel_to_pascal, camel_to_snake, snake_to_pascal},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedIdentity {
    pub program_name: String,
    pub crate_name: String,
    pub client_name: String,
    pub typescript_dir: String,
    pub typescript_package: String,
    pub python_package: String,
    pub go_package: String,
    pub rust_client_crate: String,
}

impl ResolvedIdentity {
    pub fn from_idl(idl: &Idl) -> Self {
        let program_name = idl.metadata.name.clone();
        let crate_name = idl.metadata.crate_name.clone();
        let client_name = idl.metadata.client_name().to_string();
        let go_package = client_name.replace('-', "_");

        Self {
            program_name,
            crate_name,
            typescript_dir: client_name.clone(),
            typescript_package: format!("{client_name}-client"),
            python_package: client_name.clone(),
            go_package: go_package.clone(),
            rust_client_crate: format!("{client_name}-client"),
            client_name,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgramFeatures {
    pub has_instructions: bool,
    pub has_accounts: bool,
    pub has_events: bool,
    pub has_types: bool,
    pub has_errors: bool,
    pub has_args: bool,
    pub has_pdas: bool,
    pub has_pda_account_seeds: bool,
    pub has_public_key: bool,
    pub has_option: bool,
    pub has_dynamic: bool,
    pub has_float: bool,
    pub needs_codecs: bool,
}

impl ProgramFeatures {
    pub fn from_idl(idl: &Idl) -> Self {
        let mut features = Self {
            has_instructions: !idl.instructions.is_empty(),
            has_accounts: !idl.accounts.is_empty(),
            has_events: !idl.events.is_empty(),
            has_types: !idl.types.is_empty(),
            has_errors: !idl.errors.is_empty(),
            has_args: idl.instructions.iter().any(|ix| !ix.args.is_empty()),
            has_pdas: idl
                .instructions
                .iter()
                .any(|ix| ix.accounts.iter().any(|account| account.pda.is_some())),
            has_pda_account_seeds: idl.instructions.iter().any(|ix| {
                ix.accounts.iter().any(|account| {
                    account.pda.as_ref().is_some_and(|pda| {
                        pda.seeds
                            .iter()
                            .any(|seed| matches!(seed, crate::types::IdlSeed::Account { .. }))
                    })
                })
            }),
            ..Self::default()
        };

        let mut visit = |ty: &IdlType| {
            if type_has_public_key(ty) {
                features.has_public_key = true;
            }
            if type_has_option(ty) {
                features.has_option = true;
            }
            if type_has_dynamic(ty) {
                features.has_dynamic = true;
            }
            if type_has_float(ty) {
                features.has_float = true;
            }
        };

        for type_def in &idl.types {
            for field in &type_def.ty.fields {
                visit_type(&field.ty, &mut visit);
            }
        }
        for ix in &idl.instructions {
            for arg in &ix.args {
                visit_type(&arg.ty, &mut visit);
            }
        }

        features.needs_codecs = features.has_types || features.has_args;
        features
    }
}

#[derive(Clone)]
pub struct ProgramModel<'a> {
    pub idl: &'a Idl,
    pub identity: ResolvedIdentity,
    pub features: ProgramFeatures,
}

impl<'a> ProgramModel<'a> {
    pub fn new(idl: &'a Idl) -> Self {
        Self {
            idl,
            identity: ResolvedIdentity::from_idl(idl),
            features: ProgramFeatures::from_idl(idl),
        }
    }
}

pub fn visit_type(ty: &IdlType, visit: &mut impl FnMut(&IdlType)) {
    visit(ty);
    match ty {
        IdlType::Option { option } => visit_type(option, visit),
        IdlType::DynVec { vec } => visit_type(&vec.items, visit),
        _ => {}
    }
}

pub fn type_has_dynamic(ty: &IdlType) -> bool {
    match ty {
        IdlType::Option { option } => type_has_dynamic(option),
        IdlType::DynString { .. } | IdlType::DynVec { .. } => true,
        _ => false,
    }
}

pub fn type_has_option(ty: &IdlType) -> bool {
    match ty {
        IdlType::Option { .. } => true,
        IdlType::DynVec { vec } => type_has_option(&vec.items),
        _ => false,
    }
}

pub fn type_has_float(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(p) => p == "f32" || p == "f64",
        IdlType::Option { option } => type_has_float(option),
        IdlType::DynVec { vec } => type_has_float(&vec.items),
        _ => false,
    }
}

pub fn type_has_public_key(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(p) => p == "pubkey",
        IdlType::Option { option } => type_has_public_key(option),
        IdlType::DynVec { vec } => type_has_public_key(&vec.items),
        _ => false,
    }
}

pub fn python_field_path(path: &str) -> String {
    path.split('.')
        .map(camel_to_snake)
        .collect::<Vec<_>>()
        .join(".")
}

pub fn go_field_path(path: &str) -> String {
    path.split('.')
        .map(|segment| {
            if segment.contains('_') {
                snake_to_pascal(segment)
            } else {
                camel_to_pascal(segment)
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::IdlMetadata};

    fn idl_with_names(name: &str, crate_name: &str) -> Idl {
        Idl {
            address: "11111111111111111111111111111111".to_string(),
            metadata: IdlMetadata {
                name: name.to_string(),
                crate_name: crate_name.to_string(),
                version: "0.1.0".to_string(),
                spec: "0.1.0".to_string(),
            },
            instructions: vec![],
            accounts: vec![],
            events: vec![],
            types: vec![],
            errors: vec![],
        }
    }

    #[test]
    fn resolved_identity_prefers_crate_name_when_present() {
        let idl = idl_with_names("multisig", "quasar-multisig");
        let identity = ResolvedIdentity::from_idl(&idl);

        assert_eq!(identity.client_name, "quasar-multisig");
        assert_eq!(identity.typescript_dir, "quasar-multisig");
        assert_eq!(identity.typescript_package, "quasar-multisig-client");
        assert_eq!(identity.python_package, "quasar-multisig");
        assert_eq!(identity.go_package, "quasar_multisig");
        assert_eq!(identity.rust_client_crate, "quasar-multisig-client");
    }

    #[test]
    fn resolved_identity_falls_back_to_program_name_when_crate_name_missing() {
        let idl = idl_with_names("vault", "");
        let identity = ResolvedIdentity::from_idl(&idl);

        assert_eq!(identity.client_name, "vault");
        assert_eq!(identity.typescript_package, "vault-client");
        assert_eq!(identity.go_package, "vault");
    }

    #[test]
    fn path_lowering_matches_generated_field_conventions() {
        assert_eq!(
            python_field_path("walletConfig.approvalThreshold"),
            "wallet_config.approval_threshold"
        );
        assert_eq!(
            go_field_path("walletConfig.approval_threshold"),
            "WalletConfig.ApprovalThreshold"
        );
    }
}
