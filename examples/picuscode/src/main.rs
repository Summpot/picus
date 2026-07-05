//! picuscode: a Codex-desktop-style GUI for CodeWhale.
//!
//! Architecture:
//! - A background thread ([`bridge::spawn_bridge`]) owns the CodeWhale
//!   `Runtime`, `ConfigStore`, and `StateStore`, talking to the ECS world
//!   through crossbeam channels. Config and state persist to the same
//!   `~/.codewhale/` files an installed `codewhale` binary uses, so the two
//!   are interchangeable.
//! - The UI is a Bevy + Picus ECS tree: a primary chat window (sidebar thread
//!   list + streaming transcript + composer) plus secondary About and
//!   Settings windows bound via `UiWindow`.
//! - Model turns stream through the OpenAI-compatible `/chat/completions`
//!   endpoint using provider/model/api_key resolved from the real codewhale
//!   config, so the same provider setup an installed codewhale uses is
//!   honored here.

// Event-routing logic uses `let`-chain guards for clarity; collapsing the
// nested `if`s would obscure the active-thread/response-id matching.
#![allow(clippy::collapsible_if)]

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use bevy_window::{Window, WindowClosed};
use picus::{
    AppPicusExt, PicusPlugin, StyleClass, UiEventQueue, UiMarkdown, UiRoot, UiScrollView,
    UiStreamingMarkdown, UiWindow, WorldSceneExt,
    bevy_app::{App, PostStartup, PreUpdate, Startup},
    bevy_math::Vec2,
    scene::{CommandsSceneExt, bsn, template_value},
    xilem::winit::error::EventLoopError,
};
use shared_utils::init_logging;

mod action;
mod bridge;
mod settings;
mod state;
mod ui;

use action::PicusCodeAction;
use bridge::{BridgeEvent, BridgeRequest, ChatMessage};
use state::{
    AboutRootView, ChatBodyView, ChatRootView, ChatTitleBarView, ComposerView, PicusState,
    SettingsFormView, SettingsRootView, SidebarColumnView, StatusLineView, TranscriptColumnView,
};

/// A static welcome markdown blob shown when no thread is selected.
const WELCOME_MARKDOWN: &str = "\
# picuscode

A **Codex-desktop**-style GUI for CodeWhale, built on Picus.

- Left: your CodeWhale threads (shared with the installed `codewhale` CLI).
- Center: streaming assistant replies rendered as Markdown.
- Bottom: composer — type a message and press **Send**.
- Title bar: **+ New** thread, **Settings** (provider/model/key), **About**.

Config and state persist to `~/.codewhale/`, so picuscode and your installed
`codewhale` stay in sync.

```rust
// picuscode embeds codewhale-core in-process:
let bridge = picuscode::bridge::spawn_bridge();
bridge.send(BridgeRequest::SendMessage { thread_id, input });
```
";

fn setup_chat_world(mut commands: Commands) {
    commands.spawn_scene(bsn! {
        UiRoot
        ChatRootView
        StyleClass(vec!["picuscode.root".to_string()])
        Children [
            (
                ChatTitleBarView
                StyleClass(vec!["picuscode.titlebar".to_string()])
            ),
            (
                ChatBodyView
                StyleClass(vec!["picuscode.body".to_string()])
                Children [
                    (
                        template_value(
                            UiScrollView::new(Vec2::new(220.0, 520.0), Vec2::new(220.0, 2400.0))
                                .with_vertical_scrollbar(true)
                                .with_horizontal_scrollbar(false)
                        )
                        StyleClass(vec!["picuscode.sidebar.scroll".to_string()])
                        Children [
                            (
                                SidebarColumnView
                                StyleClass(vec!["picuscode.sidebar".to_string()])
                            ),
                        ]
                    ),
                    (
                        template_value(
                            UiScrollView::new(Vec2::new(680.0, 520.0), Vec2::new(680.0, 1800.0))
                                .with_vertical_scrollbar(true)
                                .with_horizontal_scrollbar(false)
                        )
                        StyleClass(vec!["picuscode.transcript.scroll".to_string()])
                        Children [
                            (
                                TranscriptColumnView
                                StyleClass(vec!["picuscode.transcript".to_string()])
                            ),
                        ]
                    ),
                ]
            ),
            (
                ComposerView
                StyleClass(vec!["picuscode.composer".to_string()])
            ),
            (
                StatusLineView
                StyleClass(vec!["picuscode.status".to_string()])
            ),
        ]
    });
}

