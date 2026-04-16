use {
    super::model::ProgramModel,
    crate::types::{Idl, IdlSeed, IdlType},
    quasar_schema::{snake_to_pascal, to_screaming_snake as pascal_to_screaming_snake},
    std::{
        collections::{HashMap, HashSet},
        fmt::Write,
    },
};

/// Target flavor for TypeScript client generation.
#[derive(Clone, Copy, PartialEq)]
pub enum TsTarget {
    Web3js,
    Kit,
}

/// Generate a TypeScript client targeting @solana/web3.js.
pub fn generate_ts_client(idl: &Idl) -> String {
    generate_ts(idl, TsTarget::Web3js)
}

/// Generate a TypeScript client targeting @solana/kit.
pub fn generate_ts_client_kit(idl: &Idl) -> String {
    generate_ts(idl, TsTarget::Kit)
}

pub fn generate_package_json(idl: &Idl) -> String {
    let model = ProgramModel::new(idl);
    let codecs_dep = if model.features.needs_codecs {
        "\n    \"@solana/codecs\": \"^6.2.0\","
    } else {
        ""
    };

    format!(
        r#"{{
  "name": "{package_name}",
  "version": "{version}",
  "private": true,
  "exports": {{
    "./web3.js": "./web3.ts",
    "./kit": "./kit.ts"
  }},
  "dependencies": {{{codecs_dep}
    "@solana/kit": "^6.0.0",
    "@solana/web3.js": "git+https://github.com/blueshift-gg/web3.js.git#v2"
  }}
}}
"#,
        package_name = model.identity.typescript_package,
        version = idl.metadata.version,
    )
}

