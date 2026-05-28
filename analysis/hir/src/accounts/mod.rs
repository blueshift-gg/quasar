//! Resolver and planner for `#[derive(Accounts)]` structs.

pub mod lower;
pub mod model;
pub mod planner;
pub mod rules;
pub mod specs;

pub use lower::lower_semantics;
pub use model::{FieldCore, FieldKind, FieldSemantics, ValueKind};
pub use planner::build_plan;
pub use specs::{
    AccountsPlanTyped, AddressSpec, BehaviorCall, BehaviorInitSpec, BehaviorPhase, EpilogueStep,
    FieldPlan, FieldRef, InitPlan, LoweredArg, LoweredValue, PostLoadStep, PreLoadStep,
    ProgramCloseSpec, ProgramInitSpec, ReallocSpec, RentPlan,
};
