#[test]
fn ui_component_compile_failures() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/missing_default.rs");
    t.compile_fail("tests/ui/missing_clone.rs");
    t.pass("tests/ui/runtime_only_ok.rs");
}

#[test]
fn facade_rejects_removed_public_apis() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/no_public_queue.rs");
    t.compile_fail("tests/ui/no_emit_ui_action.rs");
    t.compile_fail("tests/ui/no_run_app_runner.rs");
    t.compile_fail("tests/ui/no_core_alias.rs");
}
