use {
    super::model::ProgramModel,
    crate::types::{
        AccountFlag, Idl, IdlAccountDef, IdlArg, IdlCodec, IdlFieldDef, IdlInstruction, IdlPdaSeed,
        IdlResolver, IdlType, ScalarRepr,
    },
    quasar_schema::{
        pascal_to_camel, snake_to_pascal, to_camel_case,
        to_screaming_snake as pascal_to_screaming_snake,
    },
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
    "@solana/kit": "^6.4.0",
    "@solana/web3.js": "git+https://github.com/blueshift-gg/web3.js.git#v2"
  }}
}}
"#,
        package_name = model.identity.typescript_package,
        version = idl.version,
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
    let has_dynamic_types = idl.types.iter().any(|t| has_dynamic_field_defs(&t.fields));
    let plugin_account_count = eligible_plugin_accounts(idl).len();
    let emit_plugin = target == TsTarget::Kit && (has_instructions || plugin_account_count > 0);
    let plugin_has_accounts = emit_plugin && plugin_account_count > 0;
    let plugin_has_instructions = emit_plugin && has_instructions;

    // --- Imports ---
    match target {
        TsTarget::Web3js => {
            if has_instructions || has_dynamic_types {
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
            if plugin_has_accounts {
                kit_imports.push("type ClientWithRpc");
                kit_imports.push("type GetAccountInfoApi");
                kit_imports.push("type GetMultipleAccountsApi");
            }
            if plugin_has_instructions {
                kit_imports.push("type ClientWithPayer");
                kit_imports.push("type ClientWithTransactionPlanning");
                kit_imports.push("type ClientWithTransactionSending");
            }
            writeln!(
                out,
                "import {{ {} }} from \"@solana/kit\";",
                kit_imports.join(", ")
            )
            .expect("write to String");

            if emit_plugin {
                let mut pcc_imports: Vec<&str> = Vec::new();
                if plugin_has_accounts {
                    pcc_imports.push("addSelfFetchFunctions");
                    pcc_imports.push("type SelfFetchFunctions");
                }
                if plugin_has_instructions {
                    pcc_imports.push("addSelfPlanAndSendFunctions");
                    pcc_imports.push("type SelfPlanAndSendFunctions");
                }
                writeln!(
                    out,
                    "import {{ {} }} from \"@solana/kit/program-client-core\";",
                    pcc_imports.join(", ")
                )
                .expect("write to String");
            }
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
        let fields = &type_def.fields;
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
        let has_remaining = ix.remaining_accounts.is_some();
        let user_accs: Vec<_> = ix
            .accounts
            .iter()
            .filter(|a| matches!(a.resolver, IdlResolver::Input {}))
            .collect();

        if user_accs.is_empty() && ix.args.is_empty() && !has_remaining {
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

        if has_remaining {
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
        let fields = &type_def.fields;

        if has_dynamic_field_defs(fields) {
            emit_compact_type_codec(&mut out, name, fields, target);
        } else {
            writeln!(out, "export const {}Codec = getStructCodec([", name)
                .expect("write to String");
            for field in fields {
                writeln!(
                    out,
                    "  [\"{}\", {}],",
                    field.name,
                    ts_codec_for_field_def(field, target)
                )
                .expect("write to String");
            }
            out.push_str("]);\n\n");
        }
    }

    // === Enums ===
    out.push_str("/* Enums */\n");

    if !idl.events.is_empty() {
        out.push_str("export const ProgramEvent = {\n");
        for event in &idl.events {
            writeln!(out, "  {}: \"{}\",", event.name, event.name).expect("write to String");
        }
        out.push_str("} as const;\n\n");

        out.push_str(
            "export type ProgramEvent =\n  (typeof ProgramEvent)[keyof typeof ProgramEvent];\n\n",
        );

        out.push_str("export type DecodedEvent =\n");
        for (i, event) in idl.events.iter().enumerate() {
            let has_type = idl.types.iter().any(|t| t.name == event.name);
            if has_type {
                write!(
                    out,
                    "  | {{ type: typeof ProgramEvent.{}; data: {} }}",
                    event.name, event.name
                )
                .expect("write to String");
            } else {
                write!(out, "  | {{ type: typeof ProgramEvent.{} }}", event.name)
                    .expect("write to String");
            }
            if i < idl.events.len() - 1 {
                out.push('\n');
            }
        }
        out.push_str(";\n\n");
    }

    if !idl.instructions.is_empty() {
        out.push_str("export const ProgramInstruction = {\n");
        for ix in &idl.instructions {
            let pascal = snake_to_pascal(&ix.name);
            writeln!(out, "  {}: \"{}\",", pascal, pascal).expect("write to String");
        }
        out.push_str("} as const;\n\n");

        out.push_str(
            "export type ProgramInstruction =\n  (typeof ProgramInstruction)[keyof typeof \
             ProgramInstruction];\n\n",
        );

        out.push_str("export type DecodedInstruction =\n");
        for (i, ix) in idl.instructions.iter().enumerate() {
            let pascal = snake_to_pascal(&ix.name);
            if ix.args.is_empty() {
                write!(out, "  | {{ type: typeof ProgramInstruction.{} }}", pascal)
                    .expect("write to String");
            } else {
                write!(
                    out,
                    "  | {{ type: typeof ProgramInstruction.{}; args: {}InstructionArgs }}",
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
                let has_dyn = ix.args.iter().any(is_arg_dynamic);
                writeln!(out, "    if (matchDisc(data, {})) {{", const_name)
                    .expect("write to String");

                if !has_dyn {
                    // Fixed-only: use getStructCodec
                    out.push_str("      const argsCodec = getStructCodec([\n");
                    for arg in &ix.args {
                        writeln!(
                            out,
                            "        [\"{}\", {}],",
                            arg.name,
                            ts_codec_for_arg(arg, target)
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
                } else {
                    // Compact decode: [disc][fixed][prefixes][data]
                    emit_compact_decode(&mut out, ix, &const_name, &pascal, target);
                }
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

    if emit_plugin {
        emit_program_plugin(&mut out, idl, &model);
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
    exportable_pda_helpers: &HashMap<String, String>,
    program_name: &str,
) {
    let class_name = format!("{}Client", snake_to_pascal(program_name));
    for ix in &idl.instructions {
        let has_remaining = ix.remaining_accounts.is_some();
        out.push('\n');
        let pascal = snake_to_pascal(&ix.name);
        let arg_types = instruction_arg_types(ix);

        let mut user_accs = Vec::new();
        let mut has_non_input_accounts = false;
        for acc in &ix.accounts {
            if matches!(acc.resolver, IdlResolver::Input {}) {
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
        let input_param = if user_accs.is_empty() && ix.args.is_empty() && !has_remaining {
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
            if let IdlResolver::Const { address } = &acc.resolver {
                writeln!(
                    out,
                    "    accountsMap[\"{}\"] = new Address(\"{}\");",
                    acc.name, address
                )
                .expect("write to String");
            }
        }

        // Derive PDA accounts
        for acc in &ix.accounts {
            if let IdlResolver::Pda { seeds, .. } = &acc.resolver {
                if let Some(helper_name) = exportable_pda_helpers.get(&format!("{:?}", seeds)) {
                    let args = helper_call_args(seeds, &account_expr);
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
                        seeds,
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
        let has_dyn = ix.args.iter().any(is_arg_dynamic);
        if ix.args.is_empty() {
            writeln!(out, "    const data = Buffer.from([{}]);", disc_str)
                .expect("write to String");
        } else if !has_dyn {
            // Fixed-only: use getStructCodec
            out.push_str("    const argsCodec = getStructCodec([\n");
            for arg in &ix.args {
                writeln!(
                    out,
                    "      [\"{}\", {}],",
                    arg.name,
                    ts_codec_for_arg(arg, TsTarget::Web3js)
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
        } else {
            // Compact layout: [disc][fixed fields][all prefixes][all dynamic data]
            emit_compact_encoding(out, ix, &disc_str, TsTarget::Web3js, "Buffer.from");
        }

        // Return TransactionInstruction
        out.push_str("    return new TransactionInstruction({\n");
        writeln!(out, "      programId: {class_name}.programId,").expect("write to String");
        if !ix.accounts.is_empty() || has_remaining {
            out.push_str("      keys: [\n");
            for acc in &ix.accounts {
                let pubkey_expr = account_expr(&acc.name);
                let is_signer = matches!(acc.signer, AccountFlag::Fixed(true));
                let is_writable = matches!(acc.writable, AccountFlag::Fixed(true));
                writeln!(
                    out,
                    "        {{ pubkey: {}, isSigner: {}, isWritable: {} }},",
                    pubkey_expr, is_signer, is_writable
                )
                .expect("write to String");
            }
            if has_remaining {
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
    exportable_pda_helpers: &HashMap<String, String>,
) {
    for ix in &idl.instructions {
        let has_remaining = ix.remaining_accounts.is_some();
        out.push('\n');
        let pascal = snake_to_pascal(&ix.name);
        let arg_types = instruction_arg_types(ix);

        let mut user_accs = Vec::new();
        let mut has_non_input_accounts = false;
        for acc in &ix.accounts {
            if matches!(acc.resolver, IdlResolver::Input {}) {
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
        let ix_has_pdas = ix
            .accounts
            .iter()
            .any(|a| matches!(a.resolver, IdlResolver::Pda { .. }));

        // Method signature
        let input_param = if user_accs.is_empty() && ix.args.is_empty() && !has_remaining {
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
            if let IdlResolver::Const { address } = &acc.resolver {
                writeln!(
                    out,
                    "    accountsMap[\"{}\"] = address(\"{}\");",
                    acc.name, address
                )
                .expect("write to String");
            }
        }

        // Derive PDA accounts (async in kit)
        for acc in &ix.accounts {
            if let IdlResolver::Pda { seeds, .. } = &acc.resolver {
                if let Some(helper_name) = exportable_pda_helpers.get(&format!("{:?}", seeds)) {
                    let args = helper_call_args(seeds, &account_expr);
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
                        seeds,
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
        let has_dyn = ix.args.iter().any(is_arg_dynamic);
        if ix.args.is_empty() {
            writeln!(out, "    const data = Uint8Array.from([{}]);", disc_str)
                .expect("write to String");
        } else if !has_dyn {
            // Fixed-only: use getStructCodec
            out.push_str("    const argsCodec = getStructCodec([\n");
            for arg in &ix.args {
                writeln!(
                    out,
                    "      [\"{}\", {}],",
                    arg.name,
                    ts_codec_for_arg(arg, TsTarget::Kit)
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
        } else {
            // Compact layout: [disc][fixed fields][all prefixes][all dynamic data]
            emit_compact_encoding(out, ix, &disc_str, TsTarget::Kit, "Uint8Array.from");
        }

        // Return Instruction
        out.push_str("    return {\n");
        out.push_str("      programAddress: PROGRAM_ADDRESS,\n");
        if !ix.accounts.is_empty() || has_remaining {
            out.push_str("      accounts: [\n");
            for acc in &ix.accounts {
                let addr_expr = account_expr(&acc.name);
                let is_signer = matches!(acc.signer, AccountFlag::Fixed(true));
                let is_writable = matches!(acc.writable, AccountFlag::Fixed(true));
                let role = account_role(is_signer, is_writable);
                writeln!(out, "        {{ address: {}, role: {} }},", addr_expr, role)
                    .expect("write to String");
            }
            if has_remaining {
                out.push_str("        ...(input.remainingAccounts ?? []),\n");
            }
            out.push_str("      ],\n");
        }
        out.push_str("      data,\n");
        out.push_str("    };\n");
        out.push_str("  }\n");
    }
}

// ---------------------------------------------------------------------------
// Program plugin — @solana/kit
// ---------------------------------------------------------------------------

/// Plugin accounts: only those whose struct codec is a `Codec<T, T>` from
/// `@solana/codecs` (no dynamic fields).
fn eligible_plugin_accounts(idl: &Idl) -> Vec<&IdlAccountDef> {
    let accounts_with_dynamic_fields: HashSet<&str> = idl
        .types
        .iter()
        .filter(|t| has_dynamic_field_defs(&t.fields))
        .map(|t| t.name.as_str())
        .collect();
    idl.accounts
        .iter()
        .filter(|a| !accounts_with_dynamic_fields.contains(a.name.as_str()))
        .collect()
}

fn ix_has_input(ix: &IdlInstruction) -> bool {
    let has_accounts = ix
        .accounts
        .iter()
        .any(|a| matches!(a.resolver, IdlResolver::Input {}));
    has_accounts || !ix.args.is_empty() || ix.remaining_accounts.is_some()
}

fn emit_program_plugin(out: &mut String, idl: &Idl, model: &ProgramModel) {
    let prog_pascal = snake_to_pascal(&model.identity.program_name);
    let prog_camel = to_camel_case(&model.identity.program_name);
    let class_name = format!("{}Client", prog_pascal);
    let plugin_accounts = eligible_plugin_accounts(idl);
    let has_accounts = !plugin_accounts.is_empty();
    let has_instructions = !idl.instructions.is_empty();

    out.push_str("/* Program Plugin */\n");

    // Input Client requirements
    let mut requirements: Vec<&str> = Vec::new();
    if has_accounts {
        requirements.push("ClientWithRpc<GetAccountInfoApi & GetMultipleAccountsApi>");
    }
    if has_instructions {
        requirements.push("ClientWithPayer");
        requirements.push("ClientWithTransactionPlanning");
        requirements.push("ClientWithTransactionSending");
    }
    writeln!(
        out,
        "export type {prog_pascal}PluginRequirements = {};\n",
        requirements.join(" &\n  ")
    )
    .expect("write to String");

    // --- Factory function ---
    writeln!(out, "export function {prog_camel}Program() {{").expect("write to String");
    writeln!(out, "  const __client = new {class_name}();").expect("write to String");
    writeln!(
        out,
        "  return <T extends {prog_pascal}PluginRequirements>(client: T) => ({{"
    )
    .expect("write to String");
    out.push_str("    ...client,\n");
    writeln!(out, "    {prog_camel}: {{").expect("write to String");

    if has_accounts {
        out.push_str("      accounts: {\n");
        for account in &plugin_accounts {
            let key = pascal_to_camel(&account.name);
            writeln!(
                out,
                "        {key}: addSelfFetchFunctions(client, {}Codec),",
                account.name
            )
            .expect("write to String");
        }
        out.push_str("      },\n");
    }

    if has_instructions {
        out.push_str("      instructions: {\n");
        for ix in &idl.instructions {
            let ix_pascal = snake_to_pascal(&ix.name);
            let ix_camel = to_camel_case(&ix.name);
            if ix_has_input(ix) {
                writeln!(
                    out,
                    "        {ix_camel}: (input: {ix_pascal}InstructionInput) => \
                     addSelfPlanAndSendFunctions(client, \
                     __client.create{ix_pascal}Instruction(input)),"
                )
                .expect("write to String");
            } else {
                writeln!(
                    out,
                    "        {ix_camel}: () => addSelfPlanAndSendFunctions(client, \
                     __client.create{ix_pascal}Instruction()),"
                )
                .expect("write to String");
            }
        }
        out.push_str("      },\n");
    }

    out.push_str("    },\n");
    out.push_str("  });\n");
    out.push_str("}\n\n");
}

/// Returns `true` if any field in the slice is a dynamic field (has
/// SizePrefixed codec).
fn has_dynamic_field_defs(fields: &[IdlFieldDef]) -> bool {
    fields.iter().any(is_field_def_dynamic)
}

/// Returns `true` if a field def has a SizePrefixed or Remainder codec
/// (dynamic).
fn is_field_def_dynamic(field: &IdlFieldDef) -> bool {
    matches!(
        field.codec,
        Some(IdlCodec::SizePrefixed { .. }) | Some(IdlCodec::Remainder { .. })
    )
}

/// Returns `true` if an instruction arg is dynamic (has SizePrefixed or
/// Remainder codec).
fn is_arg_dynamic(arg: &IdlArg) -> bool {
    matches!(
        arg.codec,
        Some(IdlCodec::SizePrefixed { .. }) | Some(IdlCodec::Remainder { .. })
    )
}

/// Emit a custom codec object for a type with dynamic fields (compact layout).
///
/// Layout: `[fixed fields][all length prefixes][all tail data]`
fn emit_compact_type_codec(out: &mut String, name: &str, fields: &[IdlFieldDef], target: TsTarget) {
    let fixed_fields: Vec<_> = fields.iter().filter(|f| !is_field_def_dynamic(f)).collect();
    let dyn_fields: Vec<_> = fields.iter().filter(|f| is_field_def_dynamic(f)).collect();

    let buf_ctor = match target {
        TsTarget::Web3js => "Buffer.from",
        TsTarget::Kit => "Uint8Array.from",
    };

    writeln!(out, "export const {name}Codec = {{").expect("write to String");

    // ---- encode ----
    writeln!(out, "  encode(value: {name}): Uint8Array {{").expect("write to String");

    // Phase 1: fixed fields
    if fixed_fields.is_empty() {
        out.push_str("    const fixedBytes = new Uint8Array(0);\n");
    } else {
        out.push_str("    const fixedCodec = getStructCodec([\n");
        for f in &fixed_fields {
            writeln!(
                out,
                "      [\"{}\", {}],",
                f.name,
                ts_codec_for_field_def(f, target)
            )
            .expect("write to String");
        }
        out.push_str("    ]);\n");
        let fixed_names: Vec<String> = fixed_fields
            .iter()
            .map(|f| format!("{}: value.{}", f.name, f.name))
            .collect();
        writeln!(
            out,
            "    const fixedBytes = fixedCodec.encode({{ {} }});",
            fixed_names.join(", ")
        )
        .expect("write to String");
    }

    // Phase 2: length prefixes
    for f in &dyn_fields {
        let pfx = codec_prefix_bytes(&f.codec);
        let pfx_codec = prefix_codec(pfx);
        if is_optional_dynamic_string(&f.ty) || is_optional_dynamic_vec(&f.ty) {
            writeln!(
                out,
                "    const {name}Tag = getU8Codec().encode(value.{name} === null ? 0 : 1);",
                name = f.name
            )
            .expect("write to String");
        } else if is_string_type(&f.ty) {
            writeln!(
                out,
                "    const {name}Bytes = new TextEncoder().encode(value.{name});",
                name = f.name
            )
            .expect("write to String");
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode({name}Bytes.length);",
                name = f.name,
                codec = pfx_codec
            )
            .expect("write to String");
        } else {
            // Vec
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode(value.{name}.length);",
                name = f.name,
                codec = pfx_codec
            )
            .expect("write to String");
        }
    }

    // Phase 3: tail data
    for f in &dyn_fields {
        if is_string_type(&f.ty) {
            // Already encoded as `{name}Bytes` in phase 2
        } else if let IdlType::Vec { vec } = &f.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "    const {name}Bytes = getArrayCodec({item_codec}, {{ size: value.{name}.length \
                 }}).encode(value.{name});",
                name = f.name,
                item_codec = item_codec,
            )
            .expect("write to String");
        }
    }

    // Concatenate all phases
    let mut concat_parts = vec!["fixedBytes".to_string()];
    for f in &dyn_fields {
        concat_parts.push(format!("{}Prefix", f.name));
    }
    for f in &dyn_fields {
        concat_parts.push(format!("{}Bytes", f.name));
    }

    writeln!(
        out,
        "    return {}([{}]);",
        buf_ctor,
        concat_parts
            .iter()
            .map(|p| format!("...{}", p))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("write to String");
    out.push_str("  },\n");

    // ---- decode ----
    writeln!(out, "  decode(data: Uint8Array): {name} {{").expect("write to String");
    out.push_str("    let offset = 0;\n");

    // Phase 1: decode fixed fields
    if !fixed_fields.is_empty() {
        out.push_str("    const fixedCodec = getStructCodec([\n");
        for f in &fixed_fields {
            writeln!(
                out,
                "      [\"{}\", {}],",
                f.name,
                ts_codec_for_field_def(f, target)
            )
            .expect("write to String");
        }
        out.push_str("    ]);\n");
        out.push_str("    const fixedResult = fixedCodec.decode(data.slice(offset));\n");
        out.push_str(
            "    offset += fixedCodec.fixedSize ?? fixedCodec.encode(fixedResult).length;\n",
        );
    }

    // Phase 2: decode length prefixes
    for f in &dyn_fields {
        let pfx = codec_prefix_bytes(&f.codec);
        let pfx_codec = prefix_codec(pfx);
        writeln!(
            out,
            "    const {name}Len = {codec}.decode(data.slice(offset));",
            name = f.name,
            codec = pfx_codec
        )
        .expect("write to String");
        writeln!(out, "    offset += {};", pfx).expect("write to String");
    }

    // Phase 3: decode tail data
    for f in &dyn_fields {
        if is_string_type(&f.ty) {
            writeln!(
                out,
                "    const {name} = new TextDecoder().decode(data.slice(offset, offset + \
                 Number({name}Len)));",
                name = f.name
            )
            .expect("write to String");
            writeln!(out, "    offset += Number({}Len);", f.name).expect("write to String");
        } else if let IdlType::Vec { vec } = &f.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "    const {name}Codec = getArrayCodec({item_codec}, {{ size: Number({name}Len) \
                 }});",
                name = f.name,
                item_codec = item_codec,
            )
            .expect("write to String");
            writeln!(
                out,
                "    const {name} = {name}Codec.decode(data.slice(offset));",
                name = f.name
            )
            .expect("write to String");
            writeln!(
                out,
                "    offset += {name}Codec.encode({name}).length;",
                name = f.name
            )
            .expect("write to String");
        }
    }

    // Build return object
    let mut field_exprs = Vec::new();
    for f in &fixed_fields {
        field_exprs.push(format!("{}: fixedResult.{}", f.name, f.name));
    }
    for f in &dyn_fields {
        field_exprs.push(f.name.clone());
    }
    writeln!(out, "    return {{ {} }};", field_exprs.join(", ")).expect("write to String");
    out.push_str("  },\n");

    out.push_str("};\n\n");
}

/// Emit compact (3-phase) encoding for an instruction with dynamic fields.
///
/// Layout: `[disc][fixed fields][all length prefixes][all dynamic data]`
fn emit_compact_encoding(
    out: &mut String,
    ix: &IdlInstruction,
    disc_str: &str,
    target: TsTarget,
    buf_ctor: &str,
) {
    let fixed_args: Vec<_> = ix.args.iter().filter(|a| !is_arg_dynamic(a)).collect();
    let dyn_args: Vec<_> = ix.args.iter().filter(|a| is_arg_dynamic(a)).collect();

    out.push_str("    const disc = new Uint8Array([");
    out.push_str(disc_str);
    out.push_str("]);\n");

    // Phase 1: fixed fields
    if fixed_args.is_empty() {
        out.push_str("    const fixedBytes = new Uint8Array(0);\n");
    } else {
        out.push_str("    const fixedCodec = getStructCodec([\n");
        for arg in &fixed_args {
            writeln!(
                out,
                "      [\"{}\", {}],",
                arg.name,
                ts_codec_for_arg(arg, target)
            )
            .expect("write to String");
        }
        out.push_str("    ]);\n");
        let fixed_names: Vec<String> = fixed_args
            .iter()
            .map(|a| format!("{}: input.{}", a.name, a.name))
            .collect();
        writeln!(
            out,
            "    const fixedBytes = fixedCodec.encode({{ {} }});",
            fixed_names.join(", ")
        )
        .expect("write to String");
    }

    // Phase 2: length prefixes
    for arg in &dyn_args {
        let pfx = codec_prefix_bytes(&arg.codec);
        let pfx_codec = prefix_codec(pfx);
        if is_string_type(&arg.ty) {
            writeln!(
                out,
                "    const {name}Bytes = new TextEncoder().encode(input.{name});",
                name = arg.name
            )
            .expect("write to String");
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode({name}Bytes.length);",
                name = arg.name,
                codec = pfx_codec
            )
            .expect("write to String");
        } else {
            // Vec
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode(input.{name}.length);",
                name = arg.name,
                codec = pfx_codec
            )
            .expect("write to String");
        }
    }

    // Phase 3: tail data
    for arg in &dyn_args {
        if is_string_type(&arg.ty) {
            // Already encoded as `{name}Bytes` in phase 2
        } else if let IdlType::Vec { vec } = &arg.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "    const {name}Bytes = getArrayCodec({item_codec}, {{ size: input.{name}.length \
                 }}).encode(input.{name});",
                name = arg.name,
                item_codec = item_codec,
            )
            .expect("write to String");
        }
    }

    // Concatenate all phases
    let mut concat_parts = vec!["disc".to_string(), "fixedBytes".to_string()];
    for arg in &dyn_args {
        concat_parts.push(format!("{}Prefix", arg.name));
    }
    for arg in &dyn_args {
        concat_parts.push(format!("{}Bytes", arg.name));
    }

    writeln!(
        out,
        "    const data = {}([{}]);",
        buf_ctor,
        concat_parts
            .iter()
            .map(|p| format!("...{}", p))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("write to String");
}

/// Emit compact (3-phase) decoding for an instruction with dynamic fields.
fn emit_compact_decode(
    out: &mut String,
    ix: &IdlInstruction,
    const_name: &str,
    pascal: &str,
    target: TsTarget,
) {
    let fixed_args: Vec<_> = ix.args.iter().filter(|a| !is_arg_dynamic(a)).collect();
    let dyn_args: Vec<_> = ix.args.iter().filter(|a| is_arg_dynamic(a)).collect();

    writeln!(out, "      let offset = {}.length;", const_name).expect("write to String");

    // Phase 1: decode fixed fields
    if !fixed_args.is_empty() {
        out.push_str("      const fixedCodec = getStructCodec([\n");
        for arg in &fixed_args {
            writeln!(
                out,
                "        [\"{}\", {}],",
                arg.name,
                ts_codec_for_arg(arg, target)
            )
            .expect("write to String");
        }
        out.push_str("      ]);\n");
        out.push_str("      const fixedResult = fixedCodec.decode(data.slice(offset));\n");
        out.push_str(
            "      offset += fixedCodec.fixedSize ?? fixedCodec.encode(fixedResult).length;\n",
        );
    }

    // Phase 2: decode length prefixes
    for arg in &dyn_args {
        let pfx = codec_prefix_bytes(&arg.codec);
        let pfx_codec = prefix_codec(pfx);
        writeln!(
            out,
            "      const {name}Len = {codec}.decode(data.slice(offset));",
            name = arg.name,
            codec = pfx_codec
        )
        .expect("write to String");
        writeln!(out, "      offset += {};", pfx).expect("write to String");
    }

    // Phase 3: decode tail data
    for arg in &dyn_args {
        if is_string_type(&arg.ty) {
            writeln!(
                out,
                "      const {name} = new TextDecoder().decode(data.slice(offset, offset + \
                 Number({name}Len)));",
                name = arg.name
            )
            .expect("write to String");
            writeln!(out, "      offset += Number({}Len);", arg.name).expect("write to String");
        } else if let IdlType::Vec { vec } = &arg.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "      const {name}Codec = getArrayCodec({item_codec}, {{ size: Number({name}Len) \
                 }});",
                name = arg.name,
                item_codec = item_codec,
            )
            .expect("write to String");
            writeln!(
                out,
                "      const {name} = {name}Codec.decode(data.slice(offset));",
                name = arg.name
            )
            .expect("write to String");
            writeln!(
                out,
                "      offset += {name}Codec.encode({name}).length;",
                name = arg.name
            )
            .expect("write to String");
        }
    }

    // Build the return object
    let mut field_exprs = Vec::new();
    for arg in &fixed_args {
        field_exprs.push(format!("{}: fixedResult.{}", arg.name, arg.name));
    }
    for arg in &dyn_args {
        field_exprs.push(arg.name.clone());
    }
    writeln!(
        out,
        "      return {{ type: ProgramInstruction.{}, args: {{ {} }} }};",
        pascal,
        field_exprs.join(", ")
    )
    .expect("write to String");
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
    seeds: Vec<IdlPdaSeed>,
    params: Vec<PdaParam>,
}

fn collect_pdas(idl: &Idl) -> Vec<PdaInfo> {
    let mut pdas: Vec<PdaInfo> = Vec::new();
    let mut seen_seeds: HashSet<Vec<u8>> = HashSet::new();
    let mut used_helper_names: HashMap<String, usize> = HashMap::new();

    for ix in &idl.instructions {
        let arg_types = instruction_arg_types(ix);
        for account in &ix.accounts {
            let seeds = match &account.resolver {
                IdlResolver::Pda { seeds, .. } => seeds,
                _ => continue,
            };
            if seeds.is_empty() || !pda_is_exportable(seeds, &arg_types) {
                continue;
            }

            // Use a serialized form for dedup since IdlPdaSeed doesn't impl Hash
            let seed_key = format!("{:?}", seeds).into_bytes();
            if !seen_seeds.insert(seed_key) {
                continue;
            }

            let mut params: Vec<PdaParam> = Vec::new();
            for seed in seeds {
                match seed {
                    IdlPdaSeed::Const { .. } => {}
                    IdlPdaSeed::Account { path } => {
                        if !params.iter().any(|param| param.name == *path) {
                            params.push(PdaParam {
                                name: path.clone(),
                                ty: PdaParamType::Account,
                            });
                        }
                    }
                    IdlPdaSeed::Arg { path, ty, .. } => {
                        if params.iter().any(|param| param.name == *path) {
                            continue;
                        }
                        params.push(PdaParam {
                            name: path.clone(),
                            ty: PdaParamType::Arg(ty.clone()),
                        });
                    }
                }
            }

            pdas.push(PdaInfo {
                helper_name: unique_pda_helper_name(&account.name, &mut used_helper_names),
                seeds: seeds.clone(),
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

fn pda_helper_lookup(pdas: &[PdaInfo]) -> HashMap<String, String> {
    pdas.iter()
        .map(|pda| (format!("{:?}", pda.seeds), pda.helper_name.clone()))
        .collect()
}

fn helper_call_args(seeds: &[IdlPdaSeed], account_expr: &impl Fn(&str) -> String) -> String {
    let mut args = Vec::new();
    let mut seen = HashSet::new();

    for seed in seeds {
        let (name, expr) = match seed {
            IdlPdaSeed::Const { .. } => continue,
            IdlPdaSeed::Account { path } => (path.as_str(), account_expr(path)),
            IdlPdaSeed::Arg { path, .. } => (path.as_str(), format!("input.{path}")),
        };

        if seen.insert(name.to_string()) {
            args.push(expr);
        }
    }

    args.join(", ")
}

fn write_ts_pda_seed_lines(
    out: &mut String,
    seeds: &[IdlPdaSeed],
    target: TsTarget,
    arg_types: &HashMap<String, IdlType>,
) {
    for seed in seeds {
        match seed {
            IdlPdaSeed::Const { value } => write_byte_array(out, value),
            IdlPdaSeed::Account { path } => match target {
                TsTarget::Web3js => {
                    writeln!(out, "      {}.toBytes(),", path).expect("write to String");
                }
                TsTarget::Kit => {
                    writeln!(out, "      getAddressCodec().encode({}),", path)
                        .expect("write to String");
                }
            },
            IdlPdaSeed::Arg { path, .. } => {
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
    seeds: &[IdlPdaSeed],
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
    seeds: &[IdlPdaSeed],
    target: TsTarget,
    arg_types: &HashMap<String, IdlType>,
    account_expr: &impl Fn(&str) -> String,
) {
    for seed in seeds {
        match seed {
            IdlPdaSeed::Const { value } => write_byte_array(out, value),
            IdlPdaSeed::Account { path } => match target {
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
            IdlPdaSeed::Arg { path, .. } => {
                let expr = arg_types
                    .get(path)
                    .map(|ty| ts_pda_arg_seed_expr(&format!("input.{path}"), ty, target))
                    .unwrap_or_else(|| format!("input.{path}"));
                writeln!(out, "        {},", expr).expect("write to String");
            }
        }
    }
}

fn instruction_arg_types(ix: &IdlInstruction) -> HashMap<String, IdlType> {
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

fn pda_is_exportable(seeds: &[IdlPdaSeed], arg_types: &HashMap<String, IdlType>) -> bool {
    seeds.iter().all(|seed| match seed {
        IdlPdaSeed::Const { .. } => true,
        IdlPdaSeed::Account { path } => is_identifier(path),
        IdlPdaSeed::Arg { path, .. } => is_identifier(path) && arg_types.contains_key(path),
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

/// Check if a type represents a string (for dynamic codec purposes).
fn is_string_type(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "string")
}

fn is_u8_type(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "u8")
}

fn ts_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "u8" | "u16" | "u32" | "i8" | "i16" | "i32" => "number".to_string(),
            "u64" | "u128" | "i64" | "i128" => "bigint".to_string(),
            "bool" => "boolean".to_string(),
            "pubkey" => "Address".to_string(),
            "string" => "string".to_string(),
            "bytes" => "Uint8Array".to_string(),
            other if other.starts_with('[') => "Uint8Array".to_string(),
            other => other.to_string(),
        },
        IdlType::Option { option } => format!("{} | null", ts_type(option)),
        IdlType::Defined { defined } => defined.name.clone(),
        IdlType::Vec { vec } => format!("Array<{}>", ts_type(vec)),
        IdlType::Array { array } => {
            let (item, _size) = array;
            if is_u8_type(item) {
                "Uint8Array".to_string()
            } else {
                format!("Array<{}>", ts_type(item))
            }
        }
        IdlType::Generic { generic } => generic.clone(),
    }
}

/// Generate codec expression for a type (no codec metadata).
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
            "string" => "addCodecSizePrefix(getUtf8Codec(), getU32Codec())".to_string(),
            other if other.starts_with('[') => {
                let size = super::parse_fixed_array_size(other).unwrap_or(1);
                format!("fixCodecSize(getBytesCodec(), {})", size)
            }
            other => format!("/* unknown: {} */", other),
        },
        IdlType::Option { option } => format!("getOptionCodec({})", ts_codec(option, target)),
        IdlType::Defined { defined } => format!("{}Codec", defined.name),
        IdlType::Vec { vec } => {
            format!(
                "getArrayCodec({}, {{ size: getU32Codec() }})",
                ts_codec(vec, target)
            )
        }
        IdlType::Array { array } => {
            let (item, size) = array;
            if is_u8_type(item) {
                format!("fixCodecSize(getBytesCodec(), {})", size)
            } else {
                format!(
                    "getArrayCodec({}, {{ size: {} }})",
                    ts_codec(item, target),
                    size
                )
            }
        }
        IdlType::Generic { generic } => format!("/* generic: {} */", generic),
    }
}

/// Generate codec expression for a field def, using its codec metadata if
/// present.
fn ts_codec_for_field_def(field: &IdlFieldDef, target: TsTarget) -> String {
    match &field.codec {
        Some(IdlCodec::SizePrefixed { prefix, item, .. }) => {
            let pfx_bytes = scalar_repr_bytes(prefix);
            let pfx_codec = prefix_codec(pfx_bytes);
            if is_string_type(&field.ty) {
                format!("addCodecSizePrefix(getUtf8Codec(), {})", pfx_codec)
            } else if let IdlType::Vec { vec } = &field.ty {
                let item_codec = match item {
                    Some(codec_item) => ts_codec(&codec_item.ty, target),
                    None => ts_codec(vec, target),
                };
                format!("getArrayCodec({}, {{ size: {} }})", item_codec, pfx_codec)
            } else {
                ts_codec(&field.ty, target)
            }
        }
        _ => ts_codec(&field.ty, target),
    }
}

/// Generate codec expression for an instruction arg, using its codec metadata
/// if present.
fn ts_codec_for_arg(arg: &IdlArg, target: TsTarget) -> String {
    match &arg.codec {
        Some(IdlCodec::SizePrefixed { prefix, item, .. }) => {
            let pfx_bytes = scalar_repr_bytes(prefix);
            let pfx_codec = prefix_codec(pfx_bytes);
            if is_string_type(&arg.ty) {
                format!("addCodecSizePrefix(getUtf8Codec(), {})", pfx_codec)
            } else if let IdlType::Vec { vec } = &arg.ty {
                let item_codec = match item {
                    Some(codec_item) => ts_codec(&codec_item.ty, target),
                    None => ts_codec(vec, target),
                };
                format!("getArrayCodec({}, {{ size: {} }})", item_codec, pfx_codec)
            } else {
                ts_codec(&arg.ty, target)
            }
        }
        _ => ts_codec(&arg.ty, target),
    }
}

/// Extract prefix byte width from a codec's ScalarRepr.
fn scalar_repr_bytes(repr: &ScalarRepr) -> usize {
    match repr.ty.as_str() {
        "u8" => 1,
        "u16" => 2,
        "u32" => 4,
        "u64" => 8,
        _ => 4,
    }
}

/// Extract prefix byte width from an optional codec on a field/arg.
fn codec_prefix_bytes(codec: &Option<IdlCodec>) -> usize {
    match codec {
        Some(IdlCodec::SizePrefixed { prefix, .. }) => scalar_repr_bytes(prefix),
        _ => 4, // default u32
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

/// Map prefix byte width to the integer type name used for codec tracking.
fn prefix_int_type(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        _ => "u64",
    }
}

fn collect_used_codecs(idl: &Idl) -> HashSet<String> {
    let mut used = HashSet::new();

    fn visit_type_into(ty: &IdlType, used: &mut HashSet<String>) {
        match ty {
            IdlType::Primitive(p) => {
                used.insert(p.clone());
            }
            IdlType::Option { option } => {
                used.insert("option".to_string());
                visit_type_into(option, used);
            }
            IdlType::Vec { vec } => {
                used.insert("dynVec".to_string());
                visit_type_into(vec, used);
            }
            IdlType::Array { array } => {
                if is_u8_type(&array.0) {
                    used.insert("fixedBytes".to_string());
                } else {
                    used.insert("fixedArray".to_string());
                    visit_type_into(&array.0, used);
                }
            }
            IdlType::Defined { .. } => {}
            IdlType::Generic { .. } => {}
        }
    }

    fn visit_codec_into(codec: &Option<IdlCodec>, used: &mut HashSet<String>) {
        if let Some(IdlCodec::SizePrefixed { prefix, .. }) = codec {
            let pfx_bytes = scalar_repr_bytes(prefix);
            used.insert(prefix_int_type(pfx_bytes).to_string());
            used.insert("dynString".to_string());
        }
    }

    for type_def in &idl.types {
        for field in &type_def.fields {
            visit_type_into(&field.ty, &mut used);
            visit_codec_into(&field.codec, &mut used);
        }
    }
    for ix in &idl.instructions {
        for arg in &ix.args {
            visit_type_into(&arg.ty, &mut used);
            visit_codec_into(&arg.codec, &mut used);
        }
    }

    used
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

#[cfg(test)]
mod plugin_tests {
    use super::*;

    /// Minimal IDL to test plugin features
    fn fixture_idl() -> Idl {
        const RAW: &str = r#"{
            "spec": "quasar-idl/1.0.0",
            "name": "demo_program",
            "version": "0.1.0",
            "address": "11111111111111111111111111111111",
            "metadata": {},
            "instructions": [
                {
                    "name": "set_value",
                    "discriminator": [0],
                    "accounts": [
                        { "name": "config", "writable": true, "signer": false, "resolver": { "kind": "input" } }
                    ],
                    "args": [
                        { "name": "value", "type": "u64" }
                    ]
                },
                {
                    "name": "tick",
                    "discriminator": [1],
                    "accounts": [],
                    "args": []
                }
            ],
            "accounts": [
                { "name": "ConfigAccount", "discriminator": [9, 9] }
            ],
            "events": [],
            "types": [
                {
                    "name": "ConfigAccount",
                    "kind": "struct",
                    "fields": [
                        { "name": "value", "type": "u64" }
                    ]
                }
            ],
            "errors": [],
            "constants": []
        }"#;
        serde_json::from_str(RAW).expect("fixture IDL parses")
    }

    #[test]
    fn kit_plugin_emits_factory_types_and_singleton() {
        let out = generate_ts_client_kit(&fixture_idl());

        // The plugin pulls helpers from program-client-core
        assert!(out.contains("from \"@solana/kit/program-client-core\""));

        // Exported requirements type
        assert!(out.contains("export type DemoProgramPluginRequirements = "));

        // Plugin function
        assert!(out.contains("export function demoProgramProgram() {"));
        assert!(out.contains("const __client = new DemoProgramClient();"));
        assert!(out.contains("configAccount: addSelfFetchFunctions(client, ConfigAccountCodec),"));
        assert!(out.contains(
            "setValue: (input: SetValueInstructionInput) => addSelfPlanAndSendFunctions(client, \
             __client.createSetValueInstruction(input)),"
        ));
        assert!(out.contains(
            "tick: () => addSelfPlanAndSendFunctions(client, __client.createTickInstruction()),"
        ));
    }

    #[test]
    fn web3_target_does_not_emit_plugin() {
        let out = generate_ts_client(&fixture_idl());
        assert!(
            !out.contains("/* Program Plugin */"),
            "web3.js target must not emit plugin"
        );
        assert!(!out.contains("@solana/kit/program-client-core"));
        assert!(!out.contains("addSelfFetchFunctions"));
    }

    #[test]
    fn plugin_skipped_when_no_instructions_and_no_accounts() {
        let mut idl = fixture_idl();
        idl.instructions.clear();
        idl.accounts.clear();
        let out = generate_ts_client_kit(&idl);
        assert!(!out.contains("/* Program Plugin */"));
        assert!(!out.contains("@solana/kit/program-client-core"));
    }

    #[test]
    fn dynamic_codec_accounts_are_filtered_from_plugin() {
        // Add a dynamic-field account to the IDL
        let mut idl = fixture_idl();
        idl.accounts.push(
            serde_json::from_str(r#"{ "name": "DynamicAccount", "discriminator": [1, 1] }"#)
                .expect("dynamic account parses"),
        );
        idl.types.push(
            serde_json::from_str(
                r#"{
                    "name": "DynamicAccount",
                    "kind": "struct",
                    "fields": [
                        {
                            "name": "name",
                            "type": "string",
                            "codec": {
                                "kind": "sizePrefixed",
                                "prefix": { "type": "u32", "endian": "le" },
                                "storage": "tail"
                            }
                        }
                    ]
                }"#,
            )
            .expect("dynamic type parses"),
        );

        let out = generate_ts_client_kit(&idl);
        // Struct-codec account included in plugin
        assert!(out.contains("configAccount: addSelfFetchFunctions(client, ConfigAccountCodec),"));
        // Dynamic-codec account is filtered out
        assert!(!out.contains("dynamicAccount: addSelfFetchFunctions"));
    }
}
