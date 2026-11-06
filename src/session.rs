//! Supervisor transport, controller ownership and execution-owned Git coordination.
//! Concrete native session progression lives in `agent_session`.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

mod agent_session;

pub const ROWS: u16 = 24;
pub const COLS: u16 = 100;
pub const MAX_ROWS: u16 = 80;
pub const MAX_COLS: u16 = 200;
const MAX_FRAME: usize = 4 * 1024 * 1024;
pub use crate::events::{LaunchResult, Lifecycle, Observation, Request};
pub struct Client {
    pub requests: SyncSender<Request>,
    pub events: Receiver<Observation>,
}
impl Client {
    pub fn start(directory: PathBuf) -> io::Result<Self> {
        let (requests, rx) = mpsc::sync_channel(32);
        let (tx, events) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("session-transport".into())
            .spawn(move || {
                if let Err(error) = transport(directory, rx, &tx) {
                    let _ = tx.send(Observation {
                        connection_lost: true,
                        error: Some(format!("{error}. Dashboard disconnected; press d to detach locally, then reopen. Process state unknown; stop unconfirmed.")),
                        ..Observation::default()
                    });
                }
            })?;
        Ok(Self { requests, events })
    }
    pub fn send(&self, request: Request) -> Result<(), String> {
        self.requests
            .try_send(request)
            .map_err(|e| format!("Session queue unavailable: {e}; retry the action"))
    }
}
fn transport(
    directory: PathBuf,
    requests: Receiver<Request>,
    events: &SyncSender<Observation>,
) -> Result<(), Box<dyn std::error::Error>> {
    private_directory(&directory)?;
    let socket = directory.join("supervisor.sock");
    // Every startup participates in election, including recovery of a stale socket.
    let mut supervisor = Command::new(std::env::current_exe()?);
    // SAFETY: the child only calls the async-signal-safe setsid before exec.
    unsafe {
        supervisor.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut owner = supervisor
        .arg("--supervisor")
        .arg(&directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    thread::spawn(move || {
        let _ = owner.wait();
    });
    let mut connection = None;
    for _ in 0..100 {
        match UnixStream::connect(&socket) {
            Ok(stream) => {
                connection = Some(stream);
                break;
            }
            Err(_) => thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut stream = connection
        .ok_or("Cannot acquire supervisor; ownership unresolved. Verify the private state directory; do not remove a live endpoint")?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut greeting: Observation = serde_json::from_slice(&read_frame(&mut reader)?)?;
    decode_screen(&mut greeting);
    if greeting.control_denied {
        if greeting.diff_protocol != 1
            || greeting.git_jobs_protocol != 1
            || greeting.registry_protocol != 1
            || greeting.sessions_protocol != 1
            || greeting.prompt_naming_protocol != 1
        {
            events.send(greeting)?;
            return Ok(());
        }
        drop(reader);
        drop(stream);
        events.send(greeting.clone())?;
        while let Ok(request) = requests.recv() {
            greeting.discovery = None;
            match request {
                Request::Discover {
                    request,
                    repository,
                } => {
                    let path = PathBuf::from(std::ffi::OsString::from_vec(repository.clone()));
                    let result = crate::git::read_only_discovery(&path);
                    greeting.discovery = Some(crate::git::Discovery {
                        request,
                        repository,
                        result,
                    });
                }
                Request::Shutdown => greeting.stopped = true, // local exit only; no owner control
                _ => {} // a refused dashboard cannot send any session command
            }
            let stopped = greeting.stopped;
            events.send(greeting.clone())?;
            if stopped {
                return Ok(());
            }
        }
        return Ok(());
    }
    if greeting.diff_protocol != 1
        || greeting.git_jobs_protocol != 1
        || greeting.registry_protocol != 1
        || greeting.sessions_protocol != 1
        || greeting.prompt_naming_protocol != 1
    {
        greeting.control_denied = true;
        greeting.error = Some("Existing supervisor does not support this dashboard's worktree, registry or session protocol. Use its compatible dashboard to stop activity before upgrading; d detaches locally.".into());
        events.send(greeting)?;
        return Ok(());
    }
    events.send(greeting)?;
    loop {
        let request = match requests.recv_timeout(Duration::from_millis(40)) {
            Ok(request) => request,
            Err(mpsc::RecvTimeoutError::Timeout) => Request::Screen,
            Err(_) => return Ok(()),
        };
        let reliable = !matches!(request, Request::Screen);
        serde_json::to_writer(&mut stream, &request)?;
        stream.write_all(b"\n")?;
        let mut observation: Observation = serde_json::from_slice(&read_frame(&mut reader)?)?;
        decode_screen(&mut observation);
        if reliable || observation.stopped {
            let stopped = observation.stopped;
            events.send(observation)?;
            if stopped {
                return Ok(());
            }
            continue;
        }
        // A lagging UI receives the next coherent snapshot, never a growing output queue.
        match events.try_send(observation) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => {}
            Err(_) => return Ok(()),
        }
    }
}

fn decode_screen(observation: &mut Observation) {
    if observation.rows == 0 || observation.cols == 0 {
        return;
    }
    let mut parser = vt100::Parser::new(
        observation.rows.clamp(1, MAX_ROWS).try_into().unwrap(),
        observation.cols.clamp(1, MAX_COLS).try_into().unwrap(),
        0,
    );
    parser.process(observation.formatted.as_bytes());
    observation.decoded = Some(parser.screen().clone());
}

fn read_frame(reader: &mut impl BufRead) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_FRAME + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if bytes.len() > MAX_FRAME || bytes.last() != Some(&b'\n') {
        return Err(io::Error::other("Invalid/oversized supervisor frame"));
    }
    Ok(bytes)
}

pub fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}

// A directory supplied by another user or through a symlink is never repaired.
fn private_directory(directory: &Path) -> io::Result<()> {
    if !directory.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
    }
    let metadata = fs::symlink_metadata(directory)?;
    // SAFETY: geteuid has no preconditions and does not access Rust memory.
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::other(
            "Supervisor state must be a user-owned private directory (0700), without a symlink",
        ));
    }
    Ok(())
}

fn elect_owner(directory: &Path) -> io::Result<Option<File>> {
    private_directory(directory)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(directory.join("supervisor.lock"))?;
    let metadata = lock.metadata()?;
    // SAFETY: geteuid has no preconditions.
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::other("Supervisor ownership lock is not private"));
    }
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(error) => Err(io::Error::other(error)),
    }
}

