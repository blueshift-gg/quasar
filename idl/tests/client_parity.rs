//! Cross-target parity for the supported clients: Rust, Kit, and Web3.js.
//!
//! Six generators render one IDL, and nothing used to check that they agreed.
//! That is how the TypeScript backend drifted into emitting `undefined`
//! addresses for resolver kinds four other backends handled safely.
//!
//! [`SUPPORTED`] is the tier these tests hold to a single contract: the same
//! caller inputs, the same client-resolved accounts, the same account order on
//! the wire, the same escape hatch, and the same documentation. Python, Go, and
//! C are preview backends, checked separately and only for the properties every
//! backend must hold.

use quasar_idl::{
    codegen::{self, accounts::account_source, model::ProgramModel},
    types::*,
};

fn representative_idl() -> Idl {
    serde_json::from_str(include_str!(
        "fixtures/programs/client-conformance.idl.json"
    ))
    .expect("client conformance IDL")
}

/// Accounts a caller must supply, straight from the shared classifier: the
/// contract both targets are expected to implement.
fn expected_inputs(ix: &IdlInstruction) -> Vec<&str> {
    ix.accounts
        .iter()
        .filter(|account| {
            account_source(account)
                .expect("validated account resolver")
                .is_input()
        })
        .map(|account| account.name.as_str())
        .collect()
}

/// Accounts the client works out for itself.
fn expected_resolved(ix: &IdlInstruction) -> Vec<&str> {
    ix.accounts
        .iter()
        .filter(|account| {
            !account_source(account)
                .expect("validated account resolver")
                .is_input()
        })
        .map(|account| account.name.as_str())
        .collect()
}

fn rust_client(idl: &Idl) -> String {
    codegen::rust::generate_client(idl)
        .expect("rust client")
        .into_iter()
        .map(|(path, content)| format!("//// {path}\n{content}"))
        .collect()
}