fn generate_ts(idl: &Idl, target: TsTarget) -> String {
    let model = ProgramModel::new(idl);
    let mut out = String::new();
    let pdas = collect_pdas(idl);
    let exportable_pda_helpers = pda_helper_lookup(&pdas);

    // --- Collect which codecs are actually used ---
    let used = collect_used_codecs(idl);
    let has_dyn_string = used.contains("dynString");
    let has_dyn_vec = used.contains("dynVec");
    let has_instructions = model.features.has_instructions;
    let has_public_key = used.contains("pubkey");
    let has_pdas = model.features.has_pdas;
    let has_pda_account_seeds = model.features.has_pda_account_seeds;

    // --- Imports ---
    match target {
        TsTarget::Web3js => {
            if has_instructions {
                out.push_str("import { Buffer } from \"buffer\";\n");
            }
            out.push_str(
                "import { PublicKey, TransactionInstruction } from \"@solana/web3.js\";\n",
            );
        }
        TsTarget::Kit => {
            let mut kit_imports: Vec<&str> = vec!["type Address", "address"];
            if has_instructions {
                kit_imports.push("AccountRole");
                kit_imports.push("type Instruction");
            }
            if has_pdas {
                kit_imports.push("getProgramDerivedAddress");
            }
            if has_pda_account_seeds || has_public_key {
                kit_imports.push("getAddressCodec");
            }
            writeln!(
                out,
                "import {{ {} }} from \"@solana/kit\";",
                kit_imports.join(", ")
            )
            .expect("write to String");
        }
    }

    // Build codec imports list
    let has_struct_codec = model.features.needs_codecs;
    let mut codec_imports: Vec<&str> = Vec::new();
    if has_struct_codec {
        codec_imports.push("getStructCodec");
    }
    let integer_codec_map = [
        ("u8", "getU8Codec"),
        ("u16", "getU16Codec"),
        ("u32", "getU32Codec"),
        ("u64", "getU64Codec"),
        ("u128", "getU128Codec"),
        ("i8", "getI8Codec"),
        ("i16", "getI16Codec"),
        ("i32", "getI32Codec"),
        ("i64", "getI64Codec"),
        ("i128", "getI128Codec"),
    ];
    for (used_type, codec) in integer_codec_map {
        if used.contains(used_type) {
            codec_imports.push(codec);
        }
    }
    if used.contains("bool") {
        codec_imports.push("getBooleanCodec");
    }
    if used.contains("option") {
        codec_imports.push("getOptionCodec");
    }
    // PublicKey codec imports: web3.js uses custom helper, kit uses getAddressCodec
    // from @solana/kit
    if target == TsTarget::Web3js && has_public_key {
        codec_imports.extend_from_slice(&["getBytesCodec", "fixCodecSize", "transformCodec"]);
    }

    let has_fixed_array = used.iter().any(|u| u.starts_with('['));
    if has_fixed_array {
        codec_imports.extend_from_slice(&["fixCodecSize", "getBytesCodec"]);
    }

    if has_dyn_string {
        codec_imports.extend_from_slice(&["addCodecSizePrefix", "getUtf8Codec"]);
    }

    if has_dyn_vec {
        codec_imports.push("getArrayCodec");
    }

    codec_imports.sort();
    codec_imports.dedup();

    if !codec_imports.is_empty() {
        writeln!(
            out,
            "import {{ {} }} from \"@solana/codecs\";",
            codec_imports.join(", ")
        )
        .expect("write to String");
    }
    out.push('\n');

    if target == TsTarget::Web3js {
        out.push_str("type Address = PublicKey;\n");
        out.push_str("const Address = PublicKey;\n\n");
    }

    // --- PublicKey codec helper (web3.js only) ---
    if target == TsTarget::Web3js && has_public_key {
        out.push_str(PUBLIC_KEY_CODEC_HELPER);
        out.push('\n');
    }

    // --- Discriminator match helper ---
    let has_decoders =
        !idl.accounts.is_empty() || !idl.events.is_empty() || !idl.instructions.is_empty();
    if has_decoders {
        out.push_str(MATCH_DISC_HELPER);
        out.push('\n');
    }

    // === Constants ===
    out.push_str("/* Constants */\n");
    match target {
        TsTarget::Web3js => {
            // Program address is a public readonly on the client class
        }
        TsTarget::Kit => {
            writeln!(
                out,
                "export const PROGRAM_ADDRESS = address(\"{}\");",
                idl.address
            )
            .expect("write to String");
        }
    }

    // Account discriminators
    for account in &idl.accounts {
        let const_name = pascal_to_screaming_snake(&account.name);
        let disc_str = super::format_disc_array(&account.discriminator);
        writeln!(
            out,
            "export const {}_DISCRIMINATOR = new Uint8Array({});",
            const_name, disc_str
        )
        .expect("write to String");
    }

    // Event discriminators
    for event in &idl.events {
        let const_name = pascal_to_screaming_snake(&event.name);
        let disc_str = super::format_disc_array(&event.discriminator);
        writeln!(
            out,
            "export const {}_DISCRIMINATOR = new Uint8Array({});",
            const_name, disc_str
        )
        .expect("write to String");
    }

    // Instruction discriminators
    for ix in &idl.instructions {
        let pascal = snake_to_pascal(&ix.name);
        let const_name = pascal_to_screaming_snake(&pascal);
        let disc_str = super::format_disc_array(&ix.discriminator);
        writeln!(
            out,
            "export const {}_INSTRUCTION_DISCRIMINATOR = new Uint8Array({});",
            const_name, disc_str
        )
        .expect("write to String");
    }

    out.push('\n');

    // === Interfaces ===
    out.push_str("/* Interfaces */\n");

    // Type interfaces
    for type_def in &idl.types {
        let name = &type_def.name;
        let fields = &type_def.ty.fields;
        writeln!(out, "export interface {} {{", name).expect("write to String");
        for field in fields {
            writeln!(out, "  {}: {};", field.name, ts_type(&field.ty)).expect("write to String");
        }
        out.push_str("}\n\n");
    }

    // Instruction args interfaces
    for ix in &idl.instructions {
        if ix.args.is_empty() {
            continue;
        }
        let pascal = snake_to_pascal(&ix.name);
        writeln!(out, "export interface {}InstructionArgs {{", pascal).expect("write to String");
        for arg in &ix.args {
            writeln!(out, "  {}: {};", arg.name, ts_type(&arg.ty)).expect("write to String");
        }
        out.push_str("}\n\n");
    }

    // Instruction input interfaces
    for ix in &idl.instructions {
        let user_accs: Vec<_> = ix
            .accounts
            .iter()
            .filter(|a| a.pda.is_none() && a.address.is_none())
            .collect();

        if user_accs.is_empty() && ix.args.is_empty() && !ix.has_remaining {
            continue;
        }

        let pascal = snake_to_pascal(&ix.name);

        writeln!(out, "export interface {pascal}InstructionInput {{").expect("write to String");

        if !user_accs.is_empty() {
            for acc in &user_accs {
                writeln!(out, "  {}: Address;", acc.name).expect("write to String");
            }
        }
        if !ix.args.is_empty() {
            for arg in &ix.args {
                writeln!(out, "  {}: {};", arg.name, ts_type(&arg.ty)).expect("write to String");
            }
        }

        if ix.has_remaining {
            match target {
                TsTarget::Kit => {
                    out.push_str(
                        "  remainingAccounts?: Array<{ address: Address; role: AccountRole }>;\n",
                    );
                }
                TsTarget::Web3js => {
                    out.push_str(
                        "  remainingAccounts?: Array<{ pubkey: Address; isSigner: boolean; \
                         isWritable: boolean }>;\n",
                    );
                }
            }
        }

        out.push_str("}\n\n");
    }

    // === Codecs ===
    if !idl.types.is_empty() {
        out.push_str("/* Codecs */\n");
    }
    for type_def in &idl.types {
        let name = &type_def.name;
        let fields = &type_def.ty.fields;
        writeln!(out, "export const {}Codec = getStructCodec([", name).expect("write to String");
        for field in fields {
            writeln!(
                out,
                "  [\"{}\", {}],",
                field.name,
                ts_codec(&field.ty, target)
            )
            .expect("write to String");
        }
        out.push_str("]);\n\n");
    }

    // === Enums ===
    out.push_str("/* Enums */\n");

    if !idl.events.is_empty() {
        out.push_str("export enum ProgramEvent {\n");
        for event in &idl.events {
            writeln!(out, "  {} = \"{}\",", event.name, event.name).expect("write to String");
        }
        out.push_str("}\n\n");

        out.push_str("export type DecodedEvent =\n");
        for (i, event) in idl.events.iter().enumerate() {
            let has_type = idl.types.iter().any(|t| t.name == event.name);
            if has_type {
                write!(
                    out,
                    "  | {{ type: ProgramEvent.{}; data: {} }}",
                    event.name, event.name
                )
                .expect("write to String");
            } else {
                write!(out, "  | {{ type: ProgramEvent.{} }}", event.name)
                    .expect("write to String");
            }
            if i < idl.events.len() - 1 {
                out.push('\n');
            }
        }
        out.push_str(";\n\n");
    }

    if !idl.instructions.is_empty() {
        out.push_str("export enum ProgramInstruction {\n");
        for ix in &idl.instructions {
            let pascal = snake_to_pascal(&ix.name);
            writeln!(out, "  {} = \"{}\",", pascal, pascal).expect("write to String");
        }
        out.push_str("}\n\n");

        out.push_str("export type DecodedInstruction =\n");
        for (i, ix) in idl.instructions.iter().enumerate() {
            let pascal = snake_to_pascal(&ix.name);
            if ix.args.is_empty() {
                write!(out, "  | {{ type: ProgramInstruction.{} }}", pascal)
                    .expect("write to String");
            } else {
                write!(
                    out,
                    "  | {{ type: ProgramInstruction.{}; args: {}InstructionArgs }}",
                    pascal, pascal
                )
                .expect("write to String");
            }
            if i < idl.instructions.len() - 1 {
                out.push('\n');
            }
        }
        out.push_str(";\n\n");
    }

    // === Client class ===
    out.push_str("/* Client */\n");
    let class_name = format!("{}Client", snake_to_pascal(&model.identity.program_name));
    writeln!(out, "export class {} {{", class_name).expect("write to String");

    if target == TsTarget::Web3js {
        writeln!(
            out,
            "  static readonly programId = new Address(\"{}\");",
            idl.address
        )
        .expect("write to String");
    }

    // --- Account decoders ---
    for account in &idl.accounts {
        let name = &account.name;
        let const_name = pascal_to_screaming_snake(name);
        out.push('\n');
        writeln!(out, "  decode{}(data: Uint8Array): {} {{", name, name).expect("write to String");
        writeln!(
            out,
            "    if (!matchDisc(data, {}_DISCRIMINATOR)) throw new Error(\"Invalid {} \
             discriminator\");",
            const_name, name
        )
        .expect("write to String");
        writeln!(
            out,
            "    return {}Codec.decode(data.slice({}_DISCRIMINATOR.length));",
            name, const_name
        )
        .expect("write to String");
        out.push_str("  }\n");
    }

    // --- Event decoder ---
    if !idl.events.is_empty() {
        out.push('\n');
        out.push_str("  decodeEvent(data: Uint8Array): DecodedEvent | null {\n");
        for event in &idl.events {
            let has_type = idl.types.iter().any(|t| t.name == event.name);
            let const_name = format!("{}_DISCRIMINATOR", pascal_to_screaming_snake(&event.name));
            writeln!(out, "    if (matchDisc(data, {}))", const_name).expect("write to String");
            if has_type {
                writeln!(
                    out,
                    "      return {{ type: ProgramEvent.{0}, data: \
                     {0}Codec.decode(data.slice({1}.length)) }};",
                    event.name, const_name
                )
                .expect("write to String");
            } else {
                writeln!(out, "      return {{ type: ProgramEvent.{} }};", event.name)
                    .expect("write to String");
            }
        }
        out.push_str("    return null;\n");
        out.push_str("  }\n");
    }

    // --- Instruction decoder ---
    if !idl.instructions.is_empty() {
        out.push('\n');
        out.push_str("  decodeInstruction(data: Uint8Array): DecodedInstruction | null {\n");
        for ix in &idl.instructions {
            let pascal = snake_to_pascal(&ix.name);
            let const_name = format!(
                "{}_INSTRUCTION_DISCRIMINATOR",
                pascal_to_screaming_snake(&pascal)
            );
            if ix.args.is_empty() {
                writeln!(out, "    if (matchDisc(data, {}))", const_name).expect("write to String");
                writeln!(
                    out,
                    "      return {{ type: ProgramInstruction.{} }};",
                    pascal
                )
                .expect("write to String");
            } else {
                writeln!(out, "    if (matchDisc(data, {})) {{", const_name)
                    .expect("write to String");
                out.push_str("      const argsCodec = getStructCodec([\n");
                for arg in &ix.args {
                    writeln!(
                        out,
                        "        [\"{}\", {}],",
                        arg.name,
                        ts_codec(&arg.ty, target)
                    )
                    .expect("write to String");
                }
                out.push_str("      ]);\n");
                writeln!(
                    out,
                    "      return {{ type: ProgramInstruction.{}, args: \
                     argsCodec.decode(data.slice({}.length)) }};",
                    pascal, const_name
                )
                .expect("write to String");
                out.push_str("    }\n");
            }
        }
        out.push_str("    return null;\n");
        out.push_str("  }\n");
    }

    // --- Instruction builders (target-specific) ---
    match target {
        TsTarget::Web3js => generate_instruction_builders_web3js(
            &mut out,
            idl,
            &exportable_pda_helpers,
            &model.identity.program_name,
        ),
        TsTarget::Kit => generate_instruction_builders_kit(&mut out, idl, &exportable_pda_helpers),
    }

    out.push_str("}\n\n");

    if !pdas.is_empty() {
        emit_pda_helpers(&mut out, &pdas, target, &model.identity.program_name);
    }

    // === Errors ===
    if !idl.errors.is_empty() {
        out.push_str("/* Errors */\n");
        out.push_str(
            "export const PROGRAM_ERRORS: Record<number, { name: string; msg?: string }> = {\n",
        );
        for err in &idl.errors {
            match &err.msg {
                Some(msg) => {
                    writeln!(
                        out,
                        "  {}: {{ name: \"{}\", msg: \"{}\" }},",
                        err.code, err.name, msg
                    )
                    .expect("write to String");
                }
                None => {
                    writeln!(out, "  {}: {{ name: \"{}\" }},", err.code, err.name)
                        .expect("write to String");
                }
            }
        }
        out.push_str("};\n\n");
    }

    out
}

