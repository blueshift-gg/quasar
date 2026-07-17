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
//! The same stable renderers also feed the host-only validation-plan IDL
//! extension. Keeping snapshots and the public audit surface on one renderer
//! prevents them from drifting apart.

use {
    super::{
        describe::{
            epilogue as epilogue_step, load as load_step, post_load as post_load_step,
            pre_load as pre_load_step, rent as rent_str, tokens as toks,
            user_check as user_check_str,
        },
        model::{
            AddressConstraint, AddressKind, BehaviorArg, BehaviorGroup, FieldCore, FieldKind,
            FieldSemantics, InitDirective, SeedRef, UserCheck,
        },
        specs::{
            AccountsPlanTyped, EventCpiTerm, FieldPlan, FixedAddressSource, IdlResolverPlan,
            IdlSeedPlan,
        },
    },
    std::fmt::Write,
};

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

pub(crate) fn kind_str(k: FieldKind) -> &'static str {
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
    if field.behavior_init_signer {
        let _ = writeln!(out, "    behavior_init_signer: true");
    }
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
