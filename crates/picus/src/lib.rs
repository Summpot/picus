//! Public facade for building Picus applications.
//!
//! Depend on this crate only. Implementation details live in `picus_core` and
//! are not part of the stable application surface.
//!
//! # Quick path
//!
//! 1. Create a Bevy `App`, add [`app::PicusPlugin`]
//! 2. Load a theme with [`app::AppPicusExt::load_style_sheet_ron`] (or asset path)
//! 3. Register business actions with [`app::AppPicusExt::add_ui_action`]
//! 4. Derive [`UiComponent`] and call [`register_ui_components!`]
//! 5. Handle [`events::UiAction`] with Bevy `MessageReader`
//! 6. Run with [`app::AppPicusExt::run_picus`]
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
//! | Runtime | `docs/architecture/runtime.md` |
//! | Projection | `docs/architecture/projection.md` |
//! | Public modules | `docs/reference/public-modules.md` |
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
    pub use picus_core::avatar_sizes;
    pub use picus_core::icon::{
        fluent_icon, icon, icon_glyph, icon_glyph_with_font_stack, icon_source, picus_icon,
    };
    pub use picus_core::{
        AppBreakpoints, AutoDismiss, AvatarShape, BuiltinUiAction, ButtonAppearance,
        ButtonIconPosition, ButtonShape, ButtonSize, ColorPickerChannel, FluentIcon, HasTooltip,
        IconGlyph, LocalizeText, MessageBarKind, NAV_COMPACT_MODE_THRESHOLD,
        NAV_EXPANDED_MODE_THRESHOLD, NAV_PANE_COMPACT_WIDTH, NAV_PANE_EXPANDED_WIDTH,
        NavigationBackButtonVisible, NavigationDisplayMode, NavigationItemRegion,
        NavigationPaneDisplayMode, NavigationViewItem, NavigationViewItemKind, PicusIcon,
        RatingColor, RatingSize, ScrollAxis, SplitDirection, TitleBarAction, TitleBarIcon,
        TitleBarState, ToastKind, UiAnyView, UiAvatar, UiBadge, UiBreadcrumb, UiBreadcrumbItem,
        UiButton, UiCanvas, UiCanvasCommand, UiCanvasPathCommand, UiCanvasPosition, UiCard,
        UiCheckbox, UiCheckboxChanged, UiColorPicker, UiColorPickerChanged, UiColorPickerPanel,
        UiComboBox, UiComboBoxChanged, UiComboOption, UiComponentTemplate, UiContentShell,
        UiContextMenu, UiContextMenuItem, UiContextMenuItemSelected, UiContextMenuTrigger,
        UiDataCell, UiDataColumn, UiDataRow, UiDataTable, UiDataTableSelectionChanged,
        UiDataTableSort, UiDataTableSortChanged, UiDatePicker, UiDatePickerChanged,
        UiDatePickerPanel, UiDialog, UiDivider, UiDropdownItem, UiDropdownMenu,
        UiDropdownPlacement, UiEmit, UiExpander, UiExpanderChanged, UiFlexColumn, UiFlexRow,
        UiFormRow, UiGradientStop, UiGrid, UiGridAutoFlow, UiGridCell, UiGridLength,
        UiGridLengthParseError, UiGroupBox, UiImage, UiImageAlignmentX, UiImageAlignmentY,
        UiImageViewBox, UiImageViewBoxUnits, UiInteractionEvent, UiLabel, UiLink, UiLinkAction,
        UiListSelectionMode, UiListView, UiListViewSelectionChanged, UiMarkdown, UiMenuBar,
        UiMenuBarItem, UiMenuItem, UiMenuItemPanel, UiMenuItemSelected, UiMessageBar,
        UiMultilineTextInput, UiMultilineTextInputChanged, UiNavigationBackRequested,
        UiNavigationDisplayModeChanged, UiNavigationItem, UiNavigationItemExpandedChanged,
        UiNavigationItemInvoked, UiNavigationPaneChanged, UiNavigationSelectionChanged,
        UiNavigationSettingsItem, UiNavigationView, UiNumericUpDown, UiNumericUpDownChanged,
        UiOverlayRoot, UiPasswordInput, UiPasswordInputChanged, UiPointerEvent, UiPointerHitEvent,
        UiPointerPhase, UiPopover, UiProgressBar, UiRadioGroup, UiRadioGroupChanged, UiRating,
        UiRatingChanged, UiResponsiveGrid, UiResponsiveRow, UiRoot, UiScrollView,
        UiScrollViewChanged, UiSearch, UiSearchChanged, UiSlider, UiSliderChanged, UiSortDirection,
        UiSpinner, UiSplitPane, UiStreamingMarkdown, UiSwitch, UiSwitchChanged, UiTabBar,
        UiTabChanged, UiTable, UiText, UiTextInput, UiTextInputChanged, UiThemePicker,
        UiThemePickerChanged, UiThemePickerMenu, UiThemePickerOption, UiTimePicker,
        UiTimePickerChanged, UiTimePickerPanel, UiTitleBar, UiToast, UiToolbar, UiTooltip,
        UiTreeNode, UiTreeNodeToggled, UiView, UiVisibleResponsive, UiWindow,
        WindowBackdropColorScheme, WindowBackdropMaterial, checkbox, slider, switch, text_input,
    };
}

