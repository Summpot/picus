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
//! - [`state`] — `GalleryPage` enum, `GalleryState`/`GalleryRuntime` resources, `NavCategory`
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
//! | Sidebar nav categories | Fluent UI Storybook sidebar groups |
//! | Top search bar         | Storybook search                   |
//! | Status bar events      | Storybook action logger            |
//! | `gallery.ron` theme    | Fluent UI `makeStyles` tokens      |

use picus_core::{
    AppPicusExt, InlineStyle, LayoutStyle, NavigationViewItem, PicusPlugin, UiAvatar, UiBadge,
    UiFlexColumn, UiFlexRow, UiLabel, UiNavigationView, UiRoot, UiScrollView, UiSearch,
    UiThemePicker, avatar_sizes,
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
use helpers::{PAGE_CONTENT, PAGE_VIEWPORT, class};
use state::{GalleryPage, GalleryRuntime, GalleryState};
use views::{GalleryRoot, GalleryStatus};

/// Build the full gallery application tree.
///
/// Creates the top bar, sidebar navigation, page content area, and spawns
/// all 15 component showcase pages.
fn setup_gallery(mut commands: Commands) {
    let root = commands
        .spawn((UiRoot, GalleryRoot, class("gallery.root")))
        .id();

    spawn_top_bar(&mut commands, root);

    commands.spawn((GalleryStatus, class("gallery.status"), ChildOf(root)));

    // --- Body: UiNavigationView handles sidebar + content area layout ---
    let body = commands
        .spawn((
            UiFlexColumn,
            class("gallery.body"),
            InlineStyle {
                layout: LayoutStyle {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
            ChildOf(root),
        ))
        .id();

    // Build navigation items from all gallery pages (with Lucide icon glyphs)
    let nav_items: Vec<NavigationViewItem> = GalleryPage::ALL
        .iter()
        .map(|page| {
            NavigationViewItem::new(page.label())
                .with_icon(page.icon().chars().next().unwrap_or('?'))
        })
        .collect();

    let nav_view = commands
        .spawn((
            UiNavigationView::new(nav_items),
            class("gallery.nav_view"),
            InlineStyle {
                layout: LayoutStyle {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
            ChildOf(body),
        ))
        .id();

    // Spawn all pages as children of the navigation view
    let open_dialog_btn = spawn_page(
        &mut commands,
        nav_view,
        GalleryPage::Buttons,
        pages::buttons::spawn_buttons_page,
    );
    let runtime_refs = GalleryRuntime {
        nav_view,
        search_input: Entity::PLACEHOLDER,
        open_dialog_btn,
        persistent_toast_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::Inputs,
            pages::inputs::spawn_inputs_page,
        ),
        success_toast_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::Selection,
            pages::selection::spawn_selection_page,
        ),
        warning_toast_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::WindowMenu,
            pages::window_menu::spawn_window_menu_page,
        ),
        error_toast_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::MessageBox,
            pages::message_box::spawn_message_box_page,
        ),
        prompt_dialog_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::Lists,
            pages::lists::spawn_lists_page,
        ),
        native_message_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::GridView,
            pages::grid_view::spawn_grid_view_page,
        ),
        popover_dialog_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::Panels,
            pages::panels::spawn_panels_page,
        ),
        burst_placeholder_btn: spawn_page(
            &mut commands,
            nav_view,
            GalleryPage::Layout,
            pages::layout::spawn_layout_page,
        ),
    };

    spawn_page(
        &mut commands,
        nav_view,
        GalleryPage::Typography,
        pages::typography::spawn_typography_page,
    );
    spawn_page(
        &mut commands,
        nav_view,
        GalleryPage::Media,
        pages::media::spawn_media_page,
    );
    spawn_page(
        &mut commands,
        nav_view,
        GalleryPage::Shapes,
        pages::shapes::spawn_shapes_page,
    );
    spawn_page(
        &mut commands,
        nav_view,
        GalleryPage::Icons,
        pages::icons::spawn_icons_page,
    );
    spawn_page(
        &mut commands,
        nav_view,
        GalleryPage::Transitions,
        pages::transitions::spawn_transitions_page,
    );
    spawn_page(
        &mut commands,
        nav_view,
        GalleryPage::Overlay,
        pages::overlay::spawn_overlay_page,
    );

    commands.insert_resource(runtime_refs);
}

