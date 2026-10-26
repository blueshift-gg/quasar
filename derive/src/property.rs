//! `#[property]`: one Kani harness per property.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, FnArg, ItemFn, Path, Result, Type};

/// `#[property(handler)]` on `fn name(before: &A, after: &A, data: &D) -> bool`,
/// `data` left out for an instruction without any: one Kani harness that
/// runs the handler on symbolic accounts that passed `A`'s checks and on any
/// data that decodes, and asserts the predicate whenever the handler succeeds.
pub fn expand(handler: Path, property: ItemFn) -> Result<TokenStream> {
    let name = &property.sig.ident;
    let wrong = || {
        Error::new_spanned(
            &property.sig,
            "a property takes `before: &Accounts, after: &Accounts`, then `data: &Data` if the instruction has data",
        )
    };
    let types = property.sig.inputs.iter().map(referenced_type).collect::<Result<Vec<_>>>()?;
    let (accounts, data) = match types.as_slice() {
        [accounts, _] => (accounts, syn::parse_quote!(())),
        [accounts, _, data] => (accounts, data.clone()),
        _ => return Err(wrong()),
    };
    let data_arg = (types.len() == 3).then(|| quote!(, &decode()));
    let harness = format_ident!("verify_{name}");
    let message = format!("property `{name}` holds after `{}`", quote!(#handler));

    Ok(quote! {
        ::quasar_lang::kani_only! {
            #property

            #[kani::proof]
            fn #harness() {
                use ::quasar_lang::data::InstructionData;
                let mut world = ::quasar_lang::verify::World::<#accounts, #data>::any();
                let bytes: [u8; <#data as InstructionData>::SIZE] = kani::any();
                kani::assume(<#data as InstructionData>::decode(&bytes).is_ok());
                let decode = || <#data as InstructionData>::decode(&bytes).unwrap_or_else(|_| unreachable!());
                let before = world.before();
                let result = #handler(world.ctx::<#handler::Vouched>(decode()));
                kani::cover!(result.is_ok(), "the instruction succeeds on some input");
                if result.is_ok() {
                    let after = world.after();
                    assert!(#name(&before, &after #data_arg), #message);
                }
            }
        }
    })
}

/// The `T` in an argument of type `&T`.
fn referenced_type(input: &FnArg) -> Result<Type> {
    match input {
        FnArg::Typed(arg) => match &*arg.ty {
            Type::Reference(reference) => Ok((*reference.elem).clone()),
            ty => Err(Error::new_spanned(ty, "expected a reference, `&T`")),
        },
        FnArg::Receiver(receiver) => Err(Error::new_spanned(receiver, "unexpected `self`")),
    }
}
