//! Picus Gallery — WinUI Gallery-style component showcase.
//!
//! This example demonstrates Picus UI components in a navigable gallery.
//! Each sidebar item opens a single-component page with multiple example
//! cards for that control's variants — the same structure as
//! [WinUI Gallery](https://github.com/microsoft/WinUI-Gallery).
//!
//! ## Architecture
//!
//! - [`helpers`] — Shared utilities (card, grid, note, placeholder, canvas/image helpers)
//! - [`state`] — `GalleryPage` enum, `GalleryRuntime` resource, and `NavCategory`
//! - [`views`] — shell components and projectors for `GalleryRoot` and `GalleryTopBar`
//! - [`events`] — Event dispatch for all component interactions
//! - [`pages`] — one showcase page per component, grouped into category modules

use picus::prelude::*;
use picus::{
    app::{
        bevy_app::{App, Startup, Update},
        bevy_ecs::{hierarchy::ChildOf, prelude::*, schedule::IntoScheduleConfigs},
    },
    projection::xilem::winit::{dpi::LogicalSize, error::EventLoopError},
};
use shared_utils::init_logging;

mod events;
mod helpers;
mod pages;
mod state;
mod views;

use events::{PendingGalleryActions, apply_gallery_actions, collect_gallery_actions};
use helpers::{PAGE_CONTENT, PAGE_VIEWPORT, class};
use state::{GalleryPage, GalleryRuntime};
use views::{GalleryRoot, GalleryTopBar};

