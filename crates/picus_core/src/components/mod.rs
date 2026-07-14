use std::{any::TypeId, collections::HashSet};

use bevy_app::App;
use bevy_ecs::prelude::*;

use crate::{AppPicusExt, ProjectionCtx, StyleTypeRegistry, UiProjectorRegistry, UiView};

mod avatar;
mod badge;
mod breadcrumb;
mod button;
mod canvas;
mod card;
mod checkbox;
mod color_picker;
mod combo_box;
mod context_menu;
mod data_table;
mod date_picker;
mod dialog;
mod divider;
mod expander;
mod grid;
mod group_box;
mod image;
mod link;
mod list_view;
mod markdown;
mod menu;
mod message_bar;
mod multiline_text_input;
pub(crate) mod navigation_view;
mod numeric_up_down;
mod password_input;
mod popover;
mod progress_bar;
mod radio_group;
mod rating;
mod scroll_view;
mod search;
mod slider;
mod spinner;
mod split_pane;
mod streaming_markdown;
mod switch;
mod tab_bar;
mod table;
mod text;
mod text_input;
mod theme_picker;
mod time_picker;
mod titlebar;
mod toast;
mod toolbar;
mod tooltip;
mod tree_node;

pub use avatar::*;
pub use badge::*;
pub use breadcrumb::*;
pub use button::*;
pub use canvas::*;
pub use card::*;
pub use checkbox::*;
pub use color_picker::*;
pub use combo_box::*;
pub use context_menu::*;
pub use data_table::*;
pub use date_picker::*;
pub use dialog::*;
pub use divider::*;
pub use expander::*;
pub use grid::*;
pub use group_box::*;
pub use image::*;
pub use link::*;
pub use list_view::*;
pub use markdown::*;
pub use menu::*;
pub use message_bar::*;
pub use multiline_text_input::*;
pub use navigation_view::*;
pub use numeric_up_down::*;
pub use password_input::*;
pub use popover::*;
pub use progress_bar::*;
pub use radio_group::*;
pub use rating::*;
pub use scroll_view::*;
pub use search::*;
pub use slider::*;
pub use spinner::*;
pub use split_pane::*;
pub use streaming_markdown::*;
pub use switch::*;
pub use tab_bar::*;
pub use table::*;
pub use text::*;
pub use text_input::*;
pub use theme_picker::*;
pub use time_picker::*;
pub use titlebar::*;
pub use toast::*;
pub use toolbar::*;
pub use tooltip::*;
pub use tree_node::*;

/// Unified contract for ECS-native UI components.
///
/// A UI component owns:
/// - one-time ECS expansion into template parts (`expand`),
/// - projection from ECS state into a retained Masonry view (`project`).
pub trait UiComponentTemplate: Component + Sized {
    /// Expand a newly-spawned logical UI component entity into child template parts.
    fn expand(_world: &mut World, _entity: Entity) {}

    /// Project this UI component into a Masonry view.
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView;

    /// Register non-component projection inputs used by this UI component.
    ///
    /// Implement this when `project` reads app resources through
    /// [`ProjectionCtx::world`]. Component changes are tracked automatically by
    /// `register_ui_component`; resource dependencies must be declared here so
    /// incremental synthesis can stay idle when nothing relevant changed.
    fn register_projection_dependencies(_registry: &mut UiProjectorRegistry) {}

    /// Register selector type aliases used by this UI component.
    fn register_style_types(registry: &mut StyleTypeRegistry) {
        registry.register_type_aliases::<Self>();
    }
}

/// Internal resource tracking which UI component types were already registered.
#[derive(Resource, Debug, Default)]
pub struct RegisteredUiComponentTypes {
    seen: HashSet<TypeId>,
}

impl RegisteredUiComponentTypes {
    pub fn insert<T: 'static>(&mut self) -> bool {
        self.seen.insert(TypeId::of::<T>())
    }
}

/// Generic expansion system for any [`UiComponentTemplate`].
///
/// Runs only for entities where the UI component was just added.
pub fn expand_added_ui_component_templates<T: UiComponentTemplate>(world: &mut World) {
    let entities = {
        let mut query = world.query_filtered::<Entity, Added<T>>();
        query.iter(world).collect::<Vec<_>>()
    };

    for entity in entities {
        if world.get_entity(entity).is_ok() {
            T::expand(world, entity);
        }
    }
}

