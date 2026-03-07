use std::collections::HashSet;

use crate::types::{Idl, IdlSeed, IdlType};

/// Generate a TypeScript client from the IDL.
pub fn generate_ts_client(idl: &Idl) -> String {
    let mut out = String::new();

    // --- Collect which codecs are actually used ---
    let used = collect_used_codecs(idl);
    let has_dyn_string = used.contains("dynString");
    let has_dyn_vec = used.contains("dynVec");
    let has_instructions = !idl.instructions.is_empty();

    // --- Imports ---
    if has_instructions {
        out.push_str("import { Buffer } from \"buffer\";\n");
    }
    out.push_str(
        "import { PublicKey as Address, TransactionInstruction } from \"@solana/web3.js\";\n",
    );

    // Build codec imports list
    let mut codec_imports: Vec<&str> = vec!["getStructCodec"];
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

    if used.contains("publicKey") {
        codec_imports.extend_from_slice(&["getBytesCodec", "fixCodecSize", "transformCodec"]);
    }

    if has_dyn_string {
        codec_imports.extend_from_slice(&[
            "addCodecSizePrefix",
            "fixCodecSize",
            "getU16Codec",
            "getUtf8Codec",
        ]);
    }

    if has_dyn_vec {
        codec_imports.extend_from_slice(&["fixCodecSize", "getArrayCodec", "getU16Codec"]);
    }

    codec_imports.sort();
    codec_imports.dedup();

    out.push_str(&format!(
        "import {{ {} }} from \"@solana/codecs\";\n",
        codec_imports.join(", ")
    ));
    if has_dyn_vec {
        out.push_str("import type { FixedSizeCodec } from \"@solana/codecs\";\n");
    }

    out.push('\n');

    // --- PublicKey codec helper ---
    if used.contains("publicKey") {
        out.push_str(PUBLIC_KEY_CODEC_HELPER);
        out.push('\n');
    }

    // --- DynString / DynVec helpers (only if used) ---
    if has_dyn_string {
        out.push_str(DYN_STRING_HELPER);
        out.push('\n');
    }
    if has_dyn_vec {
        out.push_str(DYN_VEC_HELPER);
        out.push('\n');
    }

    // === Constants ===
    out.push_str("/* Constants */\n");
    out.push_str(&format!(
        "export const PROGRAM_ADDRESS = new Address(\"{}\");\n",
        idl.address
    ));

    // Account discriminators
    for account in &idl.accounts {
        let const_name = pascal_to_screaming_snake(&account.name);
        let disc_str = format_disc_array(&account.discriminator);
        out.push_str(&format!(
            "export const {}_DISCRIMINATOR = new Uint8Array({});\n",
            const_name, disc_str
        ));
    }

    // Event discriminators
    for event in &idl.events {
        let const_name = pascal_to_screaming_snake(&event.name);
        let disc_str = format_disc_array(&event.discriminator);
        out.push_str(&format!(
            "export const {}_DISCRIMINATOR = new Uint8Array({});\n",
            const_name, disc_str
        ));
    }

    // Instruction discriminators
    for ix in &idl.instructions {
        let pascal = snake_to_pascal(&ix.name);
        let const_name = pascal_to_screaming_snake(&pascal);
        let disc_str = format_disc_array(&ix.discriminator);
        out.push_str(&format!(
            "export const {}_INSTRUCTION_DISCRIMINATOR = new Uint8Array({});\n",
            const_name, disc_str
        ));
    }

    out.push('\n');

    // === Interfaces ===
    out.push_str("/* Interfaces */\n");

    // Type interfaces
    for type_def in &idl.types {
        let name = &type_def.name;
        let fields = &type_def.ty.fields;
        out.push_str(&format!("export interface {} {{\n", name));
        for field in fields {
            out.push_str(&format!("  {}: {};\n", field.name, ts_type(&field.ty)));
        }
        out.push_str("}\n\n");
    }

    // Instruction args interfaces
    for ix in &idl.instructions {
        if ix.args.is_empty() {
            continue;
        }
        let pascal = snake_to_pascal(&ix.name);
        out.push_str(&format!("export interface {}InstructionArgs {{\n", pascal));
        for arg in &ix.args {
            out.push_str(&format!("  {}: {};\n", arg.name, ts_type(&arg.ty)));
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

        if user_accs.is_empty() && ix.args.is_empty() {
            continue;
        }

        let pascal = snake_to_pascal(&ix.name);

        out.push_str(&format!("export interface {pascal}InstructionInput {{\n"));

        if !user_accs.is_empty() {
            for acc in &user_accs {
                out.push_str(&format!("  {}: Address;\n", acc.name));
            }
        }
        if !ix.args.is_empty() {
            for arg in &ix.args {
                out.push_str(&format!("  {}: {};\n", arg.name, ts_type(&arg.ty)));
            }
        }

        out.push_str("}\n\n");
    }

    // === Codecs ===
    out.push_str("/* Codecs */\n");
    for type_def in &idl.types {
        let name = &type_def.name;
        let fields = &type_def.ty.fields;
        out.push_str(&format!("export const {}Codec = getStructCodec([\n", name));
        for field in fields {
            out.push_str(&format!(
                "  [\"{}\", {}],\n",
                field.name,
                ts_codec(&field.ty)
            ));
        }
        out.push_str("]);\n\n");
    }

    // === Enums ===
    out.push_str("/* Enums */\n");

    if !idl.events.is_empty() {
        out.push_str("export enum ProgramEvent {\n");
        for event in &idl.events {
            out.push_str(&format!("  {} = \"{}\",\n", event.name, event.name));
        }
        out.push_str("}\n\n");

        out.push_str("export type DecodedEvent =\n");
        for (i, event) in idl.events.iter().enumerate() {
            let has_type = idl.types.iter().any(|t| t.name == event.name);
            if has_type {
                out.push_str(&format!(
                    "  | {{ type: ProgramEvent.{}; data: {} }}",
                    event.name, event.name
                ));
            } else {
                out.push_str(&format!("  | {{ type: ProgramEvent.{} }}", event.name));
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
            out.push_str(&format!("  {} = \"{}\",\n", pascal, pascal));
        }
        out.push_str("}\n\n");

        out.push_str("export type DecodedInstruction =\n");
        for (i, ix) in idl.instructions.iter().enumerate() {
            let pascal = snake_to_pascal(&ix.name);
            if ix.args.is_empty() {
                out.push_str(&format!("  | {{ type: ProgramInstruction.{} }}", pascal));
            } else {
                out.push_str(&format!(
                    "  | {{ type: ProgramInstruction.{}; args: {}InstructionArgs }}",
                    pascal, pascal
                ));
            }
            if i < idl.instructions.len() - 1 {
                out.push('\n');
            }
        }
        out.push_str(";\n\n");
    }

    // === Client class ===
    out.push_str("/* Client */\n");
    let class_name = format!("{}Client", snake_to_pascal(&idl.metadata.name));
    out.push_str(&format!("export class {} {{\n", class_name));
    out.push_str("  constructor(readonly programId: Address = PROGRAM_ADDRESS) {}\n");

    // --- Account decoders ---
    for account in &idl.accounts {
        let name = &account.name;
        let const_name = pascal_to_screaming_snake(name);
        out.push('\n');
        out.push_str(&format!(
            "  decode{}(data: Uint8Array): {} {{\n",
            name, name
        ));
        out.push_str(&format!("    const disc = {}_DISCRIMINATOR;\n", const_name));
        out.push_str("    for (let i = 0; i < disc.length; i++) {\n");
        out.push_str(&format!(
            "      if (data[i] !== disc[i]) throw new Error(\"Invalid {} discriminator\");\n",
            name
        ));
        out.push_str("    }\n");
        out.push_str(&format!(
            "    return {}Codec.decode(data.slice(disc.length));\n",
            name
        ));
        out.push_str("  }\n");
    }

    // --- Event decoder ---
    if !idl.events.is_empty() {
        out.push('\n');
        out.push_str("  decodeEvent(data: Uint8Array): DecodedEvent | null {\n");
        for event in &idl.events {
            let has_type = idl.types.iter().any(|t| t.name == event.name);
            let const_name = format!("{}_DISCRIMINATOR", pascal_to_screaming_snake(&event.name));
            out.push_str(&format!(
                "    if (data.length >= {0}.length && {0}.every((b, i) => data[i] === b))\n",
                const_name
            ));
            if has_type {
                out.push_str(&format!(
                    "      return {{ type: ProgramEvent.{0}, data: {0}Codec.decode(data.slice({1}.length)) }};\n",
                    event.name, const_name
                ));
            } else {
                out.push_str(&format!(
                    "      return {{ type: ProgramEvent.{} }};\n",
                    event.name
                ));
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
                out.push_str(&format!(
                    "    if (data.length >= {0}.length && {0}.every((b, i) => data[i] === b))\n",
                    const_name
                ));
                out.push_str(&format!(
                    "      return {{ type: ProgramInstruction.{} }};\n",
                    pascal
                ));
            } else {
                out.push_str(&format!(
                    "    if (data.length >= {0}.length && {0}.every((b, i) => data[i] === b)) {{\n",
                    const_name
                ));
                out.push_str("      const argsCodec = getStructCodec([\n");
                for arg in &ix.args {
                    out.push_str(&format!(
                        "        [\"{}\", {}],\n",
                        arg.name,
                        ts_codec(&arg.ty)
                    ));
                }
                out.push_str("      ]);\n");
                out.push_str(&format!(
                    "      return {{ type: ProgramInstruction.{}, args: argsCodec.decode(data.slice({}.length)) }};\n",
                    pascal, const_name
                ));
                out.push_str("    }\n");
            }
        }
        out.push_str("    return null;\n");
        out.push_str("  }\n");
    }

    // --- Instruction builders ---
    for ix in &idl.instructions {
        out.push('\n');
        let pascal = snake_to_pascal(&ix.name);

        // Accounts used by instruction builders are handled in three categories:
        // - user-provided: passed by caller via InstructionInput
        // - fixed-address: resolved once and stored in accountsMap
        // - PDA: derived with findProgramAddressSync and stored in accountsMap
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

        // Map an account name to the generated TS expression that provides its value.
        // User accounts come from input.<name>; fixed/PDA accounts, when they exist,
        // come from accountsMap["name"].
        let account_expr = |name: &str| {
            if input_account_names.contains(name) {
                format!("input.{name}")
            } else {
                format!("accountsMap[\"{}\"]", name)
            }
        };

        // Method signature — interface arguments
        let input_param = if user_accs.is_empty() && ix.args.is_empty() {
            String::new()
        } else {
            format!("input: {pascal}InstructionInput")
        };
        out.push_str(&format!(
            "  create{pascal}Instruction({input_param}): TransactionInstruction {{\n"
        ));

        if has_non_input_accounts {
            out.push_str("    const accountsMap: Record<string, Address> = {};\n");
        }

        // Derive fixed-address accounts
        for acc in &ix.accounts {
            if let Some(addr) = &acc.address {
                out.push_str(&format!(
                    "    accountsMap[\"{}\"] = new Address(\"{}\");\n",
                    acc.name, addr
                ));
            }
        }

        // Derive PDA accounts
        for acc in &ix.accounts {
            if let Some(pda) = &acc.pda {
                out.push_str(&format!(
                    "    accountsMap[\"{}\"] = Address.findProgramAddressSync(\n      [\n",
                    acc.name
                ));
                for seed in &pda.seeds {
                    match seed {
                        IdlSeed::Const { value } => {
                            let bytes: Vec<String> = value.iter().map(|b| b.to_string()).collect();
                            out.push_str(&format!(
                                "        new Uint8Array([{}]),\n",
                                bytes.join(", ")
                            ));
                        }
                        IdlSeed::Account { path } => {
                            out.push_str(&format!("        {}.toBytes(),\n", account_expr(path)));
                        }
                    }
                }
                out.push_str("      ],\n      this.programId,\n    )[0];\n");
            }
        }

        // Encode instruction data
        let disc_bytes: Vec<String> = ix.discriminator.iter().map(|b| b.to_string()).collect();
        if ix.args.is_empty() {
            out.push_str(&format!(
                "    const data = Buffer.from([{}]);\n",
                disc_bytes.join(", ")
            ));
        } else {
            out.push_str("    const argsCodec = getStructCodec([\n");
            for arg in &ix.args {
                out.push_str(&format!(
                    "      [\"{}\", {}],\n",
                    arg.name,
                    ts_codec(&arg.ty)
                ));
            }
            out.push_str("    ]);\n");
            let arg_names: Vec<String> = ix
                .args
                .iter()
                .map(|a| format!("{}: input.{}", a.name, a.name))
                .collect();
            out.push_str(&format!(
                "    const data = Buffer.from([{}, ...argsCodec.encode({{ {} }})]);\n",
                disc_bytes.join(", "),
                arg_names.join(", ")
            ));
        }

        // Return TransactionInstruction
        out.push_str("    return new TransactionInstruction({\n");
        out.push_str("      programId: this.programId,\n");
        if !ix.accounts.is_empty() {
            out.push_str("      keys: [\n");
            for acc in &ix.accounts {
                let pubkey_expr = account_expr(&acc.name);
                out.push_str(&format!(
                    "        {{ pubkey: {}, isSigner: {}, isWritable: {} }},\n",
                    pubkey_expr, acc.signer, acc.writable
                ));
            }
            out.push_str("      ],\n");
        }
        out.push_str("      data,\n");
        out.push_str("    });\n");
        out.push_str("  }\n");
    }

    out.push_str("}\n\n");

    // === Errors ===
    if !idl.errors.is_empty() {
        out.push_str("/* Errors */\n");
        out.push_str(
            "export const PROGRAM_ERRORS: Record<number, { name: string; msg?: string }> = {\n",
        );
        for err in &idl.errors {
            match &err.msg {
                Some(msg) => {
                    out.push_str(&format!(
                        "  {}: {{ name: \"{}\", msg: \"{}\" }},\n",
                        err.code, err.name, msg
                    ));
                }
                None => {
                    out.push_str(&format!("  {}: {{ name: \"{}\" }},\n", err.code, err.name));
                }
            }
        }
        out.push_str("};\n\n");
    }

    out
}

fn ts_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "u8" | "u16" | "u32" | "i8" | "i16" | "i32" => "number".to_string(),
            "u64" | "u128" | "i64" | "i128" => "bigint".to_string(),
            "bool" => "boolean".to_string(),
            "publicKey" => "Address".to_string(),
            other => other.to_string(),
        },
        IdlType::Defined { defined } => defined.clone(),
        IdlType::DynString { .. } => "string".to_string(),
        IdlType::DynVec { vec } => format!("Array<{}>", ts_type(&vec.items)),
    }
}

