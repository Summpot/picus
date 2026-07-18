//! Status and info control pages (one component per page).

use crate::helpers::{card, class, grid, note};
use crate::state::GalleryButtonAction;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    AvatarShape, HasTooltip, UiAvatar, UiBadge, UiButton, UiFlexRow, UiLabel, UiMessageBar,
    UiProgressBar, UiSpinner, avatar_sizes,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

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
    note(
        commands,
        determinate,
        "Progress values are in the range 0.0–1.0.",
    );

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

pub fn spawn_info_badge_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let pills = card(commands, g, "Badge pills");
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("New"))
        ChildOf(pills)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("Beta"))
        ChildOf(pills)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("12"))
        ChildOf(pills)
    });
    note(
        commands,
        pills,
        "UiBadge is the Picus info-badge / pill control (WinUI InfoBadge).",
    );

    let with_label = card(commands, g, "Alongside content");
    let row = commands
        .spawn_scene(bsn! {
            UiFlexRow
            ChildOf(with_label)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiLabel::new("Inbox"))
        ChildOf(row)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBadge::new("3"))
        ChildOf(row)
    });
    note(
        commands,
        with_label,
        "NavigationView items can also carry an info badge string via with_info_badge.",
    );
}

pub fn spawn_info_bar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let kinds = card(commands, g, "Message bar kinds");
    commands.spawn_scene(bsn! {
        template_value(UiMessageBar::info(
            "Informational: theme resources loaded successfully.",
        ))
        ChildOf(kinds)
    });
    commands.spawn_scene(bsn! {
        template_value(UiMessageBar::success(
            "Success: changes were saved to the project.",
        ))
        ChildOf(kinds)
    });
    commands.spawn_scene(bsn! {
        template_value(UiMessageBar::warning(
            "Warning: partial theme is legal; missing tokens stay transparent.",
        ))
        ChildOf(kinds)
    });
    commands.spawn_scene(bsn! {
        template_value(UiMessageBar::error(
            "Error: structural RON parse failures should surface to the developer.",
        ))
        ChildOf(kinds)
    });
    note(
        commands,
        kinds,
        "UiMessageBar maps to WinUI InfoBar severity banners (info / success / warning / error).",
    );
}

pub fn spawn_avatar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 2);

    let initials = card(commands, g, "Initials from name");
    commands.spawn_scene(bsn! {
        template_value(UiAvatar::new("Ada Lovelace").with_size(avatar_sizes::XL))
        ChildOf(initials)
    });
    commands.spawn_scene(bsn! {
        template_value(UiAvatar::new("Grace Hopper").with_size(avatar_sizes::XXL))
        ChildOf(initials)
    });
    commands.spawn_scene(bsn! {
        template_value(UiAvatar::new("Alan Turing").with_size(avatar_sizes::XXXL))
        ChildOf(initials)
    });
    note(
        commands,
        initials,
        "UiAvatar derives initials and a stable palette color from the display name.",
    );

    let shapes = card(commands, g, "Shapes and sizes");
    commands.spawn_scene(bsn! {
        template_value(
            UiAvatar::new("Square")
                .with_size(avatar_sizes::XXL)
                .with_shape(AvatarShape::Square)
        )
        ChildOf(shapes)
    });
    commands.spawn_scene(bsn! {
        template_value(
            UiAvatar::new("Circular")
                .with_size(avatar_sizes::XXL)
                .with_shape(AvatarShape::Circular)
        )
        ChildOf(shapes)
    });
    commands.spawn_scene(bsn! {
        template_value(UiAvatar::new("XS").with_size(avatar_sizes::XS))
        ChildOf(shapes)
    });
    commands.spawn_scene(bsn! {
        template_value(UiAvatar::new("JUMBO").with_size(avatar_sizes::JUMBO))
        ChildOf(shapes)
    });
    note(
        commands,
        shapes,
        "Closest WinUI counterpart is PersonPicture; photo URLs are optional via with_image.",
    );
}
