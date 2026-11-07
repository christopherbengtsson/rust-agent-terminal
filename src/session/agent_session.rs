//! Concrete session owner. Polled by the supervisor; no lifetime session thread.
//! Launch, execution, capture and their durable obligations remain private here.
use super::{
    COLS, LaunchResult, Lifecycle, MAX_COLS, MAX_ROWS, Observation, ROWS, Request,
    launch_setup_fault,
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{self, Read, Write},
    os::unix::{ffi::OsStringExt, fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};
struct ScreenState {
    parser: vt100::Parser,
    output_bytes: u64,
}
impl ScreenState {
    fn snapshot(&self, state: &mut Observation) {
        let screen = self.parser.screen();
        state.screen = screen.contents();
        state.formatted = screen.state_formatted();
        let (rows, cols) = screen.size();
        state.rows = rows.get();
        state.cols = cols.get();
        state.output_bytes = self.output_bytes;
    }
}

// One in-flight creation owns its PTY and child even before the supervisor
// can install input/capture. The bounded receiver is also the launch reservation.
struct CreatedExecution {
    pty: portable_pty::PtyPair,
    child: crate::lifecycle::Child,
    listener: UnixListener,
    nonce: String,
}
struct CreatingExecution {
    request: Request,
    result: Receiver<Result<CreatedExecution, String>>,
}

pub(super) struct AgentSession {
    launch: Launch,
    execution: Execution,
    history: History,
    directory: PathBuf,
    endpoint_directory: PathBuf,
    state: Observation,
    task_identity: String,
    checkout: Option<PathBuf>,
    launches: HashMap<String, LaunchResult>,
    writes: crate::persistence::Obligations,
    metadata_failed: bool,
    metadata_error: Option<String>,
    shutdown: bool,
    retry_requested: bool,
    pending_history: Option<uuid::Uuid>,
    history_query: Option<(usize, Option<String>, u64)>,
}
#[derive(Default)]
struct Launch {
    resolving: Option<String>,
    cancelling: Option<(String, bool)>,
    selected: Option<(String, crate::persistence::ResumeIdentity)>,
    pending_launch: Option<Request>,
    prepared_ready: bool,
    creating: Option<CreatingExecution>,
    created: Option<Result<CreatedExecution, String>>,
    cancelled_launch: bool,
    gate_release: Option<SyncSender<()>>,
    partial_gate: Option<UnixListener>,
    gate_receipt: Option<Receiver<crate::recovery::Receipt>>,
    outcome_reply: bool,
    outcome_operation: Option<crate::persistence::Operation>,
    result_failed: bool,
}
struct Execution {
    child: Option<crate::lifecycle::Child>,
    writer: Option<SyncSender<Vec<u8>>>,
    write_errors: SyncSender<String>,
    input_errors: Receiver<String>,
    owned_scope: Option<crate::child_scope::Scope>,
    stopping: Option<Receiver<crate::lifecycle::Outcome>>,
    monitor: Option<crate::native_status::Monitor>,
    codex_monitor: Option<crate::codex_status::Monitor>,
    master: Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>>,
}
struct History {
    flush_pending: bool,
    final_flush: bool,
    history_sealed: bool,
    seal_pending: bool,
    capture_started_here: bool,
    capture_done: Option<Receiver<()>>,
    stop_drain: Arc<AtomicBool>,
    eof: bool,
    screen: Arc<Mutex<ScreenState>>,
}

impl AgentSession {
    pub(super) fn restore(
        directory: &Path,
        restored: crate::persistence::Restored,
        _storage: &crate::persistence::Worker,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut state = restored.state;
        if state.launch_request.is_empty() {
            state.launch_request = uuid::Uuid::new_v4().to_string();
        }
        let task_identity = restored.task;
        let checkout = restored
            .checkout
            .map(|bytes| PathBuf::from(std::ffi::OsString::from_vec(bytes)));
        let launches = restored
            .launches
            .into_iter()
            .map(|result| (result.request.clone(), result))
            .collect();
        let writes = crate::persistence::Obligations::default();
        state.metadata_pending = false;
        state.control_target = uuid::Uuid::new_v4().to_string();
        let (write_errors, input_errors) = mpsc::sync_channel(1);
        let endpoint_directory = PathBuf::from("/tmp").join(format!(
            "rat-{}-{}",
            unsafe { libc::geteuid() },
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        ));
        Ok(Self {
            endpoint_directory,
            directory: directory.to_owned(),
            state,
            task_identity,
            checkout,
            launches,
            writes,
            metadata_failed: false,
            metadata_error: None,
            shutdown: false,
            retry_requested: false,
            pending_history: None,
            history_query: None,
            launch: Launch::default(),
            execution: Execution {
                write_errors,
                input_errors,
                child: None,
                writer: None,
                owned_scope: None,
                stopping: None,
                monitor: None,
                codex_monitor: None,
                master: None,
            },
            history: History {
                flush_pending: false,
                final_flush: false,
                history_sealed: false,
                seal_pending: false,
                capture_started_here: false,
                capture_done: None,
                stop_drain: Arc::new(AtomicBool::new(false)),
                eof: true,
                screen: Arc::new(Mutex::new(ScreenState {
                    parser: vt100::Parser::new(
                        ROWS.try_into().unwrap(),
                        COLS.try_into().unwrap(),
                        0,
                    ),
                    output_bytes: 0,
                })),
            },
        })
    }
    pub(super) fn complete(
        &mut self,
        event: crate::persistence::Completion,
        storage: &crate::persistence::Worker,
    ) {
        let directory = self.directory.clone();
        let directory = directory.as_path();
        use crate::persistence::Completion;
        match event {
            Completion::ResumeIdentity(..) => {
                unreachable!("resolved resume rechecks supervisor admission")
            }
            Completion::ResumeCancelled(target, request, result) => {
                if target != self.state.control_target
                    || self.launch.cancelling.as_ref().map(|(id, _)| id) != Some(&request)
                {
                    return;
                }
                match result {
                    Ok(()) => self.launch.cancelling = None,
                    Err(error) => {
                        self.launch.cancelling = Some((request, false));
                        self.state.error = Some(format!(
                            "Resume cancellation not durable: {error}; Stop remains pending"
                        ));
                    }
                }
            }
            crate::persistence::Completion::Browse(_) => {}
            crate::persistence::Completion::HistoryHidden(id) => {
                if self
                    .state
                    .history
                    .as_ref()
                    .is_some_and(|page| page.session == id)
                {
                    self.state.history = None;
                }
            }
            Completion::Setups(_)
            | Completion::SetupSaved(..)
            | Completion::Creations(_)
            | Completion::CreationSaved(..)
            | Completion::Registry(_)
            | Completion::Selected(..)
            | Completion::Registered(..) => {
                unreachable!("registry completions are supervisor-owned")
            }
            Completion::Retried | Completion::Sealed => {}
            Completion::Cleanup(page) => self.state.cleanup = Some(page),
            Completion::Deleted(id) => {
                self.state.cleanup_message = Some(format!(
                    "History deletion completed: {id}. Capacity may remain low; y retry capture / s stop."
                ));
                self.state.history = None;
                self.pending_history = None;
                self.history_query = None;
                self.state.history_index = 0;
                if self
                    .state
                    .session_identity
                    .as_ref()
                    .is_some_and(|identity| identity.session == id)
                {
                    self.history.history_sealed = false;
                    self.history.capture_started_here = false;
                    self.state.accepted_bytes = 0;
                    self.state.written_bytes = 0;
                    self.state.durable_bytes = 0;
                    self.state.capture_pending_bytes = 0;
                    self.state.durable_commits.clear();
                }
                self.state.cleanup = None;
                let _ = storage
                    .commands
                    .try_send(crate::persistence::Command::Cleanup(0));
            }
            Completion::Committed(operation) => {
                if !self.writes.complete(&operation) {
                    return;
                }
                let current = operation.matches(&self.state);
                if current {
                    match operation.purpose {
                        crate::persistence::Purpose::Prepare => {
                            self.launch.prepared_ready = !self.launch.cancelled_launch
                                && self.launch.pending_launch.is_some();
                        }
                        crate::persistence::Purpose::Authorize if !self.launch.cancelled_launch => {
                            if let Some(release) = self.launch.gate_release.take() {
                                let _ = release.try_send(());
                            }
                        }
                        crate::persistence::Purpose::FinalFlush => {
                            self.history.flush_pending = false;
                            self.history.final_flush = true;
                        }
                        crate::persistence::Purpose::Seal => {
                            self.history.seal_pending = false;
                            self.history.history_sealed = true;
                        }
                        crate::persistence::Purpose::LaunchResult
                            if self.launch.outcome_operation.as_ref() == Some(&operation) =>
                        {
                            crate::recovery::crash_barrier(directory, "after-result");
                            self.launch.outcome_operation = None;
                            self.launch.outcome_reply = false;
                            self.launch.result_failed = false;
                        }
                        _ => {}
                    }
                }
                if current
                    && !matches!(
                        operation.purpose,
                        crate::persistence::Purpose::FinalFlush | crate::persistence::Purpose::Seal
                    )
                    && !self.writes.metadata_pending()
                    && !self.launch.result_failed
                {
                    if self.metadata_failed
                        && self.metadata_error.as_ref() == self.state.error.as_ref()
                    {
                        self.state.error = None;
                    }
                    self.metadata_failed = false;
                    self.metadata_error = None;
                }
            }
            Completion::Failed(operation, error) => {
                if !self.writes.complete(&operation) || !operation.matches(&self.state) {
                    return;
                }
                match operation.purpose {
                    crate::persistence::Purpose::Prepare
                    | crate::persistence::Purpose::Authorize => {
                        self.metadata_failed = true;
                        self.launch.pending_launch = None;
                        self.launch.prepared_ready = false;
                        self.launch.gate_release = None;
                    }
                    crate::persistence::Purpose::LaunchResult
                        if self.launch.outcome_operation.as_ref() == Some(&operation) =>
                    {
                        self.metadata_failed = true;
                        self.launch.outcome_operation = None;
                        self.launch.result_failed = true;
                    }
                    crate::persistence::Purpose::FinalFlush => {
                        self.history.flush_pending = false;
                        self.history.final_flush = false;
                    }
                    crate::persistence::Purpose::Seal => {
                        self.history.seal_pending = false;
                        self.history.history_sealed = false;
                    }
                    _ => {
                        self.metadata_failed = true;
                    }
                }
                self.shutdown = false;
                self.state.error = Some(format!(
                    "Launch/storage outcome not durable: {error}; stop or repair storage before retrying"
                ));
                if self.metadata_failed {
                    self.metadata_error = self.state.error.clone();
                }
            }
            Completion::History(page) => self.state.history = Some(page),
            Completion::HistoryTarget(target, request, result) => {
                if target == self.state.control_target && self.pending_history == Some(request) {
                    match result {
                        Ok(page) => {
                            if !page.projection_preparing {
                                self.pending_history = None;
                            }
                            self.state.history = Some(page);
                        }
                        Err(error) => {
                            self.pending_history = None;
                            self.state.history = None;
                            self.state.error = Some(format!("History unavailable: {error}; retry"))
                        }
                    }
                }
            }
            Completion::Error(kind, error) => {
                if kind == "cleanup" {
                    self.state.cleanup_message = Some(format!(
                        "History cleanup failed/incomplete: {error}; no recovery claimed; g reopen / select / retry, y capture retry / s stop"
                    ));
                    return;
                }
                if kind == "history" {
                    self.state.error = Some(error);
                    return;
                }
                self.history.flush_pending = false;
                self.shutdown = false;
                self.history.final_flush = false;
                self.state.error = Some(format!(
                    "History storage failed: {error}; not saved, retry quit"
                ));
            }
        }
    }
    pub(super) fn advance(&mut self, storage: &crate::persistence::Worker, adoption_pending: bool) {
        let directory = self.directory.clone();
        let directory = directory.as_path();
        if let Some(creation) = self.launch.creating.as_ref()
            && let Ok(result) = creation.result.try_recv()
        {
            self.launch.pending_launch = Some(self.launch.creating.take().unwrap().request);
            self.launch.prepared_ready = true;
            self.launch.created = Some(result);
        }
        if let Some(receipt) = &self.launch.gate_receipt
            && let Ok(result) = receipt.try_recv()
        {
            self.launch.gate_receipt = None;
            crate::recovery::crash_barrier(directory, "before-result");
            match result {
                _ if self.launch.cancelled_launch => {
                    self.state.error.get_or_insert_with(|| {
                        "Launch cancelled by Stop; metadata may still be pending".into()
                    });
                }
                crate::recovery::Receipt::Executed => {
                    let verified = self
                        .state
                        .execution
                        .as_ref()
                        .ok_or_else(|| io::Error::other("Launch stopped before live exec receipt"))
                        .and_then(|evidence| evidence.verify_live());
                    if let Err(error) = verified {
                        self.state.error = Some(format!(
                            "Exec receipt lost its live process identity: {error}; launch outcome uncertain"
                        ));
                    }
                }
                crate::recovery::Receipt::ExecFailed(error) => {
                    self.state.execution = None;
                    self.state.error = Some(
                        if self.state.agent.as_deref() == Some("Claude Code") {
                            format!(
                                "Claude Code launch failed: {error}; adopted checkout retained. Install claude on PATH and run claude auth login in your usual terminal"
                            )
                        } else if self.state.agent.as_deref() == Some("Codex CLI") {
                            format!(
                                "Codex CLI launch failed: {error}; adopted checkout retained. Install codex on PATH and run codex login in your usual terminal"
                            )
                        } else {
                            format!(
                                "Native executable launch failed: {error}; adopted checkout retained"
                            )
                        },
                    );
                }
                crate::recovery::Receipt::Unknown(error) => {
                    self.state.error = Some(format!(
                        "Native exec receipt uncertain: {error}; stop managed execution or reconcile explicitly"
                    ))
                }
            }
            let result = LaunchResult {
                request: self.state.launch_request.clone(),
                identity: self.state.identity.clone(),
                error: self.state.error.clone(),
            };
            self.launches.insert(result.request.clone(), result.clone());
            self.state.launch_result = Some(result);
            match self.writes.send(
                &storage.commands,
                crate::persistence::Command::Save(crate::persistence::Snapshot::result(
                    &self.state,
                    self.checkout.as_deref(),
                    &self.task_identity,
                )),
            ) {
                Ok(operation) => {
                    self.launch.outcome_operation = Some(operation);
                    self.launch.outcome_reply = true;
                }
                Err(error) => {
                    self.metadata_failed = true;
                    self.launch.result_failed = true;
                    self.launch.outcome_reply = true;
                    self.state.error = Some(format!(
                        "Launch outcome not durable: {error}; reconcile before replacement"
                    ));
                    self.metadata_error = self.state.error.clone();
                }
            }
        }
        if let Some(done) = &self.history.capture_done
            && done.try_recv().is_ok()
        {
            self.history.eof = true;
            self.history.capture_done = None;
        }
        if self.retry_requested {
            if self.metadata_failed {
                let snapshot = if self.launch.result_failed {
                    crate::persistence::Snapshot::result(
                        &self.state,
                        self.checkout.as_deref(),
                        &self.task_identity,
                    )
                } else {
                    crate::persistence::Snapshot::new(
                        &self.state,
                        self.checkout.as_deref(),
                        &self.task_identity,
                    )
                };
                if let Ok(operation) = self.writes.send(
                    &storage.commands,
                    crate::persistence::Command::Save(snapshot),
                ) {
                    if self.launch.result_failed {
                        self.launch.outcome_operation = Some(operation);
                        self.launch.outcome_reply = true;
                    }
                    self.retry_requested = false;
                }
            } else if storage
                .commands
                .try_send(crate::persistence::Command::Retry)
                .is_ok()
            {
                self.retry_requested = false;
            }
        }
        let progress = storage.progress_for(
            self.state
                .session_identity
                .as_ref()
                .map(|identity| identity.session.as_str()),
        );
        self.state.capacity = progress.capacity.clone();
        // Seal retires the storage registration before its completion arrives.
        // Final flush has already fixed these counters; keep them while sealing.
        if self.history.capture_started_here
            && !self.history.history_sealed
            && !self.history.seal_pending
        {
            self.state.accepted_bytes = progress.accepted;
            self.state.written_bytes = progress.written;
            self.state.storage_wait_millis = progress
                .operation
                .map_or(0, |(_, since)| since.elapsed().as_millis() as u64);
            self.state.producer_wait_millis = progress
                .producer_wait
                .map_or(0, |since| since.elapsed().as_millis() as u64);
            self.state.storage_error = progress.error.clone();
            self.state.capture_pending_bytes = progress.pending_bytes;
            self.state.capture_high_water = progress.pending_high_water;
            self.state.capture_backpressure_count = progress.backpressure_count;
            self.state.capture_backpressure_micros = progress.backpressure_micros;
            self.state.durable_bytes = progress.durable;
            self.state.durable_flush_count = progress.flush_count;
            self.state.durable_commits.clone_from(&progress.commits);
            self.state.durable_completed_millis = progress.completed_millis;
            self.state.sync_commit_micros = progress.last_flush_micros;
            self.state.accepted_to_durable_millis = progress.accepted_to_durable_millis;
        }

        self.state.metadata_pending = self.launch.resolving.is_some()
            || self.launch.cancelling.is_some()
            || self.writes.metadata_pending()
            || self.metadata_failed
            || self.launch.pending_launch.is_some()
            || self.launch.creating.is_some()
            || self.launch.gate_receipt.is_some()
            || self.launch.outcome_reply;
        if (self.shutdown
            || matches!(
                self.state.lifecycle,
                Lifecycle::Exited | Lifecycle::Stopped | Lifecycle::Failed
            ))
            && self.execution.child.is_none()
            && self.execution.stopping.is_none()
            && self.history.eof
            && !self.history.flush_pending
            && !self.history.final_flush
            && self
                .writes
                .send(
                    &storage.commands,
                    crate::persistence::Command::Flush(crate::persistence::Operation::new(
                        &self.state,
                        crate::persistence::Purpose::FinalFlush,
                    )),
                )
                .is_ok()
        {
            self.history.flush_pending = true;
        }
        if self.history.capture_started_here
            && !self.history.history_sealed
            && !self.history.seal_pending
            && self.history.final_flush
            && !self.state.metadata_pending
            && self.execution.child.is_none()
            && self.execution.stopping.is_none()
            && self.history.eof
            && self.history.capture_done.is_none()
            && progress.pending_bytes == 0
            && matches!(
                self.state.lifecycle,
                Lifecycle::Stopped | Lifecycle::Exited | Lifecycle::Failed
            )
            && self.state.session_identity.is_some()
            && self
                .writes
                .send(
                    &storage.commands,
                    crate::persistence::Command::Seal(crate::persistence::Operation::new(
                        &self.state,
                        crate::persistence::Purpose::Seal,
                    )),
                )
                .is_ok()
        {
            self.history.seal_pending = true;
        }
        if let Some((request, false)) = &self.launch.cancelling
            && storage
                .commands
                .try_send(crate::persistence::Command::CancelResume {
                    target: self.state.control_target.clone(),
                    request: request.clone(),
                })
                .is_ok()
        {
            self.launch.cancelling.as_mut().unwrap().1 = true;
        }
        self.state.stopped = self.shutdown
            && self.launch.cancelling.is_none()
            && self.history.final_flush
            && self.writes.is_empty()
            && !self.metadata_failed
            && self.launch.pending_launch.is_none()
            && self.launch.creating.is_none()
            && self.launch.gate_receipt.is_none()
            && !self.launch.outcome_reply;
        if let Ok(error) = self.execution.input_errors.try_recv() {
            self.state.error = Some(format!("Native input failed: {error}; stop or retry"));
        }
        if self.execution.stopping.is_none()
            && self.state.lifecycle != Lifecycle::StopFailed
            && self.execution.owned_scope.is_some()
            && let Some(active) = self.execution.child.as_mut()
        {
            match active.try_wait() {
                Ok(Some(status)) => {
                    self.state.process_exit = Some(crate::events::ProcessExit {
                        code: status.exit_code(),
                        signal: status.signal().map(str::to_owned),
                    });
                    self.execution.writer = None;
                    self.state.lifecycle = Lifecycle::Stopping;
                    self.history.stop_drain.store(true, Ordering::Release);
                    self.execution.stopping = Some(crate::lifecycle::stop(
                        self.execution.child.take().unwrap(),
                        self.execution.owned_scope.as_ref().unwrap().clone(),
                    ));
                }
                Ok(None) => {}
                Err(error) => {
                    self.state.error = Some(format!(
                        "Cannot observe execution: {error}; stop explicitly"
                    ))
                }
            }
        }
        if let Some(receiver) = &self.execution.stopping
            && let Ok(outcome) = receiver.try_recv()
        {
            self.execution.stopping = None;
            self.execution.child = outcome.child;
            if let Some(error) = outcome.error {
                self.state.lifecycle = Lifecycle::StopFailed;
                self.state.error = Some(error);
                self.shutdown = false;
            } else {
                self.execution.owned_scope = None;
                self.launch.partial_gate = None;
                self.state.lifecycle = match self.state.process_exit.as_ref() {
                    Some(outcome) if outcome.failed() => Lifecycle::Failed,
                    Some(_) => Lifecycle::Exited,
                    None => Lifecycle::Stopped,
                };
                self.state.execution = None;
                self.history.final_flush = false;
                if self
                    .writes
                    .send(
                        &storage.commands,
                        crate::persistence::Command::Save(crate::persistence::Snapshot::new(
                            &self.state,
                            self.checkout.as_deref(),
                            &self.task_identity,
                        )),
                    )
                    .is_ok()
                {
                    self.state.metadata_pending = true;
                } else {
                    self.metadata_failed = true;
                    self.state.metadata_pending = true;
                    self.state.error = Some("Lifecycle metadata not saved; retry quit".into());
                    self.metadata_error = self.state.error.clone();
                    self.shutdown = false;
                }
            }
        }
        if self.execution.monitor.is_some() || self.execution.codex_monitor.is_some() {
            let current = self.history.screen.lock().unwrap();
            if current.output_bytes != self.state.output_bytes {
                current.snapshot(&mut self.state);
            }
        }
        if self.launch.creating.is_none()
            && self.launch.created.is_none()
            && let Some(status_monitor) = self.execution.monitor.as_mut()
        {
            self.state.status = status_monitor.poll(
                self.state.lifecycle == Lifecycle::Running,
                &self.state.screen,
                self.state.output_bytes,
            );
        }
        if self.launch.creating.is_none()
            && self.launch.created.is_none()
            && let Some(status_monitor) = self.execution.codex_monitor.as_mut()
        {
            self.state.status = status_monitor.poll(
                self.state.lifecycle == Lifecycle::Running,
                &self.state.screen,
                self.state.output_bytes,
            );
        }
        if let Some(id) = self
            .execution
            .codex_monitor
            .as_ref()
            .and_then(|m| m.native_session())
            && self.state.native_conversation.as_deref() != Some(id)
            && uuid::Uuid::parse_str(id).is_ok()
        {
            let mut saved = self.state.clone();
            saved.native_conversation = Some(id.to_owned());
            if self
                .writes
                .send(
                    &storage.commands,
                    crate::persistence::Command::Save(crate::persistence::Snapshot::new(
                        &saved,
                        self.checkout.as_deref(),
                        &self.task_identity,
                    )),
                )
                .is_ok()
            {
                self.state.native_conversation = saved.native_conversation;
                self.state.metadata_pending = true;
            }
        }
        if self.launch.prepared_ready
            && let Some(request) = self.launch.pending_launch.take()
        {
            self.launch.prepared_ready = false;
            self.handle_inner(request, true, None, storage, adoption_pending);
        }
    }
    pub(super) fn handle(
        &mut self,
        request: Request,
        storage: &crate::persistence::Worker,
        adoption_pending: bool,
    ) {
        self.handle_inner(request, false, None, storage, adoption_pending);
    }
    pub(super) fn handle_recorded(
        &mut self,
        request: Request,
        recorded: LaunchResult,
        storage: &crate::persistence::Worker,
        adoption_pending: bool,
    ) {
        self.handle_inner(request, false, Some(recorded), storage, adoption_pending);
    }
    fn handle_inner(
        &mut self,
        request: Request,
        continuing: bool,
        recorded_result: Option<LaunchResult>,
        storage: &crate::persistence::Worker,
        adoption_pending: bool,
    ) {
        if let Request::ResumeSelected {
            request,
            workspace,
            session,
        } = &request
        {
            if let Some(recorded) = recorded_result.or_else(|| self.launches.get(request).cloned())
            {
                self.state.launch_result = Some(recorded.clone());
                self.state.error = recorded.error;
                return;
            }
            if self.launch.resolving.is_some() {
                return;
            }
            if self.reserved()
                || self.metadata_failed
                || adoption_pending
                || self.writes.metadata_pending()
            {
                self.state.error = Some("Worktree occupied or prior capture/setup is unsettled; attach to its running session, or explicitly Stop and wait for settled capture before resuming another conversation".into());
                return;
            }
            if request.is_empty() || request.len() > 128 {
                self.state.error = Some("Invalid resume request identity".into());
                return;
            }
            match storage
                .commands
                .try_send(crate::persistence::Command::ResumeIdentity {
                    target: self.state.control_target.clone(),
                    request: request.clone(),
                    workspace: workspace.clone(),
                    session: session.clone(),
                    task: self.task_identity.clone(),
                }) {
                Ok(()) => {
                    self.launch.resolving = Some(request.clone());
                    self.state.error = None;
                }
                Err(error) => {
                    self.state.error =
                        Some(format!("Resume lookup busy: {error}; explicitly retry"))
                }
            }
            return;
        }
        if self.launch.resolving.is_some()
            && matches!(
                request,
                Request::Launch(_)
                    | Request::LaunchClaude(_)
                    | Request::LaunchCodex(_)
                    | Request::LaunchNative { .. }
                    | Request::Resume(_)
                    | Request::Adopt(_)
                    | Request::Reconcile
            )
        {
            self.state.error =
                Some("Selected resume is resolving; Stop cancels it before another action".into());
            return;
        }
        let directory = self.directory.clone();
        let directory = directory.as_path();
        let progress = storage.progress_for(
            self.state
                .session_identity
                .as_ref()
                .map(|identity| identity.session.as_str()),
        );
        let mut deferred = false;
        let save_history = matches!(&request, Request::History { index, .. } if *index != self.state.history_index);
        let save = save_history
            || matches!(
                request,
                Request::Adopt(_)
                    | Request::Launch(_)
                    | Request::LaunchNative { .. }
                    | Request::LaunchClaude(_)
                    | Request::Reconcile
                    | Request::LaunchCodex(_)
                    | Request::Resume(_)
                    | Request::Stop
                    | Request::Shutdown
            );
        let initial = match &request {
            Request::LaunchNative { agent, prompt, .. } => Some((*agent, prompt.clone())),
            _ => None,
        };
        let selected = match &request {
            Request::Resume(id) => self
                .launch
                .selected
                .as_ref()
                .filter(|(request, _)| request == id)
                .map(|(_, identity)| identity.clone()),
            _ => None,
        };
        let resume = matches!(&request, Request::Resume(_));
        let claude = (matches!(&request, Request::LaunchClaude(_))
            || matches!(initial, Some((crate::events::NativeAgent::Claude, _))))
            || (resume
                && selected
                    .as_ref()
                    .map_or(self.state.agent.as_deref() == Some("Claude Code"), |id| {
                        matches!(id.agent, crate::events::NativeAgent::Claude)
                    }));
        let codex = (matches!(&request, Request::LaunchCodex(_))
            || matches!(initial, Some((crate::events::NativeAgent::Codex, _))))
            || (resume
                && selected
                    .as_ref()
                    .map_or(self.state.agent.as_deref() == Some("Codex CLI"), |id| {
                        matches!(id.agent, crate::events::NativeAgent::Codex)
                    }));
        let launch_request = match &request {
            Request::LaunchNative { request: id, .. }
            | Request::Launch(id)
            | Request::LaunchClaude(id)
            | Request::LaunchCodex(id)
            | Request::Resume(id) => Some(id.clone()),
            _ => None,
        };
        let recorded = match &request {
            Request::LaunchNative { request: id, .. }
            | Request::Launch(id)
            | Request::LaunchClaude(id)
            | Request::LaunchCodex(id)
            | Request::Resume(id)
            | Request::QueryLaunch(id) => self.launches.get(id).cloned(),
            _ => None,
        }
        .or(recorded_result);
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            // A provisional launch owns its result slot until the matching
            // result commit. Another launch/query cannot replace its request or
            // result with a cached or newly rejected request in the meantime.
            if !continuing
                && self.launch.creating.is_some()
                && launch_request.is_some()
                && launch_request.as_deref() != Some(self.state.launch_request.as_str())
            {
                deferred = true;
                self.state.error = Some("Process creation still owns a launch reservation; Stop cancels intent, then wait for cleanup".into());
                return Ok(());
            }
            if !continuing
                && (self.launch.pending_launch.is_some()
                    || self.launch.creating.is_some()
                    || self.launch.gate_receipt.is_some()
                    || self.launch.outcome_reply)
                && (launch_request.is_some() || matches!(request, Request::QueryLaunch(_)))
            {
                deferred = true;
                return Ok(());
            }
            if !matches!(request, Request::Screen | Request::Resize { .. }) && !self.metadata_failed
            {
                self.state.error = None;
            }
            if let Some(recorded) = &recorded {
                self.state.launch_result = Some(recorded.clone());
                self.state.error = recorded.error.clone();
                return Ok(());
            }
            if !continuing
                && self.launch.creating.is_some()
                && matches!(
                    request,
                    Request::Adopt(_)
                        | Request::Reconcile
                        | Request::Launch(_)
                        | Request::LaunchNative { .. }
                        | Request::LaunchClaude(_)
                        | Request::LaunchCodex(_)
                        | Request::Resume(_)
                )
            {
                return Err("Process creation still owns a launch reservation; Stop cancels intent, then wait for cleanup".into());
            }
            if !continuing && self.execution.child.is_some() && launch_request.is_some() {
                return Err("A session already exists; attach to it".into());
            }
            if adoption_pending && launch_request.is_some() {
                return Err("Adoption pending; wait for the checkout result before launch".into());
            }
            if (self.launch.pending_launch.is_some() || self.launch.gate_receipt.is_some())
                && matches!(
                    request,
                    Request::Adopt(_)
                        | Request::Launch(_)
                        | Request::LaunchNative { .. }
                        | Request::LaunchClaude(_)
                        | Request::LaunchCodex(_)
                        | Request::Resume(_)
                )
            {
                return Err("Launch reconciliation pending; wait for durable outcome".into());
            }
            match request {
                Request::ResumeSelected {..} | Request::Browse(_) | Request::EditorSource{..} | Request::InspectWorking{..} | Request::PreviewCreation{..} | Request::ConfigureSetup {..} | Request::RecoverSetup {..} | Request::CreateWorkspace(_) | Request::RecoverCreation{..} | Request::CancelCreation{..} | Request::RenameWorkspace { .. } | Request::Session { .. } | Request::SelectSession(_) | Request::Discover { .. } | Request::Adopt(_) | Request::Register(_) => unreachable!("workspace requests are supervisor-owned"),
                Request::Reconcile => {
                    if self.execution.child.is_some() || self.execution.stopping.is_some() { return Err("Session owned by current supervisor; stop it normally".into()); }
                    if let Some(evidence) = &self.state.execution { evidence.reconcile()?; }
                    self.state.execution = None;
                    self.state.reconciled = true;
                    self.state.error = Some("Reconciled: no released coding execution survives; history retained. Explicitly resume with v or launch a new conversation with c/x when ready.".into());
                }
                Request::QueryLaunch(_) => return Err("Launch request unknown to this supervisor; do not replay uncertain activity after supervisor loss".into()),
                Request::LaunchNative { request, .. } | Request::Launch(request) | Request::LaunchClaude(request) | Request::LaunchCodex(request) | Request::Resume(request) => {
                    if !continuing && self.execution.child.is_none() && self.execution.stopping.is_none() && (!self.history.eof || self.history.capture_done.is_some() || self.history.flush_pending || storage.progress_for(self.state.session_identity.as_ref().map(|identity| identity.session.as_str())).pending_bytes > 0 || self.metadata_failed || (matches!(self.state.lifecycle, Lifecycle::Stopped | Lifecycle::Exited | Lifecycle::Failed) && !self.history.final_flush)) {
                        return Err("Previous session history is still pending; repair storage and press y to retry before starting another session".into());
                    }
                    if self.state.execution.is_some() && !continuing && self.execution.child.is_none() { return Err("Interrupted execution is uncertain; u reconciles process evidence. Verified reboot resolves unknown descendants; no automatic coding restart.".into()); }
                    if request.is_empty() || request.len() > 128 {
                        return Err("Launch request must contain 1–128 bytes".into());
                    }
                    if self.launches.len() >= 1024 {
                        return Err("Launch request capacity reached; retained outcomes are preserved and further launch is blocked".into());
                    }
                    self.state.launch_request = request.clone();
                    if self.execution.child.is_some() || self.execution.stopping.is_some() {
                        return Err("A session already exists; attach to it".into());
                    }
                    let path = self.checkout.as_ref().ok_or("Adopt a checkout before launch")?;
                    if !path.is_dir() { return Err("Checkout unavailable: cannot continue in place; h opens retained history. Restore the checkout before explicitly resuming.".into()); }
                    if resume && (!claude && !codex || selected.as_ref().map(|id|id.native.as_str()).or(self.state.native_conversation.as_deref()).and_then(|id| uuid::Uuid::parse_str(id).ok()).is_none()) {
                        return Err("Native resume unavailable: no supported saved conversation identity. h opens retained history; c Claude / x Codex explicitly starts a new conversation.".into());
                    }
                    if initial.as_ref().and_then(|(_, prompt)| prompt.as_ref()).is_some_and(|prompt| prompt.len() > 4084 || prompt.contains('\0')) { return Err("First prompt exceeds 4084-byte limit or contains NUL".into()); }
                    if !continuing {
                        self.history.capture_started_here = false;
                        self.history.history_sealed = false;
                        self.state.accepted_bytes = 0;
                        self.state.written_bytes = 0;
                        self.state.durable_bytes = 0;
                        self.state.capture_pending_bytes = 0;
                        self.state.capture_high_water = 0;
                        self.state.capture_backpressure_count = 0;
                        self.state.capture_backpressure_micros = 0;
                        self.state.durable_flush_count = 0;
                        self.state.durable_commits.clear();
                        self.state.storage_error = None;
                        self.state.storage_wait_millis = 0;
                        self.state.producer_wait_millis = 0;
                        self.launch.cancelled_launch = false;
                        self.state.capture_loss_possible = false;
                        if !resume { self.state.native_conversation = None; }
                        self.state.execution = Some(crate::recovery::Execution::prepared()?);
                        self.state.reconciled = false;
                        let session = uuid::Uuid::new_v4().to_string();
                        let identity = crate::events::SessionIdentity { task: self.task_identity.clone(), session, request: request.clone() };
                        self.state.identity = Some(identity.label());
                        self.state.session_identity = Some(identity);
                        // A prior segment's late status receipt cannot rewrite the new identity.
                        self.execution.monitor = None;
                        self.execution.codex_monitor = None;
                        if let Some(selected) = &selected {
                            self.state.agent = Some(match selected.agent { crate::events::NativeAgent::Claude => "Claude Code", crate::events::NativeAgent::Codex => "Codex CLI" }.into());
                            self.state.native_conversation = Some(selected.native.clone());
                        }
                        self.state.process_exit = None;
                        self.state.launch_result = None;
                        self.state.lifecycle = Lifecycle::Interrupted;
                        let mut snapshot = crate::persistence::Snapshot::new(&self.state, Some(path), &self.task_identity);
                        snapshot.workspace_name = initial.as_ref().and_then(|(_, prompt)| prompt.as_deref()).and_then(crate::registry::prompt_name);
                        self.writes.send(&storage.commands, crate::persistence::Command::Prepare(snapshot))?;
                        self.launch.pending_launch = Some(if let Some((agent, prompt)) = initial.clone() { Request::LaunchNative { request, agent, prompt } } else if resume { Request::Resume(request) } else if claude { Request::LaunchClaude(request) } else if codex { Request::LaunchCodex(request) } else { Request::Launch(request) });
                        deferred = true;
                        return Ok(());
                    }
                    let CreatedExecution { pty, child: launched, listener: gate_listener, nonce } = if let Some(result) = self.launch.created.take() {
                        result.map_err(io::Error::other)?
                    } else {
                    let (rows, cols) = self.history.screen.lock().unwrap().parser.screen().size();
                    let pty = native_pty_system().openpty(PtySize {
                        rows: rows.get(),
                        cols: cols.get(),
                        pixel_width: 0,
                        pixel_height: 0,
                    })?;
                    crate::recovery::crash_barrier(directory, "before-create");
                    super::private_directory(&self.endpoint_directory)?;
                    let gate_path = self.endpoint_directory.join("launch-gate.sock");
                    let _ = fs::remove_file(&gate_path);
                    let gate_listener = UnixListener::bind(&gate_path)?;
                    gate_listener.set_nonblocking(true)?;
                    fs::set_permissions(&gate_path, fs::Permissions::from_mode(0o600))?;
                    let nonce = uuid::Uuid::new_v4().to_string();
                    let claude_executable = if claude {
                        std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).find_map(|directory| {
                            let directory = if directory.is_absolute() { directory } else { path.join(directory) };
                            let candidate = directory.join("claude");
                            candidate.is_file().then(|| candidate.canonicalize().ok()).flatten()
                        }))
                    } else { None };
                    let mut command = if claude {
                        CommandBuilder::new(claude_executable.as_deref().unwrap_or_else(|| Path::new("claude")))
                    } else if codex {
                        let mut command = CommandBuilder::new("codex");
                        if resume {
                            command.arg("resume");
                            command.arg(self.state.native_conversation.as_ref().unwrap());
                        }
                        command.arg("--no-daemon");
                        command
                    } else {
                        let mut command = CommandBuilder::new(std::env::current_exe()?);
                        command.arg("--scripted-child");
                        command
                    };
                    if claude && resume {
                        command.arg("--resume");
                        command.arg(self.state.native_conversation.as_ref().unwrap());
                    }
                    if !resume { self.state.native_conversation = None; }
                    self.execution.monitor = None;
                    self.execution.codex_monitor = None;
                    self.state.status = crate::native_status::Status::default();
                    if codex {
                        match crate::codex_status::Monitor::start(&self.endpoint_directory) {
                            Ok(mut status_monitor) => {
                                if resume { status_monitor.expect_session(self.state.native_conversation.as_ref().unwrap()); }
                                if status_monitor.configure(&mut command).is_ok() { self.execution.codex_monitor = Some(status_monitor); }
                                else { self.state.status.reason = "Codex status monitor unavailable".into(); }
                            }
                            Err(_) => self.state.status.reason = "Codex status monitor unavailable".into(),
                        }
                    } else {
                        match crate::native_status::Monitor::start(&self.endpoint_directory, !claude, if resume { self.state.native_conversation.as_deref() } else { None }) {

                            Ok(mut status_monitor) => {
                                status_monitor.native_executable(claude_executable.as_deref());
                                status_monitor.configure(&mut command, resume);
                                if claude { self.state.native_conversation = Some(status_monitor.native_session().to_owned()); }
                                self.execution.monitor = Some(status_monitor);
                            }
                            Err(_) => {
                                self.state.status.reason = "status monitor unavailable".into();
                                if claude && !resume {
                                    let id = uuid::Uuid::new_v4().to_string();
                                    command.arg("--session-id");
                                    command.arg(&id);
                                    self.state.native_conversation = Some(id);
                                }
                            },
                        }

                    }
                    if let Some(prompt) = initial.as_ref().and_then(|(_, prompt)| prompt.as_ref()) && !prompt.is_empty() { command.arg("--"); command.arg(prompt); }
                    #[cfg(debug_assertions)]
                    if !claude && !codex { command.env("RAT_PROOF_STORAGE_DIR", directory); }
                    command.env("TERM", "xterm-256color");
                    command.cwd(path);
                    crate::recovery::wrap(&mut command, &gate_path, &nonce)?;
                    let (finished, result) = mpsc::sync_channel(1);
                    let proof_creation = directory.join("proof-launch-creation");
                    thread::Builder::new().name("session-create".into()).spawn(move || {
                        let result = pty.slave.spawn_command(command).map(|launched_child| {
                            // No fallible setup occurs between successful creation
                            // and installation in this resource owner.
                            let execution = CreatedExecution { pty, child: launched_child, listener: gate_listener, nonce };
                            #[cfg(debug_assertions)]
                            if proof_creation.exists() {
                                let _ = fs::write(proof_creation.with_extension("ready"), execution.child.process_id().unwrap_or(0).to_string());
                                while proof_creation.exists() { thread::sleep(Duration::from_millis(10)); }
                            }
                            let _ = proof_creation;
                            execution
                        }).map_err(|error| {
                            if claude {
                                format!("Claude Code launch failed: {error}. Install claude on PATH, run claude auth login in your usual terminal, then explicitly retry c; adopted checkout retained")
                            } else if codex {
                                format!("Codex CLI launch failed: {error}. Install codex on PATH, run codex login in your usual terminal, then explicitly retry x; adopted checkout retained")
                            } else {
                                format!("Fixture launch failed: {error}; retry l")
                            }
                        });
                        let _ = finished.send(result);
                    })?;
                    self.launch.creating = Some(CreatingExecution { request: if let Some((agent, prompt)) = initial.clone() { Request::LaunchNative { request, agent, prompt } } else if resume { Request::Resume(request) } else if claude { Request::LaunchClaude(request) } else if codex { Request::LaunchCodex(request) } else { Request::Launch(request) }, result });
                    deferred = true;
                    return Ok(());
                    };
                    // Transfer the actual child and terminal before any fallible
                    // identity, input or capture installation can return early.
                    self.execution.child = Some(launched);
                    self.launch.partial_gate = Some(gate_listener);
                    drop(pty.slave);
                    self.execution.master = Some(Arc::new(Mutex::new(pty.master)));
                    crate::recovery::crash_barrier(directory, "after-create");
                    self.state.agent = Some(
                        if claude {
                            "Claude Code"
                        } else if codex {
                            "Codex CLI"
                        } else {
                            "Scripted PTY"
                        }
                        .into(),
                    );
                    let process_group = self.execution.child.as_ref().unwrap()
                        .process_id()
                        .ok_or("Missing managed process identity")?
                        .try_into()?;
                    self.state.execution.as_mut().ok_or("Missing durable intent")?.pid = Some(process_group);
                    launch_setup_fault(directory, "before-scope")?;
                    self.execution.owned_scope = Some(crate::child_scope::Scope::new(process_group)?);
                    self.state.execution.as_mut().ok_or("Missing durable intent")?.identify(process_group)?;
                    launch_setup_fault(directory, "after-create")?;
                    let (release, authorized) = mpsc::sync_channel(1);
                    let (receipt, result) = mpsc::sync_channel(1);
                    self.launch.gate_release = Some(release);
                    self.launch.gate_receipt = Some(result);
                    let gate_listener = self.launch.partial_gate.take().unwrap();
                    let proof_receipt_pause = directory.join("proof-launch-receipt");
                    let gate_path = self.endpoint_directory.join("launch-gate.sock");
                    thread::spawn(move || {
                        let result = (|| -> io::Result<crate::recovery::Receipt> {
                            let deadline = std::time::Instant::now() + Duration::from_secs(2);
                            let mut stream = loop {
                                match gate_listener.accept() {
                                    Ok((stream, _)) => break stream,
                                    Err(error) if error.kind() == io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                                    Err(error) => return Err(error),
                                }
                            };
                            // macOS accepted sockets inherit nonblocking status.
                            stream.set_nonblocking(false)?;
                            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                            let mut actual = vec![0; nonce.len()];
                            stream.read_exact(&mut actual)?;
                            if actual != nonce.as_bytes() { return Err(io::Error::other("Launch gate nonce mismatch")); }
                            authorized.recv().map_err(|_| io::Error::other("Launch authorization lost"))?;
                            stream.write_all(&[1])?;
                            let mut error = String::new();
                            stream.take(4096).read_to_string(&mut error)?;
                            if !error.is_empty() { return Ok(crate::recovery::Receipt::ExecFailed(error)); }
                            Ok(crate::recovery::Receipt::Executed)
                        })().unwrap_or_else(|e| crate::recovery::Receipt::Unknown(e.to_string()));
                        let _ = fs::remove_file(gate_path);
                        #[cfg(debug_assertions)]
                        while proof_receipt_pause.exists() { thread::sleep(Duration::from_millis(10)); }
                        let _ = proof_receipt_pause;
                        let _ = receipt.send(result);
                    });
                    launch_setup_fault(directory, "before-input")?;
                    let mut sink = self.execution.master.as_ref().unwrap().lock().unwrap().take_writer()?;
                    let (input, input_writes) = mpsc::sync_channel::<Vec<u8>>(32);
                    let errors = self.execution.write_errors.clone();
                    thread::spawn(move || {
                        while let Ok(bytes) = input_writes.recv() {
                            if let Err(error) = sink.write_all(&bytes) {
                                let _ = errors.try_send(error.to_string());
                                break;
                            }
                        }
                    });
                    self.execution.writer = Some(input);
                    self.state.lifecycle = Lifecycle::Running;
                    self.state.stopped = false;
                    self.state.history_index = 0;
                    self.history.history_sealed = false;
                    self.history.seal_pending = false;
                    let session = self.state.session_identity.as_ref().ok_or("Missing prepared identity")?.session.clone();
                    launch_setup_fault(directory, "before-capture")?;
                    let mut source = self.execution.master.as_ref().unwrap().lock().unwrap().try_clone_reader()?;
                    let capture = storage.capture_for(&session)?;
                    self.history.capture_started_here = true;
                    let (finished, done) = mpsc::sync_channel(1);
                    self.history.capture_done = Some(done);
                    self.history.eof = false;
                    self.history.final_flush = false;
                    let parser = self.history.screen.clone();
                    self.history.stop_drain.store(false, Ordering::Release);
                    let drain = self.history.stop_drain.clone();
                    thread::spawn(move || {
                        // A macOS PTY leader can await terminal output drain even
                        // after SIGKILL. Stop alone reserves a finite 1MiB raw tail;
                        // the single reader preserves byte order and retries it
                        // before reporting EOF. Exhaustion keeps backpressure and
                        // bounded lifecycle verification still reports failure.
                        const STOP_TAIL: usize = 1024 * 1024;
                        const STOP_BLOCKS: usize = 256;
                        let mut tail = VecDeque::new();
                        let mut retained = 0;
                        let mut bytes = [0; 4096];
                        while let Ok(n) = source.read(&mut bytes) {
                            if n == 0 {
                                break;
                            }
                            capture.accept(n);
                            let block = (session.clone(), bytes[..n].to_vec(), std::time::Instant::now());
                            if tail.is_empty() {
                                match capture.send(block, Some(&drain)) {
                                    Ok(Some(block)) => { retained += n; tail.push_back(block); }
                                    Ok(None) => {}
                                    Err(()) => break,
                                }
                            } else {
                                retained += n;
                                tail.push_back(block);
                            }
                            if retained >= STOP_TAIL || tail.len() >= STOP_BLOCKS {
                                let block = tail.pop_front().unwrap();
                                retained -= block.1.len();
                                if capture.send(block, None).is_err() { break; }
                            }
                            let mut capture_screen = parser.lock().unwrap();
                            capture_screen.parser.process(&bytes[..n]);
                            capture_screen.output_bytes = capture_screen.output_bytes.saturating_add(n as u64);
                        }
                        for block in tail {
                            if capture.send(block, None).is_err() { return; }
                        }
                        let _ = finished.send(());
                    });
                    launch_setup_fault(directory, "before-authorization")?;
                    if self.launch.cancelled_launch {
                        self.launch.gate_release = None;
                        self.execution.writer = None;
                        self.state.lifecycle = Lifecycle::Stopping;
                        self.history.stop_drain.store(true, Ordering::Release);
                        self.execution.stopping = Some(crate::lifecycle::stop(self.execution.child.take().unwrap(), self.execution.owned_scope.as_ref().unwrap().clone()));
                        return Err("Launch cancelled by Stop before authorization".into());
                    }
                    self.writes.send(&storage.commands, crate::persistence::Command::Authorize(crate::persistence::Snapshot::new(&self.state, Some(path), &self.task_identity)))?;
                    deferred = true;
                }
                Request::Input(bytes) => {
                    if let Some(status_monitor) = self.execution.monitor.as_mut() { status_monitor.input(self.history.screen.lock().unwrap().output_bytes, bytes.as_slice() == [3]); }
                    if let Some(status_monitor) = self.execution.codex_monitor.as_mut() { status_monitor.input(self.history.screen.lock().unwrap().output_bytes); }
                    if bytes.len() > 4096 {
                        return Err("Input exceeds prototype 4096-byte limit".into());
                    }
                    self.execution.writer
                        .as_mut()
                        .ok_or("No session; launch first")?
                        .try_send(bytes)
                        .map_err(|e| format!("Native input queue unavailable: {e}; retry"))?;
                }
                Request::Key(key) => {
                    if let Some(status_monitor) = self.execution.monitor.as_mut() { status_monitor.input(self.history.screen.lock().unwrap().output_bytes, matches!(key.code, crossterm::event::KeyCode::Char('c' | 'C')) && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT)); }
                    if let Some(status_monitor) = self.execution.codex_monitor.as_mut() { status_monitor.input(self.history.screen.lock().unwrap().output_bytes); }
                    let application_cursor =
                        self.history.screen.lock().unwrap().parser.screen().application_cursor();
                    let bytes = crate::native_input::encode(key, application_cursor)?;
                    self.execution.writer
                        .as_mut()
                        .ok_or("No session; launch first")?
                        .try_send(bytes)
                        .map_err(|e| format!("Native input queue unavailable: {e}; retry"))?;
                }
                Request::Paste(text) => {
                    if let Some(status_monitor) = self.execution.monitor.as_mut() { status_monitor.input(self.history.screen.lock().unwrap().output_bytes, false); }
                    if let Some(status_monitor) = self.execution.codex_monitor.as_mut() { status_monitor.input(self.history.screen.lock().unwrap().output_bytes); }
                    if text.len() > 4084 {
                        return Err("Paste exceeds 4084-byte limit".into());
                    }
                    let bracketed = self.history.screen.lock().unwrap().parser.screen().bracketed_paste();
                    let bytes = if bracketed {
                        format!("\x1b[200~{text}\x1b[201~").into_bytes()
                    } else {
                        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
                    };
                    self.execution.writer
                        .as_mut()
                        .ok_or("No session; launch first")?
                        .try_send(bytes)
                        .map_err(|e| format!("Native input queue unavailable: {e}; retry"))?;
                }
                Request::Resize { rows, cols } => {
                    if rows == 0 || cols == 0 {
                        return Ok(());
                    }
                    let (rows, cols) = (rows.clamp(1, MAX_ROWS), cols.clamp(1, MAX_COLS));
                    // Serialize geometry changes against output parsing. Resize the PTY
                    // first; output induced by SIGWINCH cannot enter the old parser size.
                    let mut resized_screen = self.history.screen.lock().unwrap();
                    if let Some(terminal) = self.execution.master.as_ref() {
                        terminal.lock().unwrap().resize(PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        })?;
                    }
                    resized_screen
                        .parser
                        .set_size(rows.try_into().unwrap(), cols.try_into().unwrap());
                }
                Request::HistorySession { session, offset } => {
                    self.request_history(0, Some(session), offset, storage)?;
                }
                Request::History { index, offset } => {
                    self.state.history_index = index;
                    self.request_history(index, if index == 0 { self.state.session_identity.as_ref().map(|identity| identity.session.clone()) } else { None }, offset, storage)?;
                }
                Request::Screen => {}
                Request::Cleanup { page } => {
                    self.state.cleanup = None;
                    storage.commands.try_send(crate::persistence::Command::Cleanup(page)).map_err(|e| format!("Cleanup busy: {e}; retry"))?;
                }
                Request::DeleteHistory { id } => {
                    if self.state.session_identity.as_ref().map(|identity| identity.session.as_str()) == Some(&id)
                        && (self.execution.child.is_some() || self.execution.stopping.is_some() || !self.history.eof || self.history.capture_done.is_some() || !self.history.final_flush || self.state.metadata_pending || progress.pending_bytes > 0)
                    { return Err("Selected history still has active, uncertain or unflushed capture".into()); }
                    self.state.cleanup_message = Some(format!("Deleting explicitly selected history {id}; completion pending"));
                    storage.commands.try_send(crate::persistence::Command::Delete(id)).map_err(|e| format!("Cleanup busy: {e}; retry"))?;
                }
                Request::RetryStorage => {
                    self.retry_requested = true;
                    self.history.final_flush = false;
                }
                Request::Stop | Request::Shutdown => {
                    if let Some(request) = self.launch.resolving.take() {
                        self.launch.cancelling = Some((request,false));
                    }
                    self.launch.selected = None;
                    if self.state.storage_wait_millis >= 250 || self.state.producer_wait_millis > 0 || self.state.storage_error.is_some() { self.state.capture_loss_possible = true; }
                    if (self.launch.pending_launch.is_some() || self.launch.gate_receipt.is_some()) && self.launch.creating.is_none() {
                        let result = LaunchResult { request: self.state.launch_request.clone(), identity: self.state.identity.clone(), error: Some("Launch cancelled by Stop; metadata may still be pending".into()) };
                        self.launches.insert(result.request.clone(), result.clone());
                        self.state.launch_result = Some(result);
                    }
                    let cancelled_preparation = self.launch.pending_launch.is_some() && self.launch.creating.is_none() && self.execution.child.is_none() && self.launch.gate_receipt.is_none();
                    self.launch.cancelled_launch = true;
                    self.launch.pending_launch = None;
                    self.launch.prepared_ready = false;
                    if cancelled_preparation { self.state.execution = None; }
                    self.launch.gate_release = None;
                    if self.launch.creating.is_none() && self.execution.child.is_none() && self.execution.stopping.is_none() && self.state.execution.is_some() {
                        return Err("Interrupted execution remains uncertain; u reconciles evidence, d detaches. A missing native leader may require verified reboot before replacement or confirmed shutdown.".into());
                    }
                    self.shutdown |= matches!(request, Request::Shutdown);
                    if self.execution.stopping.is_none() {
                        if self.execution.owned_scope.is_none() && let Some(active) = self.execution.child.as_ref() {
                            let pid = active.process_id().ok_or("Missing managed process identity; stop unconfirmed")?.try_into()?;
                            self.execution.owned_scope = Some(crate::child_scope::Scope::new(pid).map_err(|error| format!("Stop unconfirmed: {error}; child retained; retry Stop"))?);
                        }
                        if let Some(active) = self.execution.child.take() {
                            self.execution.writer = None;
                                    self.state.lifecycle = Lifecycle::Stopping;
                            self.history.stop_drain.store(true, Ordering::Release);
                            self.execution.stopping = Some(crate::lifecycle::stop(active, self.execution.owned_scope.as_ref().unwrap().clone()));
                        } else {
                            self.history.final_flush = false;
                        }
                    }
                    if self.launch.creating.is_some() {
                        self.state.lifecycle = Lifecycle::Stopping;
                        self.state.error = Some("Launch cancelled by Stop; process creation still outstanding, cleanup pending".into());
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            // A failed setup must never release its gated child, including a
            // delayed authorization completion from the storage worker.
            if continuing && launch_request.is_some() && !deferred {
                self.launch.cancelled_launch = true;
                self.launch.gate_release = None;
            }
            self.state.error = Some(error.to_string());
        }
        let fresh_launch = launch_request.is_some() && recorded.is_none();
        if let Some(request) = launch_request
            && !deferred
            && recorded.is_none()
            && request.len() <= 128
            && !request.is_empty()
            && self.launches.len() < 1024
        {
            let result = LaunchResult {
                request: request.clone(),
                identity: if self.state.error.is_none() {
                    self.state.identity.clone()
                } else {
                    None
                },
                error: self.state.error.clone(),
            };
            self.launches.insert(request, result.clone());
            self.state.launch_result = Some(result);
        }
        if save && !deferred {
            let snapshot = if fresh_launch {
                crate::persistence::Snapshot::result(
                    &self.state,
                    self.checkout.as_deref(),
                    &self.task_identity,
                )
            } else {
                crate::persistence::Snapshot::new(
                    &self.state,
                    self.checkout.as_deref(),
                    &self.task_identity,
                )
            };
            match self.writes.send(
                &storage.commands,
                crate::persistence::Command::Save(snapshot),
            ) {
                Ok(operation) => {
                    self.state.metadata_pending = true;
                    if fresh_launch {
                        self.launch.outcome_operation = Some(operation);
                        self.launch.outcome_reply = true;
                    }
                }
                Err(error) => {
                    self.metadata_failed = true;
                    if fresh_launch {
                        self.launch.result_failed = true;
                        self.launch.outcome_reply = true;
                    }
                    self.state.metadata_pending = true;
                    self.state.error = Some(format!("Metadata not saved: {error}; retry"));
                    self.metadata_error = self.state.error.clone();
                    self.shutdown = false;
                }
            }
        }
        self.history
            .screen
            .lock()
            .unwrap()
            .snapshot(&mut self.state);
        if self.launch.creating.is_none()
            && self.launch.created.is_none()
            && let Some(status_monitor) = self.execution.monitor.as_mut()
        {
            self.state.status = status_monitor.poll(
                self.state.lifecycle == Lifecycle::Running,
                &self.state.screen,
                self.state.output_bytes,
            );
        }
    }

    fn request_history(
        &mut self,
        index: usize,
        session: Option<String>,
        offset: u64,
        storage: &crate::persistence::Worker,
    ) -> Result<(), String> {
        let query = (index, session.clone(), offset);
        let unchanged = self.history_query.as_ref() == Some(&query);
        if unchanged && self.pending_history.is_some() {
            return Ok(());
        }
        let request = uuid::Uuid::new_v4();
        storage
            .commands
            .try_send(crate::persistence::Command::HistoryTarget {
                target: self.state.control_target.clone(),
                request,
                index,
                session,
                offset,
            })
            .map_err(|error| format!("History busy: {error}; retry"))?;
        self.pending_history = Some(request);
        self.history_query = Some(query);
        if !unchanged {
            self.state.history = None;
        }
        Ok(())
    }
    pub(super) fn observation(&mut self) -> Observation {
        // Reuse the bounded cached projection when no output/geometry changed.
        // A polling supervisor must not format the same screen on every pass.
        let screen = self.history.screen.lock().unwrap();
        let (rows, cols) = screen.parser.screen().size();
        if screen.output_bytes != self.state.output_bytes
            || rows.get() != self.state.rows
            || cols.get() != self.state.cols
        {
            screen.snapshot(&mut self.state);
        }
        self.state.metadata_pending = self.launch.resolving.is_some()
            || self.launch.cancelling.is_some()
            || self.writes.metadata_pending()
            || self.metadata_failed
            || self.launch.pending_launch.is_some()
            || self.launch.creating.is_some()
            || self.launch.gate_receipt.is_some()
            || self.launch.outcome_reply;
        let mut observation = self.state.clone();
        if let Some(request) = &self.launch.resolving {
            // Transport recovery follows the frozen new request while saved
            // historical metadata still belongs to the previous segment.
            observation.launch_request = request.clone();
            observation.launch_result = None;
        }
        observation
    }
    pub(super) fn resolved_resume(&self, request: &str) -> bool {
        self.launch
            .selected
            .as_ref()
            .is_some_and(|(id, _)| id == request)
    }
    pub(super) fn resolve_resume(
        &mut self,
        target: &str,
        request: String,
        result: Result<crate::persistence::ResumeIdentity, String>,
    ) -> Option<Request> {
        if target != self.state.control_target || self.launch.resolving.as_deref() != Some(&request)
        {
            return None;
        }
        self.launch.resolving = None;
        match result {
            Ok(identity) => {
                self.launch.selected = Some((request.clone(), identity));
                Some(Request::Resume(request))
            }
            Err(error) => {
                self.state.error = Some(error);
                None
            }
        }
    }
    pub(super) fn report_error(&mut self, error: String) {
        self.state.error = Some(error);
    }
    pub(super) fn clear_error(&mut self) {
        if !self.metadata_failed {
            self.state.error = None;
        }
    }
    pub(super) fn recorded_launch(&self, request: &str) -> Option<LaunchResult> {
        if self.launch.outcome_reply && self.state.launch_request == request {
            return None;
        }
        self.launches.get(request).cloned()
    }
    pub(super) fn pending_launch_request(&self, request: &str) -> bool {
        self.launch.resolving.as_deref() == Some(request)
            || self
                .launch
                .cancelling
                .as_ref()
                .is_some_and(|(id, _)| id == request)
            || (self.state.launch_request == request
                && (self.launch.pending_launch.is_some()
                    || self.launch.creating.is_some()
                    || self.launch.created.is_some()
                    || self.launch.gate_receipt.is_some()
                    || self.launch.outcome_reply))
    }
    pub(super) fn adopt(
        &mut self,
        identity: crate::git::CheckoutIdentity,
        same_checkout: bool,
        task: String,
        storage: &crate::persistence::Worker,
    ) {
        if !same_checkout {
            self.state.identity = None;
            self.state.session_identity = None;
            self.state.process_exit = None;
            self.state.lifecycle = Lifecycle::Idle;
            self.state.native_conversation = None;
            self.state.agent = None;
            self.task_identity = task;
        }
        self.state.adopted = Some(identity.root.display().to_string());
        self.checkout = Some(identity.root.clone());
        self.state.checkout_identity = Some(identity);
        self.save_metadata(storage);
    }
    pub(super) fn adoption_failed(&mut self, error: String, storage: &crate::persistence::Worker) {
        self.report_error(error);
        self.save_metadata(storage);
    }
    pub(super) fn reserved(&self) -> bool {
        self.launch.resolving.is_some()
            || self.launch.cancelling.is_some()
            || self.state.execution.is_some()
            || !self.history.eof
            || self.history.capture_done.is_some()
            || self.history.flush_pending
            || (self.history.capture_started_here && !self.history.history_sealed)
            || self.execution.child.is_some()
            || self.execution.stopping.is_some()
            || self.launch.pending_launch.is_some()
            || self.launch.creating.is_some()
            || self.launch.created.is_some()
            || self.launch.gate_receipt.is_some()
    }
    pub(super) fn task(&self) -> &str {
        &self.task_identity
    }
    pub(super) fn checkout(&self) -> Option<&Path> {
        self.checkout.as_deref()
    }
    fn save_metadata(&mut self, storage: &crate::persistence::Worker) {
        match self.writes.send(
            &storage.commands,
            crate::persistence::Command::Save(crate::persistence::Snapshot::new(
                &self.state,
                self.checkout.as_deref(),
                &self.task_identity,
            )),
        ) {
            Ok(_) => self.state.metadata_pending = true,
            Err(error) => {
                self.metadata_failed = true;
                self.state.metadata_pending = true;
                self.state.error = Some(format!("Metadata not saved: {error}; retry"));
                self.metadata_error = self.state.error.clone();
            }
        }
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        self.execution.monitor.take();
        self.execution.codex_monitor.take();
        let _ = fs::remove_file(self.endpoint_directory.join("launch-gate.sock"));
        let _ = fs::remove_dir(&self.endpoint_directory);
    }
}

