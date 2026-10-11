#[path = "support/native_run_summary.rs"]
mod summary;

const ONE: &str = "running 1 test\ntest actual_checkpoint ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
const MANY: &str = "running 3 tests\ntest first ... ok\ntest second ... ok\ntest third ... ok\n\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s\n";

#[test]
fn accepts_real_nonempty_success_and_future_multiple_tests() {
    assert!(summary::successful_native_test(true, ONE.as_bytes()));
    assert!(summary::successful_native_test(true, MANY.as_bytes()));
}
#[test]
fn rejects_missing_or_zero_test_final_summary() {
    for text in [
        "",
        "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
        "running 1 test\ntest thing ... ok\n",
        "success: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n",
    ] {
        assert!(!summary::successful_native_test(true, text.as_bytes()), "{text}");
    }
}
#[test]
fn rejects_failures_ignored_filtered_and_nonzero_exit() {
    assert!(!summary::successful_native_test(false, ONE.as_bytes()));
    for text in [
        "test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n",
        "test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s\n",
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s\n",
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
    ] {
        assert!(!summary::successful_native_test(true, text.as_bytes()), "{text}");
    }
}