// ---------------------------------------------------------------------------
// Instruction builders — @solana/web3.js
// ---------------------------------------------------------------------------

fn generate_instruction_builders_web3js(
    out: &mut String,
    idl: &Idl,
    exportable_pda_helpers: &HashMap<Vec<IdlSeed>, String>,
    program_name: &str,
) {
    let class_name = format!("{}Client", snake_to_pascal(program_name));
    for ix in &idl.instructions {
        out.push('\n');
        let pascal = snake_to_pascal(&ix.name);
        let arg_types = instruction_arg_types(ix);

        let mut user_accs = Vec::new();
        let mut has_non_input_accounts = false;
        for acc in &ix.accounts {
            if acc.pda.is_none() && acc.address.is_none() {
                user_accs.push(acc);
            } else {
                has_non_input_accounts = true;
            }
        }

        let input_account_names: HashSet<&str> =
            user_accs.iter().map(|a| a.name.as_str()).collect();

        let account_expr = |name: &str| {
            if input_account_names.contains(name) {
                format!("input.{name}")
            } else {
                format!("accountsMap[\"{}\"]", name)
            }
        };

        // Method signature
        let input_param = if user_accs.is_empty() && ix.args.is_empty() && !ix.has_remaining {
            String::new()
        } else {
            format!("input: {pascal}InstructionInput")
        };
        writeln!(
            out,
            "  create{pascal}Instruction({input_param}): TransactionInstruction {{"
        )
        .expect("write to String");

        if has_non_input_accounts {
            out.push_str("    const accountsMap: Record<string, Address> = {};\n");
        }

        // Derive fixed-address accounts
        for acc in &ix.accounts {
            if let Some(addr) = &acc.address {
                writeln!(
                    out,
                    "    accountsMap[\"{}\"] = new Address(\"{}\");",
                    acc.name, addr
                )
                .expect("write to String");
            }
        }

        // Derive PDA accounts
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                if let Some(helper_name) = exportable_pda_helpers.get(&pda.seeds) {
                    let args = helper_call_args(&pda.seeds, &account_expr);
                    writeln!(
                        out,
                        "    accountsMap[\"{}\"] = {}({});",
                        acc.name, helper_name, args
                    )
                    .expect("write to String");
                } else {
                    emit_inline_pda_derivation(
                        out,
                        &acc.name,
                        &pda.seeds,
                        TsTarget::Web3js,
                        &arg_types,
                        &account_expr,
                        Some(&format!("{class_name}.programId")),
                    );
                }
            }
        }

        // Encode instruction data
        let disc_str = super::format_disc_decimal(&ix.discriminator);
        if ix.args.is_empty() {
            writeln!(out, "    const data = Buffer.from([{}]);", disc_str)
                .expect("write to String");
        } else {
            out.push_str("    const argsCodec = getStructCodec([\n");
            for arg in &ix.args {
                writeln!(
                    out,
                    "      [\"{}\", {}],",
                    arg.name,
                    ts_codec(&arg.ty, TsTarget::Web3js)
                )
                .expect("write to String");
            }
            out.push_str("    ]);\n");
            let arg_names: Vec<String> = ix
                .args
                .iter()
                .map(|a| format!("{}: input.{}", a.name, a.name))
                .collect();
            writeln!(
                out,
                "    const data = Buffer.from([{}, ...argsCodec.encode({{ {} }})]);",
                disc_str,
                arg_names.join(", ")
            )
            .expect("write to String");
        }

        // Return TransactionInstruction
        out.push_str("    return new TransactionInstruction({\n");
        writeln!(out, "      programId: {class_name}.programId,").expect("write to String");
        if !ix.accounts.is_empty() || ix.has_remaining {
            out.push_str("      keys: [\n");
            for acc in &ix.accounts {
                let pubkey_expr = account_expr(&acc.name);
                writeln!(
                    out,
                    "        {{ pubkey: {}, isSigner: {}, isWritable: {} }},",
                    pubkey_expr, acc.signer, acc.writable
                )
                .expect("write to String");
            }
            if ix.has_remaining {
                out.push_str("        ...(input.remainingAccounts ?? []),\n");
            }
            out.push_str("      ],\n");
        }
        out.push_str("      data,\n");
        out.push_str("    });\n");
        out.push_str("  }\n");
    }
}

