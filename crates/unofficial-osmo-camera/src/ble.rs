//! Native BLE lifecycle. `connect` performs discovery, pairing/wake and a live
//! health check on an owned runtime; callers do not need an async runtime.
//! Running the example authorizes camera pairing/wake. It is not an offline test.
//!
//! ```no_run
//! use unofficial_osmo_camera::ble::{BleConfig, BleQuery, BleSession};
//! # fn main() -> unofficial_osmo_camera::Result<()> {
//! let mut camera = BleSession::connect(BleConfig::new(None))?;
//! camera.query(BleQuery::WifiCountry)?.require_success()?;
//! let _identity = camera.pairing_identity();
//! camera.close()?;
//! # Ok(())
//! # }
//! ```
#[cfg(test)]
use crate::installation_identity::IDENTITY_LENGTH;
#[cfg(test)]
use crate::protocol::commands::camera::cmd_02_e0_capabilities_get::CapabilityReply;
use crate::{
    CameraConfig, CameraOwnerLease, CameraSession, Error, PreviewCancellation, Result,
    installation_identity::{new_identity, validate_identity},
    protocol::{
        commands::{
            camera::cmd_02_8e_parameters_get::{CameraParameter, CameraParametersGet},
            camera::cmd_02_e0_capabilities_get::CapabilityQuery,
            camera_wake::cmd_53_10_wake::BleWakeRequest,
            common::{
                cmd_00_2b_session_keepalive::{
                    SessionKeepaliveOpenRequest, SessionKeepaliveRequest,
                },
                cmd_00_32_activation_status::{ActivationState, ActivationStatusNotification},
                cmd_00_81_device_info::AppDeviceInfoResponse,
            },
            reply::{CameraRawData, CameraReadReply},
            wifi::{
                cmd_07_0c_mac_get::WifiMacGet,
                cmd_07_0e_password::{
                    MAX_PASSWORD_BYTES, MIN_PASSWORD_LENGTH, WifiPasswordRequest,
                    WifiPasswordResponse,
                },
                cmd_07_07_ssid::{MAX_SSID_BYTES, WifiSsidGet, WifiSsidResponse},
                cmd_07_19_country_get::WifiCountryGet,
                cmd_07_39_get_role::{WifiRole, WifiRoleGet},
                cmd_07_45_pair::{
                    BlePairingArmRequest, PairingRequest, PairingResponse, PairingState,
                },
                cmd_07_46_pair_approval::{PairingApproval, PairingApprovalNotification},
                cmd_07_47_join_network::{
                    JOIN_RETRY_DETAIL, JOIN_RETRY_STATUS, JoinNetworkRequest,
                },
                cmd_07_48_set_role::WifiRoleSetRequest,
            },
        },
        identity::{
            CameraCommand, CameraWakeCommand, Command, CommonCommand, Endpoint, WifiCommand,
        },
        *,
    },
};
use btleplug::{
    api::{
        Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
        ValueNotification, WriteType, bleuuid::uuid_from_u16,
    },
    platform::{Adapter, Manager, Peripheral},
};
use futures::{Stream, StreamExt};
use std::{
    collections::VecDeque,
    future::Future,
    net::IpAddr,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc as async_mpsc,
    time::{Instant, sleep, sleep_until, timeout},
};

const SCAN_TIME: Duration = Duration::from_secs(12);
const GATT_TIMEOUT: Duration = Duration::from_secs(15);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const PAIR_TIMEOUT: Duration = Duration::from_secs(60);
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
const JOIN_TIMEOUT: Duration = Duration::from_secs(45);
const ROLE_TIMEOUT: Duration = Duration::from_secs(12);
const ROLE_SETTLE: Duration = Duration::from_secs(10);
const JOIN_RETRY_DELAY: Duration = Duration::from_secs(5);
const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(700);
const WRITE_PACING: Duration = Duration::from_millis(70);
const CANCEL_POLL: Duration = Duration::from_millis(40);
const PAIR_SETTLE: Duration = Duration::from_millis(300);
const APPROVAL_SETTLE: Duration = Duration::from_millis(150);
const WAKE_SETTLE: Duration = Duration::from_millis(800);
const CREDENTIAL_PACING: Duration = Duration::from_millis(500);
const MAX_JOIN_ATTEMPTS: usize = 3;
const MAX_CREDENTIAL_ATTEMPTS: usize = 3;
const MAX_PENDING_FRAMES: usize = 1000;
const MAX_NOTIFICATION_BYTES: usize = 64 * 1024;
const COMMAND_CAPACITY: usize = 1;
const PAIR_RETRY_TICKS: &[u32] = &[4, 8];
const SERVICE: u16 = 0xfff0;
const COMMAND_CHARACTERISTIC: u16 = 0xfff5;
const PAIRING_CHARACTERISTIC: u16 = 0xfff4;
const FIRST_BLE_ID: u16 = 0x8001;
const ATT_HEADER: usize = 3;

#[derive(Clone, Default)]
pub struct BleCancellation(Arc<AtomicBool>);
impl BleCancellation {
    pub(crate) fn for_preview(cancellation: &PreviewCancellation) -> Self {
        Self(cancellation.stop.clone())
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    async fn cancelled(&self) {
        while !self.is_cancelled() {
            sleep(CANCEL_POLL).await;
        }
    }
}

/// Router configuration is never included in Debug or library log output.
#[derive(Clone)]
pub struct WifiNetwork {
    ssid: String,
    password: String,
}
impl WifiNetwork {
    pub fn new(ssid: String, password: String) -> Result<Self> {
        if ssid.trim().is_empty()
            || ssid.len() > MAX_SSID_BYTES
            || password.chars().count() < MIN_PASSWORD_LENGTH
            || password.len() > MAX_PASSWORD_BYTES
        {
            return Err(Error::InvalidConfig {
                field: "Wi-Fi network",
                reason: "requires a nonempty SSID up to 32 UTF-8 bytes and an 8-63 byte WPA2 password",
            });
        }
        Ok(Self { ssid, password })
    }
}
impl std::fmt::Debug for WifiNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WifiNetwork { credentials: [REDACTED] }")
    }
}

pub struct HotspotCredentials {
    pub ssid: String,
    pub password: String,
}
impl std::fmt::Debug for HotspotCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HotspotCredentials { credentials: [REDACTED] }")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkCleanup {
    #[default]
    Preserve,
    RestoreHotspot,
}

/// Calling `connect` authorizes its known pairing/wake lifecycle. Supplying a
/// network additionally authorizes station selection and one bounded join cycle.
#[derive(Clone, Debug)]
pub struct BleConfig {
    /// Existing reusable identity, or `None` to generate one for this session.
    pub pairing_identity: Option<String>,
    /// Opaque platform peripheral ID, not necessarily a MAC address.
    pub device_id: Option<String>,
    pub network: Option<WifiNetwork>,
    pub network_cleanup: NetworkCleanup,
}
impl BleConfig {
    pub fn new(pairing_identity: Option<String>) -> Self {
        Self {
            pairing_identity,
            device_id: None,
            network: None,
            network_cleanup: NetworkCleanup::Preserve,
        }
    }
}

pub struct BleDevice {
    peripheral: Peripheral,
    _adapter: Adapter,
    pub id: String,
    pub name: String,
    pub rssi: Option<i16>,
}

#[derive(Clone, Copy, Debug)]
pub enum BleQuery {
    /// Read the camera's own Wi-Fi SSID, not its joined-router SSID.
    ///
    /// APP -> Wi-Fi | 0x07/0x07 | SSID GET
    ///
    /// ```text
    /// request {}
    /// reply {
    ///     status: u8          | reject if nonzero
    ///     length: u8
    ///     ssid: [u8; length]  | UTF-8
    /// }
    /// ```
    WifiSsid,
    /// Read the Wi-Fi MAC address.
    ///
    /// APP -> Wi-Fi | 0x07/0x0C | MAC GET
    ///
    /// ```text
    /// request {}
    /// reply {
    ///     status: u8  | reject if nonzero
    ///     mac: [u8; 6]
    /// }
    /// ```
    WifiMac,
    /// Read Wi-Fi country data; individual fields remain partly unmapped.
    ///
    /// APP -> Wi-Fi | 0x07/0x19 | Country GET
    ///
    /// ```text
    /// request {}
    /// reply {
    ///     status: u8             | reject if nonzero
    ///     data: [u8; remaining]  | fields partly unmapped
    /// }
    /// ```
    WifiCountry,
    /// Request the Wi-Fi role; accepted response layout remains unresolved.
    ///
    /// APP -> Wi-Fi | 0x07/0x39 | Role GET
    ///
    /// ```text
    /// request { bytes: [0x00] }
    /// reply {
    ///     status: u8             | reject if nonzero
    ///     data: [u8; remaining]  | layout unresolved
    /// }
    /// ```
    WifiRole,
    /// Read field-of-view code.
    CameraFov,
    /// Read ISO-limit code.
    CameraIsoLimit,
    /// Read parameter `0x0014`; its purpose remains unresolved.
    CameraParameter0014,
    /// Read a named capability; see [`CapabilityQuery`] for payload layouts.
    Capability(CapabilityQuery),
}
impl BleQuery {
    fn request(self, id: u16) -> Result<Message> {
        let (receiver, command, payload) = match self {
            Self::Capability(query) => {
                return Ok(Message::request(
                    query.receiver(),
                    id,
                    query.command(),
                    &encode(&query)?,
                ));
            }
            Self::WifiSsid => (
                WifiSsidGet.receiver(),
                WifiSsidGet.command(),
                encode(&WifiSsidGet)?,
            ),
            Self::WifiMac => (
                WifiMacGet.receiver(),
                WifiMacGet.command(),
                encode(&WifiMacGet)?,
            ),
            Self::WifiCountry => (
                WifiCountryGet.receiver(),
                WifiCountryGet.command(),
                encode(&WifiCountryGet)?,
            ),
            Self::WifiRole => (
                Endpoint::Wifi,
                Command::Wifi(WifiCommand::GetRole),
                encode(&WifiRoleGet)?,
            ),
            Self::CameraFov | Self::CameraIsoLimit | Self::CameraParameter0014 => {
                let parameter = match self {
                    Self::CameraFov => CameraParameter::FieldOfView,
                    Self::CameraIsoLimit => CameraParameter::IsoLimit,
                    _ => CameraParameter::Unknown0014,
                };
                return Ok(Message::request(
                    Endpoint::Camera,
                    id,
                    Command::Camera(CameraCommand::Parameters),
                    &encode(&CameraParametersGet::single(parameter)?)?,
                ));
            }
        };
        Ok(Message::request(receiver, id, command, &payload))
    }
}

