//! Theme transitions, spinners, loading indicators, and motion component examples.
//!
//! Corresponds to Fluent UI's theme switching animation and Spinner/Progress indicator components.

use crate::helpers::{card, class, grid, note, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    UiButton, UiProgressBar, UiSpinner, UiSwitch, UiThemePicker,
    scene::{CommandsSceneExt, bsn, template_value},
};

/// Theme transitions, spinner, and progress motion component examples.
///
/// Theme picker exercises color transition animation; spinners and progress bars
/// demonstrate indeterminate and determinate motion states.
pub fn spawn_transitions_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 2);

    let theme = card(commands, g, "Theme transitions");
    commands.spawn_scene(bsn! {
        template_value(UiThemePicker::fluent())
        ChildOf(theme)
    });
    note(
        commands,
        theme,
        "Changing theme variants exercises style target sync and color transition animation.",
    );
    let hover_btn = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Hover / press transition"))
            template_value(class("gallery.accent_button"))
            ChildOf(theme)
        })
        .id();
    commands
        .entity(hover_btn)
        .insert(crate::state::GalleryButtonAction::Info {
            message: "Transitions: hover/press transition clicked.".to_string(),
        });
    commands.spawn_scene(bsn! {
        template_value(UiSwitch::new(true).with_label("Animated switch target"))
        ChildOf(theme)
    });

    let loading = card(commands, g, "Motion");
    commands.spawn_scene(bsn! {
        template_value(UiSpinner::new())
        ChildOf(loading)
    });
    commands.spawn_scene(bsn! {
        template_value(UiSpinner::new().with_label("Loading resources"))
        ChildOf(loading)
    });
    commands.spawn_scene(bsn! {
        template_value(UiProgressBar::indeterminate())
        template_value(class("gallery.progress"))
        ChildOf(loading)
    });

    placeholder(
        commands,
        g,
        "ConfettiOverlay",
        "Picus has static UiCanvas commands but no retained animated canvas component API yet.",
    );

    placeholder(
        commands,
        g,
        "Storyboard transitions",
        "Current public styling exposes color/scale transitions, not arbitrary property storyboards.",
    );

    parent
}
