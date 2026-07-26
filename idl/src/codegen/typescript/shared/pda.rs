use {
    super::{codec::ts_type, InlinePdaTarget, TsTarget},
    crate::{
        codegen::{
            model::{account_field_definition, account_field_seed_inputs},
            naming::snake_to_pascal,
        },
        types::{Idl, IdlPdaProgram, IdlPdaSeed, IdlResolver, IdlType},
    },
    std::{
        collections::{HashMap, HashSet},
        fmt::Write,
    },
};

pub(super) fn account_role(signer: bool, writable: bool) -> &'static str {
    match (signer, writable) {
        (true, true) => "AccountRole.WRITABLE_SIGNER",
        (true, false) => "AccountRole.READONLY_SIGNER",
        (false, true) => "AccountRole.WRITABLE",
        (false, false) => "AccountRole.READONLY",
    }
}

#[derive(Clone)]
pub(super) struct PdaParam {
    name: String,
    ty: PdaParamType,
}

#[derive(Clone)]
pub(super) enum PdaParamType {
    Account,
    Arg(IdlType),
}

/// A collected PDA with its field name, seeds, and helper signature params.
pub(super) struct PdaInfo {
    helper_name: String,
    seeds: Vec<IdlPdaSeed>,
    params: Vec<PdaParam>,
}

pub(super) fn collect_pdas(idl: &Idl) -> Vec<PdaInfo> {
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

pub(super) fn emit_pda_helpers(
    out: &mut String,
    pdas: &[PdaInfo],
    target: TsTarget,
    program_name: &str,
) {
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

pub(super) fn ts_pda_helper_name(field_name: &str) -> String {
    format!("find{}Address", snake_to_pascal(field_name))
}

pub(super) fn unique_pda_helper_name(
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

pub(super) fn pda_helper_lookup(pdas: &[PdaInfo]) -> HashMap<String, String> {
    pdas.iter()
        .map(|pda| (format!("{:?}", pda.seeds), pda.helper_name.clone()))
        .collect()
}

pub(super) fn helper_call_args(
    seeds: &[IdlPdaSeed],
    account_expr: &impl Fn(&str) -> String,
) -> String {
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

pub(super) fn write_ts_pda_seed_lines(
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

pub(super) fn emit_inline_pda_derivation(
    out: &mut String,
    account_name: &str,
    seeds: &[IdlPdaSeed],
    idl: &Idl,
    target: InlinePdaTarget<'_>,
    arg_types: &HashMap<String, IdlType>,
    account_expr: &impl Fn(&str) -> String,
) {
    let ts_target = target.target;
    match ts_target {
        TsTarget::Web3js => {
            writeln!(
                out,
                "    accountsMap[\"{}\"] = (await Address.findProgramAddress(",
                account_name
            )
            .expect("write to String");
            out.push_str("      [\n");
            write_inline_pda_seed_lines(out, seeds, idl, ts_target, arg_types, account_expr);
            writeln!(out, "      ],\n      {},\n    ))[0];", target.program_expr)
                .expect("write to String");
        }
        TsTarget::Kit => {
            writeln!(
                out,
                "    accountsMap[\"{}\"] = (await getProgramDerivedAddress({{",
                account_name
            )
            .expect("write to String");
            writeln!(out, "      programAddress: {},", target.program_expr)
                .expect("write to String");
            out.push_str("      seeds: [\n");
            write_inline_pda_seed_lines(out, seeds, idl, ts_target, arg_types, account_expr);
            out.push_str("      ],\n");
            out.push_str("    }))[0];\n");
        }
    }
}

pub(super) fn emit_associated_token_derivation(
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

pub(super) fn write_inline_pda_seed_lines(
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

pub(super) fn instruction_arg_types(ix: &crate::types::IdlInstruction) -> HashMap<String, IdlType> {
    ix.args
        .iter()
        .map(|arg| (arg.name.clone(), arg.ty.clone()))
        .collect()
}

pub(super) fn pda_arg_types(pda: &PdaInfo) -> HashMap<String, IdlType> {
    pda.params
        .iter()
        .filter_map(|param| match &param.ty {
            PdaParamType::Arg(ty) => Some((param.name.clone(), ty.clone())),
            PdaParamType::Account => None,
        })
        .collect()
}

pub(super) fn pda_is_exportable(
    seeds: &[IdlPdaSeed],
    arg_types: &HashMap<String, IdlType>,
) -> bool {
    seeds.iter().all(|seed| match seed {
        IdlPdaSeed::Const { .. } => true,
        IdlPdaSeed::Account { path } => is_identifier(path),
        IdlPdaSeed::AccountField { .. } => false,
        IdlPdaSeed::Arg { path, .. } => is_identifier(path) && arg_types.contains_key(path),
    })
}

pub(super) fn emit_account_field_seed_resolvers(
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

pub(super) fn has_account_field_pda_seeds(idl: &Idl) -> bool {
    idl.instructions
        .iter()
        .any(instruction_has_account_field_pda_seeds)
}

pub(super) fn instruction_has_account_field_pda_seeds(ix: &crate::types::IdlInstruction) -> bool {
    !account_field_seed_inputs(ix).is_empty()
}

pub(super) fn account_field_data_var(path: &str, field: &str) -> String {
    format!("__{}Data", account_field_var_stem(path, field))
}

pub(super) fn account_field_account_var(path: &str, field: &str) -> String {
    format!("__{}Account", account_field_var_stem(path, field))
}

pub(super) fn account_field_seed_var(path: &str, field: &str) -> String {
    format!("__{}Seed", account_field_var_stem(path, field))
}

pub(super) fn account_field_var_stem(path: &str, field: &str) -> String {
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

pub(super) fn ts_field_access(field: &str) -> String {
    field.split('.').collect::<Vec<_>>().join(".")
}

pub(super) fn is_identifier(path: &str) -> bool {
    let mut chars = path.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(super) fn ts_pda_arg_seed_expr(expr: &str, ty: &IdlType, target: TsTarget) -> String {
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

/// Write a `new Uint8Array([...])` seed line directly to the output.
pub(super) fn write_byte_array(out: &mut String, value: &[u8]) {
    out.push_str("        new Uint8Array([");
    for (i, b) in value.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write!(out, "{}", b).expect("write to String");
    }
    out.push_str("]),\n");
}