fn send_observation(stream: &mut UnixStream, state: &Observation) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, state)?;
    stream.write_all(b"\n")
}

// Exactly one finite response in flight. Partial writes retain their cursor;
// the supervisor never waits for a subscriber and never starts a second frame
// before finishing the first. A stalled subscriber must reconnect to resync.
struct Delivery {
    bytes: Vec<u8>,
    offset: usize,
    since: Instant,
    stopped: bool,
}
impl Delivery {
    fn new(state: &mut Observation) -> io::Result<Self> {
        let mut visible = state.clone();
        if visible.metadata_pending {
            visible.launch_result = None;
        }
        let mut bytes = serde_json::to_vec(&visible)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME {
            return Err(io::Error::other("Supervisor snapshot exceeds frame cap"));
        }
        state.subscriber_frame_high_water = state.subscriber_frame_high_water.max(bytes.len());
        Ok(Self {
            bytes,
            offset: 0,
            since: Instant::now(),
            stopped: state.stopped,
        })
    }
    fn advance(&mut self, stream: &mut UnixStream) -> io::Result<bool> {
        if self.since.elapsed() >= Duration::from_millis(500) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Slow dashboard disconnected after 500ms; reopen to resynchronize the current screen; history capture continues",
            ));
        }
        // Bound transport work per supervisor pass as well as retained bytes.
        let end = (self.offset + 65536).min(self.bytes.len());
        match stream.write(&self.bytes[self.offset..end]) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "Dashboard closed")),
            Ok(n) => self.offset += n,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        Ok(self.offset == self.bytes.len())
    }
}

// Runtime selection belongs to the supervisor. Each concrete owner retains its
// private launch, cleanup and capture phases while another owner is selected.
struct Sessions {
    entries: Vec<(String, agent_session::AgentSession)>,
    selected: usize,
    directory: PathBuf,
    selection_pending: Option<uuid::Uuid>,
}
impl std::ops::Deref for Sessions {
    type Target = agent_session::AgentSession;
    fn deref(&self) -> &Self::Target {
        &self.entries[self.selected].1
    }
}
impl std::ops::DerefMut for Sessions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries[self.selected].1
    }
}
impl Sessions {
    fn restore(
        directory: &Path,
        restored: crate::persistence::Restored,
        storage: &crate::persistence::Worker,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut restored = restored;
        let others = std::mem::take(&mut restored.sessions);
        let selected_id = restored
            .state
            .session_identity
            .as_ref()
            .map(|id| id.session.clone());
        if let Some(id) = &selected_id
            && let Some(known) = others.iter().find(|entry| {
                entry
                    .state
                    .session_identity
                    .as_ref()
                    .is_some_and(|identity| &identity.session == id)
            })
        {
            restored.task = known.task.clone();
        }
        let selected_task = restored.task.clone();
        let launches = restored.launches.clone();
        let mut owner = agent_session::AgentSession::restore(directory, restored, storage)?;
        let target = owner.observation().control_target;
        let mut entries = vec![(target, owner)];
        for recovered in others {
            let recovered_id = recovered
                .state
                .session_identity
                .as_ref()
                .map(|id| id.session.clone());
            if recovered_id == selected_id
                && (selected_id.is_some() || recovered.task == selected_task)
            {
                continue;
            }
            let restored = crate::persistence::Restored {
                sessions: Vec::new(),
                state: recovered.state,
                checkout: recovered.checkout.as_deref().map(path_bytes),
                task: recovered.task.clone(),
                launches: launches
                    .iter()
                    .filter(|result| {
                        result.identity.as_ref().is_some_and(|label| {
                            label.starts_with(&format!("task={} session=", recovered.task))
                        })
                    })
                    .cloned()
                    .collect(),
            };
            let mut owner = agent_session::AgentSession::restore(directory, restored, storage)?;
            let target = owner.observation().control_target;
            entries.push((target, owner));
        }
        let selection_pending = if entries[0].1.checkout().is_some() {
            let operation = uuid::Uuid::new_v4();
            storage
                .commands
                .try_send(crate::persistence::Command::SelectWorkspace(
                    operation,
                    selected_task,
                ))?;
            Some(operation)
        } else {
            None
        };
        Ok(Self {
            entries,
            selected: 0,
            directory: directory.to_owned(),
            selection_pending,
        })
    }

