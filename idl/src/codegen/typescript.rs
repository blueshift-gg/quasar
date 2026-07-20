use {
    super::model::{
        account_field_definition, account_field_seed_inputs, resolved_account_order, CodegenResult,
        ProgramModel,
    },
    crate::types::{
        AccountFlag, Idl, IdlAccountDef, IdlArg, IdlCodec, IdlFieldDef, IdlInstruction,
        IdlPdaProgram, IdlPdaSeed, IdlResolver, IdlType, ScalarRepr,
    },
    quasar_schema::{
        snake_to_pascal, to_camel_case, to_screaming_snake as pascal_to_screaming_snake,
    },
    std::{
        collections::{HashMap, HashSet},
        fmt::Write,
    },
};

const SOLANA_KIT_VERSION: &str = "^6.4.0";
const SOLANA_WEB3JS_VERSION: &str = "^3.0.0-rc.1";

/// Target flavor for TypeScript client generation.
#[derive(Clone, Copy, PartialEq)]
pub enum TsTarget {
    Web3js,
    Kit,
}

pub fn client_dependency_version(target: TsTarget) -> &'static str {
    match target {
        TsTarget::Web3js => SOLANA_WEB3JS_VERSION,
        TsTarget::Kit => SOLANA_KIT_VERSION,
    }
}

#[derive(Clone, Copy)]
enum InlinePdaTarget<'a> {
    Web3js { program_expr: &'a str },
    Kit { program_expr: &'a str },
}

impl<'a> InlinePdaTarget<'a> {
    fn target(self) -> TsTarget {
        match self {
            Self::Web3js { .. } => TsTarget::Web3js,
            Self::Kit { .. } => TsTarget::Kit,
        }
    }

    fn program_expr(self) -> &'a str {
        match self {
            Self::Web3js { program_expr } | Self::Kit { program_expr } => program_expr,
        }
    }
}

/// Generate a TypeScript client targeting @solana/web3.js.
pub fn generate_ts_client(idl: &Idl) -> CodegenResult<String> {
    generate_ts(idl, TsTarget::Web3js)
}

/// Generate a TypeScript client targeting @solana/kit.
pub fn generate_ts_client_kit(idl: &Idl) -> CodegenResult<String> {
    generate_ts(idl, TsTarget::Kit)
}

