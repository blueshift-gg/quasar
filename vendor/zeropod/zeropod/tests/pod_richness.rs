//! Comprehensive ergonomics test for enriched pod types.
//! Every interaction here should feel natural for a Rust developer.

use zeropod::{pod::*, ZeroPod, ZeroPodFixed};

// --- Numeric: feels like native integers ---

#[test]
fn numeric_feels_native() {
    let mut balance = PodU64::from(1000u64);

    // Arithmetic with native values
    balance += 500u64;
    assert!(balance > 1000u64);
    assert!(1500u64 == balance); // reverse comparison works

    // Checked arithmetic
    let result = balance.checked_sub(2000u64);
    assert!(result.is_none());

    // Wrapping
    let wrapped = PodU64::from(u64::MAX).wrapping_add(1u64);
    assert!(wrapped.is_zero());

    // Formatting
    let hex = format!("{:x}", PodU64::from(255u64));
    assert_eq!(hex, "ff");

    // Set
    balance.set(42u64);
    assert_eq!(balance.get(), 42);
}

// --- Bool: feels like native bool ---

#[test]
fn bool_feels_native() {
    let mut flag = PodBool::from(false);
    assert!(flag.is_false());
    assert!(!flag.is_true());

    flag.set(true);
    assert!(flag == true);
    assert!(true == flag); // reverse

    // Bitwise
    assert!((flag & false).is_false());
    assert!((flag | false).is_true());

    // Not
    assert!((!flag).is_false());
}

// --- String: ergonomic text handling ---

#[test]
fn string_feels_ergonomic() {
    let mut name = PodString::<32>::default();

    // Result-based API
    name.try_set("Alice").unwrap();
    assert_eq!(name.as_str(), "Alice");
    assert_eq!(name, *"Alice"); // PartialEq<str>

    // Append
    name.try_push_str(" Bob").unwrap();
    assert_eq!(name.len(), 9);
    assert_eq!(name.capacity(), 32);

    // Iteration
    assert_eq!(name.chars().count(), 9);

    // Overflow is explicit
    let mut tiny = PodString::<3>::default();
    assert!(tiny.try_set("toolong").is_err());
}

// --- Vec: ergonomic collection ---

#[test]
fn vec_feels_ergonomic() {
    let mut scores = PodVec::<u8, 5>::default();

    // Result-based push
    scores.try_push(10).unwrap();
    scores.try_push(20).unwrap();
    scores.try_push(30).unwrap();
    assert_eq!(scores.as_slice(), &[10, 20, 30]);
    assert_eq!(scores.capacity(), 5);

    // Bulk set
    scores.try_set_from_slice(&[1, 2, 3, 4, 5]).unwrap();
    assert_eq!(scores.len(), 5);

    // Overflow is explicit
    assert!(scores.try_push(6).is_err());

    // Pop
    assert_eq!(scores.pop(), Some(5));
    assert_eq!(scores.len(), 4);
}

// --- Option: feels like Option<T> ---

#[test]
fn option_feels_native() {
    let mut maybe = PodOption::<PodU64>::none();
    assert!(maybe == None);

    maybe.set(Some(PodU64::from(42u64)));
    assert!(maybe == Some(PodU64::from(42u64)));

    // Unwrap with default
    let val = maybe.unwrap_or(PodU64::from(0u64));
    assert_eq!(val, 42u64);

    // Take
    let taken = maybe.take();
    assert_eq!(taken, Some(PodU64::from(42u64)));
    assert!(maybe.is_none());

    // Map or
    maybe.set(Some(PodU64::from(10u64)));
    let doubled = maybe.map_or(0u64, |v| v.get() * 2);
    assert_eq!(doubled, 20u64);
}

// --- Enum: feels like Rust enum ---

#[derive(ZeroPod, Debug, PartialEq)]
#[repr(u8)]
enum Direction {
    North = 0,
    South = 1,
    East = 2,
    West = 3,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Compass {
    pub heading: Direction,
    pub bearing: u16,
}

#[test]
fn enum_feels_natural() {
    let mut buf = [0u8; 3]; // DirectionZc(1) + PodU16(2)
    let zc = Compass::from_bytes_mut(&mut buf).unwrap();

    // Set via From
    zc.heading = Direction::East.into();

    // Compare directly
    assert!(zc.heading == Direction::East);
    assert!(zc.heading.is(Direction::East));
    assert!(!zc.heading.is(Direction::North));

    // Decode back to enum
    let dir = zc.heading.try_to_enum().unwrap();
    assert_eq!(dir, Direction::East);

    // Display
    assert_eq!(format!("{}", zc.heading), "East");
}
