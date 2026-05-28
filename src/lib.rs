use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod homogeneous_pixel;
mod linear_pixel;
mod plain_pixel;
mod white_channel;
mod zeroable_pixel;

/// Derive macro for `PlainPixel` trait.
///
/// # Requirements
/// - Struct must be `#[repr(C)]`
/// - Struct must have named fields
/// - All fields must be `PlainPixel` types themselves
///
/// # Example
/// ```ignore
/// #[derive(Clone, Copy, PlainPixel)]
/// #[repr(C)]
/// pub struct Rgb8 {
///     pub r: Saturating<u8>,
///     pub g: Saturating<u8>,
///     pub b: Saturating<u8>,
/// }
/// ```
///
/// # Generated Implementation
/// ```ignore
/// unsafe impl PlainPixel for Rgb8 {
///     const CHANNELS: &'static [usize] = &[1, 1, 1];
/// }
/// ```
#[proc_macro_derive(PlainPixel)]
pub fn derive_plain_pixel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    plain_pixel::derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive macro for `HomogeneousPixel` trait.
///
/// # Requirements
/// - Struct must be `#[repr(C)]` or `#[repr(transparent)]`
/// - For `#[repr(C)]`: all fields must have the **same type** (the channel type)
/// - For `#[repr(transparent)]`: exactly one field, treated as a single-channel pixel
/// - The struct must also implement `PlainPixel` (typically via `#[derive(PlainPixel)]`)
///
/// # Example
/// ```ignore
/// #[derive(Clone, Copy, PlainPixel, HomogeneousPixel)]
/// #[repr(C)]
/// pub struct Rgb8 {
///     pub r: Saturating<u8>,
///     pub g: Saturating<u8>,
///     pub b: Saturating<u8>,
/// }
/// ```
///
/// # Generated Implementation
/// ```ignore
/// unsafe impl HomogeneousPixel for Rgb8 {
///     type Channel = Saturating<u8>;
///     type Channels = [Saturating<u8>; 3];
/// }
/// ```
#[proc_macro_derive(HomogeneousPixel)]
pub fn derive_homogeneous_pixel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    homogeneous_pixel::derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive macro for `ZeroablePixel` trait.
///
/// Generates `zero()` by calling `ZeroablePixel::zero()` on each field's type.
///
/// # Requirements
/// - Must be a struct (not enum or union)
/// - Must have at least one field
/// - By default, all field types must implement `ZeroablePixel`
///
/// # Per-Field Control with `#[zero(...)]`
///
/// You can override the zero-initialization strategy for individual fields:
///
/// - **No attribute** (default): uses `<T as ZeroablePixel>::zero()`
/// - `#[zero(default)]`: uses `<T as Default>::default()`
/// - `#[zero(<expr>)]`: uses the given expression verbatim
///
/// # Example
/// ```ignore
/// #[derive(Clone, Copy, PlainPixel, ZeroablePixel)]
/// #[repr(C)]
/// pub struct Rgb8 {
///     pub r: Saturating<u8>,
///     pub g: Saturating<u8>,
///     pub b: Saturating<u8>,
/// }
/// ```
///
/// # Example with `#[zero(...)]` attributes
/// ```ignore
/// #[derive(Clone, Copy, ZeroablePixel)]
/// #[repr(C)]
/// pub struct PixelWithMeta {
///     pub r: u8,
///     pub g: u8,
///     pub b: u8,
///     #[zero(default)]
///     pub metadata: SomeDefaultType,
///     #[zero(MyTag::new(42))]
///     pub tag: MyTag,
/// }
/// ```
///
/// # Generated Implementation
/// ```ignore
/// impl ZeroablePixel for Rgb8 {
///     fn zero() -> Self {
///         Rgb8 {
///             r: <Saturating<u8> as ZeroablePixel>::zero(),
///             g: <Saturating<u8> as ZeroablePixel>::zero(),
///             b: <Saturating<u8> as ZeroablePixel>::zero(),
///         }
///     }
/// }
/// ```
#[proc_macro_derive(ZeroablePixel, attributes(zero))]
pub fn derive_zeroable_pixel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    zeroable_pixel::derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive macro for `LinearPixel` trait.
///
/// Generates channel-wise `Add`, `LinearPixel`, and optionally `FromLinear` impls,
/// plus the `LinearSpace` marker trait.
///
/// # Requirements
/// - Must be a struct with at least one field (named or tuple)
/// - Must have `#[linear(accumulator = AccType)]` attribute
///
/// # Attribute
/// - `accumulator = Type`: the intermediate-precision accumulator type.
///   - For storage pixels (e.g. `Rgb8`), use the float pixel (e.g. `RgbF32`).
///   - For float/accumulator pixels (e.g. `RgbF32`), use `Self`.
///   - When `Self` is used, no `FromLinear` impl is generated (blanket identity covers it).
///
/// # Example (storage pixel → float accumulator)
/// ```ignore
/// #[derive(Clone, Copy, LinearPixel)]
/// #[repr(C)]
/// #[linear(accumulator = RgbF32)]
/// pub struct Rgb8 {
///     pub r: Saturating<u8>,
///     pub g: Saturating<u8>,
///     pub b: Saturating<u8>,
/// }
/// ```
///
/// # Example (self-accumulating float pixel)
/// ```ignore
/// #[derive(Clone, Copy, LinearPixel)]
/// #[repr(C)]
/// #[linear(accumulator = Self)]
/// pub struct RgbF32 {
///     pub r: f32,
///     pub g: f32,
///     pub b: f32,
/// }
/// ```
///
/// # Generated impls
/// - `impl Add for T` — channel-wise addition
/// - `impl LinearPixel for T` — delegates `scale()` to each field's `LinearPixel::scale`
/// - `impl FromLinear<Acc> for T` — delegates per-field `FromLinear` conversion (only when accumulator ≠ `Self`)
/// - `impl LinearSpace for T` — marker trait asserting values are in a linear space
#[proc_macro_derive(LinearPixel, attributes(linear))]
pub fn derive_linear_pixel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    linear_pixel::derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive macro for `WhiteChannel` trait (ADR-0043).
///
/// Emits an impl delegating to the channel type's `BoundedChannel::MAX`.
/// This is the correct answer for every homogeneous pixel in the library
/// whose channel type is `BoundedChannel`.
///
/// **Reduced-range pixels (`Mono<BITS>`) must not use this derive.**
/// They implement `WhiteChannel` manually, returning the pixel's own
/// invariant-respecting maximum (e.g. `Saturating(1023)` for `Mono<10>`)
/// rather than the channel type's storage maximum
/// (`<Saturating<u16> as BoundedChannel>::MAX == Saturating(65535)`).
///
/// # Requirements
/// - Must be a struct (named or tuple).
/// - The struct must also implement `HomogeneousPixel` (typically via
///   `#[derive(HomogeneousPixel)]`).
/// - The struct's channel type (`<Self as HomogeneousPixel>::Channel`)
///   must implement `BoundedChannel`.
///
/// # Example
/// ```ignore
/// #[derive(Clone, Copy, PlainPixel, HomogeneousPixel, WhiteChannel)]
/// #[repr(C)]
/// pub struct Rgb8 {
///     pub r: Saturating<u8>,
///     pub g: Saturating<u8>,
///     pub b: Saturating<u8>,
/// }
/// ```
#[proc_macro_derive(WhiteChannel)]
pub fn derive_white_channel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    white_channel::derive(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
