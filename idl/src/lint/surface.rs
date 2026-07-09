use {
    crate::types::*,
    serde::{Deserialize, Serialize},
    std::{
        collections::BTreeMap,
        fmt, io,
        path::{Path, PathBuf},
    },
};

pub const LOCK_FILE_NAME: &str = "quasar.lock.json";
const SURFACE_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgramSurface {
    pub version: u32,
    pub spec: String,
    pub name: String,
    pub program_id: String,
    pub accounts: Vec<AccountSurface>,
    pub instructions: Vec<InstructionSurface>,
    pub types: Vec<TypeSurface>,
    pub events: Vec<EventSurface>,
    pub errors: Vec<ErrorSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountSurface {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub fields: Vec<FieldSurface>,
    pub layout: Option<String>,
    pub space: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstructionSurface {
    pub name: String,
    pub discriminator: Vec<u8>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "discriminatorSource"
    )]
    pub discriminator_source: Option<String>,
    pub args: Vec<FieldSurface>,
    pub accounts: Vec<AccountMetaSurface>,
    pub remaining_accounts: Option<String>,
    pub layout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountMetaSurface {
    pub name: String,
    pub optional: bool,
    pub signer: String,
    pub writable: String,
    pub resolver: String,
    pub resolver_refs: Vec<String>,
    pub pda_seeds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeSurface {
    pub name: String,
    pub kind: String,
    pub fields: Vec<FieldSurface>,
    pub variants: Vec<VariantSurface>,
    pub layout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VariantSurface {
    pub name: String,
    pub value: u64,
    pub fields: Vec<FieldSurface>,
    pub layout: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventSurface {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub fields: Vec<FieldSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorSurface {
    pub name: String,
    pub code: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldSurface {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
}

impl ProgramSurface {
    pub fn from_idl(idl: &Idl) -> Self {
        let type_map: BTreeMap<&str, &IdlTypeDef> =
            idl.types.iter().map(|ty| (ty.name.as_str(), ty)).collect();
        let discriminator_sources = instruction_discriminator_sources(&idl.metadata);

        Self {
            version: SURFACE_VERSION,
            spec: idl.spec.clone(),
            name: idl.name.clone(),
            program_id: idl.address.clone(),
            accounts: idl
                .accounts
                .iter()
                .map(|account| {
                    AccountSurface::from_idl(account, type_map.get(account.name.as_str()).copied())
                })
                .collect(),
            instructions: idl
                .instructions
                .iter()
                .map(|instruction| {
                    InstructionSurface::from_idl(instruction, &discriminator_sources)
                })
                .collect(),
            types: idl.types.iter().map(TypeSurface::from_idl).collect(),
            events: idl
                .events
                .iter()
                .map(|event| EventSurface::from_idl(event, &type_map))
                .collect(),
            errors: idl.errors.iter().map(ErrorSurface::from_idl).collect(),
        }
    }
}

impl ErrorSurface {
    fn from_idl(error: &IdlErrorDef) -> Self {
        Self {
            name: error.name.clone(),
            code: error.code,
        }
    }
}

impl AccountSurface {
    fn from_idl(account: &IdlAccountDef, ty: Option<&IdlTypeDef>) -> Self {
        Self {
            name: account.name.clone(),
            discriminator: account.discriminator.clone(),
            fields: ty
                .map(|ty| ty.fields.iter().map(FieldSurface::from_field).collect())
                .unwrap_or_default(),
            layout: ty.and_then(|ty| ty.layout.as_ref()).map(json_key),
            space: account
                .space
                .as_ref()
                .map(json_key)
                .or_else(|| ty.and_then(|ty| ty.space.as_ref()).map(json_key)),
        }
    }
}

impl InstructionSurface {
    fn from_idl(
        instruction: &IdlInstruction,
        discriminator_sources: &BTreeMap<String, String>,
    ) -> Self {
        Self {
            name: instruction.name.clone(),
            discriminator: instruction.discriminator.clone(),
            discriminator_source: discriminator_sources.get(&instruction.name).cloned(),
            args: instruction
                .args
                .iter()
                .map(FieldSurface::from_arg)
                .collect(),
            accounts: instruction
                .accounts
                .iter()
                .map(AccountMetaSurface::from_idl)
                .collect(),
            remaining_accounts: instruction.remaining_accounts.as_ref().map(json_key),
            layout: instruction.layout.as_ref().map(json_key),
        }
    }

    pub fn account_names(&self) -> Vec<&str> {
        self.accounts
            .iter()
            .map(|account| account.name.as_str())
            .collect()
    }
}

fn instruction_discriminator_sources(metadata: &IdlMetadata) -> BTreeMap<String, String> {
    metadata
        .extra
        .get("quasar:instructionDiscriminatorSource")
        .and_then(serde_json::Value::as_object)
        .map(|sources| {
            sources
                .iter()
                .filter_map(|(name, source)| {
                    source
                        .as_str()
                        .map(|source| (name.clone(), source.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

impl AccountMetaSurface {
    fn from_idl(account: &IdlAccountNode) -> Self {
        Self {
            name: account.name.clone(),
            optional: account.optional,
            signer: flag_key(&account.signer),
            writable: flag_key(&account.writable),
            resolver: json_key(&account.resolver),
            resolver_refs: resolver_refs(&account.resolver),
            pda_seeds: pda_seeds(&account.resolver),
        }
    }

    pub fn signer_required(&self) -> bool {
        self.signer != "false"
    }

    pub fn writable_required(&self) -> bool {
        self.writable != "false"
    }
}

impl TypeSurface {
    fn from_idl(ty: &IdlTypeDef) -> Self {
        Self {
            name: ty.name.clone(),
            kind: format!("{:?}", ty.kind),
            fields: ty.fields.iter().map(FieldSurface::from_field).collect(),
            variants: ty.variants.iter().map(VariantSurface::from_idl).collect(),
            layout: ty.layout.as_ref().map(json_key),
        }
    }
}

impl VariantSurface {
    fn from_idl(variant: &IdlEnumVariant) -> Self {
        Self {
            name: variant.name.clone(),
            value: variant.value,
            fields: variant
                .fields
                .iter()
                .map(FieldSurface::from_field)
                .collect(),
            layout: variant.layout.as_ref().map(json_key),
        }
    }
}

impl EventSurface {
    fn from_idl(event: &IdlEventDef, type_map: &BTreeMap<&str, &IdlTypeDef>) -> Self {
        let fields = event
            .ty
            .as_ref()
            .and_then(event_type_name)
            .or(Some(event.name.as_str()))
            .and_then(|name| type_map.get(name).copied())
            .map(|ty| ty.fields.iter().map(FieldSurface::from_field).collect())
            .unwrap_or_default();

        Self {
            name: event.name.clone(),
            discriminator: event.discriminator.clone(),
            fields,
        }
    }
}

impl FieldSurface {
    fn from_arg(arg: &IdlArg) -> Self {
        Self {
            name: arg.name.clone(),
            ty: render_type(&arg.ty),
            codec: arg.codec.as_ref().map(json_key),
        }
    }

    fn from_field(field: &IdlFieldDef) -> Self {
        Self {
            name: field.name.clone(),
            ty: render_type(&field.ty),
            codec: field.codec.as_ref().map(json_key),
        }
    }
}

fn event_type_name(ty: &IdlType) -> Option<&str> {
    match ty {
        IdlType::Defined { defined } => Some(defined.name.as_str()),
        _ => None,
    }
}

fn render_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(name) => name.clone(),
        IdlType::Option { option } => format!("Option<{}>", render_type(option)),
        IdlType::Vec { vec } => format!("Vec<{}>", render_type(vec)),
        IdlType::Array { array } => format!("[{}; {}]", render_type(&array.0), array.1),
        IdlType::Defined { defined } => {
            if defined.generics.is_empty() {
                defined.name.clone()
            } else {
                let generics = defined
                    .generics
                    .iter()
                    .map(render_generic_arg)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{generics}>", defined.name)
            }
        }
        IdlType::Generic { generic } => generic.clone(),
    }
}

fn render_generic_arg(arg: &IdlGenericArg) -> String {
    match arg {
        IdlGenericArg::Type { r#type } => render_type(r#type),
        IdlGenericArg::Const { value } => value.clone(),
    }
}

fn flag_key(flag: &AccountFlag) -> String {
    match flag {
        AccountFlag::Fixed(v) => v.to_string(),
        AccountFlag::Dynamic(AccountFlagDynamic::Input) => "dynamic:input".to_owned(),
        AccountFlag::Dynamic(AccountFlagDynamic::Runtime) => "dynamic:runtime".to_owned(),
    }
}

fn pda_seeds(resolver: &IdlResolver) -> Vec<String> {
    match resolver {
        IdlResolver::Pda { seeds, .. } => seeds.iter().map(json_key).collect(),
        IdlResolver::Optional { resolver } => pda_seeds(resolver),
        _ => Vec::new(),
    }
}

fn resolver_refs(resolver: &IdlResolver) -> Vec<String> {
    let mut refs = Vec::new();
    collect_resolver_refs(resolver, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn collect_resolver_refs(resolver: &IdlResolver, refs: &mut Vec<String>) {
    match resolver {
        IdlResolver::Pda { program, seeds, .. } => {
            if let IdlPdaProgram::Account { path } = program {
                refs.push(path.clone());
            }
            for seed in seeds {
                match seed {
                    IdlPdaSeed::Account { path } => refs.push(path.clone()),
                    IdlPdaSeed::AccountField { account, .. } => refs.push(account.clone()),
                    IdlPdaSeed::Const { .. } | IdlPdaSeed::Arg { .. } => {}
                }
            }
        }
        IdlResolver::AssociatedToken {
            mint,
            owner,
            token_program,
        } => {
            refs.push(mint.clone());
            refs.push(owner.clone());
            if let Some(token_program) = token_program {
                refs.push(token_program.clone());
            }
        }
        IdlResolver::AccountField { account, .. } => refs.push(account.clone()),
        IdlResolver::Optional { resolver } => collect_resolver_refs(resolver, refs),
        IdlResolver::Input {}
        | IdlResolver::Const { .. }
        | IdlResolver::KnownProgram { .. }
        | IdlResolver::Arg { .. }
        | IdlResolver::Remaining { .. } => {}
    }
}

fn json_key<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("IDL surface values should serialize")
}

pub fn lock_path(crate_root: &Path) -> PathBuf {
    crate_root.join(LOCK_FILE_NAME)
}

#[derive(Debug)]
pub enum LockfileError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    VersionMismatch {
        path: PathBuf,
        expected: u32,
        found: u32,
    },
}

impl fmt::Display for LockfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "failed to read or write {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            }
            Self::VersionMismatch {
                path,
                expected,
                found,
            } => write!(
                f,
                "{} has lint surface version {found}, expected {expected}; regenerate it with \
                 `quasar lint --update-lock`",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LockfileError {}

pub fn save_lockfile(path: &Path, surface: &ProgramSurface) -> Result<(), LockfileError> {
    let json = serde_json::to_string_pretty(surface).map_err(|source| LockfileError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    std::fs::write(path, format!("{json}\n")).map_err(|source| LockfileError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn load_lockfile(path: &Path) -> Result<ProgramSurface, LockfileError> {
    let bytes = std::fs::read(path).map_err(|source| LockfileError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let surface: ProgramSurface =
        serde_json::from_slice(&bytes).map_err(|source| LockfileError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    if surface.version != SURFACE_VERSION {
        return Err(LockfileError::VersionMismatch {
            path: path.to_path_buf(),
            expected: SURFACE_VERSION,
            found: surface.version,
        });
    }
    Ok(surface)
}
