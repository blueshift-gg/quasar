//! The validated program model: discriminator assignment + all scan-time rules.
//!
//! `ProgramModel::build` consumes the once-parsed [`RawInstruction`] list and
//! resolves it into the instruction/raw specs the codegen consumes. It
//! preserves the historical two-phase discriminator logic exactly — explicit
//! values are collected first (so autos skip every pinned value, including ones
//! declared later), then values are assigned in source order:
//!
//! - length/duplicate/reserved-`0xFF` rules stay byte-for-byte;
//! - an instruction whose signature fails is *recorded and skipped* only AFTER
//!   its discriminator (including any auto value) is committed, so no sibling's
//!   auto number shifts between the erroring compile and the fixed one (guarded
//!   by `lang/tests/auto_instruction_discriminator.rs`);
//! - independent signature and reserved-`0xFF` errors accumulate via
//!   `syn::Error::combine` and surface together; disc-level and attr-parse
//!   errors stay fail-fast (they recur from the scan or cascade, so
//!   accumulating them adds only noise).

use {
    super::{
        scan::RawInstruction,
        spec::{DiscriminatorSource, InstructionSpec, RawInstructionSpec},
    },
    crate::{ctx::CtxKind, helpers::parse_discriminator_bytes},
    proc_macro2::Span,
    std::collections::BTreeSet,
    syn::{Ident, LitInt},
};

/// The resolved, validated set of program instructions.
pub(super) struct ProgramModel {
    pub instruction_specs: Vec<InstructionSpec>,
    pub raw_specs: Vec<RawInstructionSpec>,
    pub disc_len: usize,
    pub any_heap: bool,
}

/// Assign the next free automatic 1-byte discriminator, skipping every value
/// already taken by an explicit or earlier automatic assignment.
fn auto_discriminator(
    used: &mut BTreeSet<u8>,
    next_auto: &mut u16,
    span: Span,
) -> syn::Result<(Vec<LitInt>, Vec<u8>)> {
    while *next_auto <= 254 {
        let value = *next_auto as u8;
        *next_auto += 1;
        if used.insert(value) {
            return Ok((vec![LitInt::new(&value.to_string(), span)], vec![value]));
        }
    }

    Err(syn::Error::new(
        span,
        "automatic instruction discriminators exhausted the 1-byte space; pin explicit \
         discriminators or split the program",
    ))
}

/// Fold an error into the running accumulator via `syn::Error::combine`, so
/// independent instruction errors surface together in one compile cycle.
fn combine_err(acc: &mut Option<syn::Error>, e: syn::Error) {
    match acc {
        Some(prev) => prev.combine(e),
        None => *acc = Some(e),
    }
}

