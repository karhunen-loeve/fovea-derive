use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Expr, Field, Fields, Meta, Result};

/// Strategy for generating the zero value of a single field.
///
/// Manual `Debug` impl is provided because `syn::Expr` does not implement `Debug`
/// without the `extra-traits` feature.
enum ZeroStrategy {
    /// Default: use `<T as ZeroablePixel>::zero()`
    Trait,
    /// Use `<T as Default>::default()`
    Default,
    /// Use a user-provided expression verbatim.
    Expr(Expr),
}

impl std::fmt::Debug for ZeroStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZeroStrategy::Trait => write!(f, "ZeroStrategy::Trait"),
            ZeroStrategy::Default => write!(f, "ZeroStrategy::Default"),
            ZeroStrategy::Expr(expr) => {
                write!(f, "ZeroStrategy::Expr({})", quote!(#expr))
            }
        }
    }
}

/// Main entry point for the ZeroablePixel derive macro.
///
/// # Steps:
/// 1. Validate that the input is a struct (not enum or union)
/// 2. Validate that the struct has at least one field
/// 3. Parse `#[zero(...)]` attributes on each field to determine the strategy
/// 4. Generate `zero()` using the per-field strategy
/// 5. Emit `impl ZeroablePixel for StructName { ... }`
///
/// # Generated Code
///
/// For named structs (no attributes):
/// ```ignore
/// impl ::fovea::pixel::ZeroablePixel for Rgb8 {
///     fn zero() -> Self {
///         Rgb8 {
///             r: <Saturating<u8> as ::fovea::pixel::ZeroablePixel>::zero(),
///             g: <Saturating<u8> as ::fovea::pixel::ZeroablePixel>::zero(),
///             b: <Saturating<u8> as ::fovea::pixel::ZeroablePixel>::zero(),
///         }
///     }
/// }
/// ```
///
/// With `#[zero(default)]`:
/// ```ignore
/// metadata: <SomeType as ::core::default::Default>::default(),
/// ```
///
/// With `#[zero(MyTag::new(42))]`:
/// ```ignore
/// tag: MyTag::new(42),
/// ```
///
/// For tuple structs:
/// ```ignore
/// impl ::fovea::pixel::ZeroablePixel for Mono8 {
///     fn zero() -> Self {
///         Mono8(
///             <Saturating<u8> as ::fovea::pixel::ZeroablePixel>::zero(),
///         )
///     }
/// }
/// ```
pub(crate) fn derive(input: DeriveInput) -> Result<TokenStream> {
    // Step 1 - Validate struct
    let fields = validate_struct(&input)?;

    // Step 2 - Validate at least one field
    validate_fields(&input, fields)?;

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Step 3 - Generate the zero() body (includes attribute parsing)
    let zero_body = generate_zero_body(name, fields)?;

    // Step 4 - Generate the impl block
    Ok(quote! {
        impl #impl_generics ::fovea::pixel::ZeroablePixel for #name #ty_generics #where_clause {
            fn zero() -> Self {
                #zero_body
            }
        }

        impl #impl_generics ::core::default::Default for #name #ty_generics #where_clause {
            fn default() -> Self {
                <Self as ::fovea::pixel::ZeroablePixel>::zero()
            }
        }
    })
}

/// Parse `#[zero(...)]` attributes on a field and return the appropriate `ZeroStrategy`.
///
/// Supported forms:
/// - No `#[zero(...)]` attribute → `ZeroStrategy::Trait`
/// - `#[zero(default)]` → `ZeroStrategy::Default`
/// - `#[zero(<expr>)]` → `ZeroStrategy::Expr(expr)`
///
/// Rejected forms (with helpful errors):
/// - Bare `#[zero]` (path form) → error
/// - Empty `#[zero()]` → error
/// - `#[zero = ...]` (name-value form) → error with guidance
/// - Duplicate `#[zero(...)]` → error
fn parse_zero_attr(field: &Field) -> Result<ZeroStrategy> {
    let mut strategy: Option<ZeroStrategy> = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("zero") {
            continue;
        }

        // Reject duplicates
        if strategy.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate #[zero(...)] attribute; only one is allowed per field",
            ));
        }

        match &attr.meta {
            // `#[zero(...)]` — the list form we support
            Meta::List(list) => {
                let tokens = &list.tokens;
                if tokens.is_empty() {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "empty #[zero()] is not allowed; use #[zero(default)] or #[zero(<expr>)]",
                    ));
                }

                // Check if it's the keyword `default`
                let content = tokens.to_string();
                if content == "default" {
                    strategy = Some(ZeroStrategy::Default);
                } else {
                    // Parse as an arbitrary expression
                    let expr: Expr = syn::parse2(tokens.clone())?;
                    strategy = Some(ZeroStrategy::Expr(expr));
                }
            }
            // `#[zero = ...]` — name-value form, rejected with guidance
            Meta::NameValue(_) => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "#[zero = ...] is not supported; use #[zero(default)] or #[zero(<expr>)] instead",
                ));
            }
            // `#[zero]` — bare path form, rejected
            Meta::Path(_) => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "bare #[zero] is not allowed; use #[zero(default)] or #[zero(<expr>)]",
                ));
            }
        }
    }

    Ok(strategy.unwrap_or(ZeroStrategy::Trait))
}

