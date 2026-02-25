use serde::Serialize;

#[derive(Serialize)]
pub struct Idl {
    pub address: String,
    pub metadata: IdlMetadata,
    pub instructions: Vec<IdlInstruction>,
    pub accounts: Vec<IdlAccountDef>,
    pub events: Vec<IdlEventDef>,
    pub types: Vec<IdlTypeDef>,
    pub errors: Vec<IdlError>,
}

#[derive(Serialize)]
pub struct IdlMetadata {
    pub name: String,
    pub version: String,
    pub spec: String,
}

#[derive(Serialize)]
pub struct IdlInstruction {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub accounts: Vec<IdlAccountItem>,
    pub args: Vec<IdlField>,
}

#[derive(Serialize)]
pub struct IdlAccountItem {
    pub name: String,
    #[serde(skip_serializing_if = "is_false")]
    pub writable: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub signer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pda: Option<IdlPda>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Serialize)]
pub struct IdlPda {
    pub seeds: Vec<IdlSeed>,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
pub enum IdlSeed {
    #[serde(rename = "const")]
    Const { value: Vec<u8> },
    #[serde(rename = "account")]
    Account { path: String },
}

#[derive(Serialize)]
pub struct IdlField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlType,
}

#[derive(Serialize)]
pub struct IdlDynString {
    #[serde(rename = "maxLength")]
    pub max_length: usize,
}

#[derive(Serialize)]
pub struct IdlDynVec {
    pub items: Box<IdlType>,
    #[serde(rename = "maxLength")]
    pub max_length: usize,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum IdlType {
    Primitive(String),
    Defined { defined: String },
    DynString { string: IdlDynString },
    DynVec { vec: IdlDynVec },
}

#[derive(Serialize)]
pub struct IdlAccountDef {
    pub name: String,
    pub discriminator: Vec<u8>,
}

#[derive(Serialize)]
pub struct IdlEventDef {
    pub name: String,
    pub discriminator: Vec<u8>,
}

#[derive(Serialize)]
pub struct IdlTypeDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: IdlTypeDefType,
}

#[derive(Serialize)]
pub struct IdlTypeDefType {
    pub kind: String,
    pub fields: Vec<IdlField>,
}

#[derive(Serialize)]
pub struct IdlError {
    pub code: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}
