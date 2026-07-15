use {
    super::model::{CodegenResult, ProgramFeatures, ProgramModel, WireType},
    crate::types::{
        AccountFlag, Idl, IdlAccountNode, IdlCodec, IdlFieldDef, IdlLayout, IdlPdaSeed,
        IdlResolver, IdlType,
    },
    quasar_schema::{
        camel_to_snake, pascal_to_snake, snake_to_pascal,
        to_screaming_snake as pascal_to_screaming_snake,
    },
    std::{
        collections::{HashMap, HashSet},
        fmt::Write,
    },
};

/// Generate Cargo.toml content for the standalone client crate.
pub fn generate_cargo_toml(name: &str, version: &str, has_pdas: bool) -> String {
    let quasar_version = env!("CARGO_PKG_VERSION");
    let solana_address = if has_pdas {
        r#"solana-address = { version = "=2.2.0", features = ["curve25519", "wincode"] }"#
    } else {
        r#"solana-address = { version = "=2.2.0", features = ["wincode"] }"#
    };
    format!(
        r#"[package]
name = "{name}-client"
version = "{version}"
edition = "2021"

[dependencies]
quasar-lang = "={quasar_version}"
wincode = {{ version = "=0.4.9", features = ["derive"] }}
{solana_address}
solana-instruction = "3"
"#,
    )
}

pub fn generate_cargo_toml_for_program(model: &ProgramModel<'_>) -> String {
    generate_cargo_toml(
        &model.identity.client_name,
        &model.idl.version,
        model.features.has_pdas,
    )
}

/// Check whether the IDL has any resolvable PDA annotations.
pub fn has_pdas(idl: &Idl) -> bool {
    ProgramFeatures::from_idl(idl).has_pdas
}

/// Generate a standalone Rust client crate from the IDL.
///
/// Returns a `Vec<(relative_path, file_content)>` where paths are relative to
/// the client crate `src/` directory.
pub fn generate_client(idl: &Idl) -> CodegenResult<Vec<(String, String)>> {
    let model = ProgramModel::try_new(idl)?;
    let mut files: Vec<(String, String)> = Vec::new();

    // Build type map for custom data types.
    let type_map: HashMap<String, Vec<IdlFieldDef>> = idl
        .types
        .iter()
        .map(|td| (td.name.clone(), td.fields.clone()))
        .collect();

    // Account and event payload definitions also appear in `idl.types` so
    // references to their fields can be resolved during generation. They are
    // emitted by the dedicated `state` and `events` modules, however, and must
    // not be emitted a second time under `types`.
    let discriminated_names: HashSet<&str> = idl
        .accounts
        .iter()
        .map(|item| item.name.as_str())
        .chain(idl.events.iter().map(|item| item.name.as_str()))
        .collect();
    let standalone_type_map: HashMap<String, Vec<IdlFieldDef>> = idl
        .types
        .iter()
        .filter(|item| !discriminated_names.contains(item.name.as_str()))
        .map(|item| (item.name.clone(), item.fields.clone()))
        .collect();

    let has_instructions = model.features.has_instructions;
    let has_state = model.features.has_accounts;
    let has_events = model.features.has_events;
    let has_types = !standalone_type_map.is_empty();
    let has_errors = model.features.has_errors;

    // Collect PDA info for pda.rs generation
    let pdas = collect_pdas(idl);
    let has_pdas = model.features.has_pdas;

    files.push((
        "lib.rs".to_string(),
        emit_lib_rs(
            idl,
            has_instructions,
            has_state,
            has_events,
            has_types,
            has_errors,
            has_pdas,
        ),
    ));

    if has_instructions {
        let (mod_rs, ix_files) = emit_instructions(idl, &type_map);
        files.push(("instructions/mod.rs".to_string(), mod_rs));
        for (name, content) in ix_files {
            files.push((format!("instructions/{}.rs", name), content));
        }
    }

    if has_state {
        let (mod_rs, state_files) = emit_discriminated_module(
            &idl.accounts,
            "account",
            "ProgramAccount",
            "decode_account",
            &type_map,
        );
        files.push(("state/mod.rs".to_string(), mod_rs));
        for (name, content) in state_files {
            files.push((format!("state/{}.rs", name), content));
        }
    }

    if has_events {
        let (mod_rs, event_files) = emit_discriminated_module(
            &idl.events,
            "event",
            "ProgramEvent",
            "decode_event",
            &type_map,
        );
        files.push(("events/mod.rs".to_string(), mod_rs));
        for (name, content) in event_files {
            files.push((format!("events/{}.rs", name), content));
        }
    }

    if has_types {
        let (mod_rs, type_files) = emit_types(&standalone_type_map);
        files.push(("types/mod.rs".to_string(), mod_rs));
        for (name, content) in type_files {
            files.push((format!("types/{}.rs", name), content));
        }
    }

    if has_errors {
        files.push((
            "errors.rs".to_string(),
            emit_errors(idl, model.identity.program_name.as_str()),
        ));
    }

    if has_pdas {
        files.push(("pda.rs".to_string(), emit_pda(&pdas)));
    }

    Ok(files)
}

fn emit_lib_rs(
    idl: &Idl,
    has_instructions: bool,
    has_state: bool,
    has_events: bool,
    has_types: bool,
    has_errors: bool,
    has_pdas: bool,
) -> String {
    let mut out = String::new();
    out.push_str("use solana_address::Address;\n\n");

    writeln!(
        out,
        "pub const ID: Address = solana_address::address!(\"{}\");",
        idl.address
    )
    .expect("write to String");

    let modules: &[(&str, bool)] = &[
        ("instructions", has_instructions),
        ("state", has_state),
        ("events", has_events),
        ("types", has_types),
        ("errors", has_errors),
        ("pda", has_pdas),
    ];

    let active: Vec<&str> = modules
        .iter()
        .filter(|(_, active)| *active)
        .map(|(name, _)| *name)
        .collect();

    if !active.is_empty() {
        out.push('\n');
        for name in &active {
            writeln!(out, "pub mod {};", name).expect("write to String");
        }
        out.push('\n');
        for name in &active {
            writeln!(out, "pub use {}::*;", name).expect("write to String");
        }
    }

    out
}