fn spawn_about_window(world: &mut World) -> (Entity, Entity) {
    let window = Window {
        title: "About picuscode".to_string(),
        resolution: bevy_window::WindowResolution::new(480, 420),
        ..Default::default()
    };
    let window_entity = world.spawn(window).id();

    let about_entity = world
        .spawn_scene(bsn! {
            UiRoot
            UiWindow(window_entity)
            AboutRootView
            StyleClass(vec!["picuscode.about.root".to_string()])
            Children [
                (UiMarkdown { source: { WELCOME_MARKDOWN.to_string() } }),
            ]
        })
        .expect("about BSN scene should spawn")
        .id();

    (about_entity, window_entity)
}

fn spawn_settings_window(world: &mut World) -> (Entity, Entity) {
    let window = Window {
        title: "picuscode Settings".to_string(),
        resolution: bevy_window::WindowResolution::new(560, 480),
        ..Default::default()
    };
    let window_entity = world.spawn(window).id();

    let settings_entity = world
        .spawn_scene(bsn! {
            UiRoot
            UiWindow(window_entity)
            SettingsRootView
            StyleClass(vec!["picuscode.settings.root".to_string()])
            Children [
                (SettingsFormView StyleClass(vec!["picuscode.settings.form".to_string()])),
            ]
        })
        .expect("settings BSN scene should spawn")
        .id();

    (settings_entity, window_entity)
}

picus::impl_ui_component_template!(ChatRootView, ui::project_chat_root);
picus::impl_ui_component_template!(ChatTitleBarView, ui::project_title_bar);
picus::impl_ui_component_template!(ChatBodyView, ui::project_chat_body);
picus::impl_ui_component_template!(SidebarColumnView, ui::project_sidebar_column);
picus::impl_ui_component_template!(TranscriptColumnView, ui::project_transcript_column);
picus::impl_ui_component_template!(ComposerView, ui::project_composer);
picus::impl_ui_component_template!(StatusLineView, ui::project_status_line);
picus::impl_ui_component_template!(AboutRootView, ui::project_about_root);
picus::impl_ui_component_template!(SettingsRootView, ui::project_settings_root);
picus::impl_ui_component_template!(SettingsFormView, ui::project_settings_form);

/// System: locate the sidebar + transcript entities, spawn the bridge, and
/// insert the shared `PicusState` resource. Requests an initial thread list
/// and config list so the first frame has real data.
fn seed_picus_state(world: &mut World) {
    let sidebar_column = {
        let mut q = world.query_filtered::<Entity, With<SidebarColumnView>>();
        q.iter(world)
            .next()
            .expect("sidebar column should exist after setup")
    };
    let transcript_column = {
        let mut q = world.query_filtered::<Entity, With<TranscriptColumnView>>();
        q.iter(world)
            .next()
            .expect("transcript column should exist after setup")
    };

    let bridge = bridge::spawn_bridge();

    // Kick off the initial data loads.
    let _ = bridge.tx.send(BridgeRequest::ListThreads);
    let _ = bridge.tx.send(BridgeRequest::ConfigList);

    world.insert_resource(PicusState {
        bridge,
        threads: Vec::new(),
        active_thread: None,
        messages: Vec::new(),
        active_response_id: None,
        streaming: false,
        status: "Ready".to_string(),
        draft: String::new(),
        about_open: false,
        about_window: None,
        about_root: None,
        settings_open: false,
        settings_window: None,
        settings_root: None,
        transcript_column,
        streaming_entity: Entity::PLACEHOLDER,
        sidebar_column,
        config_values: BTreeMap::new(),
        config_edits: BTreeMap::new(),
        config_status: None,
    });
}

