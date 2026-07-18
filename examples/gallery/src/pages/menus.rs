//! Menu and window chrome pages (one component per page).

use crate::helpers::{card, grid, info_button, note};
use crate::state::GalleryBackdropPicker;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    FluentIcon, UiButton, UiDivider, UiMenuBar, UiMenuBarItem, UiMenuItem, UiRadioGroup, UiTitleBar,
    UiToolbar,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

pub fn spawn_menu_bar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let menu = card(commands, g, "Menu bar");
    let menu_bar = commands
        .spawn_scene(bsn! {
            UiMenuBar
            ChildOf(menu)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiMenuBarItem::new(
            "File",
            vec![
                UiMenuItem::new("New", "new"),
                UiMenuItem::new("Open...", "open"),
                UiMenuItem::new("Save", "save"),
                UiMenuItem::new("Exit", "exit"),
            ],
        ))
        ChildOf(menu_bar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiMenuBarItem::new(
            "Edit",
            vec![
                UiMenuItem::new("Undo", "undo"),
                UiMenuItem::new("Redo", "redo"),
                UiMenuItem::new("Preferences", "prefs"),
            ],
        ))
        ChildOf(menu_bar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiMenuBarItem::new(
            "Help",
            vec![UiMenuItem::new("About Picus Gallery", "about")],
        ))
        ChildOf(menu_bar)
    });
    note(
        commands,
        menu,
        "MenuBar supports nested items and dropdown overlay panels.",
    );
}

pub fn spawn_toolbar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let bar = card(commands, g, "Command toolbar");
    let toolbar = commands
        .spawn_scene(bsn! {
            UiToolbar
            ChildOf(bar)
        })
        .id();
    info_button(commands, toolbar, "New", "Toolbar: New");
    info_button(commands, toolbar, "Open", "Toolbar: Open");
    commands.spawn_scene(bsn! {
        template_value(UiDivider::vertical())
        ChildOf(toolbar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Save").with_icon(FluentIcon::Accept))
        ChildOf(toolbar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Delete").with_icon(FluentIcon::Delete))
        ChildOf(toolbar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiDivider::vertical())
        ChildOf(toolbar)
    });
    commands.spawn_scene(bsn! {
        template_value(UiButton::new("Settings").with_icon(FluentIcon::Settings))
        ChildOf(toolbar)
    });
    note(
        commands,
        bar,
        "UiToolbar is the Picus CommandBar-style horizontal action strip; children are laid out compactly.",
    );

    let compact = card(commands, g, "Icon-forward actions");
    let toolbar2 = commands
        .spawn_scene(bsn! {
            UiToolbar
            ChildOf(compact)
        })
        .id();
    for (label, icon) in [
        ("Cut", FluentIcon::Remove),
        ("Copy", FluentIcon::Character),
        ("Paste", FluentIcon::Add),
        ("Undo", FluentIcon::Back),
        ("Redo", FluentIcon::Forward),
    ] {
        commands.spawn_scene(bsn! {
            template_value(UiButton::new(label).with_icon(icon))
            ChildOf(toolbar2)
        });
    }
}

pub fn spawn_title_bar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let chrome = card(commands, g, "Custom title bar");
    commands.spawn_scene(bsn! {
        template_value(UiTitleBar {
            title: "Picus Gallery — custom title bar".to_string(),
            ..Default::default()
        })
        ChildOf(chrome)
    });
    note(
        commands,
        chrome,
        "UiTitleBar draws a custom window chrome with minimize/maximize/close controls.",
    );
}

pub fn spawn_window_backdrop_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let backdrop = card(commands, g, "Native material");
    commands.spawn_scene(bsn! {
        template_value(UiRadioGroup::new(["None", "Mica", "Acrylic"]).with_selected(1))
        GalleryBackdropPicker
        ChildOf(backdrop)
    });
    note(
        commands,
        backdrop,
        "The active Fluent theme applies the native material and its backdrop-aware fill tokens together.",
    );
}
