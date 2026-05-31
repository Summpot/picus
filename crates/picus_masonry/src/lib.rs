// Copyright 2026 Summp
// SPDX-License-Identifier: Apache-2.0

//! Compatibility facade for Picus' retained widget runtime.
//!
//! New code should prefer [`picus_widget::retained`]. This crate remains so
//! existing imports can keep compiling while the legacy Masonry-derived naming
//! is phased out.

#![forbid(unsafe_code)]

pub use picus_widget::retained::*;
