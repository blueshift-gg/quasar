use {
    super::model::{python_field_path, ProgramModel},
    crate::types::{Idl, IdlArg, IdlCodec, IdlPdaSeed, IdlResolver, IdlType, IdlTypeDef},
    quasar_schema::{camel_to_snake, snake_to_pascal, to_screaming_snake},
    std::fmt::Write,
};

/// Generate a Python client module from the IDL.
///
/// Uses `solders` for Solana types (Pubkey, Instruction, AccountMeta)
/// and `struct` for binary serialization.
pub fn generate_python_client(idl: &Idl) -> String {
    let model = ProgramModel::new(idl);
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
            for field in &type_def.fields {
                out.push_str(&decode_field_expr(
                    &camel_to_snake(&field.name),
                    &field.ty,
                    field.codec.as_ref(),
                    8,
                    &idl.types,
                ));
            }
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

        // Account fields
        let mut has_any_fields = false;
        for acc in &ix.accounts {
            if matches!(acc.resolver, IdlResolver::Const { .. }) {
                continue; // Known addresses are auto-filled
            }
            if matches!(acc.resolver, IdlResolver::Pda { .. }) {
                continue; // PDAs are derived
            }
            writeln!(out, "    {}: Pubkey", camel_to_snake(&acc.name)).unwrap();
            has_any_fields = true;
        }

        for seed in account_field_seed_inputs(ix, idl) {
            writeln!(
                out,
                "    {}: {}",
                account_field_seed_input_name(&seed.path, &seed.field),
                python_type(&seed.ty)
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

        // Build accounts list
        out.push_str("    accounts = []\n");
        for acc in &ix.accounts {
            let key_expr = if let IdlResolver::Const { ref address } = acc.resolver {
                format!("Pubkey.from_string(\"{}\")", address)
            } else if let IdlResolver::Pda { ref seeds, .. } = acc.resolver {
                let mut seed_exprs = Vec::new();
                for seed in seeds {
                    match seed {
                        IdlPdaSeed::Const { value } => {
                            seed_exprs
                                .push(format!("bytes([{}])", super::format_disc_decimal(value)));
                        }
                        IdlPdaSeed::Account { path } => {
                            seed_exprs.push(format!("bytes(accounts_map[\"{}\"])", path));
                        }
                        IdlPdaSeed::AccountField {
                            path,
                            field,
                            account,
                            ..
                        } => {
                            let ty = account_field_type(idl, account, field);
                            seed_exprs.push(python_pda_seed_expr(
                                &format!("input.{}", account_field_seed_input_name(path, field)),
                                ty.as_ref(),
                            ));
                        }
                        IdlPdaSeed::Arg { path, .. } => {
                            seed_exprs.push(format!("input.{}", python_field_path(path)));
                        }
                    }
                }
                format!(
                    "Pubkey.find_program_address([{}], PROGRAM_ID)[0]",
                    seed_exprs.join(", ")
                )
            } else {
                format!("input.{}", camel_to_snake(&acc.name))
            };

            writeln!(out, "    accounts_map[\"{}\"] = {}", acc.name, key_expr).unwrap();
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

        // Build instruction data — compact wire format:
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

            // Phase 2: length table — pre-encode dynamic bytes and emit all
            // length prefixes grouped together.
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

    out
}

// ---------------------------------------------------------------------------
// Type mapping
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the arg is a top-level dynamic type (string with codec or
/// Vec with codec). These require compact 3-phase encoding at the instruction
/// level.
fn is_direct_dynamic(arg: &IdlArg) -> bool {
    arg.codec.is_some() && dynamic_payload_type(&arg.ty).is_some()
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
                // Plain string without codec — use u32 prefix (borsh-style)
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
            // Vec without codec — use u32 prefix (borsh-style)
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
                let (fmt, sz) = prefix_fmt(c.prefix_bytes());
                return format!(
                    "{pad}_len = struct.unpack_from(\"<{fmt}\", data, offset)[0]\n{pad}offset += \
                     {sz}\n{pad}{n} = data[offset:offset + _len].decode(\"utf-8\")\n{pad}offset \
                     += _len\n",
                    pad = pad,
                    n = name,
                    fmt = fmt,
                    sz = sz,
                );
            }
        }
    }

    // Handle Vec with codec
    if let IdlType::Vec { ref vec } = ty {
        if let Some(c) = codec {
            let (fmt, sz) = prefix_fmt(c.prefix_bytes());
            let item_decode = match &**vec {
                IdlType::Primitive(p) if p == "pubkey" => {
                    "Pubkey.from_bytes(data[offset:offset + 32]); offset += 32".to_string()
                }
                IdlType::Primitive(p) => {
                    let f = struct_format(p);
                    let item_sz = primitive_size(p);
                    format!(
                        "struct.unpack_from(\"<{}\", data, offset)[0]; offset += {}",
                        f, item_sz
                    )
                }
                _ => "data[offset:offset + 1]; offset += 1".to_string(),
            };
            return format!(
                "{pad}_count = struct.unpack_from(\"<{fmt}\", data, offset)[0]\n{pad}offset += \
                 {sz}\n{pad}{n} = []\n{pad}for _ in range(_count):\n{pad}    _item = \
                 {decode}\n{pad}    {n}.append(_item)\n",
                pad = pad,
                n = name,
                fmt = fmt,
                sz = sz,
                decode = item_decode,
            );
        }
    }

    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "bool" => format!(
                "{pad}{n} = struct.unpack_from(\"<?\", data, offset)[0]\n{pad}offset += 1\n",
                pad = pad,
                n = name,
            ),
            "u8" => format!(
                "{pad}{n} = data[offset]\n{pad}offset += 1\n",
                pad = pad,
                n = name,
            ),
            "i8" => format!(
                "{pad}{n} = struct.unpack_from(\"<b\", data, offset)[0]\n{pad}offset += 1\n",
                pad = pad,
                n = name,
            ),
            "pubkey" => format!(
                "{pad}{n} = Pubkey.from_bytes(data[offset:offset + 32])\n{pad}offset += 32\n",
                pad = pad,
                n = name,
            ),
            "u128" => format!(
                "{pad}{n} = int.from_bytes(data[offset:offset + 16], \
                 byteorder=\"little\")\n{pad}offset += 16\n",
                pad = pad,
                n = name,
            ),
            "i128" => format!(
                "{pad}{n} = int.from_bytes(data[offset:offset + 16], byteorder=\"little\", \
                 signed=True)\n{pad}offset += 16\n",
                pad = pad,
                n = name,
            ),
            "string" => {
                // Plain string without codec — u32 prefix (borsh-style)
                format!(
                    "{pad}_len = struct.unpack_from(\"<I\", data, offset)[0]\n{pad}offset += \
                     4\n{pad}{n} = data[offset:offset + _len].decode(\"utf-8\")\n{pad}offset += \
                     _len\n",
                    pad = pad,
                    n = name,
                )
            }
            other => {
                let fmt = struct_format(other);
                let size = primitive_size(other);
                format!(
                    "{pad}{n} = struct.unpack_from(\"<{fmt}\", data, offset)[0]\n{pad}offset += \
                     {sz}\n",
                    pad = pad,
                    n = name,
                    fmt = fmt,
                    sz = size,
                )
            }
        },
        IdlType::Vec { vec } => {
            // Vec without codec — u32 prefix (borsh-style)
            let item_decode = match &**vec {
                IdlType::Primitive(p) if p == "pubkey" => {
                    "Pubkey.from_bytes(data[offset:offset + 32]); offset += 32".to_string()
                }
                IdlType::Primitive(p) => {
                    let f = struct_format(p);
                    let item_sz = primitive_size(p);
                    format!(
                        "struct.unpack_from(\"<{}\", data, offset)[0]; offset += {}",
                        f, item_sz
                    )
                }
                _ => "data[offset:offset + 1]; offset += 1".to_string(),
            };
            format!(
                "{pad}_count = struct.unpack_from(\"<I\", data, offset)[0]\n{pad}offset += \
                 4\n{pad}{n} = []\n{pad}for _ in range(_count):\n{pad}    _item = \
                 {decode}\n{pad}    {n}.append(_item)\n",
                pad = pad,
                n = name,
                decode = item_decode,
            )
        }
        IdlType::Array {
            array: (_inner, size),
        } => {
            format!(
                "{pad}{n} = data[offset:offset + {sz}]\n{pad}offset += {sz}\n",
                pad = pad,
                n = name,
                sz = size,
            )
        }
        IdlType::Option { option } => {
            let inner =
                decode_field_expr(&format!("{}_inner", name), option, None, indent + 4, types);
            format!(
                "{pad}if data[offset] == 0:\n{pad}    {n} = None\n{pad}    offset += \
                 1\n{pad}else:\n{pad}    offset += 1\n{inner}{pad}    {n} = {n}_inner\n",
                pad = pad,
                n = name,
                inner = inner,
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
                format!(
                    "{pad}{n} = data[offset:]  # unknown type\n",
                    pad = pad,
                    n = name,
                )
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

fn primitive_size(p: &str) -> usize {
    match p {
        "bool" | "u8" | "i8" => 1,
        "u16" | "i16" => 2,
        "u32" | "i32" | "f32" => 4,
        "u64" | "i64" | "f64" => 8,
        "u128" | "i128" => 16,
        "pubkey" => 32,
        _ => 0,
    }
}

fn py_bool(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

struct AccountFieldSeedInput {
    path: String,
    field: String,
    ty: IdlType,
}

fn account_field_seed_inputs(
    ix: &crate::types::IdlInstruction,
    idl: &Idl,
) -> Vec<AccountFieldSeedInput> {
    let mut inputs = Vec::new();
    for acc in &ix.accounts {
        let IdlResolver::Pda { seeds, .. } = &acc.resolver else {
            continue;
        };
        for seed in seeds {
            let IdlPdaSeed::AccountField {
                path,
                account,
                field,
            } = seed
            else {
                continue;
            };
            if inputs
                .iter()
                .any(|input: &AccountFieldSeedInput| input.path == *path && input.field == *field)
            {
                continue;
            }
            if let Some(ty) = account_field_type(idl, account, field) {
                inputs.push(AccountFieldSeedInput {
                    path: path.clone(),
                    field: field.clone(),
                    ty,
                });
            }
        }
    }
    inputs
}

fn account_field_type(idl: &Idl, account: &str, field: &str) -> Option<IdlType> {
    let mut current_account = account.to_string();
    let mut field_ty = None;

    for segment in field.split('.') {
        let type_def = idl.types.iter().find(|ty| ty.name == current_account)?;
        let field_def = type_def.fields.iter().find(|f| f.name == segment)?;
        field_ty = Some(field_def.ty.clone());
        if let IdlType::Defined { defined } = &field_def.ty {
            current_account = defined.name.clone();
        }
    }

    field_ty
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
