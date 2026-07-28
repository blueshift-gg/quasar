use super::*;

pub(super) fn emit_instruction_builders(
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

        let user_accs: Vec<_> = ix
            .accounts
            .iter()
            .filter(|account| {
                account_source(account)
                    .expect("validated account resolver")
                    .is_input()
            })
            .collect();

        let input_account_names: HashSet<&str> =
            user_accs.iter().map(|a| a.name.as_str()).collect();
        let optional_account_names: HashSet<&str> = user_accs
            .iter()
            .filter(|account| account.optional)
            .map(|account| account.name.as_str())
            .collect();
        let has_input = instruction_has_input(ix);
        let ix_has_pdas = ix.accounts.iter().any(|account| {
            !account.optional
                && matches!(
                    account.resolver,
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
                format!("(accountOverrides.{name} ?? {})", binding_name(name))
            }
        };

        // Every PDA/ATA the builder derives is surfaced on the returned
        // instruction as a `{field}Address` property, so callers name it
        // directly instead of re-running the PDA/ATA recipe.
        let address_accessors = builder_address_accessors(ix, &account_expr);
        let accessor_type = builder_address_accessor_type(&address_accessors);

        let mut method_params = Vec::new();
        if has_input {
            method_params.push(format!("input: {pascal}InstructionInput"));
        }
        let ix_needs_async = ix_has_pdas;
        let async_kw = if ix_needs_async { "async " } else { "" };
        let return_type = if ix_needs_async {
            format!("Promise<TransactionInstruction{accessor_type}>")
        } else {
            "TransactionInstruction".to_string()
        };
        if !ix.accounts.is_empty() {
            writeln!(
                out,
                "  {async_kw}create{pascal}Instruction({}): {return_type} {{",
                method_params.join(", ")
            )
            .expect("write to String");
            let mut raw_args = Vec::new();
            if has_input {
                raw_args.push("input");
            }
            raw_args.push("{}");
            writeln!(
                out,
                "    return this.create{pascal}InstructionRaw({});",
                raw_args.join(", ")
            )
            .expect("write to String");
            out.push_str("  }\n\n");

            let override_position = usize::from(has_input);
            method_params.insert(
                override_position,
                format!("accountOverrides: {pascal}InstructionAccountOverrides"),
            );
        }
        writeln!(
            out,
            "  {async_kw}create{pascal}Instruction{}({}): {return_type} {{",
            if ix.accounts.is_empty() { "" } else { "Raw" },
            method_params.join(", ")
        )
        .expect("write to String");

        emit_account_bindings(
            out,
            ix,
            &BindingContext {
                idl,
                target: TsTarget::Web3js,
                program_id_expr: &format!("{class_name}.programId"),
                exportable_pda_helpers,
                arg_types: &arg_types,
            },
            &account_expr,
        );

        let disc = crate::codegen::format_disc_decimal(&ix.discriminator);
        let has_dynamic_args = ix.args.iter().any(is_arg_dynamic);
        if ix.args.is_empty() {
            writeln!(out, "    const data = Uint8Array.from([{}]);", disc)
                .expect("write to String");
        } else if !has_dynamic_args {
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
                .map(|arg| format!("{}: input.{}", arg.name, arg.name))
                .collect();
            writeln!(
                out,
                "    const data = Uint8Array.from([{}, ...argsCodec.encode({{ {} }})]);",
                disc,
                arg_names.join(", ")
            )
            .expect("write to String");
        } else {
            emit_compact_encoding(out, ix, &disc, TsTarget::Web3js, "Uint8Array.from");
        }

        if address_accessors.is_empty() {
            out.push_str("    return new TransactionInstruction({\n");
        } else {
            out.push_str("    const instruction = new TransactionInstruction({\n");
        }
        writeln!(out, "      programId: {class_name}.programId,").expect("write to String");
        if !ix.accounts.is_empty() || has_remaining {
            out.push_str("      keys: [\n");
            for account in &ix.accounts {
                let pubkey_expr = account_expr(&account.name);
                let is_signer = matches!(account.signer, AccountFlag::Fixed(true));
                let is_writable = matches!(account.writable, AccountFlag::Fixed(true));
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
        // `TransactionInstruction` is a class, so the derived-address accessors
        // are attached after construction; structural typing keeps the result a
        // valid instruction for send.
        if !address_accessors.is_empty() {
            out.push_str("    return Object.assign(instruction, {\n");
            for accessor in &address_accessors {
                writeln!(out, "      {}: {},", accessor.property, accessor.value_expr)
                    .expect("write to String");
            }
            out.push_str("    });\n");
        }
        out.push_str("  }\n");
    }
}
