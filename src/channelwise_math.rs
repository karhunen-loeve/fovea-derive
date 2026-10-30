use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

/// Derive `ChannelwiseMath` for a pixel type.
///
/// Emits an empty marker impl. The marker claims that every channel is a
/// scalar quantity on its own, so a scalar operation applied per channel
/// (a difference, a maximum, a threshold, a statistic) means something.
/// The macro cannot check that claim; it only states it, which is why it
/// is a separate derive and not implied by `HomogeneousPixel`: layout and
/// meaning are two different promises.
///
/// Do not derive it for a channel that is not a quantity (a component
/// label, a palette index) or for channels that together form one value
/// (the real and imaginary parts of a complex number).
///
/// # Example
/// ```ignore
/// #[derive(Clone, Copy, PlainPixel, HomogeneousPixel, ChannelwiseMath)]
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
/// impl ::fovea::pixel::ChannelwiseMath for Rgb8 {}
/// ```
pub(crate) fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    // The marker needs no field inspection: its supertrait
    // `HomogeneousPixel` carries the layout. Non-structs are still rejected
    // here, so the error is local and matches the sibling derives.
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(_) | Fields::Unnamed(_) => {}
            Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "ChannelwiseMath cannot be derived for unit structs (no channels)",
                ));
            }
        },
        Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "ChannelwiseMath can only be derived for structs, not enums",
            ));
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "ChannelwiseMath can only be derived for structs, not unions",
            ));
        }
    }

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ::fovea::pixel::ChannelwiseMath for #name #ty_generics #where_clause {}
    };

    Ok(expanded)
}