pub fn generate_package_json(idl: &Idl) -> CodegenResult<String> {
    let model = ProgramModel::try_new(idl)?;
    let codecs_dep = if model.features.needs_codecs {
        "\n    \"@solana/codecs\": \"^6.2.0\","
    } else {
        ""
    };

    Ok(format!(
        r#"{{
  "name": "{package_name}",
  "version": "{version}",
  "private": true,
  "exports": {{
    "./web3.js": "./web3.ts",
    "./kit": "./kit.ts"
  }},
  "dependencies": {{{codecs_dep}
    "@solana/kit": "{solana_kit_version}",
    "@solana/web3.js": "{solana_web3js_version}"
  }}
}}
"#,
        package_name = model.identity.typescript_package,
        solana_kit_version = client_dependency_version(TsTarget::Kit),
        solana_web3js_version = client_dependency_version(TsTarget::Web3js),
        version = idl.version,
    ))
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
    let plugin_accounts = eligible_plugin_accounts(idl);
    let plugin_instructions = eligible_plugin_instructions(idl);
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
            "    return decodeExact<{}>({}Codec, data.slice({}_DISCRIMINATOR.length));",
            name, name, const_name
        )
        .expect("write to String");
        out.push_str("  }\n");
    }

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
        }
        out.push_str("    return null;\n");
        out.push_str("  }\n");
    }

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
        TsTarget::Web3js => generate_instruction_builders_web3js(
            &mut out,
            idl,
            &exportable_pda_helpers,
            &model.identity.program_name,
        ),
        TsTarget::Kit => generate_instruction_builders_kit(&mut out, idl, &exportable_pda_helpers),
    }

    out.push_str("}\n\n");

    if emit_plugin {
        emit_kit_program_plugin(&mut out, &model, &plugin_accounts, &plugin_instructions);
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

fn emit_kit_program_plugin(
    out: &mut String,
    model: &ProgramModel,
    accounts: &[&IdlAccountDef],
    instructions: &[&IdlInstruction],
) {
    let program_pascal = snake_to_pascal(&model.identity.program_name);
    let program_camel = lower_first(&program_pascal);
    let class_name = format!("{program_pascal}Client");

    out.push_str("/* Program Plugin */\n");

    let mut requirements = Vec::new();
    if !accounts.is_empty() {
        requirements.push("ClientWithRpc<GetAccountInfoApi & GetMultipleAccountsApi>");
    }
    if !instructions.is_empty() {
        requirements.push("ClientWithPayer");
        requirements.push("ClientWithTransactionPlanning");
        requirements.push("ClientWithTransactionSending");
    }

    writeln!(
        out,
        "export type {program_pascal}PluginRequirements = {};\n",
        requirements.join(" &\n  ")
    )
    .expect("write to String");

    writeln!(out, "export function {program_camel}Program() {{").expect("write to String");
    writeln!(out, "  const __client = new {class_name}();").expect("write to String");
    writeln!(
        out,
        "  return <T extends {program_pascal}PluginRequirements>(client: T) => ({{"
    )
    .expect("write to String");
    out.push_str("    ...client,\n");
    writeln!(out, "    {program_camel}: {{").expect("write to String");

    if !accounts.is_empty() {
        out.push_str("      accounts: {\n");
        for account in accounts {
            let key = lower_first(&account.name);
            writeln!(
                out,
                "        {key}: addSelfFetchFunctions(client, {}Codec),",
                account.name
            )
            .expect("write to String");
        }
        out.push_str("      },\n");
    }

    if !instructions.is_empty() {
        out.push_str("      instructions: {\n");
        for ix in instructions {
            let ix_pascal = snake_to_pascal(&ix.name);
            let ix_camel = to_camel_case(&ix.name);
            if kit_plugin_instruction_has_input(ix) {
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

fn eligible_plugin_accounts(idl: &Idl) -> Vec<&IdlAccountDef> {
    idl.accounts
        .iter()
        .filter(|account| {
            idl.types
                .iter()
                .find(|ty| ty.name == account.name)
                .is_some_and(|ty| !has_dynamic_field_defs(&ty.fields))
        })
        .collect()
}

fn eligible_plugin_instructions(idl: &Idl) -> Vec<&IdlInstruction> {
    idl.instructions
        .iter()
        .filter(|ix| !instruction_has_account_field_pda_seeds(ix))
        .collect()
}

fn kit_plugin_instruction_has_input(ix: &IdlInstruction) -> bool {
    ix.remaining_accounts.is_some()
        || !ix.args.is_empty()
        || ix
            .accounts
            .iter()
            .any(|account| account.optional || matches!(account.resolver, IdlResolver::Input {}))
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

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
            if acc.optional || matches!(acc.resolver, IdlResolver::Input {}) {
                user_accs.push(acc);
            } else {
                has_non_input_accounts = true;
            }
        }

        let input_account_names: HashSet<&str> =
            user_accs.iter().map(|a| a.name.as_str()).collect();
        let optional_account_names: HashSet<&str> = user_accs
            .iter()
            .filter(|account| account.optional)
            .map(|account| account.name.as_str())
            .collect();
        let ix_needs_account_resolver = instruction_has_account_field_pda_seeds(ix);
        let ix_has_pdas = ix.accounts.iter().any(|a| {
            !a.optional
                && matches!(
                    a.resolver,
                    IdlResolver::Pda { .. } | IdlResolver::AssociatedToken { .. }
                )
        });

        let account_expr = |name: &str| {
            if input_account_names.contains(name) {
                if optional_account_names.contains(name) {
                    format!("(accountOverrides.{name} ?? input.{name} ?? {class_name}.programId)")
                } else {
                    format!("(accountOverrides.{name} ?? input.{name})")
                }
            } else {
                format!("(accountOverrides.{name} ?? accountsMap[\"{name}\"])")
            }
        };

        // Method signature
        let mut method_params = Vec::new();
        if !user_accs.is_empty() || !ix.args.is_empty() || has_remaining {
            method_params.push(format!("input: {pascal}InstructionInput"));
        }
        if ix_needs_account_resolver {
            method_params.push("resolver: AccountDataResolver".to_string());
        }
        let ix_needs_async = ix_needs_account_resolver || ix_has_pdas;
        let async_kw = if ix_needs_async { "async " } else { "" };
        let return_type = if ix_needs_async {
            "Promise<TransactionInstruction>"
        } else {
            "TransactionInstruction"
        };
        if !ix.accounts.is_empty() {
            writeln!(
                out,
                "  {async_kw}create{pascal}Instruction({}): {return_type} {{",
                method_params.join(", ")
            )
            .expect("write to String");
            let mut unchecked_args = Vec::new();
            if !user_accs.is_empty() || !ix.args.is_empty() || has_remaining {
                unchecked_args.push("input");
            }
            unchecked_args.push("{}");
            if ix_needs_account_resolver {
                unchecked_args.push("resolver");
            }
            writeln!(
                out,
                "    return this.create{pascal}InstructionUnchecked({});",
                unchecked_args.join(", ")
            )
            .expect("write to String");
            out.push_str("  }\n\n");

            let override_position = if !user_accs.is_empty() || !ix.args.is_empty() || has_remaining
            {
                1
            } else {
                0
            };
            method_params.insert(
                override_position,
                format!("accountOverrides: {pascal}InstructionAccountOverrides"),
            );
        }
        writeln!(
            out,
            "  {async_kw}create{pascal}Instruction{}({}): {return_type} {{",
            if ix.accounts.is_empty() {
                ""
            } else {
                "Unchecked"
            },
            method_params.join(", ")
        )
        .expect("write to String");

        if has_non_input_accounts {
            out.push_str("    const accountsMap: Record<string, Address> = {};\n");
        }

        // Derive fixed-address accounts
        for acc in &ix.accounts {
            if acc.optional {
                continue;
            }
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
        for acc in resolved_account_order(ix).expect("validated derived-account order") {
            if let IdlResolver::Pda { program, seeds } = &acc.resolver {
                let helper_name = matches!(program, IdlPdaProgram::ProgramId {})
                    .then(|| exportable_pda_helpers.get(&format!("{:?}", seeds)))
                    .flatten();
                if let Some(helper_name) = helper_name {
                    let args = helper_call_args(seeds, &account_expr);
                    writeln!(
                        out,
                        "    accountsMap[\"{}\"] = await {}({});",
                        acc.name, helper_name, args
                    )
                    .expect("write to String");
                } else {
                    emit_account_field_seed_resolvers(out, seeds, idl, &account_expr);
                    let program_expr = match program {
                        IdlPdaProgram::ProgramId {} => format!("{class_name}.programId"),
                        IdlPdaProgram::Account { path } => account_expr(path),
                    };
                    emit_inline_pda_derivation(
                        out,
                        &acc.name,
                        seeds,
                        idl,
                        InlinePdaTarget::Web3js {
                            program_expr: &program_expr,
                        },
                        &arg_types,
                        &account_expr,
                    );
                }
            } else if let IdlResolver::AssociatedToken {
                mint,
                owner,
                token_program,
            } = &acc.resolver
            {
                emit_associated_token_derivation(
                    out,
                    &acc.name,
                    mint,
                    owner,
                    token_program.as_deref(),
                    TsTarget::Web3js,
                    &account_expr,
                );
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
                "    const data = Uint8Array.from([{}, ...argsCodec.encode({{ {} }})]);",
                disc_str,
                arg_names.join(", ")
            )
            .expect("write to String");
        } else {
            // Compact layout: [disc][fixed fields][all prefixes][all dynamic data]
            emit_compact_encoding(out, ix, &disc_str, TsTarget::Web3js, "Uint8Array.from");
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
            if acc.optional || matches!(acc.resolver, IdlResolver::Input {}) {
                user_accs.push(acc);
            } else {
                has_non_input_accounts = true;
            }
        }

        let input_account_names: HashSet<&str> =
            user_accs.iter().map(|a| a.name.as_str()).collect();
        let optional_account_names: HashSet<&str> = user_accs
            .iter()
            .filter(|account| account.optional)
            .map(|account| account.name.as_str())
            .collect();
        let ix_needs_account_resolver = instruction_has_account_field_pda_seeds(ix);

        let account_expr = |name: &str| {
            if input_account_names.contains(name) {
                if optional_account_names.contains(name) {
                    format!("(accountOverrides.{name} ?? input.{name} ?? PROGRAM_ADDRESS)")
                } else {
                    format!("(accountOverrides.{name} ?? input.{name})")
                }
            } else {
                format!("(accountOverrides.{name} ?? accountsMap[\"{name}\"])")
            }
        };

        // Check if this instruction has any PDAs (requires async)
        let ix_has_pdas = ix.accounts.iter().any(|a| {
            !a.optional
                && matches!(
                    a.resolver,
                    IdlResolver::Pda { .. } | IdlResolver::AssociatedToken { .. }
                )
        });

        // Method signature
        let mut method_params = Vec::new();
        if !user_accs.is_empty() || !ix.args.is_empty() || has_remaining {
            method_params.push(format!("input: {pascal}InstructionInput"));
        }
        if ix_needs_account_resolver {
            method_params.push("resolver: AccountDataResolver".to_string());
        }
        let return_type = if ix_has_pdas {
            "Promise<Instruction>"
        } else {
            "Instruction"
        };
        let async_kw = if ix_has_pdas { "async " } else { "" };
        if !ix.accounts.is_empty() {
            writeln!(
                out,
                "  {async_kw}create{pascal}Instruction({}): {return_type} {{",
                method_params.join(", ")
            )
            .expect("write to String");
            let mut unchecked_args = Vec::new();
            if !user_accs.is_empty() || !ix.args.is_empty() || has_remaining {
                unchecked_args.push("input");
            }
            unchecked_args.push("{}");
            if ix_needs_account_resolver {
                unchecked_args.push("resolver");
            }
            writeln!(
                out,
                "    return this.create{pascal}InstructionUnchecked({});",
                unchecked_args.join(", ")
            )
            .expect("write to String");
            out.push_str("  }\n\n");

            let override_position = if !user_accs.is_empty() || !ix.args.is_empty() || has_remaining
            {
                1
            } else {
                0
            };
            method_params.insert(
                override_position,
                format!("accountOverrides: {pascal}InstructionAccountOverrides"),
            );
        }
        writeln!(
            out,
            "  {async_kw}create{pascal}Instruction{}({}): {return_type} {{",
            if ix.accounts.is_empty() {
                ""
            } else {
                "Unchecked"
            },
            method_params.join(", ")
        )
        .expect("write to String");

        if has_non_input_accounts {
            out.push_str("    const accountsMap: Record<string, Address> = {};\n");
        }

        // Derive fixed-address accounts
        for acc in &ix.accounts {
            if acc.optional {
                continue;
            }
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
        for acc in resolved_account_order(ix).expect("validated derived-account order") {
            if let IdlResolver::Pda { program, seeds } = &acc.resolver {
                let helper_name = matches!(program, IdlPdaProgram::ProgramId {})
                    .then(|| exportable_pda_helpers.get(&format!("{:?}", seeds)))
                    .flatten();
                if let Some(helper_name) = helper_name {
                    let args = helper_call_args(seeds, &account_expr);
                    writeln!(
                        out,
                        "    accountsMap[\"{}\"] = await {}({});",
                        acc.name, helper_name, args
                    )
                    .expect("write to String");
                } else {
                    emit_account_field_seed_resolvers(out, seeds, idl, &account_expr);
                    let program_expr = match program {
                        IdlPdaProgram::ProgramId {} => "PROGRAM_ADDRESS".to_string(),
                        IdlPdaProgram::Account { path } => account_expr(path),
                    };
                    emit_inline_pda_derivation(
                        out,
                        &acc.name,
                        seeds,
                        idl,
                        InlinePdaTarget::Kit {
                            program_expr: &program_expr,
                        },
                        &arg_types,
                        &account_expr,
                    );
                }
            } else if let IdlResolver::AssociatedToken {
                mint,
                owner,
                token_program,
            } = &acc.resolver
            {
                emit_associated_token_derivation(
                    out,
                    &acc.name,
                    mint,
                    owner,
                    token_program.as_deref(),
                    TsTarget::Kit,
                    &account_expr,
                );
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
        TsTarget::Web3js => "Uint8Array.from",
        TsTarget::Kit => "Uint8Array.from",
    };

    writeln!(out, "export const {name}Codec = {{").expect("write to String");

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
        if let Some(inner) = optional_dynamic_inner(&f.ty) {
            let pfx = codec_prefix_bytes(&f.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(
                    out,
                    "    const {name}Payload = value.{name} === null ? new Uint8Array(0) : new \
                     TextEncoder().encode(value.{name});",
                    name = f.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = value.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode({name}Payload.length), ...{name}Payload]);",
                    name = f.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "    const {name}Payload = value.{name} === null ? new Uint8Array(0) : \
                     getArrayCodec({item_codec}, {{ size: value.{name}.length \
                     }}).encode(value.{name});",
                    name = f.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = value.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode(value.{name}.length), ...{name}Payload]);",
                    name = f.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            }
        } else if is_string_type(&f.ty) {
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
        if optional_dynamic_inner(&f.ty).is_some() {
            concat_parts.push(format!("{}Tag", f.name));
        } else {
            concat_parts.push(format!("{}Prefix", f.name));
        }
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
        out.push_str("    const fixedSize = codecSize(fixedCodec, fixedResult);\n");
        out.push_str(
            "    if (!bytesEqual(fixedCodec.encode(fixedResult), checkedTake(data, offset, \
             fixedSize))) throw new Error(\"invalid fixed field encoding\");\n",
        );
        out.push_str("    offset += fixedSize;\n");
    }

    // Phase 2: decode length prefixes
    for f in &dyn_fields {
        let pfx = codec_prefix_bytes(&f.codec);
        let pfx_codec = prefix_codec(pfx);
        if optional_dynamic_inner(&f.ty).is_some() {
            writeln!(
                out,
                "    const {name}Tag = getU8Codec().decode(checkedTake(data, offset, 1));",
                name = f.name
            )
            .expect("write to String");
            out.push_str("    offset += 1;\n");
            writeln!(
                out,
                "    if ({name}Tag !== 0 && {name}Tag !== 1) throw new Error(\"invalid option tag \
                 for {name}\");",
                name = f.name
            )
            .expect("write to String");
        } else {
            writeln!(
                out,
                "    const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                name = f.name,
                codec = pfx_codec,
                pfx = pfx,
            )
            .expect("write to String");
            writeln!(out, "    offset += {};", pfx).expect("write to String");
        }
    }

    // Phase 3: decode tail data
    for f in &dyn_fields {
        if let Some(inner) = optional_dynamic_inner(&f.ty) {
            let pfx = codec_prefix_bytes(&f.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(out, "    let {name}: string | null = null;", name = f.name)
                    .expect("write to String");
                writeln!(out, "    if ({name}Tag === 1) {{", name = f.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "      const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = f.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "      offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "      const {name}Size = checkedLength({name}Len);\n\x20     {name} = \
                     decodeUtf8(checkedTake(data, offset, {name}Size));",
                    name = f.name
                )
                .expect("write to String");
                writeln!(out, "      offset += {}Size;", f.name).expect("write to String");
                out.push_str("    }\n");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "    let {name}: Array<{item_ty}> | null = null;",
                    name = f.name,
                    item_ty = ts_type(vec),
                )
                .expect("write to String");
                writeln!(out, "    if ({name}Tag === 1) {{", name = f.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "      const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = f.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "      offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "      const {name}Count = checkedElementCount({name}Len, data.length - \
                     offset);\n\x20     const {name}Codec = getArrayCodec({item_codec}, {{ size: \
                     {name}Count }});",
                    name = f.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "      {name} = {name}Codec.decode(data.slice(offset));",
                    name = f.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "      offset += {name}Codec.encode({name}).length;",
                    name = f.name
                )
                .expect("write to String");
                out.push_str("    }\n");
            }
        } else if is_string_type(&f.ty) {
            writeln!(
                out,
                "    const {name}Size = checkedLength({name}Len);\n\x20   const {name} = \
                 decodeUtf8(checkedTake(data, offset, {name}Size));",
                name = f.name
            )
            .expect("write to String");
            writeln!(out, "    offset += {}Size;", f.name).expect("write to String");
        } else if let IdlType::Vec { vec } = &f.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "    const {name}Count = checkedElementCount({name}Len, data.length - \
                 offset);\n\x20   const {name}Codec = getArrayCodec({item_codec}, {{ size: \
                 {name}Count }});",
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
        if let IdlType::Option { option } = &f.ty {
            field_exprs.push(format!(
                "{}: unwrapOption<{}>(fixedResult.{})",
                f.name,
                ts_type(option),
                f.name
            ));
        } else {
            field_exprs.push(format!("{}: fixedResult.{}", f.name, f.name));
        }
    }
    for f in &dyn_fields {
        field_exprs.push(f.name.clone());
    }
    writeln!(out, "    const result = {{ {} }};", field_exprs.join(", ")).expect("write to String");
    out.push_str("    assertFinished(data, offset);\n");
    out.push_str(
        "    if (!bytesEqual(this.encode(result), data)) throw new Error(\"invalid field \
         encoding\");\n",
    );
    out.push_str("    return result;\n");
    out.push_str("  },\n");

    out.push_str("};\n\n");
}

/// Emit compact (3-phase) encoding for an instruction with dynamic fields.
///
/// Layout: `[disc][fixed fields][all length prefixes][all dynamic data]`
fn emit_compact_encoding(
    out: &mut String,
    ix: &crate::types::IdlInstruction,
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
        if optional_dynamic_inner(&arg.ty).is_some() {
            writeln!(
                out,
                "    const {name}Tag = getU8Codec().encode(input.{name} === null ? 0 : 1);",
                name = arg.name
            )
            .expect("write to String");
        } else if is_string_type(&arg.ty) {
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
        if let Some(inner) = optional_dynamic_inner(&arg.ty) {
            let pfx = codec_prefix_bytes(&arg.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(
                    out,
                    "    const {name}Payload = input.{name} === null ? new Uint8Array(0) : new \
                     TextEncoder().encode(input.{name});",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = input.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode({name}Payload.length), ...{name}Payload]);",
                    name = arg.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "    const {name}Payload = input.{name} === null ? new Uint8Array(0) : \
                     getArrayCodec({item_codec}, {{ size: input.{name}.length \
                     }}).encode(input.{name});",
                    name = arg.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = input.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode(input.{name}.length), ...{name}Payload]);",
                    name = arg.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            }
        } else if is_string_type(&arg.ty) {
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
        if optional_dynamic_inner(&arg.ty).is_some() {
            concat_parts.push(format!("{}Tag", arg.name));
        } else {
            concat_parts.push(format!("{}Prefix", arg.name));
        }
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
    ix: &crate::types::IdlInstruction,
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
        out.push_str("      const fixedSize = codecSize(fixedCodec, fixedResult);\n");
        out.push_str(
            "      if (!bytesEqual(fixedCodec.encode(fixedResult), checkedTake(data, offset, \
             fixedSize))) throw new Error(\"invalid fixed field encoding\");\n",
        );
        out.push_str("      offset += fixedSize;\n");
    }

    // Phase 2: decode length prefixes
    for arg in &dyn_args {
        let pfx = codec_prefix_bytes(&arg.codec);
        let pfx_codec = prefix_codec(pfx);
        if optional_dynamic_inner(&arg.ty).is_some() {
            writeln!(
                out,
                "      const {name}Tag = getU8Codec().decode(checkedTake(data, offset, 1));",
                name = arg.name
            )
            .expect("write to String");
            out.push_str("      offset += 1;\n");
            writeln!(
                out,
                "      if ({name}Tag !== 0 && {name}Tag !== 1) throw new Error(\"invalid option \
                 tag for {name}\");",
                name = arg.name
            )
            .expect("write to String");
        } else {
            writeln!(
                out,
                "      const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                name = arg.name,
                codec = pfx_codec,
                pfx = pfx,
            )
            .expect("write to String");
            writeln!(out, "      offset += {};", pfx).expect("write to String");
        }
    }

    // Phase 3: decode tail data
    for arg in &dyn_args {
        if let Some(inner) = optional_dynamic_inner(&arg.ty) {
            let pfx = codec_prefix_bytes(&arg.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(
                    out,
                    "      let {name}: string | null = null;",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(out, "      if ({name}Tag === 1) {{", name = arg.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "        const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = arg.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "        offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "        const {name}Size = checkedLength({name}Len);\n\x20       {name} = \
                     decodeUtf8(checkedTake(data, offset, {name}Size));",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(out, "        offset += {}Size;", arg.name).expect("write to String");
                out.push_str("      }\n");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "      let {name}: Array<{item_ty}> | null = null;",
                    name = arg.name,
                    item_ty = ts_type(vec),
                )
                .expect("write to String");
                writeln!(out, "      if ({name}Tag === 1) {{", name = arg.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "        const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = arg.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "        offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "        const {name}Count = checkedElementCount({name}Len, data.length - \
                     offset);\n\x20       const {name}Codec = getArrayCodec({item_codec}, {{ \
                     size: {name}Count }});",
                    name = arg.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "        {name} = {name}Codec.decode(data.slice(offset));",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "        offset += {name}Codec.encode({name}).length;",
                    name = arg.name
                )
                .expect("write to String");
                out.push_str("      }\n");
            }
        } else if is_string_type(&arg.ty) {
            writeln!(
                out,
                "      const {name}Size = checkedLength({name}Len);\n\x20     const {name} = \
                 decodeUtf8(checkedTake(data, offset, {name}Size));",
                name = arg.name
            )
            .expect("write to String");
            writeln!(out, "      offset += {}Size;", arg.name).expect("write to String");
        } else if let IdlType::Vec { vec } = &arg.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "      const {name}Count = checkedElementCount({name}Len, data.length - \
                 offset);\n\x20     const {name}Codec = getArrayCodec({item_codec}, {{ size: \
                 {name}Count }});",
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
        if let IdlType::Option { option } = &arg.ty {
            field_exprs.push(format!(
                "{}: unwrapOption<{}>(fixedResult.{})",
                arg.name,
                ts_type(option),
                arg.name
            ));
        } else {
            field_exprs.push(format!("{}: fixedResult.{}", arg.name, arg.name));
        }
    }
    for arg in &dyn_args {
        field_exprs.push(arg.name.clone());
    }
    out.push_str("      assertFinished(data, offset);\n");
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
                IdlResolver::Pda {
                    program: IdlPdaProgram::ProgramId {},
                    seeds,
                } => seeds,
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
                    IdlPdaSeed::AccountField { .. } => {}
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
                    "export async function {}({}): Promise<Address> {{",
                    pda.helper_name, params
                )
                .expect("write to String");
                out.push_str("  return (await Address.findProgramAddress(\n");
                out.push_str("    [\n");
                write_ts_pda_seed_lines(out, &pda.seeds, target, &arg_types);
                writeln!(
                    out,
                    "    ],\n    {}Client.programId,\n  ))[0];",
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
            IdlPdaSeed::AccountField { .. } => continue,
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
            IdlPdaSeed::AccountField { .. } => {}
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
    idl: &Idl,
    target: InlinePdaTarget<'_>,
    arg_types: &HashMap<String, IdlType>,
    account_expr: &impl Fn(&str) -> String,
) {
    let ts_target = target.target();
    match target {
        InlinePdaTarget::Web3js { program_expr } => {
            writeln!(
                out,
                "    accountsMap[\"{}\"] = (await Address.findProgramAddress(",
                account_name
            )
            .expect("write to String");
            out.push_str("      [\n");
            write_inline_pda_seed_lines(out, seeds, idl, ts_target, arg_types, account_expr);
            writeln!(out, "      ],\n      {},\n    ))[0];", program_expr)
                .expect("write to String");
        }
        InlinePdaTarget::Kit { .. } => {
            writeln!(
                out,
                "    accountsMap[\"{}\"] = (await getProgramDerivedAddress({{",
                account_name
            )
            .expect("write to String");
            writeln!(out, "      programAddress: {},", target.program_expr())
                .expect("write to String");
            out.push_str("      seeds: [\n");
            write_inline_pda_seed_lines(out, seeds, idl, ts_target, arg_types, account_expr);
            out.push_str("      ],\n");
            out.push_str("    }))[0];\n");
        }
    }
}

fn emit_associated_token_derivation(
    out: &mut String,
    account_name: &str,
    mint: &str,
    owner: &str,
    token_program: Option<&str>,
    target: TsTarget,
    account_expr: &impl Fn(&str) -> String,
) {
    let mint = account_expr(mint);
    let owner = account_expr(owner);
    let token_program = token_program
        .map(account_expr)
        .unwrap_or_else(|| match target {
            TsTarget::Web3js => {
                "new Address(\"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA\")".to_string()
            }
            TsTarget::Kit => "address(\"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA\")".to_string(),
        });

    match target {
        TsTarget::Web3js => {
            writeln!(
                out,
                "    accountsMap[\"{account_name}\"] = (await Address.findProgramAddress(\n      \
                 [{owner}.toBytes(), {token_program}.toBytes(), {mint}.toBytes()],\n      new \
                 Address(\"ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL\"),\n    ))[0];"
            )
            .expect("write to String");
        }
        TsTarget::Kit => {
            writeln!(
                out,
                "    accountsMap[\"{account_name}\"] = (await getProgramDerivedAddress({{\n      \
                 programAddress: address(\"ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL\"),\n      \
                 seeds: [getAddressCodec().encode({owner}), \
                 getAddressCodec().encode({token_program}), \
                 getAddressCodec().encode({mint})],\n    }}))[0];"
            )
            .expect("write to String");
        }
    }
}

fn write_inline_pda_seed_lines(
    out: &mut String,
    seeds: &[IdlPdaSeed],
    idl: &Idl,
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
            IdlPdaSeed::AccountField {
                path,
                account,
                field,
            } => {
                let expr = account_field_seed_var(path, field);
                let Some(ty) = account_field_definition(idl, account, field).map(|field| &field.ty)
                else {
                    writeln!(out, "        {},", expr).expect("write to String");
                    continue;
                };
                writeln!(out, "        {},", ts_pda_arg_seed_expr(&expr, ty, target))
                    .expect("write to String");
            }
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

fn pda_is_exportable(seeds: &[IdlPdaSeed], arg_types: &HashMap<String, IdlType>) -> bool {
    seeds.iter().all(|seed| match seed {
        IdlPdaSeed::Const { .. } => true,
        IdlPdaSeed::Account { path } => is_identifier(path),
        IdlPdaSeed::AccountField { .. } => false,
        IdlPdaSeed::Arg { path, .. } => is_identifier(path) && arg_types.contains_key(path),
    })
}

fn emit_account_field_seed_resolvers(
    out: &mut String,
    seeds: &[IdlPdaSeed],
    idl: &Idl,
    account_expr: &impl Fn(&str) -> String,
) {
    let mut seen = HashSet::new();
    for seed in seeds {
        let IdlPdaSeed::AccountField {
            path,
            account,
            field,
        } = seed
        else {
            continue;
        };
        let key = format!("{path}.{field}");
        if !seen.insert(key) {
            continue;
        }
        if account_field_definition(idl, account, field).is_none() {
            continue;
        }

        let data_var = account_field_data_var(path, field);
        let account_var = account_field_account_var(path, field);
        let value_var = account_field_seed_var(path, field);
        writeln!(
            out,
            "    const {data_var} = await resolver.getAccountData({});",
            account_expr(path)
        )
        .expect("write to String");
        writeln!(
            out,
            "    if ({data_var} === null) throw new Error(\"Unable to resolve account data for \
             {path}\");"
        )
        .expect("write to String");
        writeln!(
            out,
            "    const {account_var} = this.decode{account}({data_var});"
        )
        .expect("write to String");
        writeln!(
            out,
            "    const {value_var} = {account_var}.{};",
            ts_field_access(field)
        )
        .expect("write to String");
    }
}

fn has_account_field_pda_seeds(idl: &Idl) -> bool {
    idl.instructions
        .iter()
        .any(instruction_has_account_field_pda_seeds)
}

fn instruction_has_account_field_pda_seeds(ix: &crate::types::IdlInstruction) -> bool {
    !account_field_seed_inputs(ix).is_empty()
}

fn account_field_data_var(path: &str, field: &str) -> String {
    format!("__{}Data", account_field_var_stem(path, field))
}

fn account_field_account_var(path: &str, field: &str) -> String {
    format!("__{}Account", account_field_var_stem(path, field))
}

fn account_field_seed_var(path: &str, field: &str) -> String {
    format!("__{}Seed", account_field_var_stem(path, field))
}

fn account_field_var_stem(path: &str, field: &str) -> String {
    let mut out = String::new();
    for part in path.split('.').chain(field.split('.')) {
        if part.is_empty() {
            continue;
        }
        if out.is_empty() {
            out.push_str(part);
        } else {
            out.push_str(&snake_to_pascal(part));
        }
    }
    out
}

fn ts_field_access(field: &str) -> String {
    field.split('.').collect::<Vec<_>>().join(".")
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

/// Check if a type represents a string (for dynamic codec purposes).
fn is_string_type(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "string")
}

fn optional_dynamic_inner(ty: &IdlType) -> Option<&IdlType> {
    match ty {
        IdlType::Option { option }
            if is_string_type(option) || matches!(**option, IdlType::Vec { .. }) =>
        {
            Some(option)
        }
        _ => None,
    }
}

fn is_optional_dynamic_string(ty: &IdlType) -> bool {
    optional_dynamic_inner(ty).is_some_and(is_string_type)
}

fn is_optional_dynamic_vec(ty: &IdlType) -> bool {
    optional_dynamic_inner(ty).is_some_and(|inner| matches!(inner, IdlType::Vec { .. }))
}

fn is_u8_type(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "u8")
        || matches!(
            ty,
            IdlType::Defined { defined } if builtin_defined_primitive(&defined.name) == Some("u8")
        )
}

fn builtin_defined_primitive(name: &str) -> Option<&'static str> {
    match name {
        "PodBool" => Some("bool"),
        "PodU8" => Some("u8"),
        "PodI8" => Some("i8"),
        "PodU16" => Some("u16"),
        "PodI16" => Some("i16"),
        "PodU32" => Some("u32"),
        "PodI32" => Some("i32"),
        "PodU64" => Some("u64"),
        "PodI64" => Some("i64"),
        "PodU128" => Some("u128"),
        "PodI128" => Some("i128"),
        _ => None,
    }
}

fn primitive_ts_type(primitive: &str) -> String {
    match primitive {
        "u8" | "u16" | "u32" | "i8" | "i16" | "i32" => "number".to_string(),
        "u64" | "u128" | "i64" | "i128" => "bigint".to_string(),
        "bool" => "boolean".to_string(),
        "pubkey" => "Address".to_string(),
        "string" => "string".to_string(),
        "bytes" => "Uint8Array".to_string(),
        other if other.starts_with('[') => "Uint8Array".to_string(),
        other => other.to_string(),
    }
}

fn primitive_ts_codec(primitive: &str, target: TsTarget) -> String {
    match primitive {
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
            TsTarget::Web3js => "getWeb3jsAddressCodec()".to_string(),
            TsTarget::Kit => "getAddressCodec()".to_string(),
        },
        "string" => "addCodecSizePrefix(getUtf8Codec(), getU32Codec())".to_string(),
        other if other.starts_with('[') => {
            let size = super::parse_fixed_array_size(other).unwrap_or(1);
            format!("fixCodecSize(getBytesCodec(), {})", size)
        }
        other => format!("/* unknown: {} */", other),
    }
}

