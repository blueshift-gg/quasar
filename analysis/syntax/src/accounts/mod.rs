//! Parser and AST for `#[derive(Accounts)]` field attributes and the
//! `#[instruction(name: Type, ...)]` form.

pub mod ast;
pub mod instruction_args;
pub mod parse;

pub use ast::{
    BehaviorArg, BehaviorGroup, CoreDirective, Directive, InitDirective, UserCheck,
};
pub use instruction_args::{
    parse_struct_instruction_args, parse_struct_instruction_args_recoverable, InstructionArg,
};
pub use parse::{
    parse_field_attrs, parse_field_attrs_recoverable, validate_behavior_arg,
    validate_behavior_arg_recoverable,
};
