pub mod core;
pub mod dialog;
pub mod dropdown;
pub mod elements;
pub mod layout;
pub mod markdown;
pub mod overlay;
pub mod popover;
pub mod theme_picker;
pub mod utils;
pub mod widgets;

pub use core::*;
pub use markdown::StreamingMarkdownParseCache;

use crate::ecs::{
    UiBadge, UiButton, UiCanvas, UiCheckbox, UiColorPicker, UiColorPickerPanel, UiComboBox,
    UiContextMenu, UiDataTable, UiDatePicker, UiDatePickerPanel, UiDialog, UiDropdownMenu,
    UiExpander, UiFlexColumn, UiFlexRow, UiGrid, UiGroupBox, UiImage, UiLabel, UiListView,
    UiMenuBar, UiMenuBarItem, UiMenuItemPanel, UiMultilineTextInput, UiNavigationView,
    UiOverlayRoot, UiPasswordInput, UiPopover, UiProgressBar, UiRadioGroup, UiResponsiveGrid,
    UiResponsiveRow, UiRoot, UiScrollView, UiSlider, UiSpinner, UiSplitPane, UiSwitch, UiTabBar,
    UiTable, UiTextInput, UiThemePicker, UiThemePickerMenu, UiTimePicker, UiTimePickerPanel,
    UiToast, UiTooltip, UiTreeNode, UiVisibleResponsive,
};

/// Register non-UI-component foundational projectors.
pub fn register_core_projectors(registry: &mut UiProjectorRegistry) {
    registry
        .register_component::<UiRoot>(layout::project_ui_root)
        .register_component::<UiFlexColumn>(layout::project_flex_column)
        .register_component::<UiFlexRow>(layout::project_flex_row)
        .register_component::<UiGrid>(layout::project_grid)
        .register_component::<UiResponsiveRow>(layout::project_responsive_row)
        .register_component::<UiVisibleResponsive>(layout::project_visible_responsive)
        .register_component::<UiResponsiveGrid>(layout::project_responsive_grid)
        .register_component::<UiLabel>(elements::project_label)
        .register_component::<UiOverlayRoot>(overlay::project_overlay_root);
}