    fn complete(
        &mut self,
        event: crate::persistence::Completion,
        storage: &crate::persistence::Worker,
    ) {
        if let crate::persistence::Completion::Selected(operation, result) = event {
            if self.selection_pending != Some(operation) {
                return;
            }
            self.selection_pending = None;
            if let Err(error) = result {
                self.report_error(format!(
                    "Workspace selection not saved: {error}; retry selection"
                ));
            }
            return;
        }
        // Attributed completions are ignored by owners without the matching
        // obligation. Projection replies belong to the selected owner only.
        if matches!(
            event,
            crate::persistence::Completion::Committed(_)
                | crate::persistence::Completion::Failed(..)
                | crate::persistence::Completion::Deleted(_)
                | crate::persistence::Completion::HistoryTarget(..)
        ) {
            for (_, owner) in &mut self.entries {
                owner.complete(event.clone(), storage);
            }
        } else {
            self.entries[self.selected].1.complete(event, storage);
        }
    }
    fn advance(&mut self, storage: &crate::persistence::Worker, adopting: bool) {
        for (_, owner) in &mut self.entries {
            owner.advance(storage, adopting);
        }
    }
    fn observation(&mut self) -> Observation {
        let mut summaries = Vec::new();
        for (target, owner) in &mut self.entries {
            let state = owner.observation();
            summaries.push(crate::events::SessionSummary {
                status: state.status.clone(),
                reserved: owner.reserved(),
                target: target.clone(),
                task: owner.task().to_owned(),
                session: state.session_identity.map(|id| id.session),
                lifecycle: state.lifecycle,
                checkout: owner.checkout().map(path_bytes),
            });
        }
        let mut state = self.entries[self.selected].1.observation();
        state.sessions_protocol = 1;
        state.control_target = self.entries[self.selected].0.clone();
        state.sessions = summaries;
        state.metadata_pending |= self.selection_pending.is_some();
        state
    }
    fn all_stopped(&mut self) -> bool {
        self.selection_pending.is_none()
            && self
                .entries
                .iter_mut()
                .all(|(_, owner)| owner.observation().stopped)
    }
    fn adopt_checkout(
        &mut self,
        identity: crate::git::CheckoutIdentity,
        task: String,
        storage: &crate::persistence::Worker,
    ) -> Result<(), String> {
        let matching = |owner: &mut agent_session::AgentSession| {
            let state = owner.observation();
            state.checkout_identity.as_ref().is_some_and(|known| {
                known.git_dir == identity.git_dir && known.common_dir == identity.common_dir
            }) || owner.checkout() == Some(identity.root.as_path())
        };
        let index = self
            .entries
            .iter_mut()
            .position(|(_, owner)| owner.reserved() && matching(owner))
            .or_else(|| {
                self.entries
                    .iter_mut()
                    .position(|(_, owner)| matching(owner))
            });
        if let Some(index) = index {
            self.selected = index;
            self.entries[index]
                .1
                .adopt(identity, true, task.clone(), storage);
            let operation = uuid::Uuid::new_v4();
            storage
                .commands
                .try_send(crate::persistence::Command::SelectWorkspace(
                    operation, task,
                ))
                .map_err(|error| {
                    format!("Workspace selection not saved: {error}; retry selection")
                })?;
            self.selection_pending = Some(operation);
            return Ok(());
        }
        if self.entries.len() >= 1024 {
            return Err("Session owner capacity reached; retained owners are preserved".into());
        }
        let restored = crate::persistence::Restored {
            sessions: Vec::new(),
            state: Observation::default(),
            checkout: None,
            task: task.clone(),
            launches: Vec::new(),
        };
        let mut owner = agent_session::AgentSession::restore(&self.directory, restored, storage)
            .map_err(|e| e.to_string())?;
        owner.adopt(identity, false, task.clone(), storage);
        let operation = uuid::Uuid::new_v4();
        storage
            .commands
            .try_send(crate::persistence::Command::SelectWorkspace(
                operation, task,
            ))
            .map_err(|error| format!("Workspace selection not saved: {error}; retry selection"))?;
        self.selection_pending = Some(operation);
        let target = owner.observation().control_target;
        self.entries.push((target, owner));
        self.selected = self.entries.len() - 1;
        Ok(())
    }
    fn handle(&mut self, request: Request, storage: &crate::persistence::Worker, adopting: bool) {
        let (index, request) = match request {
            Request::Session {
                target,
                session,
                command,
            } => {
                let Some(index) = self.entries.iter().position(|(id, _)| *id == target) else {
                    self.report_error(
                        "Session target no longer exists; refresh before retrying".into(),
                    );
                    return;
                };
                let current = self.entries[index]
                    .1
                    .observation()
                    .session_identity
                    .map(|id| id.session);
                if current != session {
                    self.entries[index].1.report_error(
                        "Session changed before command delivery; refresh before retrying".into(),
                    );
                    return;
                }
                if matches!(
                    *command,
                    Request::Session { .. }
                        | Request::SelectSession(_)
                        | Request::Shutdown
                        | Request::Adopt(_)
                        | Request::Register(_)
                        | Request::RenameWorkspace { .. }
                        | Request::Discover { .. }
                        | Request::InspectWorking { .. }
                ) {
                    self.report_error("Invalid targeted session command".into());
                    return;
                }
                (index, *command)
            }
            Request::SelectSession(target) => {
                if let Some(index) = self.entries.iter().position(|(id, _)| *id == target) {
                    self.selected = index;
                    let operation = uuid::Uuid::new_v4();
                    if let Err(error) =
                        storage
                            .commands
                            .try_send(crate::persistence::Command::SelectWorkspace(
                                operation,
                                self.entries[index].1.task().to_owned(),
                            ))
                    {
                        self.report_error(format!(
                            "Workspace selection not saved: {error}; retry selection"
                        ));
                    } else {
                        self.selection_pending = Some(operation);
                    }
                } else {
                    self.report_error("Session target no longer exists".into());
                }
                return;
            }
            Request::Shutdown => {
                for (_, owner) in &mut self.entries {
                    owner.handle(Request::Shutdown, storage, adopting);
                }
                return;
            }
            request => (self.selected, request),
        };
        let launch_request = match &request {
            Request::Launch(id)
            | Request::LaunchClaude(id)
            | Request::LaunchCodex(id)
            | Request::Resume(id)
            | Request::QueryLaunch(id)
            | Request::LaunchNative { request: id, .. } => Some(id),
            _ => None,
        };
        if let Some(id) = launch_request
            && self
                .entries
                .iter()
                .enumerate()
                .any(|(other, (_, owner))| other != index && owner.pending_launch_request(id))
        {
            self.entries[index].1.report_error("This launch request still has an unsettled result in another session; wait for its durable outcome before retrying".into());
            return;
        }
        if let Some(id) = launch_request
            && let Some(result) = self
                .entries
                .iter()
                .find_map(|(_, owner)| owner.recorded_launch(id))
        {
            self.entries[index].1.remember_launch(result);
            self.entries[index].1.handle(request, storage, adopting);
            return;
        }
        if matches!(
            request,
            Request::LaunchNative { .. }
                | Request::Launch(_)
                | Request::LaunchClaude(_)
                | Request::LaunchCodex(_)
                | Request::Resume(_)
        ) {
            let active = self
                .entries
                .iter()
                .filter(|(_, owner)| owner.reserved())
                .count();
            if active >= 32 && !self.entries[index].1.reserved() {
                self.entries[index].1.report_error(
                    "At most 32 unsettled session executions or capture streams can be admitted"
                        .into(),
                );
                return;
            }
            let target_checkout = self.entries[index].1.observation().checkout_identity;
            if target_checkout.is_some_and(|target| {
                self.entries
                    .iter_mut()
                    .enumerate()
                    .any(|(other, (_, owner))| {
                        other != index
                            && owner.reserved()
                            && owner.observation().checkout_identity.is_some_and(|known| {
                                known.git_dir == target.git_dir
                                    && known.common_dir == target.common_dir
                            })
                    })
            }) {
                self.entries[index].1.report_error("This canonical worktree already has an active or uncertain session; attach or reconcile it first".into());
                return;
            }
        }
        self.entries[index].1.handle(request, storage, adopting);
    }
}

