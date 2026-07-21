use super::*;

#[repr(C)]
struct SmallEvent {
    disc: [u8; 4],
    amount: PodU64,
    flag: PodBool,
}
const _: () = assert!(size_of::<SmallEvent>() == 13);
const _: () = assert!(align_of::<SmallEvent>() == 1);

#[repr(C)]
struct WiderEvent {
    a: [u8; 32],
    b: PodU64,
    c: PodU32,
    d: PodU16,
    e: PodBool,
}
const _: () = assert!(size_of::<WiderEvent>() == 47);
const _: () = assert!(align_of::<WiderEvent>() == 1);

#[repr(C)]
struct MaxEvent {
    disc: [u8; 8],
    a: PodU128,
    b: PodI128,
    c: PodU64,
    d: PodI64,
    e: PodU32,
    f: PodI32,
    g: PodU16,
    h: PodI16,
    i: PodBool,
}
const _: () = assert!(size_of::<MaxEvent>() == 8 + 16 + 16 + 8 + 8 + 4 + 4 + 2 + 2 + 1);
const _: () = assert!(align_of::<MaxEvent>() == 1);

#[derive(Copy, Clone, Debug, PartialEq, Eq, quasar_lang::prelude::QuasarSerialize)]
struct RoundTripArgs {
    amount: u64,
    flag: bool,
}

#[test]
fn event_memcpy_small() {
    let event = SmallEvent {
        disc: [0xDE, 0xAD, 0xBE, 0xEF],
        amount: PodU64::from(1_000_000u64),
        flag: PodBool::from(true),
    };
    let mut buf = [0u8; 13];
    unsafe {
        core::ptr::copy_nonoverlapping(
            &event as *const SmallEvent as *const u8,
            buf.as_mut_ptr(),
            13,
        );
    }
    assert_eq!(&buf[0..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(
        u64::from_le_bytes(buf[4..12].try_into().unwrap()),
        1_000_000
    );
    assert_eq!(buf[12], 1);
}

#[test]
fn event_memcpy_wider() {
    let event = WiderEvent {
        a: [0xAA; 32],
        b: PodU64::from(u64::MAX),
        c: PodU32::from(u32::MAX),
        d: PodU16::from(u16::MAX),
        e: PodBool::from(true),
    };
    let mut buf = [0u8; 47];
    unsafe {
        core::ptr::copy_nonoverlapping(
            &event as *const WiderEvent as *const u8,
            buf.as_mut_ptr(),
            47,
        );
    }
    assert!(buf[..32].iter().all(|&b| b == 0xAA));
    assert_eq!(
        u64::from_le_bytes(buf[32..40].try_into().unwrap()),
        u64::MAX
    );
    assert_eq!(
        u32::from_le_bytes(buf[40..44].try_into().unwrap()),
        u32::MAX
    );
    assert_eq!(
        u16::from_le_bytes(buf[44..46].try_into().unwrap()),
        u16::MAX
    );
    assert_eq!(buf[46], 1);
}

#[test]
fn event_memcpy_max_all_pod_types() {
    let event = MaxEvent {
        disc: [0xFF; 8],
        a: PodU128::from(u128::MAX),
        b: PodI128::from(i128::MIN),
        c: PodU64::from(u64::MAX),
        d: PodI64::from(i64::MIN),
        e: PodU32::from(u32::MAX),
        f: PodI32::from(i32::MIN),
        g: PodU16::from(u16::MAX),
        h: PodI16::from(i16::MIN),
        i: PodBool::from(true),
    };
    let size = size_of::<MaxEvent>();
    let mut buf = vec![0u8; size];
    unsafe {
        core::ptr::copy_nonoverlapping(
            &event as *const MaxEvent as *const u8,
            buf.as_mut_ptr(),
            size,
        );
    }
    assert_eq!(&buf[0..8], &[0xFF; 8]);
    assert_eq!(event.a.get(), u128::MAX);
    assert_eq!(event.b.get(), i128::MIN);
}

#[test]
fn instruction_arg_round_trip_u64() {
    let zc = <u64 as InstructionArg>::to_zc(&777u64);
    assert_eq!(<u64 as InstructionArg>::from_zc(&zc), 777u64);
    assert_eq!(align_of::<<u64 as InstructionArg>::Zc>(), 1);
}

#[test]
fn instruction_arg_round_trip_struct() {
    let value = RoundTripArgs {
        amount: 55,
        flag: true,
    };
    let zc = <RoundTripArgs as InstructionArg>::to_zc(&value);
    assert_eq!(<RoundTripArgs as InstructionArg>::from_zc(&zc), value);
    assert_eq!(align_of::<<RoundTripArgs as InstructionArg>::Zc>(), 1);
}
