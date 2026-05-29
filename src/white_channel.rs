use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

/// Derive `WhiteChannel` for a pixel type.
///
/// Emits a trivial impl delegating to the channel type's
/// `BoundedChannel::MAX`. This is the correct answer for every
/// homogeneous pixel in the library whose channel type is
/// `BoundedChannel` (every integer-channel family: `Rgb8`, `Rgba16`,
/// `Bgr32`, `MonoA64`, …).
///
/// **Reduced-range pixels must not use this derive.** `Mono<BITS>`
/// maintains a tighter invariant than its channel type exposes
/// (`(1 << BITS) - 1 <= u16::MAX`); it implements `WhiteChannel`
/// manually, returning `Saturating(Self::MAX)` instead of the channel
/// type's storage maximum.
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
///
/// Expands to:
///
/// ```ignore
/// impl ::fovea::pixel::WhiteChannel for Rgb8 {
///     #[inline(always)]
///     fn white_channel() -> <Self as ::fovea::pixel::HomogeneousPixel>::Channel {
///         <<Self as ::fovea::pixel::HomogeneousPixel>::Channel
///             as ::fovea::pixel::BoundedChannel>::MAX
///     }
/// }
/// ```
pub(crate) fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    // We only support structs (pixels are structs). The actual channel
    // type is picked up through `HomogeneousPixel`; we do not need to
    // inspect fields further — but we still reject non-structs to keep
    // the error message local and consistent with the sibling derives.
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(_) | Fields::Unnamed(_) => {}
            Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "WhiteChannel cannot be derived for unit structs (no channels)",
                ));
            }
        },
        Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "WhiteChannel can only be derived for structs, not enums",
            ));
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "WhiteChannel can only be derived for structs, not unions",
            ));
        }
    }

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ::fovea::pixel::WhiteChannel for #name #ty_generics #where_clause {
            #[inline(always)]
            fn white_channel() -> <Self as ::fovea::pixel::HomogeneousPixel>::Channel {
                <
                    <Self as ::fovea::pixel::HomogeneousPixel>::Channel
                    as ::fovea::pixel::BoundedChannel
                >::MAX
            }
        }
    };

    Ok(expanded)
}
