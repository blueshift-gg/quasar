use {
    super::model::{
        account_field_definition, account_field_seed_inputs, python_field_path, reject_generics,
        resolved_account_order, resolver_is_derived, CodegenResult, ProgramModel,
    },
    crate::codegen::naming::{camel_to_snake, snake_to_pascal, to_screaming_snake},
    crate::types::{
        Idl, IdlAccountNode, IdlArg, IdlCodec, IdlFieldDef, IdlPdaProgram, IdlPdaSeed, IdlResolver,
        IdlType, IdlTypeDef,
    },
    std::fmt::Write,
};

/// Generate a Python client module from the IDL.
///
/// Uses `solders` for Solana types (Pubkey, Instruction, AccountMeta)
/// and `struct` for binary serialization.
pub fn generate_python_client(idl: &Idl) -> CodegenResult<String> {
    let model = ProgramModel::try_new(idl)?;
    reject_generics(idl, "Python")?;
    let mut out = String::new();

    // Module docstring
    writeln!(
        out,
        r#""""Generated client for the {} program.""""#,
        model.identity.program_name
    )
    .unwrap();
    out.push_str("from __future__ import annotations\n\n");

    // Imports
    out.push_str("import struct\n");
    out.push_str("from dataclasses import dataclass\n");

    let has_events = model.features.has_events;
    let has_args = model.features.has_args;
    let has_optional = model.features.has_option;
    let has_dynamic = model.features.has_dynamic;

    if has_events || has_args || has_dynamic || has_optional {
        out.push_str("from typing import Optional\n");
    }

    out.push_str("\nfrom solders.pubkey import Pubkey\n");
    out.push_str("from solders.instruction import Instruction, AccountMeta\n\n");
    out.push_str(
        "class DecodeError(ValueError):\n\x20   pass\n\n_MAX_DECODE_ELEMENTS = 10 * 1024 * \
         1024\n\ndef _take(data: bytes, offset: int, size: int) -> tuple[bytes, int]:\n\x20   if \
         size < 0 or offset < 0 or size > len(data) - offset:\n\x20       raise \
         DecodeError(\"truncated input\")\n\x20   end = offset + size\n\x20   return \
         data[offset:end], end\n\ndef _unpack(fmt: str, data: bytes, offset: int) -> \
         tuple[object, int]:\n\x20   raw, offset = _take(data, offset, \
         struct.calcsize(fmt))\n\x20   return struct.unpack(fmt, raw)[0], offset\n\ndef \
         _finish(data: bytes, offset: int) -> None:\n\x20   if offset != len(data):\n\x20       \
         raise DecodeError(\"trailing bytes\")\n\n",
    );

    // Program ID
    writeln!(
        out,
        "PROGRAM_ID = Pubkey.from_string(\"{}\")\n",
        idl.address
    )
    .unwrap();

    // Discriminator constants
    for ix in &idl.instructions {
        let const_name = to_screaming_snake(&ix.name);
        writeln!(
            out,
            "{}_DISCRIMINATOR = bytes([{}])",
            const_name,
            super::format_disc_decimal(&ix.discriminator)
        )
        .unwrap();
    }
    if !idl.instructions.is_empty() {
        out.push('\n');
    }

    // Account discriminators
    for acc in &idl.accounts {
        let const_name = to_screaming_snake(&acc.name);
        writeln!(
            out,
            "{}_ACCOUNT_DISCRIMINATOR = bytes([{}])",
            const_name,
            super::format_disc_decimal(&acc.discriminator)
        )
        .unwrap();
    }
    if !idl.accounts.is_empty() {
        out.push('\n');
    }

    // Event discriminators
    for ev in &idl.events {
        let const_name = to_screaming_snake(&ev.name);
        writeln!(
            out,
            "{}_EVENT_DISCRIMINATOR = bytes([{}])",
            const_name,
            super::format_disc_decimal(&ev.discriminator)
        )
        .unwrap();
    }
    if !idl.events.is_empty() {
        out.push('\n');
    }

    // Type definitions (dataclasses)
    for type_def in &idl.types {
        writeln!(out, "\n@dataclass").unwrap();
        writeln!(out, "class {}:", type_def.name).unwrap();
        if type_def.fields.is_empty() {
            out.push_str("    pass\n");
        } else {
            for field in &type_def.fields {
                writeln!(
                    out,
                    "    {}: {}",
                    camel_to_snake(&field.name),
                    python_type(&field.ty)
                )
                .unwrap();
            }
        }
        out.push('\n');

        // Decode classmethod
        if !type_def.fields.is_empty() {
            writeln!(out, "    @classmethod").unwrap();
            writeln!(
                out,
                "    def decode(cls, data: bytes) -> {}:",
                type_def.name
            )
            .unwrap();
            out.push_str("        offset = 0\n");
            let fixed_fields: Vec<_> = type_def
                .fields
                .iter()
                .filter(|field| !is_dynamic_field(field))
                .collect();
            let dynamic_fields: Vec<_> = type_def
                .fields
                .iter()
                .filter(|field| is_dynamic_field(field))
                .collect();
            for field in fixed_fields {
                out.push_str(&decode_field_expr(
                    &camel_to_snake(&field.name),
                    &field.ty,
                    field.codec.as_ref(),
                    8,
                    &idl.types,
                ));
            }
            for field in &dynamic_fields {
                out.push_str(&decode_dynamic_header(
                    &camel_to_snake(&field.name),
                    &field.ty,
                    field.codec.as_ref(),
                    8,
                ));
            }
            for field in dynamic_fields {
                out.push_str(&decode_dynamic_tail(
                    &camel_to_snake(&field.name),
                    &field.ty,
                    field.codec.as_ref(),
                    8,
                    &idl.types,
                ));
            }
            out.push_str("        _finish(data, offset)\n");
            let field_names: Vec<String> = type_def
                .fields
                .iter()
                .map(|f| {
                    let snake = camel_to_snake(&f.name);
                    format!("{}={}", snake, snake)
                })
                .collect();
            writeln!(out, "        return cls({})", field_names.join(", ")).unwrap();
            out.push('\n');
        }
    }

    // Instruction input dataclasses + builder functions
    for ix in &idl.instructions {
        let class_name = snake_to_pascal(&ix.name);
        let fn_name = camel_to_snake(&ix.name);

        // Input dataclass
        writeln!(out, "\n@dataclass").unwrap();
        writeln!(out, "class {}Input:", class_name).unwrap();

        // Required account fields. Optional accounts are emitted after every
        // required field so the generated dataclass never places a defaulted
        // field before a required account, PDA seed input, or instruction arg.
        let mut has_any_fields = false;
        for acc in &ix.accounts {
            if acc.optional {
                continue;
            }
            if matches!(acc.resolver, IdlResolver::Const { .. }) {
                continue; // Known addresses are auto-filled
            }
            if matches!(
                acc.resolver,
                IdlResolver::Pda { .. } | IdlResolver::AssociatedToken { .. }
            ) {
                continue; // Derived addresses are filled by the client
            }
            writeln!(out, "    {}: Pubkey", camel_to_snake(&acc.name)).unwrap();
            has_any_fields = true;
        }

        for seed in account_field_seed_inputs(ix) {
            let Some(field) = account_field_definition(idl, seed.account, seed.field) else {
                continue;
            };
            writeln!(
                out,
                "    {}: {}",
                account_field_seed_input_name(seed.path, seed.field),
                python_type(&field.ty)
            )
            .unwrap();
            has_any_fields = true;
        }

        // Arg fields
        for arg in &ix.args {
            writeln!(
                out,
                "    {}: {}",
                camel_to_snake(&arg.name),
                python_type(&arg.ty)
            )
            .unwrap();
            has_any_fields = true;
        }

        // Optional accounts are always caller-controlled, even when the IDL
        // carries a Const/PDA resolver. `None` selects the program-id sentinel;
        // callers that want the resolved address pass it explicitly.
        for acc in ix.accounts.iter().filter(|acc| acc.optional) {
            writeln!(
                out,
                "    {}: Optional[Pubkey] = None",
                camel_to_snake(&acc.name)
            )
            .unwrap();
            has_any_fields = true;
        }

        // Remaining accounts
        if ix.remaining_accounts.is_some() {
            out.push_str("    remaining_accounts: list[AccountMeta] = None\n");
            has_any_fields = true;
        }

        if !has_any_fields {
            out.push_str("    pass\n");
        }
        out.push('\n');

        // Builder function
        writeln!(
            out,
            "\ndef create_{}_instruction(input: {}Input) -> Instruction:",
            fn_name, class_name
        )
        .unwrap();

        out.push_str("    accounts_map = {}\n");

        // Resolve addresses independently from account-meta order: derived
        // accounts may depend on fields declared later in the IDL.
        out.push_str("    accounts = []\n");
        for acc in ix
            .accounts
            .iter()
            .filter(|acc| acc.optional || !resolver_is_derived(&acc.resolver))
        {
            let key_expr = python_account_key_expr(acc, idl);
            writeln!(out, "    accounts_map[\"{}\"] = {}", acc.name, key_expr).unwrap();
        }
        for acc in resolved_account_order(ix)? {
            let key_expr = python_account_key_expr(acc, idl);
            writeln!(out, "    accounts_map[\"{}\"] = {}", acc.name, key_expr).unwrap();
        }
        for acc in &ix.accounts {
            writeln!(
                out,
                "    accounts.append(AccountMeta(accounts_map[\"{}\"], is_signer={}, \
                 is_writable={}))",
                acc.name,
                py_bool(acc.signer.is_true()),
                py_bool(acc.writable.is_true()),
            )
            .unwrap();
        }

        if ix.remaining_accounts.is_some() {
            out.push_str(
                "    if input.remaining_accounts:\n        \
                 accounts.extend(input.remaining_accounts)\n",
            );
        }

        // Compact wire format:
        //   [disc][fixed fields][all dynamic prefixes][all dynamic data]
        let const_name = to_screaming_snake(&ix.name);
        let has_dyn = ix.args.iter().any(is_direct_dynamic);
        if ix.args.is_empty() {
            writeln!(out, "    data = {}_DISCRIMINATOR", const_name).unwrap();
        } else if !has_dyn {
            // Fixed-only path: simple inline serialisation.
            writeln!(out, "    data = bytearray({}_DISCRIMINATOR)", const_name).unwrap();
            for arg in &ix.args {
                out.push_str(&serialize_field_expr(
                    &camel_to_snake(&arg.name),
                    &arg.ty,
                    arg.codec.as_ref(),
                    &idl.types,
                ));
            }
            out.push_str("    data = bytes(data)\n");
        } else {
            // Compact 3-phase encoding.
            let fixed_args: Vec<_> = ix.args.iter().filter(|a| !is_direct_dynamic(a)).collect();
            let dyn_args: Vec<_> = ix.args.iter().filter(|a| is_direct_dynamic(a)).collect();

            writeln!(out, "    data = bytearray({}_DISCRIMINATOR)", const_name).unwrap();

            // Phase 1: fixed fields
            for arg in &fixed_args {
                out.push_str(&serialize_field_expr(
                    &camel_to_snake(&arg.name),
                    &arg.ty,
                    arg.codec.as_ref(),
                    &idl.types,
                ));
            }

            // Pre-encode dynamic bytes and group all length prefixes.
            for arg in &dyn_args {
                let name = camel_to_snake(&arg.name);
                let prefix_bytes = arg.codec.as_ref().map(|c| c.prefix_bytes()).unwrap_or(2);
                let (fmt, _sz) = prefix_fmt(prefix_bytes);
                if is_optional_dynamic(&arg.ty) {
                    writeln!(
                        out,
                        "    data.append(0 if input.{name} is None else 1)",
                        name = name,
                    )
                    .unwrap();
                    continue;
                }
                match dynamic_payload_type(&arg.ty).expect("dynamic arg payload") {
                    IdlType::Primitive(p) if p == "string" => {
                        writeln!(
                            out,
                            "    _{name}_b = input.{name}.encode(\"utf-8\")",
                            name = name,
                        )
                        .unwrap();
                        writeln!(
                            out,
                            "    data += struct.pack(\"<{fmt}\", len(_{name}_b))",
                            name = name,
                            fmt = fmt,
                        )
                        .unwrap();
                    }
                    IdlType::Vec { .. } => {
                        writeln!(
                            out,
                            "    data += struct.pack(\"<{fmt}\", len(input.{name}))",
                            name = name,
                            fmt = fmt,
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
            }

            // Phase 3: tail data
            for arg in &dyn_args {
                let name = camel_to_snake(&arg.name);
                let prefix_bytes = arg.codec.as_ref().map(|c| c.prefix_bytes()).unwrap_or(2);
                let (fmt, _sz) = prefix_fmt(prefix_bytes);
                let payload_ty = dynamic_payload_type(&arg.ty).expect("dynamic arg payload");
                let optional = is_optional_dynamic(&arg.ty);
                if optional {
                    writeln!(out, "    if input.{name} is not None:", name = name).unwrap();
                }
                let pad = if optional { "        " } else { "    " };
                let value = format!("input.{name}");
                match payload_ty {
                    IdlType::Primitive(p) if p == "string" => {
                        if optional {
                            writeln!(
                                out,
                                "{pad}_{name}_b = {value}.encode(\"utf-8\")",
                                pad = pad,
                                name = name,
                                value = value,
                            )
                            .unwrap();
                            writeln!(
                                out,
                                "{pad}data += struct.pack(\"<{fmt}\", len(_{name}_b))",
                                pad = pad,
                                name = name,
                                fmt = fmt,
                            )
                            .unwrap();
                        }
                        writeln!(out, "{pad}data += _{name}_b", pad = pad, name = name).unwrap();
                    }
                    IdlType::Vec { vec } => {
                        if optional {
                            writeln!(
                                out,
                                "{pad}data += struct.pack(\"<{fmt}\", len({value}))",
                                pad = pad,
                                fmt = fmt,
                                value = value,
                            )
                            .unwrap();
                        }
                        let item_ser = python_vec_item_expr(vec, "item");
                        writeln!(
                            out,
                            "{pad}for item in {value}:\n{pad}    data += {ser}",
                            pad = pad,
                            value = value,
                            ser = item_ser,
                        )
                        .unwrap();
                    }
                    _ => unreachable!(),
                }
            }

            out.push_str("    data = bytes(data)\n");
        }

        out.push_str("    return Instruction(PROGRAM_ID, data, accounts)\n\n");
    }

    // Event decoder
    if has_events {
        // Event dataclasses are already generated via type definitions above,
        // but we need a decode_event function
        out.push_str("\ndef decode_event(data: bytes) -> Optional[tuple[str, object]]:\n");
        out.push_str(
            "    \"\"\"Decode an event from raw log data. Returns (event_name, event_data) or \
             None.\"\"\"\n",
        );
        for ev in &idl.events {
            let const_name = to_screaming_snake(&ev.name);
            let type_def = idl.types.iter().find(|t| t.name == ev.name);
            writeln!(
                out,
                "    if data[:{disc_len}] == {const_name}_EVENT_DISCRIMINATOR:",
                disc_len = ev.discriminator.len(),
                const_name = const_name,
            )
            .unwrap();
            if let Some(td) = type_def {
                if td.fields.is_empty() {
                    writeln!(out, "        return (\"{}\", None)", ev.name).unwrap();
                } else {
                    writeln!(
                        out,
                        "        return (\"{}\", {}.decode(data[{}:]))",
                        ev.name,
                        ev.name,
                        ev.discriminator.len()
                    )
                    .unwrap();
                }
            } else {
                writeln!(out, "        return (\"{}\", None)", ev.name).unwrap();
            }
        }
        out.push_str("    return None\n\n");
    }

    // Client class (convenience wrapper)
    let pascal_name = snake_to_pascal(&model.identity.program_name);
    writeln!(out, "\nclass {}Client:", pascal_name).unwrap();
    writeln!(out, "    program_id = PROGRAM_ID\n").unwrap();

    if idl.instructions.is_empty() && idl.events.is_empty() {
        out.push_str("    pass\n");
    }

    for ix in &idl.instructions {
        let fn_name = camel_to_snake(&ix.name);
        let class_name = snake_to_pascal(&ix.name);
        writeln!(out, "    @staticmethod").unwrap();
        writeln!(
            out,
            "    def {}(input: {}Input) -> Instruction:",
            fn_name, class_name
        )
        .unwrap();
        writeln!(out, "        return create_{}_instruction(input)", fn_name).unwrap();
        out.push('\n');
    }

    if has_events {
        out.push_str("    @staticmethod\n");
        out.push_str("    def decode_event(data: bytes) -> Optional[tuple[str, object]]:\n");
        out.push_str("        return decode_event(data)\n\n");
    }

    Ok(out)
}

fn python_account_key_expr(account: &IdlAccountNode, idl: &Idl) -> String {
    if account.optional {
        let name = camel_to_snake(&account.name);
        return format!("input.{name} if input.{name} is not None else PROGRAM_ID");
    }

    match &account.resolver {
        IdlResolver::Const { address } => format!("Pubkey.from_string(\"{address}\")"),
        IdlResolver::Pda { program, seeds } => {
            let seed_exprs = seeds
                .iter()
                .map(|seed| match seed {
                    IdlPdaSeed::Const { value } => {
                        format!("bytes([{}])", super::format_disc_decimal(value))
                    }
                    IdlPdaSeed::Account { path } => {
                        format!("bytes(accounts_map[\"{path}\"])")
                    }
                    IdlPdaSeed::AccountField {
                        path,
                        field,
                        account,
                    } => {
                        let ty =
                            account_field_definition(idl, account, field).map(|field| &field.ty);
                        python_pda_seed_expr(
                            &format!("input.{}", account_field_seed_input_name(path, field)),
                            ty,
                        )
                    }
                    IdlPdaSeed::Arg { path, ty } => python_pda_seed_expr(
                        &format!("input.{}", python_field_path(path)),
                        Some(ty),
                    ),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let program = match program {
                IdlPdaProgram::ProgramId {} => "PROGRAM_ID".to_string(),
                IdlPdaProgram::Account { path } => format!("accounts_map[\"{path}\"]"),
            };
            format!("Pubkey.find_program_address([{seed_exprs}], {program})[0]")
        }
        IdlResolver::AssociatedToken {
            mint,
            owner,
            token_program,
        } => {
            let token_program = token_program.as_ref().map_or_else(
                || {
                    "Pubkey.from_string(\"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA\")"
                        .to_string()
                },
                |path| format!("accounts_map[\"{path}\"]"),
            );
            format!(
                "Pubkey.find_program_address([bytes(accounts_map[\"{owner}\"]), \
                 bytes({token_program}), bytes(accounts_map[\"{mint}\"])], \
                 Pubkey.from_string(\"ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL\"))[0]"
            )
        }
        _ => format!("input.{}", camel_to_snake(&account.name)),
    }
}

fn python_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "bool" => "bool".to_string(),
            "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" => {
                "int".to_string()
            }
            "f32" | "f64" => "float".to_string(),
            "pubkey" => "Pubkey".to_string(),
            "string" => "str".to_string(),
            _ => "bytes".to_string(),
        },
        IdlType::Option { option } => format!("Optional[{}]", python_type(option)),
        IdlType::Vec { vec } => format!("list[{}]", python_type(vec)),
        IdlType::Array { .. } => "bytes".to_string(),
        IdlType::Defined { defined } => defined.name.clone(),
        IdlType::Generic { generic } => {
            panic!("Generic type '{}' not supported in Python codegen", generic)
        }
    }
}

/// Returns `true` if the arg is a top-level dynamic type (string with codec or
/// Vec with codec). These require compact 3-phase encoding at the instruction
/// level.
fn is_direct_dynamic(arg: &IdlArg) -> bool {
    arg.codec.is_some() && dynamic_payload_type(&arg.ty).is_some()
}

fn is_dynamic_field(field: &IdlFieldDef) -> bool {
    field.codec.is_some() && dynamic_payload_type(&field.ty).is_some()
}

fn dynamic_payload_type(ty: &IdlType) -> Option<&IdlType> {
    match ty {
        IdlType::Primitive(p) if p == "string" => Some(ty),
        IdlType::Vec { .. } => Some(ty),
        IdlType::Option { option } => dynamic_payload_type(option),
        _ => None,
    }
}

fn is_optional_dynamic(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Option { option } if dynamic_payload_type(option).is_some())
}

fn python_vec_item_expr(item: &IdlType, source: &str) -> String {
    match item {
        IdlType::Primitive(p) if p == "pubkey" => format!("bytes({source})"),
        IdlType::Primitive(p) => {
            let f = struct_format(p);
            format!("struct.pack(\"<{}\", {source})", f)
        }
        _ => source.to_string(),
    }
}

fn serialize_field_expr(
    name: &str,
    ty: &IdlType,
    codec: Option<&IdlCodec>,
    types: &[IdlTypeDef],
) -> String {
    // Handle dynamic string with codec
    if let IdlType::Primitive(p) = ty {
        if p == "string" {
            if let Some(c) = codec {
                let (fmt, _sz) = prefix_fmt(c.prefix_bytes());
                return format!(
                    "    _b = input.{n}.encode(\"utf-8\")\n    data += struct.pack(\"<{fmt}\", \
                     len(_b))\n    data += _b\n",
                    n = name,
                    fmt = fmt,
                );
            }
        }
    }

    // Handle Vec with codec
    if let IdlType::Vec { ref vec } = ty {
        if let Some(c) = codec {
            let (fmt, _sz) = prefix_fmt(c.prefix_bytes());
            let item_ser = match &**vec {
                IdlType::Primitive(p) if p == "pubkey" => "bytes(item)".to_string(),
                IdlType::Primitive(p) => {
                    let f = struct_format(p);
                    format!("struct.pack(\"<{}\", item)", f)
                }
                _ => "item".to_string(),
            };
            return format!(
                "    data += struct.pack(\"<{fmt}\", len(input.{n}))\n    for item in \
                 input.{n}:\n        data += {ser}\n",
                n = name,
                fmt = fmt,
                ser = item_ser,
            );
        }
    }

    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "bool" => format!("    data += struct.pack(\"<?\", input.{})\n", name),
            "u8" => format!("    data += struct.pack(\"<B\", input.{})\n", name),
            "i8" => format!("    data += struct.pack(\"<b\", input.{})\n", name),
            "u16" => format!("    data += struct.pack(\"<H\", input.{})\n", name),
            "i16" => format!("    data += struct.pack(\"<h\", input.{})\n", name),
            "u32" => format!("    data += struct.pack(\"<I\", input.{})\n", name),
            "i32" => format!("    data += struct.pack(\"<i\", input.{})\n", name),
            "u64" => format!("    data += struct.pack(\"<Q\", input.{})\n", name),
            "i64" => format!("    data += struct.pack(\"<q\", input.{})\n", name),
            "u128" => format!(
                "    data += input.{n}.to_bytes(16, byteorder=\"little\")\n",
                n = name,
            ),
            "i128" => format!(
                "    data += input.{n}.to_bytes(16, byteorder=\"little\", signed=True)\n",
                n = name,
            ),
            "f32" => format!("    data += struct.pack(\"<f\", input.{})\n", name),
            "f64" => format!("    data += struct.pack(\"<d\", input.{})\n", name),
            "pubkey" => format!("    data += bytes(input.{})\n", name),
            "string" => {
                // Plain string without codec uses a Borsh-style u32 prefix.
                format!(
                    "    _b = input.{n}.encode(\"utf-8\")\n    data += struct.pack(\"<I\", \
                     len(_b))\n    data += _b\n",
                    n = name,
                )
            }
            _ => format!("    data += input.{}  # unsupported\n", name),
        },
        IdlType::Option { option } => {
            let inner = serialize_field_expr(&format!("{}_val", name), option, None, types);
            format!(
                "    if input.{n} is None:\n        data += b'\\x00'\n    else:\n        data += \
                 b'\\x01'\n        {n}_val = input.{n}\n{inner}",
                n = name,
                inner = inner.replace("    data", "        data"),
            )
        }
        IdlType::Vec { vec } => {
            // Vec without codec uses a Borsh-style u32 prefix.
            let item_ser = match &**vec {
                IdlType::Primitive(p) if p == "pubkey" => "bytes(item)".to_string(),
                IdlType::Primitive(p) => {
                    let f = struct_format(p);
                    format!("struct.pack(\"<{}\", item)", f)
                }
                _ => "item".to_string(),
            };
            format!(
                "    data += struct.pack(\"<I\", len(input.{n}))\n    for item in \
                 input.{n}:\n        data += {ser}\n",
                n = name,
                ser = item_ser,
            )
        }
        IdlType::Array {
            array: (_inner, size),
        } => {
            format!("    data += input.{}[:{size}]\n", name)
        }
        IdlType::Defined { defined } => {
            if let Some(td) = types.iter().find(|t| t.name == defined.name) {
                let mut result = String::new();
                for field in &td.fields {
                    result.push_str(&serialize_field_expr(
                        &format!("{}.{}", name, camel_to_snake(&field.name)),
                        &field.ty,
                        field.codec.as_ref(),
                        types,
                    ));
                }
                result
            } else {
                format!("    data += input.{}  # unknown type\n", name)
            }
        }
        IdlType::Generic { generic } => {
            panic!("Generic type '{}' not supported in Python codegen", generic)
        }
    }
}

