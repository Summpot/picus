#[test]
fn ui_component_compile_failures() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/missing_default.rs");
    t.compile_fail("tests/ui/missing_clone.rs");
    t.pass("tests/ui/runtime_only_ok.rs");
}
