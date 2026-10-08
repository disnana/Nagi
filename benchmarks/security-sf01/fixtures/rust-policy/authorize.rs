const POLICY_MODE: &str = "handwritten-rust-policy";

pub fn authorize_read(
    scope: AuthScope,
    request: http::Request,
    state: Arc<super::State>,
) -> impl Future<Output = Result<Grant<super::Read>, Failure>> + Send + 'static {
    let future = authorize_inner(scope, request, state);
    AUTHORIZE_CALLBACK_BYTES.record(size_of_val(&future) as u64);
    future
}

async fn authorize_inner(
    scope: AuthScope,
    request: http::Request,
    state: Arc<super::State>,
) -> Result<Grant<super::Read>, Failure> {
    let entered = Instant::now();
    let resource = resource_id(&request)?;
    let owner_subject = state.owner_subject;
    let blocked = state.blocked;
    let policy = rust_policy(&scope, resource, owner_subject, blocked);
    measure_policy(policy).await?;
    issue_grant(scope, resource, entered)
}

async fn rust_policy(
    scope: &AuthScope,
    resource: i64,
    owner_subject: i64,
    blocked: bool,
) -> Result<(), Failure> {
    policy_pause().await;
    if scope.subject() != owner_subject || blocked || resource != 1 {
        return Err(Failure::denied());
    }
    Ok(())
}
