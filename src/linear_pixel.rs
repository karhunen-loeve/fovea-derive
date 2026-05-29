use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Data, DeriveInput, Fields, Result, Type,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

/// Parsed content of the `#[linear(...)]` attribute.
struct LinearAttr {
    accumulator: Type,
}

/// Key-value pair inside `#[linear(...)]`.
struct LinearKV {
    key: syn::Ident,
    _eq: syn::Token![=],
    value: Type,
}

impl Parse for LinearKV {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(LinearKV {
            key: input.parse()?,
            _eq: input.parse()?,
            value: input.parse()?,
        })
    }
}

/// Parses the `#[linear(accumulator = SomeType)]` attribute from the list of attributes.
fn parse_linear_attr(attrs: &[Attribute]) -> Result<LinearAttr> {
    let mut accumulator: Option<Type> = None;

    for attr in attrs {
        if !attr.path().is_ident("linear") {
            continue;
        }

        let nested = attr.parse_args_with(Punctuated::<LinearKV, Comma>::parse_terminated)?;

        for kv in nested {
            if kv.key == "accumulator" {
                if accumulator.is_some() {
                    return Err(syn::Error::new_spanned(
                        &kv.key,
                        "duplicate `accumulator` key in #[linear(...)]",
                    ));
                }
                accumulator = Some(kv.value);
            } else {
                return Err(syn::Error::new_spanned(
                    &kv.key,
                    format!(
                        "unknown key `{}` in #[linear(...)]; expected `accumulator`",
                        kv.key
                    ),
                ));
            }
        }
    }

    match accumulator {
        Some(acc) => Ok(LinearAttr { accumulator: acc }),
        None => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "LinearPixel derive requires #[linear(accumulator = Type)] attribute",
        )),
    }
}

/// Returns true if the accumulator type is `Self`.
fn is_self_accumulator(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.qself.is_none() && tp.path.is_ident("Self"))
}

/// Returns the qualified trait path the derive macro should use to
/// access a field's linear-arithmetic methods.
///
/// By default, fields are treated as **channels** and the emission
/// uses `LinearChannel<f32>`. When a field is tagged
/// `#[linear(nested)]` the macro falls back to `LinearPixel<f32>`
/// (used by the `Rgb<BITS>` / `Rgba<BITS>` families whose fields are
/// themselves pixels, not channels — the single cross-classification
/// exception the library needs).
///
/// The two sets are disjoint by construction: channel primitives
/// implement `LinearChannel` and not `LinearPixel`; pixel-named
/// types implement `LinearPixel` and not `LinearChannel`. The probe
/// is therefore unambiguous.
fn field_trait_path(field: &syn::Field) -> Result<TokenStream> {
    for attr in &field.attrs {
        if !attr.path().is_ident("linear") {
            continue;
        }
        // Field-level `#[linear(...)]` currently accepts only the
        // bare flag `nested`.
        let ident: syn::Ident = attr.parse_args()?;
        if ident == "nested" {
            return Ok(quote! { ::fovea::pixel::LinearPixel<f32> });
        }
        return Err(syn::Error::new_spanned(
            &ident,
            format!("unknown key `{ident}` in field-level #[linear(...)]; expected `nested`"),
        ));
    }
    Ok(quote! { ::fovea::pixel::LinearChannel<f32> })
}

