use std::{
    collections::HashSet,
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Name, Stream, prelude::*,
};
use single_instance::SingleInstance;
use sysuri::UriScheme;

const IPC_CONNECT_RETRY_ATTEMPTS: usize = 120;
const IPC_CONNECT_RETRY_DELAY: Duration = Duration::from_millis(50);
const IPC_PROTOCOL_PREFIX: &str = "bevy-xilem-activation-v1";
const IPC_PROTOCOL_URI: &str = "URI";
const IPC_PROTOCOL_END: &str = "END";
const IPC_PROTOCOL_ACK: &str = "ACK";
const IPC_PROTOCOL_NACK: &str = "NACK";

enum IpcIncomingLine {
    LegacyUri(String),
    ProtocolUri { request_id: String, uri: String },
    ProtocolEnd { request_id: String },
    ProtocolControl,
}

enum IpcAckLine {
    Ack,
    Nack,
}

#[cfg(target_os = "macos")]
mod macos_apple_events {
    use std::{
        cell::RefCell,
        sync::{Mutex, OnceLock, mpsc::Sender},
    };

    use objc2::rc::Retained;
    use objc2::{MainThreadOnly, define_class, msg_send};
    use objc2_core_services::{kAEISGetURL, kAEInternetSuite, keyDirectObject};
    use objc2_foundation::{
        MainThreadMarker, NSAppleEventDescriptor, NSAppleEventManager, NSObject, NSObjectProtocol,
    };

    use crate::{ActivationError, Result};

    static URI_SENDER: OnceLock<Mutex<Option<Sender<String>>>> = OnceLock::new();

    thread_local! {
        static EVENT_HANDLER: RefCell<Option<Retained<ActivationAppleEventHandler>>> = const { RefCell::new(None) };
    }

    fn extract_uri_from_event(event: &NSAppleEventDescriptor) -> Option<String> {
        if event.eventClass() != kAEInternetSuite || event.eventID() != kAEISGetURL {
            return None;
        }

        let descriptor = event.paramDescriptorForKeyword(keyDirectObject)?;
        let value = descriptor.stringValue()?.to_string();
        let trimmed = value.trim();

        if trimmed.is_empty() || !trimmed.contains("://") {
            return None;
        }

        Some(trimmed.to_string())
    }

    define_class!(
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[name = "BevyXilemActivationAppleEventHandler"]
        struct ActivationAppleEventHandler;

        unsafe impl NSObjectProtocol for ActivationAppleEventHandler {}

        impl ActivationAppleEventHandler {
            #[unsafe(method(handleAppleEvent:withReplyEvent:))]
            fn handle_apple_event(
                &self,
                event: &NSAppleEventDescriptor,
                _reply: &NSAppleEventDescriptor,
            ) {
                let Some(uri) = extract_uri_from_event(event) else {
                    return;
                };

                let Some(sender_slot) = URI_SENDER.get() else {
                    return;
                };

                let Ok(sender_guard) = sender_slot.lock() else {
                    return;
                };

                if let Some(sender) = sender_guard.as_ref() {
                    let _ = sender.send(uri);
                }
            }
        }
    );

