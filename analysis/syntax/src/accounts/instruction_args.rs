//! Parser for `#[instruction(args(...))]` on `#[derive(Accounts)]` structs.

use crate::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity};
use proc_macro2::TokenTree;
use syn::{
    parse::{discouraged::Speculative, ParseStream},
    DeriveInput, Ident, Token, Type,
};

pub struct InstructionArg {
    pub name: Ident,
    pub ty: Type,
}

pub fn parse_struct_instruction_args(
    input: &DeriveInput,
) -> syn::Result<Option<Vec<InstructionArg>>> {
    let mut sink = Diagnostics::new();
    let result = parse_struct_instruction_args_recoverable(input, &mut sink);
    if let Some(first) = sink.into_items().into_iter().next() {
        return Err(syn::Error::new(first.primary, first.message));
    }
    Ok(result)
}

pub fn parse_struct_instruction_args_recoverable(
    input: &DeriveInput,
    sink: &mut Diagnostics,
) -> Option<Vec<InstructionArg>> {
    let attr = input.attrs.iter().find(|a| a.path().is_ident("instruction"))?;

    let mut out = Vec::new();
    let _ = attr.parse_args_with(|stream: ParseStream| -> syn::Result<()> {
        parse_args_recoverable(stream, &mut out, sink);
        Ok(())
    });
    Some(out)
}

fn parse_args_recoverable(
    input: ParseStream,
    out: &mut Vec<InstructionArg>,
    sink: &mut Diagnostics,
) {
    while !input.is_empty() {
        let fork = input.fork();
        match parse_one_arg(&fork) {
            Ok(arg) => {
                input.advance_to(&fork);
                if let Some(prev) = out.iter().find(|a| a.name == arg.name) {
                    sink.emit(Diagnostic {
                        severity: Severity::Error,
                        code: DiagCode::InstructionArgDuplicate,
                        message: format!("duplicate instruction arg `{}`", arg.name),
                        primary: arg.name.span(),
                        labels: vec![syn::spanned::Spanned::span(&prev.name).into()]
                            .into_iter()
                            .map(|span| crate::diagnostics::DiagLabel {
                                span,
                                message: "first declared here".into(),
                            })
                            .collect(),
                        fixes: vec![],
                    });
                } else {
                    out.push(arg);
                }
            }
            Err(err) => {
                let span = err.span();
                sink.emit(Diagnostic {
                    severity: Severity::Error,
                    code: DiagCode::InstructionArgMalformed,
                    message: err.to_string(),
                    primary: span,
                    labels: vec![],
                    fixes: vec![],
                });
                sink.mark_parse_failed(span);
                skip_to_next_arg(input);
            }
        }

        if input.is_empty() {
            break;
        }
        if input.peek(Token![,]) {
            let _: Token![,] = input
                .parse()
                .expect("peek(Token![,]) succeeded, parse must too");
        } else {
            let stray = input.span();
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::InstructionArgMalformed,
                message: "expected `,` between instruction args".into(),
                primary: stray,
                labels: vec![],
                fixes: vec![],
            });
            sink.mark_parse_failed(stray);
            skip_to_next_arg(input);
        }
    }
}

fn parse_one_arg(input: ParseStream) -> syn::Result<InstructionArg> {
    let name: Ident = input.parse()?;
    let _: Token![:] = input.parse()?;
    let ty: Type = input.parse()?;
    Ok(InstructionArg { name, ty })
}

fn skip_to_next_arg(input: ParseStream) {
    while !input.is_empty() && !input.peek(Token![,]) {
        let _ = input.parse::<TokenTree>();
    }
}
