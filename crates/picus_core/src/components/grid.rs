use bevy_ecs::prelude::*;
use std::{fmt, str::FromStr};

use crate::{ProjectionCtx, UiView, components::UiComponentTemplate};

/// Track sizing intent for [`UiGrid`] rows and columns.
///
/// Stores `Auto`, pixel, and star tracks so applications can describe their
/// intended layout. The current Masonry-backed projector uses the
/// number of tracks and falls back to uniform cell sizing; full Auto/Star measure
/// distribution belongs in a custom grid widget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiGridLength {
    Auto,
    Px(f64),
    Star(f64),
}

impl UiGridLength {
    #[must_use]
    pub const fn auto() -> Self {
        Self::Auto
    }

    #[must_use]
    pub const fn px(value: f64) -> Self {
        Self::Px(value)
    }

    #[must_use]
    pub const fn star(value: f64) -> Self {
        Self::Star(value)
    }
}

/// Error returned when parsing a grid track specification fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiGridLengthParseError {
    pub token: String,
}

impl fmt::Display for UiGridLengthParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid grid track token '{}'", self.token)
    }
}

impl std::error::Error for UiGridLengthParseError {}

impl FromStr for UiGridLength {
    type Err = UiGridLengthParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let token = raw.trim();
        if token.eq_ignore_ascii_case("auto") {
            return Ok(Self::Auto);
        }

        if token == "*" {
            return Ok(Self::Star(1.0));
        }

        if let Some(star) = token.strip_suffix('*') {
            let value = star
                .trim()
                .parse::<f64>()
                .map_err(|_| UiGridLengthParseError {
                    token: token.to_string(),
                })?;
            if value.is_finite() && value > 0.0 {
                return Ok(Self::Star(value));
            }
            return Err(UiGridLengthParseError {
                token: token.to_string(),
            });
        }

        let px = token.strip_suffix("px").unwrap_or(token).trim();
        let value = px.parse::<f64>().map_err(|_| UiGridLengthParseError {
            token: token.to_string(),
        })?;
        if value.is_finite() && value >= 0.0 {
            Ok(Self::Px(value))
        } else {
            Err(UiGridLengthParseError {
                token: token.to_string(),
            })
        }
    }
}

/// Auto placement scan direction for children without a complete [`UiGridCell`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiGridAutoFlow {
    #[default]
    Row,
    Column,
}

/// Grid container backed by Masonry's native grid widget.
///
/// Children can opt into explicit or partial placement with [`UiGridCell`].
/// Children without a cell marker are auto-placed into the next available cell.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiGrid {
    pub columns: u32,
    pub rows: u32,
    pub column_tracks: Vec<UiGridLength>,
    pub row_tracks: Vec<UiGridLength>,
    pub auto_flow: UiGridAutoFlow,
    pub auto_indexing: bool,
    pub show_grid_lines: bool,
    pub share_star_size: bool,
}

impl UiGrid {
    #[must_use]
    pub fn new(columns: u32, rows: u32) -> Self {
        Self {
            columns: columns.max(1),
            rows: rows.max(1),
            column_tracks: Vec::new(),
            row_tracks: Vec::new(),
            auto_flow: UiGridAutoFlow::Row,
            auto_indexing: true,
            show_grid_lines: false,
            share_star_size: false,
        }
    }

    #[must_use]
    pub fn with_columns(mut self, columns: u32) -> Self {
        self.columns = columns.max(1);
        self
    }

    #[must_use]
    pub fn with_rows(mut self, rows: u32) -> Self {
        self.rows = rows.max(1);
        self
    }

    #[must_use]
    pub fn with_column_tracks(mut self, tracks: impl IntoIterator<Item = UiGridLength>) -> Self {
        self.column_tracks = tracks.into_iter().collect();
        self.columns = self.columns.max(self.column_tracks.len().max(1) as u32);
        self
    }

    #[must_use]
    pub fn with_row_tracks(mut self, tracks: impl IntoIterator<Item = UiGridLength>) -> Self {
        self.row_tracks = tracks.into_iter().collect();
        self.rows = self.rows.max(self.row_tracks.len().max(1) as u32);
        self
    }

    pub fn parse_tracks(spec: &str) -> Result<Vec<UiGridLength>, UiGridLengthParseError> {
        spec.split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace())
            .filter(|token| !token.trim().is_empty())
            .map(str::parse)
            .collect()
    }

    pub fn try_with_columns_spec(self, spec: &str) -> Result<Self, UiGridLengthParseError> {
        Ok(self.with_column_tracks(Self::parse_tracks(spec)?))
    }

    pub fn try_with_rows_spec(self, spec: &str) -> Result<Self, UiGridLengthParseError> {
        Ok(self.with_row_tracks(Self::parse_tracks(spec)?))
    }

    #[must_use]
    pub fn with_auto_flow(mut self, auto_flow: UiGridAutoFlow) -> Self {
        self.auto_flow = auto_flow;
        self
    }

    #[must_use]
    pub fn auto_indexing(mut self, auto_indexing: bool) -> Self {
        self.auto_indexing = auto_indexing;
        self
    }

    #[must_use]
    pub fn show_grid_lines(mut self, show_grid_lines: bool) -> Self {
        self.show_grid_lines = show_grid_lines;
        self
    }

    #[must_use]
    pub fn share_star_size(mut self, share_star_size: bool) -> Self {
        self.share_star_size = share_star_size;
        self
    }

    #[must_use]
    pub fn effective_columns(&self) -> u32 {
        self.columns
            .max(self.column_tracks.len().max(1) as u32)
            .max(1)
    }

    #[must_use]
    pub fn effective_rows(&self) -> u32 {
        self.rows.max(self.row_tracks.len().max(1) as u32).max(1)
    }
}

impl Default for UiGrid {
    fn default() -> Self {
        Self::new(1, 1)
    }
}

/// Placement metadata for a child inside [`UiGrid`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiGridCell {
    pub column: u32,
    pub row: u32,
    pub column_span: u32,
    pub row_span: u32,
    pub has_column: bool,
    pub has_row: bool,
}

impl UiGridCell {
    #[must_use]
    pub fn new(column: u32, row: u32) -> Self {
        Self {
            column,
            row,
            column_span: 1,
            row_span: 1,
            has_column: true,
            has_row: true,
        }
    }

    #[must_use]
    pub fn row(row: u32) -> Self {
        Self {
            row,
            has_row: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn column(column: u32) -> Self {
        Self {
            column,
            has_column: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_column(mut self, column: u32) -> Self {
        self.column = column;
        self.has_column = true;
        self
    }

    #[must_use]
    pub fn with_row(mut self, row: u32) -> Self {
        self.row = row;
        self.has_row = true;
        self
    }

    #[must_use]
    pub fn with_span(mut self, column_span: u32, row_span: u32) -> Self {
        self.column_span = column_span.max(1);
        self.row_span = row_span.max(1);
        self
    }
}

impl Default for UiGridCell {
    fn default() -> Self {
        Self {
            column: 0,
            row: 0,
            column_span: 1,
            row_span: 1,
            has_column: false,
            has_row: false,
        }
    }
}

impl UiComponentTemplate for UiGrid {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::layout::project_grid(component, ctx)
    }
}