    impl ActivationAppleEventHandler {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            unsafe { msg_send![super(Self::alloc(mtm).set_ivars(())), init] }
        }
    }

    pub fn install_uri_forwarder(sender: Sender<String>) -> Result<()> {
        let sender_slot = URI_SENDER.get_or_init(|| Mutex::new(None));
        {
            let mut sender_guard = sender_slot.lock().map_err(|_| {
                ActivationError::SingleInstance("apple-event sender mutex poisoned".to_string())
            })?;
            *sender_guard = Some(sender);
        }

        let mtm = MainThreadMarker::new().ok_or_else(|| {
            ActivationError::SingleInstance(
                "failed to get MainThreadMarker while installing apple-event handler".to_string(),
            )
        })?;

        EVENT_HANDLER.with(|handler_slot| {
            let mut handler_slot = handler_slot.borrow_mut();
            if handler_slot.is_none() {
                *handler_slot = Some(ActivationAppleEventHandler::new(mtm));
            }

            if let Some(handler) = handler_slot.as_ref() {
                let manager = NSAppleEventManager::sharedAppleEventManager();
                unsafe {
                    manager.setEventHandler_andSelector_forEventClass_andEventID(
                        &**handler,
                        objc2::sel!(handleAppleEvent:withReplyEvent:),
                        kAEInternetSuite,
                        kAEISGetURL,
                    );
                }
            }
        });

        Ok(())
    }

    pub fn current_uri() -> Option<String> {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        let event = manager.currentAppleEvent()?;
        extract_uri_from_event(&event)
    }
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
    _single_instance: SingleInstance,
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

    let mut startup_uris = collect_startup_uris();
    let single_instance = SingleInstance::new(single_instance_name(&config.app_id).as_str())
        .map_err(|error| ActivationError::SingleInstance(error.to_string()))?;

    let name = ipc_name_for_app(&config.app_id)?;

    if single_instance.is_single() {
        let thread_name = listener_thread_name(&config.app_id);
        let (sender, receiver) = mpsc::channel::<String>();

        match spawn_ipc_listener(&name, thread_name.clone(), sender.clone()) {
            Ok(()) => {
                #[cfg(target_os = "macos")]
                macos_apple_events::install_uri_forwarder(sender)?;

                Ok(BootstrapOutcome::Primary(ActivationService {
                    startup_uris,
                    receiver,
                    _single_instance: single_instance,
                }))
            }
            Err(error) if should_treat_listener_bind_as_existing_primary(&error) => {
                if primary_listener_is_reachable(&name) {
                    #[cfg(target_os = "macos")]
                    maybe_fill_startup_uris_from_secondary_apple_event(&mut startup_uris);

                    Ok(finalize_secondary_forward_result(forward_uris_to_primary(
                        &name,
                        &startup_uris,
                    )))
                } else {
                    cleanup_stale_ipc_endpoint(&config.app_id)?;
                    spawn_ipc_listener(&name, thread_name, sender.clone())?;

                    #[cfg(target_os = "macos")]
                    macos_apple_events::install_uri_forwarder(sender)?;

                    Ok(BootstrapOutcome::Primary(ActivationService {
                        startup_uris,
                        receiver,
                        _single_instance: single_instance,
                    }))
                }
            }
            Err(error) => Err(error),
        }
    } else {
        #[cfg(target_os = "macos")]
        maybe_fill_startup_uris_from_secondary_apple_event(&mut startup_uris);

        Ok(finalize_secondary_forward_result(forward_uris_to_primary(
            &name,
            &startup_uris,
        )))
    }
}

fn finalize_secondary_forward_result(forward_result: Result<()>) -> BootstrapOutcome {
    // Secondary launches should never become an interactive UI process, even if
    // forwarding to the primary fails transiently. This avoids dual-instance
    // regressions on macOS callback relaunch races.
    let _ = forward_result;
    BootstrapOutcome::SecondaryForwarded
}

fn collect_startup_uris() -> Vec<String> {
    #[cfg(target_os = "macos")]
    let use_argv = false;

    #[cfg(not(target_os = "macos"))]
    let use_argv = true;

    let apple_event_uri = {
        #[cfg(target_os = "macos")]
        {
            collect_macos_activation_uri_from_current_apple_event()
        }

        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    };

    collect_startup_uris_from_sources(std::env::args().skip(1), apple_event_uri, use_argv)
}

fn collect_startup_uris_from_sources<I, S>(
    args: I,
    apple_event_uri: Option<String>,
    use_argv: bool,
) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut uris = if use_argv {
        collect_activation_uris(args)
    } else {
        Vec::new()
    };

    if let Some(uri) = apple_event_uri {
        uris.push(uri);
    }

    dedupe_preserve_order(uris)
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

