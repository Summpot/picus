use crate::accelerator::{CurrentAcceleratorModifiers, process_keyboard_accelerators};
use crate::accessibility::{
    AccessibilityTree, handle_accessibility_actions, sync_accessibility_tree,
};
use crate::bevy_tween::{
    BevyTweenRegisterSystems, DefaultTweenPlugins, TweenCorePlugin, TweenSystemSet,
    component_tween_system,
};
use crate::clipboard::{Clipboard, handle_clipboard_events};
use crate::composition::{CompositionState, apply_composition_effects, sync_composition_visuals};
use crate::drag_drop::{DragState, dispatch_drag_events, track_drag_state};
use crate::titlebar_system::{handle_titlebar_actions, sync_titlebar_state};
use crate::validation::{ValidationRegistry, run_validation};
use bevy_app::{App, Last, Plugin, PostUpdate, PreUpdate, TaskPoolPlugin, Update};
use bevy_asset::{AssetApp, AssetEvent, AssetPlugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_input::keyboard::KeyboardInput;
use bevy_input::mouse::{MouseButtonInput, MouseWheel};
use bevy_scene::ScenePlugin;
use bevy_text::Font;
use bevy_time::TimePlugin;
use bevy_window::{
    CursorLeft, CursorMoved, Ime, RequestRedraw, WindowFocused, WindowResized,
    WindowScaleFactorChanged,
};

use crate::{
    AppBreakpoints, AppPicusExt, OverlayStack, WindowSize,
    components::register_builtin_ui_components,
    events::UiEventQueue,
    fonts::{XilemFontBridge, collect_bevy_font_assets, sync_fonts_to_xilem},
    i18n::AppI18n,
    overlay::{
        OverlayPointerRoutingState, bubble_ui_pointer_events, ensure_overlay_defaults,
        ensure_overlay_root, handle_context_menu_right_clicks, handle_global_overlay_clicks,
        handle_overlay_actions, reparent_overlay_entities, sync_overlay_positions,
        sync_overlay_stack_lifecycle,
    },
    projection::markdown::{
        StreamingMarkdownParseCache, evict_streaming_markdown_cache,
        update_streaming_markdown_cache,
    },
    projection::{UiProjectorRegistry, register_core_projectors},
    runtime::{
        MasonryRuntime, initialize_masonry_runtime_from_windows, inject_bevy_input_into_masonry,
        paint_masonry_ui, rebuild_masonry_runtime, route_masonry_view_messages,
        sync_masonry_ime_state_to_bevy_window, sync_masonry_window_lifecycle,
    },
    styling::{
        ActiveStyleSheet, ActiveStyleSheetAsset, ActiveStyleSheetSelectors,
        ActiveStyleSheetTokenNames, ActiveStyleVariant, AppliedStyleVariant, BaseStyleSheet,
        ReducedMotion, RegisteredStyleVariants, StyleAssetEventCursor, StyleSheet,
        StyleSheetRonLoader, activate_debounced_hovers, animate_style_transitions,
        ensure_active_stylesheet_asset_handle, mark_style_dirty,
        register_builtin_style_type_aliases, register_embedded_fluent_theme_variants,
        sync_active_style_variant, sync_style_targets, sync_stylesheet_asset_events,
        sync_ui_interaction_markers,
    },
    synthesize::{SynthesizedUiViews, UiSynthesisStats, sync_focus_state, synthesize_ui},
    track_window_size,
    widget_actions::{
        handle_scroll_view_wheel, handle_tooltip_hovers, handle_widget_actions,
        sync_scroll_view_layout_geometry, tick_auto_dismiss,
    },
};

/// Bevy plugin for headless Masonry runtime + ECS projection synthesis.
#[derive(Default)]
pub struct PicusPlugin;

/// Registers all built-in ECS UI components.
///
/// This plugin is automatically added by [`PicusPlugin`], so users get
/// plug-and-play built-ins without manual registration in app setup code.
#[derive(Default)]
pub struct PicusBuiltinsPlugin;

impl Plugin for PicusBuiltinsPlugin {
    fn build(&self, app: &mut App) {
        register_builtin_ui_components(app);
    }
}

impl Plugin for PicusPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TaskPoolPlugin>() {
            app.add_plugins(TaskPoolPlugin::default());
        }
        if !app.is_plugin_added::<AssetPlugin>() {
            app.add_plugins(AssetPlugin::default());
        }
        if !app.is_plugin_added::<ScenePlugin>() {
            app.add_plugins(ScenePlugin);
        }
        if !app.is_plugin_added::<TweenCorePlugin<()>>() {
            app.add_plugins(DefaultTweenPlugins::<()>::in_schedule(Update));
        }

        app.add_plugins((TimePlugin, PicusBuiltinsPlugin))
            .add_tween_systems(
                Update,
                component_tween_system::<crate::styling::ColorStyleLens>(),
            )
            .register_xilem_font_bytes(crate::icons::LUCIDE_FONT_BYTES)
            .init_asset::<StyleSheet>()
            .init_asset_loader::<StyleSheetRonLoader>()
            .init_resource::<UiProjectorRegistry>()
            .init_resource::<SynthesizedUiViews>()
            .init_resource::<UiSynthesisStats>()
            .init_resource::<UiEventQueue>()
            .init_resource::<StyleSheet>()
            .init_resource::<BaseStyleSheet>()
            .init_resource::<ActiveStyleSheet>()
            .init_resource::<ActiveStyleSheetAsset>()
            .init_resource::<ActiveStyleSheetSelectors>()
            .init_resource::<ActiveStyleSheetTokenNames>()
            .init_resource::<ActiveStyleVariant>()
            .init_resource::<AppliedStyleVariant>()
            .init_resource::<RegisteredStyleVariants>()
            .init_resource::<StyleAssetEventCursor>()
            .init_resource::<XilemFontBridge>()
            .init_resource::<AppI18n>()
            .init_resource::<AppBreakpoints>()
            .init_resource::<WindowSize>()
            .init_resource::<OverlayStack>()
            .init_resource::<OverlayPointerRoutingState>()
            .init_resource::<ReducedMotion>()
            .init_resource::<Clipboard>()
            .init_resource::<CurrentAcceleratorModifiers>()
            .init_resource::<AccessibilityTree>()
            .init_resource::<CompositionState>()
            .init_resource::<DragState>()
            .init_resource::<ValidationRegistry>()
            .init_resource::<StreamingMarkdownParseCache>()
            .init_non_send::<MasonryRuntime>()
            .add_message::<CursorMoved>()
            .add_message::<CursorLeft>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseWheel>()
            .add_message::<Ime>()
            .add_message::<RequestRedraw>()
            .add_message::<WindowFocused>()
            .add_message::<WindowResized>()
            .add_message::<WindowScaleFactorChanged>()
            .add_message::<AssetEvent<Font>>()
            .add_systems(
                PreUpdate,
                (
                    track_window_size,
                    collect_bevy_font_assets,
                    initialize_masonry_runtime_from_windows,
                    sync_fonts_to_xilem,
                    sync_masonry_window_lifecycle,
                    track_drag_state,
                    dispatch_drag_events,
                    bubble_ui_pointer_events,
                    handle_global_overlay_clicks,
                    handle_context_menu_right_clicks,
                    sync_scroll_view_layout_geometry,
                    handle_scroll_view_wheel,
                    handle_clipboard_events,
                    inject_bevy_input_into_masonry,
                    route_masonry_view_messages,
                    sync_masonry_ime_state_to_bevy_window,
                    handle_widget_actions,
                    sync_ui_interaction_markers,
                )
                    .chain(),
            )
            .add_systems(
                PreUpdate,
                process_keyboard_accelerators.after(inject_bevy_input_into_masonry),
            )
            .add_systems(
                Update,
                (
                    ensure_overlay_root,
                    reparent_overlay_entities,
                    ensure_overlay_defaults,
                    handle_overlay_actions,
                    handle_widget_actions,
                    activate_debounced_hovers,
                    handle_tooltip_hovers,
                    tick_auto_dismiss,
                    sync_overlay_stack_lifecycle,
                    ensure_active_stylesheet_asset_handle,
                    sync_stylesheet_asset_events,
                    sync_active_style_variant,
                    mark_style_dirty,
                    sync_style_targets,
                )
                    .chain()
                    .before(TweenSystemSet::UpdateInterpolationValue),
            )
            .add_systems(
                Update,
                sync_focus_state.after(inject_bevy_input_into_masonry),
            )
            .add_systems(Update, handle_titlebar_actions)
            .add_systems(
                Update,
                animate_style_transitions.after(TweenSystemSet::ApplyTween),
            )
            .add_systems(
                Update,
                (sync_composition_visuals, apply_composition_effects)
                    .chain()
                    .before(TweenSystemSet::UpdateInterpolationValue),
            )
            .add_systems(Update, run_validation)
            .add_systems(Update, update_streaming_markdown_cache)
            .add_systems(Update, evict_streaming_markdown_cache)
            .add_systems(Update, handle_accessibility_actions)
            .add_systems(
                PostUpdate,
                (
                    synthesize_ui,
                    sync_titlebar_state,
                    sync_accessibility_tree,
                    rebuild_masonry_runtime,
                    sync_masonry_ime_state_to_bevy_window,
                )
                    .chain(),
            );

        // Run overlay placement after Masonry's retained tree has been rebuilt,
        // so anchor/widget geometry is up-to-date for this frame.
        app.add_systems(
            PostUpdate,
            sync_overlay_positions.after(rebuild_masonry_runtime),
        );

        app.add_systems(Last, paint_masonry_ui);

        register_builtin_style_type_aliases(app.world_mut());
        register_embedded_fluent_theme_variants(app.world_mut()).unwrap_or_else(|error| {
            panic!("failed to parse embedded Fluent theme bundle: {error}")
        });

        {
            let mut registry = app.world_mut().resource_mut::<UiProjectorRegistry>();
            register_core_projectors(&mut registry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use crate::{AppI18n, UiRoot};
    use bevy_app::App;
    use bevy_ecs::{
        hierarchy::{ChildOf, Children},
        prelude::*,
    };

    #[test]
    fn picus_plugin_enables_bsn_ui_tree_spawning() {
        use crate::WorldSceneExt as _;

        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let root = app
            .world_mut()
            .spawn_scene(crate::bsn! {
                crate::UiRoot
                crate::UiFlexColumn
                Children [
                    crate::UiLabel {
                        text: { "Hello from BSN".to_string() },
                    },
                    crate::UiButton {
                        label: { "Click".to_string() },
                    },
                ]
            })
            .expect("PicusPlugin should install Bevy scene spawning")
            .id();

        let children = app
            .world()
            .get::<Children>(root)
            .expect("BSN Children should spawn related child entities");
        assert_eq!(children.len(), 2);

        let label = children[0];
        let button = children[1];

        assert_eq!(
            app.world()
                .get::<crate::UiLabel>(label)
                .map(|label| label.text.as_str()),
            Some("Hello from BSN")
        );
        assert_eq!(
            app.world()
                .get::<crate::UiButton>(button)
                .map(|button| button.label.as_str()),
            Some("Click")
        );
    }

    #[test]
    fn public_ui_authoring_types_are_bsn_template_ready() {
        fn assert_component<T>()
        where
            T: Component + Default + Clone + bevy_ecs::template::FromTemplate,
        {
        }

        fn assert_value<T>()
        where
            T: Default + Clone + bevy_ecs::template::FromTemplate,
        {
        }

        assert_component::<crate::UiRoot>();
        assert_component::<crate::UiOverlayRoot>();
        assert_component::<crate::UiFlexColumn>();
        assert_component::<crate::UiFlexRow>();
        assert_component::<crate::UiLabel>();
        assert_component::<crate::LocalizeText>();
        assert_component::<crate::OverlayConfig>();
        assert_component::<crate::OverlayComputedPosition>();
        assert_component::<crate::OverlayState>();
        assert_component::<crate::AutoDismiss>();
        assert_component::<crate::AnchoredTo>();
        assert_component::<crate::OverlayAnchorRect>();
        assert_component::<crate::UiResponsiveRow>();
        assert_component::<crate::UiVisibleResponsive>();
        assert_component::<crate::UiResponsiveGrid>();

        assert_component::<crate::StyleClass>();
        assert_component::<crate::StyleDirty>();
        assert_component::<crate::InteractionState>();
        assert_component::<crate::InlineStyle>();
        assert_component::<crate::LayoutStyle>();
        assert_component::<crate::ColorStyle>();
        assert_component::<crate::TextStyle>();
        assert_component::<crate::StyleTransition>();
        assert_component::<crate::StopUiPointerPropagation>();

        assert_component::<crate::AccessibleRole>();
        assert_component::<crate::AccessibleLabel>();
        assert_component::<crate::AccessibleDescription>();
        assert_component::<crate::AccessibleValue>();
        assert_component::<crate::AccessibleState>();
        assert_component::<crate::KeyboardAccelerator>();
        assert_component::<crate::AcceleratorScope>();
        assert_component::<crate::AcceleratorTextOverride>();
        assert_component::<crate::ClipboardText>();
        assert_component::<crate::CompositionVisual>();
        assert_component::<crate::CompositionLayer>();
        assert_component::<crate::DragSource>();
        assert_component::<crate::DropTarget>();
        assert_component::<crate::ValidationState>();
        assert_component::<crate::ValidationRules>();
        assert_component::<crate::ValidatedString>();
        assert_component::<crate::ValidationDisplay>();
        assert_component::<crate::NeedsValidation>();

        assert_component::<crate::UiAvatar>();
        assert_component::<crate::UiBadge>();
        assert_component::<crate::UiBreadcrumb>();
        assert_component::<crate::UiBreadcrumbItem>();
        assert_component::<crate::UiButton>();
        assert_component::<crate::UiCanvas>();
        assert_component::<crate::UiCanvasPosition>();
        assert_component::<crate::UiCard>();
        assert_component::<crate::UiCheckbox>();
        assert_component::<crate::UiColorPicker>();
        assert_component::<crate::UiColorPickerPanel>();
        assert_component::<crate::UiComboBox>();
        assert_component::<crate::UiContextMenuTrigger>();
        assert_component::<crate::UiContextMenu>();
        assert_component::<crate::UiDataTable>();
        assert_component::<crate::UiDatePicker>();
        assert_component::<crate::UiDatePickerPanel>();
        assert_component::<crate::UiDialog>();
        assert_component::<crate::UiDivider>();
        assert_component::<crate::UiDropdownMenu>();
        assert_component::<crate::UiDropdownItem>();
        assert_component::<crate::UiExpander>();
        assert_component::<crate::UiGrid>();
        assert_component::<crate::UiGridCell>();
        assert_component::<crate::UiGroupBox>();
        assert_component::<crate::UiImage>();
        assert_component::<crate::UiLink>();
        assert_component::<crate::UiListView>();
        assert_component::<crate::UiMarkdown>();
        assert_component::<crate::UiMenuBar>();
        assert_component::<crate::UiMenuBarItem>();
        assert_component::<crate::UiMenuItemPanel>();
        assert_component::<crate::UiMessageBar>();
        assert_component::<crate::UiMultilineTextInput>();
        assert_component::<crate::UiNavigationItem>();
        assert_component::<crate::UiNavigationView>();
        assert_component::<crate::UiPasswordInput>();
        assert_component::<crate::UiPopover>();
        assert_component::<crate::UiProgressBar>();
        assert_component::<crate::UiRadioGroup>();
        assert_component::<crate::UiRating>();
        assert_component::<crate::UiScrollView>();
        assert_component::<crate::UiSearch>();
        assert_component::<crate::UiSlider>();
        assert_component::<crate::UiSpinner>();
        assert_component::<crate::UiSplitPane>();
        assert_component::<crate::UiStreamingMarkdown>();
        assert_component::<crate::UiSwitch>();
        assert_component::<crate::UiTabBar>();
        assert_component::<crate::UiTable>();
        assert_component::<crate::UiText>();
        assert_component::<crate::UiTextInput>();
        assert_component::<crate::UiThemePicker>();
        assert_component::<crate::UiThemePickerMenu>();
        assert_component::<crate::UiTimePicker>();
        assert_component::<crate::UiTimePickerPanel>();
        assert_component::<crate::UiTitleBar>();
        assert_component::<crate::UiNumericUpDown>();
        assert_value::<crate::UiDataCell>();
        assert_component::<crate::UiToolbar>();
        assert_component::<crate::UiToast>();
        assert_component::<crate::UiTooltip>();
        assert_component::<crate::HasTooltip>();
        assert_component::<crate::UiTreeNode>();
        assert_component::<crate::TitleBarState>();

        assert_value::<crate::AcceleratorModifiers>();
        assert_value::<crate::AvatarShape>();
        assert_value::<crate::ButtonAppearance>();
        assert_value::<crate::ButtonIconPosition>();
        assert_value::<crate::ButtonShape>();
        assert_value::<crate::ButtonSize>();
        assert_value::<crate::ClipRect>();
        assert_value::<crate::CompositionBrush>();
        assert_value::<crate::CompositionEffect>();
        assert_value::<crate::DragData>();
        assert_value::<crate::DragDataType>();
        assert_value::<crate::DragPreview>();
        assert_value::<crate::DropShadow>();
        assert_value::<crate::GradientStop>();
        assert_value::<crate::MessageBarKind>();
        assert_value::<crate::NavigationViewItem>();
        assert_value::<crate::RatingColor>();
        assert_value::<crate::RatingSize>();
        assert_value::<crate::ScrollAxis>();
        assert_value::<crate::SplitDirection>();
        assert_value::<crate::ToastKind>();
        assert_value::<crate::TypographyPreset>();
        assert_value::<crate::UiComboOption>();
        assert_value::<crate::UiContextMenuItem>();
        assert_value::<crate::UiDataColumn>();
        assert_value::<crate::UiDataRow>();
        assert_value::<crate::UiDataTableSort>();
        assert_value::<crate::UiImageViewBox>();
        assert_value::<crate::UiImageViewBoxUnits>();
        assert_value::<crate::UiImageAlignmentX>();
        assert_value::<crate::UiImageAlignmentY>();
        assert_value::<crate::UiListSelectionMode>();
        assert_value::<crate::UiMenuItem>();
        assert_value::<crate::UiSortDirection>();
        assert_value::<crate::UiThemePickerOption>();
        assert_value::<crate::VisualTransform>();
    }

    #[test]
    fn plugin_wires_synthesis_and_runtime() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .register_projector::<TestRoot>(project_test_root);

        app.world_mut().spawn((UiRoot, TestRoot));

        app.update();

        let stats = app.world().resource::<crate::UiSynthesisStats>();
        assert_eq!(stats.root_count, 2);

        let _runtime = app.world().non_send::<crate::MasonryRuntime>();
    }

    #[test]
    fn plugin_auto_registers_builtin_ui_components_without_manual_setup() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        app.world_mut()
            .spawn((UiRoot, crate::UiButton::new("auto-builtins")));

        app.update();

        let stats = app.world().resource::<crate::UiSynthesisStats>();
        assert_eq!(stats.unhandled_count, 0);
    }

    #[test]
    fn plugin_initializes_app_i18n_resource() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        assert!(app.world().contains_resource::<AppI18n>());
    }

    #[test]
    fn plugin_auto_registers_badge_and_progress_bar_components() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        app.world_mut()
            .spawn((crate::UiBadge::new("Beta"), ChildOf(root)));
        app.world_mut()
            .spawn((crate::UiProgressBar::determinate(0.5), ChildOf(root)));

        app.update();

        let stats = app.world().resource::<crate::UiSynthesisStats>();
        assert_eq!(stats.unhandled_count, 0);
    }
}