fn pascal_of(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or(String::new(), |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// The clients quasar supports. Parity is defined over exactly this set.
const SUPPORTED: [&str; 3] = ["rust", "kit", "web3"];

/// Preview backends: generated on request, held only to the universal rules.
const PREVIEW: [&str; 3] = ["python", "go", "c"];

fn supported_clients(idl: &Idl) -> Vec<(&'static str, String)> {
    vec![
        ("rust", rust_client(idl)),
        (
            "kit",
            codegen::typescript::generate_ts_client_kit(idl).expect("kit client"),
        ),
        (
            "web3",
            codegen::typescript::generate_ts_client(idl).expect("web3 client"),
        ),
    ]
}

fn preview_clients(idl: &Idl) -> Vec<(&'static str, String)> {
    vec![
        (
            "python",
            codegen::python::generate_python_client(idl).expect("python client"),
        ),
        (
            "go",
            codegen::golang::generate_go_client(idl).expect("go client"),
        ),
        ("c", codegen::c::generate_c_client(idl).expect("c client")),
    ]
}

/// The slice of a client between two markers, or `None` when absent.
fn between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let (_, rest) = haystack.split_once(start)?;
    let (body, _) = rest.split_once(end)?;
    Some(body)
}

/// Every identifier following `prefix`, in order of appearance.
fn names_after(body: &str, prefix: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = body;
    while let Some((_, tail)) = rest.split_once(prefix) {
        let name: String = tail
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
        rest = tail;
    }
    names
}

/// The account order a client puts on the wire, per target.
fn emitted_account_order(target: &str, client: &str, ix: &IdlInstruction) -> Vec<String> {
    let pascal = pascal_of(&ix.name);
    match target {
        "rust" => {
            let body = between(
                client,
                &format!("impl From<{pascal}InstructionRaw> for Instruction"),
                "];",
            )
            .unwrap_or_else(|| panic!("rust: no meta list for `{}`", ix.name));
            let mut seen: Vec<String> = Vec::new();
            for name in names_after(body, "ix.") {
                if !seen.contains(&name) {
                    seen.push(name);
                }
            }
            seen
        }
        // Read the emitted account list only: the derivation section above it
        // also mentions accounts, as seeds.
        _ => {
            // Anchor on the signature: the plain builder *calls* the Raw one,
            // so the bare method name also matches that call site.
            let builder = between(
                client,
                &format!("accountOverrides: {pascal}InstructionAccountOverrides"),
                "\n  }",
            )
            .unwrap_or_else(|| panic!("{target}: no builder body for `{}`", ix.name));
            let list = between(builder, "accounts: [", "]")
                .or_else(|| between(builder, "keys: [", "]"))
                .unwrap_or_else(|| panic!("{target}: no account list for `{}`", ix.name));
            names_after(list, "accountOverrides.")
        }
    }
}

/// Neither target may ask the caller for an address the other one derives.
#[test]
fn rust_and_typescript_ask_for_the_same_accounts() {
    let idl = representative_idl();
    let rust = rust_client(&idl);
    let kit = codegen::typescript::generate_ts_client_kit(&idl).expect("kit client");
    let web3 = codegen::typescript::generate_ts_client(&idl).expect("web3 client");

    for ix in &idl.instructions {
        let pascal = pascal_of(&ix.name);

        let rust_struct = rust
            .split_once(&format!("pub struct {pascal}Instruction {{"))
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(body, _)| body.to_owned())
            .unwrap_or_else(|| panic!("rust builder for `{}` not found", ix.name));

        for (target, client) in [("kit", &kit), ("web3", &web3)] {
            let ts_input = client
                .split_once(&format!("export interface {pascal}InstructionInput {{"))
                .and_then(|(_, rest)| rest.split_once('}'))
                .map(|(body, _)| body.to_owned())
                .unwrap_or_default();

            for account in expected_inputs(ix) {
                // An optional account is still an input; Rust spells it
                // `Option<Address>` so `None` selects the runtime sentinel.
                let field = snake(account);
                assert!(
                    rust_struct.contains(&format!("pub {field}: Address"))
                        || rust_struct.contains(&format!("pub {field}: Option<Address>")),
                    "rust: `{}` must take `{account}` as an input",
                    ix.name
                );
                // Optional accounts are optional inputs (`name?: Address`).
                assert!(
                    ts_input.contains(&format!("{account}: Address"))
                        || ts_input.contains(&format!("{account}?: Address")),
                    "{target}: `{}` must take `{account}` as an input",
                    ix.name
                );
            }

            for account in expected_resolved(ix) {
                assert!(
                    !rust_struct.contains(&format!("pub {}: Address", snake(account))),
                    "rust: `{}` resolves `{account}` itself; it must not be an input",
                    ix.name
                );
                assert!(
                    !ts_input.contains(&format!("{account}: Address")),
                    "{target}: `{}` resolves `{account}` itself; it must not be an input",
                    ix.name
                );
            }
        }
    }
}

/// A client that infers an address must let the caller replace it, in both
/// targets, for every account — derived ones included.
#[test]
fn every_target_offers_the_same_escape_hatch() {
    let idl = representative_idl();
    let rust = rust_client(&idl);
    let python = codegen::python::generate_python_client(&idl).expect("python client");
    let go = codegen::golang::generate_go_client(&idl).expect("go client");
    let c = codegen::c::generate_c_client(&idl).expect("c client");

    for ix in &idl.instructions {
        for account in expected_resolved(ix) {
            let field = snake(account);
            let raw_struct = rust
                .split_once(&format!(
                    "pub struct {}InstructionRaw {{",
                    pascal_of(&ix.name)
                ))
                .and_then(|(_, rest)| rest.split_once('}'))
                .map(|(body, _)| body.to_owned())
                .unwrap_or_else(|| panic!("rust: no Raw builder for `{}`", ix.name));
            assert!(
                raw_struct.contains(&format!("pub {field}: Address")),
                "rust: `{account}` must be replaceable through the Raw builder"
            );
            assert!(
                python.contains(&format!("{field}: Optional[Pubkey] = None")),
                "python: `{account}` must be overridable"
            );
            assert!(
                go.contains(&format!("{} *solana.PublicKey", pascal_of(account))),
                "go: `{account}` must be overridable"
            );
            assert!(
                c.contains(&format!("Pubkey *{account}; /* optional override */")),
                "c: `{account}` must be overridable"
            );
        }
    }

    for (target, client) in [
        (
            "kit",
            codegen::typescript::generate_ts_client_kit(&idl).expect("kit client"),
        ),
        (
            "web3",
            codegen::typescript::generate_ts_client(&idl).expect("web3 client"),
        ),
    ] {
        for ix in &idl.instructions {
            for account in ix.accounts.iter().map(|account| account.name.as_str()) {
                assert!(
                    client.contains(&format!("{account}?: Address;")),
                    "{target}: `{account}` must be overridable"
                );
            }
        }
    }
}

