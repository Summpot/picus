//! ECS resources and view-marker components for picuscode.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::bridge::{BridgeHandle, ChatMessage, ThreadSummary};

/// Top-level picuscode app state shared across views.
#[derive(Resource)]
pub struct PicusState {
    /// Bridge to the CodeWhale runtime thread.
    pub bridge: BridgeHandle,
    /// All known threads (refreshed from the bridge).
    pub threads: Vec<ThreadSummary>,
    /// The currently selected thread id, if any.
    pub active_thread: Option<String>,
    /// Messages loaded for the active thread.
    pub messages: Vec<ChatMessage>,
    /// The active streaming response id, when a turn is in flight.
    pub active_response_id: Option<String>,
    /// Whether a streaming turn is currently active for the active thread.
    pub streaming: bool,
    /// Last status line text.
    pub status: String,
    /// Current composer draft text.
    pub draft: String,
    /// Whether the About window is open.
    pub about_open: bool,
    /// Entity of the About window (when open).
    pub about_window: Option<Entity>,
    /// Entity of the About `UiRoot` (when open).
    pub about_root: Option<Entity>,
    /// Whether the Settings window is open.
    pub settings_open: bool,
    /// Entity of the Settings window (when open).
    pub settings_window: Option<Entity>,
    /// Entity of the Settings `UiRoot` (when open).
    pub settings_root: Option<Entity>,
    /// Entity of the transcript column (for appending new messages).
    pub transcript_column: Entity,
    /// Entity of the current turn's `UiStreamingMarkdown` component holder.
    pub streaming_entity: Entity,
    /// Entity of the sidebar column (for rebuilding the thread list).
    #[allow(dead_code)]
    pub sidebar_column: Entity,
    /// Cached config key/values for the settings panel.
    pub config_values: BTreeMap<String, String>,
    /// Pending config edits staged by the settings panel (key -> value).
    #[allow(dead_code)]
    pub config_edits: BTreeMap<String, String>,
    /// Last config operation result message, if any.
    pub config_status: Option<String>,
}

// ── View marker components ──────────────────────────────────────────────

/// Marker for the root of the primary chat window.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatRootView;

/// Marker for the body row (sidebar + transcript).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatBodyView;

/// Marker for the sidebar column (thread list).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SidebarColumnView;

/// Marker for the title bar row.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatTitleBarView;

/// Marker for the transcript column (messages stack here).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TranscriptColumnView;

/// Marker for the composer row.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComposerView;

/// Marker for the status line.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusLineView;

/// Marker for the secondary About window root.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AboutRootView;

/// Marker for the Settings window root.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsRootView;

/// Marker for the settings form column.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsFormView;