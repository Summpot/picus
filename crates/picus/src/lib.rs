//! Public facade for building Picus applications.
//!
//! Depend on this crate only. Implementation details live in `picus_core` and
//! are not part of the stable application surface.
//!
//! # Quick path
//!
//! 1. Create a Bevy `App`, add [`PicusPlugin`]
//! 2. Load a theme with [`AppPicusExt::load_style_sheet_ron`] (or asset path)
//! 3. Register business actions with [`AppPicusExt::add_ui_action`]
//! 4. Derive [`UiComponent`] and call [`register_ui_components!`]
//! 5. Handle [`UiAction`] with Bevy `MessageReader`
//! 6. Run with [`AppPicusExt::run_picus`]
//!
//! # Guides (long form lives in `docs/`, not rustdoc)
//!
//! | Topic | Doc path |
//! |-------|----------|
//! | Application entry | `docs/guide/app.md` |
//! | Actions / schedule | `docs/guide/events-messages.md` |
//! | Themes | `docs/guide/styling-themes.md` |
//! | Macros | `docs/guide/macros.md` |
//! | Overlays / scroll | `docs/guide/overlays-scroll.md` |
//! | i18n / fonts | `docs/guide/i18n-fonts-icons.md` |
//! | Multi-window | `docs/guide/multi-window.md` |
//! | Doc map | `docs/README.md` |
#![forbid(unsafe_code)]

/// Application setup, plugins, runners, and Bevy re-exports.
///
/// See `docs/guide/app.md`.
pub mod app {
    pub use picus_core::{
        AppPicusExt, BevyWindowOptions, PicusBuiltinsPlugin, PicusPlugin, PicusUiSet,
        SyncAssetSource, SyncTextSource, WindowBackdropColorScheme, WindowBackdropMaterial,
        WindowSize, bevy_app, bevy_asset, bevy_ecs, bevy_input, bevy_math, bevy_scene, bevy_tasks,
        bevy_text, bevy_tween, bevy_window, configure_window_for_backdrop, rfd,
    };
}

