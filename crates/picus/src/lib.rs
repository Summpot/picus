//! Public facade for building Picus applications.
//!
//! `picus_core` contains the implementation crates and internal integration surface. This crate is
//! the stable user-facing entry point, grouped by the way applications usually import Picus APIs.
#![forbid(unsafe_code)]

pub use picus_core as core;
pub use picus_core::*;

/// Application setup, plugins, runners, and Bevy re-exports.
pub mod app {
    pub use picus_core::{
        AppPicusExt, BevyWindowOptions, PicusBuiltinsPlugin, PicusPlugin, SyncAssetSource,
        SyncTextSource, WindowBackdropColorScheme, WindowBackdropMaterial, WindowSize, bevy_app,
        bevy_asset, bevy_ecs, bevy_input, bevy_math, bevy_scene, bevy_tasks, bevy_text,
        bevy_tween, bevy_window, configure_window_for_backdrop, rfd, run_app,
        run_app_with_window_options,
    };
}

/// ECS authoring components, helper views, and component registration contracts.
pub mod components {
    pub use picus_core::{
        AppBreakpoints, AutoDismiss, AvatarShape, BuiltinUiAction, ButtonAppearance,
        ButtonIconPosition, ButtonShape, ButtonSize, FluentIcon, HasTooltip, IconGlyph,
        LocalizeText, MessageBarKind, NavigationViewItem, PicusIcon, RatingColor, RatingSize,
        ScrollAxis, SplitDirection, TitleBarAction, TitleBarIcon, TitleBarState, ToastKind,
        UiAnyView, UiAvatar, UiBadge, UiBreadcrumb,
        UiBreadcrumbItem, UiButton, UiCanvas, UiCanvasCommand, UiCanvasPathCommand,
        UiCanvasPosition, UiCard, UiCheckbox, UiCheckboxChanged, ColorPickerChannel, UiColorPicker,
        UiColorPickerChanged, UiColorPickerPanel, UiComboBox, UiComboBoxChanged, UiComboOption,
        UiComponentTemplate, UiContextMenu, UiContextMenuItem, UiContextMenuItemSelected,
        UiContextMenuTrigger, UiDataCell, UiDataColumn, UiDataRow, UiDataTable,
        UiDataTableSelectionChanged, UiDataTableSort, UiDataTableSortChanged, UiDatePicker,
        UiDatePickerChanged, UiDatePickerPanel, UiDialog, UiDivider, UiDropdownItem,
        UiDropdownMenu, UiDropdownPlacement, UiExpander, UiExpanderChanged, UiFlexColumn,
        UiFlexRow, UiGradientStop, UiGrid, UiGridAutoFlow, UiGridCell, UiGridLength,
        UiGridLengthParseError, UiGroupBox, UiImage, UiImageAlignmentX, UiImageAlignmentY,
        UiImageViewBox, UiImageViewBoxUnits, UiInteractionEvent, UiLabel, UiLink, UiLinkAction,
        UiListSelectionMode, UiListView, UiListViewSelectionChanged, UiMarkdown, UiMenuBar,
        UiMenuBarItem, UiMenuItem, UiMenuItemPanel, UiMenuItemSelected, UiMessageBar,
        UiMultilineTextInput, UiMultilineTextInputChanged, UiNavigationItem,
        UiNavigationItemExpandedChanged, UiNavigationPaneChanged, UiNavigationSelectionChanged,
        UiNavigationView, NAV_PANE_COMPACT_WIDTH, NAV_PANE_EXPANDED_WIDTH, UiNumericUpDown,
        UiNumericUpDownChanged,
        UiOverlayRoot, UiPasswordInput, UiPasswordInputChanged, UiPointerEvent, UiPointerHitEvent,
        UiPointerPhase, UiPopover, UiProgressBar, UiRadioGroup, UiRadioGroupChanged, UiRating,
        UiRatingChanged, UiResponsiveGrid, UiResponsiveRow, UiRoot, UiScrollView,
        UiScrollViewChanged, UiSearch, UiSearchChanged, UiSlider, UiSliderChanged,
        UiSortDirection, UiSpinner, UiSplitPane, UiStreamingMarkdown, UiSwitch, UiSwitchChanged,
        UiTabBar, UiTabChanged, UiTable, UiText, UiTextInput, UiTextInputChanged, UiThemePicker,
        UiThemePickerChanged, UiThemePickerMenu, UiThemePickerOption, UiTitleBar, UiToast,
        UiToolbar, UiTooltip, UiTreeNode, UiTreeNodeToggled, UiView, UiVisibleResponsive,
        UiWindow, WindowBackdropColorScheme, WindowBackdropMaterial, button,
        button_with_child, checkbox, slider, switch, text_input,
    };
    pub use picus_core::icon::{
        fluent_icon, icon, icon_glyph, icon_glyph_with_font_stack, icon_source, picus_icon,
    };
}

/// Low-level projection helpers for custom `UiComponentTemplate` implementations.
pub mod projection {
    pub use picus_core::{
        ButtonView, ButtonWithChildView, CheckboxView, ProjectionCtx, SliderView, SwitchView,
        UiView, button, button_with_child, checkbox, slider, switch, text_input,
    };
}

/// Styling, themes, and selector APIs.
pub mod styling {
    pub use picus_core::{
        ActiveStyleVariant, BackdropStyle, BaseStyleSheet, ColorStyle, ComputedStyle,
        CurrentColorStyle, InlineStyle, InteractionState, LayoutStyle, PseudoClass, Selector,
        StyleClass, StyleDirty, StylePseudoState, StyleRule, StyleSetter, StyleSheet,
        StyleTransition, SyncAssetSource, SyncTextSource, TargetColorStyle, TextStyle,
        ThemeBackdrop, ThemeBackdropOverride, TokenValue, WINDOW_BACKDROP_TOKEN,
        apply_active_stylesheet_ron, clear_theme_backdrop_material_override, mark_style_dirty,
        parse_stylesheet_ron, register_builtin_style_type_aliases, resolve_style,
        resolve_style_for_classes, resolve_style_for_classes_with_state,
        resolve_style_for_entity_classes, resolve_theme_backdrop_material,
        set_active_style_variant_by_name, set_theme_backdrop_material,
    };
}

/// Events, action queues, accelerators, and widget action processing.
pub mod events {
    pub use picus_core::{
        AcceleratorActivated, AcceleratorModifiers, AcceleratorScope, AcceleratorTextOverride,
        AccessibleAction, CurrentAcceleratorModifiers, KeyboardAccelerator, TypedUiEvent, UiEvent,
        UiEventQueue, WidgetUiAction, bubble_ui_pointer_events, emit_ui_action,
        format_accelerator_text, handle_accessibility_actions, handle_widget_actions,
        process_keyboard_accelerators,
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
        MasonryRuntime, ProjectionCtx, SynthesizedUiViews, UiProjectionInvalidation, UiProjector,
        UiProjectorRegistry, WindowRuntime, XilemFontBridge, collect_bevy_font_assets,
        expand_builtin_ui_component_templates, find_template_part, gather_ui_roots,
        inject_bevy_input_into_masonry, rebuild_masonry_runtime, register_builtin_projectors,
        register_builtin_ui_components, sync_fonts_to_xilem, synthesize_roots,
        synthesize_roots_with_stats, synthesize_ui, synthesize_world, track_window_size,
    };
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
    pub use picus_core::bevy_ecs::hierarchy::{ChildOf, Children};
}