/// Register built-in projectors for built-in ECS demo components.
///
/// Compatibility helper: UI components are now expected to be registered through
/// `AppPicusExt::register_ui_component::<T>()`.
pub fn register_builtin_projectors(registry: &mut UiProjectorRegistry) {
    register_core_projectors(registry);

    registry
        .register_component::<UiButton>(elements::project_button)
        .register_component::<UiBadge>(elements::project_badge)
        .register_component::<UiCheckbox>(elements::project_checkbox)
        .register_component::<UiSlider>(elements::project_slider)
        .register_component::<UiSwitch>(elements::project_switch)
        .register_component::<UiTextInput>(elements::project_text_input)
        .register_component::<UiPasswordInput>(elements::project_password_input)
        .register_component::<UiMultilineTextInput>(elements::project_multiline_text_input)
        .register_component::<UiImage>(elements::project_image)
        .register_component::<UiProgressBar>(elements::project_progress_bar)
        .register_component::<UiDialog>(dialog::project_dialog)
        .register_component::<UiPopover>(popover::project_popover)
        .register_component::<UiComboBox>(dropdown::project_combo_box)
        .register_component::<UiDropdownMenu>(dropdown::project_dropdown_menu)
        .register_component::<UiRadioGroup>(widgets::project_radio_group)
        .register_component::<UiScrollView>(widgets::project_scroll_view)
        .register_component::<UiCanvas>(widgets::project_canvas)
        .register_component::<UiTabBar>(widgets::project_tab_bar)
        .register_component::<UiListView>(widgets::project_list_view)
        .register_component::<UiTreeNode>(widgets::project_tree_node)
        .register_component::<UiTable>(widgets::project_table)
        .register_component::<UiDataTable>(widgets::project_data_table)
        .register_component::<UiMenuBar>(widgets::project_menu_bar)
        .register_component::<UiMenuBarItem>(widgets::project_menu_bar_item)
        .register_component::<UiMenuItemPanel>(widgets::project_menu_item_panel)
        .register_component::<UiTooltip>(widgets::project_tooltip)
        .register_component::<UiSpinner>(widgets::project_spinner)
        .register_component::<UiColorPicker>(widgets::project_color_picker)
        .register_component::<UiColorPickerPanel>(widgets::project_color_picker_panel)
        .register_component::<UiGroupBox>(widgets::project_group_box)
        .register_component::<UiSplitPane>(widgets::project_split_pane)
        .register_component::<UiToast>(widgets::project_toast)
        .register_component::<UiDatePicker>(widgets::project_date_picker)
        .register_component::<UiDatePickerPanel>(widgets::project_date_picker_panel)
        .register_component::<UiTimePicker>(widgets::project_time_picker)
        .register_component::<UiTimePickerPanel>(widgets::project_time_picker_panel)
        .register_component::<UiExpander>(widgets::project_expander)
        .register_component::<UiContextMenu>(widgets::project_context_menu)
        .register_component::<UiThemePicker>(theme_picker::project_theme_picker)
        .register_component::<UiThemePickerMenu>(theme_picker::project_theme_picker_menu)
        .register_component::<UiNavigationView>(widgets::project_navigation_view);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{UiEventQueue, bubble_ui_pointer_events};
    use bevy_ecs::hierarchy::ChildOf;
    use bevy_ecs::prelude::*;
    use bevy_input::mouse::MouseButton;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn pointer_hits_bubble_to_parent_until_consumed() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let root = world.spawn_empty().id();
        let parent = world
            .spawn((ChildOf(root), crate::StopUiPointerPropagation))
            .id();
        let child = world.spawn((ChildOf(parent),)).id();

        world.resource::<UiEventQueue>().push_typed(
            child,
            crate::UiPointerHitEvent {
                target: child,
                position: (12.0, 24.0),
                button: MouseButton::Left,
                phase: crate::UiPointerPhase::Pressed,
            },
        );

        bubble_ui_pointer_events(&mut world);

        let bubbled = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiPointerEvent>();

        assert_eq!(bubbled.len(), 2);
        assert_eq!(bubbled[0].entity, child);
        assert_eq!(bubbled[0].action.current_target, child);
        assert!(!bubbled[0].action.consumed);

        assert_eq!(bubbled[1].entity, parent);
        assert_eq!(bubbled[1].action.current_target, parent);
        assert!(bubbled[1].action.consumed);
    }

    #[test]
    fn projector_registry_last_registered_component_projector_wins() {
        #[derive(Component, Debug, Clone, Copy)]
        struct OverrideProbe;

        static LAST_PROJECTOR: AtomicUsize = AtomicUsize::new(0);

        fn project_first(_: &OverrideProbe, _ctx: ProjectionCtx<'_>) -> UiView {
            LAST_PROJECTOR.store(1, Ordering::SeqCst);
            Arc::new(crate::xilem::view::label("first"))
        }

        fn project_second(_: &OverrideProbe, _ctx: ProjectionCtx<'_>) -> UiView {
            LAST_PROJECTOR.store(2, Ordering::SeqCst);
            Arc::new(crate::xilem::view::label("second"))
        }

        let mut world = World::new();
        let entity = world.spawn((OverrideProbe,)).id();

        let mut registry = UiProjectorRegistry::default();
        registry
            .register_component::<OverrideProbe>(project_first)
            .register_component::<OverrideProbe>(project_second);

        LAST_PROJECTOR.store(0, Ordering::SeqCst);
        let projected = registry.project_node(&world, entity, entity.to_bits(), Vec::new());
        assert!(projected.is_some());
        assert_eq!(LAST_PROJECTOR.load(Ordering::SeqCst), 2);
    }
}
