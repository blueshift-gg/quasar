//! Compact pretty-printer for the resolved semantics IR and the typed plan IR
//! (Workstream B3).
//!
//! These dumps are the audit surface for the accounts pipeline: a reviewer
//! reading `dump_semantics` + `dump_plan` for a fixture sees every structural
//! fact lowering derived and every phase-ordered step the emitter will run,
//! without reading generated tokens. `syn` payloads (`Type`/`Expr`/`Path`)
//! are rendered with `quote!(#x).to_string()` so the output is stable and
//! readable rather than the verbose `syn` `Debug`.
//!
//! Test-only: the printer exists to snapshot the IR, not to feed codegen.

use {
    super::{
        model::{
            AddressConstraint, AddressKind, BehaviorArg, BehaviorGroup, FieldCore, FieldKind,
            FieldSemantics, InitDirective, SeedRef, UserCheck,
        },
        specs::{
            AccountsPlanTyped, AddressSpec, BehaviorCall, EpilogueStep, EventCpiTerm, FieldPlan,
            FixedAddressSource, IdlResolverPlan, IdlSeedPlan, InitPlan, LoadStep, LoweredArg,
            LoweredValue, PostLoadStep, PreLoadStep, ReallocSpec, RentPlan,
        },
    },
    quote::ToTokens,
    std::fmt::Write,
};

/// Render any `syn` node as its token string (stable, whitespace-normalized).
fn toks(node: &impl ToTokens) -> String {
    node.to_token_stream().to_string()
}

fn opt_expr(e: &Option<syn::Expr>) -> String {
    match e {
        Some(e) => format!("`{}`", toks(e)),
        None => "None".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Semantics IR
// ---------------------------------------------------------------------------

/// Pretty-print the lowered semantics for a whole accounts struct.
pub(crate) fn dump_semantics(sems: &[FieldSemantics]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "SEMANTICS ({} fields)", sems.len());
    for (i, sem) in sems.iter().enumerate() {
        let _ = writeln!(out, "[{i}] {}", sem.core.ident);
        dump_field_core(&mut out, &sem.core);
        dump_field_semantics(&mut out, sem);
    }
    out
}

fn kind_str(k: FieldKind) -> &'static str {
    match k {
        FieldKind::Single => "Single",
        FieldKind::Composite => "Composite",
    }
}

fn dump_field_core(out: &mut String, core: &FieldCore) {
    let _ = writeln!(
        out,
        "    core: effective_ty=`{}` kind={} optional={} dynamic={} declared_mut={} dup={}",
        toks(&core.effective_ty),
        kind_str(core.kind),
        core.optional,
        core.dynamic,
        core.declared_mut,
        core.dup,
    );
    let inner = core
        .inner_ty
        .as_ref()
        .map(|t| format!("`{}`", toks(t)))
        .unwrap_or_else(|| "None".to_string());
    let _ = writeln!(out, "          inner_ty={inner}");
}

fn dump_field_semantics(out: &mut String, sem: &FieldSemantics) {
    match &sem.init {
        Some(InitDirective { idempotent }) => {
            let _ = writeln!(out, "    init: Some(idempotent={idempotent})");
        }
        None => {
            let _ = writeln!(out, "    init: None");
        }
    }
    let _ = writeln!(
        out,
        "    payer: {}",
        sem.payer
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "None".to_string()),
    );
    dump_address_constraint(out, &sem.address);
    let _ = writeln!(out, "    realloc: {}", opt_expr(&sem.realloc));
    let _ = writeln!(
        out,
        "    close_dest: {}",
        sem.close_dest
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "None".to_string()),
    );
    dump_groups(out, &sem.groups);
    dump_user_checks(out, &sem.user_checks);
    let _ = writeln!(
        out,
        "    is_migration={} is_uninit={} writable={}",
        sem.is_migration,
        sem.is_uninit,
        sem.is_writable(),
    );
}

fn dump_address_constraint(out: &mut String, addr: &Option<AddressConstraint>) {
    match addr {
        Some(AddressConstraint { expr, error, kind }) => {
            let _ = writeln!(
                out,
                "    address: Some(expr=`{}` error={} kind={})",
                toks(expr),
                opt_expr(error),
                address_kind(kind),
            );
        }
        None => {
            let _ = writeln!(out, "    address: None");
        }
    }
}

fn address_kind(kind: &AddressKind) -> String {
    match kind {
        AddressKind::Opaque => "Opaque".to_string(),
        AddressKind::Seeds { account_ty, seeds } => {
            let seeds: Vec<String> = seeds.iter().map(seed_ref).collect();
            format!(
                "Seeds(account_ty=`{}` seeds=[{}])",
                toks(account_ty),
                seeds.join(", "),
            )
        }
    }
}

fn seed_ref(seed: &SeedRef) -> String {
    match seed {
        SeedRef::AccountAddr(i) => format!("AccountAddr({i})"),
        SeedRef::AccountField { base, path } => format!("AccountField(base={base} path={path})"),
        SeedRef::IxArg(i) => format!("IxArg({i})"),
        SeedRef::Const(e) => format!("Const(`{}`)", toks(e)),
    }
}

