use {
    super::TsTarget,
    crate::types::{Idl, IdlArg, IdlCodec, IdlFieldDef, IdlType, ScalarRepr},
    std::{collections::HashSet, fmt::Write},
};

/// Returns `true` if any field in the slice is a dynamic field (has
/// SizePrefixed codec).
pub(super) fn has_dynamic_field_defs(fields: &[IdlFieldDef]) -> bool {
    fields.iter().any(is_field_def_dynamic)
}

/// Returns `true` if a field def has a SizePrefixed or Remainder codec
/// (dynamic).
pub(super) fn is_field_def_dynamic(field: &IdlFieldDef) -> bool {
    matches!(
        field.codec,
        Some(IdlCodec::SizePrefixed { .. }) | Some(IdlCodec::Remainder { .. })
    )
}

/// Returns `true` if an instruction arg is dynamic (has SizePrefixed or
/// Remainder codec).
pub(super) fn is_arg_dynamic(arg: &IdlArg) -> bool {
    matches!(
        arg.codec,
        Some(IdlCodec::SizePrefixed { .. }) | Some(IdlCodec::Remainder { .. })
    )
}

/// Emit a custom codec object for a type with dynamic fields (compact layout).
///
/// Layout: `[fixed fields][all length prefixes][all tail data]`
pub(super) fn emit_compact_type_codec(
    out: &mut String,
    name: &str,
    fields: &[IdlFieldDef],
    target: TsTarget,
) {
    let fixed_fields: Vec<_> = fields.iter().filter(|f| !is_field_def_dynamic(f)).collect();
    let dyn_fields: Vec<_> = fields.iter().filter(|f| is_field_def_dynamic(f)).collect();

    let buf_ctor = match target {
        TsTarget::Web3js => "Uint8Array.from",
        TsTarget::Kit => "Uint8Array.from",
    };

    writeln!(out, "export const {name}Codec = {{").expect("write to String");

    writeln!(out, "  encode(value: {name}): Uint8Array {{").expect("write to String");

    // Phase 1: fixed fields
    if fixed_fields.is_empty() {
        out.push_str("    const fixedBytes = new Uint8Array(0);\n");
    } else {
        out.push_str("    const fixedCodec = getStructCodec([\n");
        for f in &fixed_fields {
            writeln!(
                out,
                "      [\"{}\", {}],",
                f.name,
                ts_codec_for_field_def(f, target)
            )
            .expect("write to String");
        }
        out.push_str("    ]);\n");
        let fixed_names: Vec<String> = fixed_fields
            .iter()
            .map(|f| format!("{}: value.{}", f.name, f.name))
            .collect();
        writeln!(
            out,
            "    const fixedBytes = fixedCodec.encode({{ {} }});",
            fixed_names.join(", ")
        )
        .expect("write to String");
    }

    // Phase 2: length prefixes
    for f in &dyn_fields {
        let pfx = codec_prefix_bytes(&f.codec);
        let pfx_codec = prefix_codec(pfx);
        if is_optional_dynamic_string(&f.ty) || is_optional_dynamic_vec(&f.ty) {
            writeln!(
                out,
                "    const {name}Tag = getU8Codec().encode(value.{name} === null ? 0 : 1);",
                name = f.name
            )
            .expect("write to String");
        } else if is_string_type(&f.ty) {
            writeln!(
                out,
                "    const {name}Bytes = new TextEncoder().encode(value.{name});",
                name = f.name
            )
            .expect("write to String");
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode({name}Bytes.length);",
                name = f.name,
                codec = pfx_codec
            )
            .expect("write to String");
        } else {
            // Vec
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode(value.{name}.length);",
                name = f.name,
                codec = pfx_codec
            )
            .expect("write to String");
        }
    }

    // Phase 3: tail data
    for f in &dyn_fields {
        if let Some(inner) = optional_dynamic_inner(&f.ty) {
            let pfx = codec_prefix_bytes(&f.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(
                    out,
                    "    const {name}Payload = value.{name} === null ? new Uint8Array(0) : new \
                     TextEncoder().encode(value.{name});",
                    name = f.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = value.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode({name}Payload.length), ...{name}Payload]);",
                    name = f.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "    const {name}Payload = value.{name} === null ? new Uint8Array(0) : \
                     getArrayCodec({item_codec}, {{ size: value.{name}.length \
                     }}).encode(value.{name});",
                    name = f.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = value.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode(value.{name}.length), ...{name}Payload]);",
                    name = f.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            }
        } else if is_string_type(&f.ty) {
            // Already encoded as `{name}Bytes` in phase 2
        } else if let IdlType::Vec { vec } = &f.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "    const {name}Bytes = getArrayCodec({item_codec}, {{ size: value.{name}.length \
                 }}).encode(value.{name});",
                name = f.name,
                item_codec = item_codec,
            )
            .expect("write to String");
        }
    }

    // Concatenate all phases
    let mut concat_parts = vec!["fixedBytes".to_string()];
    for f in &dyn_fields {
        if optional_dynamic_inner(&f.ty).is_some() {
            concat_parts.push(format!("{}Tag", f.name));
        } else {
            concat_parts.push(format!("{}Prefix", f.name));
        }
    }
    for f in &dyn_fields {
        concat_parts.push(format!("{}Bytes", f.name));
    }

    writeln!(
        out,
        "    return {}([{}]);",
        buf_ctor,
        concat_parts
            .iter()
            .map(|p| format!("...{}", p))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("write to String");
    out.push_str("  },\n");

    writeln!(out, "  decode(data: Uint8Array): {name} {{").expect("write to String");
    out.push_str("    let offset = 0;\n");

    // Phase 1: decode fixed fields
    if !fixed_fields.is_empty() {
        out.push_str("    const fixedCodec = getStructCodec([\n");
        for f in &fixed_fields {
            writeln!(
                out,
                "      [\"{}\", {}],",
                f.name,
                ts_codec_for_field_def(f, target)
            )
            .expect("write to String");
        }
        out.push_str("    ]);\n");
        out.push_str("    const fixedResult = fixedCodec.decode(data.slice(offset));\n");
        out.push_str("    const fixedSize = codecSize(fixedCodec, fixedResult);\n");
        out.push_str(
            "    if (!bytesEqual(fixedCodec.encode(fixedResult), checkedTake(data, offset, \
             fixedSize))) throw new Error(\"invalid fixed field encoding\");\n",
        );
        out.push_str("    offset += fixedSize;\n");
    }

    // Phase 2: decode length prefixes
    for f in &dyn_fields {
        let pfx = codec_prefix_bytes(&f.codec);
        let pfx_codec = prefix_codec(pfx);
        if optional_dynamic_inner(&f.ty).is_some() {
            writeln!(
                out,
                "    const {name}Tag = getU8Codec().decode(checkedTake(data, offset, 1));",
                name = f.name
            )
            .expect("write to String");
            out.push_str("    offset += 1;\n");
            writeln!(
                out,
                "    if ({name}Tag !== 0 && {name}Tag !== 1) throw new Error(\"invalid option tag \
                 for {name}\");",
                name = f.name
            )
            .expect("write to String");
        } else {
            writeln!(
                out,
                "    const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                name = f.name,
                codec = pfx_codec,
                pfx = pfx,
            )
            .expect("write to String");
            writeln!(out, "    offset += {};", pfx).expect("write to String");
        }
    }

    // Phase 3: decode tail data
    for f in &dyn_fields {
        if let Some(inner) = optional_dynamic_inner(&f.ty) {
            let pfx = codec_prefix_bytes(&f.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(out, "    let {name}: string | null = null;", name = f.name)
                    .expect("write to String");
                writeln!(out, "    if ({name}Tag === 1) {{", name = f.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "      const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = f.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "      offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "      const {name}Size = checkedLength({name}Len);\n\x20     {name} = \
                     decodeUtf8(checkedTake(data, offset, {name}Size));",
                    name = f.name
                )
                .expect("write to String");
                writeln!(out, "      offset += {}Size;", f.name).expect("write to String");
                out.push_str("    }\n");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "    let {name}: Array<{item_ty}> | null = null;",
                    name = f.name,
                    item_ty = ts_type(vec),
                )
                .expect("write to String");
                writeln!(out, "    if ({name}Tag === 1) {{", name = f.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "      const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = f.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "      offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "      const {name}Count = checkedElementCount({name}Len, data.length - \
                     offset);\n\x20     const {name}Codec = getArrayCodec({item_codec}, {{ size: \
                     {name}Count }});",
                    name = f.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "      {name} = {name}Codec.decode(data.slice(offset));",
                    name = f.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "      offset += {name}Codec.encode({name}).length;",
                    name = f.name
                )
                .expect("write to String");
                out.push_str("    }\n");
            }
        } else if is_string_type(&f.ty) {
            writeln!(
                out,
                "    const {name}Size = checkedLength({name}Len);\n\x20   const {name} = \
                 decodeUtf8(checkedTake(data, offset, {name}Size));",
                name = f.name
            )
            .expect("write to String");
            writeln!(out, "    offset += {}Size;", f.name).expect("write to String");
        } else if let IdlType::Vec { vec } = &f.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "    const {name}Count = checkedElementCount({name}Len, data.length - \
                 offset);\n\x20   const {name}Codec = getArrayCodec({item_codec}, {{ size: \
                 {name}Count }});",
                name = f.name,
                item_codec = item_codec,
            )
            .expect("write to String");
            writeln!(
                out,
                "    const {name} = {name}Codec.decode(data.slice(offset));",
                name = f.name
            )
            .expect("write to String");
            writeln!(
                out,
                "    offset += {name}Codec.encode({name}).length;",
                name = f.name
            )
            .expect("write to String");
        }
    }

    // Build return object
    let mut field_exprs = Vec::new();
    for f in &fixed_fields {
        if let IdlType::Option { option } = &f.ty {
            field_exprs.push(format!(
                "{}: unwrapOption<{}>(fixedResult.{})",
                f.name,
                ts_type(option),
                f.name
            ));
        } else {
            field_exprs.push(format!("{}: fixedResult.{}", f.name, f.name));
        }
    }
    for f in &dyn_fields {
        field_exprs.push(f.name.clone());
    }
    writeln!(out, "    const result = {{ {} }};", field_exprs.join(", ")).expect("write to String");
    out.push_str("    assertFinished(data, offset);\n");
    out.push_str(
        "    if (!bytesEqual(this.encode(result), data)) throw new Error(\"invalid field \
         encoding\");\n",
    );
    out.push_str("    return result;\n");
    out.push_str("  },\n");

    out.push_str("};\n\n");
}

