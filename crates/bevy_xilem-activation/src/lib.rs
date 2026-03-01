use std::{
    collections::HashSet,
    fs, io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use app_single_instance::{PrimaryHandle, notify_if_running, start_primary};
use ipc_channel::{
    IpcError, TryRecvError,
    ipc::{IpcOneShotServer, IpcSender, channel},
};
use serde::{Deserialize, Serialize};
use sysuri::{FnHandler, UriScheme};

const IPC_CONNECT_RETRY_ATTEMPTS: usize = 120;
const IPC_CONNECT_RETRY_DELAY: Duration = Duration::from_millis(50);
const IPC_ACK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActivationForwardMessage {
    request_id: String,
    uris: Vec<String>,
    ack_sender: IpcSender<ActivationForwardAck>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum ActivationForwardAck {
    Ack,
    Nack,
}

pub type Result<T> = std::result::Result<T, ActivationError>;

#[derive(Debug)]
pub enum ActivationError {
    InvalidConfig(String),
    Io(io::Error),
    Protocol(sysuri::Error),
    SingleInstance(String),
}

impl std::fmt::Display for ActivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(reason) => write!(f, "invalid activation config: {reason}"),
            Self::Io(error) => write!(f, "activation io error: {error}"),
            Self::Protocol(error) => write!(f, "protocol registration error: {error}"),
            Self::SingleInstance(error) => write!(f, "single-instance error: {error}"),
        }
    }
}

impl std::error::Error for ActivationError {}

impl From<io::Error> for ActivationError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<sysuri::Error> for ActivationError {
    fn from(value: sysuri::Error) -> Self {
        Self::Protocol(value)
    }
}

#[derive(Debug, Clone)]
pub struct ProtocolRegistration {
    pub scheme: String,
    pub description: String,
    pub executable: Option<PathBuf>,
    pub icon: Option<PathBuf>,
}

impl ProtocolRegistration {
    #[must_use]
    pub fn new(
        scheme: impl Into<String>,
        description: impl Into<String>,
        executable: Option<PathBuf>,
    ) -> Self {
        Self {
            scheme: scheme.into(),
            description: description.into(),
            executable,
            icon: None,
        }
    }

    #[must_use]
    pub fn with_icon(mut self, icon: PathBuf) -> Self {
        self.icon = Some(icon);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ActivationConfig {
    pub app_id: String,
    pub protocol: Option<ProtocolRegistration>,
}

impl ActivationConfig {
    #[must_use]
    pub fn new(app_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            protocol: None,
        }
    }

    #[must_use]
    pub fn with_protocol(mut self, protocol: ProtocolRegistration) -> Self {
        self.protocol = Some(protocol);
        self
    }
}

pub enum BootstrapOutcome {
    Primary(ActivationService),
    SecondaryForwarded,
}

pub struct ActivationService {
    startup_uris: Vec<String>,
    receiver: Receiver<String>,
    _primary_handle: PrimaryHandle,
}

impl ActivationService {
    #[must_use]
    pub fn take_startup_uris(&mut self) -> Vec<String> {
        std::mem::take(&mut self.startup_uris)
    }

