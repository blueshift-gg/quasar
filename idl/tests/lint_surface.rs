use quasar_idl::{
    lint::{self, ProgramSurface, RuleCode, Severity},
    types::*,
};

fn primitive(name: &str) -> IdlType {
    IdlType::Primitive(name.to_owned())
}

fn field(name: &str, ty: IdlType) -> IdlFieldDef {
    IdlFieldDef {
        name: name.to_owned(),
        ty,
        codec: None,
        docs: Vec::new(),
    }
}

fn account_type(name: &str, fields: Vec<IdlFieldDef>) -> IdlTypeDef {
    IdlTypeDef {
        name: name.to_owned(),
        kind: IdlTypeDefKind::Struct,
        docs: Vec::new(),
        generics: Vec::new(),
        fields,
        variants: Vec::new(),
        repr: None,
        alias: None,
        fallback: None,
        codec: None,
        layout: None,
        space: None,
        semantics: None,
    }
}

fn enum_type(name: &str, variants: Vec<(&str, u64)>) -> IdlTypeDef {
    IdlTypeDef {
        name: name.to_owned(),
        kind: IdlTypeDefKind::Enum,
        docs: Vec::new(),
        generics: Vec::new(),
        fields: Vec::new(),
        variants: variants
            .into_iter()
            .map(|(name, value)| IdlEnumVariant {
                name: name.to_owned(),
                value,
                fields: Vec::new(),
                layout: None,
            })
            .collect(),
        repr: Some("u8".to_owned()),
        alias: None,
        fallback: None,
        codec: None,
        layout: None,
        space: None,
        semantics: None,
    }
}

fn account_node(name: &str, signer: bool, writable: bool, resolver: IdlResolver) -> IdlAccountNode {
    IdlAccountNode {
        name: name.to_owned(),
        client_type: None,
        writable: AccountFlag::Fixed(writable),
        signer: AccountFlag::Fixed(signer),
        resolver,
        docs: Vec::new(),
    }
}

fn instruction(
    name: &str,
    discriminator: Vec<u8>,
    args: Vec<IdlArg>,
    accounts: Vec<IdlAccountNode>,
) -> IdlInstruction {
    IdlInstruction {
        name: name.to_owned(),
        discriminator,
        docs: Vec::new(),
        accounts,
        args,
        layout: None,
        returns: None,
        effects: Vec::new(),
        remaining_accounts: None,
    }
}

fn arg(name: &str, ty: IdlType) -> IdlArg {
    IdlArg {
        name: name.to_owned(),
        ty,
        codec: None,
        docs: Vec::new(),
    }
}

fn base_idl() -> Idl {
    Idl {
        spec: "quasar-idl/1.0.0".to_owned(),
        name: "audit_demo".to_owned(),
        version: "0.1.0".to_owned(),
        address: "11111111111111111111111111111111".to_owned(),
        metadata: IdlMetadata::default(),
        docs: Vec::new(),
        instructions: vec![instruction(
            "make",
            vec![0],
            vec![arg("amount", primitive("u64"))],
            vec![
                account_node("authority", true, false, IdlResolver::Input {}),
                account_node(
                    "vault",
                    false,
                    true,
                    IdlResolver::Pda {
                        program: IdlPdaProgram::ProgramId {},
                        seeds: vec![
                            IdlPdaSeed::Const {
                                value: b"vault".to_vec(),
                            },
                            IdlPdaSeed::Account {
                                path: "authority".to_owned(),
                            },
                        ],
                        bump: Some(IdlPdaBump::Canonical {}),
                    },
                ),
            ],
        )],
        accounts: vec![IdlAccountDef {
            name: "Vault".to_owned(),
            discriminator: vec![42],
            docs: Vec::new(),
            space: None,
        }],
        types: vec![
            account_type(
                "Vault",
                vec![
                    field("version", primitive("u8")),
                    field("amount", primitive("u64")),
                    field(
                        "_reserved",
                        IdlType::Array {
                            array: (Box::new(primitive("u8")), 64),
                        },
                    ),
                ],
            ),
            enum_type("Mode", vec![("Open", 0), ("Closed", 1)]),
        ],
        events: vec![IdlEventDef {
            name: "VaultMade".to_owned(),
            discriminator: vec![7],
            docs: Vec::new(),
            ty: None,
            transport: None,
        }],
        errors: Vec::new(),
        constants: Vec::new(),
        wrappers: None,
        extensions: None,
        hashes: None,
    }
}

#[test]
fn surface_preserves_ordered_account_instruction_and_pda_shape() {
    let surface = ProgramSurface::from_idl(&base_idl());

    assert_eq!(surface.accounts[0].name, "Vault");
    assert_eq!(
        surface.accounts[0]
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["version", "amount", "_reserved"]
    );
    assert_eq!(surface.instructions[0].accounts[1].name, "vault");
    assert_eq!(surface.instructions[0].accounts[1].pda_seeds.len(), 2);
}