/// Compatibility helper that expands all entities carrying `T`, not only `Added<T>`.
pub fn expand_all_ui_component_templates<T: UiComponentTemplate>(world: &mut World) {
    let entities = {
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).collect::<Vec<_>>()
    };

    for entity in entities {
        if world.get_entity(entity).is_ok() {
            T::expand(world, entity);
        }
    }
}

/// Register all built-in UI components with the unified UI component API.
pub fn register_builtin_ui_components(app: &mut App) {
    app.register_ui_component::<button::UiButton>()
        .register_ui_component::<avatar::UiAvatar>()
        .register_ui_component::<badge::UiBadge>()
        .register_ui_component::<breadcrumb::UiBreadcrumb>()
        .register_ui_component::<breadcrumb::UiBreadcrumbItem>()
        .register_ui_component::<canvas::UiCanvas>()
        .register_ui_component::<card::UiCard>()
        .register_ui_component::<checkbox::UiCheckbox>()
        .register_ui_component::<rating::UiRating>()
        .register_ui_component::<slider::UiSlider>()
        .register_ui_component::<switch::UiSwitch>()
        .register_ui_component::<text::UiText>()
        .register_ui_component::<text_input::UiTextInput>()
        .register_ui_component::<password_input::UiPasswordInput>()
        .register_ui_component::<multiline_text_input::UiMultilineTextInput>()
        .register_ui_component::<image::UiImage>()
        .register_ui_component::<link::UiLink>()
        .register_ui_component::<message_bar::UiMessageBar>()
        .register_ui_component::<progress_bar::UiProgressBar>()
        .register_ui_component::<dialog::UiDialog>()
        .register_ui_component::<divider::UiDivider>()
        .register_ui_component::<popover::UiPopover>()
        .register_ui_component::<combo_box::UiComboBox>()
        .register_ui_component::<combo_box::UiDropdownMenu>()
        .register_ui_component::<combo_box::UiDropdownItem>()
        .register_ui_component::<radio_group::UiRadioGroup>()
        .register_ui_component::<scroll_view::UiScrollView>()
        .register_ui_component::<search::UiSearch>()
        .register_ui_component::<grid::UiGrid>()
        .register_ui_component::<tab_bar::UiTabBar>()
        .register_ui_component::<list_view::UiListView>()
        .register_ui_component::<markdown::UiMarkdown>()
        .register_ui_component::<tree_node::UiTreeNode>()
        .register_ui_component::<split_pane::UiSplitPane>()
        .register_ui_component::<group_box::UiGroupBox>()
        .register_ui_component::<spinner::UiSpinner>()
        .register_ui_component::<streaming_markdown::UiStreamingMarkdown>()
        .register_ui_component::<table::UiTable>()
        .register_ui_component::<data_table::UiDataTable>()
        .register_ui_component::<menu::UiMenuBar>()
        .register_ui_component::<menu::UiMenuBarItem>()
        .register_ui_component::<menu::UiMenuItemPanel>()
        .register_ui_component::<tooltip::UiTooltip>()
        .register_ui_component::<toast::UiToast>()
        .register_ui_component::<color_picker::UiColorPicker>()
        .register_ui_component::<color_picker::UiColorPickerPanel>()
        .register_ui_component::<date_picker::UiDatePicker>()
        .register_ui_component::<date_picker::UiDatePickerPanel>()
        .register_ui_component::<time_picker::UiTimePicker>()
        .register_ui_component::<time_picker::UiTimePickerPanel>()
        .register_ui_component::<expander::UiExpander>()
        .register_ui_component::<context_menu::UiContextMenu>()
        .register_ui_component::<theme_picker::UiThemePicker>()
        .register_ui_component::<theme_picker::UiThemePickerMenu>()
        .register_ui_component::<navigation_view::UiNavigationView>()
        .register_ui_component::<navigation_view::UiNavigationItem>()
        .register_ui_component::<titlebar::UiTitleBar>()
        .register_ui_component::<numeric_up_down::UiNumericUpDown>()
        .register_ui_component::<toolbar::UiToolbar>();

    app.add_systems(
        bevy_app::Update,
        (
            navigation_view::sync_navigation_view_item_templates,
            navigation_view::update_navigation_view_display_mode,
        ),
    );
}