    #[must_use]
    pub fn drain_uris(&mut self) -> Vec<String> {
        let mut uris = Vec::new();
        while let Ok(uri) = self.receiver.try_recv() {
            uris.push(uri);
        }
        uris
    }
}

pub fn bootstrap(config: ActivationConfig) -> Result<BootstrapOutcome> {
    validate_config(&config)?;

    if let Some(protocol) = config.protocol.as_ref() {
        ensure_protocol_registered(protocol)?;
    }

    let startup_uris = collect_startup_uris(config.protocol.as_ref())?;
    let should_exit_as_secondary = notify_if_running(&config.app_id);

    if should_exit_as_secondary {
        return Ok(finalize_secondary_forward_result(forward_uris_to_primary(
            &config.app_id,
            &startup_uris,
        )));
    }

    let _ = cleanup_stale_ipc_endpoint(&config.app_id);
    let primary_handle = start_primary(&config.app_id, || {});

    let thread_name = listener_thread_name(&config.app_id);
    let (sender, receiver) = mpsc::channel::<String>();
    spawn_ipc_listener(&config.app_id, thread_name, sender)?;

    Ok(BootstrapOutcome::Primary(ActivationService {
        startup_uris,
        receiver,
        _primary_handle: primary_handle,
    }))
}

fn finalize_secondary_forward_result(forward_result: Result<()>) -> BootstrapOutcome {
    // Secondary launches should never become an interactive UI process, even if
    // forwarding to the primary fails transiently. This avoids dual-instance
    // regressions on macOS callback relaunch races.
    let _ = forward_result;
    BootstrapOutcome::SecondaryForwarded
}

fn collect_startup_uris(protocol: Option<&ProtocolRegistration>) -> Result<Vec<String>> {
    let Some(protocol) = protocol else {
        return Ok(sysuri::parse_args().into_iter().collect());
    };

    let pending_uris = Arc::new(Mutex::new(Vec::<String>::new()));
    let pending_uris_for_handler = Arc::clone(&pending_uris);
    let expected_scheme = protocol.scheme.clone();
    let expected_scheme_for_handler = expected_scheme.clone();

    sysuri::register_handler(
        protocol.scheme.as_str(),
        FnHandler::new(move |uri| {
            let Some(scheme) = sysuri::extract_scheme(uri) else {
                return;
            };

            if !scheme.eq_ignore_ascii_case(expected_scheme_for_handler.as_str()) {
                return;
            }

            if let Ok(mut uris) = pending_uris_for_handler.lock() {
                uris.push(uri.to_string());
            }
        }),
    );

    let mut startup_uris =
        collect_matching_protocol_uris_from_process_args(expected_scheme.as_str());
    let mut should_dispatch_sysuri = cfg!(target_os = "macos") || !startup_uris.is_empty();

    if let Some(uri) = sysuri::parse_args()
        && let Some(scheme) = sysuri::extract_scheme(&uri)
        && scheme.eq_ignore_ascii_case(protocol.scheme.as_str())
    {
        startup_uris.push(uri);
        should_dispatch_sysuri = true;
    }

    if should_dispatch_sysuri && let Err(error) = sysuri::should_handle_uri() {
        eprintln!("activation: sysuri callback dispatch failed: {error}");
    }

    startup_uris.extend(
        pending_uris
            .lock()
            .map_err(|_| {
                ActivationError::SingleInstance("sysuri callback buffer mutex poisoned".to_string())
            })?
            .iter()
            .cloned(),
    );

    Ok(dedupe_preserve_order(startup_uris))
}

fn collect_matching_protocol_uris_from_process_args(expected_scheme: &str) -> Vec<String> {
    collect_matching_protocol_uris_from_iter(std::env::args().skip(1), expected_scheme)
}

fn collect_matching_protocol_uris_from_iter<I>(args: I, expected_scheme: &str) -> Vec<String>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    args.into_iter()
        .filter_map(|arg| {
            let raw = arg.as_ref().trim();
            let candidate = raw.trim_matches('"').trim_matches('\'');

            let scheme = sysuri::extract_scheme(candidate)?;
            if scheme.eq_ignore_ascii_case(expected_scheme) {
                Some(candidate.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn dedupe_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(values.len());

    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }

    deduped
}

pub fn ensure_protocol_registered(protocol: &ProtocolRegistration) -> Result<()> {
    let executable = match protocol.executable.clone() {
        Some(path) => path,
        None => std::env::current_exe()?,
    };

    let mut scheme = UriScheme::new(
        protocol.scheme.clone(),
        protocol.description.clone(),
        executable,
    );

    if let Some(icon) = protocol.icon.clone() {
        scheme = scheme.with_icon(icon);
    }

    if !scheme.is_valid_scheme() {
        return Err(ActivationError::InvalidConfig(format!(
            "scheme `{}` is invalid",
            scheme.scheme
        )));
    }

    sysuri::register(&scheme)?;
    Ok(())
}

fn spawn_ipc_listener(app_id: &str, thread_name: String, sender: Sender<String>) -> Result<()> {
    let app_id = app_id.to_string();

    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            loop {
                if let Err(error) = run_ipc_listener_cycle(&app_id, &sender) {
                    eprintln!("activation: listener cycle failed: {error}");
                    thread::sleep(IPC_CONNECT_RETRY_DELAY);
                }
            }
        })
        .map_err(ActivationError::Io)?;

    Ok(())
}

fn run_ipc_listener_cycle(app_id: &str, sender: &Sender<String>) -> io::Result<()> {
    let (server, server_name) = IpcOneShotServer::<ActivationForwardMessage>::new()?;
    publish_ipc_server_name(app_id, &server_name)?;

    let (_, message) = server.accept().map_err(ipc_error_to_io)?;

    let mut all_forwarded = true;
    for uri in message.uris {
        if sender.send(uri).is_err() {
            all_forwarded = false;
            break;
        }
    }

    let receipt = if all_forwarded {
        ActivationForwardAck::Ack
    } else {
        ActivationForwardAck::Nack
    };
    let _ = message.ack_sender.send(receipt);

    Ok(())
}

fn cleanup_stale_ipc_endpoint(app_id: &str) -> io::Result<()> {
    let path = ipc_rendezvous_path_for_app(app_id);

    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn forward_uris_to_primary(app_id: &str, uris: &[String]) -> Result<()> {
    if uris.is_empty() {
        return Ok(());
    }

    let request_id = next_forward_request_id();
    let mut last_error: Option<io::Error> = None;

    for _ in 0..IPC_CONNECT_RETRY_ATTEMPTS {
        let server_name = match load_ipc_server_name(app_id) {
            Ok(Some(name)) => name,
            Ok(None) => {
                thread::sleep(IPC_CONNECT_RETRY_DELAY);
                continue;
            }
            Err(error) => {
                last_error = Some(error);
                thread::sleep(IPC_CONNECT_RETRY_DELAY);
                continue;
            }
        };

        let (ack_sender, ack_receiver) = match channel::<ActivationForwardAck>() {
            Ok(pair) => pair,
            Err(error) => {
                last_error = Some(error);
                thread::sleep(IPC_CONNECT_RETRY_DELAY);
                continue;
            }
        };

        let sender = match IpcSender::<ActivationForwardMessage>::connect(server_name) {
            Ok(sender) => sender,
            Err(error) => {
                last_error = Some(error);
                thread::sleep(IPC_CONNECT_RETRY_DELAY);
                continue;
            }
        };

        let payload = ActivationForwardMessage {
            request_id: request_id.clone(),
            uris: uris.to_vec(),
            ack_sender,
        };

        if let Err(error) = sender.send(payload) {
            last_error = Some(ipc_error_to_io(error));
            thread::sleep(IPC_CONNECT_RETRY_DELAY);
            continue;
        }

        match ack_receiver.try_recv_timeout(IPC_ACK_TIMEOUT) {
            Ok(ActivationForwardAck::Ack) => return Ok(()),
            Ok(ActivationForwardAck::Nack) => {
                last_error = Some(io::Error::other(
                    "primary rejected activation payload while enqueuing",
                ));
                thread::sleep(IPC_CONNECT_RETRY_DELAY);
            }
            Err(error) => {
                last_error = Some(try_recv_error_to_io(error));
                thread::sleep(IPC_CONNECT_RETRY_DELAY);
            }
        }
    }

    Err(ActivationError::Io(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "failed to deliver activation payload to primary listener",
        )
    })))
}