/// Main entry point for the LinearPixel derive macro.
///
/// # Requirements
/// - Struct must have named fields or be a tuple struct
/// - Must have `#[linear(accumulator = AccType)]` attribute
///
/// # Generated impls
/// - `impl Add for T` (channel-wise addition)
/// - `impl LinearPixel for T` (delegates `scale` to each field)
/// - `impl FromLinear<Acc> for T` (if accumulator ≠ Self, delegates per-field conversion)
/// - `impl LinearSpace for T` (marker trait)
pub(crate) fn derive(input: DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let fields = validate_struct(&input)?;
    let linear_attr = parse_linear_attr(&input.attrs)?;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let acc_ty = &linear_attr.accumulator;
    let is_self_acc = is_self_accumulator(acc_ty);

    // The actual accumulator type to use in generated code.
    // When the user writes `Self`, we substitute the concrete type name + generics.
    let concrete_acc: TokenStream = if is_self_acc {
        quote! { #name #ty_generics }
    } else {
        quote! { #acc_ty }
    };

    let add_impl = generate_add(name, fields, &impl_generics, &ty_generics, where_clause)?;
    let sub_impl = generate_sub(name, fields, &impl_generics, &ty_generics, where_clause)?;
    let mul_impl = generate_mul(name, fields, &impl_generics, &ty_generics, where_clause)?;
    let linear_impl = generate_linear_pixel(
        name,
        fields,
        &concrete_acc,
        &impl_generics,
        &ty_generics,
        where_clause,
    )?;

    let from_linear_impl = if is_self_acc {
        // Blanket identity `FromLinear<T> for T` already covers this case.
        quote! {}
    } else {
        generate_from_linear(
            name,
            fields,
            &concrete_acc,
            &impl_generics,
            &ty_generics,
            where_clause,
        )?
    };

    let linear_space_impl = quote! {
        impl #impl_generics ::fovea::pixel::LinearSpace for #name #ty_generics #where_clause {}
    };

    Ok(quote! {
        #add_impl
        #sub_impl
        #mul_impl
        #linear_impl
        #from_linear_impl
        #linear_space_impl
    })
}

/// Generates `impl Add for T` with channel-wise addition.
fn generate_add(
    name: &syn::Ident,
    fields: &Fields,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
) -> Result<TokenStream> {
    let body = match fields {
        Fields::Named(named) => {
            let field_adds = named.named.iter().map(|f| {
                let ident = f.ident.as_ref().unwrap();
                quote! { #ident: self.#ident + other.#ident }
            });
            quote! {
                #name {
                    #(#field_adds),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            let field_adds = unnamed.unnamed.iter().enumerate().map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx + other.#idx }
            });
            quote! {
                #name(#(#field_adds),*)
            }
        }
        Fields::Unit => {
            return Err(syn::Error::new_spanned(
                name,
                "LinearPixel cannot be derived for unit structs",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics ::std::ops::Add for #name #ty_generics #where_clause {
            type Output = Self;
            #[inline(always)]
            fn add(self, other: Self) -> Self {
                #body
            }
        }
    })
}

/// Generates `impl Sub for T` with channel-wise subtraction.
fn generate_sub(
    name: &syn::Ident,
    fields: &Fields,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
) -> Result<TokenStream> {
    let body = match fields {
        Fields::Named(named) => {
            let field_subs = named.named.iter().map(|f| {
                let ident = f.ident.as_ref().unwrap();
                quote! { #ident: self.#ident - other.#ident }
            });
            quote! {
                #name {
                    #(#field_subs),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            let field_subs = unnamed.unnamed.iter().enumerate().map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx - other.#idx }
            });
            quote! {
                #name(#(#field_subs),*)
            }
        }
        Fields::Unit => {
            return Err(syn::Error::new_spanned(
                name,
                "LinearPixel cannot be derived for unit structs",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics ::std::ops::Sub for #name #ty_generics #where_clause {
            type Output = Self;
            #[inline(always)]
            fn sub(self, other: Self) -> Self {
                #body
            }
        }
    })
}

/// Generates `impl Mul for T` with channel-wise multiplication.
fn generate_mul(
    name: &syn::Ident,
    fields: &Fields,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
) -> Result<TokenStream> {
    let body = match fields {
        Fields::Named(named) => {
            let field_muls = named.named.iter().map(|f| {
                let ident = f.ident.as_ref().unwrap();
                quote! { #ident: self.#ident * other.#ident }
            });
            quote! {
                #name {
                    #(#field_muls),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            let field_muls = unnamed.unnamed.iter().enumerate().map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx * other.#idx }
            });
            quote! {
                #name(#(#field_muls),*)
            }
        }
        Fields::Unit => {
            return Err(syn::Error::new_spanned(
                name,
                "LinearPixel cannot be derived for unit structs",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics ::std::ops::Mul for #name #ty_generics #where_clause {
            type Output = Self;
            #[inline(always)]
            fn mul(self, other: Self) -> Self {
                #body
            }
        }
    })
}

/// Generates `impl LinearPixel for T`.
///
/// For named structs: constructs the accumulator struct with same field names,
/// calling `LinearPixel::scale` and `LinearPixel::scale_add` on each field.
///
/// For tuple structs with one field: returns the single field's scale/scale_add
/// result directly.
///
/// For tuple structs with N fields: constructs the accumulator tuple struct
/// calling `LinearPixel::scale` and `LinearPixel::scale_add` on each field by index.
///
/// Also emits `uniform(scalar) -> Accumulator`, which broadcasts a scalar into
/// every channel of the accumulator. For named structs this constructs the
/// accumulator struct with every field initialized via
/// `<FieldTy as LinearPixel>::uniform(scalar)`; for tuple structs the same
/// pattern is applied by index; for single-field tuple structs the field's
/// `uniform` is returned directly. Supports the brightness/contrast
/// composition `scale_add(pixel, contrast, uniform(brightness))`.
fn generate_linear_pixel(
    name: &syn::Ident,
    fields: &Fields,
    acc_ty: &TokenStream,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
) -> Result<TokenStream> {
    // Per-field trait emission:
    //   - By default, a field is assumed to be a **channel** and we
    //     emit `<FieldTy as LinearChannel<f32>>::method(..)`.
    //   - When a field is tagged `#[linear(nested)]` it is a nested
    //     pixel (the `Rgb<BITS>` / `Rgba<BITS>` families that wrap
    //     `Mono<BITS>`) and we emit
    //     `<FieldTy as LinearPixel<f32>>::method(..)`.
    //
    // All method calls are qualified with the explicit scalar type
    // `f32`. The unqualified form `LinearChannel::method(..)` /
    // `LinearPixel::method(..)` is ambiguous because the library
    // ships f64-scalar siblings for f64-accumulator primitives
    //.
    let field_paths: Vec<TokenStream> = match fields {
        Fields::Named(n) => n
            .named
            .iter()
            .map(field_trait_path)
            .collect::<Result<Vec<_>>>()?,
        Fields::Unnamed(u) => u
            .unnamed
            .iter()
            .map(field_trait_path)
            .collect::<Result<Vec<_>>>()?,
        Fields::Unit => unreachable!("unit structs rejected earlier"),
    };

    let to_acc_body = match fields {
        Fields::Named(named) => {
            // call `.into()` at each field boundary so
            // that a nested-pixel field whose accumulator differs from
            // the outer accumulator's field type (e.g. `Mono<BITS>::
            // Accumulator = MonoF32` flowing into an `RgbF32 { r: f32 }`)
            // is converted via the appropriate `From` impl. For matching
            // types this is the identity blanket — zero cost.
            let field_to_accs = named.named.iter().zip(field_paths.iter()).map(|(f, path)| {
                let ident = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                quote! { #ident: <#ty as #path>::to_accumulator(&self.#ident).into() }
            });
            quote! {
                #acc_ty {
                    #(#field_to_accs),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            if unnamed.unnamed.len() == 1 {
                // Single-field tuple struct: delegate to the inner field's
                // accumulator, then convert into the declared `#acc_ty` via
                // `Into`. For matching types (e.g. `f32` → `f32`) this is the
                // identity blanket; for Phase-B types (`f32` → `MonoF32`) it
                // picks up the `From<f32> for MonoF32` impl.
                let ty = &unnamed.unnamed.first().unwrap().ty;
                let path = &field_paths[0];
                quote! {
                    ::core::convert::Into::<#acc_ty>::into(
                        <#ty as #path>::to_accumulator(&self.0)
                    )
                }
            } else {
                let field_to_accs = unnamed
                    .unnamed
                    .iter()
                    .zip(field_paths.iter())
                    .enumerate()
                    .map(|(i, (f, path))| {
                        let idx = syn::Index::from(i);
                        let ty = &f.ty;
                        quote! { <#ty as #path>::to_accumulator(&self.#idx).into() }
                    });
                quote! {
                    #acc_ty(#(#field_to_accs),*)
                }
            }
        }
        Fields::Unit => unreachable!("unit structs rejected earlier"),
    };

    let uniform_body = match fields {
        Fields::Named(named) => {
            // `.into()` at each field boundary. Identity
            // for channel fields; applies `From` for nested-pixel fields
            // whose accumulator differs from the outer field's type.
            let field_uniforms = named.named.iter().zip(field_paths.iter()).map(|(f, path)| {
                let ident = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                quote! { #ident: <#ty as #path>::uniform(scalar).into() }
            });
            quote! {
                #acc_ty {
                    #(#field_uniforms),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            if unnamed.unnamed.len() == 1 {
                // Single-field tuple struct: wrap the inner's broadcast into
                // the declared accumulator via `Into`.
                let ty = &unnamed.unnamed.first().unwrap().ty;
                let path = &field_paths[0];
                quote! {
                    ::core::convert::Into::<#acc_ty>::into(
                        <#ty as #path>::uniform(scalar)
                    )
                }
            } else {
                let field_uniforms =
                    unnamed
                        .unnamed
                        .iter()
                        .zip(field_paths.iter())
                        .map(|(f, path)| {
                            let ty = &f.ty;
                            quote! { <#ty as #path>::uniform(scalar).into() }
                        });
                quote! {
                    #acc_ty(#(#field_uniforms),*)
                }
            }
        }
        Fields::Unit => unreachable!("unit structs rejected earlier"),
    };

    let scale_body = match fields {
        Fields::Named(named) => {
            // `.into()` at each field boundary.
            let field_scales = named.named.iter().zip(field_paths.iter()).map(|(f, path)| {
                let ident = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                quote! { #ident: <#ty as #path>::scale(&self.#ident, scalar).into() }
            });
            quote! {
                #acc_ty {
                    #(#field_scales),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            if unnamed.unnamed.len() == 1 {
                // Single-field tuple struct: compute the inner's scaled
                // accumulator, then convert into the declared `#acc_ty` via
                // `Into`. For matching types this is the identity blanket;
                // for Phase-B types (`f32` → `MonoF32`) this picks up
                // `From<f32> for MonoF32`.
                let ty = &unnamed.unnamed.first().unwrap().ty;
                let path = &field_paths[0];
                quote! {
                    ::core::convert::Into::<#acc_ty>::into(
                        <#ty as #path>::scale(&self.0, scalar)
                    )
                }
            } else {
                let field_scales = unnamed
                    .unnamed
                    .iter()
                    .zip(field_paths.iter())
                    .enumerate()
                    .map(|(i, (f, path))| {
                        let idx = syn::Index::from(i);
                        let ty = &f.ty;
                        quote! { <#ty as #path>::scale(&self.#idx, scalar).into() }
                    });
                quote! {
                    #acc_ty(#(#field_scales),*)
                }
            }
        }
        Fields::Unit => unreachable!("unit structs rejected earlier"),
    };

    let scale_add_body = match fields {
        Fields::Named(named) => {
            // two-sided `.into()` at each field boundary.
            // The outer `addend` field value has the outer accumulator's
            // field type (e.g. `f32` for `RgbF32.r`); it must be converted
            // into the inner's accumulator (e.g. `MonoF32` for
            // `Mono<BITS>::Accumulator`) before delegating. The inner's
            // scaled result is then converted back to the outer field's
            // type on return.
            let field_scale_adds = named.named.iter().zip(field_paths.iter()).map(|(f, path)| {
                let ident = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                quote! {
                    #ident: <#ty as #path>::scale_add(
                        &self.#ident,
                        scalar,
                        addend.#ident.into(),
                    ).into()
                }
            });
            quote! {
                #acc_ty {
                    #(#field_scale_adds),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            if unnamed.unnamed.len() == 1 {
                // Single-field tuple struct: unwrap the outer `addend`
                // (declared `#acc_ty`) into the inner's accumulator via
                // `Into`, call the inner `scale_add`, then re-wrap the
                // result. For matching types this is the identity blanket;
                // for Phase-B types (`MonoF32` ↔ `f32`) this picks up the
                // mutual `From` impls.
                let ty = &unnamed.unnamed.first().unwrap().ty;
                let path = &field_paths[0];
                quote! {
                    ::core::convert::Into::<#acc_ty>::into(
                        <#ty as #path>::scale_add(
                            &self.0,
                            scalar,
                            ::core::convert::Into::into(addend),
                        )
                    )
                }
            } else {
                let field_scale_adds = unnamed
                    .unnamed
                    .iter()
                    .zip(field_paths.iter())
                    .enumerate()
                    .map(|(i, (f, path))| {
                        let idx = syn::Index::from(i);
                        let ty = &f.ty;
                        quote! {
                            <#ty as #path>::scale_add(
                                &self.#idx,
                                scalar,
                                addend.#idx.into(),
                            ).into()
                        }
                    });
                quote! {
                    #acc_ty(#(#field_scale_adds),*)
                }
            }
        }
        Fields::Unit => unreachable!("unit structs rejected earlier"),
    };

    Ok(quote! {
        impl #impl_generics ::fovea::pixel::LinearPixel<f32> for #name #ty_generics #where_clause {
            type Accumulator = #acc_ty;
            #[inline(always)]
            fn to_accumulator(&self) -> Self::Accumulator {
                #to_acc_body
            }
            #[inline(always)]
            fn scale(&self, scalar: f32) -> Self::Accumulator {
                #scale_body
            }
            #[inline(always)]
            fn scale_add(&self, scalar: f32, addend: Self::Accumulator) -> Self::Accumulator {
                #scale_add_body
            }
            #[inline(always)]
            fn uniform(scalar: f32) -> Self::Accumulator {
                #uniform_body
            }
        }
    })
}

/// Generates `impl FromLinear<Acc> for T`.
///
/// For named structs: constructs Self from the accumulator by calling
/// `FromLinear::from_linear` on each matching field.
///
/// For single-field tuple structs: wraps `FromLinear::from_linear(acc)`.
///
/// For multi-field tuple structs: constructs Self from the accumulator fields by index.
fn generate_from_linear(
    name: &syn::Ident,
    fields: &Fields,
    acc_ty: &TokenStream,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
) -> Result<TokenStream> {
    let body = match fields {
        Fields::Named(named) => {
            let field_converts = named.named.iter().map(|f| {
                let ident = f.ident.as_ref().unwrap();
                quote! { #ident: ::fovea::pixel::FromLinear::from_linear(acc.#ident) }
            });
            quote! {
                #name {
                    #(#field_converts),*
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            if unnamed.unnamed.len() == 1 {
                quote! {
                    #name(::fovea::pixel::FromLinear::from_linear(acc))
                }
            } else {
                let field_converts = unnamed.unnamed.iter().enumerate().map(|(i, _)| {
                    let idx = syn::Index::from(i);
                    quote! { ::fovea::pixel::FromLinear::from_linear(acc.#idx) }
                });
                quote! {
                    #name(#(#field_converts),*)
                }
            }
        }
        Fields::Unit => unreachable!("unit structs rejected earlier"),
    };

    Ok(quote! {
        impl #impl_generics ::fovea::pixel::FromLinear<#acc_ty> for #name #ty_generics #where_clause {
            #[inline(always)]
            fn from_linear(acc: #acc_ty) -> Self {
                #body
            }
        }
    })
}

/// Validates that the derive input is a struct (not enum or union) with at least one field.
fn validate_struct(input: &DeriveInput) -> Result<&Fields> {
    match &input.data {
        Data::Struct(data) => {
            if data.fields.is_empty() {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "LinearPixel cannot be derived for structs with no fields",
                ));
            }
            Ok(&data.fields)
        }
        Data::Enum(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "LinearPixel can only be derived for structs, not enums",
        )),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "LinearPixel can only be derived for structs, not unions",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::DeriveInput;

    // -----------------------------------------------------------------------
    // validate_struct tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_struct_named_fields() {
        let input: DeriveInput = syn::parse_quote! {
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
            struct TestPixel(u8);
        };
        assert!(validate_struct(&input).is_ok());
    }

    #[test]
    fn test_validate_struct_rejects_enum() {
        let input: DeriveInput = syn::parse_quote! {
            enum BadPixel { Red }
        };
        let result = validate_struct(&input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not enums"));
    }

    #[test]
    fn test_validate_struct_rejects_union() {
        let input: DeriveInput = syn::parse_quote! {
            union BadPixel { a: u8, b: u16 }
        };
        let result = validate_struct(&input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not unions"));
    }

    #[test]
    fn test_validate_struct_rejects_empty() {
        let input: DeriveInput = syn::parse_quote! {
            struct Empty {}
        };
        let result = validate_struct(&input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("no fields"));
    }

    #[test]
    fn test_validate_struct_rejects_unit() {
        let input: DeriveInput = syn::parse_quote! {
            struct Unit;
        };
        let result = validate_struct(&input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("no fields"));
    }

    // -----------------------------------------------------------------------
    // parse_linear_attr tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_linear_attr_valid() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = RgbF32)]
            struct Rgb8 { r: u8, g: u8, b: u8 }
        };
        let attr = parse_linear_attr(&input.attrs).unwrap();
        // just verify it parsed without error; type is RgbF32
        assert!(!is_self_accumulator(&attr.accumulator));
    }

    #[test]
    fn test_parse_linear_attr_self() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct RgbF32 { r: f32, g: f32, b: f32 }
        };
        let attr = parse_linear_attr(&input.attrs).unwrap();
        assert!(is_self_accumulator(&attr.accumulator));
    }

    #[test]
    fn test_parse_linear_attr_missing() {
        let input: DeriveInput = syn::parse_quote! {
            struct Rgb8 { r: u8, g: u8, b: u8 }
        };
        let result = parse_linear_attr(&input.attrs);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("requires #[linear(accumulator")
        );
    }

    #[test]
    fn test_parse_linear_attr_unknown_key() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(foo = bar)]
            struct Rgb8 { r: u8 }
        };
        let result = parse_linear_attr(&input.attrs);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("unknown key"));
    }

    #[test]
    fn test_parse_linear_attr_duplicate_accumulator() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = A, accumulator = B)]
            struct Rgb8 { r: u8 }
        };
        let result = parse_linear_attr(&input.attrs);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("duplicate"));
    }

    #[test]
    fn test_parse_linear_attr_with_path_type() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = crate::pixel::RgbF32)]
            struct Rgb8 { r: u8, g: u8, b: u8 }
        };
        let attr = parse_linear_attr(&input.attrs).unwrap();
        assert!(!is_self_accumulator(&attr.accumulator));
    }

    // -----------------------------------------------------------------------
    // is_self_accumulator tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_self_accumulator_true() {
        let ty: Type = syn::parse_quote!(Self);
        assert!(is_self_accumulator(&ty));
    }

    #[test]
    fn test_is_self_accumulator_false_for_concrete() {
        let ty: Type = syn::parse_quote!(RgbF32);
        assert!(!is_self_accumulator(&ty));
    }

    #[test]
    fn test_is_self_accumulator_false_for_path() {
        let ty: Type = syn::parse_quote!(crate::RgbF32);
        assert!(!is_self_accumulator(&ty));
    }

    // -----------------------------------------------------------------------
    // Full derive output tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_named_struct_non_self_accumulator() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = RgbF32)]
            struct Rgb8 {
                r: u8,
                g: u8,
                b: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Should contain Add impl
        assert!(output.contains("impl :: std :: ops :: Add for Rgb8"));
        // Should contain Sub impl
        assert!(output.contains("impl :: std :: ops :: Sub for Rgb8"));
        // Should contain LinearPixel impl
        assert!(output.contains("LinearPixel < f32 > for Rgb8"));
        assert!(output.contains("type Accumulator = RgbF32"));
        // Should contain FromLinear impl (non-Self accumulator)
        assert!(output.contains("FromLinear < RgbF32 > for Rgb8"));
        // Should contain LinearSpace impl
        assert!(output.contains("LinearSpace for Rgb8"));
    }

    #[test]
    fn test_derive_named_struct_self_accumulator() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct RgbF32 {
                r: f32,
                g: f32,
                b: f32,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Should contain Add impl
        assert!(output.contains("impl :: std :: ops :: Add for RgbF32"));
        // Should contain Sub impl
        assert!(output.contains("impl :: std :: ops :: Sub for RgbF32"));
        // Should contain LinearPixel impl with concrete type (not Self)
        assert!(output.contains("LinearPixel < f32 > for RgbF32"));
        assert!(output.contains("type Accumulator = RgbF32"));
        // Should NOT contain FromLinear impl (Self accumulator → blanket covers it)
        assert!(!output.contains("FromLinear"));
        // Should contain LinearSpace
        assert!(output.contains("LinearSpace for RgbF32"));
    }

    #[test]
    fn test_derive_tuple_struct_single_field() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = f32)]
            struct Mono8(u8);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Add impl
        assert!(output.contains("impl :: std :: ops :: Add for Mono8"));
        // Sub impl
        assert!(output.contains("impl :: std :: ops :: Sub for Mono8"));
        // LinearPixel
        assert!(output.contains("LinearPixel < f32 > for Mono8"));
        assert!(output.contains("type Accumulator = f32"));
        // scale delegates to self.0
        assert!(output.contains("scale (& self . 0 , scalar)"));
        // FromLinear wraps in Mono8(...)
        assert!(output.contains("FromLinear < f32 > for Mono8"));
    }

    #[test]
    fn test_derive_with_generics() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = RgbF32)]
            struct Rgb<const BITS: usize> {
                r: Mono<BITS>,
                g: Mono<BITS>,
                b: Mono<BITS>,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Should have generic params in impl
        assert!(output.contains("const BITS"));
        // Should reference Rgb with generic
        assert!(output.contains("Rgb <"));
    }

    #[test]
    fn test_derive_rejects_enum() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = f32)]
            enum Bad { A }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not enums"));
    }

    #[test]
    fn test_derive_rejects_union() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = f32)]
            union Bad { a: u8, b: u16 }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not unions"));
    }

    #[test]
    fn test_derive_rejects_empty_struct() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = f32)]
            struct Empty {}
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("no fields"));
    }

    #[test]
    fn test_derive_rejects_missing_attribute() {
        let input: DeriveInput = syn::parse_quote! {
            struct Rgb8 { r: u8, g: u8, b: u8 }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("requires #[linear")
        );
    }

    #[test]
    fn test_derive_four_field_named_struct() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = RgbaF32)]
            struct Rgba8 {
                r: u8,
                g: u8,
                b: u8,
                a: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // All four fields in Add
        assert!(output.contains("self . r + other . r"));
        assert!(output.contains("self . a + other . a"));
        // All four fields in Sub
        assert!(output.contains("self . r - other . r"));
        assert!(output.contains("self . a - other . a"));
        // All four fields in scale
        assert!(output.contains("scale (& self . r , scalar)"));
        assert!(output.contains("scale (& self . a , scalar)"));
        // All four fields in FromLinear
        assert!(output.contains("from_linear (acc . r)"));
        assert!(output.contains("from_linear (acc . a)"));
    }

    #[test]
    fn test_derive_non_repr_attr_ignored() {
        // Other attributes besides #[linear(...)] should be ignored by the parser
        let input: DeriveInput = syn::parse_quote! {
            #[repr(C)]
            #[derive(Clone, Copy)]
            #[linear(accumulator = RgbF32)]
            struct Rgb8 {
                r: u8,
                g: u8,
                b: u8,
            }
        };
        let result = derive(input);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // generate_add tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_add_named() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel { x: u8, y: u8 }
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let tokens = generate_add(&input.ident, fields, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("x : self . x + other . x"));
        assert!(output.contains("y : self . y + other . y"));
    }

    #[test]
    fn test_generate_add_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let tokens = generate_add(&input.ident, fields, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("self . 0 + other . 0"));
        assert!(output.contains("self . 1 + other . 1"));
    }

    // -----------------------------------------------------------------------
    // generate_sub tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_sub_named() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel { x: u8, y: u8 }
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let tokens = generate_sub(&input.ident, fields, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("x : self . x - other . x"));
        assert!(output.contains("y : self . y - other . y"));
    }

    #[test]
    fn test_generate_sub_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let tokens = generate_sub(&input.ident, fields, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("self . 0 - other . 0"));
        assert!(output.contains("self . 1 - other . 1"));
    }

    #[test]
    fn test_generate_sub_unit_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct Unit;
        };
        let Data::Struct(s) = &input.data else {
            panic!("expected struct");
        };
        let fields = &s.fields;
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let result = generate_sub(&input.ident, fields, &impl_g, &ty_g, wh);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("unit structs"));
    }

    // -----------------------------------------------------------------------
    // derive-level Sub integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_generates_sub_named() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct Pixel { x: f32, y: f32 }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains(":: std :: ops :: Sub"));
        assert!(output.contains("x : self . x - other . x"));
        assert!(output.contains("y : self . y - other . y"));
    }

    #[test]
    fn test_derive_generates_sub_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct Pixel(f32, f32);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains(":: std :: ops :: Sub"));
        assert!(output.contains("self . 0 - other . 0"));
        assert!(output.contains("self . 1 - other . 1"));
    }

    // -----------------------------------------------------------------------
    // generate_linear_pixel tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_linear_pixel_named() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel { x: u8, y: u8 }
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccType);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("type Accumulator = AccType"));
        assert!(output.contains("scale (& self . x , scalar)"));
        assert!(output.contains("scale (& self . y , scalar)"));
    }

    #[test]
    fn test_generate_linear_pixel_single_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(f32);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("type Accumulator = f32"));
        // Single-field: delegates directly, no tuple construction
        assert!(output.contains("scale (& self . 0 , scalar)"));
    }

    #[test]
    fn test_generate_linear_pixel_multi_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccTuple);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("scale (& self . 0 , scalar)"));
        assert!(output.contains("scale (& self . 1 , scalar)"));
    }

    // -----------------------------------------------------------------------
    // generate_from_linear tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_from_linear_named() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel { x: u8, y: u8 }
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccType);
        let tokens = generate_from_linear(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("FromLinear < AccType > for Pixel"));
        assert!(output.contains("from_linear (acc . x)"));
        assert!(output.contains("from_linear (acc . y)"));
    }

    #[test]
    fn test_generate_from_linear_single_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(f32);
        let tokens = generate_from_linear(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("FromLinear < f32 > for Pixel"));
        // Should wrap: Pixel(FromLinear::from_linear(acc))
        assert!(output.contains("Pixel ("));
        assert!(output.contains("from_linear (acc)"));
    }

    #[test]
    fn test_generate_from_linear_multi_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccTuple);
        let tokens = generate_from_linear(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("from_linear (acc . 0)"));
        assert!(output.contains("from_linear (acc . 1)"));
    }

    // -----------------------------------------------------------------------
    // Malformed #[linear(...)] attribute parsing — exercises `?` error paths
    // in LinearKV::parse (lines 25-27) and parse_args_with (line 41)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_linear_attr_malformed_not_ident() {
        // `42` is not a valid Ident → key parse fails (line 25 `?`)
        let input: DeriveInput = syn::parse_quote! {
            #[linear(42)]
            struct Rgb8 { r: u8 }
        };
        let result = parse_linear_attr(&input.attrs);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_linear_attr_malformed_missing_eq() {
        // `accumulator Type` — missing `=` → _eq parse fails (line 26 `?`)
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator Type)]
            struct Rgb8 { r: u8 }
        };
        let result = parse_linear_attr(&input.attrs);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_linear_attr_malformed_missing_value() {
        // `accumulator =` with no type → value parse fails (line 27 `?`)
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator =)]
            struct Rgb8 { r: u8 }
        };
        let result = parse_linear_attr(&input.attrs);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // generate_add / generate_linear_pixel / generate_from_linear with Unit
    // fields — exercises the Fields::Unit branches
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_add_unit_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct Unit;
        };
        let Data::Struct(s) = &input.data else {
            panic!("expected struct");
        };
        let fields = &s.fields;
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let result = generate_add(&input.ident, fields, &impl_g, &ty_g, wh);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("unit structs"));
    }

    // -----------------------------------------------------------------------
    // generate_mul tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_mul_named() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel { x: u8, y: u8 }
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let tokens = generate_mul(&input.ident, fields, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("x : self . x * other . x"));
        assert!(output.contains("y : self . y * other . y"));
    }

    #[test]
    fn test_generate_mul_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let tokens = generate_mul(&input.ident, fields, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("self . 0 * other . 0"));
        assert!(output.contains("self . 1 * other . 1"));
    }

    #[test]
    fn test_generate_mul_unit_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct Unit;
        };
        let Data::Struct(s) = &input.data else {
            panic!("expected struct");
        };
        let fields = &s.fields;
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let result = generate_mul(&input.ident, fields, &impl_g, &ty_g, wh);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("unit structs"));
    }

    // -----------------------------------------------------------------------
    // derive-level Mul integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_generates_mul_named() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct Pixel { x: f32, y: f32 }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains(":: std :: ops :: Mul"));
        assert!(output.contains("x : self . x * other . x"));
        assert!(output.contains("y : self . y * other . y"));
    }

    #[test]
    fn test_derive_generates_mul_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct Pixel(f32, f32);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains(":: std :: ops :: Mul"));
        assert!(output.contains("self . 0 * other . 0"));
        assert!(output.contains("self . 1 * other . 1"));
    }

    // -----------------------------------------------------------------------
    // generate_linear_pixel scale_add tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_linear_pixel_named_scale_add() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel { x: u8, y: u8 }
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccType);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        // addend field is re-wrapped via `.into()`
        // before forwarding; the returned inner accumulator is
        // re-wrapped via `.into()` back into the outer field type.
        assert!(
            output.contains("scale_add (& self . x , scalar , addend . x . into () ,) . into ()")
        );
        assert!(
            output.contains("scale_add (& self . y , scalar , addend . y . into () ,) . into ()")
        );
    }

    #[test]
    fn test_generate_linear_pixel_single_tuple_scale_add() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(f32);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        // Single-field (Phase B): addend is converted via `Into` into the
        // inner's accumulator type before being forwarded. The `scale_add`
        // call site therefore reads `(& self . 0 , scalar , :: core ::
        // convert :: Into :: into (addend) ,)` (note the trailing comma
        // from the `quote!` invocation).
        assert!(output.contains(
            "scale_add (& self . 0 , scalar , :: core :: convert :: Into :: into (addend) ,)"
        ));
    }

    #[test]
    fn test_generate_linear_pixel_multi_tuple_scale_add() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccTuple);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        // addend index is re-wrapped via `.into()`.
        assert!(
            output.contains("scale_add (& self . 0 , scalar , addend . 0 . into () ,) . into ()")
        );
        assert!(
            output.contains("scale_add (& self . 1 , scalar , addend . 1 . into () ,) . into ()")
        );
    }

    // -----------------------------------------------------------------------
    // derive-level scale_add integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_generates_scale_add_named() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = RgbF32)]
            struct Rgb8 { r: u8, g: u8, b: u8 }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // `.into()` wraps on both the addend field read
        // and the returned accumulator value.
        assert!(
            output.contains("scale_add (& self . r , scalar , addend . r . into () ,) . into ()")
        );
        assert!(
            output.contains("scale_add (& self . g , scalar , addend . g . into () ,) . into ()")
        );
        assert!(
            output.contains("scale_add (& self . b , scalar , addend . b . into () ,) . into ()")
        );
    }

    #[test]
    fn test_derive_generates_scale_add_self_accumulator() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct RgbF32 { r: f32, g: f32, b: f32 }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // identity-path `.into()` (no-op at runtime via
        // the blanket `From<T> for T` impl) still appears in the token
        // stream.
        assert!(
            output.contains("scale_add (& self . r , scalar , addend . r . into () ,) . into ()")
        );
        assert!(
            output.contains("scale_add (& self . g , scalar , addend . g . into () ,) . into ()")
        );
        assert!(
            output.contains("scale_add (& self . b , scalar , addend . b . into () ,) . into ()")
        );
    }

    #[test]
    fn test_derive_generates_scale_add_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = f32)]
            struct Mono8(u8);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Phase B: addend is now re-wrapped via `Into::into` before being
        // forwarded to the inner field's `scale_add`.
        assert!(output.contains(
            "scale_add (& self . 0 , scalar , :: core :: convert :: Into :: into (addend) ,)"
        ));
    }

    // -----------------------------------------------------------------------
    // generate_linear_pixel uniform tests
    //
    // `uniform(scalar) -> Accumulator` broadcasts the scalar into every
    // channel of the accumulator. For named structs the derive emits a
    // struct literal whose every field calls the field type's
    // `LinearPixel::uniform`; for tuple structs the same pattern is
    // applied by index; for single-field tuple structs the single field's
    // `uniform` is returned directly.
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_linear_pixel_named_uniform() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel { x: u8, y: u8 }
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccType);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("fn uniform (scalar : f32)"));
        // Channel fields bind on LinearChannel, not LinearPixel.
        // NOTE: `> >` (with space) reflects quote!'s tokenization when the
        // trait path is interpolated as a nested TokenStream.
        assert!(output.contains(
            "x : < u8 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
        ));
        assert!(output.contains(
            "y : < u8 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
        ));
    }

    #[test]
    fn test_generate_linear_pixel_single_tuple_uniform() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(f32);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        // Single-field tuple: return the field's uniform directly, no wrapper.
        // Channel fields bind on LinearChannel.
        assert!(
            output.contains(
                "< u8 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
            )
        );
    }

    #[test]
    fn test_generate_linear_pixel_multi_tuple_uniform() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let (impl_g, ty_g, wh) = input.generics.split_for_impl();
        let acc = quote!(AccTuple);
        let tokens = generate_linear_pixel(&input.ident, fields, &acc, &impl_g, &ty_g, wh).unwrap();
        let output = tokens.to_string();
        // Channel fields bind on LinearChannel.
        assert!(
            output.contains(
                "< u8 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
            )
        );
        assert!(
            output.contains(
                "< u16 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
            )
        );
    }

    #[test]
    fn test_derive_generates_uniform_named() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = Self)]
            struct RgbF32 { r: f32, g: f32, b: f32 }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("fn uniform (scalar : f32)"));
        // `f32` is a channel (implements `LinearChannel<f32>`),
        // so the derive emits the channel-qualified path on each field.
        assert!(output.contains(
            "r : < f32 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
        ));
        assert!(output.contains(
            "g : < f32 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
        ));
        assert!(output.contains(
            "b : < f32 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
        ));
    }

    #[test]
    fn test_derive_generates_uniform_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = f32)]
            struct Mono8(u8);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Single-field tuple: pass-through.
        assert!(output.contains("fn uniform (scalar : f32)"));
        // Channel fields bind on LinearChannel.
        assert!(
            output.contains(
                "< u8 as :: fovea :: pixel :: LinearChannel < f32 > > :: uniform (scalar)"
            )
        );
    }

    // -----------------------------------------------------------------------
    // Dual-probe tests — verify the derive emits LinearChannel on
    // channel fields and LinearPixel on `#[linear(nested)]` fields, and that
    // the two compose in a mixed struct.
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_channel_field_probe() {
        // Channel fields (no #[linear(nested)]) should emit LinearChannel
        // and not LinearPixel on their per-field accesses. The top-level
        // `impl LinearPixel<f32> for Rgb8` is unaffected.
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = RgbF32)]
            struct Rgb8 {
                r: Saturating<u8>,
                g: Saturating<u8>,
                b: Saturating<u8>,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Top-level impl block still binds on LinearPixel<f32>.
        assert!(output.contains("LinearPixel < f32 > for Rgb8"));
        // Field accesses use LinearChannel — check one canonical method.
        assert!(output.contains(
            "r : < Saturating < u8 > as :: fovea :: pixel :: LinearChannel < f32 > > :: to_accumulator (& self . r)"
        ));
        // And the field-level LinearPixel qualification is absent.
        assert!(!output.contains(
            "< Saturating < u8 > as :: fovea :: pixel :: LinearPixel < f32 > > :: to_accumulator"
        ));
    }

    #[test]
    fn test_derive_nested_pixel_field_probe() {
        // `#[linear(nested)]` fields fall back to LinearPixel.
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = RgbF32)]
            struct Rgb<const BITS: usize> {
                #[linear(nested)] r: Mono<BITS>,
                #[linear(nested)] g: Mono<BITS>,
                #[linear(nested)] b: Mono<BITS>,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Per-field accesses use LinearPixel (nested pixel composition).
        assert!(output.contains(
            "r : < Mono < BITS > as :: fovea :: pixel :: LinearPixel < f32 > > :: to_accumulator (& self . r)"
        ));
        // And the channel path is not used for these fields.
        assert!(!output.contains(
            "< Mono < BITS > as :: fovea :: pixel :: LinearChannel < f32 > > :: to_accumulator"
        ));
    }

    #[test]
    fn test_derive_mixed_probes_legal() {
        // Mixed is legal: each field's probe is emitted independently.
        // The macro does not reject nor specially handle the mix.
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = MixedAcc)]
            struct Mixed {
                #[linear(nested)] p: Mono<10>,
                c: Saturating<u8>,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Nested pixel field → LinearPixel.
        assert!(output.contains(
            "p : < Mono < 10 > as :: fovea :: pixel :: LinearPixel < f32 > > :: to_accumulator (& self . p)"
        ));
        // Channel field → LinearChannel.
        assert!(output.contains(
            "c : < Saturating < u8 > as :: fovea :: pixel :: LinearChannel < f32 > > :: to_accumulator (& self . c)"
        ));
    }

    #[test]
    fn test_derive_rejects_unknown_field_attr_key() {
        // Unknown per-field key produces a diagnostic-quality error.
        let input: DeriveInput = syn::parse_quote! {
            #[linear(accumulator = A)]
            struct Bad {
                #[linear(frobnicate)] x: u8,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(
            msg.contains("unknown key") && msg.contains("frobnicate"),
            "expected diagnostic mentioning the unknown key, got: {msg}"
        );
    }

    #[test]
    fn test_field_trait_path_default_is_linear_channel() {
        let input: DeriveInput = syn::parse_quote! {
            struct P { x: u8 }
        };
        let Data::Struct(s) = &input.data else {
            panic!("expected struct");
        };
        let Fields::Named(n) = &s.fields else {
            panic!("expected named");
        };
        let field = n.named.first().unwrap();
        let path = field_trait_path(field).unwrap();
        let s = path.to_string();
        assert!(s.contains("LinearChannel"));
        assert!(!s.contains("LinearPixel"));
    }

    #[test]
    fn test_field_trait_path_nested_is_linear_pixel() {
        let input: DeriveInput = syn::parse_quote! {
            struct P { #[linear(nested)] x: Mono<10> }
        };
        let Data::Struct(s) = &input.data else {
            panic!("expected struct");
        };
        let Fields::Named(n) = &s.fields else {
            panic!("expected named");
        };
        let field = n.named.first().unwrap();
        let path = field_trait_path(field).unwrap();
        let s = path.to_string();
        assert!(s.contains("LinearPixel"));
        assert!(!s.contains("LinearChannel"));
    }
}
