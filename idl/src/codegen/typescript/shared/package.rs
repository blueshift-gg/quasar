use {
    super::TsTarget,
    crate::{
        codegen::model::{CodegenResult, ProgramModel},
        types::Idl,
    },
};

const SOLANA_KIT_VERSION: &str = "^7.0.0";
const SOLANA_WEB3JS_VERSION: &str = "^3.0.0";

pub fn client_dependency_version(target: TsTarget) -> &'static str {
    match target {
        TsTarget::Web3js => SOLANA_WEB3JS_VERSION,
        TsTarget::Kit => SOLANA_KIT_VERSION,
    }
}

pub fn generate_package_json(idl: &Idl, target: TsTarget) -> CodegenResult<String> {
    let model = ProgramModel::try_new(idl)?;
    let codecs_version = match target {
        TsTarget::Kit => "^7.0.0",
        TsTarget::Web3js => "^6.2.0",
    };
    let codecs_dep = if model.features.needs_codecs {
        format!("\n    \"@solana/codecs\": \"{codecs_version}\",")
    } else {
        String::new()
    };
    let (target_name, dependency, dependency_version) = match target {
        TsTarget::Kit => ("kit", "@solana/kit", client_dependency_version(target)),
        TsTarget::Web3js => ("web3", "@solana/web3.js", client_dependency_version(target)),
    };

    Ok(format!(
        r#"{{
  "name": "{package_name}-{target_name}",
  "version": "{version}",
  "private": true,
  "exports": "./client.ts",
  "dependencies": {{{codecs_dep}
    "{dependency}": "{dependency_version}"
  }}
}}
"#,
        package_name = model.identity.typescript_package,
        version = idl.version,
    ))
}

#[cfg(test)]
mod package_tests {
    use {
        super::{generate_package_json, TsTarget},
        crate::types::{Idl, IdlMetadata},
    };

    fn minimal_idl() -> Idl {
        Idl {
            spec: "quasar-idl/1.0.0".to_owned(),
            name: "vault".to_owned(),
            version: "0.1.0".to_owned(),
            address: "11111111111111111111111111111111".to_owned(),
            metadata: IdlMetadata::default(),
            docs: vec![],
            instructions: vec![],
            accounts: vec![],
            types: vec![],
            events: vec![],
            errors: vec![],
            extensions: None,
            hashes: None,
        }
    }

    #[test]
    fn stable_targets_have_independent_final_only_manifests() {
        let idl = minimal_idl();
        let kit = generate_package_json(&idl, TsTarget::Kit).unwrap();
        let web3 = generate_package_json(&idl, TsTarget::Web3js).unwrap();

        assert!(kit.contains(r#""@solana/kit": "^7.0.0""#));
        assert!(!kit.contains("@solana/web3.js"));
        assert!(web3.contains(r#""@solana/web3.js": "^3.0.0""#));
        assert!(!web3.contains("@solana/kit"));
        assert!(!kit.contains("-rc."));
        assert!(!web3.contains("-rc."));
    }
}