/// Low-level projection helpers for custom `UiComponentTemplate` implementations.
pub mod projection {
    pub use picus_core::{
        ButtonView, ButtonWithChildView, CheckboxView, ProjectionCtx, SliderView, SwitchView,
        UiView, checkbox, slider, switch, text_input,
    };
    pub use picus_core::{picus_view, xilem};
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
        apply_label_style, apply_text_input_style, apply_widget_style,
        clear_theme_backdrop_material_override, mark_style_dirty, parse_stylesheet_ron,
        register_builtin_style_type_aliases, resolve_style, resolve_style_for_classes,
        resolve_style_for_classes_with_state, resolve_style_for_entity_classes,
        resolve_theme_backdrop_material, set_active_style_variant_by_name,
        set_theme_backdrop_material, styled,
    };
}

/// Application-facing UI actions and Bevy message integration.
///
/// See `docs/guide/events-messages.md`.
pub mod events {
    pub use picus_core::{
        AcceleratorActivated, AcceleratorModifiers, AcceleratorScope, AcceleratorTextOverride,
        AccessibleAction, CurrentAcceleratorModifiers, KeyboardAccelerator, UiAction,
        UiActionSender, UiEmit, format_accelerator_text,
    };
}

/// Overlay helpers and overlay lifecycle systems.
pub mod overlay {
    pub use picus_core::{
        OverlayComputedPosition, OverlayConfig, OverlayMouseButtonCursor, OverlayPlacement,
        OverlayPointerRoutingState, OverlayStack, OverlayState, dismiss_overlays_on_click,
        ensure_overlay_root, ensure_overlay_root_entity, handle_global_overlay_clicks,
        handle_tooltip_hovers, spawn_in_overlay_root,
        spawn_manual_overlay_at, spawn_popover_in_overlay_root, sync_dropdown_positions,
        sync_overlay_positions, sync_overlay_stack_lifecycle, tick_auto_dismiss, tick_toasts,
    };
}

/// Runtime synthesis and rendering integration.
pub mod runtime {
    pub use picus_core::masonry_core;
    pub use picus_core::{
        MasonryRuntime, ProjectionCtx, SynthesizedUiViews, UiDirtyReason, UiProjectionDirtyDebug,
        UiProjectionInvalidation, UiView, WindowRuntime, XilemFontBridge, collect_bevy_font_assets,
        inject_bevy_input_into_masonry, rebuild_masonry_runtime, synthesize_ui, track_window_size,
    };

    /// Low-level registration and projector APIs for advanced / framework use.
    pub mod advanced {
        pub use picus_core::{
            AdvancedAppPicusExt, UiProjector, UiProjectorRegistry,
            expand_builtin_ui_component_templates, find_template_part, gather_ui_roots,
            register_builtin_projectors, register_builtin_ui_components,
            route_masonry_view_messages, sync_fonts_to_xilem, synthesize_roots,
            synthesize_roots_with_stats, synthesize_world,
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
    pub use crate::{UiComponent, classes, register_ui_components, ui_view};
    pub use crate::{
        app::*, components::*, events::*, i18n::*, icons::*, overlay::*, projection::*, runtime::*,
        scene::*, styling::*,
    };
    pub use picus_core::bevy_ecs::hierarchy::{ChildOf, Children};
    pub use picus_core::bevy_ecs::message::MessageReader;
}

// ---------------------------------------------------------------------------
// Root-level macros and hidden macro support (no `picus_core::*` dump).
// ---------------------------------------------------------------------------

pub use picus_macros::{UiComponent, ui_view};

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
    use picus_core::{AdvancedAppPicusExt, UiComponentTemplate};

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
