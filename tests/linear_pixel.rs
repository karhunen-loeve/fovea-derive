#[test]
fn linear_pixel_compile_tests() {
    let t = trybuild::TestCases::new();
    // Compile-fail tests
    t.compile_fail("tests/ui/linear_pixel_enum.rs");
    t.compile_fail("tests/ui/linear_pixel_union.rs");
    t.compile_fail("tests/ui/linear_pixel_no_fields.rs");
    t.compile_fail("tests/ui/linear_pixel_missing_accumulator.rs");
    // Per-field `#[linear(...)]` only accepts `nested`.
    t.compile_fail("tests/ui/linear_pixel_unknown_field_attr.rs");
}