fn emit_instructions(
    idl: &Idl,
    type_map: &HashMap<String, Vec<IdlFieldDef>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut ix_files: Vec<(String, String)> = Vec::new();

    // Scan all instruction arg types for imports needed by mod.rs
    let mut needs_dyn_string = false;
    let mut needs_dyn_vec = false;
    let mut needs_address = false;
    for ix in &idl.instructions {
        for arg in &ix.args {
            collect_wrapper_needs(
                &arg.ty,
                &arg.codec,
                &mut needs_dyn_string,
                &mut needs_dyn_vec,
            );
            if field_needs_address(&arg.ty) {
                needs_address = true;
            }
        }
    }
    emit_wrapper_imports(&mut mod_rs, needs_dyn_string, needs_dyn_vec);
    if needs_address {
        mod_rs.push_str("use solana_address::Address;\n");
    }
    // Import defined types used in instruction args
    for ix in &idl.instructions {
        for arg in &ix.args {
            emit_type_use_imports(&mut mod_rs, &arg.ty, type_map);
        }
    }

    // mod declarations and re-exports
    for ix in &idl.instructions {
        let snake = camel_to_snake(&ix.name);
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    mod_rs.push('\n');
    for ix in &idl.instructions {
        let snake = camel_to_snake(&ix.name);
        writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
    }
    mod_rs.push('\n');

    // ProgramInstruction enum
    mod_rs.push_str("pub enum ProgramInstruction {\n");
    for ix in &idl.instructions {
        let pascal = snake_to_pascal(&ix.name);
        if ix.args.is_empty() {
            writeln!(mod_rs, "    {},", pascal).expect("write to String");
        } else {
            write!(mod_rs, "    {} {{ ", pascal).expect("write to String");
            for (i, arg) in ix.args.iter().enumerate() {
                if i > 0 {
                    write!(mod_rs, ", ").expect("write to String");
                }
                write!(
                    mod_rs,
                    "{}: {}",
                    camel_to_snake(&arg.name),
                    rust_field_type(&arg.ty, &arg.codec)
                )
                .expect("write to String");
            }
            writeln!(mod_rs, " }},").expect("write to String");
        }
    }
    mod_rs.push_str("}\n\n");

    // Total cursor helpers for decoding untrusted instruction bytes. Only emit
    // helpers used by this program so warning-free generated clients remain a
    // useful release gate.
    let needs_cursor = idl.instructions.iter().any(|ix| !ix.args.is_empty());
    let needs_read_len = idl.instructions.iter().any(|ix| {
        ix.args
            .iter()
            .any(|arg| is_direct_dynamic(&arg.ty, &arg.codec))
    });
    if needs_cursor {
        mod_rs.push_str(
            "fn quasar_take<'a>(data: &'a [u8], offset: &mut usize, len: usize) -> Option<&'a \
             [u8]> {\n\x20   let end = offset.checked_add(len)?;\n\x20   let bytes = \
             data.get(*offset..end)?;\n\x20   *offset = end;\n\x20   Some(bytes)\n}\n\n",
        );
    }
    if needs_read_len {
        mod_rs.push_str(
            "fn quasar_read_len(data: &[u8], offset: &mut usize, width: usize) -> Option<usize> \
             {\n\x20   let mut buf = [0u8; 8];\n\x20   \
             buf.get_mut(..width)?.copy_from_slice(quasar_take(data, offset, width)?);\n\x20   \
             usize::try_from(u64::from_le_bytes(buf)).ok()\n}\n\n",
        );
    }

    // decode_instruction function
    mod_rs.push_str("pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {\n");

    let disc_len = idl
        .instructions
        .first()
        .map(|ix| ix.discriminator.len())
        .unwrap_or(1);

    if disc_len == 1 {
        mod_rs.push_str("    let disc = *data.first()?;\n");
        mod_rs.push_str("    match disc {\n");
    } else {
        writeln!(mod_rs, "    let disc = data.get(..{})?;", disc_len).expect("write to String");
        mod_rs.push_str("    match disc {\n");
    }

    for ix in &idl.instructions {
        let pascal = snake_to_pascal(&ix.name);
        let disc_str = super::format_disc_decimal(&ix.discriminator);

        if disc_len == 1 {
            write!(mod_rs, "        {} => ", disc_str).expect("write to String");
        } else {
            write!(mod_rs, "        [{}] => ", disc_str).expect("write to String");
        }

        if ix.args.is_empty() {
            writeln!(
                mod_rs,
                "if data.len() == {disc_len} {{ Some(ProgramInstruction::{pascal}) }} else {{ \
                 None }},"
            )
            .expect("write to String");
        } else {
            mod_rs.push_str("{\n");
            writeln!(mod_rs, "            let payload = &data[{}..];", disc_len)
                .expect("write to String");

            let is_compact = matches!(ix.layout, Some(IdlLayout::Compact { .. }));
            let has_dyn = ix.args.iter().any(|a| is_direct_dynamic(&a.ty, &a.codec));

            if has_dyn && is_compact {
                // Compact layout: [fixed fields][all dynamic prefixes][all dynamic data]
                let fixed_args: Vec<_> = ix
                    .args
                    .iter()
                    .filter(|a| !is_direct_dynamic(&a.ty, &a.codec))
                    .collect();
                let dyn_args: Vec<_> = ix
                    .args
                    .iter()
                    .filter(|a| is_direct_dynamic(&a.ty, &a.codec))
                    .collect();

                mod_rs.push_str("            let mut offset = 0usize;\n");

                // Phase 1: read fixed fields
                for arg in &fixed_args {
                    let name = camel_to_snake(&arg.name);
                    let rty = rust_field_type(&arg.ty, &arg.codec);
                    writeln!(
                        mod_rs,
                        "            let {name}: {rty} = \
                         wincode::deserialize(payload.get(offset..)?).ok()?;",
                    )
                    .expect("write to String");
                    writeln!(
                        mod_rs,
                        "            let {name}_size = \
                         usize::try_from(wincode::serialized_size(&{name}).ok()?).ok()?;",
                    )
                    .expect("write to String");
                    writeln!(
                        mod_rs,
                        "            quasar_take(payload, &mut offset, {name}_size)?;"
                    )
                    .expect("write to String");
                }

                // Phase 2: read length table (all dynamic prefixes)
                for arg in &dyn_args {
                    let name = camel_to_snake(&arg.name);
                    if optional_dynamic_inner(&arg.ty).is_some() {
                        writeln!(
                            mod_rs,
                            "            let {name}_tag = *payload.get(offset)?;"
                        )
                        .expect("write to String");
                        mod_rs.push_str("            offset += 1;\n");
                        writeln!(mod_rs, "            if {name}_tag > 1 {{ return None; }}")
                            .expect("write to String");
                    } else {
                        let pfx = dynamic_prefix_bytes_from_codec(&arg.codec);
                        writeln!(
                            mod_rs,
                            "            let {name}_len = quasar_read_len(payload, &mut offset, \
                             {pfx})?;"
                        )
                        .expect("write to String");
                    }
                }

                // Phase 3: read tail data
                for arg in &dyn_args {
                    let name = camel_to_snake(&arg.name);
                    let rty = rust_field_type(&arg.ty, &arg.codec);
                    if let Some(inner) = optional_dynamic_inner(&arg.ty) {
                        let pfx = dynamic_prefix_bytes_from_codec(&arg.codec);
                        if is_dynamic_string(inner) {
                            writeln!(
                                mod_rs,
                                "            let {name}: {rty} = if {name}_tag == 0 {{"
                            )
                            .expect("write to String");
                            mod_rs.push_str("                None\n");
                            mod_rs.push_str("            } else {\n");
                            writeln!(
                                mod_rs,
                                "                let {name}_len = quasar_read_len(payload, &mut \
                                 offset, {pfx})?;"
                            )
                            .expect("write to String");
                            writeln!(
                                mod_rs,
                                "                let bytes = quasar_take(payload, &mut offset, \
                                 {name}_len)?;"
                            )
                            .expect("write to String");
                            mod_rs.push_str(
                                "                let value = \
                                 core::str::from_utf8(bytes).ok()?.into();\n",
                            );
                            mod_rs.push_str("                Some(value)\n");
                            mod_rs.push_str("            };\n");
                        } else if let Some(vec_inner) = vec_inner_type(inner) {
                            let item_ty = rust_field_type(vec_inner, &None);
                            writeln!(
                                mod_rs,
                                "            let {name}: {rty} = if {name}_tag == 0 {{"
                            )
                            .expect("write to String");
                            mod_rs.push_str("                None\n");
                            mod_rs.push_str("            } else {\n");
                            writeln!(
                                mod_rs,
                                "                let {name}_len = quasar_read_len(payload, &mut \
                                 offset, {pfx})?;"
                            )
                            .expect("write to String");
                            writeln!(
                                mod_rs,
                                "                if {name}_len > 10 * 1024 * 1024 {{ return None; \
                                 }}"
                            )
                            .expect("write to String");
                            writeln!(
                                mod_rs,
                                "                let mut items = \
                                 Vec::with_capacity({name}_len.min(4096));"
                            )
                            .expect("write to String");
                            writeln!(mod_rs, "                for _ in 0..{name}_len {{")
                                .expect("write to String");
                            writeln!(
                                mod_rs,
                                "                    let item: {item_ty} = \
                                 wincode::deserialize(payload.get(offset..)?).ok()?;"
                            )
                            .expect("write to String");
                            mod_rs.push_str(
                                "                    let item_size = \
                                 usize::try_from(wincode::serialized_size(&item).ok()?).ok()?;\n",
                            );
                            mod_rs.push_str(
                                "                    quasar_take(payload, &mut offset, \
                                 item_size)?;\n",
                            );
                            mod_rs.push_str("                    items.push(item);\n");
                            mod_rs.push_str("                }\n");
                            mod_rs.push_str("                Some(items.into())\n");
                            mod_rs.push_str("            };\n");
                        }
                    } else if is_dynamic_string(&arg.ty) {
                        writeln!(
                            mod_rs,
                            "            let {name}_bytes = quasar_take(payload, &mut offset, \
                             {name}_len)?;"
                        )
                        .expect("write to String");
                        writeln!(
                            mod_rs,
                            "            let {name}: {rty} = \
                             core::str::from_utf8({name}_bytes).ok()?.into();"
                        )
                        .expect("write to String");
                    } else if is_dynamic_vec(&arg.ty) {
                        let item_ty = rust_field_type(vec_inner_type(&arg.ty).unwrap(), &None);
                        writeln!(mod_rs, "            let {name}: {rty} = {{")
                            .expect("write to String");
                        writeln!(
                            mod_rs,
                            "                if {name}_len > 10 * 1024 * 1024 {{ return None; \
                             }}\n\x20               let mut items = \
                             Vec::with_capacity({name}_len.min(4096));"
                        )
                        .expect("write to String");
                        writeln!(mod_rs, "                for _ in 0..{name}_len {{")
                            .expect("write to String");
                        writeln!(
                            mod_rs,
                            "                    let item: {item_ty} = \
                             wincode::deserialize(payload.get(offset..)?).ok()?;"
                        )
                        .expect("write to String");
                        writeln!(
                            mod_rs,
                            "                    let item_size = \
                             usize::try_from(wincode::serialized_size(&item).ok()?).ok()?;"
                        )
                        .expect("write to String");
                        mod_rs.push_str(
                            "                    quasar_take(payload, &mut offset, item_size)?;\n",
                        );
                        mod_rs.push_str("                    items.push(item);\n");
                        mod_rs.push_str("                }\n");
                        mod_rs.push_str("                items.into()\n");
                        mod_rs.push_str("            };\n");
                    }
                }
            } else {
                // Fixed fields decode sequentially.
                mod_rs.push_str("            let mut offset = 0usize;\n");
                for arg in &ix.args {
                    let name = camel_to_snake(&arg.name);
                    let rty = rust_field_type(&arg.ty, &arg.codec);
                    writeln!(
                        mod_rs,
                        "            let {name}: {rty} = \
                         wincode::deserialize(payload.get(offset..)?).ok()?;",
                    )
                    .expect("write to String");
                    writeln!(
                        mod_rs,
                        "            let {name}_size = \
                         usize::try_from(wincode::serialized_size(&{name}).ok()?).ok()?;",
                    )
                    .expect("write to String");
                    writeln!(
                        mod_rs,
                        "            quasar_take(payload, &mut offset, {name}_size)?;"
                    )
                    .expect("write to String");
                }
            }

            mod_rs.push_str("            if offset != payload.len() { return None; }\n");

            write!(
                mod_rs,
                "            Some(ProgramInstruction::{} {{ ",
                pascal
            )
            .expect("write to String");
            for (i, arg) in ix.args.iter().enumerate() {
                if i > 0 {
                    write!(mod_rs, ", ").expect("write to String");
                }
                write!(mod_rs, "{}", camel_to_snake(&arg.name)).expect("write to String");
            }
            mod_rs.push_str(" })\n");
            mod_rs.push_str("        }\n");
        }
    }

    mod_rs.push_str("        _ => None,\n");
    mod_rs.push_str("    }\n");
    mod_rs.push_str("}\n");

    // Individual instruction files
    for ix in &idl.instructions {
        let snake = camel_to_snake(&ix.name);
        let content = emit_single_instruction(ix, type_map);
        ix_files.push((snake, content));
    }

    (mod_rs, ix_files)
}

fn emit_single_instruction(
    ix: &crate::types::IdlInstruction,
    type_map: &HashMap<String, Vec<IdlFieldDef>>,
) -> String {
    let mut out = String::new();

    let struct_name = snake_to_pascal(&ix.name);

    let has_remaining = ix.remaining_accounts.is_some();
    if has_remaining {
        out.push_str("use std::vec::Vec;\n");
    }

    out.push_str("use solana_instruction::{AccountMeta, Instruction};\n");
    out.push_str("use crate::ID;\n");

    let args_need_address = ix.args.iter().any(|arg| field_needs_address(&arg.ty));
    let accounts_need_address = ix
        .accounts
        .iter()
        .any(|account| account.optional || !matches!(account.resolver, IdlResolver::Const { .. }));
    if !args_need_address && accounts_need_address {
        out.push_str("use solana_address::Address;\n");
    }

    emit_field_imports(
        &mut out,
        ix.args.iter().map(|a| (&a.ty, &a.codec)),
        type_map,
    );

    out.push('\n');

    writeln!(out, "pub struct {}Instruction {{", struct_name).expect("write to String");

    for account in &ix.accounts {
        if !account.optional && matches!(account.resolver, IdlResolver::Const { .. }) {
            continue;
        }
        // Optional accounts become `Option<Address>`; an absent (`None`) slot is
        // encoded as the program id sentinel per the runtime convention.
        let field_ty = if account.optional {
            "Option<Address>"
        } else {
            "Address"
        };
        writeln!(
            out,
            "    pub {}: {},",
            camel_to_snake(&account.name),
            field_ty
        )
        .expect("write to String");
    }

    for arg in &ix.args {
        writeln!(
            out,
            "    pub {}: {},",
            camel_to_snake(&arg.name),
            rust_field_type(&arg.ty, &arg.codec)
        )
        .expect("write to String");
    }

    if has_remaining {
        out.push_str("    pub remaining_accounts: Vec<AccountMeta>,\n");
    }

    out.push_str("}\n\n");

    writeln!(
        out,
        "impl From<{}Instruction> for Instruction {{",
        struct_name
    )
    .expect("write to String");
    writeln!(
        out,
        "    fn from(ix: {}Instruction) -> Instruction {{",
        struct_name
    )
    .expect("write to String");
    if !has_remaining && ix.args.is_empty() && !accounts_need_address {
        out.push_str("        let _ = ix;\n");
    }

    if has_remaining {
        out.push_str("        let mut accounts = vec![\n");
    } else {
        out.push_str("        let accounts = vec![\n");
    }
    for account in &ix.accounts {
        writeln!(out, "            {},", account_meta_expr(account)).expect("write to String");
    }
    out.push_str("        ];\n");
    if has_remaining {
        out.push_str("        accounts.extend(ix.remaining_accounts);\n");
    }

    // Compact wire format:
    //   [disc][fixed fields][all dynamic prefixes][all dynamic data]
    let disc_str = super::format_disc_decimal(&ix.discriminator);
    let is_compact = matches!(ix.layout, Some(IdlLayout::Compact { .. }));

    if ix.args.is_empty() {
        writeln!(out, "        let data = vec![{}];", disc_str).expect("write to String");
    } else {
        writeln!(out, "        let mut data = vec![{}];", disc_str).expect("write to String");

        let fixed_args: Vec<_> = ix
            .args
            .iter()
            .filter(|a| !is_direct_dynamic(&a.ty, &a.codec))
            .collect();
        let dyn_args: Vec<_> = ix
            .args
            .iter()
            .filter(|a| is_direct_dynamic(&a.ty, &a.codec))
            .collect();

        // Fixed fields are serialized in IDL order.
        for arg in &fixed_args {
            writeln!(
                out,
                "        wincode::serialize_into(&mut data, &ix.{}).expect(\"serialization into \
                 Vec<u8> is infallible\");",
                camel_to_snake(&arg.name)
            )
            .expect("write to String");
        }

        if !dyn_args.is_empty() && is_compact {
            // Group all dynamic length prefixes before tail data.
            for arg in &dyn_args {
                let name = camel_to_snake(&arg.name);
                if optional_dynamic_inner(&arg.ty).is_some() {
                    writeln!(out, "        data.push(u8::from(ix.{name}.is_some()));")
                        .expect("write to String");
                } else {
                    let pfx = dynamic_prefix_bytes_from_codec(&arg.codec);
                    writeln!(
                        out,
                        "        data.extend_from_slice(&(ix.{name}.len() as \
                         u64).to_le_bytes()[..{pfx}]);"
                    )
                    .expect("write to String");
                }
            }

            // Write dynamic data in IDL order.
            for arg in &dyn_args {
                let name = camel_to_snake(&arg.name);
                if let Some(inner) = optional_dynamic_inner(&arg.ty) {
                    let pfx = dynamic_prefix_bytes_from_codec(&arg.codec);
                    if is_dynamic_string(inner) {
                        writeln!(out, "        if let Some(value) = &ix.{name} {{")
                            .expect("write to String");
                        writeln!(
                            out,
                            "            data.extend_from_slice(&(value.len() as \
                             u64).to_le_bytes()[..{pfx}]);"
                        )
                        .expect("write to String");
                        out.push_str("            data.extend_from_slice(value.as_bytes());\n");
                        out.push_str("        }\n");
                    } else if vec_inner_type(inner).is_some() {
                        writeln!(out, "        if let Some(value) = &ix.{name} {{")
                            .expect("write to String");
                        writeln!(
                            out,
                            "            data.extend_from_slice(&(value.len() as \
                             u64).to_le_bytes()[..{pfx}]);"
                        )
                        .expect("write to String");
                        out.push_str("            for item in value.iter() {\n");
                        writeln!(
                            out,
                            "                wincode::serialize_into(&mut data, \
                             item).expect(\"serialization into Vec<u8> is infallible\");"
                        )
                        .expect("write to String");
                        out.push_str("            }\n");
                        out.push_str("        }\n");
                    }
                } else if is_dynamic_string(&arg.ty) {
                    writeln!(out, "        data.extend_from_slice(ix.{name}.as_bytes());")
                        .expect("write to String");
                } else if is_dynamic_vec(&arg.ty) {
                    writeln!(out, "        for item in ix.{name}.iter() {{")
                        .expect("write to String");
                    writeln!(
                        out,
                        "            wincode::serialize_into(&mut data, \
                         item).expect(\"serialization into Vec<u8> is infallible\");"
                    )
                    .expect("write to String");
                    out.push_str("        }\n");
                }
            }
        } else if !dyn_args.is_empty() {
            // Non-compact: serialize dynamic fields sequentially with inline prefixes
            for arg in &dyn_args {
                let name = camel_to_snake(&arg.name);
                let pfx = dynamic_prefix_bytes_from_codec(&arg.codec);
                writeln!(
                    out,
                    "        data.extend_from_slice(&(ix.{name}.len() as \
                     u64).to_le_bytes()[..{pfx}]);"
                )
                .expect("write to String");
                if is_dynamic_string(&arg.ty) {
                    writeln!(out, "        data.extend_from_slice(ix.{name}.as_bytes());")
                        .expect("write to String");
                } else if is_dynamic_vec(&arg.ty) {
                    writeln!(out, "        for item in ix.{name}.iter() {{")
                        .expect("write to String");
                    writeln!(
                        out,
                        "            wincode::serialize_into(&mut data, \
                         item).expect(\"serialization into Vec<u8> is infallible\");"
                    )
                    .expect("write to String");
                    out.push_str("        }\n");
                }
            }
        }
    }

    out.push_str("        Instruction {\n");
    out.push_str("            program_id: ID,\n");
    out.push_str("            accounts,\n");
    out.push_str("            data,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    out
}

/// Trait abstracting over IdlAccountDef and IdlEventDef for shared codegen.
trait DiscriminatedItem {
    fn name(&self) -> &str;
    fn discriminator(&self) -> &[u8];
}

impl DiscriminatedItem for crate::types::IdlAccountDef {
    fn name(&self) -> &str {
        &self.name
    }
    fn discriminator(&self) -> &[u8] {
        &self.discriminator
    }
}

impl DiscriminatedItem for crate::types::IdlEventDef {
    fn name(&self) -> &str {
        &self.name
    }
    fn discriminator(&self) -> &[u8] {
        &self.discriminator
    }
}

/// Generate mod.rs + individual files for a discriminated module (state or
/// events).
fn emit_discriminated_module<T: DiscriminatedItem>(
    items: &[T],
    kind: &str,
    enum_name: &str,
    decode_fn: &str,
    type_map: &HashMap<String, Vec<IdlFieldDef>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut item_files: Vec<(String, String)> = Vec::new();

    let has_fields = |item: &T| type_map.get(item.name()).is_some_and(|f| !f.is_empty());

    let with_fields: Vec<_> = items.iter().filter(|item| has_fields(item)).collect();
    let without_fields: Vec<_> = items.iter().filter(|item| !has_fields(item)).collect();

    // mod declarations for items with fields
    for item in &with_fields {
        let snake = pascal_to_snake(item.name());
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    if !with_fields.is_empty() {
        mod_rs.push('\n');
        for item in &with_fields {
            let snake = pascal_to_snake(item.name());
            writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
        }
        mod_rs.push('\n');
    }

    // Discriminator constants for fieldless items (in mod.rs)
    let kind_upper = kind.to_ascii_uppercase();
    for item in &without_fields {
        let base = disc_base_name(item.name(), kind);
        let const_name = pascal_to_screaming_snake(base);
        let disc_str = super::format_disc_decimal(item.discriminator());
        writeln!(
            mod_rs,
            "pub const {}_{}_DISCRIMINATOR: &[u8] = &[{}];",
            const_name, kind_upper, disc_str
        )
        .expect("write to String");
    }
    if !without_fields.is_empty() {
        mod_rs.push('\n');
    }

    // Enum
    writeln!(mod_rs, "pub enum {} {{", enum_name).expect("write to String");
    for item in items {
        if has_fields(item) {
            writeln!(mod_rs, "    {}({}),", item.name(), item.name()).expect("write to String");
        } else {
            writeln!(mod_rs, "    {},", item.name()).expect("write to String");
        }
    }
    mod_rs.push_str("}\n\n");

    // decode function
    writeln!(
        mod_rs,
        "pub fn {}(data: &[u8]) -> Option<{}> {{",
        decode_fn, enum_name
    )
    .expect("write to String");
    for item in items {
        let base = disc_base_name(item.name(), kind);
        let const_name = pascal_to_screaming_snake(base);
        writeln!(
            mod_rs,
            "    if data.starts_with({}_{}_DISCRIMINATOR) {{",
            const_name, kind_upper
        )
        .expect("write to String");
        if has_fields(item) {
            writeln!(
                mod_rs,
                "        let value = wincode::deserialize::<{}>(data).ok()?;",
                item.name()
            )
            .expect("write to String");
            writeln!(
                mod_rs,
                "        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != \
                 data.len() {{ return None; }}"
            )
            .expect("write to String");
            writeln!(
                mod_rs,
                "        return Some({}::{}(value));",
                enum_name,
                item.name()
            )
            .expect("write to String");
        } else {
            writeln!(
                mod_rs,
                "        return (data.len() == {}_{}_DISCRIMINATOR.len())\n\x20           \
                 .then_some({}::{});",
                const_name,
                kind_upper,
                enum_name,
                item.name()
            )
            .expect("write to String");
        }
        mod_rs.push_str("    }\n");
    }
    mod_rs.push_str("    None\n");
    mod_rs.push_str("}\n");

    // Individual files
    for item in &with_fields {
        let snake = pascal_to_snake(item.name());
        let fields = type_map
            .get(item.name())
            .expect("invariant: with_fields only contains items present in type_map");
        let content =
            emit_single_state_or_event(item.name(), item.discriminator(), fields, kind, type_map);
        item_files.push((snake, content));
    }

    (mod_rs, item_files)
}

/// Strip "Event" suffix for event discriminator constant names to avoid
/// stutter.
fn disc_base_name<'a>(name: &'a str, kind: &str) -> &'a str {
    if kind == "event" {
        name.strip_suffix("Event").unwrap_or(name)
    } else {
        name
    }
}

fn emit_single_state_or_event(
    name: &str,
    discriminator: &[u8],
    fields: &[IdlFieldDef],
    kind: &str,
    type_map: &HashMap<String, Vec<IdlFieldDef>>,
) -> String {
    let mut out = String::new();

    // Imports for manual impls
    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");
    out.push_str("use wincode::config::ConfigCore;\n");
    out.push_str("use wincode::error::{ReadError, ReadResult, WriteResult};\n");
    out.push_str("use wincode::io::{Reader, Writer};\n");
    out.push_str("use std::mem::MaybeUninit;\n");

    emit_field_imports(&mut out, fields.iter().map(|f| (&f.ty, &f.codec)), type_map);

    out.push('\n');

    // Discriminator constant
    let base = disc_base_name(name, kind);
    let const_name = pascal_to_screaming_snake(base);
    let kind_upper = kind.to_ascii_uppercase();
    let disc_str = super::format_disc_decimal(discriminator);
    writeln!(
        out,
        "pub const {}_{}_DISCRIMINATOR: &[u8] = &[{}];",
        const_name, kind_upper, disc_str
    )
    .expect("write to String");
    out.push('\n');

    // Struct + manual impls
    emit_manual_impls(&mut out, name, discriminator, fields, kind);

    out
}

fn emit_types(type_map: &HashMap<String, Vec<IdlFieldDef>>) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut type_files: Vec<(String, String)> = Vec::new();

    // Sort for deterministic output
    let mut type_names: Vec<&String> = type_map.keys().collect();
    type_names.sort();

    for type_name in &type_names {
        let snake = pascal_to_snake(type_name);
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    mod_rs.push('\n');
    for type_name in &type_names {
        let snake = pascal_to_snake(type_name);
        writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
    }

    for type_name in &type_names {
        let fields = &type_map[*type_name];
        let snake = pascal_to_snake(type_name);
        let content = emit_single_type(type_name, fields, type_map);
        type_files.push((snake, content));
    }

    (mod_rs, type_files)
}

fn emit_single_type(
    type_name: &str,
    fields: &[IdlFieldDef],
    type_map: &HashMap<String, Vec<IdlFieldDef>>,
) -> String {
    let mut out = String::new();

    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");

    emit_field_imports(&mut out, fields.iter().map(|f| (&f.ty, &f.codec)), type_map);

    out.push('\n');

    out.push_str("#[derive(SchemaWrite, SchemaRead)]\n");
    writeln!(out, "pub struct {} {{", type_name).expect("write to String");
    for field in fields {
        writeln!(
            out,
            "    pub {}: {},",
            camel_to_snake(&field.name),
            rust_field_type(&field.ty, &field.codec)
        )
        .expect("write to String");
    }
    out.push_str("}\n");

    out
}

fn emit_errors(idl: &Idl, program_name: &str) -> String {
    let mut out = String::new();

    let enum_name = format!("{}Error", snake_to_pascal(program_name));

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    out.push_str("#[repr(u32)]\n");
    writeln!(out, "pub enum {} {{", enum_name).expect("write to String");
    for err in &idl.errors {
        writeln!(out, "    {} = {},", err.name, err.code).expect("write to String");
    }
    out.push_str("}\n\n");

    writeln!(out, "impl {} {{", enum_name).expect("write to String");

    // from_code
    out.push_str("    pub fn from_code(code: u32) -> Option<Self> {\n");
    out.push_str("        match code {\n");
    for err in &idl.errors {
        writeln!(out, "            {} => Some(Self::{}),", err.code, err.name)
            .expect("write to String");
    }
    out.push_str("            _ => None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    // message
    out.push_str("    pub fn message(&self) -> &'static str {\n");
    out.push_str("        match self {\n");
    for err in &idl.errors {
        let msg = err.msg.as_deref().unwrap_or(&err.name);
        let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
        writeln!(out, "            Self::{} => \"{}\",", err.name, escaped)
            .expect("write to String");
    }
    out.push_str("        }\n");
    out.push_str("    }\n");

    out.push_str("}\n");

    out
}

/// A collected PDA with its field name and seeds.
struct PdaInfo {
    field_name: String,
    seeds: Vec<IdlPdaSeed>,
}

fn collect_pdas(idl: &Idl) -> Vec<PdaInfo> {
    let mut pdas: Vec<PdaInfo> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for ix in &idl.instructions {
        for account in &ix.accounts {
            if let IdlResolver::Pda { ref seeds, .. } = account.resolver {
                if seeds.is_empty() {
                    continue;
                }

                // Dedup by seed identity (use debug repr as key)
                let key = format!("{:?}", seeds);
                if !seen.insert(key) {
                    continue;
                }

                pdas.push(PdaInfo {
                    field_name: camel_to_snake(&account.name),
                    seeds: seeds.clone(),
                });
            }
        }
    }

    pdas
}

/// Format a const seed value for display (doc comments or code expressions).
fn format_const_seed_display(value: &[u8]) -> String {
    if value.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        format!("b\"{}\"", String::from_utf8_lossy(value))
    } else {
        let byte_list: Vec<String> = value.iter().map(|b| b.to_string()).collect();
        format!("&[{}]", byte_list.join(", "))
    }
}

fn emit_pda(pdas: &[PdaInfo]) -> String {
    let mut out = String::new();

    out.push_str("use solana_address::Address;\n\n");

    for pda in pdas {
        // Build doc comment showing seeds
        let seed_desc: Vec<String> = pda
            .seeds
            .iter()
            .map(|s| match s {
                IdlPdaSeed::Const { value } => format_const_seed_display(value),
                IdlPdaSeed::Account { path } => camel_to_snake(path),
                IdlPdaSeed::AccountField { path, field, .. } => {
                    format!("{}:{}", camel_to_snake(path), camel_to_snake(field))
                }
                IdlPdaSeed::Arg { path, .. } => format!("arg:{}", camel_to_snake(path)),
            })
            .collect();
        writeln!(out, "/// Seeds: [{}]", seed_desc.join(", ")).expect("write to String");

        let mut params: Vec<String> = Vec::new();
        let mut seen_params: HashSet<String> = HashSet::new();
        for seed in &pda.seeds {
            match seed {
                IdlPdaSeed::Account { path } => {
                    push_unique_param(
                        &mut params,
                        &mut seen_params,
                        format!("account:{path}"),
                        format!("{}: &Address", camel_to_snake(path)),
                    );
                }
                IdlPdaSeed::AccountField { path, field, .. } => {
                    push_unique_param(
                        &mut params,
                        &mut seen_params,
                        format!("account-field:{path}:{field}"),
                        format!(
                            "{}_{}_seed: &[u8]",
                            camel_to_snake(path),
                            camel_to_snake(field)
                        ),
                    );
                }
                IdlPdaSeed::Arg { path, ty, .. } => {
                    push_unique_param(
                        &mut params,
                        &mut seen_params,
                        format!("arg:{path}"),
                        format!("{}: {}", camel_to_snake(path), rust_pda_arg_type(ty)),
                    );
                }
                _ => {}
            }
        }
        params.push("program_id: &Address".to_string());

        let fn_name = format!("find_{}_address", pda.field_name);
        writeln!(
            out,
            "pub fn {}({}) -> (Address, u8) {{",
            fn_name,
            params.join(", ")
        )
        .expect("write to String");

        let mut seen_seed_vars = HashSet::new();
        for seed in &pda.seeds {
            let IdlPdaSeed::Arg { path, ty, .. } = seed else {
                continue;
            };
            let var = rust_pda_arg_seed_var(path);
            if !seen_seed_vars.insert(var) {
                continue;
            }
            if let Some(setup) = rust_pda_arg_seed_setup(path, ty) {
                out.push_str(&setup);
            }
        }

        let seed_exprs: Vec<String> = pda
            .seeds
            .iter()
            .map(|s| match s {
                IdlPdaSeed::Const { value } => format_const_seed_display(value),
                IdlPdaSeed::Account { path } => format!("{}.as_ref()", camel_to_snake(path)),
                IdlPdaSeed::AccountField { path, field, .. } => {
                    format!("{}_{}_seed", camel_to_snake(path), camel_to_snake(field))
                }
                IdlPdaSeed::Arg { path, ty, .. } => rust_pda_arg_seed_expr(path, ty),
            })
            .collect();

        writeln!(
            out,
            "    Address::find_program_address(&[{}], program_id)",
            seed_exprs.join(", ")
        )
        .expect("write to String");
        out.push_str("}\n\n");
    }

    out
}

fn push_unique_param(
    params: &mut Vec<String>,
    seen: &mut HashSet<String>,
    key: String,
    param: String,
) {
    if seen.insert(key) {
        params.push(param);
    }
}

fn rust_pda_arg_type(ty: &IdlType) -> String {
    match pda_scalar_type(ty) {
        Some("pubkey") => "&Address".to_string(),
        Some(
            scalar @ ("bool" | "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "u128"
            | "i128"),
        ) => scalar.to_string(),
        _ => match ty {
            IdlType::Array {
                array: (_inner, size),
            } => format!("[u8; {size}]"),
            _ => "&[u8]".to_string(),
        },
    }
}

fn rust_pda_arg_seed_setup(path: &str, ty: &IdlType) -> Option<String> {
    let name = camel_to_snake(path);
    let var = rust_pda_arg_seed_var(path);
    match pda_scalar_type(ty) {
        Some("bool") => Some(format!("    let {var} = [{name} as u8];\n")),
        Some("u8") | Some("i8") => Some(format!("    let {var} = [{name} as u8];\n")),
        Some("u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "u128" | "i128") => {
            Some(format!("    let {var} = {name}.to_le_bytes();\n"))
        }
        _ => None,
    }
}

fn rust_pda_arg_seed_expr(path: &str, ty: &IdlType) -> String {
    match pda_scalar_type(ty) {
        Some("pubkey") => format!("{}.as_ref()", camel_to_snake(path)),
        Some(
            "bool" | "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "u128" | "i128",
        ) => format!("{}.as_ref()", rust_pda_arg_seed_var(path)),
        _ => match ty {
            IdlType::Array { .. } => format!("{}.as_ref()", camel_to_snake(path)),
            _ => camel_to_snake(path),
        },
    }
}

fn rust_pda_arg_seed_var(path: &str) -> String {
    format!("{}_seed", camel_to_snake(path))
}

fn pda_scalar_type(ty: &IdlType) -> Option<&str> {
    match ty {
        IdlType::Primitive(p) => Some(p),
        IdlType::Defined { defined } => match defined.name.as_str() {
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
        },
        _ => None,
    }
}

/// Returns `true` if the field is a top-level dynamic type (string or vec with
/// SizePrefixed codec). These require compact wire-format handling.
fn is_direct_dynamic(ty: &IdlType, codec: &Option<IdlCodec>) -> bool {
    matches!(codec, Some(IdlCodec::SizePrefixed { .. }))
        && (is_dynamic_string(ty) || is_dynamic_vec(ty) || optional_dynamic_inner(ty).is_some())
}

/// Check if the type represents a dynamic string.
fn is_dynamic_string(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "string")
}