/// Emit compact (3-phase) encoding for an instruction with dynamic fields.
///
/// Layout: `[disc][fixed fields][all length prefixes][all dynamic data]`
pub(super) fn emit_compact_encoding(
    out: &mut String,
    ix: &crate::types::IdlInstruction,
    disc_str: &str,
    target: TsTarget,
    buf_ctor: &str,
) {
    let fixed_args: Vec<_> = ix.args.iter().filter(|a| !is_arg_dynamic(a)).collect();
    let dyn_args: Vec<_> = ix.args.iter().filter(|a| is_arg_dynamic(a)).collect();

    out.push_str("    const disc = new Uint8Array([");
    out.push_str(disc_str);
    out.push_str("]);\n");

    // Phase 1: fixed fields
    if fixed_args.is_empty() {
        out.push_str("    const fixedBytes = new Uint8Array(0);\n");
    } else {
        out.push_str("    const fixedCodec = getStructCodec([\n");
        for arg in &fixed_args {
            writeln!(
                out,
                "      [\"{}\", {}],",
                arg.name,
                ts_codec_for_arg(arg, target)
            )
            .expect("write to String");
        }
        out.push_str("    ]);\n");
        let fixed_names: Vec<String> = fixed_args
            .iter()
            .map(|a| format!("{}: input.{}", a.name, a.name))
            .collect();
        writeln!(
            out,
            "    const fixedBytes = fixedCodec.encode({{ {} }});",
            fixed_names.join(", ")
        )
        .expect("write to String");
    }

    // Phase 2: length prefixes
    for arg in &dyn_args {
        let pfx = codec_prefix_bytes(&arg.codec);
        let pfx_codec = prefix_codec(pfx);
        if optional_dynamic_inner(&arg.ty).is_some() {
            writeln!(
                out,
                "    const {name}Tag = getU8Codec().encode(input.{name} === null ? 0 : 1);",
                name = arg.name
            )
            .expect("write to String");
        } else if is_string_type(&arg.ty) {
            writeln!(
                out,
                "    const {name}Bytes = new TextEncoder().encode(input.{name});",
                name = arg.name
            )
            .expect("write to String");
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode({name}Bytes.length);",
                name = arg.name,
                codec = pfx_codec
            )
            .expect("write to String");
        } else {
            // Vec
            writeln!(
                out,
                "    const {name}Prefix = {codec}.encode(input.{name}.length);",
                name = arg.name,
                codec = pfx_codec
            )
            .expect("write to String");
        }
    }

    // Phase 3: tail data
    for arg in &dyn_args {
        if let Some(inner) = optional_dynamic_inner(&arg.ty) {
            let pfx = codec_prefix_bytes(&arg.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(
                    out,
                    "    const {name}Payload = input.{name} === null ? new Uint8Array(0) : new \
                     TextEncoder().encode(input.{name});",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = input.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode({name}Payload.length), ...{name}Payload]);",
                    name = arg.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "    const {name}Payload = input.{name} === null ? new Uint8Array(0) : \
                     getArrayCodec({item_codec}, {{ size: input.{name}.length \
                     }}).encode(input.{name});",
                    name = arg.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "    const {name}Bytes = input.{name} === null ? new Uint8Array(0) : \
                     {buf}([...{pfx}.encode(input.{name}.length), ...{name}Payload]);",
                    name = arg.name,
                    buf = buf_ctor,
                    pfx = pfx_codec,
                )
                .expect("write to String");
            }
        } else if is_string_type(&arg.ty) {
            // Already encoded as `{name}Bytes` in phase 2
        } else if let IdlType::Vec { vec } = &arg.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "    const {name}Bytes = getArrayCodec({item_codec}, {{ size: input.{name}.length \
                 }}).encode(input.{name});",
                name = arg.name,
                item_codec = item_codec,
            )
            .expect("write to String");
        }
    }

    // Concatenate all phases
    let mut concat_parts = vec!["disc".to_string(), "fixedBytes".to_string()];
    for arg in &dyn_args {
        if optional_dynamic_inner(&arg.ty).is_some() {
            concat_parts.push(format!("{}Tag", arg.name));
        } else {
            concat_parts.push(format!("{}Prefix", arg.name));
        }
    }
    for arg in &dyn_args {
        concat_parts.push(format!("{}Bytes", arg.name));
    }

    writeln!(
        out,
        "    const data = {}([{}]);",
        buf_ctor,
        concat_parts
            .iter()
            .map(|p| format!("...{}", p))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("write to String");
}

