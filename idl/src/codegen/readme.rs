//! The README that ships with a generated client.
//!
//! A generated client is meant to be consumed by someone who did not run the
//! generator, so each package says what it is, what it depends on, and how to
//! regenerate it.

use crate::{codegen::model::ProgramModel, types::Idl};

/// Which language a README is describing.
#[derive(Clone, Copy)]
pub enum ClientKind {
    Kit,
    Web3,
    Rust,
    Python,
    Go,
    C,
}

impl ClientKind {
    fn title(self) -> &'static str {
        match self {
            Self::Kit => "@solana/kit",
            Self::Web3 => "@solana/web3.js",
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::C => "C",
        }
    }

    fn usage(self, program: &str) -> String {
        match self {
            Self::Kit | Self::Web3 => format!(
                "```ts\nimport {{ {program}Client }} from \"./client.js\";\n\nconst client = new \
                 {program}Client();\nconst instruction = await \
                 client.create<Name>Instruction({{ /* inputs */ }});\n```"
            ),
            Self::Rust => format!(
                "```rust\nuse {program}_client::*;\n\nlet instruction: \
                 solana_instruction::Instruction = <Name>Instruction {{ /* inputs */ }}.into();\n```"
            ),
            Self::Python => format!(
                "```python\nfrom {program}_client import create_<name>_instruction, \
                 <Name>Input\n\ninstruction = create_<name>_instruction(<Name>Input(...))\n```"
            ),
            Self::Go => format!(
                "```go\nix := {program}.New<Name>Instruction(&{program}.<Name>Input{{ /* inputs \
                 */ }}, nil)\n```"
            ),
            Self::C => "```c\n#include \"client.h\"\n\n/* Header-only: every builder is a `static \
                        inline` function. */\n```"
                .to_owned(),
        }
    }
}

/// Render the README for one generated client.
pub fn generate_readme(idl: &Idl, model: &ProgramModel<'_>, kind: ClientKind) -> String {
    let program = model.identity.program_name.as_str();
    let mut out = format!(
        "# {program} {} client\n\nGenerated from the `{program}` IDL (version {}). Program \
         address:\n\n```\n{}\n```\n\n",
        kind.title(),
        idl.version,
        idl.address,
    );

    out.push_str("## Usage\n\n");
    out.push_str(&kind.usage(program));
    out.push_str("\n\n## What the builders resolve\n\n");
    out.push_str(
        "Every address the client can work out itself — program-derived addresses, associated \
         token accounts, well-known programs, and addresses already carried by an instruction \
         argument — is computed for you. Callers pass only what cannot be derived. Each builder \
         also accepts overrides so any resolved address can be replaced.\n\n",
    );

    out.push_str("## Regenerating\n\n");
    out.push_str(
        "This file is generated. Re-run the generator instead of editing it:\n\n```sh\nquasar \
         client <idl-path>\n```\n",
    );
    out
}

/// The `pyproject.toml` that makes a generated Python client installable.
pub fn generate_pyproject(idl: &Idl, model: &ProgramModel<'_>) -> String {
    format!(
        r#"[build-system]
requires = ["setuptools>=68"]
build-backend = "setuptools.build_meta"

[project]
name = "{package}"
version = "{version}"
description = "Generated Solana client for the {program} program."
requires-python = ">=3.10"
dependencies = ["solders>=0.21"]

[tool.setuptools]
packages = ["{package}"]

[tool.setuptools.package-data]
"{package}" = ["py.typed"]
"#,
        package = model.identity.python_package,
        program = model.identity.program_name,
        version = idl.version,
    )
}