fn dump_groups(out: &mut String, groups: &[BehaviorGroup]) {
    if groups.is_empty() {
        let _ = writeln!(out, "    groups: []");
        return;
    }
    let _ = writeln!(out, "    groups:");
    for g in groups {
        let args: Vec<String> = g
            .args
            .iter()
            .map(|BehaviorArg { key, value }| format!("{key}=`{}`", toks(value)))
            .collect();
        let _ = writeln!(
            out,
            "      - path=`{}` args=[{}]",
            toks(&g.path),
            args.join(", "),
        );
    }
}

fn dump_user_checks(out: &mut String, checks: &[UserCheck]) {
    if checks.is_empty() {
        let _ = writeln!(out, "    user_checks: []");
        return;
    }
    let _ = writeln!(out, "    user_checks:");
    for c in checks {
        let _ = writeln!(out, "      - {}", user_check_str(c));
    }
}

fn user_check_str(c: &UserCheck) -> String {
    match c {
        UserCheck::HasOne { targets, error } => {
            let targets: Vec<String> = targets.iter().map(|i| i.to_string()).collect();
            format!(
                "HasOne targets=[{}] error={}",
                targets.join(", "),
                opt_expr(error)
            )
        }
        UserCheck::Constraints { exprs, error } => {
            let exprs: Vec<String> = exprs.iter().map(|e| format!("`{}`", toks(e))).collect();
            format!(
                "Constraints exprs=[{}] error={}",
                exprs.join(", "),
                opt_expr(error)
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Plan IR
// ---------------------------------------------------------------------------

/// Pretty-print the typed execution plan. Field plans are positional; index
/// `i` corresponds to semantics field `i`.
pub(crate) fn dump_plan(plan: &AccountsPlanTyped) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "PLAN ({} fields)", plan.fields.len());
    let _ = writeln!(out, "rent: {}", rent_str(&plan.rent));
    let _ = writeln!(out, "has_instruction_args: {}", plan.has_instruction_args);
    let event_cpi: Vec<String> = plan.event_cpi.iter().map(event_cpi_term).collect();
    let _ = writeln!(out, "event_cpi: [{}]", event_cpi.join(", "));
    for (i, field) in plan.fields.iter().enumerate() {
        let _ = writeln!(out, "[{i}]");
        dump_field_plan(&mut out, field);
    }
    out
}

fn event_cpi_term(term: &EventCpiTerm) -> String {
    match term {
        EventCpiTerm::Never => "Never".to_string(),
        EventCpiTerm::EventAuthority => "EventAuthority".to_string(),
        EventCpiTerm::Composite(ty) => format!("Composite(`{}`)", toks(ty.as_ref())),
    }
}

fn rent_str(rent: &RentPlan) -> String {
    match rent {
        RentPlan::NotNeeded => "NotNeeded".to_string(),
        RentPlan::FromSysvarField { field } => format!("FromSysvarField(field={field})"),
        RentPlan::FetchOnce => "FetchOnce".to_string(),
    }
}

fn dump_field_plan(out: &mut String, field: &FieldPlan) {
    let _ = writeln!(
        out,
        "    id: ident={} effective_ty=`{}` wrapper={:?} kind={} optional={} dup={} writable={} \
         signer={}",
        field.ident,
        toks(&field.effective_ty),
        field.wrapper,
        kind_str(field.kind),
        field.optional,
        field.dup,
        field.writable,
        field.signer,
    );
    let _ = writeln!(out, "    load: {}", load_step(&field.load));
    let _ = writeln!(
        out,
        "    bump: {}",
        if field.bump.is_some() { "Some" } else { "None" },
    );
    let behaviors: Vec<String> = field
        .behaviors
        .iter()
        .map(|b| format!("`{}`(name={})", toks(&b.path), b.name))
        .collect();
    let _ = writeln!(out, "    behaviors: [{}]", behaviors.join(", "));
    let _ = writeln!(out, "    docs: {:?}", field.docs);
    let _ = writeln!(
        out,
        "    idl_resolver: {}",
        idl_resolver(&field.idl_resolver)
    );
    let _ = writeln!(
        out,
        "    signer_helper: {}",
        match &field.signer_helper {
            Some(h) => format!("Some(addr_expr=`{}`)", toks(&h.addr_expr)),
            None => "None".to_string(),
        },
    );
    dump_steps(out, "pre_load", &field.pre_load, pre_load_step);
    dump_steps(out, "post_load", &field.post_load, post_load_step);
    dump_steps(out, "epilogue", &field.epilogue, epilogue_step);
}

fn dump_steps<S>(out: &mut String, label: &str, steps: &[S], render: fn(&S) -> String) {
    if steps.is_empty() {
        let _ = writeln!(out, "    {label}: []");
        return;
    }
    let _ = writeln!(out, "    {label}:");
    for s in steps {
        let _ = writeln!(out, "      - {}", render(s));
    }
}

fn idl_resolver(resolver: &Option<IdlResolverPlan>) -> String {
    match resolver {
        None => "None".to_string(),
        Some(IdlResolverPlan::FixedAddress { inner_ty, source }) => format!(
            "FixedAddress(inner_ty=`{}` source={})",
            toks(inner_ty),
            match source {
                FixedAddressSource::Program => "Id::ID",
                FixedAddressSource::Sysvar => "Sysvar::ID",
            }
        ),
        Some(IdlResolverPlan::Pda { account_ty, seeds }) => {
            let seeds: Vec<String> = seeds.iter().map(idl_seed).collect();
            format!(
                "Pda(account_ty=`{}` seeds=[{}])",
                toks(account_ty),
                seeds.join(", "),
            )
        }
    }
}

fn idl_seed(seed: &IdlSeedPlan) -> String {
    match seed {
        IdlSeedPlan::AccountAddr { base } => format!("AccountAddr({base})"),
        IdlSeedPlan::AccountField {
            base,
            account,
            field,
        } => format!("AccountField(base={base} account={account} field={field})"),
        IdlSeedPlan::IxArg { name, ty } => format!("IxArg(name={name} ty=`{}`)", toks(ty)),
        IdlSeedPlan::Const { expr } => format!("Const(`{}`)", toks(expr)),
    }
}

fn load_step(load: &LoadStep) -> String {
    match load {
        LoadStep::Dynamic { base_ty } => format!("Dynamic(base_ty=`{}`)", toks(base_ty)),
        LoadStep::Fixed { validates_paths } => {
            let paths: Vec<String> = validates_paths.iter().map(toks).collect();
            format!("Fixed(validates=[{}])", paths.join(", "))
        }
    }
}

fn addr_spec(a: &AddressSpec) -> String {
    format!("expr=`{}` error={}", toks(&a.expr), opt_expr(&a.error))
}

fn opt_addr_spec(a: &Option<AddressSpec>) -> String {
    match a {
        Some(a) => format!("Some({})", addr_spec(a)),
        None => "None".to_string(),
    }
}

fn pre_load_step(step: &PreLoadStep) -> String {
    match step {
        PreLoadStep::VerifyAddress(a) => format!("VerifyAddress({})", addr_spec(a.as_ref())),
        PreLoadStep::Init(plan) => match plan.as_ref() {
            InitPlan::Program(p) => format!(
                "Init::Program(payer={} space_ty=`{}` idempotent={} verified_address={})",
                p.payer.ident,
                toks(&p.space_ty),
                p.idempotent,
                opt_addr_spec(&p.verified_address),
            ),
            InitPlan::Behavior(b) => {
                let calls: Vec<String> = b
                    .init_param_calls
                    .iter()
                    .map(|c| behavior_call(c, "SetInitParam"))
                    .collect();
                format!(
                    "Init::Behavior(payer={} idempotent={} init_param_calls=[{}] \
                     verified_address={})",
                    b.payer.ident,
                    b.idempotent,
                    calls.join(", "),
                    opt_addr_spec(&b.verified_address),
                )
            }
        },
    }
}

fn post_load_step(step: &PostLoadStep) -> String {
    match step {
        PostLoadStep::Behavior { phase, call } => {
            format!("Behavior({})", behavior_call(call, &format!("{phase:?}")))
        }
        PostLoadStep::UserCheck(check) => format!("UserCheck({})", user_check_str(check)),
        PostLoadStep::VerifyExistingAddress(a) => {
            format!("VerifyExistingAddress({})", addr_spec(a))
        }
        PostLoadStep::Realloc(ReallocSpec { new_space, payer }) => {
            format!(
                "Realloc(new_space=`{}` payer={})",
                toks(new_space),
                payer.ident
            )
        }
    }
}

fn epilogue_step(step: &EpilogueStep) -> String {
    match step {
        EpilogueStep::Behavior(c) => format!("Behavior({})", behavior_call(c, "Exit")),
        EpilogueStep::ProgramClose(c) => {
            format!("ProgramClose(destination_field={})", c.destination_field)
        }
    }
}

fn behavior_call(call: &BehaviorCall, phase: &str) -> String {
    let args: Vec<String> = call
        .args
        .iter()
        .map(|LoweredArg { key, lowered }| format!("{key}={}", lowered_value(lowered)))
        .collect();
    format!(
        "path=`{}` phase={} args=[{}]",
        toks(&call.path),
        phase,
        args.join(", "),
    )
}

fn lowered_value(v: &LoweredValue) -> String {
    match v {
        LoweredValue::FieldView(i) => format!("FieldView({i})"),
        LoweredValue::OptionalFieldView(i) => format!("OptionalFieldView({i})"),
        LoweredValue::Expr(e) => format!("Expr(`{}`)", toks(e)),
        LoweredValue::NoneLiteral => "NoneLiteral".to_string(),
        LoweredValue::SomeFieldView(i) => format!("SomeFieldView({i})"),
        LoweredValue::SomeExpr(e) => format!("SomeExpr(`{}`)", toks(e)),
    }
}
