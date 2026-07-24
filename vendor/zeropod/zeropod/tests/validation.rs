use zeropod::{pod::PodBool, ZeroPod, ZeroPodCompact, ZeroPodFixed};

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Validatable {
    pub score: u64,
    pub active: bool,
    pub maybe: Option<u64>,
    pub name: zeropod::String<8>,
    pub items: zeropod::Vec<u8, 4>,
}

// Layout:
//   score:  offset 0,  size 8  (PodU64)
//   active: offset 8,  size 1  (PodBool)
//   maybe:  offset 9,  size 9  (PodOption<PodU64>: tag 1 + value 8)
//   name:   offset 18, size 9  (PodString<8,1>: len 1 + data 8)
//   items:  offset 27, size 6  (PodVec<u8,4,2>: len 2 + data 4)
//   total: 33

#[test]
fn validate_correct_size() {
    assert_eq!(<Validatable as ZeroPodFixed>::SIZE, 33);
}

#[test]
fn validate_zeroed_buffer_ok() {
    let buf = [0u8; 33];
    assert!(Validatable::from_bytes(&buf).is_ok());
}

#[test]
fn validate_bad_bool() {
    let mut buf = [0u8; 33];
    buf[8] = 2; // active field: invalid bool value
    assert!(Validatable::from_bytes(&buf).is_err());
}

#[test]
fn validate_truncated_buffer() {
    let buf = [0u8; 20]; // too small (need 33)
    assert!(Validatable::from_bytes(&buf).is_err());
}

#[test]
fn validate_bad_option_tag() {
    let mut buf = [0u8; 33];
    buf[9] = 3; // maybe field tag: invalid (must be 0 or 1)
    assert!(Validatable::from_bytes(&buf).is_err());
}

#[test]
fn validate_overlength_string() {
    let mut buf = [0u8; 33];
    buf[18] = 9; // name len prefix: 9 > max capacity 8
    assert!(Validatable::from_bytes(&buf).is_err());
}

#[test]
fn validate_overlength_vec() {
    let mut buf = [0u8; 33];
    buf[27] = 5; // items len prefix (LE u16 low byte): 5 > max capacity 4
    buf[28] = 0; // items len prefix (LE u16 high byte)
    assert!(Validatable::from_bytes(&buf).is_err());
}

// --- ZcValidate: invalid UTF-8 in fixed PodString ---

#[test]
fn validate_rejects_invalid_utf8_in_string() {
    let mut buf = [0u8; 33];
    // name field: offset 18, PodString<8,1>
    // Set len prefix to 2
    buf[18] = 2;
    // Write invalid UTF-8 bytes in the data portion (offset 19)
    buf[19] = 0xFF;
    buf[20] = 0xFE;
    assert!(Validatable::from_bytes(&buf).is_err());
}

// --- ZcValidate: Option<bool> inner validation ---

#[allow(dead_code)]
#[derive(ZeroPod)]
struct WithOptionBool {
    pub flag: Option<bool>,
}

// Layout: PodOption<PodBool>: tag(1) + PodBool(1) = 2

#[test]
fn validate_option_bool_none_ok() {
    let buf = [0u8; 2]; // tag=0, None
    assert!(WithOptionBool::from_bytes(&buf).is_ok());
}

#[test]
fn validate_option_bool_some_valid() {
    let buf = [1u8, 1]; // tag=1, inner=1 (true)
    assert!(WithOptionBool::from_bytes(&buf).is_ok());
}

#[test]
fn validate_option_bool_some_invalid_inner() {
    let buf = [1u8, 5]; // tag=1 (Some), inner byte=5 (invalid bool)
    assert!(WithOptionBool::from_bytes(&buf).is_err());
}

// --- ZcValidate: Option<Enum> inner validation ---