/// System: drain bridge events into `PicusState` and trigger transcript
/// rebuilds when the message set changes.
fn poll_bridge_events(world: &mut World) {
    let Some(events) = world
        .get_resource::<PicusState>()
        .map(|s| s.bridge.events.clone())
    else {
        return;
    };

    let mut transcript_rebuild = false;
    let mut threads_changed = false;

    // Non-blocking drain: `recv()` would block the Bevy render thread and
    // freeze the window, so use `try_recv()` and process whatever has arrived
    // since the last frame.
    while let Ok(event) = events.try_recv() {
        match event {
            BridgeEvent::Ready => {}
            BridgeEvent::Threads(threads) => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    s.threads = threads;
                }
                threads_changed = true;
            }
            BridgeEvent::ThreadCreated { thread } => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    s.active_thread = Some(thread.id.clone());
                    s.messages = Vec::new();
                    s.status = "New thread".to_string();
                }
                transcript_rebuild = true;
                let _ = world
                    .get_resource::<PicusState>()
                    .unwrap()
                    .bridge
                    .tx
                    .send(BridgeRequest::ListThreads);
            }
            BridgeEvent::ThreadHistory {
                thread_id,
                messages,
                thread: _,
            } => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    if s.active_thread.as_deref() == Some(thread_id.as_str()) {
                        s.messages = messages;
                        transcript_rebuild = true;
                    }
                }
            }
            BridgeEvent::TurnStarted {
                thread_id,
                response_id,
            } => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    if s.active_thread.as_deref() == Some(thread_id.as_str()) {
                        s.status = format!("Streaming {response_id}");
                        s.active_response_id = Some(response_id);
                        s.streaming = true;
                    }
                }
            }
            BridgeEvent::TurnDelta {
                thread_id,
                response_id,
                delta,
            } => {
                let active_ok = world.get_resource::<PicusState>().is_some_and(|s| {
                    s.active_thread.as_deref() == Some(thread_id.as_str())
                        && s.active_response_id.as_deref() == Some(response_id.as_str())
                });
                if active_ok {
                    let entity = world.resource::<PicusState>().streaming_entity;
                    if let Some(mut streaming) = world.get_mut::<UiStreamingMarkdown>(entity) {
                        streaming.append(&delta);
                        if delta.ends_with("\n\n") || delta.ends_with("```\n") {
                            streaming.flush_completed();
                        }
                    }
                }
            }
            BridgeEvent::TurnEnded {
                thread_id,
                response_id,
                ok,
            } => {
                let is_active = world.get_resource::<PicusState>().is_some_and(|s| {
                    s.active_thread.as_deref() == Some(thread_id.as_str())
                        && s.active_response_id.as_deref() == Some(response_id.as_str())
                });
                if is_active {
                    let entity = world.resource::<PicusState>().streaming_entity;
                    if let Some(mut streaming) = world.get_mut::<UiStreamingMarkdown>(entity) {
                        streaming.finish();
                    }
                    if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                        s.streaming = false;
                        s.active_response_id = None;
                        s.status = if ok {
                            "Ready".to_string()
                        } else {
                            "Turn failed — see status".to_string()
                        };
                    }
                    // Reload history so the persisted assistant message shows
                    // up if the streaming entity was never created (e.g. on
                    // error before first delta).
                    if let Some(s) = world.get_resource::<PicusState>() {
                        if let Some(tid) = s.active_thread.clone() {
                            let _ = s
                                .bridge
                                .tx
                                .send(BridgeRequest::ReadThread { thread_id: tid });
                        }
                    }
                }
            }
            BridgeEvent::TurnError {
                thread_id: _,
                response_id: _,
                message,
            } => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    s.streaming = false;
                    s.active_response_id = None;
                    s.status = format!("Error: {message}");
                }
            }
            BridgeEvent::ConfigListed(values) => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    s.config_values = values;
                }
            }
            BridgeEvent::ConfigGot { key, value } => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    s.config_values.insert(key, value.unwrap_or_default());
                }
            }
            BridgeEvent::ConfigResult { ok, error } => {
                if let Some(mut s) = world.get_resource_mut::<PicusState>() {
                    s.config_status = if ok {
                        Some("Saved.".to_string())
                    } else {
                        Some(format!("Save failed: {}", error.unwrap_or_default()))
                    };
                }
                // Refresh the config list so the panel reflects the new state.
                if let Some(s) = world.get_resource::<PicusState>() {
                    let _ = s.bridge.tx.send(BridgeRequest::ConfigList);
                }
            }
        }
    }

    if transcript_rebuild {
        rebuild_transcript(world);
    }
    if threads_changed {
        // The sidebar reads `PicusState.threads` directly in its project fn,
        // so no entity rebuild is needed — picus re-projects on the next
        // synthesis pass because the resource changed.
    }
}

