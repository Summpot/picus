//! Picus Gallery — Fluent UI-inspired component showcase.
//!
//! This example demonstrates all Picus UI components in a navigable gallery,
//! organized following the Fluent UI documentation pattern where related
//! components are grouped by category and each component variant is shown
//! as a standalone example.
//!
//! ## Architecture
//!
//! - [`helpers`] — Shared utilities (card, grid, note, placeholder, canvas/image helpers)
//! - [`state`] — `GalleryPage` enum, `GalleryState`/`GalleryRuntime` resources
//! - [`views`] — `UiComponentTemplate` implementations for `GalleryRoot` and `GalleryStatus`
//! - [`events`] — Event dispatch for all component interactions
//! - [`pages`] — 15 page modules, each showcasing a component category
//!
//! ## Fluent UI Pattern Mapping
//!
//! | Picus Gallery          | Fluent UI                          |
//! |------------------------|------------------------------------|
//! | `pages/buttons.rs`     | `Button.stories.tsx` variants      |
//! | `pages/inputs.rs`      | `TextField`, `ComboBox` examples   |
//! | `pages/selection.rs`   | `Checkbox`, `Radio` examples       |
//! | `pages/lists.rs`       | `DetailsList`, `TreeView` examples |
//! | `pages/overlay.rs`     | `Dialog`, `Toast` examples         |
//! | Sidebar navigation     | Fluent UI nav/component picker      |
//! | Status bar events      | Storybook action logger            |
//! | `gallery.ron` theme    | Fluent UI `makeStyles` tokens      |

use picus_core::{
    AppPicusExt, PicusPlugin, UiBadge, UiButton, UiFlexColumn, UiFlexRow, UiLabel, UiRoot,
    UiScrollView, UiTabBar, UiThemePicker,
    bevy_app::{App, Startup, Update},
    bevy_ecs::{hierarchy::ChildOf, prelude::*},
    run_app_with_window_options,
    xilem::winit::{dpi::LogicalSize, error::EventLoopError},
};
use shared_utils::init_logging;

mod events;
mod helpers;
mod pages;
mod state;
mod views;

use events::drain_gallery_events;
use helpers::{PAGE_CONTENT, PAGE_VIEWPORT, class, classes};
use state::{GalleryPage, GalleryRuntime, GalleryState};
use views::{GalleryRoot, GalleryStatus};

/// Build the full gallery application tree.
///
/// Creates the top bar, sidebar navigation, page tab bar, and spawns
/// all 15 component showcase pages.
fn setup_gallery(mut commands: Commands) {
    let root = commands
        .spawn((UiRoot, GalleryRoot, class("gallery.root")))
        .id();

    spawn_top_bar(&mut commands, root);

    commands.spawn((GalleryStatus, class("gallery.status"), ChildOf(root)));

    let body = commands
        .spawn((UiFlexRow, class("gallery.body"), ChildOf(root)))
        .id();

    let sidebar = commands
        .spawn((UiFlexColumn, class("gallery.sidebar"), ChildOf(body)))
        .id();

    let mut nav_buttons = Vec::new();
    for page in GalleryPage::ALL {
        let mut names = vec!["gallery.sidebar_button"];
        if page == GalleryPage::Buttons {
            names.push("gallery.sidebar_button.active");
        }
        let button = commands
            .spawn((
                UiButton::new(page.label()),
                classes(&names),
                ChildOf(sidebar),
            ))
            .id();
        nav_buttons.push(button);
    }

    let pages_tab_bar = commands
        .spawn((
            UiTabBar::new(GalleryPage::ALL.map(GalleryPage::label)).with_hidden_headers(),
            class("gallery.content_scroll"),
            ChildOf(body),
        ))
        .id();

    let open_dialog_btn = spawn_page(
        &mut commands,
        pages_tab_bar,
        GalleryPage::Buttons,
        pages::buttons::spawn_buttons_page,
    );
    let runtime_refs = GalleryRuntime {
        pages_tab_bar,
        nav_buttons,
        open_dialog_btn,
        persistent_toast_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::Inputs,
            pages::inputs::spawn_inputs_page,
        ),
        success_toast_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::Selection,
            pages::selection::spawn_selection_page,
        ),
        warning_toast_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::WindowMenu,
            pages::window_menu::spawn_window_menu_page,
        ),
        error_toast_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::MessageBox,
            pages::message_box::spawn_message_box_page,
        ),
        prompt_dialog_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::Lists,
            pages::lists::spawn_lists_page,
        ),
        native_message_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::GridView,
            pages::grid_view::spawn_grid_view_page,
        ),
        popover_dialog_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::Panels,
            pages::panels::spawn_panels_page,
        ),
        burst_placeholder_btn: spawn_page(
            &mut commands,
            pages_tab_bar,
            GalleryPage::Layout,
            pages::layout::spawn_layout_page,
        ),
    };

    spawn_page(
        &mut commands,
        pages_tab_bar,
        GalleryPage::Typography,
        pages::typography::spawn_typography_page,
    );
    spawn_page(
        &mut commands,
        pages_tab_bar,
        GalleryPage::Media,
        pages::media::spawn_media_page,
    );
    spawn_page(
        &mut commands,
        pages_tab_bar,
        GalleryPage::Shapes,
        pages::shapes::spawn_shapes_page,
    );
    spawn_page(
        &mut commands,
        pages_tab_bar,
        GalleryPage::Icons,
        pages::icons::spawn_icons_page,
    );
    spawn_page(
        &mut commands,
        pages_tab_bar,
        GalleryPage::Transitions,
        pages::transitions::spawn_transitions_page,
    );
    spawn_page(
        &mut commands,
        pages_tab_bar,
        GalleryPage::Overlay,
        pages::overlay::spawn_overlay_page,
    );

    commands.insert_resource(runtime_refs);
}