/// Documentation is part of the client surface: an IDL that carries docs must
/// produce documented clients everywhere, not just where someone remembered.
#[test]
fn docs_reach_every_backend() {
    let mut idl = representative_idl();
    let marker = "Parity marker doc.";
    idl.instructions[0].docs = vec![marker.to_owned()];
    idl.types[0].docs = vec![marker.to_owned()];
    idl.types[0].fields[0].docs = vec![marker.to_owned()];

    let clients = [
        ("rust", rust_client(&idl)),
        (
            "kit",
            codegen::typescript::generate_ts_client_kit(&idl).expect("kit client"),
        ),
        (
            "web3",
            codegen::typescript::generate_ts_client(&idl).expect("web3 client"),
        ),
        (
            "python",
            codegen::python::generate_python_client(&idl).expect("python client"),
        ),
        (
            "go",
            codegen::golang::generate_go_client(&idl).expect("go client"),
        ),
        ("c", codegen::c::generate_c_client(&idl).expect("c client")),
    ];

    for (target, client) in clients {
        assert!(
            client.contains(marker),
            "{target}: IDL docs must reach the generated client"
        );
    }
}

/// Every supported client is a package a stranger can install and read.
#[test]
fn supported_targets_ship_installable_packages() {
    let idl = representative_idl();
    let model = ProgramModel::try_new(&idl).expect("model");

    let cargo = codegen::rust::generate_cargo_toml_for_program(&model);
    assert!(
        cargo.contains("description ="),
        "rust: manifest needs a description"
    );
    assert!(
        cargo.contains("license ="),
        "rust: manifest needs a license"
    );

    for target in [
        codegen::typescript::TsTarget::Kit,
        codegen::typescript::TsTarget::Web3js,
    ] {
        let manifest =
            codegen::typescript::generate_package_json(&idl, target).expect("package manifest");
        assert!(
            !manifest.contains("\"private\": true"),
            "typescript: a private package cannot be shared"
        );
        assert!(manifest.contains("\"types\""), "typescript: needs types");
        assert!(
            manifest.contains("\"license\""),
            "typescript: needs a license"
        );
    }
}

/// An address the client computes is bound before it is read. TypeScript used
/// to read an unassigned `Record` entry, which typed as `Address` and shipped
/// `undefined`.
#[test]
fn resolved_addresses_are_bound_before_use() {
    let idl = representative_idl();
    for (target, client) in [
        (
            "kit",
            codegen::typescript::generate_ts_client_kit(&idl).expect("kit client"),
        ),
        (
            "web3",
            codegen::typescript::generate_ts_client(&idl).expect("web3 client"),
        ),
    ] {
        for ix in &idl.instructions {
            for account in expected_resolved(ix) {
                let binding = format!("const __{account}: Address =");
                let declaration = client
                    .find(&binding)
                    .unwrap_or_else(|| panic!("{target}: `{account}` is never bound"));
                assert!(
                    !client[..declaration].contains(&format!("__{account}")),
                    "{target}: `{account}` is read before it is bound"
                );
            }
        }
    }
}

