//! Builds instructions for a program on the host, for tests and Rust clients.

extern crate alloc;

pub use alloc::{format, string::String, vec::Vec};
pub use solana_instruction::{AccountMeta, Instruction};

use crate::{Address, input::Privileges, wrapper::Wrapper};

/// Account keys for an accounts struct, in declaration order.
pub trait ClientAccounts {
    type Keys;

    /// `program` stands in for absent optional accounts.
    fn metas(keys: Self::Keys, program: Address) -> Vec<AccountMeta>;

    /// Each field, in declaration order, as the IDL describes it.
    fn idl_accounts() -> Vec<IdlAccount>;

    /// The kinds the struct's `Account<K>` fields hold.
    fn idl_kinds() -> Vec<IdlKind>;
}

/// A program's interface, built from the constants the program itself uses.
pub struct Idl {
    pub address: Address,
    pub name: &'static str,
    pub instructions: Vec<IdlInstruction>,
    pub kinds: Vec<IdlKind>,
}

pub struct IdlInstruction {
    pub name: &'static str,
    pub discriminator: &'static [u8],
    pub accounts: Vec<IdlAccount>,
    pub args: Vec<IdlField>,
}

pub struct IdlAccount {
    pub name: &'static str,
    pub writable: bool,
    pub signer: bool,
    pub optional: bool,
    /// Every account after the declared ones.
    pub remaining: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IdlKind {
    pub name: &'static str,
    pub discriminator: &'static [u8],
    /// The data length, when every account of the kind has the same.
    pub size: Option<usize>,
    pub fields: &'static [IdlField],
    /// Declared PDA seeds, independent of instruction constraints.
    pub seeds: Option<&'static [IdlSeed]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IdlSeed {
    Const(&'static [u8]),
    Param(IdlField),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct IdlField {
    pub name: &'static str,
    /// The type as the program's source writes it.
    pub ty: &'static str,
}

impl Idl {
    /// The IDL as JSON, in a fixed order, so it can be committed and diffed.
    pub fn to_json(&self) -> String {
        let instructions = self.instructions.iter().map(|ix| {
            let accounts = ix.accounts.iter().map(|account| {
                let IdlAccount { name, writable, signer, optional, remaining } = account;
                format!(
                    r#"        {{ "name": "{name}", "writable": {writable}, "signer": {signer}, "optional": {optional}, "remaining": {remaining} }}"#
                )
            });
            format!(
                r#"    {{
      "name": "{name}",
      "discriminator": {discriminator:?},
      "accounts": [
{accounts}
      ],
      "args": [{args}]
    }}"#,
                name = ix.name,
                discriminator = ix.discriminator,
                accounts = accounts.collect::<Vec<_>>().join(",\n"),
                args = fields(&ix.args),
            )
        });
        let kinds = self.kinds.iter().map(|kind| {
            let IdlKind { name, discriminator, size, fields: kind_fields, seeds } = kind;
            let size = size.map_or("null".into(), |size| format!("{size}"));
            let kind_fields = fields(kind_fields);
            let seeds = seeds.map_or(String::new(), |seeds| {
                let seeds = seeds.iter().map(|seed| match seed {
                    IdlSeed::Const(value) => format!(r#"{{ "kind": "const", "value": {value:?} }}"#),
                    IdlSeed::Param(IdlField { name, ty }) => format!(r#"{{ "kind": "param", "name": "{name}", "type": "{ty}" }}"#),
                }).collect::<Vec<_>>().join(", ");
                format!(r#", "seeds": [{seeds}]"#)
            });
            format!(
                r#"    {{ "name": "{name}", "discriminator": {discriminator:?}, "size": {size}, "fields": [{kind_fields}]{seeds} }}"#
            )
        });
        format!(
            r#"{{
  "address": "{address}",
  "name": "{name}",
  "instructions": [
{instructions}
  ],
  "accounts": [
{kinds}
  ]
}}
"#,
            address = self.address,
            name = self.name,
            instructions = instructions.collect::<Vec<_>>().join(",\n"),
            kinds = kinds.collect::<Vec<_>>().join(",\n"),
        )
    }
}

fn fields(fields: &[IdlField]) -> String {
    let fields = fields
        .iter()
        .map(|field| format!(r#"{{ "name": "{}", "type": "{}" }}"#, field.name, field.ty));
    fields.collect::<Vec<_>>().join(", ")
}

/// A field as the IDL describes it; `W` decides whether it signs.
pub fn idl_account<W: Wrapper>(
    name: &'static str,
    writable: bool,
    optional: bool,
    remaining: bool,
) -> IdlAccount {
    let signer = W::REQUIRES.includes(Privileges::SIGNER);
    IdlAccount { name, writable, signer, optional, remaining }
}

/// The meta for an account that must satisfy `W`, writable if declared `mut`.
pub fn meta<W: Wrapper>(key: Address, writable: bool) -> AccountMeta {
    AccountMeta {
        pubkey: key,
        is_signer: W::REQUIRES.includes(Privileges::SIGNER),
        is_writable: writable,
    }
}

/// The meta for an optional account; the program's own address marks it absent.
pub fn optional_meta<W: Wrapper>(
    key: Option<Address>,
    writable: bool,
    program: Address,
) -> AccountMeta {
    key.map_or(AccountMeta::new_readonly(program, false), |key| meta::<W>(key, writable))
}
