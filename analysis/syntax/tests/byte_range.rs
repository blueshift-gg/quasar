use {proc_macro2::TokenStream, std::str::FromStr};

#[test]
fn byte_range_maps_to_input_offsets() {
    let source = "discriminator = 42";
    //            0123456789012345678
    //            0         1
    let tokens = TokenStream::from_str(source).expect("tokenize");
    let trees: Vec<_> = tokens.into_iter().collect();
    assert_eq!(trees.len(), 3, "expected 3 tokens: ident, =, literal");

    let ident_range = trees[0].span().byte_range();
    assert_eq!(ident_range.start, 0, "ident starts at offset 0");
    assert_eq!(ident_range.end, 13, "ident ends after `discriminator`");
    assert_eq!(&source[ident_range.clone()], "discriminator");

    let eq_range = trees[1].span().byte_range();
    assert_eq!(eq_range.start, 14, "`=` at offset 14");
    assert_eq!(&source[eq_range.clone()], "=");

    let lit_range = trees[2].span().byte_range();
    assert_eq!(lit_range.start, 16, "literal at offset 16");
    assert_eq!(lit_range.end, 18, "literal ends after `42`");
    assert_eq!(&source[lit_range], "42");
}

#[test]
fn byte_range_survives_multiline_input() {
    let source = "discriminator\n  =\n  42";
    //            0..............13.....17....22
    let tokens = TokenStream::from_str(source).expect("tokenize");
    let trees: Vec<_> = tokens.into_iter().collect();
    assert_eq!(trees.len(), 3);

    let ident_range = trees[0].span().byte_range();
    assert_eq!(&source[ident_range], "discriminator");

    let lit_range = trees[2].span().byte_range();
    assert_eq!(&source[lit_range], "42");
}

#[test]
fn byte_range_survives_non_ascii() {
    // `é` is two UTF-8 bytes. Spans must point at byte offsets, not char offsets,
    // so `discriminator = 42` after a leading comment with `é` should still
    // have the correct byte positions.
    let source = "/* é */ discriminator = 42";
    //            0      7              22  26
    //            ^ é at bytes 3..5 (2 bytes)
    let tokens = TokenStream::from_str(source).expect("tokenize");
    let trees: Vec<_> = tokens.into_iter().collect();
    assert_eq!(trees.len(), 3, "comment is not a token");

    let ident_range = trees[0].span().byte_range();
    assert_eq!(&source[ident_range], "discriminator");
}