/// Emit compact (3-phase) decoding for an instruction with dynamic fields.
pub(super) fn emit_compact_decode(
    out: &mut String,
    ix: &crate::types::IdlInstruction,
    const_name: &str,
    pascal: &str,
    target: TsTarget,
) {
    let fixed_args: Vec<_> = ix.args.iter().filter(|a| !is_arg_dynamic(a)).collect();
    let dyn_args: Vec<_> = ix.args.iter().filter(|a| is_arg_dynamic(a)).collect();

    writeln!(out, "      let offset = {}.length;", const_name).expect("write to String");

    // Phase 1: decode fixed fields
    if !fixed_args.is_empty() {
        out.push_str("      const fixedCodec = getStructCodec([\n");
        for arg in &fixed_args {
            writeln!(
                out,
                "        [\"{}\", {}],",
                arg.name,
                ts_codec_for_arg(arg, target)
            )
            .expect("write to String");
        }
        out.push_str("      ]);\n");
        out.push_str("      const fixedResult = fixedCodec.decode(data.slice(offset));\n");
        out.push_str("      const fixedSize = codecSize(fixedCodec, fixedResult);\n");
        out.push_str(
            "      if (!bytesEqual(fixedCodec.encode(fixedResult), checkedTake(data, offset, \
             fixedSize))) throw new Error(\"invalid fixed field encoding\");\n",
        );
        out.push_str("      offset += fixedSize;\n");
    }

    // Phase 2: decode length prefixes
    for arg in &dyn_args {
        let pfx = codec_prefix_bytes(&arg.codec);
        let pfx_codec = prefix_codec(pfx);
        if optional_dynamic_inner(&arg.ty).is_some() {
            writeln!(
                out,
                "      const {name}Tag = getU8Codec().decode(checkedTake(data, offset, 1));",
                name = arg.name
            )
            .expect("write to String");
            out.push_str("      offset += 1;\n");
            writeln!(
                out,
                "      if ({name}Tag !== 0 && {name}Tag !== 1) throw new Error(\"invalid option \
                 tag for {name}\");",
                name = arg.name
            )
            .expect("write to String");
        } else {
            writeln!(
                out,
                "      const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                name = arg.name,
                codec = pfx_codec,
                pfx = pfx,
            )
            .expect("write to String");
            writeln!(out, "      offset += {};", pfx).expect("write to String");
        }
    }

    // Phase 3: decode tail data
    for arg in &dyn_args {
        if let Some(inner) = optional_dynamic_inner(&arg.ty) {
            let pfx = codec_prefix_bytes(&arg.codec);
            let pfx_codec = prefix_codec(pfx);
            if is_string_type(inner) {
                writeln!(
                    out,
                    "      let {name}: string | null = null;",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(out, "      if ({name}Tag === 1) {{", name = arg.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "        const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = arg.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "        offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "        const {name}Size = checkedLength({name}Len);\n\x20       {name} = \
                     decodeUtf8(checkedTake(data, offset, {name}Size));",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(out, "        offset += {}Size;", arg.name).expect("write to String");
                out.push_str("      }\n");
            } else if let IdlType::Vec { vec } = inner {
                let item_codec = ts_codec(vec, target);
                writeln!(
                    out,
                    "      let {name}: Array<{item_ty}> | null = null;",
                    name = arg.name,
                    item_ty = ts_type(vec),
                )
                .expect("write to String");
                writeln!(out, "      if ({name}Tag === 1) {{", name = arg.name)
                    .expect("write to String");
                writeln!(
                    out,
                    "        const {name}Len = {codec}.decode(checkedTake(data, offset, {pfx}));",
                    name = arg.name,
                    codec = pfx_codec,
                    pfx = pfx,
                )
                .expect("write to String");
                writeln!(out, "        offset += {};", pfx).expect("write to String");
                writeln!(
                    out,
                    "        const {name}Count = checkedElementCount({name}Len, data.length - \
                     offset);\n\x20       const {name}Codec = getArrayCodec({item_codec}, {{ \
                     size: {name}Count }});",
                    name = arg.name,
                    item_codec = item_codec,
                )
                .expect("write to String");
                writeln!(
                    out,
                    "        {name} = {name}Codec.decode(data.slice(offset));",
                    name = arg.name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "        offset += {name}Codec.encode({name}).length;",
                    name = arg.name
                )
                .expect("write to String");
                out.push_str("      }\n");
            }
        } else if is_string_type(&arg.ty) {
            writeln!(
                out,
                "      const {name}Size = checkedLength({name}Len);\n\x20     const {name} = \
                 decodeUtf8(checkedTake(data, offset, {name}Size));",
                name = arg.name
            )
            .expect("write to String");
            writeln!(out, "      offset += {}Size;", arg.name).expect("write to String");
        } else if let IdlType::Vec { vec } = &arg.ty {
            let item_codec = ts_codec(vec, target);
            writeln!(
                out,
                "      const {name}Count = checkedElementCount({name}Len, data.length - \
                 offset);\n\x20     const {name}Codec = getArrayCodec({item_codec}, {{ size: \
                 {name}Count }});",
                name = arg.name,
                item_codec = item_codec,
            )
            .expect("write to String");
            writeln!(
                out,
                "      const {name} = {name}Codec.decode(data.slice(offset));",
                name = arg.name
            )
            .expect("write to String");
            writeln!(
                out,
                "      offset += {name}Codec.encode({name}).length;",
                name = arg.name
            )
            .expect("write to String");
        }
    }

    // Build the return object
    let mut field_exprs = Vec::new();
    for arg in &fixed_args {
        if let IdlType::Option { option } = &arg.ty {
            field_exprs.push(format!(
                "{}: unwrapOption<{}>(fixedResult.{})",
                arg.name,
                ts_type(option),
                arg.name
            ));
        } else {
            field_exprs.push(format!("{}: fixedResult.{}", arg.name, arg.name));
        }
    }
    for arg in &dyn_args {
        field_exprs.push(arg.name.clone());
    }
    out.push_str("      assertFinished(data, offset);\n");
    writeln!(
        out,
        "      return {{ type: ProgramInstruction.{}, args: {{ {} }} }};",
        pascal,
        field_exprs.join(", ")
    )
    .expect("write to String");
}

/// Check if a type represents a string (for dynamic codec purposes).
pub(super) fn is_string_type(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "string")
}

pub(super) fn optional_dynamic_inner(ty: &IdlType) -> Option<&IdlType> {
    match ty {
        IdlType::Option { option }
            if is_string_type(option) || matches!(**option, IdlType::Vec { .. }) =>
        {
            Some(option)
        }
        _ => None,
    }
}

pub(super) fn is_optional_dynamic_string(ty: &IdlType) -> bool {
    optional_dynamic_inner(ty).is_some_and(is_string_type)
}

pub(super) fn is_optional_dynamic_vec(ty: &IdlType) -> bool {
    optional_dynamic_inner(ty).is_some_and(|inner| matches!(inner, IdlType::Vec { .. }))
}

pub(super) fn is_u8_type(ty: &IdlType) -> bool {
    matches!(ty, IdlType::Primitive(p) if p == "u8")
        || matches!(
            ty,
            IdlType::Defined { defined } if builtin_defined_primitive(&defined.name) == Some("u8")
        )
}

pub(super) fn builtin_defined_primitive(name: &str) -> Option<&'static str> {
    match name {
        "PodBool" => Some("bool"),
        "PodU8" => Some("u8"),
        "PodI8" => Some("i8"),
        "PodU16" => Some("u16"),
        "PodI16" => Some("i16"),
        "PodU32" => Some("u32"),
        "PodI32" => Some("i32"),
        "PodU64" => Some("u64"),
        "PodI64" => Some("i64"),
        "PodU128" => Some("u128"),
        "PodI128" => Some("i128"),
        _ => None,
    }
}