#[cfg(target_os = "macos")]
fn maybe_fill_startup_uris_from_secondary_apple_event(startup_uris: &mut Vec<String>) {
    if !startup_uris.is_empty() {
        return;
    }

    let deadline = Instant::now() + Duration::from_millis(1200);
    loop {
        if let Some(uri) = collect_macos_activation_uri_from_current_apple_event() {
            startup_uris.push(uri);
            *startup_uris = dedupe_preserve_order(std::mem::take(startup_uris));
            return;
        }

        if Instant::now() >= deadline {
            return;
        }

        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(target_os = "macos")]
fn collect_macos_activation_uri_from_current_apple_event() -> Option<String> {
    macos_apple_events::current_uri()
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

fn spawn_ipc_listener(name: &Name<'_>, thread_name: String, sender: Sender<String>) -> Result<()> {
    let listener = ListenerOptions::new().name(name.borrow()).create_sync()?;

    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    continue;
                };
                handle_ipc_stream(stream, &sender);
            }
        })
        .map_err(ActivationError::Io)?;

    Ok(())
}

fn handle_ipc_stream(stream: Stream, sender: &Sender<String>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut pending_uris = Vec::new();
    let mut should_send_receipt = false;
    let mut request_id_for_receipt: Option<String> = None;

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let payload = line.trim();
                if payload.is_empty() {
                    continue;
                }

                match parse_ipc_incoming_line(payload) {
                    IpcIncomingLine::LegacyUri(uri) => pending_uris.push(uri),
                    IpcIncomingLine::ProtocolUri { request_id, uri } => {
                        if request_id_for_receipt.is_none() {
                            request_id_for_receipt = Some(request_id.clone());
                        }
                        pending_uris.push(uri);
                    }
                    IpcIncomingLine::ProtocolEnd { request_id } => {
                        if request_id_for_receipt.is_none() {
                            request_id_for_receipt = Some(request_id);
                        }
                        should_send_receipt = true;
                        break;
                    }
                    IpcIncomingLine::ProtocolControl => {}
                }
            }
            Err(_) => break,
        }
    }

    let mut all_forwarded = true;
    for uri in pending_uris {
        if sender.send(uri).is_err() {
            all_forwarded = false;
            break;
        }
    }

    if should_send_receipt && let Some(request_id) = request_id_for_receipt {
        let receipt_line = if all_forwarded {
            encode_ipc_ack_line(&request_id)
        } else {
            encode_ipc_nack_line(&request_id)
        };

        let stream = reader.get_mut();
        let _ = stream.write_all(receipt_line.as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }
}

fn should_treat_listener_bind_as_existing_primary(error: &ActivationError) -> bool {
    match error {
        ActivationError::Io(io_error) => matches!(
            io_error.kind(),
            io::ErrorKind::AddrInUse | io::ErrorKind::AlreadyExists
        ),
        _ => false,
    }
}

fn primary_listener_is_reachable(name: &Name<'_>) -> bool {
    for _ in 0..IPC_CONNECT_RETRY_ATTEMPTS {
        if Stream::connect(name.borrow()).is_ok() {
            return true;
        }

        thread::sleep(IPC_CONNECT_RETRY_DELAY);
    }

    false
}