#[test]
fn preflight_flags_upgrade_hostile_account_shapes() {
    let mut idl = base_idl();
    idl.accounts[0].name = "Mint".to_owned();
    idl.types[0].name = "Mint".to_owned();
    idl.types[0].fields = vec![field("amount", primitive("u64"))];
    idl.instructions[0].accounts = vec![account_node("vault", false, true, IdlResolver::Input {})];

    let report = lint::run(&idl, &lint::LintConfig::default());

    assert!(report.contains(RuleCode::P001));
    assert!(report.contains(RuleCode::P002));
    assert!(report.contains(RuleCode::P005));
    assert!(report.contains(RuleCode::P006));
    assert!(!report.has_errors());
}

#[test]
fn diff_rules_find_breaking_release_surface_changes() {
    let old = ProgramSurface::from_idl(&base_idl());
    let mut new_idl = base_idl();
    new_idl.types[0].fields[1] = field("amount", primitive("u32"));
    new_idl.instructions[0].args[0] = arg("amount", primitive("u32"));
    new_idl.instructions[0].accounts[1].resolver = IdlResolver::Pda {
        program: IdlPdaProgram::ProgramId {},
        seeds: vec![IdlPdaSeed::Const {
            value: b"vault-v2".to_vec(),
        }],
        bump: Some(IdlPdaBump::Canonical {}),
    };
    new_idl.events[0].discriminator = vec![8];
    new_idl.types[1] = enum_type("Mode", vec![("Closed", 1)]);
    let new = ProgramSurface::from_idl(&new_idl);

    let report = lint::diff(&old, &new);

    assert!(report.contains(RuleCode::R002));
    assert!(report.contains(RuleCode::R008));
    assert!(report.contains(RuleCode::R011));
    assert!(report.contains(RuleCode::R013));
    assert!(report.contains(RuleCode::R016));
    assert!(report.has_errors());
}

#[test]
fn additive_changes_do_not_fail_by_default() {
    let old = ProgramSurface::from_idl(&base_idl());
    let mut new_idl = base_idl();
    new_idl.types[0]
        .fields
        .push(field("new_tail_field", primitive("u64")));
    new_idl.types[1] = enum_type("Mode", vec![("Open", 0), ("Closed", 1), ("Paused", 2)]);
    let new = ProgramSurface::from_idl(&new_idl);

    let report = lint::diff(&old, &new);

    assert_eq!(
        report
            .diagnostics
            .iter()
            .find(|diag| diag.rule == RuleCode::R005)
            .map(|diag| diag.severity),
        Some(Severity::Warning)
    );
    assert_eq!(
        report
            .diagnostics
            .iter()
            .find(|diag| diag.rule == RuleCode::R012)
            .map(|diag| diag.severity),
        Some(Severity::Info)
    );
    assert!(!report.has_errors());
}

#[test]
fn preflight_warns_when_auto_instruction_discriminators_are_unlocked() {
    let mut idl = base_idl();
    idl.metadata.extra.insert(
        "quasar:instructionDiscriminatorSource".to_owned(),
        serde_json::json!({ "make": "auto" }),
    );

    let report = lint::run(
        &idl,
        &lint::LintConfig {
            strict: false,
            lockfile_present: false,
        },
    );
    assert!(report.contains(RuleCode::P008));

    let locked_report = lint::run(
        &idl,
        &lint::LintConfig {
            strict: false,
            lockfile_present: true,
        },
    );
    assert!(!locked_report.contains(RuleCode::P008));
}

#[test]
fn diff_rules_cover_account_layout_breaks() {
    let old = ProgramSurface::from_idl(&base_idl());

    let mut reordered = base_idl();
    reordered.types[0].fields.swap(0, 1);
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&reordered)).contains(RuleCode::R001));

    let mut removed = base_idl();
    removed.types[0].fields.pop();
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&removed)).contains(RuleCode::R003));

    let mut inserted = base_idl();
    inserted.types[0]
        .fields
        .insert(1, field("inserted", primitive("u8")));
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&inserted)).contains(RuleCode::R004));

    let mut discriminator = base_idl();
    discriminator.accounts[0].discriminator = vec![99];
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&discriminator)).contains(RuleCode::R006));

    let mut account_removed = base_idl();
    account_removed.accounts.clear();
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&account_removed)).contains(RuleCode::R015));
}

#[test]
fn diff_rules_cover_instruction_meta_breaks() {
    let old = ProgramSurface::from_idl(&base_idl());

    let mut removed = base_idl();
    removed.instructions.clear();
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&removed)).contains(RuleCode::R007));

    let mut accounts_changed = base_idl();
    accounts_changed.instructions[0].accounts.remove(0);
    assert!(
        lint::diff(&old, &ProgramSurface::from_idl(&accounts_changed)).contains(RuleCode::R009)
    );

    let mut flags_changed = base_idl();
    flags_changed.instructions[0].accounts[1].writable = AccountFlag::Fixed(false);
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&flags_changed)).contains(RuleCode::R010));

    let mut discriminator = base_idl();
    discriminator.instructions[0].discriminator = vec![44];
    assert!(lint::diff(&old, &ProgramSurface::from_idl(&discriminator)).contains(RuleCode::R014));
}