fn ts_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => primitive_ts_type(p),
        IdlType::Option { option } => format!("{} | null", ts_type(option)),
        IdlType::Defined { defined } => builtin_defined_primitive(&defined.name)
            .map(primitive_ts_type)
            .unwrap_or_else(|| defined.name.clone()),
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
        IdlType::Primitive(p) => primitive_ts_codec(p, target),
        IdlType::Option { option } => format!("getOptionCodec({})", ts_codec(option, target)),
        IdlType::Defined { defined } => builtin_defined_primitive(&defined.name)
            .map(|primitive| primitive_ts_codec(primitive, target))
            .unwrap_or_else(|| format!("{}Codec", defined.name)),
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
            IdlType::Defined { defined } => {
                if let Some(primitive) = builtin_defined_primitive(&defined.name) {
                    used.insert(primitive.to_string());
                }
            }
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

const WEB3JS_ADDRESS_CODEC_HELPER: &str = r#"function getWeb3jsAddressCodec() {
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

const TOTAL_DECODE_HELPERS: &str = r#"
const MAX_DECODE_ELEMENTS = 10 * 1024 * 1024;

function checkedLength(value: number | bigint): number {
  const result = Number(value);
  if (!Number.isSafeInteger(result) || result < 0) throw new Error("invalid length prefix");
  return result;
}