#[cfg(test)]
mod launch_replay_tests {
    use super::*;

    #[test]
    fn another_owners_cached_result_cannot_replace_a_provisional_launch() {
        let directory = tempfile::tempdir().unwrap();
        let (worker, restored) = crate::persistence::Worker::start(directory.path()).unwrap();
        let mut owner = AgentSession::restore(directory.path(), restored, &worker).unwrap();
        owner.handle(Request::Launch("own pending result".into()), &worker, false);
        let before = owner.observation();
        assert!(before.metadata_pending);
        for request in [
            Request::QueryLaunch("another owners request".into()),
            Request::Launch("another owners request".into()),
        ] {
            owner.handle_recorded(
                request,
                LaunchResult {
                    request: "another owners request".into(),
                    identity: Some("another identity".into()),
                    error: Some("another error".into()),
                },
                &worker,
                false,
            );
            let after = owner.observation();
            assert_eq!(after.control_target, before.control_target);
            assert_eq!(
                serde_json::to_value(after.session_identity).unwrap(),
                serde_json::to_value(&before.session_identity).unwrap()
            );
            assert_eq!(after.launch_request, before.launch_request);
            assert_eq!(
                serde_json::to_value(after.launch_result).unwrap(),
                serde_json::to_value(&before.launch_result).unwrap()
            );
            assert_eq!(after.error, before.error);
            assert!(after.metadata_pending);
        }
    }
}