// ---------------------------------------------------------------------------
// Instruction builders — @solana/kit
// ---------------------------------------------------------------------------

fn generate_instruction_builders_kit(
    out: &mut String,
    idl: &Idl,
    exportable_pda_helpers: &HashMap<Vec<IdlSeed>, String>,
) {
    for ix in &idl.instructions {
        out.push('\n');
        let pascal = snake_to_pascal(&ix.name);
        let arg_types = instruction_arg_types(ix);

        let mut user_accs = Vec::new();
        let mut has_non_input_accounts = false;
        for acc in &ix.accounts {
            if acc.pda.is_none() && acc.address.is_none() {
                user_accs.push(acc);
            } else {
                has_non_input_accounts = true;
            }
        }

        let input_account_names: HashSet<&str> =
            user_accs.iter().map(|a| a.name.as_str()).collect();

        let account_expr = |name: &str| {
            if input_account_names.contains(name) {
                format!("input.{name}")
            } else {
                format!("accountsMap[\"{}\"]", name)
            }
        };

        // Check if this instruction has any PDAs (requires async)
        let ix_has_pdas = ix.accounts.iter().any(|a| a.pda.is_some());

        // Method signature
        let input_param = if user_accs.is_empty() && ix.args.is_empty() && !ix.has_remaining {
            String::new()
        } else {
            format!("input: {pascal}InstructionInput")
        };
        let return_type = if ix_has_pdas {
            "Promise<Instruction>"
        } else {
            "Instruction"
        };
        let async_kw = if ix_has_pdas { "async " } else { "" };
        writeln!(
            out,
            "  {async_kw}create{pascal}Instruction({input_param}): {return_type} {{"
        )
        .expect("write to String");

        if has_non_input_accounts {
            out.push_str("    const accountsMap: Record<string, Address> = {};\n");
        }

        // Derive fixed-address accounts
        for acc in &ix.accounts {
            if let Some(addr) = &acc.address {
                writeln!(
                    out,
                    "    accountsMap[\"{}\"] = address(\"{}\");",
                    acc.name, addr
                )
                .expect("write to String");
            }
        }

        // Derive PDA accounts (async in kit)
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                if let Some(helper_name) = exportable_pda_helpers.get(&pda.seeds) {
                    let args = helper_call_args(&pda.seeds, &account_expr);
                    writeln!(
                        out,
                        "    accountsMap[\"{}\"] = await {}({});",
                        acc.name, helper_name, args
                    )
                    .expect("write to String");
                } else {
                    emit_inline_pda_derivation(
                        out,
                        &acc.name,
                        &pda.seeds,
                        TsTarget::Kit,
                        &arg_types,
                        &account_expr,
                        None,
                    );
                }
            }
        }

        // Encode instruction data
        let disc_str = super::format_disc_decimal(&ix.discriminator);
        if ix.args.is_empty() {
            writeln!(out, "    const data = Uint8Array.from([{}]);", disc_str)
                .expect("write to String");
        } else {
            out.push_str("    const argsCodec = getStructCodec([\n");
            for arg in &ix.args {
                writeln!(
                    out,
                    "      [\"{}\", {}],",
                    arg.name,
                    ts_codec(&arg.ty, TsTarget::Kit)
                )
                .expect("write to String");
            }
            out.push_str("    ]);\n");
            let arg_names: Vec<String> = ix
                .args
                .iter()
                .map(|a| format!("{}: input.{}", a.name, a.name))
                .collect();
            writeln!(
                out,
                "    const data = Uint8Array.from([{}, ...argsCodec.encode({{ {} }})]);",
                disc_str,
                arg_names.join(", ")
            )
            .expect("write to String");
        }

        // Return Instruction
        out.push_str("    return {\n");
        out.push_str("      programAddress: PROGRAM_ADDRESS,\n");
        if !ix.accounts.is_empty() || ix.has_remaining {
            out.push_str("      accounts: [\n");
            for acc in &ix.accounts {
                let addr_expr = account_expr(&acc.name);
                let role = account_role(acc.signer, acc.writable);
                writeln!(out, "        {{ address: {}, role: {} }},", addr_expr, role)
                    .expect("write to String");
            }
            if ix.has_remaining {
                out.push_str("        ...(input.remainingAccounts ?? []),\n");
            }
            out.push_str("      ],\n");
        }
        out.push_str("      data,\n");
        out.push_str("    };\n");
        out.push_str("  }\n");
    }
}