fn decode_dynamic_header(
    name: &str,
    ty: &IdlType,
    codec: Option<&IdlCodec>,
    indent: usize,
) -> String {
    let pad = " ".repeat(indent);
    if is_optional_dynamic(ty) {
        return format!(
            "{pad}{name}_tag, offset = _unpack(\"<B\", data, offset)\n{pad}if {name}_tag not in \
             (0, 1):\n{pad}    raise DecodeError(\"invalid option tag\")\n"
        );
    }
    let (fmt, _) = prefix_fmt(codec.expect("dynamic field codec").prefix_bytes());
    format!("{pad}{name}_len, offset = _unpack(\"<{fmt}\", data, offset)\n")
}

fn decode_dynamic_tail(
    name: &str,
    ty: &IdlType,
    codec: Option<&IdlCodec>,
    indent: usize,
    types: &[IdlTypeDef],
) -> String {
    let pad = " ".repeat(indent);
    if let IdlType::Option { option } = ty {
        let value = decode_dynamic_value(name, option, codec, indent + 4, false, types);
        return format!("{pad}if {name}_tag == 0:\n{pad}    {name} = None\n{pad}else:\n{value}");
    }
    decode_dynamic_value(name, ty, codec, indent, true, types)
}

fn decode_dynamic_value(
    name: &str,
    ty: &IdlType,
    codec: Option<&IdlCodec>,
    indent: usize,
    length_is_known: bool,
    types: &[IdlTypeDef],
) -> String {
    let pad = " ".repeat(indent);
    let prefix = if length_is_known {
        String::new()
    } else {
        let (fmt, _) = prefix_fmt(codec.expect("dynamic field codec").prefix_bytes());
        format!("{pad}{name}_len, offset = _unpack(\"<{fmt}\", data, offset)\n")
    };
    match ty {
        IdlType::Primitive(primitive) if primitive == "string" => format!(
            "{prefix}{pad}_raw, offset = _take(data, offset, {name}_len)\n{pad}try:\n{pad}    \
             {name} = _raw.decode(\"utf-8\")\n{pad}except UnicodeDecodeError as exc:\n{pad}    \
             raise DecodeError(\"invalid UTF-8\") from exc\n"
        ),
        IdlType::Vec { vec } => {
            let item = decode_field_expr("_item", vec, None, indent + 4, types);
            format!(
                "{prefix}{pad}if {name}_len > _MAX_DECODE_ELEMENTS or {name}_len > len(data) - \
                 offset:\n{pad}    raise DecodeError(\"element count exceeds \
                 limit\")\n{pad}{name} = []\n{pad}for _ in range({name}_len):\n{item}{pad}    \
                 {name}.append(_item)\n"
            )
        }
        _ => format!("{pad}raise DecodeError(\"invalid dynamic field\")\n"),
    }
}

