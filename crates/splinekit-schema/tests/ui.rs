#[test]
fn compare_adapter_findings_with_real_serde_compiler_failures() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
