//! In-process CodeWhale runtime bridge.
//!
//! This owns a dedicated tokio runtime on a background thread and drives the
//! real `codewhale-core` `Runtime` plus `codewhale-config::ConfigStore` and
//! `codewhale-state::StateStore`. Because those crates resolve their on-disk
//! paths against the same `~/.codewhale/` directory an installed `codewhale`
//! binary uses, picuscode is fully config- and state-compatible with the
//! user's installed CodeWhale.
//!
//! The bridge communicates with the ECS world through two crossbeam channels:
//! `BridgeRequest` in, `BridgeEvent` out. ECS systems push requests and poll
//! events each frame, keeping the async runtime off the Bevy render thread.
//!
//! For the actual model turn, `Runtime::handle_thread(Message)` in the fork
//! only records the user message and emits a `queued` delta — the real LLM
//! call lives in the TUI's own client. picuscode therefore drives the
//! OpenAI-compatible `/chat/completions` streaming endpoint directly, using
//! `codewhale-config`'s `resolve_runtime_options` for provider/model/key
//! resolution so the same config an installed codewhale uses is honored.

// The bridge uses `let`-chain style guards (`if let Some(x) = ... && cond`)
// extensively for clarity; collapsing them hurts readability.
#![allow(clippy::collapsible_if)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use codewhale_agent::ModelRegistry;
use codewhale_config::{CliRuntimeOverrides, ConfigStore, provider::WireFormat};
use codewhale_core::{InitialHistory, Runtime};
use codewhale_hooks::{HookDispatcher, JsonlHookSink, StdoutHookSink};
use codewhale_mcp::McpManager;
use codewhale_protocol::{Thread, ThreadStatus};
use codewhale_state::{StateStore, ThreadListFilters};
use codewhale_tools::ToolRegistry;
use crossbeam_channel::{Receiver, Sender, unbounded};
use codewhale_execpolicy::ExecPolicyEngine;
use serde_json::{Value, json};
use tokio::runtime::Runtime as TokioRuntime;
use tracing::warn;

/// A request pushed from the ECS world to the bridge thread.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum BridgeRequest {
    /// Refresh the thread list from the state store.
    ListThreads,
    /// Create a fresh thread and return its id.
    CreateThread,
    /// Load a thread's persisted messages and goal.
    ReadThread { thread_id: String },
    /// Send a user message and start a streaming model turn.
    SendMessage { thread_id: String, input: String },
    /// Cancel the in-flight turn for a thread, if any.
    CancelTurn { thread_id: String },
    /// Rename a thread.
    SetThreadName { thread_id: String, name: String },
    /// Archive a thread.
    ArchiveThread { thread_id: String },
    /// List all config key/values (display form).
    ConfigList,
    /// Read a single config key (raw form).
    ConfigGet { key: String },
    /// Set a config key and persist.
    ConfigSet { key: String, value: String },
    /// Unset a config key and persist.
    ConfigUnset { key: String },
    /// Reload config + exec policy from disk.
    ConfigReload,
}

/// An event streamed back from the bridge thread to the ECS world.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum BridgeEvent {
    /// The thread list was refreshed.
    Threads(Vec<ThreadSummary>),
    /// A thread's history was loaded.
    ThreadHistory {
        thread_id: String,
        messages: Vec<ChatMessage>,
        thread: Option<Thread>,
    },
    /// A new thread was created.
    ThreadCreated { thread: Thread },
    /// A streaming turn started.
    TurnStarted { thread_id: String, response_id: String },
    /// An incremental assistant delta.
    TurnDelta {
        thread_id: String,
        response_id: String,
        delta: String,
    },
    /// A streaming turn finished.
    TurnEnded {
        thread_id: String,
        response_id: String,
        ok: bool,
    },
    /// An error occurred on a turn (e.g. missing API key).
    TurnError {
        thread_id: String,
        response_id: String,
        message: String,
    },
    /// A config list response.
    ConfigListed(BTreeMap<String, String>),
    /// A single config value response.
    ConfigGot { key: String, value: Option<String> },
    /// A config set/unset/reload result.
    ConfigResult { ok: bool, error: Option<String> },
    /// Bridge thread is ready.
    Ready,
}

/// A flattened thread summary for UI rendering.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ThreadSummary {
    pub id: String,
    pub name: Option<String>,
    pub preview: String,
    pub model_provider: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: bool,
}