fn publish_ipc_server_name(app_id: &str, server_name: &str) -> io::Result<()> {
    let path = ipc_rendezvous_path_for_app(app_id);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, server_name)?;

    match fs::rename(&temp_path, &path) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::write(&path, server_name)?;
            let _ = fs::remove_file(&temp_path);
            Ok(())
        }
    }
}

fn load_ipc_server_name(app_id: &str) -> io::Result<Option<String>> {
    let path = ipc_rendezvous_path_for_app(app_id);
    match fs::read_to_string(path) {
        Ok(raw) => {
            let name = raw.trim().to_string();
            if name.is_empty() {
                Ok(None)
            } else {
                Ok(Some(name))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn ipc_rendezvous_path_for_app(app_id: &str) -> PathBuf {
    let normalized = normalize_app_id(app_id);
    std::env::temp_dir().join(format!("{normalized}.activation.ipc-name"))
}

fn try_recv_error_to_io(error: TryRecvError) -> io::Error {
    match error {
        TryRecvError::IpcError(ipc_error) => ipc_error_to_io(ipc_error),
        TryRecvError::Empty => io::Error::new(
            io::ErrorKind::TimedOut,
            "timed out waiting for primary activation acknowledgement",
        ),
    }
}

fn ipc_error_to_io(error: IpcError) -> io::Error {
    match error {
        IpcError::Io(io_error) => io_error,
        IpcError::SerializationError(error) => {
            io::Error::new(io::ErrorKind::InvalidData, error.to_string())
        }
        IpcError::Disconnected => io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "ipc channel disconnected unexpectedly",
        ),
    }
}

fn next_forward_request_id() -> String {
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{timestamp_nanos}", std::process::id())
}

fn validate_config(config: &ActivationConfig) -> Result<()> {
    if config.app_id.trim().is_empty() {
        return Err(ActivationError::InvalidConfig(
            "app_id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn normalize_app_id(app_id: &str) -> String {
    app_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
}

fn listener_thread_name(app_id: &str) -> String {
    format!("{}-activation-listener", normalize_app_id(app_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_preserve_order_keeps_first_occurrence() {
        let values = vec![
            "pixiv://account/login?code=abc".to_string(),
            "https://example.com".to_string(),
            "pixiv://account/login?code=abc".to_string(),
            "https://example.com".to_string(),
            "pixiv://account/login?code=def".to_string(),
        ];

        let deduped = dedupe_preserve_order(values);
        assert_eq!(
            deduped,
            vec![
                "pixiv://account/login?code=abc".to_string(),
                "https://example.com".to_string(),
                "pixiv://account/login?code=def".to_string(),
            ]
        );
    }

    #[test]
    fn app_id_normalization_is_stable() {
        assert_eq!(
            normalize_app_id("Pixiv Client@Desktop"),
            "pixiv-client-desktop"
        );
    }

    #[test]
    fn empty_app_id_is_rejected() {
        let result = validate_config(&ActivationConfig::new("  "));
        assert!(result.is_err());
    }

    #[test]
    fn protocol_builder_keeps_scheme() {
        let registration = ProtocolRegistration::new("pixiv", "Pixiv", None);
        assert_eq!(registration.scheme, "pixiv");
        assert_eq!(registration.description, "Pixiv");
    }

    #[test]
    fn stale_ipc_endpoint_cleanup_removes_rendezvous_file() {
        let app_id = "bevy-xilem-activation-test-cleanup";
        let path = ipc_rendezvous_path_for_app(app_id);

        let _ = fs::remove_file(&path);
        fs::write(&path, b"stale").expect("should create stale endpoint marker");
        cleanup_stale_ipc_endpoint(app_id).expect("cleanup should succeed");

        assert!(!path.exists());
    }

    #[test]
    fn secondary_forward_success_still_exits_secondary() {
        let outcome = finalize_secondary_forward_result(Ok(()));
        assert!(matches!(outcome, BootstrapOutcome::SecondaryForwarded));
    }

    #[test]
    fn secondary_forward_failure_still_exits_secondary() {
        let outcome = finalize_secondary_forward_result(Err(ActivationError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "boom",
        ))));
        assert!(matches!(outcome, BootstrapOutcome::SecondaryForwarded));
    }

    #[test]
    fn protocol_uri_fallback_from_raw_args_filters_by_scheme() {
        let args = vec![
            "-psn_0_12345".to_string(),
            "pixiv://account/login?code=first".to_string(),
            "\"PIXIV://account/login?code=second\"".to_string(),
            "https://example.com/callback?code=ignored".to_string(),
        ];

        let uris = collect_matching_protocol_uris_from_iter(args, "pixiv");
        assert_eq!(
            uris,
            vec![
                "pixiv://account/login?code=first".to_string(),
                "PIXIV://account/login?code=second".to_string(),
            ]
        );
    }

    #[test]
    fn forward_uris_to_primary_delivers_payload_with_receipt() {
        let short_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .subsec_nanos();
        let unique = format!("ack-{}-{short_nanos}", std::process::id(),);

        let _ = cleanup_stale_ipc_endpoint(&unique);
        let (sender, receiver) = mpsc::channel::<String>();

        spawn_ipc_listener(&unique, listener_thread_name(&unique), sender)
            .expect("should spawn listener");

        let uris = vec!["pixiv://account/login?code=ack-check".to_string()];
        forward_uris_to_primary(&unique, &uris).expect("forward should succeed with receipt");

        let forwarded = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("listener should forward URI to primary receiver");
        assert_eq!(forwarded, uris[0]);
    }
}