pub struct BleSession {
    commands: async_mpsc::Sender<Work>,
    cancellation: BleCancellation,
    worker: Option<JoinHandle<Result<()>>>,
    lease: Option<Arc<CameraOwnerLease>>,
    pairing_identity: String,
}
enum Operation {
    Query(BleQuery),
    Raw(ExplorationRequest),
    Receive(Duration),
    Credentials,
    Join(WifiNetwork, NetworkCleanup),
    Restore,
    Close,
}
enum Reply {
    Message(Message),
    Received(Option<Message>),
    Credentials(HotspotCredentials),
    Done,
}
struct Work {
    operation: Operation,
    response: mpsc::SyncSender<Result<Reply>>,
}

impl BleSession {
    /// Discovery only. No pairing, wake, GATT connection or network writes.
    pub fn discover() -> Result<Vec<BleDevice>> {
        Self::discover_cancellable(&BleCancellation::default())
    }
    pub fn discover_cancellable(cancellation: &BleCancellation) -> Result<Vec<BleDevice>> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let _lease = CameraOwnerLease::acquire()?;
        let cancellation = cancellation.clone();
        thread::Builder::new()
            .name("osmo-camera-ble-scan".into())
            .spawn(move || runtime()?.block_on(discover(&cancellation)))
            .map_err(|error| Error::io("spawn the BLE discovery worker", error))?
            .join()
            .map_err(|_| Error::WorkerPanicked)?
    }

    /// Initializes by rediscovering rather than trusting a saved BLE address,
    /// reusing the installation identity, pairing/waking, then verifying a reply.
    /// An established-session loss is terminal, never an implicit reconnect.
    pub fn connect(config: BleConfig) -> Result<Self> {
        Self::connect_cancellable(config, &BleCancellation::default())
    }
    pub fn connect_cancellable(config: BleConfig, cancellation: &BleCancellation) -> Result<Self> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let lease = Arc::new(CameraOwnerLease::acquire()?);
        let identity = match &config.pairing_identity {
            Some(identity) => {
                validate_identity(identity)?;
                identity.trim().to_owned()
            }
            None => new_identity(),
        };
        let (commands, receiver) = async_mpsc::channel(COMMAND_CAPACITY);
        let (ready, result) = mpsc::sync_channel(1);
        let worker_cancel = cancellation.clone();
        let worker_lease = lease.clone();
        let worker_identity = identity.clone();
        let worker = thread::Builder::new()
            .name("osmo-camera-ble".into())
            .spawn(move || {
                let _lease = worker_lease;
                let result = match runtime() {
                    Ok(runtime) => runtime.block_on(connect_worker(
                        config,
                        worker_identity,
                        worker_cancel,
                        receiver,
                        ready.clone(),
                    )),
                    Err(error) => Err(error),
                };
                if let Err(error) = &result {
                    let _ = ready.send(Err(error.clone()));
                    log::error!("BLE lifecycle stopped: {error}");
                }
                result
            })
            .map_err(|error| Error::io("spawn the BLE lifecycle worker", error))?;
        let mut session = Self {
            commands,
            cancellation: cancellation.clone(),
            worker: Some(worker),
            lease: Some(lease),
            pairing_identity: identity.clone(),
        };
        match result.recv() {
            Ok(Ok(())) => Ok(session),
            Ok(Err(primary)) => match session.finish() {
                Err(error) => Err(error),
                Ok(()) => Err(primary),
            },
            Err(_) => {
                session.finish()?;
                Err(Error::WorkerTerminated)
            }
        }
    }

    pub fn pairing_identity(&self) -> &str {
        &self.pairing_identity
    }

    pub fn query(&mut self, query: BleQuery) -> Result<Message> {
        match self.exchange(Operation::Query(query))? {
            Reply::Message(message) => Ok(message),
            _ => Err(Error::InvalidState("unexpected BLE query result")),
        }
    }
    /// Executes one typed query through this owner's BLE worker.
    pub fn execute_query<Q: Query>(&mut self, query: Q) -> Result<Q::Response> {
        let request = ExplorationRequest {
            receiver: query.receiver(),
            command: query.command(),
            flags: Flags::REQUEST,
            payload: encode(&query)?,
            timeout: query.timeout(),
        };
        let reply = self.request_raw(request, false, false)?;
        decode::<Q::Response>(&reply.payload, query.response_args())
    }
    /// Exact numeric exploration requests still use this owner's pacing,
    /// keepalives and correlation; rejection status is returned unchanged.
    pub fn request_raw(
        &mut self,
        request: ExplorationRequest,
        session_approved: bool,
        request_approved: bool,
    ) -> Result<Message> {
        request.validate(session_approved, request_approved)?;
        match self.exchange(Operation::Raw(request))? {
            Reply::Message(message) => Ok(message),
            _ => Err(Error::InvalidState("unexpected raw BLE result")),
        }
    }
    /// Receives one inbound frame while maintaining this owner's existing
    /// acknowledgments and keepalives. No request or subscription is added.
    /// Silence returns `None`; link failures and cancellation remain errors.
    pub fn receive(&mut self, timeout: Duration) -> Result<Option<Message>> {
        crate::validate_receive_timeout(timeout)?;
        match self.exchange(Operation::Receive(timeout))? {
            Reply::Received(message) => Ok(message),
            _ => Err(Error::InvalidState("unexpected passive BLE result")),
        }
    }
    pub fn hotspot_credentials(&mut self) -> Result<HotspotCredentials> {
        match self.exchange(Operation::Credentials)? {
            Reply::Credentials(credentials) => Ok(credentials),
            _ => Err(Error::InvalidState("unexpected BLE credentials result")),
        }
    }
    pub fn join_network(&mut self, network: WifiNetwork, cleanup: NetworkCleanup) -> Result<()> {
        self.exchange(Operation::Join(network, cleanup)).map(|_| ())
    }
    pub fn restore_hotspot(&mut self) -> Result<()> {
        self.exchange(Operation::Restore).map(|_| ())
    }
    /// Shares this owner's lease with the existing native LAN preview worker.
    /// The caller supplies a verified LAN address; this never changes PC Wi-Fi.
    pub fn start_preview(&self, host: IpAddr) -> Result<CameraSession> {
        self.start_preview_cancellable(host, &PreviewCancellation::default())
    }
    /// Cancellation covers LAN startup and streaming without cancelling BLE,
    /// so callers can still inspect its health and explicitly close the owner.
    pub fn start_preview_cancellable(
        &self,
        host: IpAddr,
        cancellation: &PreviewCancellation,
    ) -> Result<CameraSession> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if self.worker.as_ref().is_none_or(JoinHandle::is_finished)
            || self.cancellation.is_cancelled()
        {
            return Err(Error::InvalidState("BLE session is no longer live"));
        }
        CameraSession::start_with_lease(
            CameraConfig {
                host,
                pairing_identity: self.pairing_identity.clone(),
                allow_writes: true,
            },
            cancellation,
            self.lease.clone(),
        )
    }
    pub fn close(&mut self) -> Result<()> {
        if self.worker.is_none() {
            return Ok(());
        }
        let result = if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
            && !self.cancellation.is_cancelled()
        {
            self.exchange(Operation::Close).map(|_| ())
        } else {
            Ok(())
        };
        self.finish()?;
        result
    }
    fn exchange(&mut self, operation: Operation) -> Result<Reply> {
        if self.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let (response, receive) = mpsc::sync_channel(1);
        self.commands
            .try_send(Work {
                operation,
                response,
            })
            .map_err(|_| Error::WorkerTerminated)?;
        receive.recv().map_err(|_| Error::WorkerTerminated)?
    }
    fn finish(&mut self) -> Result<()> {
        let result = match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| Error::WorkerPanicked)
                .and_then(|result| result),
            None => Ok(()),
        };
        self.lease.take();
        result
    }
}
impl Drop for BleSession {
    fn drop(&mut self) {
        if self.worker.is_some() {
            self.cancellation.cancel();
            if let Err(error) = self.finish() {
                log::error!("BLE cleanup on drop: {error}");
            }
        }
    }
}