/// A flattened chat message for UI rendering.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

impl From<codewhale_state::ThreadMetadata> for ThreadSummary {
    fn from(m: codewhale_state::ThreadMetadata) -> Self {
        Self {
            id: m.id,
            name: m.name,
            preview: m.preview,
            model_provider: m.model_provider,
            created_at: m.created_at,
            updated_at: m.updated_at,
            archived: m.archived,
        }
    }
}

impl From<codewhale_state::MessageRecord> for ChatMessage {
    fn from(m: codewhale_state::MessageRecord) -> Self {
        Self {
            id: m.id,
            role: m.role,
            content: m.content,
            created_at: m.created_at,
        }
    }
}

/// Handle held by the ECS world to talk to the bridge thread.
#[derive(Clone)]
pub struct BridgeHandle {
    pub tx: Sender<BridgeRequest>,
    pub events: Receiver<BridgeEvent>,
}

/// Spawns the bridge background thread and returns a handle.
///
/// The thread owns its own tokio runtime and the CodeWhale `Runtime`,
/// `ConfigStore`, and `StateStore`. Dropping the handle does not stop the
/// thread; the process exits when the UI loop exits.
pub fn spawn_bridge() -> BridgeHandle {
    spawn_bridge_with_config_path(None)
}

/// Like [`spawn_bridge`] but pins the config file to `config_path`.
///
/// Tests use this with a tempdir path so they never touch the user's real
/// `~/.codewhale/` config. `None` falls back to the default codewhale path
/// resolution, sharing state with an installed `codewhale` binary.
pub fn spawn_bridge_with_config_path(config_path: Option<PathBuf>) -> BridgeHandle {
    let (req_tx, req_rx) = unbounded::<BridgeRequest>();
    let (evt_tx, evt_rx) = unbounded::<BridgeEvent>();

    std::thread::Builder::new()
        .name("picuscode-bridge".into())
        .spawn(move || {
            if let Err(err) = run_bridge(req_rx, evt_tx.clone(), config_path) {
                warn!("picuscode bridge thread exited with error: {err:#}");
            }
        })
        .expect("failed to spawn picuscode bridge thread");

    BridgeHandle {
        tx: req_tx,
        events: evt_rx,
    }
}

fn run_bridge(
    req_rx: Receiver<BridgeRequest>,
    evt_tx: Sender<BridgeEvent>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let tokio_rt = TokioRuntime::new()?;
    let _guard = tokio_rt.enter();

    // Load config + state using the same default path resolution as an
    // installed `codewhale` binary, so config/state stay compatible. Tests
    // pass an explicit tempdir path to stay isolated from the user's real
    // ~/.codewhale.
    let store = ConfigStore::load(config_path)?;
    let config_path = store.path().to_path_buf();
    let config = store.config.clone();
    let exec_policy = store.exec_policy_engine();

    let state_db_path = config_path
        .parent()
        .map(|parent| parent.join("state.db"))
        .ok_or_else(|| anyhow!("config path has no parent directory"))?;
    let state_store = StateStore::open(Some(state_db_path))?;

    let mut hooks = HookDispatcher::default();
    hooks.add_sink(Arc::new(StdoutHookSink));
    let hook_log_path = config_path
        .parent()
        .map(|parent| parent.join("events.jsonl"))
        .unwrap_or_else(|| PathBuf::from(".codewhale/events.jsonl"));
    hooks.add_sink(Arc::new(JsonlHookSink::new(hook_log_path)));

    let registry = ModelRegistry::default();
    let runtime = Runtime::new(
        config.clone(),
        registry.clone(),
        state_store,
        Arc::new(ToolRegistry::default()),
        Arc::new(McpManager::default()),
        exec_policy,
        hooks,
    );

    // Shared mutable state guarded by tokio Mutex; the config store is kept
    // alongside so ConfigSet can persist with comment preservation.
    let state = Arc::new(tokio::sync::Mutex::new(BridgeState {
        runtime,
        config_store: store,
        registry,
        active_turns: BTreeMap::new(),
    }));

    let _ = evt_tx.send(BridgeEvent::Ready);

    // Track cancellation flags per thread.
    let cancel_flags: Arc<tokio::sync::Mutex<BTreeMap<String, Arc<AtomicBool>>>> =
        Arc::new(tokio::sync::Mutex::new(BTreeMap::new()));

    while let Ok(req) = req_rx.recv() {
        let state = state.clone();
        let cancel_flags = cancel_flags.clone();
        let evt_tx = evt_tx.clone();

        tokio_rt.spawn(async move {
            if let Err(err) = handle_request(req, state, cancel_flags, evt_tx).await {
                warn!("bridge request failed: {err:#}");
            }
        });
    }

    Ok(())
}

