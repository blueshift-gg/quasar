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
                format!("(accountOverrides.{name} ?? accountsMap[\"{name}\"])")
            }
        };

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
            let mut raw_args = Vec::new();
            if !user_accs.is_empty() || !ix.args.is_empty() || has_remaining {
                raw_args.push("input");
            }
            raw_args.push("{}");
            if ix_needs_account_resolver {
                raw_args.push("resolver");
            }
            writeln!(
                out,
                "    return this.create{pascal}InstructionRaw({});",
                raw_args.join(", ")
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
            if ix.accounts.is_empty() { "" } else { "Raw" },
            method_params.join(", ")
        )
        .expect("write to String");

        if has_non_input_accounts {
            out.push_str("    const accountsMap: Record<string, Address> = {};\n");
        }

        for account in &ix.accounts {
            if account.optional {
                continue;
            }
            if let IdlResolver::Const { address } = &account.resolver {
                writeln!(
                    out,
                    "    accountsMap[\"{}\"] = new Address(\"{}\");",
                    account.name, address
                )
                .expect("write to String");
            }
        }

        for account in resolved_account_order(ix).expect("validated derived-account order") {
            if let IdlResolver::Pda { program, seeds } = &account.resolver {
                let helper_name = matches!(program, IdlPdaProgram::ProgramId {})
                    .then(|| exportable_pda_helpers.get(&format!("{:?}", seeds)))
                    .flatten();
                if let Some(helper_name) = helper_name {
                    let args = helper_call_args(seeds, &account_expr);
                    writeln!(
                        out,
                        "    accountsMap[\"{}\"] = await {}({});",
                        account.name, helper_name, args
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
                        &account.name,
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
            } = &account.resolver
            {
                emit_associated_token_derivation(
                    out,
                    &account.name,
                    mint,
                    owner,
                    token_program.as_deref(),
                    TsTarget::Web3js,
                    &account_expr,
                );
            }
        }

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

        out.push_str("    return new TransactionInstruction({\n");
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
        out.push_str("  }\n");
    }
}
