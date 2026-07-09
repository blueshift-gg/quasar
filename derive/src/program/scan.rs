//! One pass over the module body collecting raw per-instruction data.
//!
//! Each `#[instruction]`/`#[instruction(...)]` attribute is parsed exactly once
//! here (killing the historical double-parse); discriminator assignment and all
//! validation happen in `model.rs` over the returned list, so the token-level
//! attribute is never re-parsed.

use {
    crate::helpers::InstructionArgs,
    syn::{Item, Meta},
};

/// One scanned instruction function: the handler item, its parsed attribute
/// arguments, and the attribute token itself (kept for exact error spans).
pub(super) struct RawInstruction<'a> {
    pub func: &'a syn::ItemFn,
    pub attr: &'a syn::Attribute,
    pub args: InstructionArgs,
}

/// Parse the `#[instruction]`/`#[instruction(...)]` attribute.
fn parse_instruction_attr(attr: &syn::Attribute) -> syn::Result<InstructionArgs> {
    match &attr.meta {
        Meta::Path(_) => Ok(InstructionArgs {
            discriminator: None,
            heap: false,
            raw: false,
        }),
        Meta::List(_) => attr.parse_args(),
        Meta::NameValue(_) => Err(syn::Error::new_spanned(
            attr,
            "expected `#[instruction]` or `#[instruction(...)]`",
        )),
    }
}

/// Scan the module items in source order, returning one `RawInstruction` per
/// `#[instruction]`-annotated function. Attribute parse errors surface here (the
/// same error the first historical pass raised), fail-fast.
pub(super) fn scan(items: &[Item]) -> syn::Result<Vec<RawInstruction<'_>>> {
    let mut raw = Vec::new();
    for item in items {
        if let Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("instruction") {
                    let args = parse_instruction_attr(attr)?;
                    raw.push(RawInstruction { func, attr, args });
                    break;
                }
            }
        }
    }
    Ok(raw)
}