pub fn supervisor(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let Some(_ownership) = elect_owner(directory)? else {
        return Ok(());
    };
    let socket = directory.join("supervisor.sock");
    if let Ok(metadata) = fs::symlink_metadata(&socket) {
        if !metadata.file_type().is_socket() {
            return Err("Supervisor endpoint is not a socket; refusing recovery".into());
        }
        // Only a refused connection proves a stale socket. Other failures leave
        // ownership uncertain and must never displace an existing endpoint.
        match UnixStream::connect(&socket) {
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {}
            _ => {
                return Err(
                    "Supervisor ownership is unresolved; refusing to displace a live endpoint"
                        .into(),
                );
            }
        }
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let (storage, restored) = match crate::persistence::Worker::start(directory) {
        Ok(ready) => ready,
        Err(error) => {
            let failure = Observation {
                error: Some(format!(
                    "History storage unavailable: {error}; repair storage then reopen the dashboard"
                )),
                control_denied: true,
                ..Observation::default()
            };
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                if let Ok((mut incoming, _)) = listener.accept() {
                    let _ = send_observation(&mut incoming, &failure);
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
            fs::remove_file(&socket)?;
            return Ok(());
        }
    };
    let mut registry = restored.state.registry.clone();
    let mut registry_error = None;
    let mut registry_operation = None;
    let mut registering = false;
    let mut session = Sessions::restore(directory, restored, &storage)?;
    let mut adoption: Option<crate::git::AdoptionJob> = None;
    let mut discovery: Option<crate::git::GitWorker> = None;
    let mut pending_discovery: Option<(u64, PathBuf)> = None;
    let mut discovery_result = None;
    let mut diff_job: Option<crate::working_diff::Job> = None;
    let mut diff_result = None;
    let mut adoption_pending = false;
    let mut quitting = false;
    let mut subscriber_frame_high_water = 0;
    let mut controller: Option<UnixStream> = None;
    let mut delivery: Option<Delivery> = None;
    let mut pending = Vec::new();
    loop {
        if diff_job.as_ref().is_some_and(|job| job.is_finished())
            && let Some(job) = diff_job.take()
        {
            diff_result = job.result.try_recv().ok();
        }
        if adoption
            .as_ref()
            .is_some_and(|job| !adoption_pending && job.is_finished())
        {
            adoption = None;
        }
        if discovery
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            discovery = None;
        }
        if let Some(worker) = &discovery {
            while let Ok(crate::events::Event::DiscoveryCompleted {
                request,
                repository,
                result,
            }) = worker.events.try_recv()
            {
                if pending_discovery.as_ref() != Some(&(request, repository.clone())) {
                    continue;
                }
                pending_discovery = None;
                discovery_result = Some(crate::git::Discovery {
                    request,
                    repository: path_bytes(&repository),
                    result,
                });
            }
        }
        if let Some(result) = adoption
            .as_ref()
            .filter(|_| adoption_pending)
            .and_then(|job| job.result.try_recv().ok())
        {
            adoption = None;
            adoption_pending = false;
            if registering {
                registering = false;
                let operation = uuid::Uuid::new_v4();
                match result {
                    Ok(identity) => match storage
                        .commands
                        .try_send(crate::persistence::Command::Register(operation, identity))
                    {
                        Ok(()) => registry_operation = Some(operation),
                        Err(error) => {
                            registry_error = Some(format!(
                                "Project not registered / workspace name not saved: {error}; retry"
                            ))
                        }
                    },
                    Err(error) => registry_error = Some(error),
                }
                continue;
            }
            let result = result.and_then(|identity| {
                let task = registry
                    .checkout(&identity)
                    .map(|workspace| workspace.task.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                session.adopt_checkout(identity, task, &storage)?;
                Ok(())
            });
            if let Err(error) = result {
                session.adoption_failed(error, &storage);
            }
        }
        while let Ok(event) = storage.events.try_recv() {
            match event {
                crate::persistence::Completion::Registry(snapshot) => registry = snapshot,
                crate::persistence::Completion::Selected(operation, result) => {
                    session.complete(
                        crate::persistence::Completion::Selected(operation, result.clone()),
                        &storage,
                    );
                    if let Ok(snapshot) = result {
                        registry = snapshot;
                    }
                }
                crate::persistence::Completion::Registered(operation, result) => {
                    if registry_operation != Some(operation) {
                        continue;
                    }
                    registry_operation = None;
                    match result {
                        Ok(snapshot) => registry = snapshot,
                        Err(error) => {
                            registry_error = Some(format!(
                                "Project not registered / workspace name not saved: {error}; retry"
                            ))
                        }
                    }
                }
                event => session.complete(event, &storage),
            }
        }
        session.advance(&storage, adoption.is_some());
        let mut state = session.observation();
        state.subscriber_frame_high_water = subscriber_frame_high_water;
        state.diff_protocol = 1;
        state.working_diff = diff_result.clone();
        state.git_jobs_protocol = 1;
        state.registry_protocol = 1;
        state.prompt_naming_protocol = 1;
        state.registry_pending = registering || registry_operation.is_some();
        state.registry = registry.clone();
        state.error = state.error.or_else(|| registry_error.clone());
        state.adoption_pending = adoption_pending;
        state.discovery = discovery_result.clone();
        state.stopped = session.all_stopped()
            && quitting
            && adoption.is_none()
            && discovery.is_none()
            && diff_job.is_none()
            && registry_operation.is_none();
        if state.stopped && controller.is_none() {
            fs::remove_file(&socket)?;
            return Ok(());
        }
        // Accept/reject independently of the active client's commands or output.
        for _ in 0..32 {
            match listener.accept() {
                Ok((mut incoming, _)) => {
                    incoming.set_nonblocking(true)?;
                    if controller.is_some() {
                        let refusal = Observation {
                            diff_protocol: 1,
                            git_jobs_protocol: 1,
                            registry_protocol: 1,
                            sessions_protocol: 1,
                            prompt_naming_protocol: 1,
                            control_denied: true,
                            error: Some("Another dashboard controls this supervisor; close it before reconnecting".into()),
                            ..Observation::default()
                        };
                        let _ = send_observation(&mut incoming, &refusal);
                    } else {
                        delivery = Some(Delivery::new(&mut state)?);
                        subscriber_frame_high_water =
                            subscriber_frame_high_water.max(delivery.as_ref().unwrap().bytes.len());
                        controller = Some(incoming);
                        pending.clear();
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        if let Some(stream) = controller.as_mut()
            && let Some(response) = delivery.as_mut()
        {
            let stopped = response.stopped;
            match response.advance(stream) {
                Ok(true) => {
                    delivery = None;
                }
                Ok(false) => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => {
                    if error.kind() == io::ErrorKind::TimedOut {
                        session.report_error(error.to_string());
                    }
                    controller = None;
                    delivery = None;
                    pending.clear();
                    continue;
                }
            }
            if stopped {
                fs::remove_file(&socket)?;
                return Ok(());
            }
        }
        let Some(stream) = controller.as_mut() else {
            thread::sleep(Duration::from_millis(5));
            continue;
        };
        let mut incoming = [0; 4096];
        match stream.read(&mut incoming) {
            Ok(0) => {
                controller = None;
                pending.clear();
                continue;
            }
            Ok(n) => pending.extend_from_slice(&incoming[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => {
                controller = None;
                pending.clear();
                continue;
            }
        }
        if pending.len() > MAX_FRAME {
            controller = None;
            pending.clear();
            continue;
        }
        let Some(end) = pending.iter().position(|byte| *byte == b'\n') else {
            thread::sleep(Duration::from_millis(5));
            continue;
        };
        let bytes: Vec<_> = pending.drain(..=end).collect();
        let request: Request = match serde_json::from_slice(&bytes) {
            Ok(request) => request,
            Err(_) => {
                controller = None;
                pending.clear();
                continue;
            }
        };
        if matches!(&request, Request::Session {command, ..} if matches!(**command, Request::Stop))
            && let Some(job) = &diff_job
        {
            job.cancel();
        }
        match request {
            Request::InspectWorking {
                request,
                workspace,
                selected,
            } => {
                let result = (|| -> Result<(), String> {
                    if request.len() > 128
                        || workspace.len() > 256
                        || selected
                            .as_ref()
                            .is_some_and(|path| path.len() > 32768 || path.contains(&0))
                    {
                        return Err("Invalid or oversized comparison identity/path".into());
                    }
                    if diff_job.is_some() {
                        return Err("Comparison job busy; retry refresh".into());
                    }
                    let known = registry
                        .workspaces
                        .iter()
                        .find(|known| known.id == workspace)
                        .ok_or("Workspace unavailable; refresh registry")?;
                    let checkout = known.checkout.as_ref().ok_or("Checkout identity unavailable; explicitly adopt an existing checkout first")?;
                    diff_job = Some(
                        crate::working_diff::Job::start(
                            checkout.clone(),
                            workspace.clone(),
                            request.clone(),
                            selected,
                        )
                        .map_err(|error| error.to_string())?,
                    );
                    diff_result = None;
                    Ok(())
                })();
                if let Err(error) = result {
                    diff_result = Some(crate::working_diff::Response {
                        workspace,
                        request,
                        result: Err(error),
                    });
                }
            }
            Request::Discover {
                request,
                repository,
            } => {
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    if repository.len() > 32768 {
                        return Err("Repository path exceeds 32768 bytes".into());
                    }
                    let repository = PathBuf::from(std::ffi::OsString::from_vec(repository));
                    if discovery.is_none() {
                        discovery = Some(crate::git::GitWorker::start(repository.clone())?);
                    }
                    if let Err(error) = discovery
                        .as_ref()
                        .unwrap()
                        .refresh_for(request, repository.clone())
                    {
                        discovery_result = Some(crate::git::Discovery {
                            request,
                            repository: path_bytes(&repository),
                            result: Err(error),
                        });
                        return Ok(());
                    }
                    pending_discovery = Some((request, repository));
                    discovery_result = None;
                    Ok(())
                })();
                if let Err(error) = result {
                    session.report_error(error.to_string());
                }
            }
            Request::RenameWorkspace { workspace, name } => {
                registry_error = None;
                if registry_operation.is_some() || adoption.is_some() {
                    registry_error =
                        Some("Workspace operation pending; wait before renaming".into());
                } else {
                    let operation = uuid::Uuid::new_v4();
                    match storage
                        .commands
                        .try_send(crate::persistence::Command::Rename(
                            operation, workspace, name,
                        )) {
                        Ok(()) => registry_operation = Some(operation),
                        Err(error) => {
                            registry_error =
                                Some(format!("Workspace name not saved: {error}; retry"))
                        }
                    }
                }
            }
            Request::Register(bytes) => {
                registry_error = None;
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    if bytes.len() > 32768 {
                        return Err("Repository path exceeds 32768 bytes".into());
                    }
                    if adoption.is_some() || registry_operation.is_some() {
                        return Err("Project operation pending; wait or Stop to cancel".into());
                    }
                    adoption = Some(crate::git::AdoptionJob::start(PathBuf::from(
                        std::ffi::OsString::from_vec(bytes),
                    ))?);
                    registering = true;
                    adoption_pending = true;
                    session.clear_error();
                    Ok(())
                })();
                if let Err(error) = result {
                    registry_error = Some(error.to_string());
                }
            }
            Request::Adopt(bytes) => {
                registry_error = None;
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    if bytes.len() > 32768 {
                        return Err("Checkout path exceeds 32768 bytes".into());
                    }
                    if adoption.is_some() || registry_operation.is_some() {
                        return Err("Adoption already pending; wait or Stop to cancel".into());
                    }
                    adoption = Some(crate::git::AdoptionJob::start(PathBuf::from(
                        std::ffi::OsString::from_vec(bytes),
                    ))?);
                    adoption_pending = true;
                    session.clear_error();
                    Ok(())
                })();
                if let Err(error) = result {
                    session.report_error(error.to_string());
                }
            }
            Request::Stop | Request::Shutdown => {
                if let Some(job) = &diff_job {
                    job.cancel();
                }
                if let Some(job) = &adoption {
                    job.cancel();
                }
                adoption_pending = false;
                registering = false;
                if let Some(worker) = &discovery {
                    worker.cancel();
                }
                if let Some((request, repository)) = pending_discovery.take() {
                    discovery_result = Some(crate::git::Discovery {
                        request,
                        repository: path_bytes(&repository),
                        result: Err("Git discovery cancelled by Stop".into()),
                    });
                }
                quitting |= matches!(request, Request::Shutdown);
                session.handle(request, &storage, adoption.is_some());
            }
            request => session.handle(request, &storage, adoption.is_some()),
        }
        let mut state = session.observation();
        state.subscriber_frame_high_water = subscriber_frame_high_water;
        state.diff_protocol = 1;
        state.working_diff = diff_result.clone();
        state.git_jobs_protocol = 1;
        state.registry_protocol = 1;
        state.prompt_naming_protocol = 1;
        state.registry_pending = registering || registry_operation.is_some();
        state.registry = registry.clone();
        state.error = state.error.or_else(|| registry_error.clone());
        state.adoption_pending = adoption_pending;
        state.discovery = discovery_result.clone();
        state.stopped = session.all_stopped()
            && quitting
            && adoption.is_none()
            && discovery.is_none()
            && diff_job.is_none()
            && registry_operation.is_none();
        if controller.is_some() {
            delivery = Some(Delivery::new(&mut state)?);
            subscriber_frame_high_water =
                subscriber_frame_high_water.max(delivery.as_ref().unwrap().bytes.len());
        }
    }
}

pub fn scripted_child() -> io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    struct RawGuard;
    impl Drop for RawGuard {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    let _guard = RawGuard;
    println!(
        "Scripted PTY · pid={} · cwd={}\r",
        std::process::id(),
        std::env::current_dir()?.display()
    );
    // Split both UTF-8 and a mode sequence to exercise incremental screen parsing.
    for bytes in [b"\xce".as_slice(), b"\xbb\r\n\x1b[?20", b"04hfixture> "] {
        io::stdout().write_all(bytes)?;
        io::stdout().flush()?;
    }
    let mut line = Vec::new();
    let mut approval = false;
    let mut flood_run = 0;
    let mut native_active = false;
    let mut paced: Option<std::thread::JoinHandle<io::Result<()>>> = None;
    for byte in io::stdin().lock().bytes() {
        let byte = byte?;
        match byte {
            3 => {
                line.clear();
                if native_active {
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n  ⎿  Interrupted · What should Claude do instead?\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle)"
                    );
                    native_active = false;
                } else {
                    println!("interrupted: 03\r");
                    print!("fixture> ");
                }
            }
            b'\r' => {
                if line == b"exit" {
                    if let Some(producer) = paced.take() {
                        producer
                            .join()
                            .map_err(|_| io::Error::other("Paced producer panicked"))??;
                    }
                    break;
                }
                if line == b"survive" {
                    // Isolated crash fixture: remain alive when the owning PTY closes.
                    unsafe {
                        libc::signal(libc::SIGHUP, libc::SIG_IGN);
                    }
                    println!("survival enabled\r");
                    io::stdout().flush()?;
                    loop {
                        thread::sleep(Duration::from_secs(60));
                    }
                } else if line == b"no-read" {
                    println!("input paused\r");
                    io::stdout().flush()?;
                    loop {
                        thread::sleep(Duration::from_secs(60));
                    }
                } else if line == b"tool-child" {
                    use std::os::unix::process::CommandExt;
                    let mut command = Command::new("python3");
                    command.args(["-c", "import os,signal,time\nsignal.signal(signal.SIGHUP,signal.SIG_IGN)\nsignal.signal(signal.SIGTERM,signal.SIG_IGN)\nopen('tool-child.pid','w').write(str(os.getpid()))\nfor i in range(300):\n with open('tool-child.heartbeat','a') as f: f.write('beat\\n')\n time.sleep(.02)\n"]);
                    command.process_group(0);
                    command
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());
                    let mut descendant = command.spawn()?;
                    thread::spawn(move || {
                        let _ = descendant.wait();
                    });
                } else if line == b"descendant" {
                    // Both leader and descendant refuse TERM so bounded KILL is exercised.
                    ignore_term();
                    let mut command = Command::new(std::env::current_exe()?);
                    command.arg("--scripted-descendant");
                    let mut descendant = command.spawn()?;
                    thread::spawn(move || {
                        let _ = descendant.wait();
                    });
                } else if line == b"size" {
                    let (cols, rows) = crossterm::terminal::size()?;
                    print!("size: {cols}x{rows}\r\nfixture> ");
                } else if line == b"fixture-failed-test" {
                    println!("test result: FAILED; agent continues\r");
                } else if line == b"status-native-submit" {
                    crate::native_status::fixture("UserPromptSubmit");
                    print!("\x1b[2J\x1b[HClaude Code v2.1.318\r\n❯ proof\r\n");
                } else if line == b"status-native-active"
                    || line == b"status-native-old-ack"
                    || line == b"status-native-tool-active"
                {
                    native_active = true;
                    if line == b"status-native-tool-active" {
                        crate::native_status::fixture("FixtureNativeBashPre");
                    }
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n⏺ Ownership controls when values are destroyed.\r\n\r\n✢ Noodling…\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt"
                    );
                } else if line == b"status-native-blocked-stop" {
                    crate::native_status::fixture("Stop");
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n✢ Running Stop hooks…\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt"
                    );
                } else if line == b"status-native-old-done" {
                    crate::native_status::fixture("UserPromptSubmit");
                    io::stdout().flush()?;
                    thread::sleep(Duration::from_millis(80));
                    crate::native_status::fixture("Stop");
                    println!("\r");
                } else if line == b"status-native-done" || line == b"status-native-quoted-done" {
                    if line == b"status-native-done" {
                        crate::native_status::fixture("Stop");
                    }
                    io::stdout().flush()?;
                    thread::sleep(Duration::from_millis(80));
                    print!(
                        "\x1b[2J\x1b[HClaude Code v2.1.318\r\n⏺ proof answer\r\n\r\n✻ Cogitated for 1s · done 3:12 PM\r\n\r\n────────────────────────────────────────\r\n❯ \r\n────────────────────────────────────────\r\n  ⏵⏵ auto mode on (shift+tab to cycle) · ← for agents"
                    );
                } else if line == b"status-native-post" {
                    crate::native_status::fixture("PostToolUse");
                    println!("question answered; old dialog text is retained\r");
                } else if line == b"status-native-approval" {
                    crate::native_status::fixture("PostToolUse");
                    crate::native_status::fixture("FixtureNativeBashPre");
                    crate::native_status::fixture("FixtureNativeBashPermission");
                    print!(
                        "\x1b[2J\x1b[H────────────────────────────────────────\r\n Bash command\r\n Do you want to proceed?\r\n ❯ 1. Yes\r\n   2. No\r\n Esc to cancel · Tab to amend"
                    );
                } else if line == b"status-native-question" {
                    crate::native_status::fixture("PreToolUse");
                    print!(
                        "\x1b[2J\x1b[H────────────────────────────────────────\r\n ☐ Proof marker\r\nShould the proof marker be alpha or beta?\r\n❯ 1. Alpha\r\n  2. Beta\r\n────────────────────────────────────────\r\nEnter to select · ↑/↓ to navigate · Esc to cancel"
                    );
                } else if line.starts_with(b"status-") {
                    let event = match line.as_slice() {
                        b"status-working" => "FixtureWorking",
                        b"status-input" => "FixtureInput",
                        b"status-approval" => "FixtureApproval",
                        b"status-completed" => "FixtureCompleted",
                        b"status-interrupted" => "FixtureInterrupted",
                        b"status-wrong-session" => "FixtureWrongSession",
                        b"status-old-stop" => "FixtureOldStop",
                        b"status-prior-turn-stop" => "FixturePreviousTurnStop",
                        _ => "FixtureInvalid",
                    };
                    crate::native_status::fixture(event);
                    println!("fixture status evidence emitted\r");
                } else if line == b"screen" {
                    print!("\x1b[2J\x1b[H\x1b[1;31mstyled ");
                    io::stdout().flush()?;
                    for bytes in [
                        b"\xce".as_slice(),
                        b"\xbb\xe7\x95",
                        b"\x8c\x1b[0",
                        b"m\r\nfixture> ",
                    ] {
                        io::stdout().write_all(bytes)?;
                        io::stdout().flush()?;
                    }
                } else if line == b"alternate" {
                    print!(
                        "\x1b[?1049h\x1b[2J\x1b[H\x1b[?1003h\x1b[?1006halternate screen\r\nfixture> "
                    );
                } else if line == b"primary" {
                    print!("\x1b[?1003l\x1b[?1006l\x1b[?1049l\r\nprimary restored\r\nfixture> ");
                } else if line == b"mosaic" {
                    let (cols, rows) = crossterm::terminal::size()?;
                    for row in 1..=rows {
                        for col in 1..=cols {
                            print!(
                                "\x1b[{row};{col}H\x1b[1;38;2;{};{};7;48;2;1;2;3mX",
                                row % 256,
                                col % 256
                            );
                        }
                    }
                    print!("\x1b[0m\x1b[{rows};1Hmax geometry complete");
                } else if line == b"stream" {
                    for n in 0..200 {
                        println!("stream: {n:04}\r");
                        io::stdout().flush()?;
                        thread::sleep(Duration::from_millis(5));
                    }
                    print!("fixture> ");
                } else if line == b"osc13" {
                    let mut stdout = io::stdout().lock();
                    stdout.write_all(b"\x1b]2;")?;
                    for _ in 0..2048 {
                        stdout.write_all(&[b'q'; 4096])?;
                    }
                    stdout.write_all(b"\x07osc13-complete\r\nfixture> ")?;
                } else if let Some(length) = line.strip_prefix(b"pad14 ") {
                    let length = std::str::from_utf8(length)
                        .ok()
                        .and_then(|value| value.parse::<usize>().ok())
                        .filter(|length| *length <= 65536)
                        .ok_or_else(|| io::Error::other("Invalid proof padding"))?;
                    io::stdout().write_all(&vec![b'R'; length])?;
                    print!("fixture> ");
                } else if let Some(config) = line.strip_prefix(b"pace17 ") {
                    if paced
                        .as_ref()
                        .is_some_and(|producer| !producer.is_finished())
                    {
                        println!("pace17 busy; one producer maximum\r");
                        line.clear();
                        continue;
                    }
                    if let Some(previous) = paced.take() {
                        previous
                            .join()
                            .map_err(|_| io::Error::other("Paced producer panicked"))??;
                    }
                    let values: Vec<u64> = std::str::from_utf8(config)
                        .map_err(io::Error::other)?
                        .split_whitespace()
                        .map(str::parse)
                        .collect::<std::result::Result<_, _>>()
                        .map_err(io::Error::other)?;
                    if values.len() != 3
                        || values[1] > 300_000
                        || !(1024..=4_194_304).contains(&values[2])
                    {
                        return Err(io::Error::other(
                            "pace17 requires run duration_ms(<=300000) bytes_per_second(1024..4194304)",
                        ));
                    }
                    paced = Some(thread::spawn(move || {
                        paced_fixture(values[0], values[1], values[2])
                    }));
                } else if let Some(nonce) = line.strip_prefix(b"ack17 ") {
                    // Input receipt precedes stdout acquisition: full storage can
                    // block its visible ACK without blocking receipt itself.
                    let received = fixture_clock()?;
                    let nonce = String::from_utf8_lossy(nonce);
                    if let Some(mut ledger) = fixture_ledger("proof-input17.jsonl") {
                        writeln!(
                            ledger,
                            "{}",
                            serde_json::json!({"nonce":nonce,"received_ns":received})
                        )?;
                    }
                    let mut stdout = io::stdout().lock();
                    write!(
                        stdout,
                        "\x1b[s\x1b[1;1H\x1b[2KACK17 {nonce} received_ns={received}\x1b[u"
                    )?;
                    stdout.flush()?;
                } else if line == b"flood13" {
                    flood_run += 1;
                    let start_ns = fixture_clock()?;
                    let since = Instant::now();
                    let mut max_write_micros = 0;
                    let payload = "X".repeat(111);
                    let mut stdout = io::stdout().lock();
                    let mut ledger = std::env::var_os("RAT_PROOF_STORAGE_DIR")
                        .map(PathBuf::from)
                        .filter(|path| path.join("proof-storage-ledger").is_file())
                        .and_then(|path| {
                            OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path.join("proof-emitted.jsonl"))
                                .ok()
                        });
                    for number in 0..32768 {
                        let record = format!("flood13:{number:06}:{payload}\r\n");
                        let write = Instant::now();
                        stdout.write_all(record.as_bytes())?;
                        let micros = write.elapsed().as_micros();
                        max_write_micros = max_write_micros.max(micros);
                        if let Some(ledger) = ledger.as_mut() {
                            // Format the complete bounded receipt before touching
                            // the file: formatting Value directly into File emits
                            // many writes that Stop can interrupt inside the JSON.
                            let receipt = format!(
                                "{}\n",
                                serde_json::json!({"record":number,"bytes":record.len(),"write_us":micros,"elapsed_us":since.elapsed().as_micros()})
                            );
                            ledger.write_all(receipt.as_bytes())?;
                        }
                    }
                    writeln!(
                        stdout,
                        "flood13-complete run={flood_run} bytes=4194304 elapsed_us={} max_write_us={max_write_micros} start_ns={start_ns} finish_ns={}\r",
                        since.elapsed().as_micros(),
                        fixture_clock()?
                    )?;
                    write!(stdout, "fixture> ")?;
                } else if line == b"history-bulk" {
                    for number in 0..6000 {
                        println!(
                            "bulk08:{number:05}:abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789--\r"
                        );
                    }
                    println!("history-bulk-complete\r");
                    print!("fixture> ");
                } else if line == b"history-controls" {
                    print!(
                        "erased-response-08\r\x1b[2Kreplacement-08\r\n\x1b[?1049halternate-response-08\x1b[?1049lafter-history-controls\r\nfixture> "
                    );
                } else if line == b"identity" {
                    println!("same-child pid={}\r", std::process::id());
                    print!("fixture> ");
                } else if line == b"cursor-on" {
                    print!("\x1b[?1hcursor application\r\nfixture> ");
                } else if line == b"paste-off" {
                    print!("\x1b[?2004lpaste plain\r\nfixture> ");
                } else if approval {
                    println!("approval: {}\r", if line == b"y" { "yes" } else { "no" });
                    approval = false;
                } else if line == b"approve" {
                    print!("Approve action? [y/n] ");
                    approval = true;
                } else {
                    if line == b"attach13-nonce" {
                        println!("input13-received_ns={}\r", fixture_clock()?);
                    }
                    println!(
                        "received: {}\r",
                        String::from_utf8_lossy(&line).escape_debug()
                    );
                    let hex: String = line.iter().map(|b| format!("{b:02x}")).collect();
                    println!("bytes: {hex}\r");
                    print!("fixture> ");
                }
                line.clear();
            }
            byte => {
                if line.len() < 4096 {
                    line.push(byte);
                } else {
                    return Err(io::Error::other("Fixture input exceeds 4096 bytes"));
                }
            }
        }
        // Ordinary input collection must not acquire stdout: a producer can
        // legitimately be blocked there by preserved-history backpressure.
        if matches!(byte, 3 | b'\r') {
            io::stdout().flush()?;
        }
    }
    print!("\x1b[?2004l");
    io::stdout().flush()
}

