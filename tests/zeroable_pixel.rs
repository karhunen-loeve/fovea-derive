#[test]
fn zeroable_pixel_compile_tests() {
    let t = trybuild::TestCases::new();
    // Compile-fail tests
    t.compile_fail("tests/ui/zeroable_pixel_enum.rs");
    t.compile_fail("tests/ui/zeroable_pixel_union.rs");
    t.compile_fail("tests/ui/zeroable_pixel_no_fields.rs");
    // Attribute-related compile-fail tests
    t.compile_fail("tests/ui/zeroable_pixel_bare_zero_attr.rs");
    t.compile_fail("tests/ui/zeroable_pixel_empty_zero_attr.rs");
    t.compile_fail("tests/ui/zeroable_pixel_duplicate_zero_attr.rs");
    t.compile_fail("tests/ui/zeroable_pixel_name_value_zero_attr.rs");
}