/// Check if the type represents a dynamic vec.
fn is_dynamic_vec(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Vec { .. })
}

/// Get the inner type of a Vec type.
fn vec_inner_type(ty: &IdlType) -> Option<&IdlType> {
    match ty {
        IdlType::Vec { vec } => Some(vec.as_ref()),
        _ => None,
    }
}

fn optional_dynamic_inner(ty: &IdlType) -> Option<&IdlType> {
    match ty {
        IdlType::Option { option }
            if is_dynamic_string(option) || matches!(**option, IdlType::Vec { .. }) =>
        {
            Some(option)
        }
        _ => None,
    }
}

/// Return the byte-width of the length prefix from codec info.
fn dynamic_prefix_bytes_from_codec(codec: &Option<IdlCodec>) -> usize {
    if let Some(IdlCodec::SizePrefixed { ref prefix, .. }) = codec {
        match prefix.ty.as_str() {
            "u8" => 1,
            "u16" => 2,
            "u32" => 4,
            "u64" => 8,
            _ => 2,
        }
    } else {
        2 // default
    }
}

/// Scan field types and emit wrapper imports (DynString, DynVec), Address
/// import, and defined type imports.
fn emit_field_imports<'a>(
    out: &mut String,
    types: impl Iterator<Item = (&'a IdlType, &'a Option<IdlCodec>)>,
    type_map: &HashMap<String, Vec<IdlFieldDef>>,
) {
    let mut needs_address = false;
    let mut needs_dyn_string = false;
    let mut needs_dyn_vec = false;
    for (ty, codec) in types {
        collect_wrapper_needs(ty, codec, &mut needs_dyn_string, &mut needs_dyn_vec);
        if field_needs_address(ty) {
            needs_address = true;
        }
        emit_type_use_imports(out, ty, type_map);
    }
    if needs_address {
        out.push_str("use solana_address::Address;\n");
    }
    emit_wrapper_imports(out, needs_dyn_string, needs_dyn_vec);
}

