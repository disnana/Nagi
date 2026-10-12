//! Closed one-use Auth admission into the parent's bounded retained ledger.
use super::*;
use crate::auth;
use std::time::Instant;

#[derive(Debug)]
pub(super) struct Command {
    pub(super) id: u64,
    pub(super) subject: i64,
    pub(super) resource: i64,
    pub(super) end: Instant,
    pub(super) payload: Vec<u8>,
}

#[derive(Debug)]
pub(crate) enum AdmissionError {
    Ledger(LedgerError),
    Auth(auth::Failure),
}

struct Reservation {
    id: u64,
    payload: Vec<u8>,
    tasks: Vec<TrackedTask>,
}

impl Owner {
    fn reserve_command(&mut self, bytes: usize) -> Result<(u64, Vec<TrackedTask>), LedgerError> {
        if self.is_closing() {
            return Err(LedgerError::Closing);
        }
        if self.first_fault.is_some() {
            return Err(LedgerError::Faulted);
        }
        let retained = self
            .retained_job_bytes
            .checked_add(bytes)
            .ok_or(LedgerError::Capacity)?;
        if self.jobs.len() >= self.limits.jobs
            || self.used_queue >= self.limits.queue
            || retained > self.job_byte_limit
            || self.used_tasks[0] + self.reserved_roots[0] >= self.limits.job_roots
            || self.used_tasks[1] + self.reserved_roots[1] >= self.limits.dns_roots
        {
            return Err(LedgerError::Capacity);
        }
        self.next_job_id
            .checked_add(1)
            .ok_or(LedgerError::Capacity)?;
        // Every fallible allocation precedes permit issuance. Client gate stays held.
        self.jobs
            .try_reserve(1)
            .map_err(|_| LedgerError::Allocation)?;
        let mut tasks = Vec::new();
        tasks
            .try_reserve_exact(
                self.limits.job_roots
                    + self.limits.dns_roots
                    + self.limits.dns_children
                    + self.limits.http_drivers,
            )
            .map_err(|_| LedgerError::Allocation)?;
        Ok((self.next_job_id, tasks))
    }
    fn enqueue(
        &mut self,
        subject: i64,
        resource: i64,
        end: Instant,
        reservation: Reservation,
    ) -> u64 {
        let Reservation { id, payload, tasks } = reservation;
        let bytes = payload.len();
        self.jobs.push(JobRecord {
            id,
            tasks,
            dns_frozen: false,
            cancel_requested: false,
            end: Some(end),
            payload_bytes: bytes,
            reserved_roots: [true; 2],
            started: false,
            queued: Some(Command {
                id,
                subject,
                resource,
                end,
                payload,
            }),
        });
        self.next_job_id = id + 1; // checked while the same client gate was held
        self.used_queue += 1;
        self.reserved_roots[0] += 1;
        self.reserved_roots[1] += 1;
        self.retained_job_bytes += bytes; // checked by reserve_command
        self.shared.notify();
        id
    }
}