fn runtime() -> Result<Runtime> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::io("create the BLE runtime", error))
}
fn combine<T>(result: Result<T>, cleanup: Result<()>) -> Result<T> {
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
    }
}
fn bluetooth(operation: &'static str, source: btleplug::Error) -> Error {
    Error::Bluetooth {
        operation,
        source: Arc::new(source),
    }
}
async fn bounded<T>(
    cancellation: &BleCancellation,
    duration: Duration,
    operation: &'static str,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(Error::Cancelled),
        result = timeout(duration, future) => result.map_err(|_| Error::Timeout { operation })?,
    }
}
async fn discover(cancellation: &BleCancellation) -> Result<Vec<BleDevice>> {
    log::info!("Scanning for Nano cameras");
    let manager = bounded(cancellation, IO_TIMEOUT, "open BLE manager", async {
        Manager::new()
            .await
            .map_err(|error| bluetooth("open BLE manager", error))
    })
    .await?;
    let adapters = bounded(cancellation, IO_TIMEOUT, "enumerate BLE adapters", async {
        manager
            .adapters()
            .await
            .map_err(|error| bluetooth("enumerate BLE adapters", error))
    })
    .await?;
    if adapters.is_empty() {
        return Err(Error::InvalidState(
            "no BLE adapter available; enable Bluetooth",
        ));
    }
    let mut devices = Vec::new();
    // Scan adapters sequentially so failure/cleanup is attributable to one radio.
    for adapter in adapters {
        let result = bounded(cancellation, IO_TIMEOUT, "start BLE scan", async {
            adapter
                .start_scan(ScanFilter::default())
                .await
                .map_err(|error| bluetooth("start BLE scan", error))
        })
        .await;
        let scan = match result {
            Ok(()) => {
                bounded(
                    cancellation,
                    SCAN_TIME + IO_TIMEOUT,
                    "scan for Nano cameras",
                    async {
                        sleep(SCAN_TIME).await;
                        let peripherals = adapter
                            .peripherals()
                            .await
                            .map_err(|error| bluetooth("collect BLE scan results", error))?;
                        let mut found = Vec::new();
                        for peripheral in peripherals {
                            let properties = peripheral
                                .properties()
                                .await
                                .map_err(|error| bluetooth("read BLE advertisement", error))?;
                            if let Some(properties) = properties {
                                let name = properties
                                    .advertisement_name
                                    .or(properties.local_name)
                                    .unwrap_or_default();
                                if is_nano_name(&name) {
                                    found.push(BleDevice {
                                        id: peripheral.id().to_string(),
                                        peripheral,
                                        _adapter: adapter.clone(),
                                        name,
                                        rssi: properties.rssi,
                                    });
                                }
                            }
                        }
                        Ok(found)
                    },
                )
                .await
            }
            Err(error) => Err(error),
        };
        let cleanup = timeout(IO_TIMEOUT, adapter.stop_scan())
            .await
            .map_err(|_| Error::Timeout {
                operation: "stop BLE scan",
            })
            .and_then(|result| result.map_err(|error| bluetooth("stop BLE scan", error)));
        devices.extend(combine(scan, cleanup)?);
    }
    log::info!("BLE scan complete: {} candidate(s)", devices.len());
    Ok(devices)
}
fn is_nano_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("osmonano") || name.starts_with("osmo nano")
}
fn select_device(mut devices: Vec<BleDevice>, id: Option<&str>) -> Result<BleDevice> {
    if let Some(id) = id {
        devices.retain(|device| device.id == id);
    }
    match devices.len() {
        0 => Err(Error::InvalidState(
            "no matching Nano found; wake the camera/dock and call connect again",
        )),
        1 => Ok(devices.remove(0)),
        _ => Err(Error::InvalidState(
            "multiple Nanos found; select a device_id from discover()",
        )),
    }
}

trait BleIo: Send {
    async fn write(&mut self, pairing_arm: bool, bytes: &[u8]) -> Result<()>;
    async fn next(&mut self) -> Result<Vec<u8>>;
    async fn disconnect(&mut self) -> Result<()>;
    fn mtu(&self) -> usize;
}
struct NativeIo {
    peripheral: Peripheral,
    _adapter: Adapter,
    command: Characteristic,
    arm: Characteristic,
    notifications: Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
}
impl NativeIo {
    async fn open(device: BleDevice, cancellation: &BleCancellation) -> Result<Self> {
        let peripheral = device.peripheral;
        let result = bounded(
            cancellation,
            GATT_TIMEOUT,
            "connect and discover Nano GATT",
            async {
                peripheral
                    .connect()
                    .await
                    .map_err(|error| bluetooth("connect Nano GATT", error))?;
                peripheral
                    .discover_services()
                    .await
                    .map_err(|error| bluetooth("discover Nano GATT", error))?;
                let chars = peripheral.characteristics();
                let find = |id| {
                    let mut matches = chars.iter().filter(|ch| {
                        ch.service_uuid == uuid_from_u16(SERVICE) && ch.uuid == uuid_from_u16(id)
                    });
                    match (matches.next(), matches.next()) {
                        (Some(characteristic), None) => Ok(characteristic.clone()),
                        _ => Err(Error::Protocol("expected one Nano service characteristic")),
                    }
                };
                let command = find(COMMAND_CHARACTERISTIC)?;
                let arm = find(PAIRING_CHARACTERISTIC)?;
                if !command
                    .properties
                    .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
                    || !arm.properties.contains(CharPropFlags::WRITE)
                {
                    return Err(Error::Protocol("Nano lacks required BLE write modes"));
                }
                let notifications = peripheral
                    .notifications()
                    .await
                    .map_err(|error| bluetooth("open Nano notifications", error))?;
                let mut subscriptions = 0;
                for ch in [&arm, &command] {
                    if ch.properties.contains(CharPropFlags::NOTIFY) {
                        peripheral
                            .subscribe(ch)
                            .await
                            .map_err(|error| bluetooth("subscribe to Nano notifications", error))?;
                        subscriptions += 1;
                    }
                }
                if subscriptions == 0 {
                    return Err(Error::Protocol("Nano offers no BLE notifications"));
                }
                Ok((command, arm, notifications))
            },
        )
        .await;
        match result {
            Ok((command, arm, notifications)) => Ok(Self {
                peripheral,
                _adapter: device._adapter,
                command,
                arm,
                notifications,
            }),
            Err(error) => {
                let cleanup = timeout(IO_TIMEOUT, peripheral.disconnect())
                    .await
                    .map_err(|_| Error::Timeout {
                        operation: "disconnect failed BLE setup",
                    })
                    .and_then(|result| {
                        result.map_err(|error| bluetooth("disconnect failed BLE setup", error))
                    });
                combine(Err(error), cleanup)
            }
        }
    }
}
impl BleIo for NativeIo {
    async fn write(&mut self, pairing_arm: bool, bytes: &[u8]) -> Result<()> {
        let (characteristic, mode) = if pairing_arm {
            (&self.arm, WriteType::WithResponse)
        } else {
            (&self.command, WriteType::WithoutResponse)
        };
        self.peripheral
            .write(characteristic, bytes, mode)
            .await
            .map_err(|error| bluetooth("write Nano BLE characteristic", error))
    }
    async fn next(&mut self) -> Result<Vec<u8>> {
        loop {
            let notification = self.notifications.next().await.ok_or(Error::InvalidState(
                "Nano BLE notifications ended; call connect explicitly",
            ))?;
            if notification.uuid == self.command.uuid || notification.uuid == self.arm.uuid {
                return Ok(notification.value);
            }
        }
    }
    async fn disconnect(&mut self) -> Result<()> {
        self.peripheral
            .disconnect()
            .await
            .map_err(|error| bluetooth("disconnect Nano BLE", error))
    }
    fn mtu(&self) -> usize {
        usize::from(self.peripheral.mtu())
    }
}

async fn connect_worker(
    config: BleConfig,
    identity: String,
    cancellation: BleCancellation,
    receiver: async_mpsc::Receiver<Work>,
    ready: mpsc::SyncSender<Result<()>>,
) -> Result<()> {
    let device = select_device(discover(&cancellation).await?, config.device_id.as_deref())?;
    log::info!("Opening Nano BLE; reusing the saved installation identity");
    let io = NativeIo::open(device, &cancellation).await?;
    let mut worker = Worker::new(io, cancellation);
    let setup = async {
        worker
            .setup(&identity, config.network.as_ref(), config.network_cleanup)
            .await?;
        ready.send(Ok(())).map_err(|_| Error::Cancelled)?;
        worker.serve(receiver).await
    }
    .await;
    let cleanup = worker.cleanup(setup.is_ok()).await;
    combine(setup, cleanup)
}

#[derive(Default)]
struct FrameBuffer(Vec<u8>);
impl FrameBuffer {
    fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Message>> {
        if bytes.len() > MAX_NOTIFICATION_BYTES.saturating_sub(self.0.len()) {
            return Err(Error::LimitExceeded("BLE notification buffer overflow"));
        }
        self.0.extend_from_slice(bytes);
        let mut messages = Vec::new();
        while self.0.len() > DUML_CRC8 {
            let prefix = decode::<DumlPrefix>(&self.0[..DUML_CRC8 + 1], ())?;
            let header = prefix.length;
            let length = usize::from(header & DUML_LENGTH_MASK);
            if prefix.sync != DUML_SYNC
                || header & !DUML_LENGTH_MASK != DUML_VERSION
                || length < DUML_MIN
                || crc8(&self.0[..DUML_CRC8]) != prefix.checksum
            {
                return Err(Error::Protocol("invalid BLE DUML header"));
            }
            if self.0.len() < length {
                break;
            }
            let parsed = Message::scan(&self.0[..length]);
            if parsed.len() != 1 {
                return Err(Error::Protocol("invalid BLE DUML checksum"));
            }
            messages.extend(parsed);
            self.0.drain(..length);
        }
        Ok(messages)
    }
}