impl ProgramModel {
    pub(super) fn build(raw: &[RawInstruction<'_>], module_ident: &Ident) -> syn::Result<Self> {
        let mut instruction_specs: Vec<InstructionSpec> = Vec::new();
        let mut raw_specs: Vec<RawInstructionSpec> = Vec::new();
        let mut seen_discriminators: Vec<(Vec<u8>, String)> = Vec::new();
        let mut disc_len: Option<usize> = None;
        let mut has_auto_discriminators = false;
        let mut explicit_discriminators: Vec<(Vec<u8>, String)> = Vec::new();

        // Pass 1: collect explicit discriminators first so automatic values skip
        // every pinned value, including values that appear later in the module.
        for ri in raw {
            let fn_name = ri.func.sig.ident.to_string();
            let Some(disc_bytes) = &ri.args.discriminator else {
                has_auto_discriminators = true;
                continue;
            };

            match disc_len {
                Some(len) => {
                    if disc_bytes.len() != len {
                        return Err(syn::Error::new_spanned(
                            ri.attr,
                            format!(
                                "all instruction discriminators must have the same length: \
                                 expected {} byte(s), found {}",
                                len,
                                disc_bytes.len()
                            ),
                        ));
                    }
                }
                None => disc_len = Some(disc_bytes.len()),
            }

            let disc_values = parse_discriminator_bytes(disc_bytes)?;
            if let Some((_, prev_fn)) = explicit_discriminators
                .iter()
                .find(|(v, _)| *v == disc_values)
            {
                return Err(syn::Error::new_spanned(
                    ri.attr,
                    format!(
                        "duplicate discriminator {:?}: already used by `{}`",
                        disc_values, prev_fn
                    ),
                ));
            }
            explicit_discriminators.push((disc_values, fn_name));
        }

        if has_auto_discriminators {
            if let Some(len) = disc_len {
                if len != 1 {
                    return Err(syn::Error::new_spanned(
                        module_ident,
                        "automatic instruction discriminators require 1-byte program \
                         discriminators; pin every instruction when using multi-byte \
                         discriminators",
                    ));
                }
            } else {
                disc_len = Some(1);
            }
        }

        let mut used_auto_discriminators = explicit_discriminators
            .iter()
            .filter_map(|(values, _)| values.first().copied())
            .collect::<BTreeSet<_>>();
        let mut next_auto = 0u16;

        // Pass 2: assign discriminators in source order and build the specs.
        let mut errors: Option<syn::Error> = None;

        for ri in raw {
            let func = ri.func;
            let args = &ri.args;
            let fn_name = &func.sig.ident;
            let (disc_bytes, disc_values, discriminator_source) = match &args.discriminator {
                Some(disc_bytes) => {
                    let disc_values = parse_discriminator_bytes(disc_bytes)?;
                    (
                        disc_bytes.clone(),
                        disc_values,
                        DiscriminatorSource::Explicit,
                    )
                }
                None => match auto_discriminator(
                    &mut used_auto_discriminators,
                    &mut next_auto,
                    fn_name.span(),
                ) {
                    Ok((disc_bytes, disc_values)) => {
                        (disc_bytes, disc_values, DiscriminatorSource::Auto)
                    }
                    Err(e) => return Err(e),
                },
            };
            if let Some((_, prev_fn)) = seen_discriminators.iter().find(|(v, _)| *v == disc_values)
            {
                return Err(syn::Error::new_spanned(
                    ri.attr,
                    format!(
                        "duplicate discriminator {:?}: already used by `{}`",
                        disc_values, prev_fn
                    ),
                ));
            }
            seen_discriminators.push((disc_values.clone(), fn_name.to_string()));

            if args.raw {
                // Raw instruction: no CtxKind, no accounts_type, no client args.
                raw_specs.push(RawInstructionSpec {
                    fn_name: fn_name.clone(),
                    disc_bytes: disc_bytes.clone(),
                    disc_values: disc_values.clone(),
                    discriminator_source,
                    heap: args.heap,
                    docs: crate::helpers::extract_doc_lines(&func.attrs),
                });
                continue;
            }

            let ctx_kind = match CtxKind::classify(&func.sig) {
                Ok(k) => k,
                // Signature error: this instruction's disc is already recorded
                // (continuity preserved); record and move on so sibling
                // instructions' signature errors also surface.
                Err(e) => {
                    combine_err(&mut errors, e);
                    continue;
                }
            };

            let spec = InstructionSpec::from_handler(
                func,
                disc_bytes,
                disc_values,
                discriminator_source,
                args.heap,
                ctx_kind,
            )?;
            instruction_specs.push(spec);
        }

        let disc_len = disc_len.unwrap_or(1);

        if let Some((_, fn_name)) = seen_discriminators
            .iter()
            .find(|(v, _)| v.first() == Some(&0xFF))
        {
            combine_err(
                &mut errors,
                syn::Error::new_spanned(
                    module_ident,
                    format!(
                        "instruction `{}` has a discriminator starting with 0xFF which is \
                         reserved for events",
                        fn_name
                    ),
                ),
            );
        }

        // Never emit dispatch for a partial instruction set: surface every
        // recorded error and stop before codegen.
        if let Some(e) = errors {
            return Err(e);
        }

        let any_heap = instruction_specs.iter().any(|spec| spec.heap)
            || raw_specs.iter().any(|spec| spec.heap);

        Ok(ProgramModel {
            instruction_specs,
            raw_specs,
            disc_len,
            any_heap,
        })
    }
}
