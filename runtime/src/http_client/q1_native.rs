//! Private Q1 native adapter. No public or compiler entry point.
use super::super::q1_dial::{AttemptCleanup, AttemptControl, AttemptFailure, DialPlan, PlanStop};
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailoverError {
    Plan(PlanStop),
    Socket(SocketError),
}

async fn connect_plan(
    plan: DialPlan,
    control: &DialControl,
    quota: &SocketQuota,
) -> Result<(PinnedSocket, DialPlan), FailoverError> {
    connect_plan_inner(plan, control, quota, TcpStream::connect).await
}

// Both first-poll ordering and cancellation are supplied by the existing
// numeric socket helper. No detached connector task or child is spawned here.
fn attempt_control(control: &DialControl) -> AttemptControl {
    let mut state = AttemptControl::default();
    match control.checked_state() {
        Ok(_) => {}
        Err(SocketError::Closing) => state.close(),
        Err(_) => state.latch_fault(1),
    }
    state
}

async fn connect_plan_inner<F, Fut>(
    mut plan: DialPlan,
    control: &DialControl,
    quota: &SocketQuota,
    mut make: F,
) -> Result<(PinnedSocket, DialPlan), FailoverError>
where
    F: FnMut(SocketAddr) -> Fut,
    Fut: Future<Output = std::io::Result<TcpStream>>,
{
    loop {
        let attempt = plan
            .next_attempt(Instant::now(), &attempt_control(control))
            .map_err(FailoverError::Plan)?;
        let selection = plan.dial_selection(&attempt).map_err(FailoverError::Plan)?;
        let slot_end = attempt.slot_end();
        match connect_selection_inner(selection, slot_end, control, quota, &mut make).await {
            Ok(socket) => {
                plan.connected_after_peer_check(attempt, Instant::now(), &attempt_control(control))
                    .map_err(FailoverError::Plan)?;
                // peer_checked seals candidate selection; this plan has no
                // route back from TLS/HTTP to another connection attempt.
                return Ok((socket, plan));
            }
            Err(error) => {
                let failure = match error {
                    SocketError::Connect(kind) => AttemptFailure::Io(kind),
                    SocketError::Deadline => AttemptFailure::SlotExpired,
                    // Capacity, policy, close/fault and peer errors never
                    // become retryable transport failures.
                    _ => return Err(FailoverError::Socket(error)),
                };
                // The helper has returned only after its connector future and
                // socket ticket were dropped. It never creates a child task.
                plan.fail_after_cleanup(
                    attempt,
                    failure,
                    AttemptCleanup::SocketDisposedAndProvenNoChild,
                    Instant::now(),
                    &attempt_control(control),
                )
                .map_err(FailoverError::Plan)?;
            }
        }
    }
}

#[cfg(test)]
#[path = "q1_native_tests.rs"]
mod tests;