struct Worker<I> {
    io: I,
    cancellation: BleCancellation,
    next_id: u16,
    next_write: Instant,
    next_keepalive: Instant,
    frames: FrameBuffer,
    pending: VecDeque<Message>,
    restore_hotspot: bool,
    inactive: bool,
}
impl<I: BleIo> Worker<I> {
    fn new(io: I, cancellation: BleCancellation) -> Self {
        Self {
            io,
            cancellation,
            next_id: FIRST_BLE_ID,
            next_write: Instant::now(),
            next_keepalive: Instant::now() + KEEPALIVE_INTERVAL,
            frames: FrameBuffer::default(),
            pending: VecDeque::new(),
            restore_hotspot: false,
            inactive: false,
        }
    }
    fn request(&mut self, receiver: Endpoint, command: Command, payload: &[u8]) -> Message {
        let id = self.id();
        Message::request(receiver, id, command, payload)
    }
    fn id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = if id == MAX_ID { MIN_ID } else { id + 1 };
        id
    }
    async fn setup(
        &mut self,
        identity: &str,
        network: Option<&WifiNetwork>,
        cleanup: NetworkCleanup,
    ) -> Result<()> {
        self.pair(identity).await?;
        if let Some(network) = network {
            self.join(network, cleanup).await?;
        }
        self.query(BleQuery::WifiCountry).await?.require_success()?;
        log::info!("Nano BLE ready: live read-only health query accepted");
        Ok(())
    }
    async fn write(&mut self, arm: bool, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.io.mtu().saturating_sub(ATT_HEADER) {
            return Err(Error::LimitExceeded(
                "BLE MTU too small for Nano command; no unverified fragmentation",
            ));
        }
        bounded(&self.cancellation, IO_TIMEOUT, "pace BLE write", async {
            sleep_until(self.next_write).await;
            Ok(())
        })
        .await?;
        bounded(
            &self.cancellation,
            IO_TIMEOUT,
            "write Nano BLE command",
            self.io.write(arm, bytes),
        )
        .await?;
        log::trace!("BLE send packet: arm={arm} length={}", bytes.len());
        self.next_write = Instant::now() + WRITE_PACING;
        Ok(())
    }
    async fn send(&mut self, message: &Message) -> Result<()> {
        self.write(false, &encode(&message)?).await?;
        log::debug!("BLE send message: {}", message.diagnostic_metadata());
        Ok(())
    }
    async fn keepalive(&mut self) -> Result<()> {
        self.next_keepalive = Instant::now() + KEEPALIVE_INTERVAL;
        let message = self.request(
            Endpoint::Session,
            Command::Common(CommonCommand::SessionKeepalive),
            &encode(&SessionKeepaliveRequest)?,
        );
        self.send(&message).await
    }
    async fn message(&mut self, until: Instant) -> Result<Option<Message>> {
        loop {
            if self.cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if Instant::now() >= until {
                return Ok(None);
            }
            if Instant::now() >= self.next_keepalive {
                self.keepalive().await?;
            }
            if let Some(message) = self.pending.pop_front() {
                if message.receiver == Endpoint::App && message.flags == Flags::REQUEST {
                    let payload = if message.command == Command::Common(CommonCommand::DeviceInfo) {
                        encode(&AppDeviceInfoResponse)?
                    } else if message.command == Command::Wifi(WifiCommand::PairApproval) {
                        encode(&ResponseStatus::Success)?
                    } else {
                        message.payload.clone()
                    };
                    self.send(&Message {
                        sender: message.receiver,
                        receiver: message.sender,
                        flags: Flags::ACKNOWLEDGMENT,
                        payload,
                        ..message.clone()
                    })
                    .await?;
                }
                if message.command == Command::Common(CommonCommand::ActivationStatus)
                    && let Ok(notification) =
                        decode::<ActivationStatusNotification>(&message.payload, ())
                {
                    self.inactive = matches!(
                        notification.state,
                        ActivationState::Unknown00 | ActivationState::Unknown02
                    );
                }
                return Ok(Some(message));
            }
            if Instant::now() >= until {
                return Ok(None);
            }
            tokio::select! {
                biased;
                _ = self.cancellation.cancelled() => return Err(Error::Cancelled),
                _ = sleep_until(until.min(self.next_keepalive)) => {},
                bytes = self.io.next() => {
                    let bytes = bytes?;
                    log::trace!("BLE receive packet: length={}", bytes.len());
                    let messages = self.frames.feed(&bytes)?;
                    for message in &messages {
                        log::debug!("BLE receive message: {}", message.diagnostic_metadata());
                    }
                    self.pending.extend(messages);
                    if self.pending.len() > MAX_PENDING_FRAMES {
                        return Err(Error::LimitExceeded("BLE pending frame overflow"));
                    }
                },
            }
        }
    }
    async fn pause(&mut self, duration: Duration) -> Result<()> {
        let until = Instant::now() + duration;
        while self.message(until).await?.is_some() {}
        Ok(())
    }
    async fn response(&mut self, request: &Message, duration: Duration) -> Result<Message> {
        let until = Instant::now() + duration;
        while let Some(message) = self.message(until).await? {
            if message.is_reply_to(request) {
                log::debug!("BLE matched reply: {}", message.diagnostic_metadata());
                return Ok(message);
            }
        }
        Err(Error::ReplyTimeout {
            command: request.command,
        })
    }
    async fn query(&mut self, query: BleQuery) -> Result<Message> {
        let request = query.request(self.id())?;
        self.send(&request).await?;
        self.response(&request, QUERY_TIMEOUT).await
    }
    async fn raw(&mut self, request: ExplorationRequest) -> Result<Message> {
        let message = request.message(self.id());
        self.send(&message).await?;
        self.response(&message, request.timeout).await
    }
    async fn pair(&mut self, identity: &str) -> Result<()> {
        log::info!("Arming Nano pairing and waking BLE session");
        self.write(true, &encode(&BlePairingArmRequest)?).await?;
        let open = self.request(
            Endpoint::Session,
            Command::Common(CommonCommand::SessionKeepalive),
            &encode(&SessionKeepaliveOpenRequest)?,
        );
        self.send(&open).await?;
        self.pause(PAIR_SETTLE).await?;
        let payload = encode(&PairingRequest { identity })?;
        let first = self.request(Endpoint::Wifi, Command::Wifi(WifiCommand::Pair), &payload);
        self.send(&first).await?;
        let mut requests = vec![first];
        let start = Instant::now();
        let deadline = start + PAIR_TIMEOUT;
        let mut retry = 0;
        log::info!("Pairing requested; approve OSMO on the camera/dock if prompted");
        loop {
            let retry_at = PAIR_RETRY_TICKS
                .get(retry)
                .map(|tick| start + KEEPALIVE_INTERVAL * *tick);
            let until = retry_at.unwrap_or(deadline);
            if let Some(message) = self.message(until).await? {
                if pairing_approved(&message, &requests)? {
                    break;
                }
            } else if retry_at.is_some() {
                log::info!("Pairing approval pending; repeating the known pairing request");
                let request =
                    self.request(Endpoint::Wifi, Command::Wifi(WifiCommand::Pair), &payload);
                self.send(&request).await?;
                requests.push(request);
                retry += 1;
            } else {
                return Err(Error::Timeout {
                    operation: "Nano pairing approval",
                });
            }
        }
        self.pause(APPROVAL_SETTLE).await?;
        let wake = self.request(
            Endpoint::CameraWake,
            Command::CameraWake(CameraWakeCommand::Wake),
            &encode(&BleWakeRequest)?,
        );
        self.send(&wake).await?;
        self.pause(WAKE_SETTLE).await?;
        log::info!("Nano pairing and wake complete");
        Ok(())
    }
    async fn credentials(&mut self) -> Result<HotspotCredentials> {
        let mut ssid = None;
        let mut password = None;
        for attempt in 0..MAX_CREDENTIAL_ATTEMPTS {
            if ssid.is_none() {
                let request = self.request(
                    Endpoint::Wifi,
                    Command::Wifi(WifiCommand::Ssid),
                    &encode(&WifiSsidGet)?,
                );
                self.send(&request).await?;
                match self.response(&request, IO_TIMEOUT).await {
                    Ok(message) => {
                        message.require_success()?;
                        if let CameraReadReply::Accepted(value) =
                            decode::<CameraReadReply<WifiSsidResponse>>(&message.payload, ())?
                        {
                            ssid = Some(value.ssid);
                        }
                    }
                    Err(Error::ReplyTimeout { .. }) => {}
                    Err(error) => return Err(error),
                }
            }
            self.pause(CREDENTIAL_PACING).await?;
            if password.is_none() {
                let request = self.request(
                    Endpoint::Wifi,
                    Command::Wifi(WifiCommand::Password),
                    &encode(&WifiPasswordRequest)?,
                );
                self.send(&request).await?;
                match self.response(&request, IO_TIMEOUT).await {
                    Ok(message) => {
                        message.require_success()?;
                        if let CameraReadReply::Accepted(value) =
                            decode::<CameraReadReply<WifiPasswordResponse>>(&message.payload, ())?
                        {
                            password = Some(value.password);
                        }
                    }
                    Err(Error::ReplyTimeout { .. }) => {}
                    Err(error) => return Err(error),
                }
            }
            if ssid.is_some() && password.is_some() {
                break;
            }
            if self.inactive {
                return Err(Error::InvalidState(
                    "Nano activation unknown; activate once with DJI Mimo",
                ));
            }
            if attempt + 1 < MAX_CREDENTIAL_ATTEMPTS {
                log::warn!(
                    "Hotspot credential reply missing; waking and retrying only missing queries"
                );
                let wake = self.request(
                    Endpoint::CameraWake,
                    Command::CameraWake(CameraWakeCommand::Wake),
                    &encode(&BleWakeRequest)?,
                );
                self.send(&wake).await?;
                self.pause(WAKE_SETTLE).await?;
            }
        }
        match (ssid, password) {
            (Some(ssid), Some(password))
                if !ssid.trim().is_empty() && password.len() >= MIN_PASSWORD_LENGTH =>
            {
                Ok(HotspotCredentials { ssid, password })
            }
            _ => Err(Error::InvalidState(
                "Nano did not provide valid hotspot credentials after bounded attempts",
            )),
        }
    }
    async fn set_role(&mut self, role: WifiRole) -> Result<()> {
        let request = self.request(
            Endpoint::Wifi,
            Command::Wifi(WifiCommand::SetRole),
            &encode(&WifiRoleSetRequest { role })?,
        );
        self.send(&request).await?;
        self.response(&request, ROLE_TIMEOUT)
            .await?
            .require_success()
    }
    async fn join(&mut self, network: &WifiNetwork, cleanup: NetworkCleanup) -> Result<()> {
        self.restore_hotspot = cleanup == NetworkCleanup::RestoreHotspot;
        log::info!("Selecting camera station mode; computer Wi-Fi is unchanged");
        self.set_role(WifiRole::Station).await?;
        self.pause(ROLE_SETTLE).await?;
        let payload = encode(&JoinNetworkRequest {
            ssid: &network.ssid,
            password: &network.password,
        })?;
        for attempt in 0..MAX_JOIN_ATTEMPTS {
            log::info!(
                "Joining configured router: attempt {}/{}",
                attempt + 1,
                MAX_JOIN_ATTEMPTS
            );
            let request = self.request(
                Endpoint::Wifi,
                Command::Wifi(WifiCommand::JoinNetwork),
                &payload,
            );
            self.send(&request).await?;
            let reply = self.response(&request, JOIN_TIMEOUT).await?;
            let response =
                decode::<CameraReadReply<CameraRawData>>(&reply.payload, ()).map_err(|_| {
                    Error::InvalidReply {
                        command: reply.command,
                        reason: "missing status byte",
                    }
                })?;
            if matches!(&response, CameraReadReply::Accepted(data) if data.0.iter().all(|byte| *byte == 0))
            {
                log::info!("Camera accepted router join; LAN reachability is not yet established");
                return Ok(());
            }
            if matches!(&response, CameraReadReply::Rejected { status, extension }
                if *status == JOIN_RETRY_STATUS && extension.0.first() == Some(&JOIN_RETRY_DETAIL))
                && attempt + 1 < MAX_JOIN_ATTEMPTS
            {
                log::warn!(
                    "Camera temporarily refused router join; retrying the same network after delay"
                );
                self.pause(JOIN_RETRY_DELAY).await?;
                continue;
            }
            reply.require_success()?;
            return Err(Error::InvalidReply {
                command: request.command,
                reason: "unexpected router join reply",
            });
        }
        Err(Error::InvalidState("router join attempt limit reached"))
    }
    async fn restore(&mut self) -> Result<()> {
        log::info!("Restoring camera hotspot");
        self.set_role(WifiRole::Hotspot).await?;
        self.restore_hotspot = false;
        Ok(())
    }
    async fn serve(&mut self, mut commands: async_mpsc::Receiver<Work>) -> Result<()> {
        loop {
            match commands.try_recv() {
                Ok(work) => {
                    let close = matches!(work.operation, Operation::Close);
                    let result = match work.operation {
                        Operation::Query(query) => self.query(query).await.map(Reply::Message),
                        Operation::Raw(request) => self.raw(request).await.map(Reply::Message),
                        Operation::Receive(duration) => self
                            .message(Instant::now() + duration)
                            .await
                            .map(Reply::Received),
                        Operation::Credentials => self.credentials().await.map(Reply::Credentials),
                        Operation::Join(network, cleanup) => {
                            self.join(&network, cleanup).await.map(|_| Reply::Done)
                        }
                        Operation::Restore => self.restore().await.map(|_| Reply::Done),
                        Operation::Close => Ok(Reply::Done),
                    };
                    let failure = result.as_ref().err().cloned();
                    work.response.send(result).map_err(|_| Error::Cancelled)?;
                    if let Some(error) = failure {
                        return Err(error);
                    }
                    if close {
                        return Ok(());
                    }
                }
                Err(async_mpsc::error::TryRecvError::Disconnected) => return Err(Error::Cancelled),
                Err(async_mpsc::error::TryRecvError::Empty) => {
                    self.message(Instant::now() + CANCEL_POLL).await?;
                }
            }
        }
    }
    async fn cleanup(&mut self, orderly: bool) -> Result<()> {
        let restore = if self.restore_hotspot {
            if orderly {
                self.restore().await
            } else {
                Err(Error::InvalidState(
                    "hotspot restoration unconfirmed after BLE failure/cancellation; no reconnect attempted",
                ))
            }
        } else {
            Ok(())
        };
        let disconnect = timeout(IO_TIMEOUT, self.io.disconnect())
            .await
            .map_err(|_| Error::Timeout {
                operation: "disconnect Nano BLE",
            })
            .and_then(|result| result);
        combine(restore, disconnect)
    }
}