struct BridgeState {
    runtime: Runtime,
    config_store: ConfigStore,
    registry: ModelRegistry,
    active_turns: BTreeMap<String, String>,
}

async fn handle_request(
    req: BridgeRequest,
    state: Arc<tokio::sync::Mutex<BridgeState>>,
    cancel_flags: Arc<tokio::sync::Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
    evt_tx: Sender<BridgeEvent>,
) -> Result<()> {
    match req {
        BridgeRequest::ListThreads => {
            let s = state.lock().await;
            let threads = s
                .runtime
                .thread_manager
                .state_store()
                .list_threads(ThreadListFilters::default())?;
            let summaries = threads.into_iter().map(ThreadSummary::from).collect();
            let _ = evt_tx.send(BridgeEvent::Threads(summaries));
            Ok(())
        }
        BridgeRequest::CreateThread => {
            let mut s = state.lock().await;
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let provider = s.runtime.config.provider;
            let new = s.runtime.thread_manager.spawn_thread_with_history(
                provider.as_str().to_string(),
                cwd,
                InitialHistory::New,
                true,
            )?;
            let thread = to_protocol_thread(&new);
            let _ = evt_tx.send(BridgeEvent::ThreadCreated { thread });
            Ok(())
        }
        BridgeRequest::ReadThread { thread_id } => {
            let s = state.lock().await;
            let store = s.runtime.thread_manager.state_store();
            let messages = store
                .list_messages(&thread_id, Some(500))?
                .into_iter()
                .map(ChatMessage::from)
                .collect();
            let thread_meta = store.get_thread(&thread_id)?;
            let thread = thread_meta.map(to_protocol_thread_from_meta);
            let _ = evt_tx.send(BridgeEvent::ThreadHistory {
                thread_id,
                messages,
                thread,
            });
            Ok(())
        }
        BridgeRequest::SendMessage { thread_id, input } => {
            start_turn(thread_id, input, state, cancel_flags, evt_tx).await
        }
        BridgeRequest::CancelTurn { thread_id } => {
            let mut flags = cancel_flags.lock().await;
            if let Some(flag) = flags.remove(&thread_id) {
                flag.store(true, Ordering::SeqCst);
            }
            Ok(())
        }
        BridgeRequest::SetThreadName { thread_id, name } => {
            let s = state.lock().await;
            let store = s.runtime.thread_manager.state_store();
            let now = chrono::Utc::now().timestamp();
            store.append_thread_name(&thread_id, Some(name), now, None)?;
            drop(s);
            Box::pin(handle_request(
                BridgeRequest::ListThreads,
                state,
                cancel_flags,
                evt_tx,
            ))
            .await
        }
        BridgeRequest::ArchiveThread { thread_id } => {
            let s = state.lock().await;
            s.runtime
                .thread_manager
                .state_store()
                .mark_archived(&thread_id)?;
            drop(s);
            Box::pin(handle_request(
                BridgeRequest::ListThreads,
                state,
                cancel_flags,
                evt_tx,
            ))
            .await
        }
        BridgeRequest::ConfigList => {
            let s = state.lock().await;
            let values = s.config_store.config.list_values();
            let _ = evt_tx.send(BridgeEvent::ConfigListed(values));
            Ok(())
        }
        BridgeRequest::ConfigGet { key } => {
            let s = state.lock().await;
            let value = s.config_store.config.get_value(&key);
            let _ = evt_tx.send(BridgeEvent::ConfigGot { key, value });
            Ok(())
        }
        BridgeRequest::ConfigSet { key, value } => {
            let mut s = state.lock().await;
            let result = s.config_store.config.set_value(&key, &value);
            let ok = result.is_ok();
            let error = result.err().map(|e| e.to_string());
            if ok {
                if let Err(e) = s.config_store.save() {
                    let _ = evt_tx.send(BridgeEvent::ConfigResult {
                        ok: false,
                        error: Some(format!("failed to save config: {e}")),
                    });
                    return Ok(());
                }
                let snapshot = s.config_store.config.clone();
                s.runtime.update_config(snapshot);
            }
            let _ = evt_tx.send(BridgeEvent::ConfigResult { ok, error });
            Ok(())
        }
        BridgeRequest::ConfigUnset { key } => {
            let mut s = state.lock().await;
            let result = s.config_store.config.unset_value(&key);
            let ok = result.is_ok();
            let error = result.err().map(|e| e.to_string());
            if ok {
                if let Err(e) = s.config_store.save() {
                    let _ = evt_tx.send(BridgeEvent::ConfigResult {
                        ok: false,
                        error: Some(format!("failed to save config: {e}")),
                    });
                    return Ok(());
                }
                let snapshot = s.config_store.config.clone();
                s.runtime.update_config(snapshot);
            }
            let _ = evt_tx.send(BridgeEvent::ConfigResult { ok, error });
            Ok(())
        }
        BridgeRequest::ConfigReload => {
            let mut s = state.lock().await;
            match ConfigStore::load(None) {
                Ok(store) => {
                    let config = store.config.clone();
                    let exec_policy: ExecPolicyEngine = store.exec_policy_engine();
                    s.runtime.reload_config_and_policy(config, exec_policy);
                    s.config_store = store;
                    let _ = evt_tx.send(BridgeEvent::ConfigResult {
                        ok: true,
                        error: None,
                    });
                }
                Err(e) => {
                    let _ = evt_tx.send(BridgeEvent::ConfigResult {
                        ok: false,
                        error: Some(format!("failed to reload config: {e}")),
                    });
                }
            }
            Ok(())
        }
    }
}

