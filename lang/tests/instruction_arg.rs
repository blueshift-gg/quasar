use quasar_lang::{
    instruction_arg::{InstructionArg, OptionZc},
    pod::{PodBool, PodString, PodU64, PodVec},
};

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, quasar_lang::prelude::QuasarSerialize)]
enum Status {
    Pending = 1,
    Ready = 2,
    Failed = 9,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, quasar_lang::prelude::QuasarSerialize)]
struct NestedInner {
    maybe_amount: Option<u64>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, quasar_lang::prelude::QuasarSerialize)]
struct NestedOuter<T: InstructionArg> {
    inner: NestedInner,
    value: T,
}

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

#[test]
fn nested_custom_struct_round_trip() {
    let val = NestedOuter {
        inner: NestedInner {
            maybe_amount: Some(42),
        },
        value: 7u64,
    };
    let zc = val.to_zc();
    let decoded = NestedOuter::<u64>::from_zc(&zc);
    assert_eq!(decoded, val);
}

#[test]
fn nested_custom_struct_validate_recurses() {
    let bad = __NestedOuterZc::<u64> {
        inner: __NestedInnerZc {
            maybe_amount: OptionZc {
                tag: 2,
                value: core::mem::MaybeUninit::new(PodU64::from(42)),
            },
        },
        value: PodU64::from(7),
    };
    assert!(NestedOuter::<u64>::validate_zc(&bad).is_err());
}

#[test]
fn option_validate_recurses_into_inner() {
    // Option<Option<u64>>: outer tag=1 (Some), inner tag=5 (corrupt)
    // validate_zc must reject this via recursive validation.
    use quasar_lang::instruction_arg::InstructionArg;

    let bad: OptionZc<OptionZc<PodU64>> = OptionZc {
        tag: 1,
        value: core::mem::MaybeUninit::new(OptionZc {
            tag: 5, // corrupt inner tag
            value: core::mem::MaybeUninit::new(PodU64::from(42)),
        }),
    };
    assert!(
        <Option<Option<u64>> as InstructionArg>::validate_zc(&bad).is_err(),
        "should reject corrupt inner Option tag via recursive validate_zc"
    );
}

// --- PodString<N, PFX> as InstructionArg ---

#[test]
fn podstring_round_trip() {
    let mut s = PodString::<32>::default();
    assert!(s.set("hello"));
    let zc = <PodString<32> as InstructionArg>::to_zc(&s);
    let decoded = <PodString<32> as InstructionArg>::from_zc(&zc);
    assert_eq!(&*decoded, "hello");
}

#[test]
fn podstring_validate_valid() {
    let mut s = PodString::<32>::default();
    assert!(s.set("hi"));
    assert!(<PodString<32> as InstructionArg>::validate_zc(&s).is_ok());
}

#[test]
fn podstring_validate_rejects_corrupted_len() {
    // Simulate a corrupted length prefix that claims more than N bytes.
    let mut s = PodString::<4>::default();
    assert!(s.set("abcd"));
    // Corrupt the length to 5 (> N=4).
    // PodString<4, 1>: len is [u8; 1].
    // We need to set the raw len field to 5.
    // Use to_zc (identity) then corrupt via write.
    let mut zc = <PodString<4> as InstructionArg>::to_zc(&s);
    // decode_len > N should be rejected.
    // Access len bytes via ptr: PodString<4,1> is [len: [u8;1]][data:
    // [MaybeUninit<u8>;4]]
    let ptr = &mut zc as *mut PodString<4> as *mut u8;
    unsafe { *ptr = 5 }; // set len prefix to 5 > N=4
    assert!(<PodString<4> as InstructionArg>::validate_zc(&zc).is_err());
}

#[test]
fn podstring_zc_is_self() {
    // Verify Zc = Self (identity): no copy overhead
    assert_eq!(
        core::mem::size_of::<<PodString<32> as InstructionArg>::Zc>(),
        core::mem::size_of::<PodString<32>>()
    );
    assert_eq!(
        core::mem::align_of::<<PodString<32> as InstructionArg>::Zc>(),
        1
    );
}

// --- PodVec<T, N, PFX> as InstructionArg ---

#[test]
fn podvec_round_trip() {
    let mut v = PodVec::<u8, 8>::default();
    assert!(v.push(1));
    assert!(v.push(2));
    assert!(v.push(3));
    let zc = <PodVec<u8, 8> as InstructionArg>::to_zc(&v);
    let decoded = <PodVec<u8, 8> as InstructionArg>::from_zc(&zc);
    assert_eq!(decoded.as_slice(), &[1u8, 2, 3]);
}

#[test]
fn podvec_validate_valid() {
    let mut v = PodVec::<u8, 8>::default();
    assert!(v.push(42));
    assert!(<PodVec<u8, 8> as InstructionArg>::validate_zc(&v).is_ok());
}

#[test]
fn podvec_validate_rejects_corrupted_len() {
    // PodVec<u8, 4, 2>: len is [u8; 2] (PFX=2 default for PodVec<u8, 4>)
    let v = PodVec::<u8, 4>::default();
    let mut zc = <PodVec<u8, 4> as InstructionArg>::to_zc(&v);
    // Set len prefix to 5 (> N=4). PodVec<u8,4,2>: first 2 bytes are len (LE u16).
    let ptr = &mut zc as *mut PodVec<u8, 4> as *mut u8;
    unsafe {
        *ptr = 5;
        *ptr.add(1) = 0;
    } // len = 5 in LE u16
    assert!(<PodVec<u8, 4> as InstructionArg>::validate_zc(&zc).is_err());
}

#[test]
fn podvec_zc_is_self() {
    assert_eq!(
        core::mem::size_of::<<PodVec<u8, 8> as InstructionArg>::Zc>(),
        core::mem::size_of::<PodVec<u8, 8>>()
    );
    assert_eq!(
        core::mem::align_of::<<PodVec<u8, 8> as InstructionArg>::Zc>(),
        1
    );
}

// --- repr-backed enums as InstructionArg ---

#[test]
fn repr_enum_round_trip() {
    let zc = Status::Ready.to_zc();
    assert_eq!(Status::from_zc(&zc), Status::Ready);
}

#[test]
fn repr_enum_validate_accepts_known_discriminants() {
    for status in [Status::Pending, Status::Ready, Status::Failed] {
        let zc = status.to_zc();
        assert!(Status::validate_zc(&zc).is_ok());
    }
}

#[test]
fn repr_enum_validate_rejects_invalid_discriminant() {
    let zc = 3u8;
    assert!(Status::validate_zc(&zc).is_err());
}
