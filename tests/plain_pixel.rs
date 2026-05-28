#[test]
fn plain_pixel_compile_tests() {
    let t = trybuild::TestCases::new();
    // Compile-fail tests
    t.compile_fail("tests/ui/plain_pixel_missing_repr_c.rs");
    t.compile_fail("tests/ui/plain_pixel_enum.rs");
    t.compile_fail("tests/ui/plain_pixel_union.rs");
}