/// Create the top bar with branding, subtitle, theme picker, and badge.
fn spawn_top_bar(commands: &mut Commands, root: Entity) {
    let top = commands
        .spawn((UiFlexRow, class("gallery.top_bar"), ChildOf(root)))
        .id();
    let brand = commands
        .spawn((UiFlexRow, class("gallery.brand"), ChildOf(top)))
        .id();
    commands.spawn((UiLabel::new("P"), class("gallery.logo"), ChildOf(brand)));
    let title_col = commands
        .spawn((UiFlexColumn, class("gallery.brand"), ChildOf(brand)))
        .id();
    commands.spawn((
        UiLabel::new("Picus Gallery"),
        class("gallery.title"),
        ChildOf(title_col),
    ));
    commands.spawn((
        UiLabel::new("MewUI FBA gallery inspired example, rebuilt with ECS-native Picus controls"),
        class("gallery.subtitle"),
        ChildOf(title_col),
    ));

    let tools = commands
        .spawn((UiFlexRow, class("gallery.brand"), ChildOf(top)))
        .id();
    commands.spawn((UiThemePicker::fluent(), ChildOf(tools)));
    commands.spawn((UiBadge::new("FBA parity pass"), ChildOf(tools)));
}

/// Spawn a single gallery page inside the pages tab bar.
fn spawn_page(
    commands: &mut Commands,
    pages_tab_bar: Entity,
    page: GalleryPage,
    build: fn(&mut Commands, Entity) -> Entity,
) -> Entity {
    let scroll = commands
        .spawn((
            UiScrollView::new(PAGE_VIEWPORT, PAGE_CONTENT)
                .with_vertical_scrollbar(true)
                .with_horizontal_scrollbar(false),
            class("gallery.content_scroll"),
            ChildOf(pages_tab_bar),
        ))
        .id();
    let page_col = commands
        .spawn((UiFlexColumn, class("gallery.page"), ChildOf(scroll)))
        .id();
    commands.spawn((
        UiLabel::new(page.label()),
        class("gallery.section_title"),
        ChildOf(page_col),
    ));
    build(commands, page_col)
}

picus_core::impl_ui_component_template!(GalleryRoot, views::project_gallery_root);
picus_core::impl_ui_component_template!(GalleryStatus, views::project_gallery_status);

/// Build the Bevy application with all gallery systems and resources.
fn build_gallery_app() -> App {
    init_logging();

    let mut app = App::new();
    app.add_plugins(PicusPlugin)
        .load_style_sheet_ron(include_str!("../assets/themes/gallery.ron"))
        .register_xilem_font(picus_core::SyncAssetSource::Bytes(include_bytes!(
            "../../../assets/fonts/NotoSans-Regular.ttf",
        )))
        .insert_resource(GalleryState::default())
        .register_ui_component::<GalleryRoot>()
        .register_ui_component::<GalleryStatus>()
        .add_systems(Startup, setup_gallery)
        .add_systems(
            Update,
            drain_gallery_events
                .after(picus_core::handle_widget_actions)
                .after(picus_core::handle_overlay_actions),
        );

    app
}

/// Application entry point.
///
/// Creates a 1360×760 window with the Fluent UI-inspired Picus Gallery.
fn main() -> Result<(), EventLoopError> {
    run_app_with_window_options(build_gallery_app(), "Picus Gallery", |options| {
        options.with_initial_inner_size(LogicalSize::new(1360.0, 760.0))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_gallery_theme_ron_parses() {
        picus_core::parse_stylesheet_ron(include_str!("../assets/themes/gallery.ron"))
            .expect("embedded gallery stylesheet should parse");
    }

    #[test]
    fn gallery_pages_match_mewui_gallery_sections() {
        let labels = GalleryPage::ALL.map(GalleryPage::label);
        assert_eq!(
            labels,
            [
                "Buttons",
                "Inputs",
                "Selection",
                "Window/Menu",
                "MessageBox",
                "Lists",
                "GridView",
                "Panels",
                "Layout",
                "Typography",
                "Media",
                "Shapes",
                "Icons",
                "Transitions",
                "Overlay",
            ],
        );
    }
}