pub(super) fn primitive_ts_type(primitive: &str) -> String {
    match primitive {
        "u8" | "u16" | "u32" | "i8" | "i16" | "i32" => "number".to_string(),
        "u64" | "u128" | "i64" | "i128" => "bigint".to_string(),
        "bool" => "boolean".to_string(),
        "pubkey" => "Address".to_string(),
        "string" => "string".to_string(),
        "bytes" => "Uint8Array".to_string(),
        other if other.starts_with('[') => "Uint8Array".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn primitive_ts_codec(primitive: &str, target: TsTarget) -> String {
    match primitive {
        "u8" => "getU8Codec()".to_string(),
        "u16" => "getU16Codec()".to_string(),
        "u32" => "getU32Codec()".to_string(),
        "u64" => "getU64Codec()".to_string(),
        "u128" => "getU128Codec()".to_string(),
        "i8" => "getI8Codec()".to_string(),
        "i16" => "getI16Codec()".to_string(),
        "i32" => "getI32Codec()".to_string(),
        "i64" => "getI64Codec()".to_string(),
        "i128" => "getI128Codec()".to_string(),
        "bool" => "getBooleanCodec()".to_string(),
        "pubkey" => match target {
            TsTarget::Web3js => "getWeb3jsAddressCodec()".to_string(),
            TsTarget::Kit => "getAddressCodec()".to_string(),
        },
        "string" => "addCodecSizePrefix(getUtf8Codec(), getU32Codec())".to_string(),
        other if other.starts_with('[') => {
            let size = crate::codegen::parse_fixed_array_size(other).unwrap_or(1);
            format!("fixCodecSize(getBytesCodec(), {})", size)
        }
        other => format!("/* unknown: {} */", other),
    }
}

pub(super) fn ts_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => primitive_ts_type(p),
        IdlType::Option { option } => format!("{} | null", ts_type(option)),
        IdlType::Defined { defined } => builtin_defined_primitive(&defined.name)
            .map(primitive_ts_type)
            .unwrap_or_else(|| defined.name.clone()),
        IdlType::Vec { vec } => format!("Array<{}>", ts_type(vec)),
        IdlType::Array { array } => {
            let (item, _size) = array;
            if is_u8_type(item) {
                "Uint8Array".to_string()
            } else {
                format!("Array<{}>", ts_type(item))
            }
        }
        IdlType::Generic { generic } => generic.clone(),
    }
}

