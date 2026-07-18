//! Navigation control pages (one component per page).

use crate::helpers::{card, class, grid, note};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus::prelude::{
    FluentIcon, NavigationBackButtonVisible, NavigationPaneDisplayMode, NavigationViewItem,
    UiBreadcrumb, UiBreadcrumbItem, UiFlexColumn, UiLabel, UiNavigationView,
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

    note(
        commands,
        g,
        "The gallery shell itself is a full UiNavigationView (left pane, hierarchical MenuItems, Settings footer). The cards below embed smaller samples for pane modes, back chrome, info badges, and settings.",
    );

    // --- Hierarchical sample with badges + settings (Left / Expanded) ---
    spawn_nav_sample(
        commands,
        g,
        "Hierarchical menu + settings (Left)",
        UiNavigationView::new(sample_menu_items())
            .with_selected(0)
            .with_pane_title("Sample")
            .with_header("Home")
            .with_settings_visible(true)
            .with_settings_label("Settings")
            .with_footer_items([NavigationViewItem::new("Account")
                .with_icon(FluentIcon::Contact)
                .with_info_badge("!")]),
        &[
            (
                "Home",
                "Sample home content for the embedded NavigationView.",
            ),
            ("Documents", "Leaf page under Library → Documents."),
            ("Downloads", "Leaf page under Library → Downloads."),
            ("Inbox", "Destination with an info badge count."),
            ("Account", "Footer menu item with an info badge."),
            // Settings leaf has no content child → empty content area.
        ],
        "Left mode keeps an expanded inline pane. Footer items sit above the framework Settings leaf. Info badges appear on Inbox and Account.",
    );

    // --- LeftCompact ---
    spawn_nav_sample(
        commands,
        g,
        "LeftCompact pane",
        UiNavigationView::new(sample_menu_items())
            .with_selected(0)
            .with_pane_title("Compact")
            .with_pane_display_mode(NavigationPaneDisplayMode::LeftCompact)
            .with_pane_open(false)
            .with_settings_visible(true)
            .with_settings_label("Settings"),
        &[
            (
                "Home",
                "Compact rail: open the pane (hamburger) to see labels.",
            ),
            ("Documents", "Documents leaf under Library."),
            ("Downloads", "Downloads leaf under Library."),
            ("Inbox", "Inbox with badge."),
        ],
        "LeftCompact uses an icon rail when closed; opening the pane overlays content. Hierarchical parents open flyouts in rail mode.",
    );

    // --- LeftMinimal ---
    spawn_nav_sample(
        commands,
        g,
        "LeftMinimal pane",
        UiNavigationView::new(sample_menu_items())
            .with_selected(1)
            .with_pane_title("Minimal")
            .with_pane_display_mode(NavigationPaneDisplayMode::LeftMinimal)
            .with_pane_open(false)
            .with_settings_visible(false),
        &[
            ("Home", "Minimal mode hides the rail when closed."),
            ("Documents", "Documents leaf."),
            ("Downloads", "Downloads leaf."),
            ("Inbox", "Inbox leaf."),
        ],
        "LeftMinimal shows only chrome (toggle) when the pane is closed. Open the pane to navigate.",
    );

    // --- Back button ---
    spawn_nav_sample(
        commands,
        g,
        "Back button chrome",
        UiNavigationView::new(sample_menu_items())
            .with_selected(2)
            .with_pane_title("History")
            .with_back_button(NavigationBackButtonVisible::Visible, true)
            .with_settings_visible(true)
            .with_settings_label("Settings"),
        &[
            (
                "Home",
                "Back is enabled and visible (emits UiNavigationBackRequested).",
            ),
            ("Documents", "Documents leaf."),
            ("Downloads", "Downloads leaf."),
            ("Inbox", "Inbox leaf."),
        ],
        "with_back_button(Visible, true) shows an enabled back control in the pane header. Wire UiNavigationBackRequested in app code for navigation history.",
    );

    // --- Auto mode note ---
    spawn_nav_sample(
        commands,
        g,
        "Auto display mode",
        UiNavigationView::new(sample_menu_items())
            .with_selected(0)
            .with_pane_title("Auto")
            .with_pane_display_mode(NavigationPaneDisplayMode::Auto)
            .with_settings_visible(true),
        &[
            (
                "Home",
                "Auto resolves Expanded / Compact / Minimal from width.",
            ),
            ("Documents", "Documents leaf."),
            ("Downloads", "Downloads leaf."),
            ("Inbox", "Inbox leaf."),
        ],
        "Auto uses compact/expanded width thresholds (WinUI defaults). Resize the window to observe DisplayMode changes on the shell or this sample.",
    );
}

fn sample_menu_items() -> Vec<NavigationViewItem> {
    vec![
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
    ]
}

fn spawn_nav_sample(
    commands: &mut Commands,
    parent: Entity,
    title: &str,
    nav_config: UiNavigationView,
    pages: &[(&str, &str)],
    caption: &str,
) {
    let host = card(commands, parent, title);
    note(commands, host, caption);

    let nav = commands
        .spawn_scene(bsn! {
            template_value(nav_config)
            template_value(class("gallery.nav_view"))
            ChildOf(host)
        })
        .id();

    for (title, body) in pages {
        let page = commands
            .spawn_scene(bsn! {
                UiFlexColumn
                template_value(class("gallery.page"))
                ChildOf(nav)
            })
            .id();
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(*title))
            template_value(class("gallery.card_title"))
            ChildOf(page)
        });
        commands.spawn_scene(bsn! {
            template_value(UiLabel::new(*body))
            template_value(class("gallery.note"))
            ChildOf(page)
        });
    }
}
