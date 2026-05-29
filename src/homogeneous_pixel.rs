use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Type};

/// Main entry point for the HomogeneousPixel derive macro.
///
/// # Steps:
/// 1. Validate that the input is a struct (not enum or union)
/// 2. Validate that the struct has `#[repr(C)]` or `#[repr(transparent)]`
/// 3. Extract fields and verify all fields have the same type
/// 4. Generate `Channel` = common field type, `Channels` = `[Channel; N]`
/// 5. Emit `unsafe impl HomogeneousPixel for StructName { ... }`
pub(crate) fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    // Step 1 - Validate struct
    let name = &input.ident;
    let fields = validate_struct(&input)?;

    // Step 2 - Validate repr
    let repr = validate_repr(&input, fields)?;

    // Step 3 - Extract channel type and count
    let (channel_ty, channel_count) = extract_uniform_channel(fields, &repr)?;

    // Step 4 - Generate the impl block
    //
    // Force evaluation of the compile-time size assertion defined as a
    // default associated constant in `HomogeneousPixel`.  See REVIEW.md issue #2.
    let expanded = quote! {
        unsafe impl ::fovea::pixel::HomogeneousPixel for #name {
            type Channel = #channel_ty;
            type Channels = [#channel_ty; #channel_count];
        }
        const _: () = { let _ = <#name as ::fovea::pixel::HomogeneousPixel>::_SIZE_ASSERT; };
    };

    Ok(expanded)
}

/// Extracts the uniform channel type and count from the struct fields.
///
/// For `repr(C)`: all fields must have the same type. Returns (field_type, field_count).
/// For `repr(transparent)`: exactly one field. Returns (field_type, 1).
fn extract_uniform_channel(fields: &Fields, repr: &Repr) -> syn::Result<(Type, usize)> {
    let field_list: Vec<_> = fields.iter().collect();

    match repr {
        Repr::C => {
            // Already validated: at least one field
            let first_ty = &field_list[0].ty;
            let first_ty_str = type_to_string(first_ty);

            for (i, field) in field_list.iter().enumerate().skip(1) {
                let ty_str = type_to_string(&field.ty);
                if ty_str != first_ty_str {
                    let msg = if let Some(ref ident) = field.ident {
                        format!(
                            "HomogeneousPixel requires all fields to have the same type, \
                             but field `{ident}` (index {i}) has type `{ty_str}` while the first field has type `{first_ty_str}`"
                        )
                    } else {
                        format!(
                            "HomogeneousPixel requires all fields to have the same type, \
                             but field at index {i} has type `{ty_str}` while the first field has type `{first_ty_str}`"
                        )
                    };
                    return Err(syn::Error::new_spanned(&field.ty, msg));
                }
            }

            Ok((first_ty.clone(), field_list.len()))
        }
        Repr::Transparent => {
            // Already validated: exactly one field
            let field = &field_list[0];
            Ok((field.ty.clone(), 1))
        }
    }
}

/// Converts a `syn::Type` to a comparable string representation.
fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string()
}

/// Validates that the input is a struct.
/// Returns the fields if valid.
fn validate_struct(input: &DeriveInput) -> syn::Result<&Fields> {
    match &input.data {
        Data::Struct(data) => Ok(&data.fields),
        Data::Enum(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "HomogeneousPixel can only be derived for structs, not enums",
        )),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "HomogeneousPixel can only be derived for structs, not unions",
        )),
    }
}

#[derive(Debug)]
enum Repr {
    C,
    Transparent,
}

/// Validates that the struct has `#[repr(C)]` or `#[repr(transparent)]`.
fn validate_repr(input: &DeriveInput, fields: &Fields) -> syn::Result<Repr> {
    let has_repr_c = has_repr_c(&input.attrs);
    let has_repr_transparent = has_repr_transparent(&input.attrs);

    match (has_repr_c, has_repr_transparent) {
        (true, false) => {
            if fields.is_empty() {
                Err(syn::Error::new_spanned(
                    &input.ident,
                    "HomogeneousPixel requires at least one field",
                ))
            } else {
                Ok(Repr::C)
            }
        }
        (false, true) => {
            if fields.len() != 1 {
                Err(syn::Error::new_spanned(
                    &input.ident,
                    "HomogeneousPixel requires exactly one field when #[repr(transparent)] is used",
                ))
            } else {
                Ok(Repr::Transparent)
            }
        }
        (true, true) => Err(syn::Error::new_spanned(
            &input.ident,
            "HomogeneousPixel cannot be derived for structs with both #[repr(C)] and #[repr(transparent)] attributes",
        )),
        (false, false) => Err(syn::Error::new_spanned(
            &input.ident,
            "HomogeneousPixel requires #[repr(C)] or #[repr(transparent)] attribute",
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
    fn test_validate_struct_named_fields() {
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
    fn test_validate_struct_tuple_fields() {
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
    fn test_extract_uniform_channel_same_types() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel {
                r: u8,
                g: u8,
                b: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let repr = Repr::C;
        let (ty, count) = extract_uniform_channel(fields, &repr).unwrap();
        assert_eq!(type_to_string(&ty), "u8");
        assert_eq!(count, 3);
    }

    #[test]
    fn test_extract_uniform_channel_mixed_types() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel {
                r: u8,
                g: u16,
                b: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let repr = Repr::C;
        let result = extract_uniform_channel(fields, &repr);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("same type"));
    }

    #[test]
    fn test_extract_uniform_channel_transparent_single() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(transparent)]
            struct TestPixel(u8);
        };
        let fields = validate_struct(&input).unwrap();
        let repr = Repr::Transparent;
        let (ty, count) = extract_uniform_channel(fields, &repr).unwrap();
        assert_eq!(type_to_string(&ty), "u8");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_extract_uniform_channel_tuple_struct() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel(u16, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let repr = Repr::C;
        let (ty, count) = extract_uniform_channel(fields, &repr).unwrap();
        assert_eq!(type_to_string(&ty), "u16");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_derive_generates_correct_tokens() {
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
        assert!(output.contains("HomogeneousPixel"));
        assert!(output.contains("type Channel = u8"));
        // quote! may render the count as `3usize`, `3_usize`, or `3` depending on version
        assert!(output.contains("type Channels = [u8 ;"));
        assert!(
            output.contains("_SIZE_ASSERT"),
            "must force-evaluate _SIZE_ASSERT"
        );
    }

    #[test]
    fn test_extract_uniform_channel_tuple_mixed_types() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct TestPixel(u8, u16, u8);
        };
        let fields = validate_struct(&input).unwrap();
        let repr = Repr::C;
        let result = extract_uniform_channel(fields, &repr);
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("same type"));
        assert!(err_msg.contains("index"));
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
    fn test_derive_repr_transparent_full() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(transparent)]
            struct Wrapped {
                value: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("HomogeneousPixel"));
        assert!(output.contains("type Channel = u8"));
        assert!(output.contains("type Channels = [u8 ;"));
        assert!(
            output.contains("_SIZE_ASSERT"),
            "must force-evaluate _SIZE_ASSERT"
        );
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

    #[test]
    fn test_derive_rejects_mixed_types() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            struct BadPixel { r: u8, g: u16 }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("same type"));
    }
}
