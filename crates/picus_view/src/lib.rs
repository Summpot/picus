// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Picus-native Xilem view layer, targeting [`picus_widget`].

#![forbid(unsafe_code)]
#![allow(
    clippy::all,
    reason = "Vendored upstream view adapter code is kept close to the source while Picus integration tests cover its behavior."
)]
#![expect(
    missing_debug_implementations,
    reason = "Vendored upstream view types are intentionally light on Debug impls."
)]
#![expect(clippy::missing_assert_message, reason = "Vendored upstream behavior.")]

pub use picus_widget;
pub use picus_widget::masonry_core;
pub use xilem::core as core;

pub mod style;
pub mod views;
pub use views as view;

mod any_view;
mod masonry_root;
mod one_of;
mod pod;
mod view_ctx;
mod widget_view;

pub use any_view::AnyWidgetView;
pub use masonry_root::{InitialRootWidget, MasonryRoot};
pub use pod::Pod;
pub use view_ctx::ViewCtx;
pub use widget_view::{WidgetView, WidgetViewSequence};

// TODO - Remove these re-exports and fix the places in the crate that use them
pub(crate) use picus_widget::parley::Alignment as TextAlign;
pub(crate) use picus_widget::peniko::Color;
pub(crate) use picus_widget::widgets::InsertNewline;