/// Create the top bar with branding, search, theme picker, and badge.
fn spawn_top_bar(commands: &mut Commands, root: Entity) {
    let top = commands
        .spawn((UiFlexRow, class("gallery.top_bar"), ChildOf(root)))
        .id();

    // Brand area: avatar + title
    let brand = commands
        .spawn((UiFlexRow, class("gallery.brand"), ChildOf(top)))
        .id();
    commands.spawn((
        UiAvatar::new("P").with_size(avatar_sizes::MD),
        ChildOf(brand),
    ));
    let title_col = commands
        .spawn((UiFlexColumn, class("gallery.brand"), ChildOf(brand)))
        .id();
    commands.spawn((
        UiLabel::new("Picus Gallery"),
        class("gallery.title"),
        ChildOf(title_col),
    ));
    commands.spawn((
        UiLabel::new("Component showcase"),
        class("gallery.subtitle"),
        ChildOf(title_col),
    ));

    // Search bar (middle area)
    let search_row = commands
        .spawn((UiFlexRow, class("gallery.search_row"), ChildOf(top)))
        .id();
    commands.spawn((
        UiSearch::new("Find a component\u{2026}"),
        class("gallery.search"),
        ChildOf(search_row),
    ));

    // Tools: theme picker + badge
    let tools = commands
        .spawn((UiFlexRow, class("gallery.tools"), ChildOf(top)))
        .id();
    commands.spawn((UiThemePicker::fluent(), ChildOf(tools)));
    commands.spawn((UiBadge::new("FBA parity"), ChildOf(tools)));
}

/// Spawn a single gallery page inside the navigation view.
fn spawn_page(
    commands: &mut Commands,
    nav_view: Entity,
    page: GalleryPage,
    build: fn(&mut Commands, Entity) -> Entity,
) -> Entity {
    let scroll = commands
        .spawn((
            UiScrollView::new(PAGE_VIEWPORT, PAGE_CONTENT)
                .with_vertical_scrollbar(true)
                .with_horizontal_scrollbar(false),
            class("gallery.content_scroll"),
            ChildOf(nav_view),
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
    commands.spawn((
        UiLabel::new(page.description()),
        class("gallery.page_description"),
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
    use picus_core::bevy_window::{PrimaryWindow, Window, WindowResized};

    #[test]
    fn embedded_gallery_theme_ron_parses() {
        picus_core::parse_stylesheet_ron(include_str!("../assets/themes/gallery.ron"))
            .expect("embedded gallery stylesheet should parse");
    }

    #[test]
    fn gallery_pages_match_fluent_ui_sections() {
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

    #[test]
    fn gallery_categories_cover_all_pages() {
        let total: usize = GalleryPage::CATEGORIES.iter().map(|c| c.page_count).sum();
        assert_eq!(total, GalleryPage::ALL.len());
    }

    #[test]
    fn gallery_navigation_view_tracks_invisible_window_resize() {
        let mut app = build_gallery_app();

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(900.0, 320.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        app.update();

        let nav = app.world().resource::<GalleryRuntime>().nav_view;
        let body = app
            .world()
            .get::<ChildOf>(nav)
            .expect("nav should have a body parent")
            .parent();
        assert_eq!(
            picus_core::resolve_style(app.world(), body)
                .layout
                .flex_grow,
            1.0
        );
        assert_eq!(
            picus_core::resolve_style(app.world(), nav).layout.flex_grow,
            1.0
        );

        resize_primary_window(&mut app, window_entity, 900.0, 320.0);
        assert_eq!(
            app.world()
                .non_send::<picus_core::MasonryRuntime>()
                .viewport_size(),
            (900.0, 320.0)
        );
        let short_height = widget_height_for_entity(&mut app, nav);

        resize_primary_window(&mut app, window_entity, 900.0, 640.0);
        assert_eq!(
            app.world()
                .non_send::<picus_core::MasonryRuntime>()
                .viewport_size(),
            (900.0, 640.0)
        );
        let tall_height = widget_height_for_entity(&mut app, nav);

        assert!(
            !app.world()
                .get::<Window>(window_entity)
                .expect("primary window should exist")
                .visible
        );
        assert!(
            (tall_height - short_height - 320.0).abs() <= 2.0,
            "nav height should grow with the invisible window resize; short={short_height}, tall={tall_height}"
        );
    }

    fn resize_primary_window(app: &mut App, window_entity: Entity, width: f32, height: f32) {
        {
            let mut window = app
                .world_mut()
                .get_mut::<Window>(window_entity)
                .expect("window should exist");
            window.resolution.set(width, height);
        }

        app.world_mut().write_message(WindowResized {
            window: window_entity,
            width: 1.0,
            height: 1.0,
        });

        app.update();
    }

    fn widget_height_for_entity(app: &mut App, entity: Entity) -> f64 {
        let mut runtime = app.world_mut().non_send_mut::<picus_core::MasonryRuntime>();
        let _ = runtime.render_root.redraw();
        let widget_id = runtime
            .find_widget_id_for_entity_bits(entity.to_bits(), false)
            .expect("entity should resolve to a Masonry widget");
        runtime
            .render_root
            .get_widget(widget_id)
            .expect("widget id should resolve in render tree")
            .ctx()
            .border_box().size()
            .height
    }
}