/// Rebuilds the transcript column children from `PicusState.messages`.
///
/// Despawns all existing children of the transcript column, spawns a
/// `UiMarkdown` for each persisted message, and leaves room for the active
/// streaming entity (which is created separately on `Send`).
fn rebuild_transcript(world: &mut World) {
    let (transcript_column, messages, active_thread) = {
        let s = world.resource::<PicusState>();
        (
            s.transcript_column,
            s.messages.clone(),
            s.active_thread.clone(),
        )
    };

    // Despawn existing children of the transcript column.
    let children_to_despawn: Vec<Entity> = {
        let mut q = world.query::<(Entity, &bevy_ecs::hierarchy::ChildOf)>();
        q.iter(world)
            .filter(|(_, parent)| parent.parent() == transcript_column)
            .map(|(e, _)| e)
            .collect()
    };
    for e in children_to_despawn {
        if let Ok(ent) = world.get_entity_mut(e) {
            ent.despawn();
        }
    }

    if active_thread.is_none() {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            s.streaming_entity = Entity::PLACEHOLDER;
        }
        return;
    }

    for m in &messages {
        let rendered = render_message_markdown(m);
        world.spawn((
            UiMarkdown::new(rendered),
            bevy_ecs::hierarchy::ChildOf(transcript_column),
        ));
    }
}

fn render_message_markdown(m: &ChatMessage) -> String {
    match m.role.as_str() {
        "user" => format!("**You:** {}", m.content),
        "assistant" => m.content.clone(),
        "system" | "history" => format!("> _system:_ {}", m.content),
        other => format!("**{other}:** {}", m.content),
    }
}

/// System: drain UI actions and dispatch them to the bridge or window
/// lifecycle.
fn handle_picuscode_actions(world: &mut World) {
    let actions = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<PicusCodeAction>();

    let mut to_send = false;
    let mut to_cancel = false;
    let mut to_open_about = false;
    let mut to_close_about = false;
    let mut to_open_settings = false;
    let mut to_close_settings = false;
    let mut to_new_thread = false;
    let mut to_reload_config = false;
    let mut to_apply_config_edits = false;
    let mut latest_draft = None;
    let mut select_thread: Option<String> = None;
    let mut config_edits: Vec<(String, String)> = Vec::new();
    let mut set_config: Option<(String, String)> = None;
    let mut rename_thread: Option<String> = None;

    for event in actions {
        match event.action {
            PicusCodeAction::Send => to_send = true,
            PicusCodeAction::ComposerChanged(value) => latest_draft = Some(value),
            PicusCodeAction::CancelTurn => to_cancel = true,
            PicusCodeAction::NewThread => to_new_thread = true,
            PicusCodeAction::SelectThread(id) => select_thread = Some(id),
            PicusCodeAction::OpenAbout => to_open_about = true,
            PicusCodeAction::CloseAbout => to_close_about = true,
            PicusCodeAction::OpenSettings => to_open_settings = true,
            PicusCodeAction::CloseSettings => to_close_settings = true,
            PicusCodeAction::RefreshConfig => {
                if let Some(s) = world.get_resource::<PicusState>() {
                    let _ = s.bridge.tx.send(BridgeRequest::ConfigList);
                }
            }
            PicusCodeAction::EditConfig(k, v) => config_edits.push((k, v)),
            PicusCodeAction::ApplyConfigEdits => to_apply_config_edits = true,
            PicusCodeAction::SetConfig(k, v) => set_config = Some((k, v)),
            PicusCodeAction::ReloadConfig => to_reload_config = true,
            PicusCodeAction::RenameThread(name) => rename_thread = Some(name),
        }
    }

    if let Some(draft) = latest_draft {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            s.draft = draft;
        }
    }

    if to_close_about {
        close_secondary_window(world, false);
    }
    if to_close_settings {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            s.config_edits.clear();
        }
        close_secondary_window(world, true);
    }
    if to_open_about {
        open_secondary_window(world, false);
    }
    if to_open_settings {
        open_secondary_window(world, true);
        if let Some(s) = world.get_resource::<PicusState>() {
            let _ = s.bridge.tx.send(BridgeRequest::ConfigList);
        }
    }

    if to_new_thread {
        if let Some(s) = world.get_resource::<PicusState>() {
            let _ = s.bridge.tx.send(BridgeRequest::CreateThread);
        }
    }

    if to_reload_config {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            s.config_edits.clear();
            let _ = s.bridge.tx.send(BridgeRequest::ConfigReload);
        }
    }

    if !config_edits.is_empty() {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            for (key, value) in config_edits {
                s.config_edits.insert(key, value);
            }
            let count = s.config_edits.len();
            s.config_status = Some(format!("{count} unsaved change(s)."));
        }
    }

    if to_apply_config_edits {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            let edits = std::mem::take(&mut s.config_edits);
            if edits.is_empty() {
                s.config_status = Some("No changes to save.".to_string());
            } else {
                let count = edits.len();
                for (key, value) in edits {
                    let _ = s.bridge.tx.send(BridgeRequest::ConfigSet { key, value });
                }
                s.config_status = Some(format!("Saving {count} change(s)..."));
            }
        }
    }

    if let Some(id) = select_thread {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            if s.active_thread.as_deref() != Some(id.as_str()) {
                s.active_thread = Some(id.clone());
                s.messages = Vec::new();
                s.streaming = false;
                s.active_response_id = None;
                s.status = format!("Loading {id}");
                let _ = s.bridge.tx.send(BridgeRequest::ReadThread {
                    thread_id: id.clone(),
                });
            }
        }
        rebuild_transcript(world);
    }

    if let Some((key, value)) = set_config {
        if let Some(s) = world.get_resource::<PicusState>() {
            let _ = s.bridge.tx.send(BridgeRequest::ConfigSet { key, value });
        }
    }

    if let Some(name) = rename_thread {
        if let Some(s) = world.get_resource::<PicusState>() {
            if let Some(tid) = s.active_thread.clone() {
                let _ = s.bridge.tx.send(BridgeRequest::SetThreadName {
                    thread_id: tid,
                    name,
                });
            }
        }
    }

    if to_cancel {
        if let Some(s) = world.get_resource::<PicusState>() {
            if let Some(tid) = s.active_thread.clone() {
                let _ = s
                    .bridge
                    .tx
                    .send(BridgeRequest::CancelTurn { thread_id: tid });
            }
        }
    }

    if to_send {
        start_send_turn(world);
    }
}