/// Emit struct definition + manual SchemaWrite/SchemaRead impls with
/// discriminator handling.
fn emit_manual_impls(
    out: &mut String,
    name: &str,
    discriminator: &[u8],
    idl_fields: &[IdlFieldDef],
    kind: &str,
) {
    let has_dynamic = idl_fields
        .iter()
        .any(|f| is_direct_dynamic(&f.ty, &f.codec));

    if has_dynamic {
        out.push_str("#[derive(Clone)]\n");
    } else {
        out.push_str("#[derive(Clone, Copy)]\n");
    }
    writeln!(out, "pub struct {} {{", name).expect("write to String");
    let fields: Vec<(String, String)> = idl_fields
        .iter()
        .map(|f| (camel_to_snake(&f.name), rust_field_type(&f.ty, &f.codec)))
        .collect();
    for (field_name, field_type) in &fields {
        writeln!(out, "    pub {}: {},", field_name, field_type).expect("write to String");
    }
    out.push_str("}\n\n");

    // Partition into fixed and dynamic fields
    let fixed_fields: Vec<(&str, &str, &IdlFieldDef)> = idl_fields
        .iter()
        .zip(fields.iter())
        .filter(|(f, _)| !is_direct_dynamic(&f.ty, &f.codec))
        .map(|(f, (n, t))| (n.as_str(), t.as_str(), f))
        .collect();
    let dyn_fields: Vec<(&str, &str, &IdlFieldDef)> = idl_fields
        .iter()
        .zip(fields.iter())
        .filter(|(f, _)| is_direct_dynamic(&f.ty, &f.codec))
        .map(|(f, (n, t))| (n.as_str(), t.as_str(), f))
        .collect();

    // Collect unique types for trait bounds.
    let unique_bound_types: Vec<String> = {
        let mut types: Vec<String> = fixed_fields
            .iter()
            .map(|(_, ty, _)| ty.to_string())
            .collect();
        for (_, _, idl_f) in &dyn_fields {
            if let Some(inner) = vec_inner_type(&idl_f.ty) {
                types.push(rust_field_type(inner, &None));
            }
        }
        types.sort();
        types.dedup();
        types
    };

    let base = disc_base_name(name, kind);
    let const_name = pascal_to_screaming_snake(base);
    let disc_const = format!("{}_{}_DISCRIMINATOR", const_name, kind.to_ascii_uppercase());

    writeln!(
        out,
        "unsafe impl<C: ConfigCore> SchemaWrite<C> for {}",
        name
    )
    .expect("write to String");
    if !unique_bound_types.is_empty() {
        out.push_str("where\n");
        for ty in &unique_bound_types {
            writeln!(out, "    {ty}: SchemaWrite<C, Src = {ty}>,").expect("write to String");
        }
    }
    out.push_str("{\n");
    out.push_str("    type Src = Self;\n\n");

    // size_of
    out.push_str("    fn size_of(src: &Self) -> WriteResult<usize> {\n");
    write!(out, "        Ok({}", discriminator.len()).expect("write to String");
    for (field_name, field_type, _) in &fixed_fields {
        write!(
            out,
            "\n            + <{field_type} as SchemaWrite<C>>::size_of(&src.{field_name})?"
        )
        .expect("write to String");
    }
    // Dynamic prefix sizes (constant per field)
    for (_, _, idl_f) in &dyn_fields {
        let pfx = dynamic_prefix_bytes_from_codec(&idl_f.codec);
        write!(out, "\n            + {pfx}").expect("write to String");
    }
    // Dynamic data sizes
    for (field_name, _, idl_f) in &dyn_fields {
        if is_dynamic_string(&idl_f.ty) {
            write!(out, "\n            + src.{field_name}.len()").expect("write to String");
        } else if is_dynamic_vec(&idl_f.ty) {
            let item_ty = rust_field_type(vec_inner_type(&idl_f.ty).unwrap(), &None);
            write!(out, "\n            + {{").expect("write to String");
            write!(out, "\n                let mut s = 0usize;").expect("write to String");
            write!(
                out,
                "\n                for item in src.{field_name}.iter() {{"
            )
            .expect("write to String");
            write!(
                out,
                "\n                    s += <{item_ty} as SchemaWrite<C>>::size_of(item)?;"
            )
            .expect("write to String");
            write!(out, "\n                }}").expect("write to String");
            write!(out, "\n                s").expect("write to String");
            write!(out, "\n            }}").expect("write to String");
        }
    }
    out.push_str(")\n");
    out.push_str("    }\n\n");

    // Compact layout: [disc][fixed][all prefixes][all data].
    out.push_str("    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {\n");
    writeln!(out, "        writer.write({disc_const})?;").expect("write to String");

    // Phase 1: fixed fields
    for (field_name, field_type, _) in &fixed_fields {
        writeln!(
            out,
            "        <{field_type} as SchemaWrite<C>>::write(writer.by_ref(), &src.{field_name})?;"
        )
        .expect("write to String");
    }

    if has_dynamic {
        // Phase 2: length table
        for (field_name, _, idl_f) in &dyn_fields {
            let pfx = dynamic_prefix_bytes_from_codec(&idl_f.codec);
            writeln!(
                out,
                "        writer.write(&(src.{field_name}.len() as u64).to_le_bytes()[..{pfx}])?;"
            )
            .expect("write to String");
        }

        // Phase 3: tail data
        for (field_name, _, idl_f) in &dyn_fields {
            if is_dynamic_string(&idl_f.ty) {
                writeln!(out, "        writer.write(src.{field_name}.as_bytes())?;")
                    .expect("write to String");
            } else if is_dynamic_vec(&idl_f.ty) {
                let item_ty = rust_field_type(vec_inner_type(&idl_f.ty).unwrap(), &None);
                writeln!(out, "        for item in src.{field_name}.iter() {{")
                    .expect("write to String");
                writeln!(
                    out,
                    "            <{item_ty} as SchemaWrite<C>>::write(writer.by_ref(), item)?;"
                )
                .expect("write to String");
                out.push_str("        }\n");
            }
        }
    }

    out.push_str("        Ok(())\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    writeln!(
        out,
        "unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for {}",
        name
    )
    .expect("write to String");
    if !unique_bound_types.is_empty() {
        out.push_str("where\n");
        for ty in &unique_bound_types {
            writeln!(out, "    {ty}: SchemaRead<'de, C, Dst = {ty}>,").expect("write to String");
        }
    }
    out.push_str("{\n");
    out.push_str("    type Dst = Self;\n\n");
    out.push_str(
        "    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> \
         {\n",
    );

    // Discriminator check
    if discriminator.len() == 1 {
        out.push_str("        let disc = reader.take_byte()?;\n");
        writeln!(out, "        if disc != {} {{", discriminator[0]).expect("write to String");
    } else {
        writeln!(
            out,
            "        let disc = reader.take_array::<{}>()?;",
            discriminator.len()
        )
        .expect("write to String");
        let disc_str = super::format_disc_decimal(discriminator);
        writeln!(out, "        if disc != [{disc_str}] {{").expect("write to String");
    }
    let disc_kind = if kind == "account" {
        "account discriminator"
    } else {
        "event discriminator"
    };
    writeln!(
        out,
        "            return Err(ReadError::InvalidValue(\"invalid {disc_kind}\"));"
    )
    .expect("write to String");
    out.push_str("        }\n");

    if !has_dynamic {
        // No dynamic fields: simple sequential read.
        out.push_str("        dst.write(Self {\n");
        for (field_name, field_type, _) in &fixed_fields {
            writeln!(
                out,
                "            {field_name}: <{field_type} as SchemaRead<'de, \
                 C>>::get(reader.by_ref())?,"
            )
            .expect("write to String");
        }
        out.push_str("        });\n");
    } else {
        // Compact layout: read fixed fields, then length table, then tail data

        // Phase 1: fixed fields
        for (field_name, field_type, _) in &fixed_fields {
            writeln!(
                out,
                "        let {field_name} = <{field_type} as SchemaRead<'de, \
                 C>>::get(reader.by_ref())?;"
            )
            .expect("write to String");
        }

        // Phase 2: length table
        for (field_name, _, idl_f) in &dyn_fields {
            let pfx = dynamic_prefix_bytes_from_codec(&idl_f.codec);
            writeln!(out, "        let {field_name}_len = {{").expect("write to String");
            writeln!(out, "            let mut buf = [0u8; 8];").expect("write to String");
            writeln!(
                out,
                "            let pfx_bytes = reader.take_scoped({pfx})?;"
            )
            .expect("write to String");
            writeln!(out, "            buf[..{pfx}].copy_from_slice(pfx_bytes);")
                .expect("write to String");
            writeln!(
                out,
                "            usize::try_from(u64::from_le_bytes(buf))\n\x20               \
                 .map_err(|_| ReadError::PointerSizedReadError)?"
            )
            .expect("write to String");
            out.push_str("        };\n");
        }

        // Phase 3: tail data
        for (field_name, field_type, idl_f) in &dyn_fields {
            if is_dynamic_string(&idl_f.ty) {
                writeln!(
                    out,
                    "        let {field_name}: {field_type} = {{\n\
                     \x20           let bytes = reader.take_scoped({field_name}_len)?;\n\
                     \x20           core::str::from_utf8(bytes)?;\n\
                     \x20           bytes.to_vec().into()\n\
                     \x20       }};"
                )
                .expect("write to String");
            } else if is_dynamic_vec(&idl_f.ty) {
                let item_ty = rust_field_type(vec_inner_type(&idl_f.ty).unwrap(), &None);
                writeln!(out, "        let {field_name}: {field_type} = {{")
                    .expect("write to String");
                writeln!(
                    out,
                    "            const MAX_DECODE_ELEMENTS: usize = 10 * 1024 * 1024;"
                )
                .expect("write to String");
                writeln!(
                    out,
                    "            if {field_name}_len > MAX_DECODE_ELEMENTS {{"
                )
                .expect("write to String");
                writeln!(
                    out,
                    "                return Err(ReadError::PreallocationSizeLimit {{ needed: \
                     {field_name}_len, limit: MAX_DECODE_ELEMENTS }});"
                )
                .expect("write to String");
                out.push_str("            }\n");
                writeln!(
                    out,
                    "            let mut items = Vec::with_capacity({field_name}_len.min(4096));"
                )
                .expect("write to String");
                writeln!(out, "            for _ in 0..{field_name}_len {{")
                    .expect("write to String");
                writeln!(
                    out,
                    "                items.push(<{item_ty} as SchemaRead<'de, \
                     C>>::get(reader.by_ref())?);"
                )
                .expect("write to String");
                out.push_str("            }\n");
                out.push_str("            items.into()\n");
                out.push_str("        };\n");
            }
        }

        // Assemble struct in IDL order.
        out.push_str("        dst.write(Self {\n");
        for (field_name, _) in &fields {
            writeln!(out, "            {field_name},").expect("write to String");
        }
        out.push_str("        });\n");
    }

    out.push_str("        Ok(())\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
}

fn account_meta_expr(account: &IdlAccountNode) -> String {
    let field_name = camel_to_snake(&account.name);
    let signer = matches!(account.signer, AccountFlag::Fixed(true));
    // Optional accounts default an absent slot to the program id sentinel.
    let key = if account.optional {
        format!("ix.{field_name}.unwrap_or(ID)")
    } else if let IdlResolver::Const { address } = &account.resolver {
        format!("solana_address::address!(\"{address}\")")
    } else {
        format!("ix.{field_name}")
    };
    if matches!(account.writable, AccountFlag::Fixed(true)) {
        format!("AccountMeta::new({}, {})", key, signer)
    } else {
        format!("AccountMeta::new_readonly({}, {})", key, signer)
    }
}

/// Map an `IdlType` to its Rust field type for the client struct.
///
/// Resolution (scalar widths, size-prefix widths, option tags) is computed once
/// by [`WireType`]; this function only renders the Rust spelling of the wire
/// type. Codec-less dynamic types never reach a generated client — the IDL
/// producer mandates a size-prefix codec for every dynamic (P009) — so the
/// `Err` arm is dead in practice and renders the legacy bare spelling only so a
/// hand-authored, codec-omitting IDL stays byte-identical.
fn rust_field_type(ty: &IdlType, codec: &Option<IdlCodec>) -> String {
    match WireType::resolve(ty, codec) {
        Ok(wire) => rust_wire_type(&wire),
        Err(_) => rust_field_type_codecless(ty),
    }
}

/// Render a resolved [`WireType`] as its Rust field-type spelling. Pod aliases
/// (`PodU64`, …) stay as their defined name — the Rust client keeps them,
/// unlike the TypeScript client which folds them to primitives.
fn rust_wire_type(wire: &WireType) -> String {
    match wire {
        WireType::Bool => "bool".to_string(),
        WireType::Scalar {
            width,
            signed,
            float,
        } => rust_scalar(*width, *signed, *float).to_string(),
        WireType::Pubkey => "Address".to_string(),
        WireType::Bytes => "bytes".to_string(),
        WireType::FixedBytes(size) => format!("[u8; {size}]"),
        WireType::Array { len, item } => format!("[{}; {}]", rust_wire_type(item), len),
        WireType::Str { prefix } => format!("DynString<{}>", prefix_rust_type(*prefix)),
        WireType::List { prefix, item } => {
            format!(
                "DynVec<{}, {}>",
                rust_wire_type(item),
                prefix_rust_type(*prefix)
            )
        }
        WireType::Option { inner, .. } => format!("Option<{}>", rust_wire_type(inner)),
        WireType::Defined(name) => name.clone(),
    }
}

/// Rust primitive spelling for a fixed-width scalar.
fn rust_scalar(width: u8, signed: bool, float: bool) -> &'static str {
    match (width, signed, float) {
        (4, _, true) => "f32",
        (8, _, true) => "f64",
        (1, false, _) => "u8",
        (2, false, _) => "u16",
        (4, false, _) => "u32",
        (8, false, _) => "u64",
        (16, false, _) => "u128",
        (1, true, _) => "i8",
        (2, true, _) => "i16",
        (4, true, _) => "i32",
        (8, true, _) => "i64",
        (16, true, _) => "i128",
        _ => "u8",
    }
}