/// Generate codec expression for a type (no codec metadata).
pub(super) fn ts_codec(ty: &IdlType, target: TsTarget) -> String {
    match ty {
        IdlType::Primitive(p) => primitive_ts_codec(p, target),
        IdlType::Option { option } => format!("getOptionCodec({})", ts_codec(option, target)),
        IdlType::Defined { defined } => builtin_defined_primitive(&defined.name)
            .map(|primitive| primitive_ts_codec(primitive, target))
            .unwrap_or_else(|| format!("{}Codec", defined.name)),
        IdlType::Vec { vec } => {
            format!(
                "getArrayCodec({}, {{ size: getU32Codec() }})",
                ts_codec(vec, target)
            )
        }
        IdlType::Array { array } => {
            let (item, size) = array;
            if is_u8_type(item) {
                format!("fixCodecSize(getBytesCodec(), {})", size)
            } else {
                format!(
                    "getArrayCodec({}, {{ size: {} }})",
                    ts_codec(item, target),
                    size
                )
            }
        }
        IdlType::Generic { generic } => format!("/* generic: {} */", generic),
    }
}

/// Generate codec expression for a field def, using its codec metadata if
/// present.
pub(super) fn ts_codec_for_field_def(field: &IdlFieldDef, target: TsTarget) -> String {
    match &field.codec {
        Some(IdlCodec::SizePrefixed { prefix, item, .. }) => {
            let pfx_bytes = scalar_repr_bytes(prefix);
            let pfx_codec = prefix_codec(pfx_bytes);
            if is_string_type(&field.ty) {
                format!("addCodecSizePrefix(getUtf8Codec(), {})", pfx_codec)
            } else if let IdlType::Vec { vec } = &field.ty {
                let item_codec = match item {
                    Some(codec_item) => ts_codec(&codec_item.ty, target),
                    None => ts_codec(vec, target),
                };
                format!("getArrayCodec({}, {{ size: {} }})", item_codec, pfx_codec)
            } else {
                ts_codec(&field.ty, target)
            }
        }
        _ => ts_codec(&field.ty, target),
    }
}