fn account_role(signer: bool, writable: bool) -> &'static str {
    match (signer, writable) {
        (true, true) => "AccountRole.WRITABLE_SIGNER",
        (true, false) => "AccountRole.READONLY_SIGNER",
        (false, true) => "AccountRole.WRITABLE",
        (false, false) => "AccountRole.READONLY",
    }
}

#[derive(Clone)]
struct PdaParam {
    name: String,
    ty: PdaParamType,
}

#[derive(Clone)]
enum PdaParamType {
    Account,
    Arg(IdlType),
}

/// A collected PDA with its field name, seeds, and helper signature params.
struct PdaInfo {
    helper_name: String,
    seeds: Vec<IdlSeed>,
    params: Vec<PdaParam>,
}

fn collect_pdas(idl: &Idl) -> Vec<PdaInfo> {
    let mut pdas: Vec<PdaInfo> = Vec::new();
    let mut seen_seeds: HashSet<Vec<IdlSeed>> = HashSet::new();
    let mut used_helper_names: HashMap<String, usize> = HashMap::new();

    for ix in &idl.instructions {
        let arg_types = instruction_arg_types(ix);
        for account in &ix.accounts {
            let Some(pda) = &account.pda else {
                continue;
            };
            if pda.seeds.is_empty()
                || !pda_is_exportable(&pda.seeds, &arg_types)
                || !seen_seeds.insert(pda.seeds.clone())
            {
                continue;
            }

            let mut params: Vec<PdaParam> = Vec::new();
            for seed in &pda.seeds {
                match seed {
                    IdlSeed::Const { .. } => {}
                    IdlSeed::Account { path } => {
                        if !params.iter().any(|param| param.name == *path) {
                            params.push(PdaParam {
                                name: path.clone(),
                                ty: PdaParamType::Account,
                            });
                        }
                    }
                    IdlSeed::Arg { path } => {
                        if params.iter().any(|param| param.name == *path) {
                            continue;
                        }

                        let arg = arg_types.get(path).unwrap_or_else(|| {
                            panic!("missing PDA arg seed definition for {path}")
                        });
                        params.push(PdaParam {
                            name: path.clone(),
                            ty: PdaParamType::Arg((*arg).clone()),
                        });
                    }
                }
            }

            pdas.push(PdaInfo {
                helper_name: unique_pda_helper_name(&account.name, &mut used_helper_names),
                seeds: pda.seeds.clone(),
                params,
            });
        }
    }

    pdas
}

