use crate::cancellation::*;
use std::time::Duration;

#[tokio::test]
async fn cancellation_has_priority_over_a_ready_operation() {
    let (stop_token, stop_handle) = stop_pair();
    stop_handle.stop();

    assert_eq!(
        stop_token.run_until_cancelled(async { 7 }).await,
        ControlledFutureOutcome::Cancelled
    );
}

#[tokio::test]
async fn timeout_is_reported_separately() {
    let (stop_token, _stop_handle) = stop_pair();
    let outcome = stop_token
        .run_until_cancelled_or_timeout(Duration::ZERO, std::future::pending::<()>())
        .await;

    assert_eq!(outcome, ControlledFutureOutcome::TimedOut);
}
