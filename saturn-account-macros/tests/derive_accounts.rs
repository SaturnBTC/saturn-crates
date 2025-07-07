#[test]
fn pass_tests() {
    let t = trybuild::TestCases::new();
    // Compile all expected-to-pass test cases under the `pass` and `ui` directories.
    t.pass("tests/pass/*.rs");
    t.pass("tests/ui/*.rs");
    // Compile-fail cases expected to produce macro errors.
    t.compile_fail("tests/compile_fail/*.rs");
}
