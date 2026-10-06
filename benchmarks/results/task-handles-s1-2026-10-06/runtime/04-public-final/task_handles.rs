use nagi_runtime::{Error, ErrorKind, Task, TaskFailure, TaskFailureKind, TaskScope};

fn read_failure(failure: &TaskFailure) -> (TaskFailureKind, &str) {
    (failure.kind(), failure.message())
}

#[tokio::test]
async fn public_task_results_keep_business_error_inside_and_legacy_error_at_exit() {
    let mut scope = TaskScope::new();
    let task: Task<Result<String, u8>> = scope.spawn_value(async { Err(7) });
    let value: Result<Result<String, u8>, TaskFailure> = scope.receive(task).await;
    assert_eq!(value.unwrap(), Err(7));
    let ignored = scope.spawn_value(async { 42_u64 });
    let discard: fn(&mut TaskScope, Task<u64>) = TaskScope::discard::<u64>;
    discard(&mut scope, ignored);
    scope.spawn(async { Ok(()) });
    scope.join().await.unwrap();
    scope.spawn(async { Err(Error::invalid("original legacy error")) });
    let error: Error = scope.join().await.unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Invalid));
    assert_eq!(error.message, "original legacy error");
    let repeated: Error = scope.cancel().await.unwrap_err();
    assert!(matches!(repeated.kind, ErrorKind::Invalid));
    assert_eq!(repeated.message, error.message);
}

#[tokio::test]
async fn public_failure_getters_keep_fault_sticky_after_receive() {
    let mut scope = TaskScope::new();
    let task: Task<()> = scope.spawn_value(async { panic!("public child panic") });
    let failure = scope.receive(task).await.unwrap_err();
    let (kind, message) = read_failure(&failure);
    let copied = kind;
    assert_eq!(kind, copied);
    assert_eq!(kind, TaskFailureKind::Panicked);
    assert!(!message.is_empty());
    let error = scope.join().await.unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Internal));
    assert_eq!(error.message, message);
    assert!(scope.join().await.is_err());
    let _other_kinds = [
        TaskFailureKind::Cancelled,
        TaskFailureKind::LegacyError,
        TaskFailureKind::Internal,
    ];
}

#[tokio::test]
async fn public_mixed_legacy_failure_keeps_failure_category_and_native_exit_error() {
    let mut scope = TaskScope::new();
    let task = scope.spawn_value(std::future::pending::<u64>());
    scope.spawn(async { Err(Error::invalid("mixed legacy cause")) });
    let failure = scope.receive(task).await.unwrap_err();
    assert_eq!(failure.kind(), TaskFailureKind::LegacyError);
    assert_eq!(failure.message(), "Invalid: mixed legacy cause");
    let error = scope.join().await.unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Invalid));
    assert_eq!(error.message, "mixed legacy cause");
}
