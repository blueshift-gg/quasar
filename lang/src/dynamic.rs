/// Dynamic string field for `#[account]` and `#[instruction]` structs.
///
/// `String<N>` is a type alias for `PodString<N>`. In account structs, the
/// derive macro detects it and generates dynamic-layout accessors with a u8
/// length prefix. In instruction args, it becomes `&str` at runtime.
///
/// - `N`: maximum byte length. Defaults to `255`. Validated at write time.
///
/// # Examples
///
/// ```ignore
/// #[account(discriminator = 5)]
/// pub struct Profile {
///     pub owner: Address,
///     pub name: String<32>,           // max 32 bytes
///     pub bio: String<255>,           // max 255 bytes (default)
/// }
/// ```
pub type String<const N: usize = 255> = crate::pod::PodString<N>;

/// Dynamic array field for `#[account]` and `#[instruction]` structs.
///
/// `Vec<T, N>` is a type alias for `PodVec<T, N>`. In account structs, the
/// derive macro detects it and generates dynamic-layout accessors with a u16
/// element count prefix. In instruction args, it becomes `&[T]` at runtime.
///
/// - `T`: element type. Must be fixed-size with alignment 1.
/// - `N`: maximum element count. Defaults to `8`. Validated at write time.
///
/// # Examples
///
/// ```ignore
/// #[account(discriminator = 5)]
/// pub struct Profile {
///     pub owner: Address,
///     pub tags: Vec<Address, 8>,      // max 8 elements
/// }
/// ```
pub type Vec<T, const N: usize = 8> = crate::pod::PodVec<T, N>;