fn start_send_turn(world: &mut World) {
    let (can_send, draft, thread_id, transcript) = {
        let s = world.resource::<PicusState>();
        (
            !s.streaming && s.active_thread.is_some() && !s.draft.is_empty(),
            s.draft.clone(),
            s.active_thread.clone(),
            s.transcript_column,
        )
    };
    if !can_send {
        if let Some(mut s) = world.get_resource_mut::<PicusState>() {
            if s.active_thread.is_none() {
                s.status = "Create or select a thread first".to_string();
            } else if s.draft.is_empty() {
                s.status = "Composer is empty".to_string();
            }
        }
        return;
    }
    let Some(thread_id) = thread_id else {
        return;
    };

    // Spawn the user bubble + a streaming markdown holder for the assistant.
    world.spawn((
        UiMarkdown::new(format!("**You:** {draft}")),
        bevy_ecs::hierarchy::ChildOf(transcript),
    ));
    let streaming_entity = world
        .spawn((
            UiStreamingMarkdown::new(),
            bevy_ecs::hierarchy::ChildOf(transcript),
        ))
        .id();

    {
        let mut s = world.resource_mut::<PicusState>();
        s.streaming_entity = streaming_entity;
        s.streaming = true;
        s.active_response_id = None;
        s.status = "Starting turn".to_string();
        s.draft.clear();
    }

    if let Some(s) = world.get_resource::<PicusState>() {
        let _ = s.bridge.tx.send(BridgeRequest::SendMessage {
            thread_id,
            input: draft,
        });
    }
}

fn open_secondary_window(world: &mut World, settings: bool) {
    let should_open = {
        let mut s = world.resource_mut::<PicusState>();
        if settings {
            if s.settings_open {
                false
            } else {
                s.settings_open = true;
                true
            }
        } else if s.about_open {
            false
        } else {
            s.about_open = true;
            true
        }
    };

    if should_open {
        if settings {
            let (root, window) = spawn_settings_window(world);
            let mut s = world.resource_mut::<PicusState>();
            s.settings_root = Some(root);
            s.settings_window = Some(window);
        } else {
            let (root, window) = spawn_about_window(world);
            let mut s = world.resource_mut::<PicusState>();
            s.about_root = Some(root);
            s.about_window = Some(window);
        }
    }
}

