use super::*;

pub(super) fn emit_program_plugin(
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
        for instruction in instructions {
            let instruction_pascal = snake_to_pascal(&instruction.name);
            let instruction_camel = to_camel_case(&instruction.name);
            if plugin_instruction_has_input(instruction) {
                writeln!(
                    out,
                    "        {instruction_camel}: (input: {instruction_pascal}InstructionInput) => \
                     addSelfPlanAndSendFunctions(client, \
                     __client.create{instruction_pascal}Instruction(input)),"
                )
                .expect("write to String");
            } else {
                writeln!(
                    out,
                    "        {instruction_camel}: () => addSelfPlanAndSendFunctions(client, \
                     __client.create{instruction_pascal}Instruction()),"
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

pub(super) fn eligible_plugin_accounts(idl: &Idl) -> Vec<&IdlAccountDef> {
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

pub(super) fn eligible_plugin_instructions(idl: &Idl) -> Vec<&IdlInstruction> {
    idl.instructions
        .iter()
        .filter(|instruction| !instruction_has_account_field_pda_seeds(instruction))
        .collect()
}

fn plugin_instruction_has_input(instruction: &IdlInstruction) -> bool {
    instruction.remaining_accounts.is_some()
        || !instruction.args.is_empty()
        || instruction
            .accounts
            .iter()
            .any(|account| account.optional || matches!(account.resolver, IdlResolver::Input {}))
}

fn lower_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

pub(super) fn emit_instruction_builders(
    out: &mut String,
    idl: &Idl,
    exportable_pda_helpers: &HashMap<String, String>,
) {
    for instruction in &idl.instructions {
        let has_remaining = instruction.remaining_accounts.is_some();
        out.push('\n');
        let pascal = snake_to_pascal(&instruction.name);
        let arg_types = instruction_arg_types(instruction);

        let mut user_accounts = Vec::new();
        let mut has_non_input_accounts = false;
        for account in &instruction.accounts {
            if account.optional || matches!(account.resolver, IdlResolver::Input {}) {
                user_accounts.push(account);
            } else {
                has_non_input_accounts = true;
            }
        }

        let input_account_names: HashSet<&str> = user_accounts
            .iter()
            .map(|account| account.name.as_str())
            .collect();
        let optional_account_names: HashSet<&str> = user_accounts
            .iter()
            .filter(|account| account.optional)
            .map(|account| account.name.as_str())
            .collect();
        let needs_account_resolver = instruction_has_account_field_pda_seeds(instruction);

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

        let has_pdas = instruction.accounts.iter().any(|account| {
            !account.optional
                && matches!(
                    account.resolver,
                    IdlResolver::Pda { .. } | IdlResolver::AssociatedToken { .. }
                )
        });

        let mut method_params = Vec::new();
        if !user_accounts.is_empty() || !instruction.args.is_empty() || has_remaining {
            method_params.push(format!("input: {pascal}InstructionInput"));
        }
        if needs_account_resolver {
            method_params.push("resolver: AccountDataResolver".to_string());
        }
        let return_type = if has_pdas {
            "Promise<Instruction>"
        } else {
            "Instruction"
        };
        let async_keyword = if has_pdas { "async " } else { "" };
        if !instruction.accounts.is_empty() {
            writeln!(
                out,
                "  {async_keyword}create{pascal}Instruction({}): {return_type} {{",
                method_params.join(", ")
            )
            .expect("write to String");
            let mut unchecked_args = Vec::new();
            if !user_accounts.is_empty() || !instruction.args.is_empty() || has_remaining {
                unchecked_args.push("input");
            }
            unchecked_args.push("{}");
            if needs_account_resolver {
                unchecked_args.push("resolver");
            }
            writeln!(
                out,
                "    return this.create{pascal}InstructionUnchecked({});",
                unchecked_args.join(", ")
            )
            .expect("write to String");
            out.push_str("  }\n\n");

            let override_position =
                if !user_accounts.is_empty() || !instruction.args.is_empty() || has_remaining {
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
            "  {async_keyword}create{pascal}Instruction{}({}): {return_type} {{",
            if instruction.accounts.is_empty() {
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

        for account in &instruction.accounts {
            if account.optional {
                continue;
            }
            if let IdlResolver::Const { address: value } = &account.resolver {
                writeln!(
                    out,
                    "    accountsMap[\"{}\"] = address(\"{}\");",
                    account.name, value
                )
                .expect("write to String");
            }
        }

        for account in resolved_account_order(instruction).expect("validated derived-account order")
        {
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
                        IdlPdaProgram::ProgramId {} => "PROGRAM_ADDRESS".to_string(),
                        IdlPdaProgram::Account { path } => account_expr(path),
                    };
                    emit_inline_pda_derivation(
                        out,
                        &account.name,
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
            } = &account.resolver
            {
                emit_associated_token_derivation(
                    out,
                    &account.name,
                    mint,
                    owner,
                    token_program.as_deref(),
                    TsTarget::Kit,
                    &account_expr,
                );
            }
        }

        let disc = crate::codegen::format_disc_decimal(&instruction.discriminator);
        let has_dynamic_args = instruction.args.iter().any(is_arg_dynamic);
        if instruction.args.is_empty() {
            writeln!(out, "    const data = Uint8Array.from([{}]);", disc)
                .expect("write to String");
        } else if !has_dynamic_args {
            out.push_str("    const argsCodec = getStructCodec([\n");
            for arg in &instruction.args {
                writeln!(
                    out,
                    "      [\"{}\", {}],",
                    arg.name,
                    ts_codec_for_arg(arg, TsTarget::Kit)
                )
                .expect("write to String");
            }
            out.push_str("    ]);\n");
            let arg_names: Vec<String> = instruction
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
            emit_compact_encoding(out, instruction, &disc, TsTarget::Kit, "Uint8Array.from");
        }

        out.push_str("    return {\n");
        out.push_str("      programAddress: PROGRAM_ADDRESS,\n");
        if !instruction.accounts.is_empty() || has_remaining {
            out.push_str("      accounts: [\n");
            for account in &instruction.accounts {
                let address_expr = account_expr(&account.name);
                let is_signer = matches!(account.signer, AccountFlag::Fixed(true));
                let is_writable = matches!(account.writable, AccountFlag::Fixed(true));
                let role = account_role(is_signer, is_writable);
                writeln!(
                    out,
                    "        {{ address: {}, role: {} }},",
                    address_expr, role
                )
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