/// ECS authoring components, helper views, and component registration contracts.
pub mod components {
    pub use picus_core::{
        AppBreakpoints, AutoDismiss, AvatarShape, BuiltinUiAction, ButtonAppearance,
        ButtonIconPosition, ButtonShape, ButtonSize, FluentIcon, HasTooltip, IconGlyph,
        LocalizeText, MessageBarKind, NavigationBackButtonVisible, NavigationDisplayMode,
        NavigationItemRegion, NavigationPaneDisplayMode, NavigationViewItem,
        NavigationViewItemKind, PicusIcon, RatingColor, RatingSize, ScrollAxis, SplitDirection,
        TitleBarAction, TitleBarIcon, TitleBarState, ToastKind, UiAnyView, UiAvatar, UiBadge,
        UiBreadcrumb, UiBreadcrumbItem, UiButton, UiCanvas, UiCanvasCommand, UiCanvasPathCommand,
        UiCanvasPosition, UiCard, UiCheckbox, UiCheckboxChanged, ColorPickerChannel, UiColorPicker,
        UiColorPickerChanged, UiColorPickerPanel, UiComboBox, UiComboBoxChanged, UiComboOption,
        UiComponentTemplate, UiContextMenu, UiContextMenuItem, UiContextMenuItemSelected,
        UiContextMenuTrigger, UiDataCell, UiDataColumn, UiDataRow, UiDataTable,
        UiDataTableSelectionChanged, UiDataTableSort, UiDataTableSortChanged, UiDatePicker,
        UiDatePickerChanged, UiDatePickerPanel, UiDialog, UiDivider, UiDropdownItem,
        UiDropdownMenu, UiDropdownPlacement, UiEmit, UiExpander, UiExpanderChanged, UiFlexColumn,
        UiFlexRow, UiGradientStop, UiGrid, UiGridAutoFlow, UiGridCell, UiGridLength,
        UiGridLengthParseError, UiGroupBox, UiImage, UiImageAlignmentX, UiImageAlignmentY,
        UiImageViewBox, UiImageViewBoxUnits, UiInteractionEvent, UiLabel, UiLink, UiLinkAction,
        UiListSelectionMode, UiListView, UiListViewSelectionChanged, UiMarkdown, UiMenuBar,
        UiMenuBarItem, UiMenuItem, UiMenuItemPanel, UiMenuItemSelected, UiMessageBar,
        UiMultilineTextInput, UiMultilineTextInputChanged, UiNavigationBackRequested,
        UiNavigationDisplayModeChanged, UiNavigationItem, UiNavigationItemExpandedChanged,
        UiNavigationItemInvoked, UiNavigationPaneChanged, UiNavigationSelectionChanged,
        UiNavigationSettingsItem, UiNavigationView, NAV_COMPACT_MODE_THRESHOLD,
        NAV_EXPANDED_MODE_THRESHOLD, NAV_PANE_COMPACT_WIDTH, NAV_PANE_EXPANDED_WIDTH,
        UiNumericUpDown, UiNumericUpDownChanged, UiOverlayRoot, UiPasswordInput,
        UiPasswordInputChanged, UiPointerEvent, UiPointerHitEvent, UiPointerPhase, UiPopover,
        UiProgressBar, UiRadioGroup, UiRadioGroupChanged, UiRating, UiRatingChanged,
        UiResponsiveGrid, UiResponsiveRow, UiRoot, UiScrollView, UiScrollViewChanged, UiSearch,
        UiSearchChanged, UiSlider, UiSliderChanged, UiSortDirection, UiSpinner, UiSplitPane,
        UiStreamingMarkdown, UiSwitch, UiSwitchChanged, UiTabBar, UiTabChanged, UiTable, UiText,
        UiTextInput, UiTextInputChanged, UiThemePicker, UiThemePickerChanged, UiThemePickerMenu,
        UiThemePickerOption, UiTitleBar, UiToast, UiToolbar, UiTooltip, UiTreeNode,
        UiTreeNodeToggled, UiView, UiVisibleResponsive, UiWindow, WindowBackdropColorScheme,
        WindowBackdropMaterial, checkbox, slider, switch, text_input,
    };
    pub use picus_core::avatar_sizes;
    pub use picus_core::icon::{
        fluent_icon, icon, icon_glyph, icon_glyph_with_font_stack, icon_source, picus_icon,
    };
}

/// Low-level projection helpers for custom `UiComponentTemplate` implementations.
pub mod projection {
    pub use picus_core::{
        ButtonView, ButtonWithChildView, CheckboxView, ProjectionCtx, SliderView, SwitchView,
        UiView, checkbox, slider, switch, text_input,
    };
}

/// Styling, themes, and selector APIs.
///
/// See `docs/guide/styling-themes.md`.
pub mod styling {
    pub use picus_core::{
        ActiveStyleVariant, BackdropStyle, BaseStyleSheet, ColorStyle, ComputedStyle,
        CurrentColorStyle, InlineStyle, InteractionState, LayoutStyle, PseudoClass, Selector,
        StyleClass, StyleDirty, StylePseudoState, StyleRule, StyleSetter, StyleSheet,
        StyleTransition, SyncAssetSource, SyncTextSource, TargetColorStyle, TextStyle,
        ThemeBackdrop, ThemeBackdropOverride, TokenValue, WINDOW_BACKDROP_TOKEN,
        apply_active_stylesheet_ron, apply_direct_text_input_style, apply_direct_widget_style,
        apply_label_style, apply_text_input_style, apply_widget_style, styled,
        clear_theme_backdrop_material_override, mark_style_dirty, parse_stylesheet_ron,
        register_builtin_style_type_aliases, resolve_style, resolve_style_for_classes,
        resolve_style_for_classes_with_state, resolve_style_for_entity_classes,
        resolve_theme_backdrop_material, set_active_style_variant_by_name,
        set_theme_backdrop_material,
    };
}