/// Generate codec expression for an instruction arg, using its codec metadata
/// if present.
pub(super) fn ts_codec_for_arg(arg: &IdlArg, target: TsTarget) -> String {
    match &arg.codec {
        Some(IdlCodec::SizePrefixed { prefix, item, .. }) => {
            let pfx_bytes = scalar_repr_bytes(prefix);
            let pfx_codec = prefix_codec(pfx_bytes);
            if is_string_type(&arg.ty) {
                format!("addCodecSizePrefix(getUtf8Codec(), {})", pfx_codec)
            } else if let IdlType::Vec { vec } = &arg.ty {
                let item_codec = match item {
                    Some(codec_item) => ts_codec(&codec_item.ty, target),
                    None => ts_codec(vec, target),
                };
                format!("getArrayCodec({}, {{ size: {} }})", item_codec, pfx_codec)
            } else {
                ts_codec(&arg.ty, target)
            }
        }
        _ => ts_codec(&arg.ty, target),
    }
}

/// Extract prefix byte width from a codec's ScalarRepr.
pub(super) fn scalar_repr_bytes(repr: &ScalarRepr) -> usize {
    match repr.ty.as_str() {
        "u8" => 1,
        "u16" => 2,
        "u32" => 4,
        "u64" => 8,
        _ => 4,
    }
}

