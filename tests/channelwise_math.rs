#[test]
fn channelwise_math_compile_tests() {
    let t = trybuild::TestCases::new();
    // Compile-fail tests
    t.compile_fail("tests/ui/channelwise_math_enum.rs");
    t.compile_fail("tests/ui/channelwise_math_union.rs");
    t.compile_fail("tests/ui/channelwise_math_unit_struct.rs");
}
