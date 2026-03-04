/// Dynamic string field for `#[account]` structs.
///
/// `String<'a, N>` is a marker type recognized by the `#[account]` macro.
/// In the emitted code, it becomes `&'a str`. `N` is the maximum byte length.
///
/// A `PodU16` length descriptor is stored in the ZC companion struct.
/// The actual string bytes are packed in the variable-length tail region
/// after all fixed fields and length descriptors.
///
/// # Example
///
/// ```ignore
/// #[account(discriminator = 5)]
/// pub struct Profile<'a> {
///     pub owner: Address,
///     pub name: String<'a, 32>,
/// }
/// ```
pub struct String<'a, const MAX: usize>(core::marker::PhantomData<&'a str>);

/// Dynamic array field for `#[account]` structs.
///
/// `Vec<'a, T, N>` is a marker type recognized by the `#[account]` macro.
/// In the emitted code, it becomes `&'a [T]`. `N` is the maximum element count.
/// `T` must be a fixed-size, alignment-1 type (e.g. `Address`, `PodU64`).
///
/// A `PodU16` count descriptor is stored in the ZC companion struct.
/// The actual elements are packed in the variable-length tail region.
///
/// # Example
///
/// ```ignore
/// #[account(discriminator = 5)]
/// pub struct Profile<'a> {
///     pub owner: Address,
///     pub tags: Vec<'a, Address, 10>,
/// }
/// ```
pub struct Vec<'a, T, const MAX: usize>(core::marker::PhantomData<&'a [T]>);

/// Maximum stack buffer size used for dynamic field updates when `alloc` is disabled.
pub const MAX_DYNAMIC_TAIL: usize = 2048;