async fn start_turn(
    thread_id: String,
    input: String,
    state: Arc<tokio::sync::Mutex<BridgeState>>,
    cancel_flags: Arc<tokio::sync::Mutex<BTreeMap<String, Arc<AtomicBool>>>>,
    evt_tx: Sender<BridgeEvent>,
) -> Result<()> {
    // Record the user message and resolve provider endpoint while holding the
    // lock briefly, then release it for the streaming HTTP call.
    let (response_id, resolved, history) = {
        let mut s = state.lock().await;
        s.runtime
            .thread_manager
            .touch_message(&thread_id, &input)?;
        s.runtime
            .thread_manager
            .state_store()
            .append_message(&thread_id, "user", &input, None)?;

        let overrides = CliRuntimeOverrides::default();
        let resolved = s.config_store.config.resolve_runtime_options(&overrides);
        let selection = s
            .registry
            .resolve(Some(&resolved.model), Some(resolved.provider));
        let resolved_model = selection.resolved.id.clone();

        let history = s
            .runtime
            .thread_manager
            .state_store()
            .list_messages(&thread_id, Some(500))?;

        let response_id = format!("resp-{}", uuid::Uuid::new_v4());
        s.active_turns.insert(thread_id.clone(), response_id.clone());
        (response_id, (resolved, resolved_model), history)
    };

    let (resolved, resolved_model) = resolved;
    let provider_meta = resolved.provider.provider();

    let _ = evt_tx.send(BridgeEvent::TurnStarted {
        thread_id: thread_id.clone(),
        response_id: response_id.clone(),
    });

    let api_key = resolved.api_key;
    let base_url = resolved.base_url;
    let wire = provider_meta.wire();
    let http_headers = resolved.http_headers.clone();
    let insecure = resolved.insecure_skip_tls_verify;

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut flags = cancel_flags.lock().await;
        flags.insert(thread_id.clone(), cancel_flag.clone());
    }

    let evt_tx_stream = evt_tx.clone();
    let state_stream = state.clone();
    let cancel_flags_stream = cancel_flags.clone();
    let thread_id_stream = thread_id.clone();
    let response_id_stream = response_id.clone();

    tokio::spawn(async move {
        let outcome = run_streaming_turn(
            &thread_id_stream,
            &response_id_stream,
            wire,
            &base_url,
            &resolved_model,
            api_key.as_deref(),
            &http_headers,
            insecure,
            &history,
            &input,
            cancel_flag,
            evt_tx_stream.clone(),
        )
        .await;

        let ok = outcome.is_ok();
        if let Err(err) = &outcome {
            let _ = evt_tx_stream.send(BridgeEvent::TurnError {
                thread_id: thread_id_stream.clone(),
                response_id: response_id_stream.clone(),
                message: err.to_string(),
            });
        }

        // Persist the assistant reply text (best-effort) and clear the
        // active turn marker.
        if let Ok(text) = outcome.as_ref() {
            let mut s = state_stream.lock().await;
            let store = s.runtime.thread_manager.state_store();
            let payload = json!({
                "provider": resolved.provider.as_str(),
                "model": resolved_model,
                "response_id": response_id_stream,
            });
            if let Err(e) = store.append_message(
                &thread_id_stream,
                "assistant",
                text,
                Some(payload),
            ) {
                warn!("failed to persist assistant message: {e:#}");
            }
            s.active_turns.remove(&thread_id_stream);
        } else {
            let mut s = state_stream.lock().await;
            s.active_turns.remove(&thread_id_stream);
        }

        let mut flags = cancel_flags_stream.lock().await;
        flags.remove(&thread_id_stream);

        let _ = evt_tx_stream.send(BridgeEvent::TurnEnded {
            thread_id: thread_id_stream,
            response_id: response_id_stream,
            ok,
        });
    });

    Ok(())
}

