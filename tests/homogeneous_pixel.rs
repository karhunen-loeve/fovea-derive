#[test]
fn homogeneous_pixel_compile_tests() {
    let t = trybuild::TestCases::new();
    // Compile-fail tests
    t.compile_fail("tests/ui/homogeneous_pixel_enum.rs");
    t.compile_fail("tests/ui/homogeneous_pixel_union.rs");
    t.compile_fail("tests/ui/homogeneous_pixel_missing_repr_c.rs");
    t.compile_fail("tests/ui/homogeneous_pixel_mixed_types.rs");
    t.compile_fail("tests/ui/homogeneous_pixel_no_fields.rs");
}