/// Generate the token stream for a single field's zero initialization.
fn generate_field_init(field: &Field, strategy: &ZeroStrategy) -> TokenStream {
    let ty = &field.ty;
    match strategy {
        ZeroStrategy::Trait => {
            quote! { <#ty as ::fovea::pixel::ZeroablePixel>::zero() }
        }
        ZeroStrategy::Default => {
            quote! { <#ty as ::core::default::Default>::default() }
        }
        ZeroStrategy::Expr(expr) => {
            quote! { #expr }
        }
    }
}

/// Generates the body of `zero()` depending on field style.
fn generate_zero_body(name: &syn::Ident, fields: &Fields) -> Result<TokenStream> {
    match fields {
        Fields::Named(named) => {
            let mut field_inits = Vec::new();
            for field in &named.named {
                let strategy = parse_zero_attr(field)?;
                let field_name = field.ident.as_ref().expect("Named field must have ident");
                let init = generate_field_init(field, &strategy);
                field_inits.push(quote! {
                    #field_name: #init
                });
            }
            Ok(quote! {
                #name {
                    #(#field_inits),*
                }
            })
        }
        Fields::Unnamed(unnamed) => {
            let mut field_inits = Vec::new();
            for field in &unnamed.unnamed {
                let strategy = parse_zero_attr(field)?;
                let init = generate_field_init(field, &strategy);
                field_inits.push(init);
            }
            Ok(quote! {
                #name(#(#field_inits),*)
            })
        }
        Fields::Unit => {
            // Should not reach here due to validate_fields, but handle gracefully
            Ok(quote! { #name })
        }
    }
}

/// Validates that the input is a struct.
/// Returns the fields if valid.
fn validate_struct(input: &DeriveInput) -> Result<&Fields> {
    match &input.data {
        Data::Struct(data) => Ok(&data.fields),
        Data::Enum(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "ZeroablePixel can only be derived for structs, not enums",
        )),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "ZeroablePixel can only be derived for structs, not unions",
        )),
    }
}

/// Validates that the struct has at least one field.
fn validate_fields(input: &DeriveInput, fields: &Fields) -> Result<()> {
    if fields.is_empty() {
        Err(syn::Error::new_spanned(
            &input.ident,
            "ZeroablePixel requires at least one field",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::DeriveInput;

    // -----------------------------------------------------------------------
    // Validation tests
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
    fn test_validate_fields_non_empty() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel {
                r: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        assert!(validate_fields(&input, fields).is_ok());
    }

    #[test]
    fn test_validate_fields_empty() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel {}
        };
        let fields = validate_struct(&input).unwrap();
        let result = validate_fields(&input, fields);
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
    fn test_validate_fields_unit_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct UnitPixel;
        };
        let fields = validate_struct(&input).unwrap();
        let result = validate_fields(&input, fields);
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
    fn test_validate_fields_empty_tuple_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel();
        };
        let fields = validate_struct(&input).unwrap();
        let result = validate_fields(&input, fields);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("at least one field")
        );
    }

    // -----------------------------------------------------------------------
    // Attribute parsing tests
    // -----------------------------------------------------------------------

    /// Helper: parse a struct with one named field and return the ZeroStrategy
    fn parse_strategy_named(input: DeriveInput) -> Result<ZeroStrategy> {
        let fields = validate_struct(&input).unwrap();
        let field = fields.iter().next().expect("expected at least one field");
        parse_zero_attr(field)
    }

    #[test]
    fn test_parse_no_attribute_yields_trait() {
        let input: DeriveInput = syn::parse_quote! {
            struct P { x: u8 }
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Trait));
    }

    #[test]
    fn test_parse_zero_default_attribute() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(default)]
                x: u8,
            }
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Default));
    }

    #[test]
    fn test_parse_zero_expr_attribute_literal() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(42)]
                x: u8,
            }
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Expr(_)));
    }

    #[test]
    fn test_parse_zero_expr_attribute_function_call() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(MyType::new(10))]
                x: MyType,
            }
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Expr(_)));
    }

    #[test]
    fn test_parse_zero_expr_attribute_complex_expr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero({ let v = 3 + 4; v })]
                x: u8,
            }
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Expr(_)));
    }

    #[test]
    fn test_parse_bare_zero_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero]
                x: u8,
            }
        };
        let err = parse_strategy_named(input).unwrap_err();
        assert!(err.to_string().contains("bare #[zero]"));
    }

    #[test]
    fn test_parse_empty_zero_parens_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero()]
                x: u8,
            }
        };
        let err = parse_strategy_named(input).unwrap_err();
        assert!(err.to_string().contains("empty #[zero()]"));
    }

    #[test]
    fn test_parse_name_value_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero = "default"]
                x: u8,
            }
        };
        let err = parse_strategy_named(input).unwrap_err();
        assert!(err.to_string().contains("#[zero = ...] is not supported"));
    }

    #[test]
    fn test_parse_duplicate_zero_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(default)]
                #[zero(42)]
                x: u8,
            }
        };
        let err = parse_strategy_named(input).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn test_parse_tuple_struct_no_attr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(u8);
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Trait));
    }

    #[test]
    fn test_parse_tuple_struct_with_default_attr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero(default)] u8);
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Default));
    }

    #[test]
    fn test_parse_tuple_struct_with_expr_attr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero(99)] u8);
        };
        let strategy = parse_strategy_named(input).unwrap();
        assert!(matches!(strategy, ZeroStrategy::Expr(_)));
    }

    // -----------------------------------------------------------------------
    // Full derive output tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_named_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct Rgb8 {
                r: u8,
                g: u8,
                b: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("ZeroablePixel"));
        assert!(output.contains("fn zero"));
        assert!(output.contains("r :"));
        assert!(output.contains("g :"));
        assert!(output.contains("b :"));
    }

    #[test]
    fn test_derive_tuple_struct() {
        let input: DeriveInput = syn::parse_quote! {
            struct Mono8(u8);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("ZeroablePixel"));
        assert!(output.contains("fn zero"));
        assert!(output.contains("Mono8"));
    }

    #[test]
    fn test_derive_rejects_enum() {
        let input: DeriveInput = syn::parse_quote! {
            enum BadPixel {
                Red,
                Green,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not enums"));
    }

    #[test]
    fn test_derive_rejects_union() {
        let input: DeriveInput = syn::parse_quote! {
            union BadPixel {
                a: u8,
                b: u16,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not unions"));
    }

    #[test]
    fn test_derive_rejects_empty_struct() {
        let input: DeriveInput = syn::parse_quote! {
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
    fn test_derive_named_struct_generates_qualified_zero_calls() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel {
                x: u16,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("ZeroablePixel"));
        assert!(output.contains(":: zero"));
    }

    #[test]
    fn test_derive_tuple_struct_multi_field() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel(u8, u16, u32);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("ZeroablePixel"));
        assert!(output.contains("u8"));
        assert!(output.contains("u16"));
        assert!(output.contains("u32"));
    }

    #[test]
    fn test_derive_with_generic_type_field() {
        let input: DeriveInput = syn::parse_quote! {
            struct TestPixel {
                value: Saturating<u8>,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("Saturating"));
    }

    #[test]
    fn test_generate_zero_body_named() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel {
                r: u8,
                g: u8,
            }
        };
        let fields = validate_struct(&input).unwrap();
        let body = generate_zero_body(&input.ident, fields).unwrap();
        let output = body.to_string();
        assert!(output.contains("Pixel"));
        assert!(output.contains("r :"));
        assert!(output.contains("g :"));
    }

    #[test]
    fn test_generate_zero_body_tuple() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel(u8, u16);
        };
        let fields = validate_struct(&input).unwrap();
        let body = generate_zero_body(&input.ident, fields).unwrap();
        let output = body.to_string();
        assert!(output.contains("Pixel"));
        assert!(output.contains("u8"));
        assert!(output.contains("u16"));
    }

    #[test]
    fn test_generate_zero_body_unit() {
        let input: DeriveInput = syn::parse_quote! {
            struct Pixel;
        };
        let fields = validate_struct(&input).unwrap();
        let body = generate_zero_body(&input.ident, fields).unwrap();
        let output = body.to_string();
        assert!(output.contains("Pixel"));
    }

    // -----------------------------------------------------------------------
    // Attribute-driven codegen tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_named_with_default_attr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(default)]
                x: u8,
            }
        };
        let tokens = derive(input.clone()).unwrap();
        let output = tokens.to_string();
        // The full output contains both a ZeroablePixel impl (using Default
        // for the #[zero(default)] field) and a Default impl (delegating to
        // ZeroablePixel::zero()). Verify both are present.
        assert!(output.contains("Default"));
        assert!(output.contains("ZeroablePixel"));
        // Check the zero body specifically: the #[zero(default)] field
        // should use Default::default(), not ZeroablePixel::zero().
        let fields = validate_struct(&input).unwrap();
        let zero_body = generate_zero_body(&input.ident, fields)
            .unwrap()
            .to_string();
        assert!(!zero_body.contains("ZeroablePixel"));
        assert!(zero_body.contains("Default"));
    }

    #[test]
    fn test_derive_named_with_expr_attr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(42u8)]
                x: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("42u8"));
    }

    #[test]
    fn test_derive_named_mixed_strategies() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                r: u8,
                #[zero(default)]
                meta: SomeType,
                #[zero(MyTag::new(5))]
                tag: MyTag,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // `r` should use ZeroablePixel::zero()
        assert!(output.contains("ZeroablePixel"));
        // `meta` should use Default::default()
        assert!(output.contains("Default"));
        // `tag` should use MyTag::new(5)
        assert!(output.contains("MyTag :: new"));
    }

    #[test]
    fn test_derive_tuple_with_default_attr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero(default)] u8);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("Default"));
    }

    #[test]
    fn test_derive_tuple_with_expr_attr() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero(99)] u8);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("99"));
    }

    #[test]
    fn test_derive_tuple_mixed_strategies() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(u8, #[zero(default)] u16, #[zero(100u32)] u32);
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("ZeroablePixel"));
        assert!(output.contains("Default"));
        assert!(output.contains("100u32"));
    }

    #[test]
    fn test_derive_bare_zero_attr_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero]
                x: u8,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("bare #[zero]"));
    }

    #[test]
    fn test_derive_empty_zero_attr_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero()]
                x: u8,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("empty #[zero()]")
        );
    }

    #[test]
    fn test_derive_name_value_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero = "default"]
                x: u8,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("#[zero = ...] is not supported")
        );
    }

    #[test]
    fn test_derive_duplicate_zero_attr_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(default)]
                #[zero(42)]
                x: u8,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("duplicate"));
    }

    #[test]
    fn test_derive_expr_method_call() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(SomeType::with_value(1, 2, 3))]
                x: SomeType,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("SomeType :: with_value"));
    }

    #[test]
    fn test_derive_expr_block() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero({ let v = 1 + 2; v })]
                x: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        assert!(output.contains("let v"));
    }

    #[test]
    fn test_non_zero_attrs_ignored() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[allow(unused)]
                #[cfg(test)]
                x: u8,
            }
        };
        let tokens = derive(input).unwrap();
        let output = tokens.to_string();
        // Should still use ZeroablePixel::zero() (non-`zero` attributes are ignored)
        assert!(output.contains("ZeroablePixel"));
    }

    // -----------------------------------------------------------------------
    // Debug impl coverage
    // -----------------------------------------------------------------------

    #[test]
    fn test_debug_zero_strategy_trait() {
        let strategy = ZeroStrategy::Trait;
        let dbg = format!("{strategy:?}");
        assert_eq!(dbg, "ZeroStrategy::Trait");
    }

    #[test]
    fn test_debug_zero_strategy_default() {
        let strategy = ZeroStrategy::Default;
        let dbg = format!("{strategy:?}");
        assert_eq!(dbg, "ZeroStrategy::Default");
    }

    #[test]
    fn test_debug_zero_strategy_expr() {
        let expr: Expr = syn::parse_quote! { 42u8 };
        let strategy = ZeroStrategy::Expr(expr);
        let dbg = format!("{strategy:?}");
        assert!(dbg.starts_with("ZeroStrategy::Expr("));
        assert!(dbg.contains("42"));
    }

    // -----------------------------------------------------------------------
    // Expression parse error coverage (covers ? on syn::parse2 in parse_zero_attr)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_zero_invalid_expression_rejected() {
        // `in` is a keyword that cannot start an expression, so syn::parse2 will fail
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(in)]
                x: u8,
            }
        };
        let result = parse_strategy_named(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_named_invalid_expression_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P {
                #[zero(in)]
                x: u8,
            }
        };
        let result = derive(input);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Tuple struct attribute error coverage (covers ? on parse_zero_attr in unnamed branch)
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_tuple_bare_zero_attr_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero] u8);
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("bare #[zero]"));
    }

    #[test]
    fn test_derive_tuple_empty_zero_attr_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero()] u8);
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("empty #[zero()]")
        );
    }

    #[test]
    fn test_derive_tuple_duplicate_zero_attr_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero(default)] #[zero(42)] u8);
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("duplicate"));
    }

    #[test]
    fn test_derive_tuple_name_value_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero = "default"] u8);
        };
        let result = derive(input);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("#[zero = ...] is not supported")
        );
    }

    #[test]
    fn test_derive_tuple_invalid_expression_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            struct P(#[zero(in)] u8);
        };
        let result = derive(input);
        assert!(result.is_err());
    }
}