impl ClientSender {
    // First polling owns only a finite registered waiter. The unpolled caller
    // input is not retained by the core; waiter Drop releases registered bytes.
    pub(crate) async fn submit<P>(
        &self,
        grant: auth::Grant<P>,
        payload: Vec<u8>,
        whole_end: Instant,
    ) -> Result<u64, AdmissionError> {
        let owner = self
            .owner
            .as_ref()
            .ok_or(AdmissionError::Ledger(LedgerError::Closing))?;
        if whole_end <= Instant::now() {
            return Err(AdmissionError::Ledger(LedgerError::Expired));
        }
        // Freeze into a compact bounded core-owned allocation. A caller Vec may
        // have huge spare capacity; retaining it by len alone is not a byte bound.
        let job_byte_limit = owner
            .lock()
            .map_err(|_| AdmissionError::Ledger(LedgerError::Faulted))?
            .job_byte_limit;
        if payload.len() > self.shared.limits.waiter_bytes || payload.len() > job_byte_limit {
            return Err(AdmissionError::Ledger(LedgerError::Capacity));
        }
        let mut frozen = Vec::new();
        frozen
            .try_reserve_exact(payload.len())
            .map_err(|_| AdmissionError::Ledger(LedgerError::Allocation))?;
        frozen.extend_from_slice(&payload);
        drop(payload);
        let payload = frozen;
        let mut events = self.shared.events_tx.subscribe();
        let ticket = owner
            .lock()
            .map_err(|_| AdmissionError::Ledger(LedgerError::Faulted))?
            .try_register_waiter(payload)
            .map_err(|(error, _)| AdmissionError::Ledger(error))?;
        loop {
            // No lease check is extracted as a bool. The actual consumed Grant
            // issues its permit and captures min(original ceilings) under Lease gate.
            {
                let mut state = owner
                    .lock()
                    .map_err(|_| AdmissionError::Ledger(LedgerError::Faulted))?;
                if whole_end <= Instant::now() {
                    return Err(AdmissionError::Ledger(LedgerError::Expired));
                }
                let bytes = {
                    let book = self
                        .shared
                        .waiters
                        .lock()
                        .map_err(|_| AdmissionError::Ledger(LedgerError::Faulted))?;
                    book.records
                        .iter()
                        .find(|record| record.id == ticket.id)
                        .map(|record| record.payload.len())
                        .ok_or(AdmissionError::Ledger(LedgerError::Closing))?
                };
                match state.reserve_command(bytes) {
                    Ok((id, tasks)) => {
                        let payload = {
                            let mut book = self
                                .shared
                                .waiters
                                .lock()
                                .map_err(|_| AdmissionError::Ledger(LedgerError::Faulted))?;
                            let index = book
                                .records
                                .iter()
                                .position(|record| record.id == ticket.id)
                                .ok_or(AdmissionError::Ledger(LedgerError::Closing))?;
                            let record = book.records.swap_remove(index);
                            book.bytes -= record.payload.len();
                            record.payload
                        };
                        let reservation = Reservation { id, payload, tasks };
                        // Lease guard is released by auth before this synchronous
                        // closed enqueue callback; client gate remains held throughout.
                        return grant
                            .submit_private(
                                reservation,
                                whole_end,
                                |subject, resource, end, reservation| {
                                    state.enqueue(subject, resource, end, reservation)
                                },
                            )
                            .map_err(AdmissionError::Auth);
                    }
                    Err(LedgerError::Capacity) => {}
                    Err(error) => return Err(AdmissionError::Ledger(error)),
                }
            }
            tokio::select! {
                changed = events.changed() => {
                    if changed.is_err() { return Err(AdmissionError::Ledger(LedgerError::Closing)); }
                }
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(whole_end)) => {
                    return Err(AdmissionError::Ledger(LedgerError::Expired));
                }
            }
        }
    }
    // Only the private reviewed runner receives a queued bound command. There
    // is no public callback or raw proof-return path. Native IO is a later adapter.
    pub(super) fn take_command(&self) -> Result<Option<Command>, LedgerError> {
        let owner = self.owner.as_ref().ok_or(LedgerError::Closing)?;
        let mut state = owner.lock().map_err(|_| LedgerError::Faulted)?;
        if state.is_closing() {
            return Err(LedgerError::Closing);
        }
        if state.fault().is_some() {
            return Err(LedgerError::Faulted);
        }
        for job in &mut state.jobs {
            if job.cancel_requested {
                continue;
            }
            if job.end.is_some_and(|end| end <= Instant::now()) {
                continue;
            }
            if let Some(command) = job.queued.take() {
                // Reservation remains live across the command-to-first-spawn
                // gap. Only an actual registered task marks the job started.
                return Ok(Some(command));
            }
        }
        Ok(None)
    }
    pub(super) fn spawn<F: Future<Output = ()> + Send + 'static>(
        &self,
        id: u64,
        kind: TaskKind,
        future: F,
    ) -> Result<(), (LedgerError, F)> {
        let Some(owner) = &self.owner else {
            return Err((LedgerError::Closing, future));
        };
        let Ok(mut state) = owner.lock() else {
            return Err((LedgerError::Faulted, future));
        };
        state.try_spawn(id, kind, future)
    }
    pub(super) fn cancel(&self, id: u64) -> Result<(), LedgerError> {
        self.owner
            .as_ref()
            .ok_or(LedgerError::Closing)?
            .lock()
            .map_err(|_| LedgerError::Faulted)?
            .request_cancel(id)
    }
    // The native adapters must use only a short primitive-specific synchronous
    // callback here. Whole root/lookup futures are never polled under this gate.
    pub(super) fn phase<T>(
        &self,
        id: u64,
        operation: impl FnOnce() -> T,
    ) -> Result<T, LedgerError> {
        let state = self
            .owner
            .as_ref()
            .ok_or(LedgerError::Closing)?
            .lock()
            .map_err(|_| LedgerError::Faulted)?;
        if state.is_closing() {
            return Err(LedgerError::Closing);
        }
        if state.fault().is_some() {
            return Err(LedgerError::Faulted);
        }
        let job = state
            .jobs
            .iter()
            .find(|job| job.id == id)
            .ok_or(LedgerError::UnknownJob)?;
        if job.cancel_requested {
            return Err(LedgerError::Closing);
        }
        if job.end.is_none_or(|end| end <= Instant::now()) {
            return Err(LedgerError::Expired);
        }
        Ok(operation())
    }
}

#[cfg(test)]
#[path = "parent_auth_tests.rs"]
mod tests;
