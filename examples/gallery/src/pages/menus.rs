//! Menu and window chrome pages (one component per page).

use crate::helpers::{card, grid, note};
use crate::state::GalleryBackdropPicker;
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::{
    UiMenuBar, UiMenuBarItem, UiMenuItem, UiRadioGroup, UiTitleBar,
    scene::{CommandsSceneExt, bsn, template_value},
};

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
