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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<IdlReturnData>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<IdlEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "remainingAccounts")]
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

/// Return data specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdlReturnData {
    #[serde(rename = "type")]
    pub ty: IdlType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<IdlCodec>,
}

/// An instruction effect (client-visible side effect metadata).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum IdlEffect {
    #[serde(rename = "createsAccount")]
    CreatesAccount { account: String, payer: String },
    #[serde(rename = "createsAssociatedTokenAccount")]
    CreatesAssociatedTokenAccount { account: String, mint: String, owner: String },
    #[serde(rename = "initializesAccountData")]
    InitializesAccountData { account: String },
    #[serde(rename = "reallocatesAccount")]
    ReallocatesAccount { account: String },
    #[serde(rename = "closesAccount")]
    ClosesAccount { account: String, destination: String },
    #[serde(rename = "migratesAccount")]
    MigratesAccount { account: String },
    #[serde(rename = "transfersLamports")]
    TransfersLamports { from: String, to: String },
    #[serde(rename = "invokesProgram")]
    InvokesProgram { program: String },
    #[serde(rename = "emitsEvent")]
    EmitsEvent { event: String },
    #[serde(rename = "returnsData")]
    ReturnsData {
        #[serde(rename = "type")]
        ty: String,
    },
    #[serde(rename = "requiresRemainingAccounts")]
    RequiresRemainingAccounts {},
}