fn decode_field_expr(
    name: &str,
    ty: &IdlType,
    codec: Option<&IdlCodec>,
    indent: usize,
    types: &[IdlTypeDef],
) -> String {
    let pad = " ".repeat(indent);

    // Handle dynamic string with codec
    if let IdlType::Primitive(p) = ty {
        if p == "string" {
            if let Some(c) = codec {
                let (fmt, _) = prefix_fmt(c.prefix_bytes());
                return format!(
                    "{pad}_len, offset = _unpack(\"<{fmt}\", data, offset)\n{pad}_raw, offset = \
                     _take(data, offset, _len)\n{pad}try:\n{pad}    {name} = \
                     _raw.decode(\"utf-8\")\n{pad}except UnicodeDecodeError as exc:\n{pad}    \
                     raise DecodeError(\"invalid UTF-8\") from exc\n",
                );
            }
        }
    }

    // Handle Vec with codec
    if let IdlType::Vec { ref vec } = ty {
        if let Some(c) = codec {
            let (fmt, _) = prefix_fmt(c.prefix_bytes());
            let item_decode = decode_field_expr("_item", vec, None, indent + 4, types);
            return format!(
                "{pad}_count, offset = _unpack(\"<{fmt}\", data, offset)\n{pad}if _count > \
                 _MAX_DECODE_ELEMENTS or _count > len(data) - offset:\n{pad}    raise \
                 DecodeError(\"element count exceeds limit\")\n{pad}{name} = []\n{pad}for _ in \
                 range(_count):\n{item_decode}{pad}    {name}.append(_item)\n",
            );
        }
    }

    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "bool" => format!(
                "{pad}_raw, offset = _unpack(\"<B\", data, offset)\n{pad}if _raw not in (0, \
                 1):\n{pad}    raise DecodeError(\"invalid bool\")\n{pad}{name} = bool(_raw)\n",
            ),
            "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "f32" | "f64" => format!(
                "{pad}{name}, offset = _unpack(\"<{fmt}\", data, offset)\n",
                fmt = struct_format(p),
            ),
            "pubkey" => format!(
                "{pad}_raw, offset = _take(data, offset, 32)\n{pad}{name} = \
                 Pubkey.from_bytes(_raw)\n",
            ),
            "u128" => format!(
                "{pad}_raw, offset = _take(data, offset, 16)\n{pad}{name} = int.from_bytes(_raw, \
                 byteorder=\"little\")\n",
            ),
            "i128" => format!(
                "{pad}_raw, offset = _take(data, offset, 16)\n{pad}{name} = int.from_bytes(_raw, \
                 byteorder=\"little\", signed=True)\n",
            ),
            "string" => {
                // Plain string without codec uses a Borsh-style u32 prefix.
                format!(
                    "{pad}_len, offset = _unpack(\"<I\", data, offset)\n{pad}_raw, offset = \
                     _take(data, offset, _len)\n{pad}try:\n{pad}    {name} = \
                     _raw.decode(\"utf-8\")\n{pad}except UnicodeDecodeError as exc:\n{pad}    \
                     raise DecodeError(\"invalid UTF-8\") from exc\n",
                )
            }
            other => {
                let fmt = struct_format(other);
                format!("{pad}{name}, offset = _unpack(\"<{fmt}\", data, offset)\n",)
            }
        },
        IdlType::Vec { vec } => {
            // Vec without codec uses a Borsh-style u32 prefix.
            let item_decode = decode_field_expr("_item", vec, None, indent + 4, types);
            format!(
                "{pad}_count, offset = _unpack(\"<I\", data, offset)\n{pad}if _count > \
                 _MAX_DECODE_ELEMENTS or _count > len(data) - offset:\n{pad}    raise \
                 DecodeError(\"element count exceeds limit\")\n{pad}{name} = []\n{pad}for _ in \
                 range(_count):\n{item_decode}{pad}    {name}.append(_item)\n",
            )
        }
        IdlType::Array {
            array: (_inner, size),
        } => format!("{pad}{name}, offset = _take(data, offset, {size})\n"),
        IdlType::Option { option } => {
            let inner =
                decode_field_expr(&format!("{}_inner", name), option, codec, indent + 4, types);
            format!(
                "{pad}_tag, offset = _unpack(\"<B\", data, offset)\n{pad}if _tag == 0:\n{pad}    \
                 {name} = None\n{pad}elif _tag == 1:\n{inner}{pad}    {name} = \
                 {name}_inner\n{pad}else:\n{pad}    raise DecodeError(\"invalid option tag\")\n",
            )
        }
        IdlType::Defined { defined } => {
            if let Some(td) = types.iter().find(|t| t.name == defined.name) {
                let mut result = String::new();
                for field in &td.fields {
                    result.push_str(&decode_field_expr(
                        &format!("_{}", camel_to_snake(&field.name)),
                        &field.ty,
                        field.codec.as_ref(),
                        indent,
                        types,
                    ));
                }
                let field_names: Vec<String> = td
                    .fields
                    .iter()
                    .map(|f| {
                        let snake = camel_to_snake(&f.name);
                        format!("{}=_{}", snake, snake)
                    })
                    .collect();
                result.push_str(&format!(
                    "{pad}{n} = {cls}({args})\n",
                    pad = pad,
                    n = name,
                    cls = defined.name,
                    args = field_names.join(", "),
                ));
                result
            } else {
                format!("{pad}raise DecodeError(\"unknown defined type\")\n")
            }
        }
        IdlType::Generic { generic } => {
            panic!("Generic type '{}' not supported in Python codegen", generic)
        }
    }
}