/// Extract prefix byte width from an optional codec on a field/arg.
pub(super) fn codec_prefix_bytes(codec: &Option<IdlCodec>) -> usize {
    match codec {
        Some(IdlCodec::SizePrefixed { prefix, .. }) => scalar_repr_bytes(prefix),
        _ => 4, // default u32
    }
}

/// Map prefix byte width to the corresponding TS codec expression.
pub(super) fn prefix_codec(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "getU8Codec()",
        2 => "getU16Codec()",
        4 => "getU32Codec()",
        _ => "getU64Codec()",
    }
}

/// Map prefix byte width to the integer type name used for codec tracking.
pub(super) fn prefix_int_type(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        _ => "u64",
    }
}

pub(super) fn collect_used_codecs(idl: &Idl) -> HashSet<String> {
    let mut used = HashSet::new();

    fn visit_type_into(ty: &IdlType, used: &mut HashSet<String>) {
        match ty {
            IdlType::Primitive(p) => {
                used.insert(p.clone());
            }
            IdlType::Option { option } => {
                used.insert("option".to_string());
                visit_type_into(option, used);
            }
            IdlType::Vec { vec } => {
                used.insert("dynVec".to_string());
                visit_type_into(vec, used);
            }
            IdlType::Array { array } => {
                if is_u8_type(&array.0) {
                    used.insert("fixedBytes".to_string());
                } else {
                    used.insert("fixedArray".to_string());
                    visit_type_into(&array.0, used);
                }
            }
            IdlType::Defined { defined } => {
                if let Some(primitive) = builtin_defined_primitive(&defined.name) {
                    used.insert(primitive.to_string());
                }
            }
            IdlType::Generic { .. } => {}
        }
    }

    fn visit_codec_into(codec: &Option<IdlCodec>, used: &mut HashSet<String>) {
        if let Some(IdlCodec::SizePrefixed { prefix, .. }) = codec {
            let pfx_bytes = scalar_repr_bytes(prefix);
            used.insert(prefix_int_type(pfx_bytes).to_string());
            used.insert("dynString".to_string());
        }
    }

    for type_def in &idl.types {
        for field in &type_def.fields {
            visit_type_into(&field.ty, &mut used);
            visit_codec_into(&field.codec, &mut used);
        }
    }
    for ix in &idl.instructions {
        for arg in &ix.args {
            visit_type_into(&arg.ty, &mut used);
            visit_codec_into(&arg.codec, &mut used);
        }
    }

    used
}