fn emit_pda_helpers(out: &mut String, pdas: &[PdaInfo], target: TsTarget, program_name: &str) {
    out.push_str("/* PDA Helpers */\n");

    for pda in pdas {
        let arg_types = pda_arg_types(pda);
        let params = pda
            .params
            .iter()
            .map(|param| match &param.ty {
                PdaParamType::Account => format!("{}: Address", param.name),
                PdaParamType::Arg(ty) => format!("{}: {}", param.name, ts_type(ty)),
            })
            .collect::<Vec<_>>()
            .join(", ");

        match target {
            TsTarget::Web3js => {
                writeln!(
                    out,
                    "export function {}({}): Address {{",
                    pda.helper_name, params
                )
                .expect("write to String");
                out.push_str("  return Address.findProgramAddressSync(\n");
                out.push_str("    [\n");
                write_ts_pda_seed_lines(out, &pda.seeds, target, &arg_types);
                writeln!(
                    out,
                    "    ],\n    {}Client.programId,\n  )[0];",
                    snake_to_pascal(program_name)
                )
                .expect("write to String");
            }
            TsTarget::Kit => {
                writeln!(
                    out,
                    "export async function {}({}): Promise<Address> {{",
                    pda.helper_name, params
                )
                .expect("write to String");
                out.push_str("  return (await getProgramDerivedAddress({\n");
                out.push_str("    programAddress: PROGRAM_ADDRESS,\n");
                out.push_str("    seeds: [\n");
                write_ts_pda_seed_lines(out, &pda.seeds, target, &arg_types);
                out.push_str("    ],\n");
                out.push_str("  }))[0];\n");
            }
        }
        out.push_str("}\n\n");
    }
}

fn ts_pda_helper_name(field_name: &str) -> String {
    format!("find{}Address", snake_to_pascal(field_name))
}

fn unique_pda_helper_name(
    field_name: &str,
    used_helper_names: &mut HashMap<String, usize>,
) -> String {
    let base = ts_pda_helper_name(field_name);
    match used_helper_names.entry(base.clone()) {
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(1);
            base
        }
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            let suffix = *entry.get() + 1;
            entry.insert(suffix);
            format!("{base}{suffix}")
        }
    }
}

fn pda_helper_lookup(pdas: &[PdaInfo]) -> HashMap<Vec<IdlSeed>, String> {
    pdas.iter()
        .map(|pda| (pda.seeds.clone(), pda.helper_name.clone()))
        .collect()
}

fn helper_call_args(seeds: &[IdlSeed], account_expr: &impl Fn(&str) -> String) -> String {
    let mut args = Vec::new();
    let mut seen = HashSet::new();

    for seed in seeds {
        let (name, expr) = match seed {
            IdlSeed::Const { .. } => continue,
            IdlSeed::Account { path } => (path.as_str(), account_expr(path)),
            IdlSeed::Arg { path } => (path.as_str(), format!("input.{path}")),
        };

        if seen.insert(name.to_string()) {
            args.push(expr);
        }
    }

    args.join(", ")
}

