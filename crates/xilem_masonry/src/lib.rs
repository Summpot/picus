// Copyright 2024 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

//! Picus-local Xilem view adapter for the Picus Masonry widget set.
//!
//! This crate vendors the upstream `xilem_masonry` view layer, retargeted to
//! [`picus_masonry`].

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

pub extern crate picus_masonry as masonry;
pub use xilem_core as core;

pub mod style;
pub mod view;

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
pub(crate) use masonry::parley::Alignment as TextAlign;
pub(crate) use masonry::peniko::Color;
pub(crate) use masonry::widgets::InsertNewline;
