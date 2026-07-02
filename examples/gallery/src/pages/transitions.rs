//! Theme transitions, spinners, loading indicators, and motion component examples.
//!
//! Corresponds to Fluent UI's theme switching animation and Spinner/Progress indicator components.

use crate::helpers::{card, class, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{UiButton, UiProgressBar, UiSpinner, UiSwitch, UiThemePicker};

/// Theme transitions, spinner, and progress motion component examples.
///
/// Theme picker exercises color transition animation; spinners and progress bars
/// demonstrate indeterminate and determinate motion states.
pub fn spawn_transitions_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let theme = card(commands, g, "Theme transitions");
    commands.spawn((UiThemePicker::fluent(), ChildOf(theme)));
    note(
        commands,
        theme,
        "Changing theme variants exercises style target sync and color transition animation.",
    );
    commands.spawn((
        UiButton::new("Hover / press transition"),
        class("gallery.accent_button"),
        ChildOf(theme),
    ));
    commands.spawn((
        UiSwitch::new(true).with_label("Animated switch target"),
        ChildOf(theme),
    ));

    let loading = card(commands, g, "Motion");
    commands.spawn((UiSpinner::new(), ChildOf(loading)));
    commands.spawn((
        UiSpinner::new().with_label("Loading resources"),
        ChildOf(loading),
    ));
    commands.spawn((
        UiProgressBar::indeterminate(),
        class("gallery.progress"),
        ChildOf(loading),
    ));

    placeholder(
        commands,
        g,
        "ConfettiOverlay",
        "MewUI draws timer-driven custom particles; Picus has static UiCanvas commands but no retained animated canvas component API yet.",
    );

    placeholder(
        commands,
        g,
        "Storyboard transitions",
        "Current public styling exposes color/scale transitions, not arbitrary property storyboards.",
    );

    parent
}
