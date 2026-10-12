//! Closed one-use Auth admission into the parent's bounded retained ledger.
use super::parent_drain::RunnerOwnerHandle;
use super::*;
use crate::auth;
use std::time::Instant;

#[derive(Debug)]
pub(super) struct Command {
    pub(super) id: u64,
    pub(super) key: JobKey,
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
    key: JobKey,
    payload: Vec<u8>,
    tasks: Vec<TrackedTask>,
}

impl Owner {
    fn reserve_command(&mut self, bytes: usize) -> Result<(JobKey, Vec<TrackedTask>), LedgerError> {
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
        Ok((
            JobKey {
                id: self.next_job_id,
                seal: Arc::new(JobSeal::new()),
            },
            tasks,
        ))
    }
    fn enqueue(
        &mut self,
        subject: i64,
        resource: i64,
        end: Instant,
        reservation: &mut std::cell::OnceCell<Reservation>,
    ) -> Result<JobKey, LedgerError> {
        self.enqueue_with_after_entry(subject, resource, end, reservation, || {})
    }

    // The closure is a closed deterministic race hook in tests. The normal
    // adapter monomorphizes it to a no-op after the successful CAS.
    fn enqueue_with_after_entry(
        &mut self,
        subject: i64,
        resource: i64,
        end: Instant,
        reservation: &mut std::cell::OnceCell<Reservation>,
        after_entry: impl FnOnce(),
    ) -> Result<JobKey, LedgerError> {
        let _entry = self.shared.begin_enqueue()?;
        after_entry();
        let Reservation {
            key,
            payload,
            tasks,
        } = reservation.take().expect("reserved command");
        let id = key.id;
        let bytes = payload.len();
        self.jobs.push(JobRecord {
            id,
            seal: Arc::clone(&key.seal),
            tasks,
            dns_frozen: false,
            cancel_requested: false,
            end: Some(end),
            payload_bytes: bytes,
            reserved_roots: [true; 2],
            started: false,
            queued: Some(Command {
                id,
                key: key.clone(),
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
        Ok(key)
    }

    fn take_command(&mut self) -> Result<Option<Command>, LedgerError> {
        if self.is_closing() {
            return Err(LedgerError::Closing);
        }
        if self.fault().is_some() {
            return Err(LedgerError::Faulted);
        }
        for job in &mut self.jobs {
            if job.cancel_requested || job.end.is_some_and(|end| end <= Instant::now()) {
                continue;
            }
            if let Some(command) = job.queued.take() {
                // The retained record keeps capacity until actual registration
                // and joins, even after the runner owns the command.
                return Ok(Some(command));
            }
        }
        Ok(None)
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
            // This holder precedes the Owner guard in declaration order, so an
            // Auth or callback unwind drops Owner before any rejected payload.
            let mut reservation = std::cell::OnceCell::<Reservation>::new();
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
                    Ok((key, tasks)) => {
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
                        reservation.get_or_init(|| Reservation {
                            key,
                            payload,
                            tasks,
                        });
                        // Lease guard is released by auth before this synchronous
                        // closed enqueue callback; client gate remains held throughout.
                        let issued = grant.submit_private(
                            &mut reservation,
                            whole_end,
                            |subject, resource, end, reservation| {
                                state.enqueue(subject, resource, end, reservation)
                            },
                        );
                        // On Auth refusal or a losing STOP CAS, the owned
                        // reservation is still here and is dropped after Owner.
                        drop(state);
                        drop(reservation);
                        return match issued {
                            Ok(Ok(key)) => Ok(key.id()),
                            Ok(Err(error)) => Err(AdmissionError::Ledger(error)),
                            Err(error) => Err(AdmissionError::Auth(error)),
                        };
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
    #[cfg(test)]
    pub(super) fn take_command(&self) -> Result<Option<Command>, LedgerError> {
        let owner = self.owner.as_ref().ok_or(LedgerError::Closing)?;
        let mut state = owner.lock().map_err(|_| LedgerError::Faulted)?;
        state.take_command()
    }
    #[cfg(test)]
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
    #[cfg(test)]
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
    #[cfg(test)]
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

impl RunnerOwnerHandle {
    pub(super) fn take_command(&self) -> Result<Option<Command>, LedgerError> {
        self.owner
            .lock()
            .map_err(|_| LedgerError::Faulted)?
            .take_command()
    }

    // The seal is checked under the same Owner gate as the indexed ledger
    // transition. The rejected future is returned for Drop outside this gate.
    pub(super) fn spawn<F: Future<Output = ()> + Send + 'static>(
        &self,
        key: &JobKey,
        kind: TaskKind,
        future: F,
    ) -> Result<(), (LedgerError, F)> {
        let Ok(mut state) = self.owner.lock() else {
            return Err((LedgerError::Faulted, future));
        };
        let index = match state.key_index(key) {
            Ok(index) => index,
            Err(error) => return Err((error, future)),
        };
        if state.jobs[index].queued.is_some()
            || (kind == TaskKind::JobRoot && key.seal.root_registered())
            || (kind == TaskKind::HttpDriver && key.seal.driver_registered())
            || (kind != TaskKind::JobRoot && !key.seal.root_registered())
        {
            return Err((LedgerError::PhaseClosed, future));
        }
        let shared = Arc::clone(&state.shared);
        let _entry = match shared.begin_enqueue() {
            Ok(entry) => entry,
            Err(error) => return Err((error, future)),
        };
        state.try_spawn_impl(key.id, kind, future, true)
    }

    pub(super) fn cancel(&self, key: &JobKey) -> Result<(), LedgerError> {
        let mut state = self.owner.lock().map_err(|_| LedgerError::Faulted)?;
        state.key_index(key)?;
        state.request_cancel(key.id)
    }

    // For a primitive gate only: never poll a resolver, TLS handshake, root,
    // or HTTP driver Future inside this synchronous Owner-held closure.
    pub(super) fn phase<T>(
        &self,
        key: &JobKey,
        operation: impl FnOnce() -> T,
    ) -> Result<T, LedgerError> {
        let state = self.owner.lock().map_err(|_| LedgerError::Faulted)?;
        let index = state.key_index(key)?;
        if state.is_closing() {
            return Err(LedgerError::Closing);
        }
        if state.fault().is_some() {
            return Err(LedgerError::Faulted);
        }
        let job = &state.jobs[index];
        if job.cancel_requested || !key.seal.root_registered() {
            return Err(LedgerError::PhaseClosed);
        }
        if job.end.is_none_or(|end| end <= Instant::now()) {
            return Err(LedgerError::Expired);
        }
        let shared = Arc::clone(&state.shared);
        let _entry = shared.begin_enqueue()?;
        Ok(operation())
    }
}

#[cfg(test)]
#[path = "parent_auth_tests.rs"]
mod tests;
