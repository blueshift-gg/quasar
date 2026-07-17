//! Stable, compact descriptions of the resolved accounts plan.
//!
//! The strings emitted here are deliberately shared by compiler snapshots and
//! the host-only IDL validation extension. That keeps `quasar audit` aligned
//! with the exact typed plan used by code generation.

use {
    super::{
        model::UserCheck,
        specs::{
            AddressSpec, BehaviorCall, EpilogueStep, InitPlan, LoadStep, LoweredArg, LoweredValue,
            PostLoadStep, PreLoadStep, ReallocSpec, RentPlan,
        },
    },
    quote::ToTokens,
};

/// Render any `syn` node as a stable, whitespace-normalized token string.
pub(crate) fn tokens(node: &impl ToTokens) -> String {
    node.to_token_stream().to_string()
}

pub(crate) fn rent(plan: &RentPlan) -> String {
    match plan {
        RentPlan::NotNeeded => "NotNeeded".to_string(),
        RentPlan::FromSysvarField { field } => format!("FromSysvarField(field={field})"),
        RentPlan::FetchOnce => "FetchOnce".to_string(),
    }
}

pub(crate) fn load(step: &LoadStep) -> String {
    match step {
        LoadStep::Dynamic { base_ty } => format!("Dynamic(base_ty=`{}`)", tokens(base_ty)),
        LoadStep::Fixed { validates_paths } => {
            let paths: Vec<String> = validates_paths.iter().map(tokens).collect();
            format!("Fixed(validates=[{}])", paths.join(", "))
        }
    }
}

pub(crate) fn pre_load(step: &PreLoadStep) -> String {
    match step {
        PreLoadStep::VerifyAddress(address) => {
            format!("VerifyAddress({})", address_spec(address.as_ref()))
        }
        PreLoadStep::Init(plan) => match plan.as_ref() {
            InitPlan::Program(plan) => format!(
                "Init::Program(payer={} space_ty=`{}` idempotent={} verified_address={})",
                plan.payer.ident,
                tokens(&plan.space_ty),
                plan.idempotent,
                optional_address_spec(&plan.verified_address),
            ),
            InitPlan::Behavior(plan) => {
                let calls: Vec<String> = plan
                    .init_param_calls
                    .iter()
                    .map(|call| behavior_call(call, "SetInitParam"))
                    .collect();
                format!(
                    "Init::Behavior(payer={} idempotent={} init_param_calls=[{}] \
                     verified_address={})",
                    plan.payer.ident,
                    plan.idempotent,
                    calls.join(", "),
                    optional_address_spec(&plan.verified_address),
                )
            }
        },
    }
}

pub(crate) fn post_load(step: &PostLoadStep) -> String {
    match step {
        PostLoadStep::Behavior { phase, call } => {
            format!("Behavior({})", behavior_call(call, &format!("{phase:?}")))
        }
        PostLoadStep::UserCheck(check) => format!("UserCheck({})", user_check(check)),
        PostLoadStep::VerifyExistingAddress(address) => {
            format!("VerifyExistingAddress({})", address_spec(address))
        }
        PostLoadStep::Realloc(ReallocSpec { new_space, payer }) => format!(
            "Realloc(new_space=`{}` payer={})",
            tokens(new_space),
            payer.ident
        ),
    }
}

pub(crate) fn epilogue(step: &EpilogueStep) -> String {
    match step {
        EpilogueStep::Behavior(call) => format!("Behavior({})", behavior_call(call, "Exit")),
        EpilogueStep::ProgramClose(close) => {
            format!(
                "ProgramClose(destination_field={})",
                close.destination_field
            )
        }
    }
}

pub(crate) fn user_check(check: &UserCheck) -> String {
    match check {
        UserCheck::HasOne { targets, error } => {
            let targets: Vec<String> = targets.iter().map(ToString::to_string).collect();
            format!(
                "HasOne targets=[{}] error={}",
                targets.join(", "),
                optional_expr(error)
            )
        }
        UserCheck::Constraints { exprs, error } => {
            let exprs: Vec<String> = exprs
                .iter()
                .map(|expr| format!("`{}`", tokens(expr)))
                .collect();
            format!(
                "Constraints exprs=[{}] error={}",
                exprs.join(", "),
                optional_expr(error)
            )
        }
    }
}

fn address_spec(address: &AddressSpec) -> String {
    format!(
        "expr=`{}` error={}",
        tokens(&address.expr),
        optional_expr(&address.error)
    )
}

fn optional_address_spec(address: &Option<AddressSpec>) -> String {
    match address {
        Some(address) => format!("Some({})", address_spec(address)),
        None => "None".to_string(),
    }
}

fn optional_expr(expr: &Option<syn::Expr>) -> String {
    match expr {
        Some(expr) => format!("`{}`", tokens(expr)),
        None => "None".to_string(),
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
        tokens(&call.path),
        phase,
        args.join(", "),
    )
}

fn lowered_value(value: &LoweredValue) -> String {
    match value {
        LoweredValue::FieldView(index) => format!("FieldView({index})"),
        LoweredValue::OptionalFieldView(index) => format!("OptionalFieldView({index})"),
        LoweredValue::Expr(expr) => format!("Expr(`{}`)", tokens(expr)),
        LoweredValue::NoneLiteral => "NoneLiteral".to_string(),
        LoweredValue::SomeFieldView(index) => format!("SomeFieldView({index})"),
        LoweredValue::SomeExpr(expr) => format!("SomeExpr(`{}`)", tokens(expr)),
    }
}
