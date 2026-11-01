use crate::{
    CameraOwnerLease, Error, PreviewReader, Result,
    control::ControlSender,
    installation_identity::validate_identity,
    preview::PreviewReceiver,
    protocol::{
        commands::{
            camera::{
                cmd_02_0c_playback::{PlaybackMode, PlaybackRequest},
                cmd_02_09_live_view_source::LiveViewSourceRequest,
                cmd_02_80_camera_state::CameraStateNotification,
            },
            common::{
                cmd_00_81_device_info::AppDeviceInfoResponse,
                cmd_00_88_app_presence::AppPresenceRequest,
            },
            gimbal::cmd_03_da_initialize::GimbalInitializeRequest,
            live_view::cmd_09_a8_start_transmission::StartTransmissionRequest,
            wifi::cmd_07_45_pair::{PAIRING_ID, PairingRequest},
        },
        identity::{
            CameraCommand, Command, CommonCommand, Endpoint, GimbalCommand, LiveViewCommand,
            WifiCommand,
        },
        *,
    },
};
use std::{
    collections::VecDeque,
    io::Write,
    net::{IpAddr, SocketAddr, TcpStream, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const IO_POLL: Duration = Duration::from_millis(40);
const TCP_TIMEOUT: Duration = Duration::from_secs(3);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(25);
const REPLY_TIMEOUT: Duration = Duration::from_secs(3);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(4);
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const PRESENCE_INTERVAL: Duration = Duration::from_millis(700);
const PAIRING_DELAY: Duration = Duration::from_millis(400);
const HANDSHAKE_WINDOW: Duration = Duration::from_millis(350);
const REGISTRATION_WINDOW: Duration = Duration::from_millis(400);
const PLAYBACK_WINDOW: Duration = Duration::from_millis(900);
const MAX_BACKLOG: usize = 1000;
const HANDSHAKE_ATTEMPTS: usize = 20;
const PLAYBACK_ATTEMPTS: usize = 3;
const ACK_PRIMING_WINDOWS: usize = 5;
const UDP_RECEIVE_BYTES: usize = 1024 * 1024;

pub struct CameraConfig {
    pub host: IpAddr,
    pub pairing_identity: String,
    pub allow_writes: bool,
}
impl CameraConfig {
    fn validate(&self) -> Result<()> {
        if !self.allow_writes {
            return Err(Error::InvalidConfig {
                field: "allow-writes",
                reason: "camera registration and live preview require explicit approval",
            });
        }
        if self.host.is_unspecified() || self.host.is_multicast() {
            return Err(Error::InvalidConfig {
                field: "host",
                reason: "an explicit unicast LAN camera address is required",
            });
        }
        validate_identity(&self.pairing_identity)?;
        Ok(())
    }
}

pub struct CameraSession {
    reader: PreviewReader,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<()>>>,
    #[cfg(feature = "ble")]
    pub(crate) ble_owner: Option<crate::ble::BleSession>,
}

/// LAN control owner. Connection pairs, registers APP presence and checks a
/// country-data reply; preview requires a separate explicit lifecycle action.
/// No network join or discovery occurs.
pub struct ControlSession {
    commands: mpsc::SyncSender<ControlWork>,
    stop: Arc<AtomicBool>,
    closing: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<()>>>,
    preview_units: Arc<AtomicU64>,
    preview_commands: Arc<Mutex<Vec<PreviewCommandEvent>>>,
}

/// Sanitized lifecycle evidence. Payloads, identities and network data are not retained.
#[derive(Clone)]
pub struct PreviewCommandEvent {
    pub elapsed: Duration,
    pub sent: bool,
    pub sender: Endpoint,
    pub receiver: Endpoint,
    pub command: Command,
    pub id: u16,
    pub flags: Flags,
    pub payload_length: usize,
    pub status: Option<ResponseStatus>,
    pub sequence: u16,
}
struct ControlWork {
    operation: ControlOperation,
    response: mpsc::SyncSender<Result<Option<Message>>>,
}
enum ControlOperation {
    Request(ExplorationRequest),
    Receive(Duration),
    StartPreview,
    StopPreview,
}
impl ControlSession {
    pub fn connect(config: CameraConfig, cancellation: &PreviewCancellation) -> Result<Self> {
        config.validate()?;
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let (commands, work) = mpsc::sync_channel(1);
        let (ready, initialized) = mpsc::sync_channel(1);
        let stop = cancellation.stop.clone();
        let worker_stop = stop.clone();
        let closing = Arc::new(AtomicBool::new(false));
        let worker_closing = closing.clone();
        let preview_units = Arc::new(AtomicU64::new(0));
        let worker_preview_units = preview_units.clone();
        let preview_commands = Arc::new(Mutex::new(Vec::new()));
        let worker_preview_commands = preview_commands.clone();
        let worker = thread::Builder::new()
            .name("osmo-control-io".into())
            .spawn(move || {
                let mut link =
                    match Link::open(&config, worker_stop, PreviewReader::default(), None) {
                        Ok(link) => link,
                        Err(error) => {
                            let _ = ready.send(Err(error.clone()));
                            return Err(error);
                        }
                    };
                link.sampled_preview_units = Some(worker_preview_units);
                link.preview_commands = Some(worker_preview_commands);
                let setup: Result<()> = (|| {
                    link.setup_control()?;
                    link.deadline = None;
                    link.request(Endpoint::Wifi, Command::Wifi(WifiCommand::Country), &[])?;
                    Ok(())
                })();
                if let Err(error) = setup {
                    let _ = ready.send(Err(error.clone()));
                    return Err(error);
                }
                ready.send(Ok(())).map_err(|_| Error::Cancelled)?;
                let result = (|| loop {
                    if worker_closing.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    if link.stop.load(Ordering::Acquire) {
                        return Err(Error::Cancelled);
                    }
                    match work.try_recv() {
                        Ok(ControlWork {
                            operation,
                            response,
                        }) => {
                            let result = match operation {
                                ControlOperation::Request(request) => {
                                    link.exchange_request(request).map(Some)
                                }
                                ControlOperation::Receive(duration) => link.receive(duration),
                                ControlOperation::StartPreview => {
                                    link.setup_preview().map(|()| None)
                                }
                                ControlOperation::StopPreview => link.stop_preview().map(|()| None),
                            };
                            let failure = result.as_ref().err().cloned();
                            response.send(result).map_err(|_| Error::Cancelled)?;
                            if let Some(error) = failure {
                                return Err(error);
                            }
                        }
                        Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                    link.control_tick()?;
                    if link.preview.is_none() {
                        link.backlog.clear();
                    }
                })();
                let cleanup = link.stop_preview();
                match (result, cleanup) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
                    (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
                }
            })
            .map_err(|error| Error::io("spawn control worker", error))?;
        let mut session = Self {
            commands,
            stop,
            closing,
            worker: Some(worker),
            preview_units,
            preview_commands,
        };
        match initialized.recv_timeout(STARTUP_TIMEOUT + TCP_TIMEOUT + REPLY_TIMEOUT) {
            Ok(Ok(())) => Ok(session),
            result => {
                let primary = match result {
                    Ok(Err(error)) => error,
                    Err(source) => Error::WorkerStartup { source },
                    Ok(Ok(())) => unreachable!(),
                };
                match session.close() {
                    Ok(()) => Err(primary),
                    Err(worker_error) => Err(worker_error),
                }
            }
        }
    }

    pub fn request_raw(
        &mut self,
        request: ExplorationRequest,
        session_approved: bool,
        request_approved: bool,
    ) -> Result<Message> {
        request.validate(session_approved, request_approved)?;
        self.exchange(ControlOperation::Request(request))?
            .ok_or(Error::InvalidState("unexpected empty control reply"))
    }

    /// Receives one inbound frame without adding a request or subscription.
    /// Existing APP presence and control acknowledgments continue.
    /// Silence returns `None`; link failures and cancellation remain errors.
    pub fn receive(&mut self, timeout: Duration) -> Result<Option<Message>> {
        crate::validate_receive_timeout(timeout)?;
        self.exchange(ControlOperation::Receive(timeout))
    }

    /// Starts the existing native preview lifecycle on this owner. Video is
    /// assembled and acknowledged, then discarded rather than queued for display.
    pub fn start_preview(&mut self, approved: bool) -> Result<()> {
        if !approved {
            return Err(Error::InvalidConfig {
                field: "preview",
                reason: "preview lifecycle requires explicit approval",
            });
        }
        self.exchange(ControlOperation::StartPreview).map(|_| ())
    }

    /// Restores normal camera mode without closing this control owner.
    pub fn stop_preview(&mut self) -> Result<()> {
        self.exchange(ControlOperation::StopPreview).map(|_| ())
    }

    /// Complete native access units assembled during this owner's preview.
    pub fn preview_units(&self) -> u64 {
        self.preview_units.load(Ordering::Acquire)
    }

    pub fn preview_commands(&self) -> Result<Vec<PreviewCommandEvent>> {
        self.preview_commands
            .lock()
            .map(|events| events.clone())
            .map_err(|_| Error::InvalidState("preview command log poisoned"))
    }

    fn exchange(&mut self, operation: ControlOperation) -> Result<Option<Message>> {
        if self.stop.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        if self.worker.as_ref().is_none_or(JoinHandle::is_finished) {
            return Err(Error::WorkerTerminated);
        }
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(ControlWork {
                operation,
                response,
            })
            .map_err(|_| Error::WorkerTerminated)?;
        result.recv().map_err(|_| Error::WorkerTerminated)?
    }

    /// Executes one typed query over this owner's control session.
    pub fn execute_query<Q: Query>(&mut self, query: Q) -> Result<Q::Response> {
        let request = ExplorationRequest {
            receiver: query.receiver(),
            command: query.command(),
            flags: Flags::REQUEST,
            payload: query.encode()?,
            timeout: query.timeout(),
        };
        let reply = self.request_raw(request, false, false)?;
        <Q::Response as WireDecode<Q>>::decode(&reply.payload, &query)
    }

    pub fn close(&mut self) -> Result<()> {
        self.closing.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker.join().map_err(|_| Error::WorkerPanicked)?,
            None => Ok(()),
        }
    }
}
impl Drop for ControlSession {
    fn drop(&mut self) {
        if self.worker.is_some()
            && let Err(error) = self.close()
        {
            eprintln!("Control owner cleanup failed: {error}");
        }
    }
}
struct WorkerCompletion {
    reader: PreviewReader,
    finished: bool,
}
impl Drop for WorkerCompletion {
    fn drop(&mut self) {
        if !self.finished {
            self.reader.finish(Err(Error::WorkerTerminated));
        }
    }
}
#[derive(Clone, Default)]
pub struct PreviewCancellation {
    pub(crate) stop: Arc<AtomicBool>,
}
impl PreviewCancellation {
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }
}
impl CameraSession {
    pub fn start_preview(config: CameraConfig) -> Result<Self> {
        Self::start_preview_cancellable(config, &PreviewCancellation::default())
    }
    pub fn start_preview_cancellable(
        config: CameraConfig,
        cancellation: &PreviewCancellation,
    ) -> Result<Self> {
        log::info!("Starting the explicitly approved LAN preview lifecycle");
        Self::start_with_lease(config, cancellation, None)
    }
    pub(crate) fn start_with_lease(
        config: CameraConfig,
        cancellation: &PreviewCancellation,
        lease: Option<Arc<CameraOwnerLease>>,
    ) -> Result<Self> {
        config.validate()?;
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let reader = PreviewReader::default();
        let stop = cancellation.stop.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker_reader = reader.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("osmo-camera-io".into())
            .spawn(move || {
                let mut completion = WorkerCompletion {
                    reader: worker_reader.clone(),
                    finished: false,
                };
                let result = run(config, worker_stop, worker_reader.clone(), ready_tx, lease);
                worker_reader.finish(result.clone());
                completion.finished = true;
                result
            })
            .map_err(|error| Error::io("spawn the camera I/O worker", error))?;
        let mut session = Self {
            reader,
            stop,
            worker: Some(worker),
            #[cfg(feature = "ble")]
            ble_owner: None,
        };
        match ready_rx.recv_timeout(STARTUP_TIMEOUT + TCP_TIMEOUT + CLEANUP_TIMEOUT + IO_POLL) {
            Ok(Ok(())) if cancellation.is_cancelled() => match session.close() {
                Ok(()) => Err(Error::Cancelled),
                Err(cleanup) => Err(Error::cleanup(Error::Cancelled, cleanup)),
            },
            Ok(Ok(())) => {
                log::info!("LAN preview startup completed");
                Ok(session)
            }
            Ok(Err(startup)) => {
                log::error!("LAN preview startup failed: {startup}");
                match session.close() {
                    Err(error) => Err(error),
                    Ok(()) => Err(startup),
                }
            }
            Err(error) => {
                let failure = Error::WorkerStartup { source: error };
                match session.close() {
                    Ok(()) => Err(failure),
                    Err(cleanup) => Err(Error::cleanup(failure, cleanup)),
                }
            }
        }
    }
    pub fn reader(&self) -> PreviewReader {
        self.reader.clone()
    }
    pub fn close(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        self.reader.set_flushing(true);
        let result = if let Some(worker) = self.worker.take() {
            let result = worker
                .join()
                .map_err(|_| Error::WorkerPanicked)
                .and_then(|result| result);
            if let Err(error) = &result {
                log::error!("LAN camera session worker failed: {error}");
            } else {
                log::debug!("LAN camera session closed cleanly");
            }
            result
        } else {
            Ok(())
        };
        #[cfg(feature = "ble")]
        if let Some(mut ble) = self.ble_owner.take() {
            let cleanup = match ble.close() {
                Err(Error::Cancelled) => {
                    log::debug!("BLE preview owner cancelled and disconnected");
                    Ok(())
                }
                result => result,
            };
            return match (result, cleanup) {
                (Ok(()), Ok(())) => Ok(()),
                (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
                (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            };
        }
        result
    }
}
impl Drop for CameraSession {
    fn drop(&mut self) {
        if self.worker.is_some()
            && let Err(error) = self.close()
        {
            eprintln!("Osmo camera shutdown failed: {error}");
        }
    }
}

struct Link {
    _lease: Arc<CameraOwnerLease>,
    udp: UdpSocket,
    session: u16,
    sender: ControlSender,
    encoding: AckEncoding,
    next_id: u16,
    stop: Arc<AtomicBool>,
    deadline: Option<Instant>,
    reader: PreviewReader,
    preview: Option<PreviewReceiver>,
    video: u16,
    download: u16,
    backlog: VecDeque<Message>,
    deferred: VecDeque<Message>,
    registered: bool,
    playback_requested: bool,
    cleaning_up: bool,
    last_presence: Instant,
    last_frame: Instant,
    sampled_preview_units: Option<Arc<AtomicU64>>,
    preview_commands: Option<Arc<Mutex<Vec<PreviewCommandEvent>>>>,
    opened_at: Instant,
}
fn run(
    config: CameraConfig,
    stop: Arc<AtomicBool>,
    reader: PreviewReader,
    ready: mpsc::SyncSender<Result<()>>,
    lease: Option<Arc<CameraOwnerLease>>,
) -> Result<()> {
    let mut link = match Link::open(&config, stop, reader, lease) {
        Ok(link) => link,
        Err(error) => {
            log::error!("Could not open LAN camera transport: {error}");
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
    };
    let setup = link.setup();
    let result = match setup {
        Ok(()) => {
            if ready.send(Ok(())).is_err() {
                Err(Error::Cancelled)
            } else {
                link.stream()
            }
        }
        Err(error) => {
            log::error!("LAN camera registration or preview startup failed: {error}");
            let _ = ready.send(Err(error.clone()));
            Err(error)
        }
    };
    let result = if matches!(result, Err(Error::Cancelled)) {
        Ok(())
    } else {
        result
    };
    let cleanup = link.cleanup();
    let result = match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
    };
    if let Err(error) = &result {
        log::error!("LAN camera session stopped with an error: {error}");
    }
    result
}
impl Link {
    fn open(
        config: &CameraConfig,
        stop: Arc<AtomicBool>,
        reader: PreviewReader,
        lease: Option<Arc<CameraOwnerLease>>,
    ) -> Result<Self> {
        log::debug!("Opening LAN camera transport");
        let lease = match lease {
            Some(lease) => lease,
            None => Arc::new(CameraOwnerLease::acquire()?),
        };
        let identity = config.pairing_identity.clone();
        if stop.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        let mut tcp =
            TcpStream::connect_timeout(&SocketAddr::new(config.host, PAIRING_PORT), TCP_TIMEOUT)
                .map_err(|error| Error::io("connect the TCP pairing socket", error))?;
        log::debug!("TCP pairing socket connected");
        tcp.set_write_timeout(Some(TCP_TIMEOUT))
            .map_err(|error| Error::io("set the TCP write deadline", error))?;
        tcp.write_all(
            &Message::request(
                Endpoint::Wifi,
                PAIRING_ID,
                Command::Wifi(WifiCommand::Pair),
                &PairingRequest {
                    identity: &identity,
                }
                .encode()?,
            )
            .encode()?,
        )
        .map_err(|error| Error::io("write the TCP pairing request", error))?;
        let until = Instant::now() + PAIRING_DELAY;
        while Instant::now() < until {
            if stop.load(Ordering::Acquire) {
                return Err(Error::Cancelled);
            }
            thread::sleep(IO_POLL.min(until.saturating_duration_since(Instant::now())));
        }
        drop(tcp);
        let bind = if config.host.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let udp = UdpSocket::bind(bind).map_err(|error| Error::io("bind the UDP socket", error))?;
        socket2::SockRef::from(&udp)
            .set_recv_buffer_size(UDP_RECEIVE_BYTES)
            .map_err(|error| Error::io("size the UDP receive buffer", error))?;
        udp.connect(SocketAddr::new(config.host, UDP_PORT))
            .map_err(|error| Error::io("connect the UDP socket", error))?;
        log::debug!("UDP camera transport connected");
        udp.set_read_timeout(Some(IO_POLL))
            .map_err(|error| Error::io("set the UDP read deadline", error))?;
        udp.set_write_timeout(Some(IO_POLL))
            .map_err(|error| Error::io("set the UDP write deadline", error))?;
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|source| Error::Clock {
                source: Arc::new(source),
            })?
            .subsec_nanos()
            ^ std::process::id();
        let session = (seed as u16) | INITIAL_SESSION;
        let base = ((seed >> u16::BITS) as u16) & INDEX_BITS;
        Ok(Self {
            _lease: lease,
            udp,
            session,
            sender: ControlSender::new(base),
            encoding: AckEncoding::List,
            next_id: FIRST_ID,
            stop,
            deadline: Some(Instant::now() + STARTUP_TIMEOUT),
            reader,
            preview: None,
            video: 0,
            download: 0,
            backlog: VecDeque::new(),
            deferred: VecDeque::new(),
            registered: false,
            playback_requested: false,
            cleaning_up: false,
            last_presence: Instant::now(),
            last_frame: Instant::now(),
            sampled_preview_units: None,
            preview_commands: None,
            opened_at: Instant::now(),
        })
    }
    fn check(&self) -> Result<()> {
        if self.stop.load(Ordering::Acquire) && !self.cleaning_up {
            return Err(Error::Cancelled);
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(Error::Timeout {
                operation: if self.cleaning_up {
                    "playback cleanup"
                } else {
                    "camera startup"
                },
            });
        }
        Ok(())
    }
    fn send_raw(&self, kind: PacketType, payload: &[u8]) -> Result<()> {
        self.check()?;
        let bytes = datagram(kind, self.session, 0, payload)?;
        self.send_packet(&bytes)
    }
    fn send_packet(&self, bytes: &[u8]) -> Result<()> {
        let sent = self
            .udp
            .send(bytes)
            .map_err(|error| Error::io("send a UDP datagram", error))?;
        if sent != bytes.len() {
            return Err(Error::io(
                "send a UDP datagram",
                std::io::Error::new(std::io::ErrorKind::WriteZero, "partial UDP write"),
            ));
        }
        if bytes[TYPE_OFFSET] == PacketType::RoutedCommand.to_wire_u8()
            && (self.preview_commands.is_some() || log::log_enabled!(log::Level::Debug))
        {
            for message in Message::scan(bytes) {
                self.observe_preview_command(true, &message, u16_at(bytes, SEQUENCE_OFFSET))?;
            }
        }
        Ok(())
    }
    fn send_message(&mut self, message: Message) -> Result<()> {
        let deadline = Instant::now() + REPLY_TIMEOUT;
        while !self.sender.can_send() {
            self.check()?;
            if Instant::now() >= deadline {
                return Err(Error::Timeout {
                    operation: "a free control-window slot",
                });
            }
            self.pump(IO_POLL)?;
        }
        let bytes = self.sender.send(message, self.session)?;
        self.send_packet(&bytes)
    }
    fn command(&mut self, endpoint: Endpoint, command: Command, payload: &[u8]) -> Result<Message> {
        self.command_with_flags(endpoint, command, payload, Flags::REQUEST)
    }
    fn command_with_flags(
        &mut self,
        endpoint: Endpoint,
        command: Command,
        payload: &[u8],
        flags: Flags,
    ) -> Result<Message> {
        self.check()?;
        let id = self.next_id;
        self.next_id = if id == MAX_ID { MIN_ID } else { id + 1 };
        let mut message = Message::request(endpoint, id, command, payload);
        message.flags = flags;
        self.send_message(message.clone())?;
        Ok(message)
    }
    fn request(&mut self, endpoint: Endpoint, command: Command, payload: &[u8]) -> Result<()> {
        self.exchange_request(ExplorationRequest {
            receiver: endpoint,
            command,
            payload: payload.to_vec(),
            flags: Flags::REQUEST,
            timeout: REPLY_TIMEOUT,
        })?
        .require_success()
    }
    fn exchange_request(&mut self, request: ExplorationRequest) -> Result<Message> {
        let message = self.command_with_flags(
            request.receiver,
            request.command,
            &request.payload,
            request.flags,
        )?;
        let deadline = Instant::now() + request.timeout;
        loop {
            if let Some(position) = self.backlog.iter().position(|m| m.is_reply_to(&message)) {
                let reply = self.backlog.remove(position).expect("matched reply");
                return Ok(reply);
            }
            if Instant::now() >= deadline {
                log::error!(
                    "LAN reply deadline exceeded: command={:?}, receiver={:?}, id={:#06x}, timeout_ms={}, buffered_messages={}, control_left={}, control_right={}, deferred_replies={}, cleaning_up={}",
                    message.command,
                    message.receiver,
                    message.id,
                    request.timeout.as_millis(),
                    self.backlog.len(),
                    self.sender.left,
                    self.sender.right,
                    self.deferred.len(),
                    self.cleaning_up
                );
                return Err(Error::ReplyTimeout {
                    command: request.command,
                });
            }
            self.control_tick()?;
        }
    }
    fn control_tick(&mut self) -> Result<()> {
        self.pump(IO_POLL)?;
        self.ack()?;
        if self.last_presence.elapsed() >= PRESENCE_INTERVAL && self.sender.can_send() {
            self.presence()?;
        }
        if !self.cleaning_up && self.preview.is_some() && self.last_frame.elapsed() >= FRAME_TIMEOUT
        {
            return Err(Error::Timeout {
                operation: "complete video (five seconds)",
            });
        }
        Ok(())
    }
    fn receive(&mut self, duration: Duration) -> Result<Option<Message>> {
        let deadline = Instant::now() + duration;
        loop {
            self.check()?;
            if let Some(message) = self.backlog.pop_front() {
                return Ok(Some(message));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            self.control_tick()?;
        }
    }
    fn presence(&mut self) -> Result<()> {
        self.command(
            Endpoint::Presence,
            Command::Common(CommonCommand::AppPresence),
            &AppPresenceRequest.encode()?,
        )?;
        self.last_presence = Instant::now();
        Ok(())
    }
    fn ack(&self) -> Result<()> {
        let feedback = self
            .preview
            .as_ref()
            .and_then(|p| p.acknowledgment(self.encoding));
        self.send_raw(
            PacketType::Acknowledgment,
            &self
                .sender
                .acknowledgment(self.encoding, self.video, self.download, feedback),
        )
    }
    fn setup(&mut self) -> Result<()> {
        log::info!("Performing LAN camera registration");
        self.setup_control()?;
        log::info!("Requesting camera live preview");
        self.setup_preview()
    }
    fn setup_control(&mut self) -> Result<()> {
        let mut syn = SYN_PARAMETERS.to_vec();
        put16(&mut syn, SYN_BASE, self.sender.right);
        put16(&mut syn, SYN_CAPACITY, CONTROL_CAPACITY);
        let mut accepted = false;
        for _ in 0..HANDSHAKE_ATTEMPTS {
            self.send_raw(PacketType::Handshake, &syn)?;
            if self.pump_handshake(HANDSHAKE_WINDOW)? {
                accepted = true;
                break;
            }
        }
        if !accepted {
            return Err(Error::Timeout {
                operation: "the camera UDP handshake reply",
            });
        }
        log::debug!("Camera UDP handshake accepted");
        for _ in 0..ACK_PRIMING_WINDOWS {
            self.pump(REGISTRATION_WINDOW)?;
            self.ack()?;
        }
        self.registered = true;
        log::debug!("Camera registration completed");
        self.command_with_flags(
            Endpoint::DeviceInfo,
            Command::Common(CommonCommand::DeviceInfo),
            &AppDeviceInfoResponse.encode()?,
            Flags::RESPONSE,
        )?;
        self.pump(REGISTRATION_WINDOW)?;
        self.presence()?;
        self.pump(REGISTRATION_WINDOW)?;
        Ok(())
    }
    fn setup_preview(&mut self) -> Result<()> {
        if self.preview.is_some() || self.playback_requested {
            return Err(Error::InvalidState("preview lifecycle is already active"));
        }
        self.command(
            Endpoint::Gimbal,
            Command::Gimbal(GimbalCommand::Initialize),
            &GimbalInitializeRequest.encode()?,
        )?;
        self.pump(REGISTRATION_WINDOW)?;
        let mut playback = false;
        log::debug!("Waiting for camera playback-mode confirmation");
        for _ in 0..PLAYBACK_ATTEMPTS {
            self.presence()?;
            self.playback_requested = true;
            let request = self.command(
                Endpoint::Camera,
                Command::Camera(CameraCommand::Playback),
                &PlaybackRequest {
                    mode: PlaybackMode::Enter,
                }
                .encode()?,
            )?;
            self.pump(PLAYBACK_WINDOW)?;
            for message in &self.backlog {
                if message.sender == Endpoint::Camera
                    && message.receiver == Endpoint::App
                    && message.command == Command::Camera(CameraCommand::State)
                    && CameraStateNotification::decode(&message.payload, &())
                        .is_ok_and(|state| state.playback_mode_active())
                {
                    playback = true;
                }
                if message.is_reply_to(&request) && !message.payload.is_empty() {
                    message.require_success()?;
                }
            }
            self.ack()?;
            if playback {
                break;
            }
        }
        if !playback {
            return Err(Error::Timeout {
                operation: "playback mode confirmation",
            });
        }
        log::debug!("Camera playback mode confirmed");
        self.request(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Playback),
            &PlaybackRequest {
                mode: PlaybackMode::Leave,
            }
            .encode()?,
        )?;
        self.request(
            Endpoint::Camera,
            Command::Camera(CameraCommand::LiveViewSource),
            &LiveViewSourceRequest.encode()?,
        )?;
        self.preview = Some(PreviewReceiver::new(self.session));
        self.last_frame = Instant::now();
        self.request(
            Endpoint::LiveView,
            Command::LiveView(LiveViewCommand::StartTransmission),
            &StartTransmissionRequest.encode()?,
        )?;
        self.deadline = None;
        Ok(())
    }
    fn pump_handshake(&mut self, duration: Duration) -> Result<bool> {
        let end = Instant::now() + duration;
        let mut bytes = vec![0; usize::from(UDP_LENGTH_MASK) + 1];
        while Instant::now() < end {
            self.check()?;
            match self.udp.recv(&mut bytes) {
                Ok(length) => {
                    let packet = &bytes[..length];
                    if valid_datagram(packet, self.session)
                        && packet[TYPE_OFFSET] == PacketType::Handshake.to_wire_u8()
                    {
                        self.encoding = AckEncoding::parse(
                            packet
                                .get(UDP_HEADER)
                                .copied()
                                .unwrap_or(AckEncoding::List as u8),
                        )?;
                        return Ok(true);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => return Err(Error::io("receive the UDP handshake", error)),
            }
        }
        Ok(false)
    }
    fn pump(&mut self, duration: Duration) -> Result<()> {
        let end = Instant::now() + duration;
        let mut bytes = vec![0; usize::from(UDP_LENGTH_MASK) + 1];
        while Instant::now() < end {
            self.check()?;
            match self.udp.recv(&mut bytes) {
                Ok(length) => self.observe(&bytes[..length])?,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => return Err(Error::io("receive a UDP datagram", error)),
            }
        }
        self.flush_replies()
    }
    fn observe(&mut self, packet: &[u8]) -> Result<()> {
        if !valid_datagram(packet, self.session) {
            return Ok(());
        }
        let kind = packet[TYPE_OFFSET];
        if kind == PacketType::Video.to_wire_u8() {
            if let Some(preview) = &mut self.preview {
                for unit in preview.feed(packet, Instant::now())? {
                    if unit.frame_number == 1 {
                        log::info!(
                            "LAN first complete video access unit: elapsed_ms={}, bytes={}, keyframe={}, cleaning_up={}",
                            self.opened_at.elapsed().as_millis(),
                            unit.bytes.len(),
                            unit.keyframe,
                            self.cleaning_up
                        );
                    }
                    let gap = self.last_frame.elapsed();
                    if unit.frame_number > 1 && gap >= Duration::from_millis(100) {
                        log::warn!(
                            "LAN video arrival gap: gap_ms={}, frame={}, keyframe={}, discontinuity={}",
                            gap.as_millis(),
                            unit.frame_number,
                            unit.keyframe,
                            unit.discontinuity
                        );
                    }
                    self.last_frame = Instant::now();
                    if let Some(count) = &self.sampled_preview_units {
                        count.fetch_add(1, Ordering::Release);
                    } else if !self.cleaning_up {
                        let frame_number = unit.frame_number;
                        if let Err(error) = self.reader.push(unit) {
                            log::error!(
                                "LAN video queue publication failed: frame={}, elapsed_ms={}, error={error}",
                                frame_number,
                                self.opened_at.elapsed().as_millis()
                            );
                            return Err(error);
                        }
                    }
                }
                self.video = preview.cursor().unwrap_or(self.video);
            }
            return Ok(());
        }
        if kind == PacketType::StreamWindow.to_wire_u8() {
            if let Some(preview) = &mut self.preview {
                preview.observe_window(&packet[UDP_HEADER..])?;
            }
            // Invalid feedback is terminal in this prototype, not a success-shaped empty ACK.
            let retries =
                self.sender
                    .observe(&packet[UDP_HEADER..], self.encoding, self.session)?;
            log::trace!(
                "LAN control feedback: left={}, right={}, retransmissions={}",
                self.sender.left,
                self.sender.right,
                retries.len()
            );
            if !retries.is_empty() {
                log::debug!("LAN control retransmitting {} packet(s)", retries.len());
            }
            for retry in retries {
                self.send_packet(&retry)?;
            }
            if packet.len() >= UDP_HEADER + CONTROL_ACK_OFFSET {
                if self.preview.is_none() {
                    self.video = u16_at(packet, UDP_HEADER + RIGHT);
                }
                self.download = u16_at(packet, UDP_HEADER + WINDOW_LENGTH + RIGHT);
            }
        }
        for message in Message::scan(packet) {
            self.observe_preview_command(false, &message, u16_at(packet, SEQUENCE_OFFSET))?;
            if self.registered
                && message.sender == Endpoint::DeviceInfo
                && message.receiver == Endpoint::App
                && message.flags == Flags::REQUEST
                && message.command == Command::Common(CommonCommand::DeviceInfo)
            {
                let mut reply = Message::request(
                    Endpoint::DeviceInfo,
                    message.id,
                    Command::Common(CommonCommand::DeviceInfo),
                    &AppDeviceInfoResponse.encode()?,
                );
                reply.flags = Flags::ACKNOWLEDGMENT;
                if !self.sender.has_reply(&reply)
                    && !self.deferred.iter().any(|queued| queued.id == reply.id)
                {
                    if self.deferred.len() >= usize::from(CONTROL_CAPACITY) {
                        return Err(Error::LimitExceeded("Device-info reply queue overflow"));
                    }
                    self.deferred.push_back(reply);
                }
            }
            if self.backlog.len() == MAX_BACKLOG {
                return Err(Error::LimitExceeded(
                    "Control receive backlog limit exceeded",
                ));
            }
            self.backlog.push_back(message);
        }
        Ok(())
    }
    fn observe_preview_command(&self, sent: bool, message: &Message, sequence: u16) -> Result<()> {
        if ![
            Command::Gimbal(GimbalCommand::Initialize),
            Command::Camera(CameraCommand::Playback),
            Command::Camera(CameraCommand::LiveViewSource),
            Command::LiveView(LiveViewCommand::StartTransmission),
        ]
        .contains(&message.command)
        {
            return Ok(());
        }
        log::debug!(
            "LAN preview {}: {:?} {:?}->{:?} id={:#06x} flags={:#04x} sequence={} payload_length={} status={:?} elapsed_ms={}",
            if sent { "send" } else { "receive" },
            message.command,
            message.sender,
            message.receiver,
            message.id,
            message.flags.0,
            sequence,
            message.payload.len(),
            if message.flags.is_response() {
                message.payload.first().copied().map(ResponseStatus::from)
            } else {
                None
            },
            self.opened_at.elapsed().as_millis()
        );
        if let Some(events) = &self.preview_commands {
            let mut events = events
                .lock()
                .map_err(|_| Error::InvalidState("preview command log poisoned"))?;
            if events.len() >= 256 {
                return Err(Error::LimitExceeded("preview command log overflow"));
            }
            events.push(PreviewCommandEvent {
                elapsed: self.opened_at.elapsed(),
                sent,
                sender: message.sender,
                receiver: message.receiver,
                command: message.command,
                id: message.id,
                flags: message.flags,
                payload_length: message.payload.len(),
                status: if message.flags.is_response() {
                    message.payload.first().copied().map(ResponseStatus::from)
                } else {
                    None
                },
                sequence,
            });
        }
        Ok(())
    }
    fn flush_replies(&mut self) -> Result<()> {
        while self.sender.can_send() {
            let Some(reply) = self.deferred.pop_front() else {
                break;
            };
            self.send_message(reply)?;
        }
        Ok(())
    }
    fn stream(&mut self) -> Result<()> {
        loop {
            self.pump(IO_POLL)?;
            self.ack()?;
            if self.last_presence.elapsed() >= PRESENCE_INTERVAL && self.sender.can_send() {
                self.presence()?;
            }
            self.backlog.clear();
            if self.last_frame.elapsed() >= FRAME_TIMEOUT {
                return Err(Error::Timeout {
                    operation: "complete video (five seconds)",
                });
            }
        }
    }
    fn stop_preview(&mut self) -> Result<()> {
        if !self.playback_requested {
            return Ok(());
        }
        self.playback_requested = false;
        self.cleaning_up = true;
        log::debug!("Stopping LAN preview with playback cleanup");
        let deadline = self.deadline;
        self.deadline = Some(Instant::now() + CLEANUP_TIMEOUT);
        let restore = self.request(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Playback),
            &PlaybackRequest {
                mode: PlaybackMode::Enter,
            }
            .encode()?,
        );
        let release = self.request(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Playback),
            &PlaybackRequest {
                mode: PlaybackMode::Leave,
            }
            .encode()?,
        );
        self.preview = None;
        self.deadline = deadline;
        self.cleaning_up = false;
        if let Err(error) = &restore {
            log::error!("LAN playback cleanup enter failed: {error}");
        }
        if let Err(error) = &release {
            log::error!("LAN playback cleanup leave failed: {error}");
        }
        if restore.is_ok() && release.is_ok() {
            log::debug!("LAN playback cleanup completed");
        }
        match (restore, release) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
        }
    }
    fn cleanup(&mut self) -> Result<()> {
        let result = self.stop_preview();
        self.stop.store(true, Ordering::Release);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unexpected_worker_exit_wakes_consumers() {
        let reader = PreviewReader::default();
        {
            let _completion = WorkerCompletion {
                reader: reader.clone(),
                finished: false,
            };
        }
        assert!(matches!(reader.receive(), Err(Error::WorkerTerminated)));
    }
}