fn cleanup_stale_ipc_endpoint(app_id: &str) -> io::Result<()> {
    let Some(path) = ipc_socket_path_for_app(app_id) else {
        return Ok(());
    };

    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn forward_uris_to_primary(name: &Name<'_>, uris: &[String]) -> Result<()> {
    if uris.is_empty() {
        return Ok(());
    }

    let request_id = next_forward_request_id();
    let mut last_error: Option<io::Error> = None;

    for _ in 0..IPC_CONNECT_RETRY_ATTEMPTS {
        match Stream::connect(name.borrow()) {
            Ok(stream) => {
                let mut reader = BufReader::new(stream);
                let mut write_failed = false;

                for uri in uris {
                    let payload = encode_ipc_uri_line(&request_id, uri);
                    if let Err(error) = write_ipc_line(reader.get_mut(), payload.as_str()) {
                        last_error = Some(error);
                        write_failed = true;
                        break;
                    }
                }

                if write_failed {
                    thread::sleep(IPC_CONNECT_RETRY_DELAY);
                    continue;
                }

                let end_payload = encode_ipc_end_line(&request_id);
                if let Err(error) = write_ipc_line(reader.get_mut(), end_payload.as_str()) {
                    last_error = Some(error);
                    thread::sleep(IPC_CONNECT_RETRY_DELAY);
                    continue;
                }

                if let Err(error) = reader.get_mut().flush() {
                    last_error = Some(error);
                    thread::sleep(IPC_CONNECT_RETRY_DELAY);
                    continue;
                }

                let mut ack_line = String::new();
                match reader.read_line(&mut ack_line) {
                    Ok(0) => {
                        last_error = Some(io::Error::new(
                            io::ErrorKind::ConnectionAborted,
                            "primary closed activation stream before acknowledgement",
                        ));
                        thread::sleep(IPC_CONNECT_RETRY_DELAY);
                    }
                    Ok(_) => {
                        let ack_payload = ack_line.trim();
                        match parse_ipc_ack_line(ack_payload, &request_id) {
                            Some(IpcAckLine::Ack) => return Ok(()),
                            Some(IpcAckLine::Nack) => {
                                last_error =
                                    Some(io::Error::other("primary rejected activation payload"));
                                thread::sleep(IPC_CONNECT_RETRY_DELAY);
                            }
                            None => {
                                last_error = Some(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "invalid activation acknowledgement from primary: {ack_payload}"
                                    ),
                                ));
                                thread::sleep(IPC_CONNECT_RETRY_DELAY);
                            }
                        }
                    }
                    Err(error) => {
                        last_error = Some(error);
                        thread::sleep(IPC_CONNECT_RETRY_DELAY);
                    }
                }
            }
            Err(error) => {
                last_error = Some(error);
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

fn parse_ipc_incoming_line(payload: &str) -> IpcIncomingLine {
    let mut parts = payload.splitn(4, '\t');
    let Some(prefix) = parts.next() else {
        return IpcIncomingLine::ProtocolControl;
    };

    if prefix != IPC_PROTOCOL_PREFIX {
        return IpcIncomingLine::LegacyUri(payload.to_string());
    }

    let Some(request_id) = parts.next() else {
        return IpcIncomingLine::ProtocolControl;
    };
    let Some(kind) = parts.next() else {
        return IpcIncomingLine::ProtocolControl;
    };

    match kind {
        IPC_PROTOCOL_URI => {
            let Some(uri) = parts.next() else {
                return IpcIncomingLine::ProtocolControl;
            };

            let uri = uri.trim();
            if uri.is_empty() {
                IpcIncomingLine::ProtocolControl
            } else {
                IpcIncomingLine::ProtocolUri {
                    request_id: request_id.to_string(),
                    uri: uri.to_string(),
                }
            }
        }
        IPC_PROTOCOL_END => IpcIncomingLine::ProtocolEnd {
            request_id: request_id.to_string(),
        },
        _ => IpcIncomingLine::ProtocolControl,
    }
}

fn parse_ipc_ack_line(payload: &str, request_id: &str) -> Option<IpcAckLine> {
    let mut parts = payload.splitn(4, '\t');
    let prefix = parts.next()?;
    if prefix != IPC_PROTOCOL_PREFIX {
        return None;
    }

    let response_request_id = parts.next()?;
    if response_request_id != request_id {
        return None;
    }

    match parts.next()? {
        IPC_PROTOCOL_ACK => Some(IpcAckLine::Ack),
        IPC_PROTOCOL_NACK => Some(IpcAckLine::Nack),
        _ => None,
    }
}

fn encode_ipc_uri_line(request_id: &str, uri: &str) -> String {
    format!("{IPC_PROTOCOL_PREFIX}\t{request_id}\t{IPC_PROTOCOL_URI}\t{uri}")
}

fn encode_ipc_end_line(request_id: &str) -> String {
    format!("{IPC_PROTOCOL_PREFIX}\t{request_id}\t{IPC_PROTOCOL_END}")
}

fn encode_ipc_ack_line(request_id: &str) -> String {
    format!("{IPC_PROTOCOL_PREFIX}\t{request_id}\t{IPC_PROTOCOL_ACK}")
}

fn encode_ipc_nack_line(request_id: &str) -> String {
    format!("{IPC_PROTOCOL_PREFIX}\t{request_id}\t{IPC_PROTOCOL_NACK}")
}

fn write_ipc_line(stream: &mut Stream, payload: &str) -> io::Result<()> {
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    Ok(())
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

fn collect_activation_uris<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .map(|value| value.as_ref().trim().to_string())
        .filter(|value| value.contains("://"))
        .collect()
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

fn single_instance_name(app_id: &str) -> String {
    let normalized = normalize_app_id(app_id);

    #[cfg(target_os = "macos")]
    {
        return std::env::temp_dir()
            .join(format!("{normalized}.lock"))
            .to_string_lossy()
            .into_owned();
    }

    #[allow(unreachable_code)]
    normalized
}

fn listener_thread_name(app_id: &str) -> String {
    format!("{}-activation-listener", normalize_app_id(app_id))
}

fn ipc_name_for_app(app_id: &str) -> io::Result<Name<'static>> {
    let normalized = normalize_app_id(app_id);
    let token = format!("{normalized}.activation");

    if use_namespaced_ipc_socket() {
        token
            .to_ns_name::<GenericNamespaced>()
            .map(|name| name.into_owned())
    } else {
        let socket_path = ipc_socket_path_for_app(app_id)
            .expect("filesystem local sockets must have an ipc path");
        socket_path
            .to_string_lossy()
            .to_string()
            .to_fs_name::<GenericFilePath>()
            .map(|name| name.into_owned())
    }
}

fn ipc_socket_path_for_app(app_id: &str) -> Option<PathBuf> {
    if use_namespaced_ipc_socket() {
        return None;
    }

    let normalized = normalize_app_id(app_id);
    let token = format!("{normalized}.activation");
    Some(std::env::temp_dir().join(format!("{token}.sock")))
}

fn use_namespaced_ipc_socket() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Keep macOS on filesystem sockets so stale endpoints can be cleaned up
        // deterministically and listener reachability checks are stable.
        return false;
    }

    #[allow(unreachable_code)]
    GenericNamespaced::is_supported()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_only_uri_like_arguments() {
        let args = vec![
            "--flag".to_string(),
            "pixiv://account/login?code=abc&via=login".to_string(),
            "https://example.com".to_string(),
            "plain-text".to_string(),
        ];

        let uris = collect_activation_uris(args);
        assert_eq!(uris.len(), 2);
        assert!(uris[0].starts_with("pixiv://"));
        assert!(uris[1].starts_with("https://"));
    }

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
    fn startup_source_policy_can_disable_argv_and_keep_apple_event() {
        let args = vec![
            "--flag".to_string(),
            "pixiv://account/login?code=from-argv".to_string(),
        ];

        let uris = collect_startup_uris_from_sources(
            args,
            Some("pixiv://account/login?code=from-apple-event".to_string()),
            false,
        );

        assert_eq!(
            uris,
            vec!["pixiv://account/login?code=from-apple-event".to_string()]
        );
    }

    #[test]
    fn startup_source_policy_merges_and_dedupes_when_argv_enabled() {
        let args = vec![
            "pixiv://account/login?code=abc".to_string(),
            "https://example.com".to_string(),
            "pixiv://account/login?code=abc".to_string(),
        ];

        let uris = collect_startup_uris_from_sources(
            args,
            Some("pixiv://account/login?code=abc".to_string()),
            true,
        );

        assert_eq!(
            uris,
            vec![
                "pixiv://account/login?code=abc".to_string(),
                "https://example.com".to_string(),
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
    fn listener_bind_conflicts_are_treated_as_existing_primary() {
        let addr_in_use = ActivationError::Io(io::Error::new(io::ErrorKind::AddrInUse, "boom"));
        let already_exists =
            ActivationError::Io(io::Error::new(io::ErrorKind::AlreadyExists, "boom"));

        assert!(should_treat_listener_bind_as_existing_primary(&addr_in_use));
        assert!(should_treat_listener_bind_as_existing_primary(
            &already_exists
        ));
    }

    #[test]
    fn non_conflict_listener_errors_are_not_treated_as_existing_primary() {
        let permission_denied =
            ActivationError::Io(io::Error::new(io::ErrorKind::PermissionDenied, "boom"));
        assert!(!should_treat_listener_bind_as_existing_primary(
            &permission_denied
        ));
    }

    #[test]
    fn stale_ipc_endpoint_cleanup_removes_filesystem_socket_path() {
        let app_id = "bevy-xilem-activation-test-cleanup";
        let Some(path) = ipc_socket_path_for_app(app_id) else {
            return;
        };

        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, b"stale").expect("should create stale endpoint marker");
        cleanup_stale_ipc_endpoint(app_id).expect("cleanup should succeed");

        assert!(!path.exists());
    }

    #[test]
    fn socket_transport_selection_is_platform_consistent() {
        #[cfg(target_os = "macos")]
        assert!(!use_namespaced_ipc_socket());

        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            use_namespaced_ipc_socket(),
            GenericNamespaced::is_supported()
        );
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
    fn protocol_parser_keeps_legacy_uri_payloads() {
        let parsed = parse_ipc_incoming_line("pixiv://account/login?code=abc");
        match parsed {
            IpcIncomingLine::LegacyUri(uri) => {
                assert_eq!(uri, "pixiv://account/login?code=abc");
            }
            _ => panic!("expected legacy URI payload"),
        }
    }

    #[test]
    fn protocol_parser_reads_uri_and_end_markers() {
        let request_id = "req-123";
        let uri_line = encode_ipc_uri_line(request_id, "pixiv://account/login?code=abc");
        let end_line = encode_ipc_end_line(request_id);

        match parse_ipc_incoming_line(uri_line.as_str()) {
            IpcIncomingLine::ProtocolUri {
                request_id: parsed_request,
                uri,
            } => {
                assert_eq!(parsed_request, request_id);
                assert_eq!(uri, "pixiv://account/login?code=abc");
            }
            _ => panic!("expected protocol URI line"),
        }

        match parse_ipc_incoming_line(end_line.as_str()) {
            IpcIncomingLine::ProtocolEnd {
                request_id: parsed_request,
            } => {
                assert_eq!(parsed_request, request_id);
            }
            _ => panic!("expected protocol END line"),
        }
    }

    #[test]
    fn protocol_ack_parser_validates_request_id() {
        let request_id = "req-ack";
        let ack_line = encode_ipc_ack_line(request_id);
        let nack_line = encode_ipc_nack_line(request_id);

        assert!(matches!(
            parse_ipc_ack_line(ack_line.as_str(), request_id),
            Some(IpcAckLine::Ack)
        ));
        assert!(matches!(
            parse_ipc_ack_line(nack_line.as_str(), request_id),
            Some(IpcAckLine::Nack)
        ));
        assert!(parse_ipc_ack_line(ack_line.as_str(), "other-request").is_none());
    }

    #[test]
    fn forward_uris_to_primary_delivers_payload_with_receipt() {
        let short_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .subsec_nanos();
        let unique = format!("ack-{}-{short_nanos}", std::process::id(),);

        let _ = cleanup_stale_ipc_endpoint(&unique);
        let name = ipc_name_for_app(&unique).expect("should build ipc name");
        let (sender, receiver) = mpsc::channel::<String>();

        spawn_ipc_listener(&name, listener_thread_name(&unique), sender)
            .expect("should spawn listener");

        let uris = vec!["pixiv://account/login?code=ack-check".to_string()];
        forward_uris_to_primary(&name, &uris).expect("forward should succeed with receipt");

        let forwarded = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("listener should forward URI to primary receiver");
        assert_eq!(forwarded, uris[0]);
    }
}
