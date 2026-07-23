mod codec;
mod kit;
mod package;
mod pda;
mod web3;

pub use package::{client_dependency_version, generate_package_json};

use {codec::*, pda::*};

use {
    super::super::model::{resolved_account_order, CodegenResult, ProgramModel},
    crate::codegen::naming::{
        snake_to_pascal, to_camel_case, to_screaming_snake as pascal_to_screaming_snake,
    },
    crate::types::{AccountFlag, Idl, IdlAccountDef, IdlInstruction, IdlPdaProgram, IdlResolver},
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

#[derive(Clone, Copy)]
struct InlinePdaTarget<'a> {
    target: TsTarget,
    program_expr: &'a str,
}

/// Generate a TypeScript client targeting @solana/web3.js.
pub fn generate_ts_client(idl: &Idl) -> CodegenResult<String> {
    generate_ts(idl, TsTarget::Web3js)
}

/// Generate a TypeScript client targeting @solana/kit.
pub fn generate_ts_client_kit(idl: &Idl) -> CodegenResult<String> {
    generate_ts(idl, TsTarget::Kit)
}

fn generate_ts(idl: &Idl, target: TsTarget) -> CodegenResult<String> {
    let model = ProgramModel::try_new(idl)?;
    let mut out = String::new();
    let pdas = collect_pdas(idl);
    let exportable_pda_helpers = pda_helper_lookup(&pdas);

    let used = collect_used_codecs(idl);
    let has_dyn_string = used.contains("dynString");
    let has_dyn_vec = used.contains("dynVec");
    let has_instructions = model.features.has_instructions;
    let has_public_key = used.contains("pubkey");
    let has_pdas = model.features.has_pdas;
    let has_pda_account_seeds = model.features.has_pda_account_seeds;
    let has_pda_account_field_seeds = has_account_field_pda_seeds(idl);
    let plugin_accounts = kit::eligible_plugin_accounts(idl);
    let plugin_instructions = kit::eligible_plugin_instructions(idl);
    let emit_plugin =
        target == TsTarget::Kit && (!plugin_accounts.is_empty() || !plugin_instructions.is_empty());

    match target {
        TsTarget::Web3js => {
            out.push_str("import { Address, TransactionInstruction } from \"@solana/web3.js\";\n");
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
            if !plugin_accounts.is_empty() {
                kit_imports.push("type ClientWithRpc");
                kit_imports.push("type GetAccountInfoApi");
                kit_imports.push("type GetMultipleAccountsApi");
            }
            if !plugin_instructions.is_empty() {
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
                let mut core_imports: Vec<&str> = Vec::new();
                if !plugin_accounts.is_empty() {
                    core_imports.push("addSelfFetchFunctions");
                }
                if !plugin_instructions.is_empty() {
                    core_imports.push("addSelfPlanAndSendFunctions");
                }
                writeln!(
                    out,
                    "import {{ {} }} from \"@solana/kit/program-client-core\";",
                    core_imports.join(", ")
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
    // For Web3.js v3, a custom codec is needed to handle its Address type
    if target == TsTarget::Web3js && has_public_key {
        codec_imports.extend_from_slice(&["getBytesCodec", "fixCodecSize", "transformCodec"]);
    }

    let has_fixed_bytes = used.contains("fixedBytes");
    if has_fixed_bytes {
        codec_imports.extend_from_slice(&["fixCodecSize", "getBytesCodec"]);
    }

    if has_dyn_string {
        codec_imports.extend_from_slice(&["addCodecSizePrefix", "getUtf8Codec"]);
    }

    if has_dyn_vec || used.contains("fixedArray") {
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

    if has_pda_account_field_seeds {
        out.push_str("export interface AccountDataResolver {\n");
        out.push_str("  getAccountData(address: Address): Promise<Uint8Array | null>;\n");
        out.push_str("}\n\n");
    }

    if target == TsTarget::Web3js && has_public_key {
        out.push_str(WEB3JS_ADDRESS_CODEC_HELPER);
        out.push('\n');
    }

    let has_decoders =
        !idl.accounts.is_empty() || !idl.events.is_empty() || !idl.instructions.is_empty();
    if has_decoders {
        out.push_str(MATCH_DISC_HELPER);
        out.push_str(TOTAL_DECODE_HELPERS);
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
        let disc_str = crate::codegen::format_disc_array(&account.discriminator);
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
        let disc_str = crate::codegen::format_disc_array(&event.discriminator);
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
        let disc_str = crate::codegen::format_disc_array(&ix.discriminator);
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
            .filter(|a| a.optional || matches!(a.resolver, IdlResolver::Input {}))
            .collect();

        if user_accs.is_empty() && ix.args.is_empty() && !has_remaining {
            continue;
        }

        let pascal = snake_to_pascal(&ix.name);

        writeln!(out, "export interface {pascal}InstructionInput {{").expect("write to String");

        if !user_accs.is_empty() {
            for acc in &user_accs {
                // Optional accounts are optional inputs; an omitted one is
                // encoded as the program address sentinel.
                let opt = if acc.optional { "?" } else { "" };
                writeln!(out, "  {}{}: Address;", acc.name, opt).expect("write to String");
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

    // Deliberate account overrides for adversarial tests and custom routing.
    for ix in &idl.instructions {
        if ix.accounts.is_empty() {
            continue;
        }
        let pascal = snake_to_pascal(&ix.name);
        writeln!(
            out,
            "export interface {pascal}InstructionAccountOverrides {{"
        )
        .expect("write to String");
        for account in &ix.accounts {
            writeln!(out, "  {}?: Address;", account.name).expect("write to String");
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
            writeln!(out, "const {}StructCodec = getStructCodec([", name).expect("write to String");
            for field in fields {
                writeln!(
                    out,
                    "  [\"{}\", {}],",
                    field.name,
                    ts_codec_for_field_def(field, target)
                )
                .expect("write to String");
            }
            out.push_str("]);\n");
            writeln!(out, "export const {name}Codec = {{").expect("write to String");
            writeln!(out, "  ...{name}StructCodec,").expect("write to String");
            writeln!(
                out,
                "  decode(data: Parameters<typeof {name}StructCodec.decode>[0], offset = 0): \
                 {name} {{ return decodeExact<{name}>({name}StructCodec, \
                 Uint8Array.from(data).slice(offset)); }},"
            )
            .expect("write to String");
            out.push_str("};\n\n");
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

    for account in &idl.accounts {
        let name = &account.name;
        let const_name = pascal_to_screaming_snake(name);
        out.push('\n');
        writeln!(
            out,
            "  decode{}(input: ArrayLike<number>): {} {{",
            name, name
        )
        .expect("write to String");
        out.push_str("    const data = Uint8Array.from(input);\n");
        writeln!(
            out,
            "    if (!matchDisc(data, {}_DISCRIMINATOR)) throw new Error(\"Invalid {} \
             discriminator\");",
            const_name, name
        )
        .expect("write to String");
        writeln!(
            out,
            "    return decodeExact<{}>({}Codec, data.slice({}_DISCRIMINATOR.length));",
            name, name, const_name
        )
        .expect("write to String");
        out.push_str("  }\n");
    }

    if !idl.events.is_empty() {
        out.push('\n');
        out.push_str("  decodeEvent(input: ArrayLike<number>): DecodedEvent | null {\n");
        out.push_str("    const data = Uint8Array.from(input);\n");
        for event in &idl.events {
            let has_type = idl.types.iter().any(|t| t.name == event.name);
            let const_name = format!("{}_DISCRIMINATOR", pascal_to_screaming_snake(&event.name));
            writeln!(out, "    if (matchDisc(data, {})) {{", const_name).expect("write to String");
            if has_type {
                writeln!(
                    out,
                    "      return {{ type: ProgramEvent.{0}, data: decodeExact<{0}>({0}Codec, \
                     data.slice({1}.length)) }};",
                    event.name, const_name
                )
                .expect("write to String");
            } else {
                writeln!(
                    out,
                    "      if (data.length !== {const_name}.length) throw new Error(\"trailing \
                     bytes\");"
                )
                .expect("write to String");
                writeln!(out, "      return {{ type: ProgramEvent.{} }};", event.name)
                    .expect("write to String");
            }
            out.push_str("    }\n");
        }
        out.push_str("    return null;\n");
        out.push_str("  }\n");
    }

    if !idl.instructions.is_empty() {
        out.push('\n');
        out.push_str(
            "  decodeInstruction(input: ArrayLike<number>): DecodedInstruction | null {\n",
        );
        out.push_str("    const data = Uint8Array.from(input);\n");
        for ix in &idl.instructions {
            let pascal = snake_to_pascal(&ix.name);
            let const_name = format!(
                "{}_INSTRUCTION_DISCRIMINATOR",
                pascal_to_screaming_snake(&pascal)
            );
            if ix.args.is_empty() {
                writeln!(out, "    if (matchDisc(data, {})) {{", const_name)
                    .expect("write to String");
                writeln!(
                    out,
                    "      if (data.length !== {const_name}.length) throw new Error(\"trailing \
                     bytes\");"
                )
                .expect("write to String");
                writeln!(
                    out,
                    "      return {{ type: ProgramInstruction.{} }};",
                    pascal
                )
                .expect("write to String");
                out.push_str("    }\n");
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
                        "      return {{ type: ProgramInstruction.{0}, args: \
                         decodeExact<{0}InstructionArgs>(argsCodec, data.slice({1}.length)) }};",
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

    match target {
        TsTarget::Web3js => web3::emit_instruction_builders(
            &mut out,
            idl,
            &exportable_pda_helpers,
            &model.identity.program_name,
        ),
        TsTarget::Kit => kit::emit_instruction_builders(&mut out, idl, &exportable_pda_helpers),
    }

    out.push_str("}\n\n");

    if emit_plugin {
        kit::emit_program_plugin(&mut out, &model, &plugin_accounts, &plugin_instructions);
    }

    if !pdas.is_empty() {
        emit_pda_helpers(&mut out, &pdas, target, &model.identity.program_name);
    }

    // === Errors ===
    if !idl.errors.is_empty() {
        out.push_str("/* Errors */\n");
        out.push_str("export const PROGRAM_ERROR_CODES = {\n");
        for err in &idl.errors {
            writeln!(out, "  {}: {},", err.name, err.code).expect("write to String");
        }
        out.push_str("} as const;\n\n");
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

    Ok(out)
}
