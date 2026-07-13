//! Status and info control pages (one component per page).

use crate::helpers::{card, class, grid, note};
use crate::state::GalleryButtonAction;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    HasTooltip, UiButton, UiProgressBar, UiSpinner,
    scene::{CommandsSceneExt, bsn, template_value},
};

pub fn spawn_progress_bar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let determinate = card(commands, g, "Determinate");
    commands.spawn_scene(bsn! {
        template_value(UiProgressBar::determinate(0.20))
        template_value(class("gallery.progress"))
        ChildOf(determinate)
    });
    commands.spawn_scene(bsn! {
        template_value(UiProgressBar::determinate(0.65))
        template_value(class("gallery.progress"))
        ChildOf(determinate)
    });
    note(commands, determinate, "Progress values are in the range 0.0–1.0.");

    let indeterminate = card(commands, g, "Indeterminate");
    commands.spawn_scene(bsn! {
        template_value(UiProgressBar::indeterminate())
        template_value(class("gallery.progress"))
        ChildOf(indeterminate)
    });
}

pub fn spawn_spinner_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let plain = card(commands, g, "Spinner");
    commands.spawn_scene(bsn! {
        template_value(UiSpinner::new())
        ChildOf(plain)
    });

    let labeled = card(commands, g, "Labeled spinner");
    commands.spawn_scene(bsn! {
        template_value(UiSpinner::new().with_label("Loading resources"))
        ChildOf(labeled)
    });
    note(
        commands,
        labeled,
        "Spinners indicate activity without a known completion percentage.",
    );
}

pub fn spawn_tooltip_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let hover = card(commands, g, "Hover tooltip");
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Hover for tooltip"))
        template_value(HasTooltip::new("Tooltip overlay anchored to this button."))
        ChildOf(hover)
    });

    let source = card(commands, g, "Tooltip source");
    let tooltip_src = commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Tooltip source"))
            template_value(HasTooltip::new("Tooltip overlay follows its source entity."))
            ChildOf(source)
        })
        .id();
    commands
        .entity(tooltip_src)
        .insert(GalleryButtonAction::Info {
            message: "ToolTip: source clicked (hover for tooltip).".to_string(),
        });
    note(
        commands,
        source,
        "HasTooltip attaches an anchored tooltip to any interactive control.",
    );
}