#[derive(ZeroPod, Debug, PartialEq)]
#[repr(u8)]
enum Color {
    Red = 0,
    Green = 1,
    Blue = 2,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct WithOptionEnum {
    pub color: Option<Color>,
}

// Layout: PodOption<ColorZc>: tag(1) + ColorZc(1) = 2

#[test]
fn validate_option_enum_none_ok() {
    let buf = [0u8; 2]; // tag=0, None
    assert!(WithOptionEnum::from_bytes(&buf).is_ok());
}

#[test]
fn validate_option_enum_some_valid() {
    let buf = [1u8, 2]; // tag=1, inner=2 (Blue)
    assert!(WithOptionEnum::from_bytes(&buf).is_ok());
}

#[test]
fn validate_option_enum_some_invalid_inner() {
    let buf = [1u8, 99]; // tag=1 (Some), inner=99 (invalid discriminant)
    assert!(WithOptionEnum::from_bytes(&buf).is_err());
}

// --- Compact validation ---

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct CompactVal {
    pub authority: [u8; 32],
    pub bio: zeropod::String<16>,
}

// Compact header: authority(32) + bio_len(1, PFX=1) = 33

#[test]
fn compact_validate_overlength_tail_string() {
    let mut buf = vec![0u8; 100];
    buf[32] = 17; // bio_len = 17 > max 16
    assert!(CompactVal::validate(&buf).is_err());
}

#[test]
fn compact_validate_tail_exceeds_buffer() {
    let mut buf = vec![0u8; 40]; // header(33) + only 7 bytes of tail
    buf[32] = 10; // bio_len = 10, needs 33 + 10 = 43 bytes
    assert!(CompactVal::validate(&buf).is_err());
}

#[test]
fn compact_validate_rejects_invalid_utf8_in_tail_string() {
    let mut buf = vec![0u8; 100];
    // bio_len at offset 32, PFX=1
    buf[32] = 3; // bio_len = 3
                 // bio data starts at offset 33 (header size = 33)
    buf[33] = 0xFF; // invalid UTF-8
    buf[34] = 0xFE;
    buf[35] = 0xFD;
    assert!(CompactVal::validate(&buf).is_err());
}

// --- Compact: inline bool validation via ZcValidate ---

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct CompactWithBool {
    pub active: bool,
    pub bio: zeropod::String<8>,
}

// Header: PodBool(1) + bio_len(1) = 2

#[test]
fn compact_validate_inline_bad_bool() {
    let mut buf = vec![0u8; 20];
    buf[0] = 5; // active field: invalid bool
    assert!(CompactWithBool::validate(&buf).is_err());
}

// --- ZcValidate: PodVec element validation ---
//
// Tests validate that PodVec<PodBool, N> correctly validates each element.
// We test at the storage level directly (no derive) since the derive lowers
// bool → PodBool automatically, and PodVec<bool> is intentionally not valid
// (bool is not ZcElem because &bool from arbitrary bytes is UB).

#[test]
fn validate_vec_bool_all_valid() {
    // Layout: PodVec<PodBool, 5, 2>: len(2) + data(5) = 7
    let mut buf = [0u8; 7];
    // len = 3 (LE u16)
    buf[0] = 3;
    buf[1] = 0;
    // elements: 0, 1, 0 (all valid bool)
    buf[2] = 0;
    buf[3] = 1;
    buf[4] = 0;
    let v = unsafe { &*(buf.as_ptr() as *const zeropod::pod::PodVec<PodBool, 5>) };
    assert!(zeropod::ZcValidate::validate_ref(v).is_ok());
}

#[test]
fn validate_vec_bool_rejects_invalid_element() {
    let mut buf = [0u8; 7];
    // len = 3 (LE u16)
    buf[0] = 3;
    buf[1] = 0;
    // elements: 0, 1, 5 — third element is invalid bool
    buf[2] = 0;
    buf[3] = 1;
    buf[4] = 5;
    let v = unsafe { &*(buf.as_ptr() as *const zeropod::pod::PodVec<PodBool, 5>) };
    assert!(zeropod::ZcValidate::validate_ref(v).is_err());
}

// --- ZcValidate: PodVec element validation works at the pod level ---
// Vec<Enum, N> in schema doesn't work directly because the type alias
// expands to PodVec<Enum, N> and Enum isn't Copy. This is a known v1
// limitation. For enum vectors, use PodBool as a proxy test since
// the ZcValidate recursion works the same way for any validated element type.

// --- PodString truncate char boundary ---

#[test]
fn podstring_truncate_snaps_to_char_boundary() {
    use zeropod::pod::PodString;
    let mut s = PodString::<32>::default();
    let _ = s.set("h\u{00e9}llo"); // 'e\u{0301}' — actually \u{00e9} is 2 bytes: [0xC3, 0xA9]
                                   // String bytes: h(1) + \u{00e9}(2) + l(1) + l(1) + o(1) = 6 bytes
    assert_eq!(s.len(), 6);

    // Truncate at byte 2 — mid-codepoint (inside the 2-byte \u{00e9})
    s.truncate(2);
    // Should snap back to byte 1 (after 'h')
    assert_eq!(s.len(), 1);
    assert_eq!(s.as_str(), "h");
    // Verify it's valid UTF-8
    assert!(core::str::from_utf8(s.as_bytes()).is_ok());
}

#[test]
fn podstring_truncate_at_boundary_is_exact() {
    use zeropod::pod::PodString;
    let mut s = PodString::<32>::default();
    let _ = s.set("h\u{00e9}llo");
    // Truncate at byte 3 — exactly after \u{00e9} (valid boundary)
    s.truncate(3);
    assert_eq!(s.len(), 3);
    assert_eq!(s.as_str(), "h\u{00e9}");
}

// --- Error variant specificity tests ---

#[test]
fn error_invalid_bool_variant() {
    let buf = [2u8]; // bad bool byte
    let val = unsafe { &*(buf.as_ptr() as *const zeropod::pod::PodBool) };
    let err = <zeropod::pod::PodBool as zeropod::ZcValidate>::validate_ref(val);
    assert_eq!(err, Err(zeropod::ZeroPodError::InvalidBool));
}

#[test]
fn error_invalid_tag_variant() {
    let buf = [5u8, 0u8]; // bad option tag
    let val = unsafe { &*(buf.as_ptr() as *const zeropod::pod::PodOption<u8>) };
    let err = <zeropod::pod::PodOption<u8> as zeropod::ZcValidate>::validate_ref(val);
    assert_eq!(err, Err(zeropod::ZeroPodError::InvalidTag));
}

// --- PodOption: is_some/is_none on invalid tag ---

#[test]
fn pod_option_invalid_tag_is_not_some() {
    // Construct a PodOption with raw tag = 0xFF (invalid).
    let buf = [0xFFu8, 42u8]; // PodOption<u8>: tag(1) + value(1)
    let opt = unsafe { &*(buf.as_ptr() as *const zeropod::pod::PodOption<u8>) };

    // is_some() must NOT return true for invalid tags.
    assert!(
        !opt.is_some(),
        "invalid tag 0xFF must not be treated as Some"
    );
    assert!(opt.is_none(), "invalid tag 0xFF must be treated as None");
    // get() must return None for invalid tags.
    assert_eq!(opt.get(), None);
}

// --- Wincode: PodOption inner validation ---

#[cfg(feature = "wincode")]
mod wincode_option_validation {
    use zeropod::pod::{PodBool, PodOption};

