use quasar_lang::{
    instruction_arg::{InstructionArg, OptionZc},
    pod::{PodBool, PodU64},
};

#[test]
fn option_u64_some_round_trip() {
    let val: Option<u64> = Some(42);
    let zc = val.to_zc();
    assert_eq!(zc.tag, 1);
    let decoded = Option::<u64>::from_zc(&zc);
    assert_eq!(decoded, Some(42));
}

#[test]
fn option_u64_none_round_trip() {
    let val: Option<u64> = None;
    let zc = val.to_zc();
    assert_eq!(zc.tag, 0);
    let decoded = Option::<u64>::from_zc(&zc);
    assert_eq!(decoded, None);
}

#[test]
fn option_address_some_round_trip() {
    let addr = solana_address::Address::from([42u8; 32]);
    let val: Option<solana_address::Address> = Some(addr);
    let zc = val.to_zc();
    assert_eq!(zc.tag, 1);
    let decoded = Option::<solana_address::Address>::from_zc(&zc);
    assert_eq!(decoded, Some(addr));
}

#[test]
fn option_address_none_round_trip() {
    let val: Option<solana_address::Address> = None;
    let zc = val.to_zc();
    assert_eq!(zc.tag, 0);
    let decoded = Option::<solana_address::Address>::from_zc(&zc);
    assert_eq!(decoded, None);
}

#[test]
fn option_zc_alignment_is_one() {
    assert_eq!(core::mem::align_of::<OptionZc<[u8; 8]>>(), 1);
    assert_eq!(core::mem::align_of::<OptionZc<[u8; 32]>>(), 1);
    assert_eq!(core::mem::align_of::<OptionZc<PodU64>>(), 1);
}

#[test]
fn option_zc_size_is_fixed() {
    // OptionZc<PodU64> = 1 (tag) + 8 (MaybeUninit<PodU64>) = 9
    assert_eq!(
        core::mem::size_of::<OptionZc<PodU64>>(),
        1 + core::mem::size_of::<PodU64>()
    );
    // OptionZc<Address> = 1 (tag) + 32 (MaybeUninit<Address>) = 33
    assert_eq!(
        core::mem::size_of::<OptionZc<solana_address::Address>>(),
        1 + core::mem::size_of::<solana_address::Address>()
    );
}

#[test]
fn option_tag_invalid_rejected() {
    let zc = OptionZc {
        tag: 2,
        value: core::mem::MaybeUninit::new(PodU64::from(42)),
    };
    assert!(Option::<u64>::validate_zc(&zc).is_err());
}

#[test]
fn option_tag_0xff_rejected() {
    let zc = OptionZc {
        tag: 0xFF,
        value: core::mem::MaybeUninit::new(PodU64::from(42)),
    };
    assert!(Option::<u64>::validate_zc(&zc).is_err());
}

#[test]
fn option_tag_valid_accepted() {
    let none_zc = None::<u64>.to_zc();
    assert!(Option::<u64>::validate_zc(&none_zc).is_ok());

    let some_zc = Some(42u64).to_zc();
    assert!(Option::<u64>::validate_zc(&some_zc).is_ok());
}

#[test]
fn option_none_payload_is_zeroed() {
    let zc = None::<u64>.to_zc();
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &zc.value as *const _ as *const u8,
            core::mem::size_of::<PodU64>(),
        )
    };
    assert!(bytes.iter().all(|&b| b == 0x00));
}

#[test]
fn option_nested_round_trip() {
    let some_some: Option<Option<u64>> = Some(Some(42));
    let zc = some_some.to_zc();
    assert_eq!(Option::<Option<u64>>::from_zc(&zc), Some(Some(42)));

    let some_none: Option<Option<u64>> = Some(None);
    let zc = some_none.to_zc();
    assert_eq!(Option::<Option<u64>>::from_zc(&zc), Some(None));

    let none: Option<Option<u64>> = None;
    let zc = none.to_zc();
    assert_eq!(Option::<Option<u64>>::from_zc(&zc), None);
}

#[test]
fn option_nested_size() {
    // OptionZc<OptionZc<PodU64>> = 1 (outer tag) + 1 (inner tag) + 8 (PodU64) = 10
    assert_eq!(core::mem::size_of::<OptionZc<OptionZc<PodU64>>>(), 10,);
}

#[test]
fn option_nested_validate_outer_invalid() {
    // Outer tag invalid, inner valid
    let zc = OptionZc {
        tag: 3,
        value: core::mem::MaybeUninit::new(Some(42u64).to_zc()),
    };
    assert!(Option::<Option<u64>>::validate_zc(&zc).is_err());
}

#[test]
fn option_nested_validate_both_valid() {
    let some_some = Some(Some(42u64)).to_zc();
    assert!(Option::<Option<u64>>::validate_zc(&some_some).is_ok());

    let some_none = Some(None::<u64>).to_zc();
    assert!(Option::<Option<u64>>::validate_zc(&some_none).is_ok());

    let none = None::<Option<u64>>.to_zc();
    assert!(Option::<Option<u64>>::validate_zc(&none).is_ok());
}

#[test]
fn validate_zc_noop_for_primitives() {
    // Primitives always pass validation (default no-op)
    assert!(u64::validate_zc(&PodU64::from(42)).is_ok());
    assert!(u8::validate_zc(&0u8).is_ok());
    assert!(bool::validate_zc(&PodBool::from(true)).is_ok());
}

#[test]
fn option_validate_all_boundary_tags() {
    // Tag 0 and 1 are valid
    for tag in 0..=1u8 {
        let zc = OptionZc {
            tag,
            value: core::mem::MaybeUninit::new(PodU64::from(0)),
        };
        assert!(
            Option::<u64>::validate_zc(&zc).is_ok(),
            "tag={tag} should be valid"
        );
    }
    // Tags 2..=255 are invalid
    for tag in 2..=255u8 {
        let zc = OptionZc {
            tag,
            value: core::mem::MaybeUninit::new(PodU64::from(0)),
        };
        assert!(
            Option::<u64>::validate_zc(&zc).is_err(),
            "tag={tag} should be invalid"
        );
    }
}
