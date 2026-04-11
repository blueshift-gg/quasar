use {
    crate::types::{Idl, IdlSeed, IdlType},
    std::{collections::HashSet, fmt::Write},
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

fn generate_ts(idl: &Idl, target: TsTarget) -> String {
    let mut out = String::new();

    // --- Collect which codecs are actually used ---
    let used = collect_used_codecs(idl);
    let has_dyn_string = used.contains("dynString");
    let has_dyn_vec = used.contains("dynVec");
    let has_instructions = !idl.instructions.is_empty();
    let has_public_key = used.contains("publicKey");

    // Check if any instruction uses PDAs or PDA account seeds
    let has_pdas = idl
        .instructions
        .iter()
        .any(|ix| ix.accounts.iter().any(|a| a.pda.is_some()));
    let has_pda_account_seeds = idl.instructions.iter().any(|ix| {
        ix.accounts.iter().any(|a| {
            a.pda.as_ref().is_some_and(|pda| {
                pda.seeds
                    .iter()
                    .any(|s| matches!(s, IdlSeed::Account { .. }))
            })
        })
    });

    // --- Imports ---
    match target {
        TsTarget::Web3js => {
            if has_instructions {
                out.push_str("import { Buffer } from \"buffer\";\n");
            }
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
            writeln!(
                out,
                "import {{ {} }} from \"@solana/kit\";",
                kit_imports.join(", ")
            )
            .expect("write to String");
        }
    }

    // Build codec imports list
    let has_struct_codec =
        !idl.types.is_empty() || idl.instructions.iter().any(|ix| !ix.args.is_empty());
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
        let disc_str = format_disc_array(&account.discriminator);
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
        let disc_str = format_disc_array(&event.discriminator);
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
        let disc_str = format_disc_array(&ix.discriminator);
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
    let class_name = format!("{}Client", snake_to_pascal(&idl.metadata.name));
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
        TsTarget::Web3js => generate_instruction_builders_web3js(&mut out, idl),
        TsTarget::Kit => generate_instruction_builders_kit(&mut out, idl),
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

fn generate_instruction_builders_web3js(out: &mut String, idl: &Idl) {
    let class_name = format!("{}Client", snake_to_pascal(&idl.metadata.name));
    for ix in &idl.instructions {
        out.push('\n');
        let pascal = snake_to_pascal(&ix.name);

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
                write!(
                    out,
                    "    accountsMap[\"{}\"] = Address.findProgramAddressSync(\n      [\n",
                    acc.name
                )
                .expect("write to String");
                for seed in &pda.seeds {
                    match seed {
                        IdlSeed::Const { value } => {
                            write_byte_array(out, value);
                        }
                        IdlSeed::Account { path } => {
                            writeln!(out, "        {}.toBytes(),", account_expr(path))
                                .expect("write to String");
                        }
                        IdlSeed::Arg { path } => {
                            writeln!(out, "        input.{},  // arg seed", path)
                                .expect("write to String");
                        }
                    }
                }
                write!(out, "      ],\n      {class_name}.programId,\n    )[0];\n")
                    .expect("write to String");
            }
        }

        // Encode instruction data
        let disc_str = format_disc_list(&ix.discriminator);
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

fn generate_instruction_builders_kit(out: &mut String, idl: &Idl) {
    for ix in &idl.instructions {
        out.push('\n');
        let pascal = snake_to_pascal(&ix.name);

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
                write!(
                    out,
                    "    accountsMap[\"{}\"] = (await getProgramDerivedAddress({{\n      \
                     programAddress: PROGRAM_ADDRESS,\n      seeds: [\n",
                    acc.name
                )
                .expect("write to String");
                for seed in &pda.seeds {
                    match seed {
                        IdlSeed::Const { value } => {
                            write_byte_array(out, value);
                        }
                        IdlSeed::Account { path } => {
                            writeln!(
                                out,
                                "        getAddressCodec().encode({}),",
                                account_expr(path)
                            )
                            .expect("write to String");
                        }
                        IdlSeed::Arg { path } => {
                            writeln!(out, "        input.{},  // arg seed", path)
                                .expect("write to String");
                        }
                    }
                }
                out.push_str("      ],\n    }))[0];\n");
            }
        }

        // Encode instruction data
        let disc_str = format_disc_list(&ix.discriminator);
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

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn ts_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "u8" | "u16" | "u32" | "i8" | "i16" | "i32" => "number".to_string(),
            "u64" | "u128" | "i64" | "i128" => "bigint".to_string(),
            "bool" => "boolean".to_string(),
            "publicKey" => "Address".to_string(),
            other if other.starts_with('[') => "Uint8Array".to_string(),
            other => other.to_string(),
        },
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
            "publicKey" => match target {
                TsTarget::Web3js => "getPublicKeyCodec()".to_string(),
                TsTarget::Kit => "getAddressCodec()".to_string(),
            },
            other if other.starts_with('[') => {
                let size = parse_fixed_array_size(other).unwrap_or(0);
                format!("fixCodecSize(getBytesCodec(), {})", size)
            }
            other => format!("/* unknown: {} */", other),
        },
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
        _ => "u32",
    }
}

/// Map prefix byte width to the corresponding TS codec expression.
fn prefix_codec(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "getU8Codec()",
        2 => "getU16Codec()",
        _ => "getU32Codec()",
    }
}

fn collect_used_codecs(idl: &Idl) -> HashSet<String> {
    let mut used = HashSet::new();

    let mut visit = |ty: &IdlType| match ty {
        IdlType::Primitive(p) => {
            used.insert(p.clone());
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
    if let IdlType::DynVec { vec } = ty {
        visit_type(&vec.items, visit);
    }
}

/// Parse the size from a fixed-size array primitive like "[u8; 8]" → 8.
fn parse_fixed_array_size(p: &str) -> Option<usize> {
    let inner = p.strip_prefix('[')?.strip_suffix(']')?;
    let (_, size_str) = inner.split_once(';')?;
    size_str.trim().parse().ok()
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
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

fn format_disc_array(disc: &[u8]) -> String {
    let mut s = String::with_capacity(disc.len() * 4 + 2);
    s.push('[');
    for (i, b) in disc.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write!(s, "{}", b).expect("write to String");
    }
    s.push(']');
    s
}

/// Format discriminator bytes as a comma-separated list (no brackets).
fn format_disc_list(disc: &[u8]) -> String {
    let mut s = String::with_capacity(disc.len() * 4);
    for (i, b) in disc.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write!(s, "{}", b).expect("write to String");
    }
    s
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