fn close_secondary_window(world: &mut World, settings: bool) {
    let close_targets = {
        let mut s = world.resource_mut::<PicusState>();
        if settings {
            let root = s.settings_root.take();
            let window = s.settings_window.take();
            if window.is_some() {
                s.settings_open = false;
            }
            window.map(|w| (root, w))
        } else {
            let root = s.about_root.take();
            let window = s.about_window.take();
            if window.is_some() {
                s.about_open = false;
            }
            window.map(|w| (root, w))
        }
    };

    if let Some((root, window)) = close_targets {
        if let Some(root) = root
            && let Ok(entity) = world.get_entity_mut(root)
        {
            entity.despawn();
        }
        if let Ok(entity) = world.get_entity_mut(window) {
            entity.despawn();
        }
    }
}

fn handle_secondary_window_closed(
    mut closed_windows: MessageReader<WindowClosed>,
    state: Option<ResMut<PicusState>>,
    mut commands: Commands,
) {
    let Some(mut state) = state else {
        closed_windows.clear();
        return;
    };

    for event in closed_windows.read() {
        if state.about_window == Some(event.window) {
            state.about_window = None;
            state.about_open = false;
            if let Some(root) = state.about_root.take() {
                commands.entity(root).despawn();
            }
        }

        if state.settings_window == Some(event.window) {
            state.settings_window = None;
            state.settings_open = false;
            if let Some(root) = state.settings_root.take() {
                commands.entity(root).despawn();
            }
        }
    }
}

/// Periodically request a thread list refresh so newly created threads (from
/// other codewhale clients) show up.
fn refresh_thread_list(period: std::time::Duration) -> impl std::ops::FnMut(&mut World) {
    let mut last = std::time::Instant::now();
    move |world: &mut World| {
        if last.elapsed() < period {
            return;
        }
        last = std::time::Instant::now();
        if let Some(s) = world.get_resource::<PicusState>() {
            let _ = s.bridge.tx.send(BridgeRequest::ListThreads);
        }
    }
}

fn build_picuscode_app() -> App {
    init_logging();

    let mut app = App::new();
    app.add_plugins(PicusPlugin);

    app.load_style_sheet_ron(include_str!("../assets/themes/picuscode.ron"))
        .register_ui_component::<ChatRootView>()
        .register_ui_component::<ChatTitleBarView>()
        .register_ui_component::<ChatBodyView>()
        .register_ui_component::<SidebarColumnView>()
        .register_ui_component::<TranscriptColumnView>()
        .register_ui_component::<ComposerView>()
        .register_ui_component::<StatusLineView>()
        .register_ui_component::<AboutRootView>()
        .register_ui_component::<SettingsRootView>()
        .register_ui_component::<SettingsFormView>()
        .add_systems(Startup, setup_chat_world)
        .add_systems(PostStartup, seed_picus_state)
        .add_systems(
            PreUpdate,
            (
                handle_secondary_window_closed,
                handle_picuscode_actions,
                poll_bridge_events,
            )
                .chain()
                .after(picus::core::route_masonry_view_messages),
        )
        .add_systems(
            PreUpdate,
            refresh_thread_list(std::time::Duration::from_secs(3)),
        );

    app
}

fn main() -> Result<(), EventLoopError> {
    picus::run_app_with_window_options(build_picuscode_app(), "picuscode", |options| {
        options.with_initial_inner_size(picus::xilem::winit::dpi::LogicalSize::new(960.0, 720.0))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_picuscode_theme_ron_parses() {
        picus::parse_stylesheet_ron(include_str!("../assets/themes/picuscode.ron"))
            .expect("embedded picuscode stylesheet should parse");
    }

    #[test]
    fn picuscode_theme_declares_fluent_dark_default_variant() {
        let sheet = picus::parse_stylesheet_ron(include_str!("../assets/themes/picuscode.ron"))
            .expect("embedded picuscode stylesheet should parse");
        assert_eq!(sheet.default_variant.as_deref(), Some("dark"));
    }

    #[test]
    fn picuscode_app_applies_stylesheet_default_variant() {
        let app = super::build_picuscode_app();

        let active = app.world().resource::<picus::ActiveStyleVariant>();
        assert_eq!(active.0.as_deref(), Some("dark"));

        let applied = app.world().resource::<picus::AppliedStyleVariant>();
        assert_eq!(applied.0.as_deref(), Some("dark"));

        let sheet = app.world().resource::<picus::StyleSheet>();
        assert!(
            sheet.tokens.contains_key("surface-bg"),
            "picuscode stylesheet references Fluent tokens, so a Fluent variant must be active"
        );
    }
}