/// Application-facing UI actions and Bevy message integration.
///
/// See `docs/guide/events-messages.md`.
pub mod events {
    pub use picus_core::{
        AcceleratorActivated, AcceleratorModifiers, AcceleratorScope, AcceleratorTextOverride,
        AccessibleAction, CurrentAcceleratorModifiers, KeyboardAccelerator, UiAction,
        UiActionSender, UiEmit, WidgetUiAction, bubble_ui_pointer_events, dispatch_ui_actions,
        format_accelerator_text, process_keyboard_accelerators,
    };
}

/// Overlay helpers and overlay lifecycle systems.
pub mod overlay {
    pub use picus_core::{
        OverlayComputedPosition, OverlayConfig, OverlayMouseButtonCursor, OverlayPlacement,
        OverlayPointerRoutingState, OverlayStack, OverlayState, OverlayUiAction,
        dismiss_overlays_on_click, ensure_overlay_root, ensure_overlay_root_entity,
        handle_global_overlay_clicks, handle_overlay_actions, handle_tooltip_hovers,
        spawn_in_overlay_root, spawn_manual_overlay_at, spawn_popover_in_overlay_root,
        sync_dropdown_positions, sync_overlay_positions, sync_overlay_stack_lifecycle,
        tick_auto_dismiss, tick_toasts,
    };
}

/// Runtime synthesis and rendering integration.
pub mod runtime {
    pub use picus_core::{
        MasonryRuntime, ProjectionCtx, SynthesizedUiViews, UiProjectionInvalidation, UiView,
        WindowRuntime, XilemFontBridge, collect_bevy_font_assets, inject_bevy_input_into_masonry,
        rebuild_masonry_runtime, synthesize_ui, track_window_size,
    };

    /// Low-level registration and projector APIs for advanced / framework use.
    pub mod advanced {
        pub use picus_core::{
            UiProjector, UiProjectorRegistry, expand_builtin_ui_component_templates,
            find_template_part, gather_ui_roots, register_builtin_projectors,
            register_builtin_ui_components, route_masonry_view_messages, sync_fonts_to_xilem,
            synthesize_roots, synthesize_roots_with_stats, synthesize_world,
        };
    }
}

/// Internationalization helpers.
pub mod i18n {
    pub use picus_core::{AppI18n, resolve_localized_text};
}

/// Icon definitions and bundled icon font data.
pub mod icons {
    pub use picus_core::icons::*;
}

/// Validation helpers.
pub mod validation {
    pub use picus_core::validation::*;
}

/// BSN scene authoring helpers.
pub mod scene {
    pub use picus_core::scene::*;
}

/// Common imports for Picus applications.
pub mod prelude {
    pub use crate::{
        app::*, components::*, events::*, i18n::*, icons::*, overlay::*, projection::*,
        runtime::*, scene::*, styling::*,
    };
    pub use crate::{UiComponent, classes, register_ui_components};
    pub use picus_core::bevy_ecs::hierarchy::{ChildOf, Children};
    pub use picus_core::bevy_ecs::message::MessageReader;
}

// ---------------------------------------------------------------------------
// Root-level macros and selected re-exports (no `picus_core::*` dump).
// ---------------------------------------------------------------------------

pub use picus_macros::UiComponent;

/// Construct a [`StyleClass`] from string literals or expressions.
#[macro_export]
macro_rules! classes {
    ($($class:expr),* $(,)?) => {
        $crate::styling::StyleClass(
            ::std::vec![
                $(::std::string::ToString::to_string(&$class)),*
            ],
        )
    };
}

/// Register one or more `#[derive(UiComponent)]` types on a mutable Bevy `App`.
#[macro_export]
macro_rules! register_ui_components {
    ($app:expr $(, $ty:ty)* $(,)?) => {{
        $(
            <$ty as $crate::__macro_support::UiComponentRegistration>::register($app);
        )*
    }};
}