fn ts_codec(ty: &IdlType) -> String {
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
            "publicKey" => "getPublicKeyCodec()".to_string(),
            other => format!("/* unknown: {} */", other),
        },
        IdlType::Defined { defined } => format!("{}Codec", defined),
        IdlType::DynString { string } => {
            format!("getDynStringCodec({})", string.max_length)
        }
        IdlType::DynVec { vec } => {
            format!(
                "getDynVecCodec({}, {})",
                ts_codec(&vec.items),
                vec.max_length
            )
        }
    }
}

fn collect_used_codecs(idl: &Idl) -> HashSet<String> {
    let mut used = HashSet::new();

    let mut visit = |ty: &IdlType| match ty {
        IdlType::Primitive(p) => {
            used.insert(p.clone());
        }
        IdlType::Defined { .. } => {}
        IdlType::DynString { .. } => {
            used.insert("dynString".to_string());
        }
        IdlType::DynVec { .. } => {
            used.insert("dynVec".to_string());
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
    if let IdlType::DynVec { vec } = ty {
        visit_type(&vec.items, visit);
    }
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.collect::<String>(),
            }
        })
        .collect()
}

fn pascal_to_screaming_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

fn format_disc_array(disc: &[u8]) -> String {
    let bytes: Vec<String> = disc.iter().map(|b| b.to_string()).collect();
    format!("[{}]", bytes.join(", "))
}

const PUBLIC_KEY_CODEC_HELPER: &str = r#"function getPublicKeyCodec() {
  return transformCodec(
    fixCodecSize(getBytesCodec(), 32),
    (value: Address) => value.toBytes(),
    bytes => new Address(bytes),
  );
}
"#;

const DYN_STRING_HELPER: &str = r#"function getDynStringCodec(maxLength: number) {
  return fixCodecSize(
    addCodecSizePrefix(getUtf8Codec(), getU16Codec()),
      2 + maxLength,
  );
}
"#;

const DYN_VEC_HELPER: &str = r#"function getDynVecCodec<TFrom, TTo extends TFrom = TFrom>(
  itemCodec: FixedSizeCodec<TFrom, TTo>,
  maxLength: number,
) {
  return fixCodecSize(
    getArrayCodec(itemCodec, { size: getU16Codec() }),
    2 + maxLength * itemCodec.fixedSize,
  );
}
"#;