function checkedElementCount(value: number | bigint, remaining: number): number {
  const result = checkedLength(value);
  if (result > MAX_DECODE_ELEMENTS || result > remaining) {
    throw new Error("element count exceeds limit");
  }
  return result;
}

function checkedTake(data: Uint8Array, offset: number, size: number): Uint8Array {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(size) || offset < 0 || size < 0 || size > data.length - offset) {
    throw new Error("truncated input");
  }
  return data.slice(offset, offset + size);
}

function decodeUtf8(data: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: true }).decode(data);
}

function unwrapOption<T>(value: unknown): T | null {
  if (typeof value === "object" && value !== null && "__option" in value) {
    const option = value as { __option: string; value?: T };
    if (option.__option === "None") return null;
    if (option.__option === "Some") return option.value as T;
    throw new Error("invalid option tag");
  }
  return value as T | null;
}

function codecSize(
  codec: { encode(value: any): ArrayLike<number> },
  value: unknown,
): number {
  const fixedSize = (codec as { fixedSize?: unknown }).fixedSize;
  return typeof fixedSize === "number" ? fixedSize : codec.encode(value).length;
}

function bytesEqual(left: ArrayLike<number>, right: ArrayLike<number>): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}

function decodeExact<T>(
  codec: { decode(data: Uint8Array): unknown; encode(value: any): ArrayLike<number> },
  data: Uint8Array,
): T {
  const value = codec.decode(data);
  if (!bytesEqual(codec.encode(value), data)) throw new Error("invalid or trailing bytes");
  return value as T;
}

function assertFinished(data: Uint8Array, offset: number): void {
  if (offset !== data.length) throw new Error("trailing bytes");
}
"#;