/// Legacy spelling for a codec-less dynamic type (dead in practice; see
/// [`rust_field_type`]).
fn rust_field_type_codecless(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) if p == "string" => "String".to_string(),
        IdlType::Vec { vec } => format!("Vec<{}>", rust_field_type(vec, &None)),
        IdlType::Option { option } => format!("Option<{}>", rust_field_type(option, &None)),
        _ => ty_to_string(ty),
    }
}

/// Fallback rendering for an unexpected non-dynamic type in the codec-less
/// path.
fn ty_to_string(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => p.clone(),
        IdlType::Defined { defined } => defined.name.clone(),
        IdlType::Generic { generic } => generic.clone(),
        _ => String::new(),
    }
}

/// Map a size-prefix byte width to its Rust unsigned-integer type.
fn prefix_rust_type(prefix_bytes: u8) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        _ => "u64",
    }
}

fn collect_wrapper_needs(
    ty: &IdlType,
    codec: &Option<IdlCodec>,
    needs_dyn_string: &mut bool,
    needs_dyn_vec: &mut bool,
) {
    match ty {
        IdlType::Option { option } => {
            if optional_dynamic_inner(ty).is_some() {
                collect_wrapper_needs(option, codec, needs_dyn_string, needs_dyn_vec)
            } else {
                collect_wrapper_needs(option, &None, needs_dyn_string, needs_dyn_vec)
            }
        }
        IdlType::Primitive(p) if p == "string" => {
            if matches!(codec, Some(IdlCodec::SizePrefixed { .. })) {
                *needs_dyn_string = true;
            }
        }
        IdlType::Vec { vec } => {
            if matches!(codec, Some(IdlCodec::SizePrefixed { .. })) {
                *needs_dyn_vec = true;
            }
            collect_wrapper_needs(vec, &None, needs_dyn_string, needs_dyn_vec);
        }
        _ => {}
    }
}

