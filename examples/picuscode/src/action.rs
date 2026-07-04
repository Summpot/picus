//! UI actions emitted by picuscode controls and drained by the action system.

#![allow(dead_code)]

/// Actions emitted by picuscode UI controls.
///
/// Button helpers emit the bare action variant, so each control maps to one
/// distinct variant here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PicusCodeAction {
    /// Send the current composer draft on the active thread.
    Send,
    /// Composer text changed.
    ComposerChanged(String),
    /// Composer send was cancelled (escape during streaming).
    CancelTurn,
    /// Create a new thread.
    NewThread,
    /// Select a thread in the sidebar.
    SelectThread(String),
    /// Open the About window.
    OpenAbout,
    /// Close the About window.
    CloseAbout,
    /// Open the Settings window.
    OpenSettings,
    /// Close the Settings window.
    CloseSettings,
    /// Request a config list refresh.
    RefreshConfig,
    /// Set a config key (key|value payload).
    SetConfig(String, String),
    /// Reload config from disk.
    ReloadConfig,
    /// Rename the active thread.
    RenameThread(String),
}