fn pairing_approved(message: &Message, requests: &[Message]) -> Result<bool> {
    if requests.iter().any(|request| message.is_reply_to(request)) {
        message.require_success()?;
        let response =
            decode::<CameraReadReply<PairingResponse>>(&message.payload, ()).map_err(|_| {
                Error::InvalidReply {
                    command: message.command,
                    reason: "pairing rejected or missing approval state",
                }
            })?;
        return match response {
            CameraReadReply::Accepted(PairingResponse {
                state: PairingState::Approved,
                ..
            }) => Ok(true),
            CameraReadReply::Accepted(PairingResponse {
                state: PairingState::AwaitingApproval,
                ..
            }) => Ok(false),
            _ => Err(Error::InvalidReply {
                command: message.command,
                reason: "pairing rejected or missing approval state",
            }),
        };
    }
    if message.sender == Endpoint::Wifi
        && message.receiver == Endpoint::App
        && message.flags == Flags::REQUEST
        && message.command == Command::Wifi(WifiCommand::PairApproval)
    {
        let notification =
            decode::<PairingApprovalNotification>(&message.payload, ()).map_err(|_| {
                Error::InvalidReply {
                    command: message.command,
                    reason: "pairing approval empty, rejected or expired",
                }
            })?;
        return match notification.approval {
            PairingApproval::Accepted00 | PairingApproval::Accepted01 => Ok(true),
            _ => Err(Error::InvalidReply {
                command: message.command,
                reason: "pairing approval empty, rejected or expired",
            }),
        };
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    const TEST_IDENTITY: &str = "0123456789abcdef0123456789abcdef";
    const SUCCESS: &[u8] = &[0];
    const PAIR_APPROVED: &[u8] = &[0, 1];
    const PAIR_PENDING: &[u8] = &[0, 2];
    const REJECTED: &[u8] = &[0xd6];
    const TEST_SSID: &str = "synthetic-router";
    const TEST_PASSWORD: &str = "synthetic-password";
    const TEST_MTU: usize = 247;

    fn join_retry_reply() -> Vec<u8> {
        encode(&CameraReadReply::<CameraRawData>::Rejected {
            status: JOIN_RETRY_STATUS,
            extension: CameraRawData(vec![JOIN_RETRY_DETAIL]),
        })
        .unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn activation_body_dispatch_preserves_routing_and_ignores_other_layouts() {
        use crate::protocol::commands::common::cmd_00_32_activation_status::ACTIVATION_UNKNOWN_LENGTH;

        let mut worker = worker(MockIo::default());
        for sender in [Endpoint::Camera, Endpoint::Session] {
            for flags in [Flags::RESPONSE, Flags::ACKNOWLEDGMENT, Flags::REQUEST] {
                let payload = encode(&ActivationStatusNotification {
                    unknown: [0xa5; ACTIVATION_UNKNOWN_LENGTH],
                    state: ActivationState::Unknown02,
                    extension: CameraRawData(vec![0xab]),
                })
                .unwrap();
                let message = Message {
                    sender,
                    receiver: Endpoint::App,
                    id: FIRST_BLE_ID,
                    flags,
                    command: Command::Common(CommonCommand::ActivationStatus),
                    payload,
                };
                worker.inactive = false;
                worker.pending.push_back(message.clone());
                assert_eq!(
                    worker.message(Instant::now() + IO_TIMEOUT).await.unwrap(),
                    Some(message)
                );
                assert!(worker.inactive);
            }
        }
        for payload in [vec![], vec![0x33], vec![0; ACTIVATION_UNKNOWN_LENGTH]] {
            let message = Message {
                sender: Endpoint::Camera,
                receiver: Endpoint::App,
                id: FIRST_BLE_ID,
                flags: Flags::RESPONSE,
                command: Command::Common(CommonCommand::ActivationStatus),
                payload,
            };
            worker.inactive = false;
            worker.pending.push_back(message.clone());
            assert_eq!(
                worker.message(Instant::now() + IO_TIMEOUT).await.unwrap(),
                Some(message)
            );
            assert!(!worker.inactive);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn join_accepts_only_nonempty_all_zero_success_and_preserves_failures() {
        for payload in [vec![0], vec![0; 4]] {
            let mut worker = worker(MockIo {
                join_replies: VecDeque::from([payload]),
                ..MockIo::default()
            });
            worker
                .join(&router(), NetworkCleanup::Preserve)
                .await
                .unwrap();
        }
        for payload in [vec![], vec![0, 1], REJECTED.to_vec()] {
            let rejection = payload == REJECTED;
            let mut worker = worker(MockIo {
                join_replies: VecDeque::from([payload]),
                ..MockIo::default()
            });
            let error = worker
                .join(&router(), NetworkCleanup::Preserve)
                .await
                .unwrap_err();
            if rejection {
                assert!(matches!(error, Error::CameraRejected { .. }));
            } else {
                assert!(matches!(error, Error::InvalidReply { .. }));
            }
            assert_eq!(
                outgoing(&worker)
                    .iter()
                    .filter(|message| message.command == Command::Wifi(WifiCommand::JoinNetwork))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn preview_cancellation_and_closed_owner_fail_before_lan_setup() {
        let (commands, _receiver) = async_mpsc::channel(1);
        let session = BleSession {
            commands,
            cancellation: BleCancellation::default(),
            worker: None,
            lease: None,
            pairing_identity: TEST_IDENTITY.into(),
        };
        let cancellation = PreviewCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            session.start_preview_cancellable("127.0.0.1".parse().unwrap(), &cancellation),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            session.start_preview("127.0.0.1".parse().unwrap()),
            Err(Error::InvalidState(_))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn capability_queries_reuse_correlated_worker_and_preserve_rejections() {
        for query in [
            CapabilityQuery::Accessories,
            CapabilityQuery::EiRange,
            CapabilityQuery::RecommendedEi,
            CapabilityQuery::LensInformation,
        ] {
            let request = BleQuery::Capability(query).request(FIRST_BLE_ID).unwrap();
            let expected = reply(&request, &[0xe0]);
            let mut wrong = expected.clone();
            wrong.id += 1;
            let mut io = MockIo::default();
            io.incoming
                .extend([encode(&wrong).unwrap(), encode(&expected).unwrap()]);
            let mut worker = worker(io);
            let actual = worker.query(BleQuery::Capability(query)).await.unwrap();
            assert_eq!(actual, expected);
            assert!(matches!(
                decode::<CapabilityReply>(&actual.payload, (&query,)).unwrap(),
                CapabilityReply::Rejected { .. }
            ));
            assert_eq!(outgoing(&worker), [request]);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn capability_timeout_and_cancellation_do_not_retry_or_reconnect() {
        let query = BleQuery::Capability(CapabilityQuery::Accessories);
        let mut timed_out = worker(MockIo::default());
        assert!(matches!(
            timed_out.query(query).await,
            Err(Error::ReplyTimeout { .. })
        ));
        assert_eq!(
            outgoing(&timed_out)
                .iter()
                .filter(|message| message.command == Command::Camera(CameraCommand::Capabilities))
                .count(),
            1
        );
        assert!(
            !outgoing(&timed_out)
                .iter()
                .any(|message| message.command == Command::Wifi(WifiCommand::Pair))
        );
        let mut cancelled = worker(MockIo::default());
        cancelled.cancellation.cancel();
        assert!(matches!(
            cancelled.query(query).await,
            Err(Error::Cancelled)
        ));
        assert!(cancelled.io.writes.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn raw_exploration_preserves_numeric_payload_and_correlation() {
        const TEST_SET: u8 = 0xfd;
        const TEST_COMMAND: u8 = 0x99;
        let request = ExplorationRequest {
            receiver: Endpoint::Camera,
            command: Command::from_wire(TEST_SET, TEST_COMMAND),
            flags: Flags::REQUEST,
            payload: vec![0, 254],
            timeout: Duration::from_secs(1),
        };
        request.validate(true, true).unwrap();
        let message = request.message(FIRST_BLE_ID);
        let expected = reply(&message, &[0xe0]);
        let mut wrong = expected.clone();
        wrong.receiver = Endpoint::Wifi;
        let mut io = MockIo::default();
        io.incoming
            .extend([encode(&wrong).unwrap(), encode(&expected).unwrap()]);
        let mut active = worker(io);
        assert_eq!(active.raw(request.clone()).await.unwrap(), expected);
        assert_eq!(outgoing(&active), [message]);
        let mut cancelled = worker(MockIo::default());
        cancelled.cancellation.cancel();
        assert!(matches!(
            cancelled.raw(request).await,
            Err(Error::Cancelled)
        ));
        assert!(cancelled.io.writes.is_empty());
    }

    struct MockIo {
        writes: Vec<(bool, Vec<u8>)>,
        write_times: Vec<Instant>,
        incoming: VecDeque<Vec<u8>>,
        pair_reply: Option<Vec<u8>>,
        approval_request: bool,
        join_replies: VecDeque<Vec<u8>>,
        drop_credentials: bool,
        drop_role: bool,
        drop_join: bool,
        drop_health: bool,
        health_reply: Vec<u8>,
        mtu: usize,
        disconnected: bool,
        disconnect_fails: bool,
        link_lost: bool,
    }
    impl Default for MockIo {
        fn default() -> Self {
            Self {
                writes: vec![],
                write_times: vec![],
                incoming: VecDeque::new(),
                pair_reply: Some(PAIR_APPROVED.to_vec()),
                approval_request: false,
                join_replies: VecDeque::new(),
                drop_credentials: false,
                drop_role: false,
                drop_join: false,
                drop_health: false,
                health_reply: SUCCESS.to_vec(),
                mtu: TEST_MTU,
                disconnected: false,
                disconnect_fails: false,
                link_lost: false,
            }
        }
    }
    impl BleIo for MockIo {
        async fn write(&mut self, arm: bool, bytes: &[u8]) -> Result<()> {
            self.writes.push((arm, bytes.to_vec()));
            self.write_times.push(Instant::now());
            if arm {
                return Ok(());
            }
            let request = Message::scan(bytes).pop().expect("valid outgoing DUML");
            if request.flags.is_response() {
                return Ok(());
            }
            let mut response = reply(&request, SUCCESS);
            match request.command {
                Command::Wifi(WifiCommand::Pair) if self.approval_request => {
                    response.command = Command::Wifi(WifiCommand::PairApproval);
                    response.flags = Flags::REQUEST;
                }
                Command::Wifi(WifiCommand::Pair) => match &self.pair_reply {
                    Some(payload) => response.payload = payload.clone(),
                    None => return Ok(()),
                },
                Command::Wifi(WifiCommand::Ssid | WifiCommand::Password) => {
                    if self.drop_credentials {
                        return Ok(());
                    }
                    response.payload = if request.command == Command::Wifi(WifiCommand::Ssid) {
                        encode(&CameraReadReply::Accepted(WifiSsidResponse {
                            ssid: TEST_SSID.to_owned(),
                            extension: CameraRawData(vec![]),
                        }))?
                    } else {
                        encode(&CameraReadReply::Accepted(WifiPasswordResponse {
                            password: TEST_PASSWORD.to_owned(),
                            extension: CameraRawData(vec![]),
                        }))?
                    };
                }
                Command::Wifi(WifiCommand::SetRole) if self.drop_role => return Ok(()),
                Command::Wifi(WifiCommand::SetRole) => {}
                Command::Wifi(WifiCommand::JoinNetwork) => {
                    if self.drop_join {
                        return Ok(());
                    }
                    response.payload = self
                        .join_replies
                        .pop_front()
                        .unwrap_or_else(|| SUCCESS.to_vec());
                }
                Command::Wifi(WifiCommand::Country) if self.drop_health => return Ok(()),
                Command::Wifi(WifiCommand::Country) => response.payload = self.health_reply.clone(),
                _ => return Ok(()),
            }
            self.incoming.push_back(encode(&response)?);
            Ok(())
        }
        async fn next(&mut self) -> Result<Vec<u8>> {
            if self.link_lost {
                return Err(Error::InvalidState("synthetic BLE link lost"));
            }
            match self.incoming.pop_front() {
                Some(bytes) => Ok(bytes),
                None => std::future::pending().await,
            }
        }
        async fn disconnect(&mut self) -> Result<()> {
            self.disconnected = true;
            if self.disconnect_fails {
                Err(Error::InvalidState("synthetic disconnect failure"))
            } else {
                Ok(())
            }
        }
        fn mtu(&self) -> usize {
            self.mtu
        }
    }
    fn reply(request: &Message, payload: &[u8]) -> Message {
        Message {
            sender: request.receiver,
            receiver: request.sender,
            flags: Flags::ACKNOWLEDGMENT,
            payload: payload.to_vec(),
            ..request.clone()
        }
    }
    fn worker(io: MockIo) -> Worker<MockIo> {
        Worker::new(io, BleCancellation::default())
    }
    fn outgoing(worker: &Worker<MockIo>) -> Vec<Message> {
        worker
            .io
            .writes
            .iter()
            .filter(|(arm, _)| !arm)
            .flat_map(|(_, bytes)| Message::scan(bytes))
            .collect()
    }
    fn router() -> WifiNetwork {
        WifiNetwork::new(TEST_SSID.into(), TEST_PASSWORD.into()).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn standard_log_progress_never_contains_identity_or_credentials() {
        struct Logs(std::sync::Mutex<Vec<String>>);
        impl log::Log for Logs {
            fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
                metadata.target().starts_with("unofficial_osmo_camera::ble")
            }
            fn log(&self, record: &log::Record<'_>) {
                if self.enabled(record.metadata()) {
                    self.0.lock().unwrap().push(record.args().to_string());
                }
            }
            fn flush(&self) {}
        }
        static LOGS: Logs = Logs(std::sync::Mutex::new(Vec::new()));
        log::set_logger(&LOGS).unwrap();
        log::set_max_level(log::LevelFilter::Debug);
        let mut worker = worker(MockIo::default());
        worker
            .setup(TEST_IDENTITY, Some(&router()), NetworkCleanup::Preserve)
            .await
            .unwrap();
        worker.credentials().await.unwrap();
        worker.cleanup(true).await.unwrap();
        let output = LOGS.0.lock().unwrap().join("\n");
        assert!(output.contains("Pairing requested"));
        assert!(output.contains("live read-only health query accepted"));
        assert!(output.contains("Joining configured router"));
        for secret in [TEST_IDENTITY, TEST_SSID, TEST_PASSWORD] {
            assert!(!output.contains(secret));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn response_correlation_ignores_wrong_endpoints_ids_commands_and_flags() {
        let request = BleQuery::WifiCountry.request(FIRST_BLE_ID).unwrap();
        let valid = reply(&request, SUCCESS);
        let mut io = MockIo::default();
        for message in [
            Message {
                id: request.id + 1,
                ..valid.clone()
            },
            Message {
                sender: Endpoint::Camera,
                ..valid.clone()
            },
            Message {
                receiver: Endpoint::Camera,
                ..valid.clone()
            },
            Message {
                command: Command::Wifi(WifiCommand::Ssid),
                ..valid.clone()
            },
            Message {
                flags: Flags::REQUEST,
                ..valid.clone()
            },
            valid.clone(),
        ] {
            io.incoming.push_back(encode(&message).unwrap());
        }
        let mut worker = worker(io);
        assert_eq!(
            worker.response(&request, QUERY_TIMEOUT).await.unwrap(),
            valid
        );
    }

    #[tokio::test(start_paused = true)]
    async fn rejected_or_empty_health_reply_never_completes_setup() {
        for payload in [REJECTED.to_vec(), vec![]] {
            let mut worker = worker(MockIo {
                health_reply: payload,
                ..MockIo::default()
            });
            assert!(
                worker
                    .setup(TEST_IDENTITY, None, NetworkCleanup::Preserve)
                    .await
                    .is_err()
            );
        }
    }

    #[test]
    fn ble_requests_match_wire_contracts_and_use_exact_gets() {
        assert_eq!(
            (SERVICE, PAIRING_CHARACTERISTIC, COMMAND_CHARACTERISTIC),
            (0xfff0, 0xfff4, 0xfff5)
        );
        assert_eq!(encode(&BlePairingArmRequest).unwrap(), [1, 0]);
        assert_eq!(encode(&SessionKeepaliveOpenRequest).unwrap(), [4, 0]);
        assert_eq!(encode(&SessionKeepaliveRequest).unwrap(), [1, 1]);
        assert_eq!(encode(&BleWakeRequest).unwrap(), [0, 0, 0, 0]);
        for (query, endpoint, wire, payload) in [
            (BleQuery::WifiSsid, 0x07, (0x07, 0x07), vec![]),
            (BleQuery::WifiMac, 0x07, (0x07, 0x0c), vec![]),
            (BleQuery::WifiCountry, 0x07, (0x07, 0x19), vec![]),
            (BleQuery::WifiRole, 0x07, (0x07, 0x39), vec![0]),
            (BleQuery::CameraFov, 0x01, (0x02, 0x8e), vec![0, 1, 9, 0]),
            (
                BleQuery::CameraIsoLimit,
                0x01,
                (0x02, 0x8e),
                vec![0, 1, 15, 0],
            ),
            (
                BleQuery::CameraParameter0014,
                0x01,
                (0x02, 0x8e),
                vec![0, 1, 20, 0],
            ),
        ] {
            let request = query.request(FIRST_BLE_ID).unwrap();
            assert_eq!(
                encode(&request.receiver).expect("one-byte enum encodes into memory")[0],
                endpoint
            );
            assert_eq!(request.command.wire(), wire);
            assert_eq!(request.payload, payload);
            assert_eq!(request.flags, Flags::REQUEST);
        }
        let payload = encode(&PairingRequest {
            identity: TEST_IDENTITY,
        })
        .unwrap();
        assert_eq!(payload[0] as usize, IDENTITY_LENGTH);
        assert_eq!(&payload[1..=IDENTITY_LENGTH], TEST_IDENTITY.as_bytes());
        assert_eq!(&payload[IDENTITY_LENGTH + 1..], b"\x04osmo");
    }

    #[test]
    fn frame_buffer_preserves_every_fragment_boundary_and_multiple_frames() {
        let request = BleQuery::WifiCountry.request(FIRST_BLE_ID).unwrap();
        let message = reply(&request, SUCCESS);
        let bytes = encode(&message).unwrap();
        for split in 0..=bytes.len() {
            let mut buffer = FrameBuffer::default();
            let mut messages = buffer.feed(&bytes[..split]).unwrap();
            messages.extend(buffer.feed(&bytes[split..]).unwrap());
            assert_eq!(messages, std::slice::from_ref(&message));
            assert!(buffer.0.is_empty());
        }
        let mut both = bytes.clone();
        both.extend(&bytes);
        assert_eq!(
            FrameBuffer::default().feed(&both).unwrap(),
            [message.clone(), message]
        );
        let mut corrupted = bytes;
        corrupted[DUML_COMMAND] ^= 1;
        assert!(FrameBuffer::default().feed(&corrupted).is_err());
        assert!(
            FrameBuffer::default()
                .feed(&vec![0; MAX_NOTIFICATION_BYTES + 1])
                .is_err()
        );
    }

    #[test]
    fn pairing_requires_exact_endpoints_ids_flags_status_and_approval() {
        let request = Message::request(
            Endpoint::Wifi,
            FIRST_BLE_ID,
            Command::Wifi(WifiCommand::Pair),
            &[],
        );
        let approved = reply(&request, PAIR_APPROVED);
        assert!(pairing_approved(&approved, std::slice::from_ref(&request)).unwrap());
        for wrong in [
            Message {
                id: request.id + 1,
                ..approved.clone()
            },
            Message {
                receiver: Endpoint::Camera,
                ..approved.clone()
            },
            Message {
                sender: Endpoint::Camera,
                ..approved.clone()
            },
            Message {
                flags: Flags::REQUEST,
                ..approved.clone()
            },
        ] {
            assert!(!pairing_approved(&wrong, std::slice::from_ref(&request)).unwrap());
        }
        assert!(
            !pairing_approved(
                &reply(&request, PAIR_PENDING),
                std::slice::from_ref(&request)
            )
            .unwrap()
        );
        assert!(pairing_approved(&reply(&request, REJECTED), &[request]).is_err());
        let approval = Message {
            command: Command::Wifi(WifiCommand::PairApproval),
            flags: Flags::REQUEST,
            payload: vec![],
            ..approved
        };
        assert!(pairing_approved(&approval, &[]).is_err());
        for value in [0x00, 0x01] {
            assert!(
                pairing_approved(
                    &Message {
                        payload: vec![value],
                        ..approval.clone()
                    },
                    &[]
                )
                .unwrap()
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn one_call_setup_pairs_wakes_keeps_alive_joins_and_checks_health() {
        let mut worker = worker(MockIo::default());
        worker
            .setup(
                TEST_IDENTITY,
                Some(&router()),
                NetworkCleanup::RestoreHotspot,
            )
            .await
            .unwrap();
        assert!(
            worker
                .io
                .write_times
                .windows(2)
                .all(|times| times[1] - times[0] >= WRITE_PACING)
        );
        assert_eq!(
            worker.io.writes[0],
            (true, encode(&BlePairingArmRequest).unwrap())
        );
        let messages = outgoing(&worker);
        for (endpoint, command, payload) in [
            (
                Endpoint::Session,
                Command::Common(CommonCommand::SessionKeepalive),
                encode(&SessionKeepaliveOpenRequest).unwrap(),
            ),
            (
                Endpoint::Wifi,
                Command::Wifi(WifiCommand::Pair),
                encode(&PairingRequest {
                    identity: TEST_IDENTITY,
                })
                .unwrap(),
            ),
            (
                Endpoint::CameraWake,
                Command::CameraWake(CameraWakeCommand::Wake),
                encode(&BleWakeRequest).unwrap(),
            ),
            (
                Endpoint::Wifi,
                Command::Wifi(WifiCommand::SetRole),
                vec![encode(&WifiRole::Station).expect("one-byte enum encodes into memory")[0]],
            ),
        ] {
            assert!(messages.iter().any(|message| message.receiver == endpoint
                && message.command == command
                && message.payload == payload));
        }
        assert!(
            messages
                .iter()
                .filter(|message| message.payload == encode(&SessionKeepaliveRequest).unwrap())
                .count()
                >= 10
        );
        let join = messages
            .iter()
            .find(|message| message.command == Command::Wifi(WifiCommand::JoinNetwork))
            .unwrap();
        let expected = encode(&JoinNetworkRequest {
            ssid: TEST_SSID,
            password: TEST_PASSWORD,
        })
        .unwrap();
        assert_eq!(join.payload, expected);
        assert_eq!(
            messages.last().unwrap().command,
            Command::Wifi(WifiCommand::Country)
        );
        worker.cleanup(true).await.unwrap();
        assert!(worker.io.disconnected);
        assert!(outgoing(&worker).iter().any(|message| message.command
            == Command::Wifi(WifiCommand::SetRole)
            && message.payload
                == [encode(&WifiRole::Hotspot).expect("one-byte enum encodes into memory")[0]]));
    }

    #[tokio::test(start_paused = true)]
    async fn default_setup_preserves_wifi_and_does_not_fetch_secrets() {
        let mut worker = worker(MockIo::default());
        worker
            .setup(TEST_IDENTITY, None, NetworkCleanup::Preserve)
            .await
            .unwrap();
        worker.cleanup(true).await.unwrap();
        assert!(!outgoing(&worker).iter().any(|message| matches!(
            message.command,
            Command::Wifi(WifiCommand::Password | WifiCommand::SetRole | WifiCommand::JoinNetwork)
        )));
    }

    #[tokio::test(start_paused = true)]
    async fn unsolicited_pairing_approval_is_acknowledged_before_wake() {
        let mut worker = worker(MockIo {
            approval_request: true,
            ..MockIo::default()
        });
        worker.pair(TEST_IDENTITY).await.unwrap();
        let messages = outgoing(&worker);
        let ack = messages
            .iter()
            .position(|message| {
                message.command == Command::Wifi(WifiCommand::PairApproval)
                    && message.flags == Flags::ACKNOWLEDGMENT
            })
            .unwrap();
        let wake = messages
            .iter()
            .position(|message| message.receiver == Endpoint::CameraWake)
            .unwrap();
        assert!(ack < wake);
        assert_eq!(
            messages[ack].payload,
            encode(&ResponseStatus::Success).unwrap()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn pairing_timeout_has_only_two_repeats_and_no_wake_or_reconnect() {
        let mut worker = worker(MockIo {
            pair_reply: None,
            ..MockIo::default()
        });
        let start = Instant::now();
        assert!(matches!(
            worker.pair(TEST_IDENTITY).await,
            Err(Error::Timeout { .. })
        ));
        assert!(Instant::now() - start < PAIR_TIMEOUT + IO_TIMEOUT);
        let messages = outgoing(&worker);
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.command == Command::Wifi(WifiCommand::Pair))
                .count(),
            PAIR_RETRY_TICKS.len() + 1
        );
        assert!(
            !messages
                .iter()
                .any(|message| message.receiver == Endpoint::CameraWake)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn health_timeout_does_not_claim_a_responsive_connection() {
        let mut worker = worker(MockIo {
            drop_health: true,
            ..MockIo::default()
        });
        assert!(matches!(
            worker
                .setup(TEST_IDENTITY, None, NetworkCleanup::Preserve)
                .await,
            Err(Error::ReplyTimeout {
                command: Command::Wifi(WifiCommand::Country)
            })
        ));
        assert_eq!(
            outgoing(&worker)
                .iter()
                .filter(|message| message.command == Command::Wifi(WifiCommand::Pair))
                .count(),
            1
        );
    }

    #[tokio::test(start_paused = true)]
    async fn credentials_are_parsed_and_debug_redacts_both_fields() {
        let mut worker = worker(MockIo::default());
        let credentials = worker.credentials().await.unwrap();
        assert_eq!(credentials.ssid, TEST_SSID);
        assert_eq!(credentials.password, TEST_PASSWORD);
        assert!(!format!("{credentials:?}").contains(TEST_PASSWORD));
        assert!(!format!("{credentials:?}").contains(TEST_SSID));
        assert!(!format!("{:?}", router()).contains(TEST_PASSWORD));
        let request = Message::request(
            Endpoint::Wifi,
            FIRST_BLE_ID,
            Command::Wifi(WifiCommand::Password),
            &encode(&WifiPasswordRequest).unwrap(),
        );
        for payload in [&[][..], &[0][..], &[0, 8, 1][..], REJECTED] {
            let message = reply(&request, payload);
            assert!(
                message
                    .require_success()
                    .and_then(|()| decode::<CameraReadReply::<WifiPasswordResponse>>(
                        &message.payload,
                        ()
                    )
                    .map(|_| ()))
                    .is_err()
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn missing_credentials_are_bounded_and_do_not_join_a_network() {
        let mut worker = worker(MockIo {
            drop_credentials: true,
            ..MockIo::default()
        });
        assert!(worker.credentials().await.is_err());
        let messages = outgoing(&worker);
        for command in [WifiCommand::Ssid, WifiCommand::Password] {
            assert_eq!(
                messages
                    .iter()
                    .filter(|message| message.command == Command::Wifi(command))
                    .count(),
                MAX_CREDENTIAL_ATTEMPTS
            );
        }
        assert!(
            !messages
                .iter()
                .any(|message| message.command == Command::Wifi(WifiCommand::JoinNetwork))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn transient_join_refusal_retries_only_the_same_network_and_is_bounded() {
        let mut worker = worker(MockIo {
            join_replies: VecDeque::from([
                join_retry_reply(),
                join_retry_reply(),
                SUCCESS.to_vec(),
            ]),
            ..MockIo::default()
        });
        worker
            .join(&router(), NetworkCleanup::Preserve)
            .await
            .unwrap();
        let messages = outgoing(&worker);
        let joins: Vec<_> = messages
            .iter()
            .filter(|message| message.command == Command::Wifi(WifiCommand::JoinNetwork))
            .collect();
        assert_eq!(joins.len(), MAX_JOIN_ATTEMPTS);
        assert!(
            joins
                .iter()
                .all(|message| message.payload == joins[0].payload)
        );
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.command == Command::Wifi(WifiCommand::SetRole))
                .count(),
            1
        );
        let mut exhausted = super::tests::worker(MockIo {
            join_replies: VecDeque::from(vec![join_retry_reply(); MAX_JOIN_ATTEMPTS]),
            ..MockIo::default()
        });
        assert!(
            exhausted
                .join(&router(), NetworkCleanup::Preserve)
                .await
                .is_err()
        );
        assert_eq!(
            outgoing(&exhausted)
                .iter()
                .filter(|message| message.command == Command::Wifi(WifiCommand::JoinNetwork))
                .count(),
            MAX_JOIN_ATTEMPTS
        );
    }

    #[tokio::test(start_paused = true)]
    async fn role_and_join_timeouts_are_errors_not_success_shaped_fallbacks() {
        for io in [
            MockIo {
                drop_role: true,
                ..MockIo::default()
            },
            MockIo {
                drop_join: true,
                ..MockIo::default()
            },
        ] {
            let mut worker = worker(io);
            assert!(matches!(
                worker.join(&router(), NetworkCleanup::Preserve).await,
                Err(Error::ReplyTimeout { .. })
            ));
            assert!(
                outgoing(&worker)
                    .iter()
                    .filter(|message| message.command == Command::Wifi(WifiCommand::JoinNetwork))
                    .count()
                    <= 1
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_mtu_and_link_loss_stop_without_repair_or_extra_writes() {
        let mut cancelled = worker(MockIo::default());
        cancelled.cancellation.cancel();
        assert!(matches!(
            cancelled.pair(TEST_IDENTITY).await,
            Err(Error::Cancelled)
        ));
        assert!(cancelled.io.writes.is_empty());
        let mut small = worker(MockIo {
            mtu: ATT_HEADER + encode(&BlePairingArmRequest).unwrap().len() - 1,
            ..MockIo::default()
        });
        assert!(matches!(
            small.pair(TEST_IDENTITY).await,
            Err(Error::LimitExceeded(_))
        ));
        assert!(small.io.writes.is_empty());
        let mut lost = worker(MockIo {
            link_lost: true,
            ..MockIo::default()
        });
        assert!(lost.pair(TEST_IDENTITY).await.is_err());
        let writes = lost.io.writes.len();
        lost.cleanup(false).await.unwrap();
        assert_eq!(lost.io.writes.len(), writes);
        assert!(lost.io.disconnected);
    }

    #[tokio::test(start_paused = true)]
    async fn cleanup_reports_unconfirmed_restoration_and_disconnect_failure() {
        let mut worker = worker(MockIo {
            disconnect_fails: true,
            ..MockIo::default()
        });
        worker.restore_hotspot = true;
        assert!(matches!(
            worker.cleanup(false).await,
            Err(Error::Cleanup { .. })
        ));
        assert!(worker.io.disconnected);
        assert!(worker.io.writes.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn idle_worker_keeps_alive_and_processes_close() {
        let mut worker = worker(MockIo::default());
        let (send, receive) = async_mpsc::channel(COMMAND_CAPACITY);
        let (response, result) = mpsc::sync_channel(1);
        let client = async move {
            sleep(KEEPALIVE_INTERVAL * 3).await;
            send.send(Work {
                operation: Operation::Close,
                response,
            })
            .await
            .unwrap();
        };
        let (served, ()) = tokio::join!(worker.serve(receive), client);
        served.unwrap();
        assert!(matches!(result.recv().unwrap(), Ok(Reply::Done)));
        assert!(outgoing(&worker).len() >= 2);
    }

    #[tokio::test(start_paused = true)]
    async fn passive_receive_work_returns_a_frame_and_preserves_silence() {
        for frame in [
            None,
            Some(Message::request(
                Endpoint::Camera,
                FIRST_BLE_ID,
                Command::Common(CommonCommand::DeviceInfo),
                SUCCESS,
            )),
        ] {
            let mut worker = worker(MockIo::default());
            worker.pending.extend(frame.clone());
            let (send, receive) = async_mpsc::channel(COMMAND_CAPACITY);
            let (response, result) = mpsc::sync_channel(1);
            send.try_send(Work {
                operation: Operation::Receive(KEEPALIVE_INTERVAL * 3),
                response,
            })
            .unwrap();
            let (closed, closed_result) = mpsc::sync_channel(1);
            let client = async move {
                send.send(Work {
                    operation: Operation::Close,
                    response: closed,
                })
                .await
                .unwrap();
            };
            let (served, ()) = tokio::join!(worker.serve(receive), client);
            served.unwrap();
            assert!(matches!(closed_result.recv().unwrap(), Ok(Reply::Done)));
            let Ok(Reply::Received(received)) = result.recv().unwrap() else {
                panic!("expected passive receive result");
            };
            assert_eq!(received, frame);
            if frame.is_some() {
                assert!(outgoing(&worker).is_empty());
            } else {
                assert!(outgoing(&worker).len() >= 2);
            }
        }
        let mut worker = worker(MockIo::default());
        worker.cancellation.cancel();
        assert!(matches!(
            worker.message(Instant::now() + CANCEL_POLL).await,
            Err(Error::Cancelled)
        ));
    }

    #[test]
    fn ids_wrap_in_the_existing_range_and_network_input_is_validated() {
        let mut worker = worker(MockIo::default());
        assert_eq!(worker.id(), FIRST_BLE_ID);
        worker.next_id = MAX_ID;
        assert_eq!(worker.id(), MAX_ID);
        assert_eq!(worker.id(), MIN_ID);
        assert!(WifiNetwork::new(" ".into(), TEST_PASSWORD.into()).is_err());
        assert!(WifiNetwork::new("é".repeat(MAX_SSID_BYTES), TEST_PASSWORD.into()).is_err());
        assert!(WifiNetwork::new(TEST_SSID.into(), "short".into()).is_err());
        assert!(WifiNetwork::new(TEST_SSID.into(), "x".repeat(MAX_PASSWORD_BYTES + 1)).is_err());
        assert!(is_nano_name("OSMO NANO synthetic"));
        assert!(!is_nano_name("unrelated device"));
        assert!(select_device(vec![], None).is_err());
        let cancellation = BleCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            BleSession::connect_cancellable(BleConfig::new(None), &cancellation),
            Err(Error::Cancelled)
        ));
    }

    #[test]
    fn ble_connection_rejects_an_invalid_supplied_identity() {
        assert!(matches!(
            BleSession::connect(BleConfig::new(Some("".into()))),
            Err(Error::InvalidConfig {
                field: "pairing identity",
                ..
            })
        ));
    }

    #[test]
    fn generated_pairing_identities_are_valid_and_unique() {
        let first = new_identity();
        let second = new_identity();
        assert_eq!(first.len(), 32);
        validate_identity(&first).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn lease_excludes_another_owner_and_can_be_reacquired_after_release() {
        let directory =
            std::env::temp_dir().join(format!("osmo-lease-test-{}", uuid::Uuid::new_v4()));
        let path = directory.join("camera-owner.lock");
        let lease = CameraOwnerLease::acquire_at(&path).unwrap();
        assert!(matches!(
            CameraOwnerLease::acquire_at(&path),
            Err(Error::Ownership { .. })
        ));
        let shared = Arc::new(lease);
        let retained = shared.clone();
        drop(shared);
        assert!(CameraOwnerLease::acquire_at(&path).is_err());
        drop(retained);
        drop(CameraOwnerLease::acquire_at(&path).unwrap());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_interrupts_pairing_and_provisioning_delays() {
        for provisioning in [false, true] {
            let mut worker = worker(MockIo {
                pair_reply: None,
                ..MockIo::default()
            });
            let cancellation = worker.cancellation.clone();
            let operation = async {
                if provisioning {
                    worker.join(&router(), NetworkCleanup::Preserve).await
                } else {
                    worker.pair(TEST_IDENTITY).await
                }
            };
            let interrupt = async {
                sleep(KEEPALIVE_INTERVAL).await;
                cancellation.cancel();
            };
            let start = Instant::now();
            let (result, ()) = tokio::join!(operation, interrupt);
            assert!(matches!(result, Err(Error::Cancelled)));
            assert!(Instant::now() - start <= KEEPALIVE_INTERVAL + CANCEL_POLL);
            assert!(
                !outgoing(&worker)
                    .iter()
                    .any(|message| message.command == Command::Wifi(WifiCommand::JoinNetwork))
            );
        }
    }
}