fn write_ts_pda_seed_lines(
    out: &mut String,
    seeds: &[IdlSeed],
    target: TsTarget,
    arg_types: &HashMap<String, IdlType>,
) {
    for seed in seeds {
        match seed {
            IdlSeed::Const { value } => write_byte_array(out, value),
            IdlSeed::Account { path } => match target {
                TsTarget::Web3js => {
                    writeln!(out, "      {}.toBytes(),", path).expect("write to String");
                }
                TsTarget::Kit => {
                    writeln!(out, "      getAddressCodec().encode({}),", path)
                        .expect("write to String");
                }
            },
            IdlSeed::Arg { path } => {
                let expr = arg_types
                    .get(path)
                    .map(|ty| ts_pda_arg_seed_expr(path, ty, target))
                    .unwrap_or_else(|| path.clone());
                writeln!(out, "      {},", expr).expect("write to String");
            }
        }
    }
}

fn emit_inline_pda_derivation(
    out: &mut String,
    account_name: &str,
    seeds: &[IdlSeed],
    target: TsTarget,
    arg_types: &HashMap<String, IdlType>,
    account_expr: &impl Fn(&str) -> String,
    web3_program_expr: Option<&str>,
) {
    match target {
        TsTarget::Web3js => {
            writeln!(
                out,
                "    accountsMap[\"{}\"] = Address.findProgramAddressSync(",
                account_name
            )
            .expect("write to String");
            out.push_str("      [\n");
            write_inline_pda_seed_lines(out, seeds, target, arg_types, account_expr);
            writeln!(
                out,
                "      ],\n      {},\n    )[0];",
                web3_program_expr.expect("web3 PDA derivation requires program id expression")
            )
            .expect("write to String");
        }
        TsTarget::Kit => {
            writeln!(
                out,
                "    accountsMap[\"{}\"] = (await getProgramDerivedAddress({{",
                account_name
            )
            .expect("write to String");
            out.push_str("      programAddress: PROGRAM_ADDRESS,\n");
            out.push_str("      seeds: [\n");
            write_inline_pda_seed_lines(out, seeds, target, arg_types, account_expr);
            out.push_str("      ],\n");
            out.push_str("    }))[0];\n");
        }
    }
}

fn write_inline_pda_seed_lines(
    out: &mut String,
    seeds: &[IdlSeed],
    target: TsTarget,
    arg_types: &HashMap<String, IdlType>,
    account_expr: &impl Fn(&str) -> String,
) {
    for seed in seeds {
        match seed {
            IdlSeed::Const { value } => write_byte_array(out, value),
            IdlSeed::Account { path } => match target {
                TsTarget::Web3js => {
                    writeln!(out, "        {}.toBytes(),", account_expr(path))
                        .expect("write to String");
                }
                TsTarget::Kit => {
                    writeln!(
                        out,
                        "        getAddressCodec().encode({}),",
                        account_expr(path)
                    )
                    .expect("write to String");
                }
            },
            IdlSeed::Arg { path } => {
                let expr = arg_types
                    .get(path)
                    .map(|ty| ts_pda_arg_seed_expr(&format!("input.{path}"), ty, target))
                    .unwrap_or_else(|| format!("input.{path}"));
                writeln!(out, "        {},", expr).expect("write to String");
            }
        }
    }
}

fn instruction_arg_types(ix: &crate::types::IdlInstruction) -> HashMap<String, IdlType> {
    ix.args
        .iter()
        .map(|arg| (arg.name.clone(), arg.ty.clone()))
        .collect()
}

fn pda_arg_types(pda: &PdaInfo) -> HashMap<String, IdlType> {
    pda.params
        .iter()
        .filter_map(|param| match &param.ty {
            PdaParamType::Arg(ty) => Some((param.name.clone(), ty.clone())),
            PdaParamType::Account => None,
        })
        .collect()
}

fn pda_is_exportable(seeds: &[IdlSeed], arg_types: &HashMap<String, IdlType>) -> bool {
    seeds.iter().all(|seed| match seed {
        IdlSeed::Const { .. } => true,
        IdlSeed::Account { path } => is_identifier(path),
        IdlSeed::Arg { path } => is_identifier(path) && arg_types.contains_key(path),
    })
}