#[cfg(test)]
mod readable_history_tests {
    use super::*;
    use crate::persistence::{Completion, HistoryPage, Worker};

    #[test]
    fn repeated_history_poll_retains_the_inflight_reply_and_completed_page() {
        let directory = tempfile::tempdir().unwrap();
        let (worker, restored) = Worker::start(directory.path()).unwrap();
        let mut owner = AgentSession::restore(directory.path(), restored, &worker).unwrap();
        let poll = || Request::History {
            index: 0,
            offset: 0,
        };
        owner.handle(poll(), &worker, false);
        let Completion::HistoryTarget(target, request, _) =
            worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("expected history reply")
        };
        owner.handle(poll(), &worker, false);
        owner.complete(
            Completion::HistoryTarget(
                target.clone(),
                request,
                Ok(HistoryPage {
                    text: "retained raw page".into(),
                    projection_preparing: true,
                    ..Default::default()
                }),
            ),
            &worker,
        );
        assert_eq!(
            owner
                .observation()
                .history
                .as_ref()
                .map(|page| page.text.as_str()),
            Some("retained raw page")
        );
        owner.handle(poll(), &worker, false);
        owner.complete(
            Completion::HistoryTarget(
                target,
                request,
                Ok(HistoryPage {
                    text: "retained raw page".into(),
                    readable: Some(crate::history::ReadablePage {
                        text: "completed readable page".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            ),
            &worker,
        );
        assert_eq!(
            owner.observation().history.unwrap().readable.unwrap().text,
            "completed readable page"
        );
        owner.handle(poll(), &worker, false);
        assert_eq!(
            owner
                .observation()
                .history
                .as_ref()
                .map(|page| page.text.as_str()),
            Some("retained raw page")
        );
        let Completion::HistoryTarget(target, refreshed, _) =
            worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("expected same-passage refresh")
        };
        assert_ne!(refreshed, request);
        owner.complete(
            Completion::HistoryTarget(
                target,
                refreshed,
                Ok(HistoryPage {
                    text: "new committed suffix".into(),
                    committed: 42,
                    readable: Some(crate::history::ReadablePage {
                        text: "fresh readable suffix".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            ),
            &worker,
        );
        let refreshed = owner.observation().history.unwrap();
        assert_eq!(refreshed.committed, 42);
        assert_eq!(refreshed.text, "new committed suffix");
        assert_eq!(refreshed.readable.unwrap().text, "fresh readable suffix");
    }

    #[test]
    fn failed_readable_stage_clears_preparing_page_and_allows_same_passage_retry() {
        let directory = tempfile::tempdir().unwrap();
        let (worker, restored) = Worker::start(directory.path()).unwrap();
        let mut owner = AgentSession::restore(directory.path(), restored, &worker).unwrap();
        let poll = || Request::History {
            index: 0,
            offset: 0,
        };
        owner.handle(poll(), &worker, false);
        let Completion::HistoryTarget(target, request, _) =
            worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("expected history reply")
        };
        owner.complete(
            Completion::HistoryTarget(
                target.clone(),
                request,
                Ok(HistoryPage {
                    projection_preparing: true,
                    ..Default::default()
                }),
            ),
            &worker,
        );
        owner.complete(
            Completion::HistoryTarget(
                target.clone(),
                request,
                Err("Selected history was deleted".into()),
            ),
            &worker,
        );
        assert!(owner.observation().history.is_none());
        assert!(owner.observation().error.unwrap().contains("deleted"));
        owner.handle(poll(), &worker, false);
        let Completion::HistoryTarget(retried_target, retried, _) =
            worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("expected explicit retry reply")
        };
        assert_eq!(retried_target, target);
        assert_ne!(retried, request);
        assert!(owner.observation().error.is_none());
    }

    #[test]
    fn late_readable_results_and_errors_cannot_replace_a_newer_selected_passage() {
        let directory = tempfile::tempdir().unwrap();
        let (worker, restored) = Worker::start(directory.path()).unwrap();
        let mut owner = AgentSession::restore(directory.path(), restored, &worker).unwrap();
        owner.handle(
            Request::History {
                index: 0,
                offset: 0,
            },
            &worker,
            false,
        );
        let Completion::HistoryTarget(target, old, _) =
            worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("expected history reply");
        };
        owner.handle(
            Request::History {
                index: 0,
                offset: 2048,
            },
            &worker,
            false,
        );
        let Completion::HistoryTarget(current_target, current, _) =
            worker.events.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("expected second history reply");
        };
        owner.complete(
            Completion::HistoryTarget(
                target.clone(),
                old,
                Ok(HistoryPage {
                    text: "stale text".into(),
                    ..Default::default()
                }),
            ),
            &worker,
        );
        owner.complete(
            Completion::HistoryTarget(target, old, Err("stale error".into())),
            &worker,
        );
        assert!(owner.observation().history.is_none());
        assert!(owner.observation().error.is_none());
        owner.complete(
            Completion::HistoryTarget(
                "another owner".into(),
                current,
                Ok(HistoryPage {
                    text: "wrong session".into(),
                    ..Default::default()
                }),
            ),
            &worker,
        );
        assert!(owner.observation().history.is_none());
        owner.complete(
            Completion::HistoryTarget(
                current_target,
                current,
                Ok(HistoryPage {
                    offset: 2048,
                    text: "selected passage".into(),
                    ..Default::default()
                }),
            ),
            &worker,
        );
        assert_eq!(
            owner.observation().history.unwrap().text,
            "selected passage"
        );
    }
}