fn field_needs_address(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(p) => p == "pubkey",
        IdlType::Option { option } => field_needs_address(option),
        IdlType::Vec { vec } => field_needs_address(vec),
        IdlType::Array { array: (inner, _) } => field_needs_address(inner),
        _ => false,
    }
}

fn emit_wrapper_imports(out: &mut String, needs_dyn_string: bool, needs_dyn_vec: bool) {
    let mut wrappers = Vec::new();
    if needs_dyn_string {
        wrappers.push("DynString");
    }
    if needs_dyn_vec {
        wrappers.push("DynVec");
    }
    if !wrappers.is_empty() {
        writeln!(out, "use quasar_lang::client::{{{}}};", wrappers.join(", "))
            .expect("write to String");
    }
}

fn emit_type_use_imports(
    out: &mut String,
    ty: &IdlType,
    type_map: &HashMap<String, Vec<IdlFieldDef>>,
) {
    match ty {
        IdlType::Defined { defined } if type_map.contains_key(&defined.name) => {
            let import = format!("use crate::types::{};\n", defined.name);
            if !out.contains(&import) {
                out.push_str(&import);
            }
        }
        IdlType::Option { option } => emit_type_use_imports(out, option, type_map),
        IdlType::Vec { vec } => emit_type_use_imports(out, vec, type_map),
        IdlType::Array { array: (inner, _) } => emit_type_use_imports(out, inner, type_map),
        _ => {}
    }
}