/// Returns the `struct` format character and byte size for a length prefix.
fn prefix_fmt(prefix_bytes: usize) -> (&'static str, usize) {
    match prefix_bytes {
        1 => ("B", 1),
        2 => ("H", 2),
        4 => ("I", 4),
        _ => ("Q", 8),
    }
}

fn struct_format(primitive: &str) -> &'static str {
    match primitive {
        "bool" => "?",
        "u8" => "B",
        "i8" => "b",
        "u16" => "H",
        "i16" => "h",
        "u32" => "I",
        "i32" => "i",
        "u64" => "Q",
        "i64" => "q",
        "f32" => "f",
        "f64" => "d",
        _ => "B",
    }
}

fn py_bool(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

fn account_field_seed_input_name(path: &str, field: &str) -> String {
    format!(
        "{}_{}_seed",
        camel_to_snake(path),
        field
            .split('.')
            .map(camel_to_snake)
            .collect::<Vec<_>>()
            .join("_")
    )
}

fn python_pda_seed_expr(expr: &str, ty: Option<&IdlType>) -> String {
    match ty {
        Some(IdlType::Primitive(p)) => match p.as_str() {
            "pubkey" => format!("bytes({expr})"),
            "bool" => format!("bytes([1 if {expr} else 0])"),
            "u8" => format!("struct.pack(\"<B\", {expr})"),
            "i8" => format!("struct.pack(\"<b\", {expr})"),
            "u16" => format!("struct.pack(\"<H\", {expr})"),
            "i16" => format!("struct.pack(\"<h\", {expr})"),
            "u32" => format!("struct.pack(\"<I\", {expr})"),
            "i32" => format!("struct.pack(\"<i\", {expr})"),
            "u64" => format!("struct.pack(\"<Q\", {expr})"),
            "i64" => format!("struct.pack(\"<q\", {expr})"),
            "u128" | "i128" => format!(
                "int({expr}).to_bytes(16, \"little\", signed={})",
                p.starts_with('i')
            ),
            _ => expr.to_string(),
        },
        Some(IdlType::Array { .. }) => format!("bytes({expr})"),
        _ => expr.to_string(),
    }
}