/// Build the full gallery application tree.
///
/// Creates the top bar, sidebar navigation, page content area, and spawns
/// one content page per [`GalleryPage`] control.
fn setup_gallery(mut commands: Commands) {
    let root = commands
        .spawn_scene(bsn! {
            UiRoot
            GalleryRoot
            template_value(class("gallery.root"))
        })
        .id();

    spawn_top_bar(&mut commands, root);

    // --- Body: UiNavigationView handles sidebar + content area layout ---
    let body = commands
        .spawn_scene(bsn! {
            UiFlexColumn
            template_value(class("gallery.body"))
            template_value(InlineStyle {
                layout: LayoutStyle {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            ChildOf(root)
        })
        .id();

    // WinUI Gallery-style hierarchical MenuItems: category parents with leaf pages.
    // Content children stay in GalleryPage::ALL leaf order so selected leaf index maps 1:1.
    let nav_items = build_gallery_nav_items();

    let nav_view = commands
        .spawn_scene(bsn! {
            template_value(
                UiNavigationView::new(nav_items)
                    .with_settings_visible(true)
                    .with_pane_title("Gallery")
                    .with_settings_label("Settings"),
            )
            template_value(class("gallery.nav_view"))
            template_value(InlineStyle {
                layout: LayoutStyle {
                    flex_grow: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            ChildOf(body)
        })
        .id();

    // Spawn all leaf pages as children of the navigation view (order matches GalleryPage::ALL).
    for page in GalleryPage::ALL {
        spawn_page(&mut commands, nav_view, page);
    }

    commands.insert_resource(GalleryRuntime {
        nav_view,
        search_input: Entity::PLACEHOLDER,
    });
}

/// Build hierarchical nav items: one expandable category per [`GalleryPage::CATEGORIES`],
/// with leaf control pages as nested MenuItems (WinUI `NavigationViewItem.MenuItems`).
fn build_gallery_nav_items() -> Vec<NavigationViewItem> {
    GalleryPage::CATEGORIES
        .iter()
        .enumerate()
        .map(|(category_index, category)| {
            let children = GalleryPage::ALL
                [category.first_page_index..category.first_page_index + category.page_count]
                .iter()
                .map(|page| NavigationViewItem::new(page.label()).with_icon(page.icon()))
                .collect::<Vec<_>>();
            // Expand only the category that owns the default selection. Fully
            // expanding every category mounted ~40 nav leaves into the retained
            // tree on every frame.
            let item = NavigationViewItem::new(category.label).with_children(children);
            if category_index == 0 {
                item.expanded()
            } else {
                item
            }
        })
        .collect()
}

/// Create the top bar with branding, search, theme picker, and badge.
fn spawn_top_bar(commands: &mut Commands, root: Entity) {
    commands.spawn_scene(bsn! {
        GalleryTopBar
        template_value(class("gallery.top_bar"))
        ChildOf(root)
        Children [
            (
                UiFlexRow
                template_value(class("gallery.brand"))
                Children [
                    template_value(UiAvatar::new("P").with_size(avatar_sizes::MD)),
                    (
                        UiFlexColumn
                        template_value(class("gallery.brand"))
                        Children [
                            (
                                template_value(UiLabel::new("Picus Gallery"))
                                template_value(class("gallery.title"))
                            ),
                            (
                                template_value(UiLabel::new("Component showcase"))
                                template_value(class("gallery.subtitle"))
                            ),
                        ]
                    ),
                ]
            ),
            (
                template_value(UiSearch::new("Find a component\u{2026}"))
                template_value(class("gallery.search"))
            ),
            UiThemePicker,
        ]
    });
}

/// Spawn a single gallery page inside the navigation view.
fn spawn_page(commands: &mut Commands, nav_view: Entity, page: GalleryPage) {
    let scroll = commands
        .spawn_scene(bsn! {
            template_value(
                UiScrollView::new(PAGE_VIEWPORT, PAGE_CONTENT)
                    .with_vertical_scrollbar(true)
                    .with_horizontal_scrollbar(false)
            )
            template_value(class("gallery.content_scroll"))
            ChildOf(nav_view)
        })
        .id();
    let page_col = commands
        .spawn_scene(bsn! {
            UiFlexColumn
            template_value(class("gallery.page"))
            ChildOf(scroll)
            Children [
                (
                    template_value(UiLabel::new(page.label()))
                    template_value(class("gallery.section_title"))
                ),
                (
                    template_value(UiLabel::new(page.description()))
                    template_value(class("gallery.page_description"))
                ),
            ]
        })
        .id();
    pages::spawn_page_content(commands, page_col, page);
}

/// Build the Bevy application with all gallery systems and resources.
fn build_gallery_app() -> App {
    init_logging();

    let mut app = App::new();
    app.add_plugins(PicusPlugin)
        .load_style_sheet_ron(include_str!("../assets/themes/gallery.ron"))
        .insert_resource(AppI18n::new("en-US".parse().unwrap()))
        .register_i18n_bundle(
            "en-US",
            SyncTextSource::String(include_str!("../assets/locales/en-US/main.ftl")),
            vec!["Inter", "sans-serif"],
        )
        .register_i18n_bundle(
            "zh-CN",
            SyncTextSource::String(include_str!("../assets/locales/zh-CN/main.ftl")),
            vec!["Inter", "sans-serif"],
        )
        .register_i18n_bundle(
            "ja-JP",
            SyncTextSource::String(include_str!("../assets/locales/ja-JP/main.ftl")),
            vec!["Inter", "sans-serif"],
        )
        .init_resource::<PendingGalleryActions>()
        .add_systems(Startup, setup_gallery)
        .add_systems(
            Update,
            (collect_gallery_actions, apply_gallery_actions).chain(),
        );
    register_ui_components!(&mut app, GalleryRoot, GalleryTopBar);

    set_theme_backdrop_material(app.world_mut(), WindowBackdropMaterial::Mica);

    app
}

/// Application entry point.
///
/// Creates a 1360×760 window with the WinUI Gallery-style Picus Gallery.
fn main() -> Result<(), EventLoopError> {
    build_gallery_app().run_picus(
        "Picus Gallery",
        BevyWindowOptions::default().with_initial_inner_size(LogicalSize::new(1360.0, 760.0)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::picus::app::bevy_window::{PrimaryWindow, Window, WindowResized};
    use ::picus::prelude as picus;

    #[test]
    fn embedded_gallery_theme_ron_parses() {
        let sheet = picus::parse_stylesheet_ron(include_str!("../assets/themes/gallery.ron"))
            .expect("embedded gallery stylesheet should parse");
        assert_eq!(sheet.default_variant.as_deref(), Some("dark"));
    }

    #[test]
    fn gallery_uses_theme_managed_mica_and_exposes_material_picker() {
        let mut app = build_gallery_app();
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();

        app.update();

        assert_eq!(
            picus::resolve_theme_backdrop_material(app.world().resource::<picus::StyleSheet>()),
            Some(picus::WindowBackdropMaterial::Mica)
        );
        assert_eq!(
            app.world().get::<picus::WindowBackdropMaterial>(window),
            Some(&picus::WindowBackdropMaterial::Mica)
        );
        assert_eq!(
            app.world().get::<picus::WindowBackdropColorScheme>(window),
            Some(&picus::WindowBackdropColorScheme::Dark)
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["gallery.content_scroll"])
                .colors
                .bg,
            Some(picus::xilem::Color::TRANSPARENT)
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["gallery.content_scroll"])
                .layout
                .border_width,
            0.0
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["template.scroll_view.viewport"],)
                .colors
                .bg,
            Some(picus::xilem::Color::TRANSPARENT)
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["template.scroll_view.viewport"],)
                .layout
                .border_width,
            0.0
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["nav.sidebar"])
                .colors
                .bg,
            Some(picus::xilem::Color::TRANSPARENT)
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["nav.sidebar"])
                .layout
                .border_width,
            0.0
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["gallery.top_bar"])
                .colors
                .bg,
            Some(picus::xilem::Color::TRANSPARENT)
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["gallery.top_bar"])
                .layout
                .border_width,
            0.0
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["gallery.search"])
                .colors
                .bg,
            Some(picus::xilem::Color::from_rgba8(255, 255, 255, 15))
        );
        let card_fill = Some(picus::xilem::Color::from_rgba8(255, 255, 255, 13));
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["gallery.card"])
                .colors
                .bg,
            card_fill
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["gallery.page"])
                .colors
                .bg,
            None
        );
        let nav_view = app.world().resource::<GalleryRuntime>().nav_view;
        assert_eq!(
            picus::resolve_style(app.world(), nav_view).colors.bg,
            Some(picus::xilem::Color::TRANSPARENT)
        );
        assert_eq!(
            picus::resolve_style_for_classes(app.world(), ["nav.content"])
                .colors
                .bg,
            Some(picus::xilem::Color::TRANSPARENT)
        );
        let scroll_entities = {
            let mut query = app
                .world_mut()
                .query_filtered::<Entity, With<UiScrollView>>();
            query.iter(app.world()).collect::<Vec<_>>()
        };
        assert!(!scroll_entities.is_empty());
        for scroll in scroll_entities {
            let style = picus::resolve_style(app.world(), scroll);
            assert_eq!(
                style.colors.bg,
                Some(picus::xilem::Color::TRANSPARENT),
                "gallery scroll shells must reveal the native backdrop"
            );
            assert_eq!(
                style.layout.border_width, 0.0,
                "gallery scroll shells must remain borderless"
            );
        }
        let mut picker_query = app
            .world_mut()
            .query_filtered::<&picus::UiRadioGroup, With<state::GalleryBackdropPicker>>();
        let picker = picker_query
            .iter(app.world())
            .next()
            .expect("gallery should expose a backdrop material picker");
        assert_eq!(picker.options, ["None", "Mica", "Acrylic"]);
        assert_eq!(picker.selected, 1);
        let has_status = {
            let mut query = app.world_mut().query::<&picus::StyleClass>();
            query
                .iter(app.world())
                .any(|classes| classes.0.iter().any(|class| class == "gallery.status"))
        };
        assert!(!has_status, "gallery status text should not be spawned");
    }

    #[test]
    fn gallery_theme_styles_navigation_view_sidebar() {
        let app = build_gallery_app();

        let sidebar = picus::resolve_style_for_classes(app.world(), ["nav.sidebar"]);
        let item = picus::resolve_style_for_classes(app.world(), ["nav.item"]);
        let item_hover = picus::resolve_style_for_classes_with_state(
            app.world(),
            ["nav.item"],
            picus::StylePseudoState::hovered(),
        );
        let active_item =
            picus::resolve_style_for_classes(app.world(), ["nav.item", "nav.item.active"]);
        let active_item_hover = picus::resolve_style_for_classes_with_state(
            app.world(),
            ["nav.item", "nav.item.active"],
            picus::StylePseudoState::hovered(),
        );

        assert!(
            sidebar.colors.bg.is_some() && sidebar.layout.border_width == 0.0,
            "gallery navigation sidebar should resolve a borderless backdrop fill, got {sidebar:?}"
        );
        assert!(
            item.colors.text.is_some() && item.layout.padding > 0.0,
            "gallery navigation items should resolve visible text and spacing, got {item:?}"
        );
        assert!(
            active_item.colors.bg.is_some() && active_item.colors.text.is_some(),
            "gallery active navigation item should resolve visible selected colors, got {active_item:?}"
        );
        assert!(
            item_hover.colors.bg.is_some() && item_hover.colors.bg != item.colors.bg,
            "gallery navigation item hover should resolve a distinct hover background, got base={item:?} hover={item_hover:?}"
        );
        assert!(
            active_item_hover.colors.bg.is_some()
                && active_item_hover.colors.bg != active_item.colors.bg,
            "gallery active navigation item hover should resolve a distinct hover background, got base={active_item:?} hover={active_item_hover:?}"
        );
    }

    #[test]
    fn gallery_pages_are_one_component_each() {
        let labels = GalleryPage::ALL.map(GalleryPage::label);
        assert_eq!(labels.len(), 41);
        assert_eq!(labels[0], "Button");
        assert_eq!(labels[1], "ToggleSwitch");
        assert_eq!(labels[2], "CheckBox");
        assert!(
            labels.contains(&"DataTable"),
            "expected a dedicated DataTable page"
        );
        assert!(
            labels.contains(&"Markdown"),
            "expected a dedicated Markdown page"
        );
        // No multi-component category labels from the old gallery.
        assert!(!labels.contains(&"Buttons"));
        assert!(!labels.contains(&"Inputs"));
        assert!(!labels.contains(&"Selection"));
        assert!(!labels.contains(&"Window/Menu"));
        assert!(!labels.contains(&"MessageBox"));
        assert!(!labels.contains(&"Lists"));
        assert!(!labels.contains(&"GridView"));
        assert!(!labels.contains(&"Panels"));
        assert!(!labels.contains(&"Layout"));
        assert!(!labels.contains(&"Media"));
        assert!(!labels.contains(&"Overlay"));
        assert!(!labels.contains(&"Transitions"));
    }

    #[test]
    fn gallery_categories_cover_all_pages() {
        let total: usize = GalleryPage::CATEGORIES.iter().map(|c| c.page_count).sum();
        assert_eq!(total, GalleryPage::ALL.len());
        // Categories should be contiguous and cover the full index range.
        let mut next = 0;
        for category in GalleryPage::CATEGORIES {
            assert_eq!(category.first_page_index, next);
            next += category.page_count;
        }
        assert_eq!(next, GalleryPage::ALL.len());
    }

    #[test]
    fn gallery_nav_items_are_hierarchical_categories() {
        let items = build_gallery_nav_items();
        assert_eq!(items.len(), GalleryPage::CATEGORIES.len());
        let leaf_count: usize = items.iter().map(|item| item.leaf_count()).sum();
        assert_eq!(leaf_count, GalleryPage::ALL.len());
        for (index, (item, category)) in items.iter().zip(GalleryPage::CATEGORIES.iter()).enumerate()
        {
            assert_eq!(item.label, category.label);
            assert!(
                !item.is_leaf(),
                "category parents must have MenuItems children"
            );
            // Only the category owning the default selection starts expanded so
            // the retained sidebar does not mount every control leaf at once.
            assert_eq!(
                item.is_expanded,
                index == 0,
                "only the first gallery category should start expanded"
            );
            assert_eq!(item.children.len(), category.page_count);
            assert!(
                item.children.iter().all(|child| child.is_leaf()),
                "control pages must be leaf MenuItems"
            );
        }
    }

    #[test]
    fn gallery_demo_buttons_carry_echo_actions() {
        let mut app = build_gallery_app();
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<&state::GalleryButtonAction>();
        let count = query.iter(world).count();
        assert!(
            count >= 15,
            "gallery should attach GalleryButtonAction markers to at least 15 demo buttons, got {count}"
        );
    }

    #[test]
    fn gallery_markdown_page_exposes_markdown_sample() {
        let mut app = build_gallery_app();
        app.update();

        let has_sample = {
            let mut query = app.world_mut().query::<&picus::UiMarkdown>();
            query
                .iter(app.world())
                .any(|markdown| markdown.source.contains("Fenced code"))
        };
        let markdown_style = picus::resolve_style_for_classes(app.world(), ["gallery.markdown"]);

        assert!(
            has_sample,
            "gallery should spawn the markdown typography sample"
        );
        assert!(
            markdown_style.colors.text.is_some(),
            "gallery markdown sample needs an explicit text color"
        );
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
            picus::resolve_style(app.world(), body).layout.flex_grow,
            1.0
        );
        assert_eq!(picus::resolve_style(app.world(), nav).layout.flex_grow, 1.0);

        resize_primary_window(&mut app, window_entity, 900.0, 320.0);
        assert_eq!(
            app.world()
                .non_send::<picus::MasonryRuntime>()
                .primary()
                .expect("primary window runtime should exist")
                .viewport_size(),
            (900.0, 320.0)
        );
        let short_height = widget_height_for_entity(&mut app, nav);

        resize_primary_window(&mut app, window_entity, 900.0, 640.0);
        assert_eq!(
            app.world()
                .non_send::<picus::MasonryRuntime>()
                .primary()
                .expect("primary window runtime should exist")
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

    #[test]
    fn gallery_top_bar_keeps_search_and_theme_picker_anchored_after_theme_switch() {
        let mut app = build_gallery_app();

        let mut window = Window {
            visible: false,
            ..Default::default()
        };
        window.resolution.set(1360.0, 760.0);
        app.world_mut().spawn((window, PrimaryWindow));

        app.update();

        let search = {
            let mut query = app.world_mut().query_filtered::<Entity, With<UiSearch>>();
            query
                .iter(app.world())
                .next()
                .expect("gallery should spawn a search box")
        };
        let theme_picker = {
            let mut query = app
                .world_mut()
                .query_filtered::<Entity, With<UiThemePicker>>();
            query
                .iter(app.world())
                .next()
                .expect("gallery should spawn a theme picker")
        };

        let before_search = widget_rect_for_entity(&mut app, search);
        let before_picker = widget_rect_for_entity(&mut app, theme_picker);

        picus::set_active_style_variant_by_name(app.world_mut(), "light");
        app.update();

        let after_search = widget_rect_for_entity(&mut app, search);
        let after_picker = widget_rect_for_entity(&mut app, theme_picker);

        assert!(
            after_search.width() >= 320.0,
            "search should keep a usable width after theme switch; before={before_search:?}, after={after_search:?}"
        );
        assert!(
            after_picker.x0 > after_search.x1,
            "theme picker should stay to the right of search; search={after_search:?}, picker={after_picker:?}"
        );
        assert!(
            (after_search.x0 - before_search.x0).abs() <= 4.0
                && (after_picker.x0 - before_picker.x0).abs() <= 4.0,
            "top bar controls should not jump after theme switch; search before={before_search:?} after={after_search:?}, picker before={before_picker:?} after={after_picker:?}"
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
        let mut runtime = app.world_mut().non_send_mut::<picus::MasonryRuntime>();
        let window_runtime = runtime
            .primary_mut()
            .expect("primary window runtime should exist");
        let _ = window_runtime.render_root.redraw();
        let widget_id = window_runtime
            .find_widget_id_for_entity_bits(entity.to_bits(), false)
            .expect("entity should resolve to a Masonry widget");
        window_runtime
            .render_root
            .get_widget(widget_id)
            .expect("widget id should resolve in render tree")
            .ctx()
            .border_box()
            .size()
            .height
    }

    fn widget_rect_for_entity(app: &mut App, entity: Entity) -> picus::masonry_core::kurbo::Rect {
        let mut runtime = app.world_mut().non_send_mut::<picus::MasonryRuntime>();
        let window_runtime = runtime
            .primary_mut()
            .expect("primary window runtime should exist");
        let _ = window_runtime.render_root.redraw();
        let widget_id = window_runtime
            .find_widget_id_for_entity_bits(entity.to_bits(), false)
            .expect("entity should resolve to a Masonry widget");
        window_runtime
            .render_root
            .get_widget(widget_id)
            .expect("widget id should resolve in render tree")
            .ctx()
            .bounding_box()
    }
}