fn is_identifier(path: &str) -> bool {
    let mut chars = path.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn ts_pda_arg_seed_expr(expr: &str, ty: &IdlType, target: TsTarget) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "pubkey" => match target {
                TsTarget::Web3js => format!("{expr}.toBytes()"),
                TsTarget::Kit => format!("getAddressCodec().encode({expr})"),
            },
            "u8" => format!("getU8Codec().encode({expr})"),
            "u16" => format!("getU16Codec().encode({expr})"),
            "u32" => format!("getU32Codec().encode({expr})"),
            "u64" => format!("getU64Codec().encode({expr})"),
            "u128" => format!("getU128Codec().encode({expr})"),
            "i8" => format!("getI8Codec().encode({expr})"),
            "i16" => format!("getI16Codec().encode({expr})"),
            "i32" => format!("getI32Codec().encode({expr})"),
            "i64" => format!("getI64Codec().encode({expr})"),
            "i128" => format!("getI128Codec().encode({expr})"),
            "bool" => format!("getBooleanCodec().encode({expr})"),
            other if other.starts_with('[') => expr.to_string(),
            _ => expr.to_string(),
        },
        _ => expr.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn ts_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "u8" | "u16" | "u32" | "i8" | "i16" | "i32" => "number".to_string(),
            "u64" | "u128" | "i64" | "i128" => "bigint".to_string(),
            "bool" => "boolean".to_string(),
            "pubkey" => "Address".to_string(),
            other if other.starts_with('[') => "Uint8Array".to_string(),
            other => other.to_string(),
        },
        IdlType::Option { option } => format!("{} | null", ts_type(option)),
        IdlType::Defined { defined } => defined.clone(),
        IdlType::DynString { .. } => "string".to_string(),
        IdlType::DynVec { vec } => format!("Array<{}>", ts_type(&vec.items)),
    }
}

fn ts_codec(ty: &IdlType, target: TsTarget) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "u8" => "getU8Codec()".to_string(),
            "u16" => "getU16Codec()".to_string(),
            "u32" => "getU32Codec()".to_string(),
            "u64" => "getU64Codec()".to_string(),
            "u128" => "getU128Codec()".to_string(),
            "i8" => "getI8Codec()".to_string(),
            "i16" => "getI16Codec()".to_string(),
            "i32" => "getI32Codec()".to_string(),
            "i64" => "getI64Codec()".to_string(),
            "i128" => "getI128Codec()".to_string(),
            "bool" => "getBooleanCodec()".to_string(),
            "pubkey" => match target {
                TsTarget::Web3js => "getPublicKeyCodec()".to_string(),
                TsTarget::Kit => "getAddressCodec()".to_string(),
            },
            other if other.starts_with('[') => {
                let size = super::parse_fixed_array_size(other).unwrap_or(1);
                format!("fixCodecSize(getBytesCodec(), {})", size)
            }
            other => format!("/* unknown: {} */", other),
        },
        IdlType::Option { option } => format!("getOptionCodec({})", ts_codec(option, target)),
        IdlType::Defined { defined } => format!("{}Codec", defined),
        IdlType::DynString { string } => {
            format!(
                "addCodecSizePrefix(getUtf8Codec(), {})",
                prefix_codec(string.prefix_bytes)
            )
        }
        IdlType::DynVec { vec } => {
            format!(
                "getArrayCodec({}, {{ size: {} }})",
                ts_codec(&vec.items, target),
                prefix_codec(vec.prefix_bytes)
            )
        }
    }
}

/// Map prefix byte width to the integer type name used for codec tracking.
fn prefix_int_type(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        _ => "u64",
    }
}

/// Map prefix byte width to the corresponding TS codec expression.
fn prefix_codec(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "getU8Codec()",
        2 => "getU16Codec()",
        4 => "getU32Codec()",
        _ => "getU64Codec()",
    }
}

fn collect_used_codecs(idl: &Idl) -> HashSet<String> {
    let mut used = HashSet::new();

    let mut visit = |ty: &IdlType| match ty {
        IdlType::Primitive(p) => {
            used.insert(p.clone());
        }
        IdlType::Option { .. } => {
            used.insert("option".to_string());
        }
        IdlType::Defined { .. } => {}
        IdlType::DynString { string } => {
            used.insert("dynString".to_string());
            used.insert(prefix_int_type(string.prefix_bytes).to_string());
        }
        IdlType::DynVec { vec } => {
            used.insert("dynVec".to_string());
            used.insert(prefix_int_type(vec.prefix_bytes).to_string());
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

    used
}

fn visit_type(ty: &IdlType, visit: &mut impl FnMut(&IdlType)) {
    visit(ty);
    match ty {
        IdlType::Option { option } => visit_type(option, visit),
        IdlType::DynVec { vec } => visit_type(&vec.items, visit),
        _ => {}
    }
}

/// Write a `new Uint8Array([...])` seed line directly to the output.
fn write_byte_array(out: &mut String, value: &[u8]) {
    out.push_str("        new Uint8Array([");
    for (i, b) in value.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write!(out, "{}", b).expect("write to String");
    }
    out.push_str("]),\n");
}

const PUBLIC_KEY_CODEC_HELPER: &str = r#"function getPublicKeyCodec() {
  return transformCodec(
    fixCodecSize(getBytesCodec(), 32),
    (value: Address) => value.toBytes(),
    bytes => new Address(bytes),
  );
}
"#;

const MATCH_DISC_HELPER: &str = r#"function matchDisc(data: Uint8Array, disc: Uint8Array): boolean {
  if (data.length < disc.length) return false;
  for (let i = 0; i < disc.length; i++) {
    if (data[i] !== disc[i]) return false;
  }
  return true;
}
"#;
