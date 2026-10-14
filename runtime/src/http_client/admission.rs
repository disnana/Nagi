//! Closed one-use Auth admission into the parent's bounded retained ledger.
use super::parent_drain::RunnerOwnerHandle;
use super::*;
use crate::auth;
use std::time::Instant;

#[derive(Debug)]
pub(super) struct Command {
    pub(super) id: u64,
    key: JobKey,
    pub(super) subject: i64,
    pub(super) resource: i64,
    pub(super) end: Instant,
    payload: Vec<u8>,
    shared: Arc<Shared>,
    armed: bool,
}

impl Command {
    pub(super) fn is_public(&self) -> bool {
        self.key.seal.explicit_public.load(Ordering::Acquire)
    }
    pub(super) fn key(&self) -> &JobKey {
        &self.key
    }
    pub(super) fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub(super) fn take_payload(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.payload)
    }
}

impl Drop for Command {
    fn drop(&mut self) {
        // A monitor may retire immediately after this publication. The payload
        // must be gone first, including on the no-root and cancelled paths.
        drop(std::mem::take(&mut self.payload));
        if self.key.seal.completion.load(Ordering::Acquire) & COMMAND_POPPED != 0 {
            self.key
                .seal
                .command_disposed(self.armed && !self.key.seal.root_registered());
            // Drop takes no Owner, waiter, or watch-value lock.
            self.shared.stop_notify.notify_waiters();
        }
    }
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
        key.seal
            .admitted_end
            .set(end)
            .expect("one-use bound command deadline");
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
                shared: Arc::clone(&self.shared),
                armed: false,
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
        self.take_command_matching(None)
    }
    fn take_command_matching(&mut self, only: Option<u64>) -> Result<Option<Command>, LedgerError> {
        if self.is_closing() {
            return Err(LedgerError::Closing);
        }
        if self.fault().is_some() {
            return Err(LedgerError::Faulted);
        }
        for job in &mut self.jobs {
            if only.is_some_and(|id| id != job.id) {
                continue;
            }
            if job.cancel_requested || job.end.is_some_and(|end| end <= Instant::now()) {
                continue;
            }
            if let Some(mut command) = job.queued.take() {
                // The retained record also follows the caller-owned payload
                // until Command::drop publishes disposal after payload Drop.
                command.armed = true;
                command.key.seal.command_popped();
                return Ok(Some(command));
            }
        }
        Ok(None)
    }
}

