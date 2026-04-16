use {
    super::model::{python_field_path, ProgramModel},
    crate::types::{Idl, IdlType, IdlTypeDef},
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
        if type_def.ty.fields.is_empty() {
            out.push_str("    pass\n");
        } else {
            for field in &type_def.ty.fields {
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
        if !type_def.ty.fields.is_empty() {
            writeln!(out, "    @classmethod").unwrap();
            writeln!(
                out,
                "    def decode(cls, data: bytes) -> {}:",
                type_def.name
            )
            .unwrap();
            out.push_str("        offset = 0\n");
            for field in &type_def.ty.fields {
                out.push_str(&decode_field_expr(
                    &camel_to_snake(&field.name),
                    &field.ty,
                    8,
                    &idl.types,
                ));
            }
            let field_names: Vec<String> = type_def
                .ty
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
            if acc.address.is_some() {
                continue; // Known addresses are auto-filled
            }
            if acc.pda.is_some() {
                continue; // PDAs are derived
            }
            writeln!(out, "    {}: Pubkey", camel_to_snake(&acc.name)).unwrap();
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
        if ix.has_remaining {
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
            let key_expr = if let Some(ref addr) = acc.address {
                format!("Pubkey.from_string(\"{}\")", addr)
            } else if let Some(ref pda) = acc.pda {
                let mut seeds = Vec::new();
                for seed in &pda.seeds {
                    match seed {
                        crate::types::IdlSeed::Const { value } => {
                            seeds.push(format!("bytes([{}])", super::format_disc_decimal(value)));
                        }
                        crate::types::IdlSeed::Account { path } => {
                            seeds.push(format!("bytes(accounts_map[\"{}\"])", path));
                        }
                        crate::types::IdlSeed::Arg { path } => {
                            seeds.push(format!("input.{}", python_field_path(path)));
                        }
                    }
                }
                format!(
                    "Pubkey.find_program_address([{}], PROGRAM_ID)[0]",
                    seeds.join(", ")
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
                py_bool(acc.signer),
                py_bool(acc.writable),
            )
            .unwrap();
        }

        if ix.has_remaining {
            out.push_str(
                "    if input.remaining_accounts:\n        \
                 accounts.extend(input.remaining_accounts)\n",
            );
        }

        // Build instruction data
        let const_name = to_screaming_snake(&ix.name);
        if ix.args.is_empty() {
            writeln!(out, "    data = {}_DISCRIMINATOR", const_name).unwrap();
        } else {
            writeln!(out, "    data = bytearray({}_DISCRIMINATOR)", const_name).unwrap();
            for arg in &ix.args {
                out.push_str(&serialize_field_expr(
                    &camel_to_snake(&arg.name),
                    &arg.ty,
                    &idl.types,
                ));
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
                if td.ty.fields.is_empty() {
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
            _ if p.starts_with('[') => "bytes".to_string(),
            _ => "bytes".to_string(),
        },
        IdlType::Option { option } => format!("Optional[{}]", python_type(option)),
        IdlType::DynString { .. } => "str".to_string(),
        IdlType::DynVec { .. } => "list".to_string(),
        IdlType::Defined { defined } => defined.clone(),
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn serialize_field_expr(name: &str, ty: &IdlType, types: &[IdlTypeDef]) -> String {
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
            _ if p.starts_with('[') => {
                format!("    data += input.{}\n", name)
            }
            _ => format!("    data += input.{}  # unsupported\n", name),
        },
        IdlType::Option { option } => {
            let inner = serialize_field_expr(&format!("{}_val", name), option, types);
            format!(
                "    if input.{n} is None:\n        data += b'\\x00'\n    else:\n        data += \
                 b'\\x01'\n        {n}_val = input.{n}\n{inner}",
                n = name,
                inner = inner.replace("    data", "        data"),
            )
        }
        IdlType::DynString { string } => {
            let (fmt, _sz) = prefix_fmt(string.prefix_bytes);
            format!(
                "    _b = input.{n}.encode(\"utf-8\")\n    data += struct.pack(\"<{fmt}\", \
                 len(_b))\n    data += _b\n",
                n = name,
                fmt = fmt,
            )
        }
        IdlType::DynVec { vec } => {
            let (fmt, _sz) = prefix_fmt(vec.prefix_bytes);
            let item_ser = match &*vec.items {
                IdlType::Primitive(p) if p == "pubkey" => "bytes(item)".to_string(),
                IdlType::Primitive(p) => {
                    let f = struct_format(p);
                    format!("struct.pack(\"<{}\", item)", f)
                }
                _ => "item".to_string(),
            };
            format!(
                "    data += struct.pack(\"<{fmt}\", len(input.{n}))\n    for item in \
                 input.{n}:\n        data += {ser}\n",
                n = name,
                fmt = fmt,
                ser = item_ser,
            )
        }
        IdlType::Defined { defined } => {
            if let Some(td) = types.iter().find(|t| t.name == *defined) {
                let mut result = String::new();
                for field in &td.ty.fields {
                    result.push_str(&serialize_field_expr(
                        &format!("{}.{}", name, camel_to_snake(&field.name)),
                        &field.ty,
                        types,
                    ));
                }
                result
            } else {
                format!("    data += input.{}  # unknown type\n", name)
            }
        }
    }
}

fn decode_field_expr(name: &str, ty: &IdlType, indent: usize, types: &[IdlTypeDef]) -> String {
    let pad = " ".repeat(indent);
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
            other if other.starts_with('[') => {
                let size = super::parse_fixed_array_size(other).unwrap_or(1);
                format!(
                    "{pad}{n} = data[offset:offset + {sz}]\n{pad}offset += {sz}\n",
                    pad = pad,
                    n = name,
                    sz = size,
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
        IdlType::DynString { string } => {
            let (fmt, sz) = prefix_fmt(string.prefix_bytes);
            format!(
                "{pad}_len = struct.unpack_from(\"<{fmt}\", data, offset)[0]\n{pad}offset += \
                 {sz}\n{pad}{n} = data[offset:offset + _len].decode(\"utf-8\")\n{pad}offset += \
                 _len\n",
                pad = pad,
                n = name,
                fmt = fmt,
                sz = sz,
            )
        }
        IdlType::DynVec { vec } => {
            let (fmt, sz) = prefix_fmt(vec.prefix_bytes);
            let item_decode = match &*vec.items {
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
                "{pad}_count = struct.unpack_from(\"<{fmt}\", data, offset)[0]\n{pad}offset += \
                 {sz}\n{pad}{n} = []\n{pad}for _ in range(_count):\n{pad}    _item = \
                 {decode}\n{pad}    {n}.append(_item)\n",
                pad = pad,
                n = name,
                fmt = fmt,
                sz = sz,
                decode = item_decode,
            )
        }
        IdlType::Option { option } => {
            let inner = decode_field_expr(&format!("{}_inner", name), option, indent + 4, types);
            format!(
                "{pad}if data[offset] == 0:\n{pad}    {n} = None\n{pad}    offset += \
                 1\n{pad}else:\n{pad}    offset += 1\n{inner}{pad}    {n} = {n}_inner\n",
                pad = pad,
                n = name,
                inner = inner,
            )
        }
        IdlType::Defined { defined } => {
            if let Some(td) = types.iter().find(|t| t.name == *defined) {
                let mut result = String::new();
                for field in &td.ty.fields {
                    result.push_str(&decode_field_expr(
                        &format!("_{}", camel_to_snake(&field.name)),
                        &field.ty,
                        indent,
                        types,
                    ));
                }
                let field_names: Vec<String> = td
                    .ty
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
                    cls = defined,
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
    }
}

/// Returns the `struct` format character and byte size for a length prefix.
fn prefix_fmt(prefix_bytes: usize) -> (&'static str, usize) {
    match prefix_bytes {
        1 => ("B", 1),
        2 => ("H", 2),
        _ => ("I", 4),
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
