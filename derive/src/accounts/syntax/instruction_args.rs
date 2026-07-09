use syn::{parse::ParseStream, DeriveInput, Ident, Token, Type};

pub(crate) struct InstructionArg {
    pub name: Ident,
    pub ty: Type,
}

pub(crate) fn parse_struct_instruction_args(
    input: &DeriveInput,
) -> syn::Result<Option<Vec<InstructionArg>>> {
    let attr = match input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("instruction"))
    {
        Some(attr) => attr,
        None => return Ok(None),
    };

    let args = attr.parse_args_with(|stream: ParseStream| {
        let mut args = Vec::new();
        while !stream.is_empty() {
            let name: Ident = stream.parse()?;
            let _: Token![:] = stream.parse()?;
            let ty: Type = stream.parse()?;
            if args.iter().any(|arg: &InstructionArg| arg.name == name) {
                return Err(syn::Error::new_spanned(
                    &name,
                    format!("duplicate instruction arg `{name}`"),
                ));
            }
            args.push(InstructionArg { name, ty });
            if !stream.is_empty() {
                let _: Token![,] = stream.parse()?;
            }
        }
        Ok(args)
    })?;

    Ok(Some(args))
}