fn fixture_ledger(name: &str) -> Option<std::fs::File> {
    std::env::var_os("RAT_PROOF_STORAGE_DIR")
        .map(PathBuf::from)
        .filter(|path| path.join("proof-storage-ledger").is_file())
        .and_then(|path| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path.join(name))
                .ok()
        })
}

// A native-style producer: one finite worker, one <=1024-byte stdout lock,
// pacing outside that lock, and no catch-up burst after backpressure.
fn paced_fixture(run: u64, duration_ms: u64, rate: u64) -> io::Result<()> {
    let started = Instant::now();
    let start_ns = fixture_clock()?;
    let period = Duration::from_secs_f64(1024.0 / rate as f64);
    let mut ledger = fixture_ledger("proof-emitted17.jsonl");
    let mut sequence = 0u64;
    let mut max_write_us = 0;
    let mut write_us = 0;
    let mut ledger_us = 0;
    let mut max_ledger_us = 0;
    while started.elapsed() < Duration::from_millis(duration_ms) {
        let cycle = Instant::now();
        let mut record = format!(
            "\x1b[2;1H\x1b[2Kpace17 run={run} seq={sequence:08}\x1b[3;1H\x1b[32mλ界\x1b[0m"
        )
        .into_bytes();
        record.resize(1024, b'X');
        let before = Instant::now();
        {
            let mut stdout = io::stdout().lock();
            stdout.write_all(&record)?;
            stdout.flush()?;
        }
        let elapsed = before.elapsed().as_micros() as u64;
        max_write_us = max_write_us.max(elapsed);
        write_us += elapsed;
        if let Some(ledger) = ledger.as_mut() {
            let cost = Instant::now();
            writeln!(
                ledger,
                "{}",
                serde_json::json!({"run":run,"record":sequence,"bytes":1024,"write_us":elapsed,"completed_ns":fixture_clock()?})
            )?;
            let cost = cost.elapsed().as_micros() as u64;
            ledger_us += cost;
            max_ledger_us = max_ledger_us.max(cost);
        }
        sequence += 1;
        if let Some(rest) = period.checked_sub(cycle.elapsed()) {
            thread::sleep(rest);
        }
    }
    let finish_ns = fixture_clock()?;
    let mut stdout = io::stdout().lock();
    write!(
        stdout,
        "\x1b[2;1H\x1b[2Kpace17-complete run={run} records={sequence} bytes={} start_ns={start_ns} finish_ns={finish_ns} write_us={write_us} max_write_us={max_write_us}\r\nfixture> ",
        sequence * 1024
    )?;
    stdout.flush()?;
    if let Some(ledger) = ledger.as_mut() {
        writeln!(
            ledger,
            "{}",
            serde_json::json!({"kind":"complete","run":run,"records":sequence,"bytes":sequence*1024,"start_ns":start_ns,"finish_ns":finish_ns,"write_us":write_us,"max_write_us":max_write_us,"ledger_us":ledger_us,"max_ledger_us":max_ledger_us})
        )?;
    }
    Ok(())
}

fn fixture_clock() -> io::Result<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: writable timespec; the isolated fixture and harness share this kernel clock.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64)
}

fn ignore_term() {
    // SAFETY: installs the POSIX constant disposition, with no Rust signal handler.
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }
}
pub fn scripted_descendant() -> io::Result<()> {
    ignore_term();
    println!("descendant pid={}\r", std::process::id());
    io::stdout().flush()?;
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn launch_setup_fault(directory: &Path, boundary: &str) -> io::Result<()> {
    #[cfg(debug_assertions)]
    if fs::read_to_string(directory.join("proof-launch-setup"))
        .is_ok_and(|value| value.trim() == boundary)
    {
        return Err(io::Error::other(format!(
            "Injected launch setup failure: {boundary}"
        )));
    }
    let _ = (directory, boundary);
    Ok(())
}
