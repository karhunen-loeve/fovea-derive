use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Result};

/// Main entry point for the PlainPixel derive macro.
///
/// # Steps to implement:
/// 1. Validate that the input is a struct (not enum or union)
/// 2. Validate that the struct has `#[repr(C)]`
/// 3. Extract named fields from the struct
/// 4. For each field, compute its channel size (will use the field type's SIZE)
/// 5. Generate the `CHANNELS` constant as a slice of sizes
/// 6. Emit the `unsafe impl PlainPixel for StructName { ... }`
pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    // Step 1 - Validate struct
    let name = &input.ident;
    let fields = validate_struct(&input)?;

    // Step 2 - Validate #[repr(C)] or #[repr(transparent)]
    let repr = validate_repr(&input, fields)?;

    // Step 3 - Generate CHANNELS constant
    let channels = generate_channels_constant(fields, &repr)?;

    // Step 4 - Generate the impl block
    //
    // Force evaluation of the compile-time assertions defined as default
    // associated constants in `PlainPixel`.  Without these references the
    // assertions are dead code — a broken pixel type could slip through
    // without triggering them.  See REVIEW.md issue #2.
    // ADR-0046: `PlainPixel` extends `PlainChannel`, so the derive
    // must emit both impls. `PlainChannel` carries the byte-layout
    // role (`SIZE`, `ALIGN`, `as_bytes`, `from_bytes`,
    // `_ASSERT_SIZE`); `PlainPixel` carries the pixel role
    // (`CHANNELS`, `DIM`, endian helpers, `cast_slice`,
    // `_ASSERT_CHANNELS`). Both use default method bodies; the
    // `unsafe impl` blocks exist solely to witness the invariants.
    //
    // SAFETY: the caller has `#[repr(C)]` or `#[repr(transparent)]`
    // (validated above). The compile-time `_ASSERT_SIZE` (byte-level
    // witness) and `_ASSERT_CHANNELS` (pixel-level witness) are
    // forced below so a broken pixel type cannot slip through
    // silently.
    let expanded = quote! {
        unsafe impl ::irys_cv::pixel::PlainChannel for #name {}
        unsafe impl ::irys_cv::pixel::PlainPixel for #name {
            const CHANNELS: &'static [usize] = #channels;
        }
        const _: () = { let _ = <#name as ::irys_cv::pixel::PlainChannel>::_ASSERT_SIZE; };
        const _: () = { let _ = <#name as ::irys_cv::pixel::PlainPixel>::_ASSERT_CHANNELS; };
    };

    Ok(expanded)
}

