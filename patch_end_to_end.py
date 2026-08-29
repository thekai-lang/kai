import re
with open('crates/kai-driver/tests/end_to_end.rs', 'r') as f:
    text = f.read()

text = text.replace('''#[test]
fn v0012_full_pipeline_matches_golden_ir() {
    let source = fixture(V0012);
    let ir = pipeline::compile(&source).expect("compilation should succeed");

    assert_golden("v0012/main.expected.ll", &ir);
}

#[test]
fn v0012_jit_module_tree_returns_expected_value() {
    let source = fixture(V0012);
    let code = pipeline::jit_file(&source).expect("jit execution should succeed");
    assert_eq!(code, 0, "test program should return 0 on success");
}''', '''fn v0012_entry() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/v0012/main.kai")
}

#[test]
fn v0012_full_pipeline_matches_golden_ir() {
    let ir = pipeline::compile_file(&v0012_entry()).expect("compilation should succeed");
    assert_golden("v0012/main.expected.ll", &ir);
}

#[test]
fn v0012_jit_module_tree_returns_expected_value() {
    assert_eq!(pipeline::jit_file(&v0012_entry()).unwrap(), 0);
}''')

with open('crates/kai-driver/tests/end_to_end.rs', 'w') as f:
    f.write(text)
