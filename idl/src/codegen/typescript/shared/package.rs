use {
    super::TsTarget,
    crate::{
        codegen::model::{CodegenResult, ProgramModel},
        types::Idl,
    },
};

const SOLANA_KIT_VERSION: &str = "^7.0.0";
// web3.js v3 has shipped only release candidates; a bare `^3.0.0` matches no
// published version, so the range must include the prereleases until 3.0.0
// finals.
const SOLANA_WEB3JS_VERSION: &str = "^3.0.0-rc.2";

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

    // Publishable by default: `private` made every generated client
    // unshareable, and without `types`/`files` a consumer got no editor types
    // and an unbounded tarball. The package ships TypeScript source, so
    // `types` and the default export both point at it.
    Ok(format!(
        r#"{{
  "name": "{package_name}-{target_name}",
  "version": "{version}",
  "description": "Generated Solana client for the {program_name} program.",
  "license": "Apache-2.0 OR MIT",
  "type": "module",
  "types": "./client.ts",
  "exports": {{
    ".": {{
      "types": "./client.ts",
      "default": "./client.ts"
    }}
  }},
  "files": [
    "client.ts",
    "README.md"
  ],
  "sideEffects": false,
  "dependencies": {{{codecs_dep}
    "{dependency}": "{dependency_version}"
  }}
}}
"#,
        package_name = model.identity.typescript_package,
        program_name = model.identity.program_name,
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
    fn stable_targets_have_independent_resolvable_manifests() {
        let idl = minimal_idl();
        let kit = generate_package_json(&idl, TsTarget::Kit).unwrap();
        let web3 = generate_package_json(&idl, TsTarget::Web3js).unwrap();

        assert!(kit.contains(r#""@solana/kit": "^7.0.0""#));
        assert!(!kit.contains("@solana/web3.js"));
        assert!(!kit.contains("-rc."));
        // web3.js v3 has published only release candidates, so its range must
        // include them to resolve at all; kit stays final-only. Drop the
        // exception once web3.js 3.0.0 finals ship.
        assert!(web3.contains(r#""@solana/web3.js": "^3.0.0-rc.2""#));
        assert!(!web3.contains("@solana/kit"));
    }
}