/// Hidden support surface used only by macro expansions.
#[doc(hidden)]
pub mod __macro_support {
    use bevy_app::App;
    use bevy_ecs::prelude::{Component, Resource};
    use picus_core::{AppPicusExt, UiComponentTemplate};

    /// Implemented by `#[derive(UiComponent)]`.
    pub trait UiComponentRegistration {
        fn register(app: &mut App);
    }

    pub fn register_ui_component<T: UiComponentTemplate>(app: &mut App) {
        app.register_ui_component::<T>();
    }

    pub fn register_projection_resource<R: Resource>(app: &mut App) {
        app.register_projection_resource::<R>();
    }

    pub fn register_style_selector_type<T: Component>(app: &mut App, name: &str) {
        app.register_style_selector_type::<T>(name);
    }
}

// --- Selected root re-exports for ergonomic application imports ---

pub use app::{
    AppPicusExt, BevyWindowOptions, PicusBuiltinsPlugin, PicusPlugin, PicusUiSet, SyncAssetSource,
    SyncTextSource, WindowBackdropColorScheme, WindowBackdropMaterial, WindowSize, bevy_app,
    bevy_asset, bevy_ecs, bevy_input, bevy_math, bevy_scene, bevy_tasks, bevy_text, bevy_tween,
    bevy_window, configure_window_for_backdrop, rfd,
};
pub use components::*;
pub use events::{UiAction, UiActionSender, dispatch_ui_actions};
pub use i18n::{AppI18n, resolve_localized_text};
pub use icons::*;
pub use overlay::{
    OverlayPlacement, OverlayState, OverlayUiAction, dismiss_overlays_on_click,
    ensure_overlay_root, spawn_in_overlay_root, spawn_manual_overlay_at,
    spawn_popover_in_overlay_root,
};
pub use projection::ProjectionCtx;
pub use scene::*;
pub use styling::*;

// Masonry / xilem bridge types used by custom projection.
pub use picus_core::{masonry_core, picus_view, xilem};

/// Read newly arrived [`UiAction`] messages in an exclusive `World` system.
///
/// Prefer ordinary `MessageReader<UiAction<T>>` systems when possible.
pub fn take_ui_actions<T: Clone + Send + Sync + 'static>(
    world: &mut bevy_ecs::world::World,
    cursor: &mut bevy_ecs::message::MessageCursor<UiAction<T>>,
) -> Vec<UiAction<T>> {
    let messages = world.resource::<bevy_ecs::message::Messages<UiAction<T>>>();
    cursor.read(messages).cloned().collect()
}

/// Convenience drain of new [`UiAction<T>`] messages for exclusive systems.
///
/// Stores a per-type cursor in a private resource so each call returns only
/// messages that arrived since the previous drain of `T`.
pub fn drain_ui_actions<T: Clone + Send + Sync + 'static>(
    world: &mut bevy_ecs::world::World,
) -> Vec<UiAction<T>> {
    use bevy_ecs::message::MessageCursor;
    use std::any::{Any, TypeId};
    use std::collections::HashMap;

    #[derive(bevy_ecs::prelude::Resource, Default)]
    struct UiActionCursors(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

    world.init_resource::<UiActionCursors>();
    let type_id = TypeId::of::<T>();
    let mut cursor = {
        let mut cursors = world.resource_mut::<UiActionCursors>();
        cursors
            .0
            .remove(&type_id)
            .and_then(|boxed| boxed.downcast::<MessageCursor<UiAction<T>>>().ok())
            .map(|boxed| *boxed)
            .unwrap_or_default()
    };
    let actions = take_ui_actions::<T>(world, &mut cursor);
    world
        .resource_mut::<UiActionCursors>()
        .0
        .insert(type_id, Box::new(cursor));
    actions
}