/// Runs a streaming chat-completions turn against the resolved provider
/// endpoint and emits `TurnDelta` events as SSE chunks arrive.
///
/// Only OpenAI-compatible `ChatCompletions` wire format is supported in this
/// first cut; Anthropic Messages and Responses APIs will land in a follow-up
/// along with tool-call rendering.
#[allow(clippy::too_many_arguments)]
async fn run_streaming_turn(
    thread_id: &str,
    response_id: &str,
    wire: WireFormat,
    base_url: &str,
    model: &str,
    api_key: Option<&str>,
    http_headers: &BTreeMap<String, String>,
    insecure_skip_tls_verify: bool,
    history: &[codewhale_state::MessageRecord],
    input: &str,
    cancel_flag: Arc<AtomicBool>,
    evt_tx: Sender<BridgeEvent>,
) -> Result<String> {
    if wire != WireFormat::ChatCompletions {
        return Err(anyhow!(
            "picuscode streaming currently only supports OpenAI-compatible chat-completions providers (got {wire:?}). Set provider to deepseek/openai/openrouter/etc."
        ));
    }

    let api_key = api_key.ok_or_else(|| {
        anyhow!(
            "no API key configured for provider. Set it in Settings \
             (api_key) or via the provider's env var, then retry."
        )
    })?;

    let mut messages = Vec::new();
    for m in history {
        let role = match m.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            "system" | "history" => "system",
            other => other,
        };
        // Skip empty content (e.g. structured-only items).
        if m.content.trim().is_empty() {
            continue;
        }
        messages.push(json!({ "role": role, "content": m.content }));
    }
    // The just-appended user message is already in `history`, but guard
    // against any ordering issue by ensuring the latest user turn is present.
    if messages
        .last()
        .is_none_or(|last| last["role"] != "user" || last["content"] != input)
    {
        messages.push(json!({ "role": "user", "content": input }));
    }

    let body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let mut client_builder = reqwest::Client::builder();
    if insecure_skip_tls_verify {
        client_builder = client_builder.danger_accept_invalid_certs(true);
    }
    let client = client_builder.build()?;

    let mut req = client
        .post(&url)
        .header("authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .json(&body);
    for (k, v) in http_headers {
        req = req.header(k, v);
    }

    let response = req.send().await?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("upstream returned {status}: {text}"));
    }

    use futures::StreamExt as _;
    let mut stream = response.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();

    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            return Ok(full);
        }
        let maybe_chunk = stream.next().await;
        let Some(chunk) = maybe_chunk else { break };
        let chunk = chunk?;
        buf.push_str(std::str::from_utf8(chunk.as_ref()).unwrap_or(""));
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            buf.drain(..=nl);
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("data: ") {
                if rest.trim() == "[DONE]" {
                    return Ok(full);
                }
                if let Ok(value) = serde_json::from_str::<Value>(rest) {
                    if let Some(delta) = value
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        if !delta.is_empty() {
                            full.push_str(delta);
                            let _ = evt_tx.send(BridgeEvent::TurnDelta {
                                thread_id: thread_id.to_string(),
                                response_id: response_id.to_string(),
                                delta: delta.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(full)
}

fn to_protocol_thread(new: &codewhale_core::NewThread) -> Thread {
    new.thread.clone()
}

fn to_protocol_thread_from_meta(m: codewhale_state::ThreadMetadata) -> Thread {
    let status = match m.status {
        codewhale_state::ThreadStatus::Running => ThreadStatus::Running,
        codewhale_state::ThreadStatus::Idle => ThreadStatus::Idle,
        codewhale_state::ThreadStatus::Completed => ThreadStatus::Completed,
        codewhale_state::ThreadStatus::Failed => ThreadStatus::Failed,
        codewhale_state::ThreadStatus::Paused => ThreadStatus::Paused,
        codewhale_state::ThreadStatus::Archived => ThreadStatus::Archived,
    };
    Thread {
        id: m.id,
        preview: m.preview,
        ephemeral: m.ephemeral,
        model_provider: m.model_provider,
        created_at: m.created_at,
        updated_at: m.updated_at,
        status,
        path: m.path,
        cwd: m.cwd,
        cli_version: m.cli_version,
        source: match m.source {
            codewhale_state::SessionSource::Interactive => {
                codewhale_protocol::SessionSource::Interactive
            }
            codewhale_state::SessionSource::Resume => codewhale_protocol::SessionSource::Resume,
            codewhale_state::SessionSource::Fork => codewhale_protocol::SessionSource::Fork,
            codewhale_state::SessionSource::Api => codewhale_protocol::SessionSource::Api,
            codewhale_state::SessionSource::Unknown => codewhale_protocol::SessionSource::Unknown,
        },
        name: m.name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Drains bridge events until a predicate matches or the timeout expires.
    fn wait_event<F>(handle: &BridgeHandle, predicate: F) -> Option<BridgeEvent>
    where
        F: Fn(&BridgeEvent) -> bool,
    {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(ev) = handle.events.recv_timeout(Duration::from_millis(50)) {
                if predicate(&ev) {
                    return Some(ev);
                }
            }
        }
        None
    }

    #[test]
    fn bridge_thread_list_create_and_config_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config_path = tmp.path().join("config.toml");
        let handle = spawn_bridge_with_config_path(Some(config_path.clone()));

        // Bridge signals readiness, then the initial ListThreads + ConfigList
        // requests (sent by seed_picus_state) come back. Here we drive them
        // directly.
        let _ = handle.tx.send(BridgeRequest::ListThreads);
        let threads = wait_event(&handle, |e| matches!(e, BridgeEvent::Threads(_)));
        assert!(matches!(threads, Some(BridgeEvent::Threads(_))));

        let _ = handle.tx.send(BridgeRequest::ConfigSet {
            key: "model".to_string(),
            value: "deepseek-chat".to_string(),
        });
        let set_result = wait_event(&handle, |e| matches!(e, BridgeEvent::ConfigResult { .. }));
        assert!(
            matches!(&set_result, Some(BridgeEvent::ConfigResult { ok: true, .. })),
            "config set should succeed: {set_result:?}"
        );

        let _ = handle.tx.send(BridgeRequest::ConfigList);
        let listed = wait_event(&handle, |e| matches!(e, BridgeEvent::ConfigListed(_)));
        if let Some(BridgeEvent::ConfigListed(values)) = listed {
            assert_eq!(values.get("model").map(String::as_str), Some("deepseek-chat"));
        } else {
            panic!("expected ConfigListed event");
        }

        // The persisted file should exist on disk (config-compatible with an
        // installed codewhale pointing at the same path).
        assert!(config_path.exists(), "config.toml should be persisted");

        let _ = handle.tx.send(BridgeRequest::CreateThread);
        let created = wait_event(&handle, |e| matches!(e, BridgeEvent::ThreadCreated { .. }));
        let thread_id = match created {
            Some(BridgeEvent::ThreadCreated { thread }) => thread.id,
            other => panic!("expected ThreadCreated, got {other:?}"),
        };

        let _ = handle.tx.send(BridgeRequest::ReadThread {
            thread_id: thread_id.clone(),
        });
        let history = wait_event(&handle, |e| matches!(e, BridgeEvent::ThreadHistory { .. }));
        assert!(
            matches!(&history, Some(BridgeEvent::ThreadHistory { messages, .. }) if messages.is_empty()),
            "freshly created thread should have empty history: {history:?}"
        );
    }
}