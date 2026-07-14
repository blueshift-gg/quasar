use {
    crate::{
        account::{IdlAccountNode, IdlRemainingAccounts},
        codec::IdlCodec,
        layout::IdlLayout,
        types::IdlType,
    },
    serde::{Deserialize, Serialize},
};

/// An instruction definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlInstruction {
    pub name: String,
    pub discriminator: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
    pub accounts: Vec<IdlAccountNode>,
    pub args: Vec<IdlArg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<IdlLayout>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "remainingAccounts"
    )]
    pub remaining_accounts: Option<IdlRemainingAccounts>,
}

/// An instruction argument.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdlArg {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<IdlCodec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
}
