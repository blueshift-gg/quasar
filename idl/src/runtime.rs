//! Adapt the runtime's small interface to the existing client schema. Do not
//! infer resolvers or constraints that the runtime does not describe.
use crate::types::Idl;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Interface {
    address: String,
    name: String,
    instructions: Vec<Instruction>,
    accounts: Vec<Account>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Instruction {
    name: String,
    discriminator: Vec<u8>,
    accounts: Vec<Meta>,
    args: Vec<Field>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta {
    name: String,
    writable: bool,
    signer: bool,
    optional: bool,
    remaining: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    name: String,
    discriminator: Vec<u8>,
    size: Option<usize>,
    fields: Vec<Field>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Field {
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

/// Read either a public schema IDL or the interface emitted by Quasar 0.3.
/// Unknown type encodings fail explicitly instead of producing broken clients.
pub fn read(json: &str) -> Result<Idl, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if value.get("spec").is_some() {
        return serde_json::from_value(value).map_err(|e| e.to_string());
    }
    let runtime: Interface = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut types = Vec::new();
    let mut accounts = Vec::new();
    for account in runtime.accounts {
        let fields = fields(account.fields, &mut types)?;
        types.push(json!({"name": account.name, "kind": "struct", "fields": fields}));
        let space = account.size.map(
            |size| json!({"min": size, "max": size, "discriminator": account.discriminator.len()}),
        );
        accounts.push(
            json!({"name": account.name, "discriminator": account.discriminator, "space": space}),
        );
    }
    let mut instructions = Vec::new();
    for instruction in runtime.instructions {
        let mut accounts = Vec::new();
        let mut remaining = None;
        let count = instruction.accounts.len();
        for (index, account) in instruction.accounts.into_iter().enumerate() {
            if account.remaining {
                if index + 1 != count || account.optional {
                    return Err("remaining accounts must be the final, non-optional field".into());
                }
                remaining = Some(json!({
                    "kind": "append", "name": account.name, "min": 0, "max": null,
                    "item": {"clientType": "AccountMeta", "signer": account.signer, "writable": account.writable},
                    "policy": {"position": "afterDeclaredAccounts", "order": "preserveInput"}
                }));
            } else {
                accounts.push(json!({"name": account.name, "writable": account.writable, "signer": account.signer, "optional": account.optional, "resolver": {"kind": "input"}}));
            }
        }
        instructions.push(json!({"name": quasar_schema::to_camel_case(&instruction.name), "discriminator": instruction.discriminator, "accounts": accounts, "args": fields(instruction.args, &mut types)?, "remainingAccounts": remaining}));
    }
    serde_json::from_value(json!({
        "spec": "quasar-idl/1.0.0", "name": runtime.name, "version": "0.0.0",
        "address": runtime.address, "instructions": instructions, "accounts": accounts, "types": types,
        "metadata": {"extra": {"encoding": "borsh", "interface": "quasar-0.3"}}
    })).map_err(|e| e.to_string())
}

fn fields(fields: Vec<Field>, types: &mut Vec<Value>) -> Result<Vec<Value>, String> {
    fields
        .into_iter()
        .map(|field| Ok(json!({"name": field.name, "type": field_type(&field.ty, types)?})))
        .collect()
}

fn field_type(ty: &str, types: &mut Vec<Value>) -> Result<Value, String> {
    let ty: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    let scalar = match ty.as_str() {
        "Address" | "Pubkey" => "pubkey",
        "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "f32" | "f64" => &ty,
        "String" => "string",
        _ => "",
    };
    if !scalar.is_empty() {
        return Ok(json!(scalar));
    }
    if let Some(body) = ty.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
        && let Some((item, count)) = body.rsplit_once(';')
    {
        let count =
            count.parse::<usize>().map_err(|_| format!("array length must be a literal: {ty}"))?;
        return Ok(json!({"array": [field_type(item, types)?, count]}));
    }
    if let Some((name, body)) =
        ty.split_once('<').and_then(|(n, b)| b.strip_suffix('>').map(|b| (n, b)))
    {
        let args = arguments(body);
        match (name, args.as_slice()) {
            ("String", [_]) => return Ok(json!("string")),
            ("Vec", [item, ..]) => return Ok(json!({"vec": field_type(item, types)?})),
            ("Option", [item]) => return Ok(json!({"option": field_type(item, types)?})),
            ("ArrayString", [count]) | ("ArrayVec", [_, count]) => {
                let count = count
                    .parse::<usize>()
                    .map_err(|_| format!("capacity must be a literal: {ty}"))?;
                let item =
                    if name == "ArrayString" { json!("u8") } else { field_type(args[0], types)? };
                // Expose the fixed-capacity wire representation explicitly: u32
                // count followed by every slot, including unused padding.
                let name: String = ty.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                if !types.iter().any(|t| t["name"] == name) {
                    types.push(json!({"name": name, "kind": "struct", "fields": [
                        {"name": "length", "type": "u32"},
                        {"name": "storage", "type": {"array": [item, count]}}
                    ]}));
                }
                return Ok(json!({"defined": {"name": name}}));
            }
            _ => {}
        }
    }
    Err(format!(
        "the runtime IDL does not describe the encoding of `{ty}`; use a supported field type or supply a complete schema IDL"
    ))
}

fn arguments(body: &str) -> Vec<&str> {
    let mut depth = 0;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, c) in body.char_indices() {
        match c {
            '<' | '[' => depth += 1,
            '>' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_all_rewrite_examples() {
        for json in [
            include_str!("../../examples/vault/idl.json"),
            include_str!("../../examples/lamport-vault/idl.json"),
            include_str!("../../examples/escrow/idl.json"),
            include_str!("../../examples/multisig/idl.json"),
            include_str!("../../examples/amm/idl.json"),
        ] {
            let idl = read(json).unwrap();
            assert!(!idl.instructions.is_empty());
            assert_eq!(idl.metadata.extra["encoding"], "borsh");
        }
    }
    #[test]
    fn keeps_flags_and_remaining_accounts() {
        let idl = read(include_str!("../../examples/multisig/idl.json")).unwrap();
        let create = &idl.instructions[0];
        assert_eq!(create.accounts.len(), 4);
        assert!(create.accounts[0].signer.is_true());
        assert!(create.remaining_accounts.as_ref().unwrap().item.signer.is_true());
        assert_eq!(
            idl.types.iter().find(|t| t.name == "MultisigConfig").unwrap().fields[3].ty,
            crate::types::IdlType::Primitive("string".into())
        );
    }
    #[test]
    fn rejects_unreported_custom_types() {
        assert!(field_type("CustomState", &mut Vec::new()).is_err());
    }
}