impl ClientSender {
    // First polling owns only a finite registered waiter. The unpolled caller
    // input is not retained by the core; waiter Drop releases registered bytes.
    #[cfg(test)]
    pub(crate) async fn submit<P>(
        &self,
        grant: auth::Grant<P>,
        payload: Vec<u8>,
        whole_end: Instant,
    ) -> Result<u64, AdmissionError> {
        self.submit_key(grant, payload, whole_end)
            .await
            .map(|key| key.id())
    }
    #[cfg(test)]
    pub(super) async fn submit_key<P>(
        &self,
        grant: auth::Grant<P>,
        payload: Vec<u8>,
        whole_end: Instant,
    ) -> Result<JobKey, AdmissionError> {
        self.submit_bound(grant, payload, whole_end, false, None)
            .await
    }
    pub(super) async fn submit_operation<P>(
        &self,
        grant: auth::Grant<P>,
        payload: Vec<u8>,
        whole_end: Instant,
        operation: RegisteredOperation,
    ) -> Result<JobKey, AdmissionError> {
        if operation.explicit_public {
            return Err(AdmissionError::Ledger(LedgerError::InvalidLimits));
        }
        self.submit_bound(grant, payload, whole_end, true, Some(operation))
            .await
    }
    async fn submit_bound<P>(
        &self,
        grant: auth::Grant<P>,
        payload: Vec<u8>,
        whole_end: Instant,
        retain_result: bool,
        operation: Option<RegisteredOperation>,
    ) -> Result<JobKey, AdmissionError> {
        let mut events = self.shared.events_tx.subscribe();
        let (owner, ticket) = self
            .compact_waiter(payload, whole_end)
            .map_err(AdmissionError::Ledger)?;
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
                match self.reserve_waiter_command(
                    &mut state,
                    &ticket,
                    retain_result,
                    false,
                    operation,
                ) {
                    Ok(reserved) => {
                        reservation.get_or_init(|| reserved);
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
                            Ok(Ok(key)) => Ok(key),
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
    // Independent grant-free admission. It never invokes protected submission
    // or constructs a Grant/permit; the immutable registered public operation
    // is its sole egress declaration. Both routes share only bounded bookkeeping.
    pub(super) async fn submit_public_operation(
        &self,
        payload: Vec<u8>,
        whole_end: Instant,
        operation: RegisteredOperation,
    ) -> Result<JobKey, LedgerError> {
        if !operation.explicit_public {
            return Err(LedgerError::InvalidLimits);
        }
        let mut events = self.shared.events_tx.subscribe();
        let (owner, ticket) = self.compact_waiter(payload, whole_end)?;
        loop {
            let mut reservation = std::cell::OnceCell::new();
            {
                let mut state = owner.lock().map_err(|_| LedgerError::Faulted)?;
                if whole_end <= Instant::now() {
                    return Err(LedgerError::Expired);
                }
                match self.reserve_waiter_command(&mut state, &ticket, true, true, Some(operation))
                {
                    Ok(reserved) => {
                        reservation.get_or_init(|| reserved);
                        // These unused compatibility fields are not authority.
                        // Public preparation receives no BoundValues or secret.
                        let result = state.enqueue(0, 0, whole_end, &mut reservation);
                        drop(state);
                        drop(reservation);
                        return result;
                    }
                    Err(LedgerError::Capacity) => {}
                    Err(error) => return Err(error),
                }
            }
            tokio::select! {
                changed=events.changed() => { if changed.is_err() { return Err(LedgerError::Closing); } }
                _=tokio::time::sleep_until(whole_end.into()) => return Err(LedgerError::Expired),
            }
        }
    }
    fn compact_waiter(
        &self,
        payload: Vec<u8>,
        whole_end: Instant,
    ) -> Result<(Arc<Mutex<Owner>>, WaiterTicket), LedgerError> {
        let owner = Arc::clone(self.owner.as_ref().ok_or(LedgerError::Closing)?);
        if whole_end <= Instant::now() {
            return Err(LedgerError::Expired);
        }
        let job_limit = owner
            .lock()
            .map_err(|_| LedgerError::Faulted)?
            .job_byte_limit;
        if payload.len() > self.shared.limits.waiter_bytes || payload.len() > job_limit {
            return Err(LedgerError::Capacity);
        }
        let mut frozen = Vec::new();
        frozen
            .try_reserve_exact(payload.len())
            .map_err(|_| LedgerError::Allocation)?;
        frozen.extend_from_slice(&payload);
        drop(payload);
        let ticket = owner
            .lock()
            .map_err(|_| LedgerError::Faulted)?
            .try_register_waiter(frozen)
            .map_err(|(error, _)| error)?;
        Ok((owner, ticket))
    }
    fn reserve_waiter_command(
        &self,
        state: &mut Owner,
        ticket: &WaiterTicket,
        retain_result: bool,
        explicit_public: bool,
        operation: Option<RegisteredOperation>,
    ) -> Result<Reservation, LedgerError> {
        let mut book = self
            .shared
            .waiters
            .lock()
            .map_err(|_| LedgerError::Faulted)?;
        let index = book
            .records
            .iter()
            .position(|record| record.id == ticket.id)
            .ok_or(LedgerError::Closing)?;
        let (key, tasks) = state.reserve_command(book.records[index].payload.len())?;
        key.seal
            .publication_pending
            .store(retain_result, Ordering::Release);
        key.seal
            .explicit_public
            .store(explicit_public, Ordering::Release);
        if let Some(operation) = operation {
            key.seal
                .operation
                .set(operation)
                .unwrap_or_else(|_| panic!("one-use registered operation"));
        }
        let record = book.records.swap_remove(index);
        book.bytes -= record.payload.len();
        Ok(Reservation {
            key,
            payload: record.payload,
            tasks,
        })
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
        let (delivery, waiting) = GateFreeFuture::new(future);
        let prepared = PreparedLaunch::new(kind);
        let registered = (|| {
            let owner = self.owner.as_ref().ok_or(LedgerError::Closing)?;
            let mut state = owner.lock().map_err(|_| LedgerError::Faulted)?;
            state
                .stage(
                    id,
                    kind,
                    waiting,
                    false,
                    Some(Arc::clone(&delivery.receipt)),
                    prepared,
                )
                .map_err(|(error, _)| error)
        })();
        let registered = registered.map(|(launch, waiting)| {
            launch.run(self.owner.as_ref().expect("retained owner"), waiting);
        });
        delivery.finish(registered)
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
    pub(super) fn take_command_for(&self, id: u64) -> Result<Option<Command>, LedgerError> {
        self.owner
            .lock()
            .map_err(|_| LedgerError::Faulted)?
            .take_command_matching(Some(id))
    }
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
        let (delivery, waiting) = GateFreeFuture::new(future);
        let prepared = PreparedLaunch::new(kind);
        let registered = (|| {
            let mut state = self.owner.lock().map_err(|_| LedgerError::Faulted)?;
            let index = match state.key_index(key) {
                Ok(index) => index,
                Err(error) => return Err(error),
            };
            // A bare key is not authority to register the one JobRoot. Only the
            // still-owned popped Command can cross that transition.
            if kind == TaskKind::JobRoot {
                return Err(LedgerError::PhaseClosed);
            }
            if state.jobs[index].queued.is_some()
                || (kind == TaskKind::JobRoot && key.seal.root_registered())
                || (kind == TaskKind::HttpDriver && key.seal.staged(TaskKind::HttpDriver))
                || (kind != TaskKind::JobRoot && !key.seal.root_registered())
            {
                return Err(LedgerError::PhaseClosed);
            }
            let shared = Arc::clone(&state.shared);
            let _entry = match shared.begin_enqueue() {
                Ok(entry) => entry,
                Err(error) => return Err(error),
            };
            state
                .stage(
                    key.id,
                    kind,
                    waiting,
                    true,
                    Some(Arc::clone(&delivery.receipt)),
                    prepared,
                )
                .map_err(|(error, _)| error)
        })();
        let registered = registered.map(|(launch, waiting)| launch.run(&self.owner, waiting));
        delivery.finish(registered)
    }

    // Command remains caller-owned until after the actual handle is registered.
    // No payload or Command destructor is delegated to runtime.spawn while the
    // Owner gate is held. The native payload-transfer port is a later slice.
    pub(super) fn spawn_root<F: Future<Output = ()> + Send + 'static>(
        &self,
        command: &mut Command,
        future: F,
    ) -> Result<(), (LedgerError, F)> {
        let (delivery, waiting) = GateFreeFuture::new(future);
        let prepared = PreparedLaunch::new(TaskKind::JobRoot);
        let registered = (|| {
            if !command.armed {
                return Err(LedgerError::PhaseClosed);
            }
            let mut state = self.owner.lock().map_err(|_| LedgerError::Faulted)?;
            let index = match state.key_index(command.key()) {
                Ok(index) => index,
                Err(error) => return Err(error),
            };
            if command.id != command.key.id
                || state.jobs[index].queued.is_some()
                || command.key.seal.staged(TaskKind::JobRoot)
                || command.key.seal.command_abandoned()
            {
                return Err(LedgerError::PhaseClosed);
            }
            let shared = Arc::clone(&state.shared);
            let _entry = match shared.begin_enqueue() {
                Ok(entry) => entry,
                Err(error) => return Err(error),
            };
            state
                .stage(
                    command.id,
                    TaskKind::JobRoot,
                    waiting,
                    true,
                    Some(Arc::clone(&delivery.receipt)),
                    prepared,
                )
                .map_err(|(error, _)| error)
        })();
        let registered = registered.map(|(launch, waiting)| {
            launch.run(&self.owner, waiting);
            command.armed = false;
        });
        delivery.finish(registered)
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