/// Account order is the wire contract: a client that reorders one account
/// produces a transaction the program rejects. All supported targets must
/// emit exactly the IDL order.
#[test]
fn supported_targets_emit_accounts_in_idl_order() {
    let idl = representative_idl();
    let clients = supported_clients(&idl);
    assert_eq!(clients.len(), SUPPORTED.len());

    for ix in &idl.instructions {
        let expected: Vec<String> = ix
            .accounts
            .iter()
            .map(|account| account.name.clone())
            .collect();

        for (target, client) in &clients {
            let emitted = emitted_account_order(target, client, ix);
            let emitted: Vec<String> = emitted
                .iter()
                .map(|name| {
                    if *target == "rust" {
                        pascal_of(name)
                    } else {
                        name.clone()
                    }
                })
                .map(|name| snake(&name))
                .collect();
            let expected_snake: Vec<String> = expected.iter().map(|name| snake(name)).collect();
            assert_eq!(
                emitted, expected_snake,
                "{target}: `{}` emits accounts out of IDL order",
                ix.name
            );
        }
    }
}

/// Preview backends are generated on request and are not held to the supported
/// contract, but they must still produce a client at all.
#[test]
fn preview_backends_still_generate() {
    let idl = representative_idl();
    let clients = preview_clients(&idl);
    assert_eq!(clients.len(), PREVIEW.len());
    for (target, client) in clients {
        assert!(!client.is_empty(), "{target}: generated nothing");
    }
}

/// Instruction data is the other half of the wire contract, and the half the
/// account tests do not touch: a codec that disagrees between targets produces
/// bytes the program misreads. Assert every supported target prefixes the same
/// discriminator and encodes the declared args in the declared order.
#[test]
fn supported_targets_encode_the_same_instruction_data() {
    let idl = representative_idl();
    let clients = supported_clients(&idl);

    for ix in &idl.instructions {
        let pascal = pascal_of(&ix.name);
        let discriminator = ix
            .discriminator
            .iter()
            .map(|byte| byte.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        for (target, client) in &clients {
            let body = match *target {
                "rust" => between(
                    client,
                    &format!("impl From<{pascal}InstructionRaw> for Instruction"),
                    "\n}",
                ),
                _ => between(
                    client,
                    &format!("accountOverrides: {pascal}InstructionAccountOverrides"),
                    "\n  }",
                ),
            }
            .unwrap_or_else(|| panic!("{target}: no builder body for `{}`", ix.name));

            assert!(
                body.contains(&format!("vec![{discriminator}]"))
                    || body.contains(&format!("from([{discriminator}]"))
                    || body.contains(&format!("from([{discriminator},")),
                "{target}: `{}` must prefix discriminator [{discriminator}]",
                ix.name
            );

            // Argument order is positional on the wire, so the encoder must
            // mention each argument exactly in the order the IDL declares.
            let mut cursor = 0usize;
            for arg in &ix.args {
                let needle = match *target {
                    "rust" => format!("ix.{}", snake(&arg.name)),
                    _ => format!("input.{}", arg.name),
                };
                let found = body[cursor..].find(&needle).unwrap_or_else(|| {
                    panic!(
                        "{target}: `{}` never encodes argument `{}`",
                        ix.name, arg.name
                    )
                });
                cursor += found + needle.len();
            }
        }
    }
}

/// Optional arguments must use the codec that matches the declared type.
/// `getOptionCodec` decodes to `{ __option }`, which no consumer following a
/// declared `T | null` can use, and the mismatch is invisible byte-wise.
#[test]
fn optional_values_use_the_nullable_codec() {
    let idl = representative_idl();
    // Ask the IDL, not the emitted text: `| null` also appears in decoder
    // return types, which have nothing to do with optional values.
    let declares_option = idl
        .instructions
        .iter()
        .flat_map(|ix| ix.args.iter().map(|arg| &arg.ty))
        .chain(
            idl.types
                .iter()
                .flat_map(|ty| ty.fields.iter().map(|f| &f.ty)),
        )
        .any(|ty| matches!(ty, IdlType::Option { .. }));
    assert!(
        declares_option,
        "fixture must declare an optional value for this test to mean anything"
    );

    for (target, client) in supported_clients(&idl) {
        if target == "rust" {
            continue;
        }
        assert!(
            client.contains("getNullableCodec"),
            "{target}: an optional value must use the codec matching `| null`"
        );
        assert!(
            !client.contains("getOptionCodec"),
            "{target}: `getOptionCodec` does not decode to the declared `| null`"
        );
    }
}