/// Generates the CHANNELS constant based on repr and fields.
///
/// For repr(C): &[<Field0>::SIZE, <Field1>::SIZE, ...]
/// For repr(transparent): <InnerField>::CHANNELS (inherit from inner type)
fn generate_channels_constant(fields: &Fields, repr: &Repr) -> Result<TokenStream> {
    match repr {
        Repr::C => {
            // for repr(C), CHANNELS = array of each field's SIZE.
            //
            // ADR-0046: resolve `SIZE` through `PlainChannel` (the
            // byte-layout role) instead of `PlainPixel` (the
            // pixel-role). Every existing `PlainPixel` field type
            // still satisfies this bound via the supertrait, and
            // `f32` / `f64` now satisfy it too, unblocking
            // float-channelled pixel structs (`MonoF32`, `RgbF32`,
            // ...) after ADR-0044 Phase E revokes
            // `PlainPixel for f32 / f64`.
            let field_types = fields.iter().map(|field| {
                let ty = &field.ty;
                quote! { <#ty as ::irys_cv::pixel::PlainChannel>::SIZE }
            });
            Ok(quote! { &[#(#field_types),*] })
        }
        Repr::Transparent => {
            // For repr(transparent), the wrapper has exactly one
            // field whose byte layout it inherits verbatim. Emit
            // CHANNELS as a single-channel slice whose only entry is
            // the inner type's `PlainChannel::SIZE`.
            //
            // ADR-0046: before this ADR the derive resolved through
            // `<ty as PlainPixel>::CHANNELS`, which required the
            // inner type to be a pixel. That's too strong — every
            // transparent pixel in the library wraps a single scalar
            // channel (`Mono8(Saturating<u8>)`, `MonoF32(f32)`,
            // etc.), and the channel role is what byte-inheritance
            // actually requires. Going through `PlainChannel::SIZE`
            // unblocks `MonoF32` / `MonoF64` (whose inner `f32` /
            // `f64` are `PlainChannel` but not `PlainPixel` after
            // ADR-0044 Phase E), and gives the same result for
            // every other transparent wrapper because they are all
            // single-channel.
            let field = fields.iter().next().expect("Expected exactly one field");
            let ty = &field.ty;
            Ok(quote! { &[<#ty as ::irys_cv::pixel::PlainChannel>::SIZE] })
        }
    }
}

/// Validates that the input is a struct with named fields.
/// Returns the fields if valid.
fn validate_struct(input: &DeriveInput) -> Result<&Fields> {
    match &input.data {
        Data::Struct(data) => Ok(&data.fields),
        Data::Enum(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "PlainPixel can only be derived for structs, not enums",
        )),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "PlainPixel can only be derived for structs, not unions",
        )),
    }
}

enum Repr {
    C,
    Transparent,
}

/// Validates that the struct has `#[repr(C)]` attribute.
fn validate_repr(input: &DeriveInput, fields: &Fields) -> Result<Repr> {
    let has_repr_c = has_repr_c(&input.attrs);
    let has_repr_transparent = has_repr_transparent(&input.attrs);

    match (has_repr_c, has_repr_transparent) {
        (true, false) => {
            if fields.is_empty() {
                Err(syn::Error::new_spanned(
                    &input.ident,
                    "PlainPixel requires at least one field",
                ))
            } else {
                Ok(Repr::C)
            }
        }
        (false, true) => {
            if fields.len() != 1 {
                Err(syn::Error::new_spanned(
                    &input.ident,
                    "PlainPixel requires exactly one field when #[repr(transparent)] is used",
                ))
            } else {
                Ok(Repr::Transparent)
            }
        }
        (true, true) => Err(syn::Error::new_spanned(
            &input.ident,
            "PlainPixel cannot be derived for structs with both #[repr(C)] and #[repr(transparent)] attributes",
        )),
        (false, false) => Err(syn::Error::new_spanned(
            &input.ident,
            "PlainPixel requires #[repr(C)] or #[repr(transparent)] attribute",
        )),
    }
}

fn has_repr_c(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("repr")
            && matches!(&attr.meta, syn::Meta::List(ml) if ml.tokens.to_string().contains("C"))
    })
}

fn has_repr_transparent(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("repr")
            && matches!(&attr.meta, syn::Meta::List(ml) if ml.tokens.to_string().contains("transparent"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::DeriveInput;

    #[test]
    fn test_validate_struct_with_named_fields() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel {
                r: u8,
                g: u8,
                b: u8,
            }
        };
        assert!(validate_struct(&input).is_ok());
    }

    #[test]
    fn test_validate_struct_with_tuple_fields() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel(u8, u8, u8);
        };
        assert!(validate_struct(&input).is_ok());
    }

    #[test]
    fn test_validate_struct_rejects_enum() {
        let input: DeriveInput = syn::parse_quote! {
            enum BadPixel {
                Red,
            }
        };
        let result = validate_struct(&input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not enums"));
    }

    #[test]
    fn test_validate_struct_rejects_union() {
        let input: DeriveInput = syn::parse_quote! {
            union BadPixel {
                a: u8,
                b: u16,
            }
        };
        let result = validate_struct(&input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not unions"));
    }

    #[test]
    fn test_validate_repr_c() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel {
                r: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        assert!(matches!(validate_repr(&input, fields), Ok(Repr::C)));
    }

    #[test]
    fn test_validate_repr_transparent() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(transparent)]
            struct TestPixel {
                value: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        assert!(matches!(
            validate_repr(&input, fields),
            Ok(Repr::Transparent)
        ));
    }

    #[test]
    fn test_validate_repr_missing() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel {
                r: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let result = validate_repr(&input, fields);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("requires #[repr(C)]")
        );
    }

    #[test]
    fn test_validate_repr_c_no_fields() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel {}
        };
        let fields = validate_struct(&input).unwrap();
        let result = validate_repr(&input, fields);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("at least one field")
        );
    }

    #[test]
    fn test_validate_repr_both_c_and_transparent() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            #[repr(transparent)]
            struct TestPixel {
                value: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let result = validate_repr(&input, fields);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("both"));
    }

    #[test]
    fn test_validate_repr_transparent_multiple_fields() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(transparent)]
            struct TestPixel {
                a: u8,
                b: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let result = validate_repr(&input, fields);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("exactly one field")
        );
    }

    #[test]
    fn test_generate_channels_repr_c() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel {
                r: u8,
                g: u8,
                b: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let tokens = generate_channels_constant(fields, &Repr::C).unwrap();
        let output = tokens.to_string();
        // ADR-0046: the derive resolves per-field `SIZE` through the
        // byte-layout trait `PlainChannel` (not the pixel-role trait
        // `PlainPixel`), so that float-channelled pixels like
        // `MonoF32` / `RgbF32` can be derived even though `f32` /
        // `f64` are no longer `PlainPixel` after ADR-0044 Phase E.
        assert!(output.contains("PlainChannel"));
        assert!(output.contains("SIZE"));
    }

    #[test]
    fn test_generate_channels_repr_transparent() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(transparent)]
            struct TestPixel {
                value: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let tokens = generate_channels_constant(fields, &Repr::Transparent).unwrap();
        let output = tokens.to_string();
        // ADR-0046: transparent wrappers now emit
        // `&[<InnerTy as PlainChannel>::SIZE]` instead of inheriting
        // `<InnerTy as PlainPixel>::CHANNELS`. This works for every
        // existing transparent pixel (they all wrap a single scalar
        // channel) and unblocks `MonoF32(f32)` / `MonoF64(f64)` after
        // ADR-0044 Phase E, where the inner float is a
        // `PlainChannel` but not a `PlainPixel`.
        assert!(output.contains("PlainChannel"));
        assert!(output.contains("SIZE"));
    }

    #[test]
    fn test_derive_repr_c_full() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct Rgb8 {
                r: u8,
                g: u8,
                b: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("PlainPixel"));
        assert!(output.contains("CHANNELS"));
        assert!(
            output.contains("_ASSERT_SIZE"),
            "must force-evaluate _ASSERT_SIZE"
        );
        assert!(
            output.contains("_ASSERT_CHANNELS"),
            "must force-evaluate _ASSERT_CHANNELS"
        );
    }

    #[test]
    fn test_derive_repr_transparent_full() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(transparent)]
            struct Wrapped {
                value: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("PlainPixel"));
        assert!(output.contains("CHANNELS"));
        assert!(
            output.contains("_ASSERT_SIZE"),
            "must force-evaluate _ASSERT_SIZE"
        );
        assert!(
            output.contains("_ASSERT_CHANNELS"),
            "must force-evaluate _ASSERT_CHANNELS"
        );
    }

    #[test]
    fn test_has_repr_c_true() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct Foo { x: u8 }
        };
        assert!(has_repr_c(&input.attrs));
        assert!(!has_repr_transparent(&input.attrs));
    }

    #[test]
    fn test_has_repr_transparent_true() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(transparent)]
            struct Foo { x: u8 }
        };
        assert!(!has_repr_c(&input.attrs));
        assert!(has_repr_transparent(&input.attrs));
    }

    #[test]
    fn test_has_repr_neither() {
        let input: DeriveInput = syn::parse_quote! {
            struct Foo { x: u8 }
        };
        assert!(!has_repr_c(&input.attrs));
        assert!(!has_repr_transparent(&input.attrs));
    }

    #[test]
    fn test_has_repr_with_non_repr_attr() {
        // A non-repr attribute exercises the `false` branch inside the `.any()` closure
        let input: DeriveInput = syn::parse_quote! {
            #[derive(Clone)]
            struct Foo { x: u8 }
        };
        assert!(!has_repr_c(&input.attrs));
        assert!(!has_repr_transparent(&input.attrs));
    }

    // -----------------------------------------------------------------------
    // derive() error-path tests — exercise the `?` branches inside derive()
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_rejects_enum() {
        let input: DeriveInput = syn::parse_quote! {
            enum BadPixel { Red }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not enums"));
    }

    #[test]
    fn test_derive_rejects_union() {
        let input: DeriveInput = syn::parse_quote! {
            union BadPixel { a: u8, b: u16 }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not unions"));
    }

    #[test]
    fn test_derive_rejects_missing_repr() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel { r: u8 }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("requires #[repr(C)]")
        );
    }

    #[test]
    fn test_derive_rejects_repr_c_no_fields() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct Empty {}
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("at least one field")
        );
    }
}
