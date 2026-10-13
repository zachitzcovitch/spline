#[test]
fn derive_diagnostics_have_useful_source_spans() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
    #[cfg(feature = "http")]
    cases.compile_fail("tests/ui-http/*.rs");
}