    #[test]
    fn wincode_read_rejects_option_with_invalid_inner() {
        // Construct raw bytes: tag=1 (Some), inner byte=5 (invalid PodBool).
        let bytes: [u8; 2] = [1, 5];
        let result = wincode::deserialize::<PodOption<PodBool>>(&bytes);
        assert!(
            result.is_err(),
            "wincode SchemaRead must reject PodOption<PodBool> with invalid inner byte"
        );
    }

    #[test]
    fn wincode_read_accepts_valid_option_some() {
        let bytes: [u8; 2] = [1, 1]; // tag=1, inner=1 (true)
        let result = wincode::deserialize::<PodOption<PodBool>>(&bytes);
        assert!(result.is_ok());
    }

    #[test]
    fn wincode_read_accepts_valid_option_none() {
        let bytes: [u8; 2] = [0, 0]; // tag=0
        let result = wincode::deserialize::<PodOption<PodBool>>(&bytes);
        assert!(result.is_ok());
    }
}

// --- Compact: tail Vec element validation ---

#[allow(dead_code)]
#[derive(ZeroPod)]
#[zeropod(compact)]
struct CompactWithVecBool {
    pub score: u64,
    pub flags: zeropod::Vec<PodBool, 4>,
}

// Compact header: score(PodU64=8) + flags_len([u8;2]=2) = 10
// On-chain: [header(10)][tail: flags data]

#[test]
fn compact_validate_rejects_invalid_vec_bool_element() {
    let mut buf = vec![0u8; 13]; // 10 header + 3 tail
                                 // flags_len at offset 8, PFX=2 (LE u16)
    buf[8] = 3; // count = 3
    buf[9] = 0;
    // Tail data at offset 10
    buf[10] = 0; // valid PodBool
    buf[11] = 1; // valid PodBool
    buf[12] = 5; // INVALID PodBool (byte > 1)

    assert!(
        CompactWithVecBool::validate(&buf).is_err(),
        "compact validate must reject Vec tail with invalid PodBool element"
    );
}

#[test]
fn compact_validate_accepts_valid_vec_bool_elements() {
    let mut buf = vec![0u8; 12]; // 10 header + 2 tail
    buf[8] = 2; // count = 2
    buf[9] = 0;
    buf[10] = 0; // valid
    buf[11] = 1; // valid

    assert!(CompactWithVecBool::validate(&buf).is_ok());
}
