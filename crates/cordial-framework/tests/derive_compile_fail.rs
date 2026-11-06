//! Compile-fail tests of the derive macros. Each file in `tests/ui` must give the errors in
//! its `.stderr` file.
//!
//! Run with `TRYBUILD=overwrite` to write new `.stderr` files after a change of a message.

#[test]
fn compile_fail() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