pub(super) const WEB3JS_ADDRESS_CODEC_HELPER: &str = r#"function getWeb3jsAddressCodec() {
  return transformCodec(
    fixCodecSize(getBytesCodec(), 32),
    (value: Address) => value.toBytes(),
    bytes => new Address(bytes),
  );
}
"#;

pub(super) const MATCH_DISC_HELPER: &str = r#"function matchDisc(data: Uint8Array, disc: Uint8Array): boolean {
  if (data.length < disc.length) return false;
  for (let i = 0; i < disc.length; i++) {
    if (data[i] !== disc[i]) return false;
  }
  return true;
}
"#;

pub(super) const TOTAL_DECODE_HELPERS: &str = r#"
const MAX_DECODE_ELEMENTS = 10 * 1024 * 1024;

function checkedLength(value: number | bigint): number {
  const result = Number(value);
  if (!Number.isSafeInteger(result) || result < 0) throw new Error("invalid length prefix");
  return result;
}

function checkedElementCount(value: number | bigint, remaining: number): number {
  const result = checkedLength(value);
  if (result > MAX_DECODE_ELEMENTS || result > remaining) {
    throw new Error("element count exceeds limit");
  }
  return result;
}

function checkedTake(data: Uint8Array, offset: number, size: number): Uint8Array {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(size) || offset < 0 || size < 0 || size > data.length - offset) {
    throw new Error("truncated input");
  }
  return data.slice(offset, offset + size);
}

function decodeUtf8(data: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: true }).decode(data);
}

function unwrapOption<T>(value: unknown): T | null {
  if (typeof value === "object" && value !== null && "__option" in value) {
    const option = value as { __option: string; value?: T };
    if (option.__option === "None") return null;
    if (option.__option === "Some") return option.value as T;
    throw new Error("invalid option tag");
  }
  return value as T | null;
}

function codecSize(
  codec: { encode(value: any): ArrayLike<number> },
  value: unknown,
): number {
  const fixedSize = (codec as { fixedSize?: unknown }).fixedSize;
  return typeof fixedSize === "number" ? fixedSize : codec.encode(value).length;
}

function bytesEqual(left: ArrayLike<number>, right: ArrayLike<number>): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}

function decodeExact<T>(
  codec: { decode(data: Uint8Array): unknown; encode(value: any): ArrayLike<number> },
  data: Uint8Array,
): T {
  const value = codec.decode(data);
  if (!bytesEqual(codec.encode(value), data)) throw new Error("invalid or trailing bytes");
  return value as T;
}

function assertFinished(data: Uint8Array, offset: number): void {
  if (offset !== data.length) throw new Error("trailing bytes");
}
"#;
