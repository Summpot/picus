//! Navigation control pages (one component per page).

use crate::helpers::{card, class, grid, note};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    FluentIcon, NavigationViewItem, UiBreadcrumb, UiBreadcrumbItem, UiFlexColumn, UiLabel,
    UiNavigationView,
};
use picus::scene::{CommandsSceneExt, bsn, template_value};

pub fn spawn_breadcrumb_bar_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let path = card(commands, g, "Navigation path");
    let crumb = commands
        .spawn_scene(bsn! {
            UiBreadcrumb
            ChildOf(path)
        })
        .id();
    for label in ["Home", "Library", "Controls", "Navigation", "BreadcrumbBar"] {
        commands.spawn_scene(bsn! {
            template_value(UiBreadcrumbItem::new(label))
            ChildOf(crumb)
        });
    }
    note(
        commands,
        path,
        "UiBreadcrumb renders items with chevron separators; the last item is the current page.",
    );

    let short = card(commands, g, "Short trail");
    let crumb2 = commands
        .spawn_scene(bsn! {
            UiBreadcrumb
            ChildOf(short)
        })
        .id();
    commands.spawn_scene(bsn! {
        template_value(UiBreadcrumbItem::new("Projects"))
        ChildOf(crumb2)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBreadcrumbItem::new("picus"))
        ChildOf(crumb2)
    });
    commands.spawn_scene(bsn! {
        template_value(UiBreadcrumbItem::new("gallery"))
        ChildOf(crumb2)
    });
}

pub fn spawn_navigation_view_page(commands: &mut Commands, parent: Entity) {
    let g = grid(commands, parent, 1);

    let sample = card(commands, g, "Embedded sample");
    note(
        commands,
        sample,
        "The gallery shell itself is a full UiNavigationView. This page embeds a smaller sample with hierarchical items and a settings footer.",
    );

    let items = vec![
        NavigationViewItem::new("Home").with_icon(FluentIcon::AllApps),
        NavigationViewItem::new("Library")
            .with_icon(FluentIcon::Folder)
            .with_children([
                NavigationViewItem::new("Documents").with_icon(FluentIcon::Character),
                NavigationViewItem::new("Downloads").with_icon(FluentIcon::Folder),
            ])
            .expanded(),
        NavigationViewItem::new("Inbox")
            .with_icon(FluentIcon::Message)
            .with_info_badge("3"),
    ];

    let nav = commands
        .spawn_scene(bsn! {
            template_value(
                UiNavigationView::new(items)
                    .with_selected(0)
                    .with_pane_title("Sample")
                    .with_settings_visible(true)
                    .with_settings_label("Settings")
            )
            template_value(class("gallery.nav_view"))
            ChildOf(sample)
        })
        .id();

    for (title, body) in [
        ("Home", "Sample home content for the embedded NavigationView."),
        ("Documents", "Leaf page under Library → Documents."),
        ("Downloads", "Leaf page under Library → Downloads."),
        ("Inbox", "Destination with an info badge."),
    ] {
        let page = commands
            .spawn_scene(bsn! {
                UiFlexColumn
                template_value(class("gallery.page"))
                ChildOf(nav)
            })
            .id();
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(title))
            template_value(class("gallery.card_title"))
            ChildOf(page)
        });
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(body))
            template_value(class("gallery.note"))
            ChildOf(page)
        });
    }
}
