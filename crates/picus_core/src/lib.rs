//! Bevy + Masonry Core integration with ECS-driven UI projection.
//!
//! `picus_core` contains the implementation surface for Picus:
//! - register ECS UI components through [`UiComponentTemplate`],
//! - collect typed UI actions through [`UiEventQueue`],
//! - describe ECS UI trees with Bevy Scene Notation (`bsn!`),
//! - incrementally synthesize ECS UI changes into a retained Masonry tree.
//!
//! # Minimal setup
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use picus_core::{
//!     AppPicusExt, PicusPlugin, ProjectionCtx, UiComponentTemplate, UiEventQueue, UiRoot,
//!     UiView,
//!     bevy_app::{App, PreUpdate, Startup},
//!     bevy_ecs::prelude::*,
//!     button,
//! };
//!
//! #[derive(Component, Clone, Copy)]
//! struct Root;
//!
//! #[derive(Debug, Clone, Copy)]
//! enum Action {
//!     Clicked,
//! }
//!
//! impl UiComponentTemplate for Root {
//!     fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
//!         Arc::new(button(ctx.entity, Action::Clicked, "Click"))
//!     }
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn((UiRoot, Root));
//! }
//!
//! fn drain(world: &mut World) {
//!     let _ = world.resource_mut::<UiEventQueue>().drain_actions::<Action>();
//! }
//!
//! let mut app = App::new();
//! app.add_plugins(PicusPlugin)
//!     .register_ui_component::<Root>()
//!     .add_systems(Startup, setup)
//!     .add_systems(PreUpdate, drain);
//! ```
#![forbid(unsafe_code)]

pub mod accelerator;
pub mod accessibility;
pub mod app_ext;
pub mod backdrop;
pub mod bevy_tween;
pub mod clipboard;
pub mod components;
pub mod composition;
pub mod drag_drop;
pub mod ecs;
pub mod events;
pub mod fonts;
pub mod i18n;
pub mod icon;
pub mod icons;
pub mod overlay;
pub mod plugin;
pub mod projection;
pub mod resize;
pub mod runner;
pub mod runtime;
pub mod scene;
pub mod styling;
pub mod synthesize;
pub mod templates;
pub mod titlebar_system;
pub mod validation;
pub mod widget_actions;
pub mod xilem;

mod retained_bridge;
mod retained_widgets;

pub use bevy_app;
pub use bevy_asset;
pub use bevy_ecs;
pub use bevy_input;
pub use bevy_math;
pub use bevy_scene;
pub use bevy_tasks;
pub use bevy_text;
pub use bevy_window;
pub use masonry_core;
pub use picus_view;
pub use rfd;

pub use accelerator::*;
pub use accessibility::*;
pub use app_ext::*;
pub use backdrop::*;

pub use clipboard::*;
pub use components::*;
pub use composition::*;
pub use drag_drop::*;
pub use ecs::*;
pub use events::*;
pub use fonts::*;
pub use i18n::*;
pub use icons::*;
pub use overlay::*;
pub use plugin::*;
pub use projection::*;
pub use resize::*;
pub use retained_bridge::{
    ButtonView, ButtonWithChildView, CheckboxView, SliderView, SwitchView, button,
    button_with_child, checkbox, slider, switch, text_input,
};
pub use runner::*;
pub use runtime::*;
pub use scene::*;
pub use styling::*;
pub use synthesize::*;
pub use templates::*;
pub use titlebar_system::*;
pub use validation::*;
pub use widget_actions::*;

pub mod prelude {
    //! Convenience exports for Picus internals and legacy `picus_core` users.

    pub use crate::scene::*;
    pub use bevy_ecs::hierarchy::{ChildOf, Children};

    pub use crate::{
        AppBreakpoints, AppI18n, AppPicusExt, AutoDismiss, AvatarShape, BevyWindowOptions,
        BuiltinUiAction, ButtonAppearance, ButtonIconPosition, ButtonShape, ButtonSize, ButtonView,
        BackdropStyle, CheckboxView, ColorStyle, ComputedStyle, CurrentColorStyle, FluentIcon,
        HasTooltip, IconGlyph, InlineStyle, InteractionState, LayoutStyle, LocalizeText,
        MasonryRuntime,
        MessageBarKind, NavigationViewItem, ObjectFit, OverlayComputedPosition, OverlayConfig,
        OverlayMouseButtonCursor, OverlayPlacement, OverlayPointerRoutingState, OverlayStack,
        OverlayState, OverlayUiAction, PicusBuiltinsPlugin, PicusIcon, PicusPlugin, ProjectionCtx,
        PseudoClass, RatingColor, RatingSize, ScrollAxis, Selector, SliderView, SplitDirection,
        StopUiPointerPropagation, StreamingMarkdownParseCache, StyleClass, StyleDirty, StyleRule,
        StyleSetter, StyleSheet, StyleTransition, SwitchView, SyncAssetSource, SyncTextSource,
        SynthesizedUiViews, TargetColorStyle, TextStyle, ThemeBackdrop, ThemeBackdropOverride,
        TitleBarAction, TitleBarIcon,
        TitleBarState, ToastKind, TypedUiEvent, UiAnyView, UiAvatar, UiBadge, UiBreadcrumb,
        UiBreadcrumbItem, UiButton, UiCanvas, UiCanvasCommand, UiCanvasPathCommand,
        UiCanvasPosition, UiCard, UiCheckbox, UiCheckboxChanged, ColorPickerChannel, UiColorPicker,
        UiColorPickerChanged, UiColorPickerPanel, UiComboBox, UiComboBoxChanged, UiComboOption,
        UiComponentTemplate, UiContextMenu, UiContextMenuItem, UiContextMenuItemSelected,
        UiContextMenuTrigger, UiDataCell, UiDataColumn, UiDataRow, UiDataTable,
        UiDataTableSelectionChanged, UiDataTableSort, UiDataTableSortChanged, UiDatePicker,
        UiDatePickerChanged, UiDatePickerPanel, UiDialog, UiDivider, UiDropdownItem,
        UiDropdownMenu, UiDropdownPlacement, UiEvent, UiEventQueue, UiExpander, UiExpanderChanged,
        UiFlexColumn, UiFlexRow, UiGradientStop, UiGrid, UiGridAutoFlow, UiGridCell, UiGridLength,
        UiGridLengthParseError, UiGroupBox, UiImage, UiImageAlignmentX, UiImageAlignmentY,
        UiImageViewBox, UiImageViewBoxUnits, UiInteractionEvent, UiLabel, UiLink, UiLinkAction,
        UiListSelectionMode, UiListView, UiListViewSelectionChanged, UiMarkdown, UiMenuBar,
        UiMenuBarItem, UiMenuItem, UiMenuItemPanel, UiMenuItemSelected, UiMessageBar,
        UiMultilineTextInput, UiMultilineTextInputChanged, NavigationBackButtonVisible,
        NavigationDisplayMode, NavigationItemRegion, NavigationPaneDisplayMode,
        NavigationViewItemKind, UiNavigationBackRequested, UiNavigationDisplayModeChanged,
        UiNavigationItem, UiNavigationItemExpandedChanged, UiNavigationItemInvoked,
        UiNavigationPaneChanged, UiNavigationSelectionChanged, UiNavigationSettingsItem,
        UiNavigationView, NAV_COMPACT_MODE_THRESHOLD, NAV_EXPANDED_MODE_THRESHOLD,
        NAV_PANE_COMPACT_WIDTH, NAV_PANE_EXPANDED_WIDTH, UiNumericUpDown,
        UiNumericUpDownChanged, UiOverlayRoot, UiPasswordInput,
        UiPasswordInputChanged, UiPointerEvent, UiPointerHitEvent, UiPointerPhase, UiPopover,
        UiProgressBar, UiProjectionInvalidation, UiProjector, UiProjectorRegistry, UiRadioGroup,
        UiRadioGroupChanged, UiRating, UiRatingChanged, UiResponsiveGrid, UiResponsiveRow, UiRoot,
        UiScrollView, UiScrollViewChanged, UiSearch, UiSearchChanged, UiSlider, UiSliderChanged,
        UiSortDirection, UiSpinner, UiSplitPane, UiStreamingMarkdown, UiSwitch, UiSwitchChanged,
        UiSynthesisStats, UiTabBar, UiTabChanged, UiTable, UiText, UiTextInput, UiTextInputChanged,
        UiThemePicker,
        UiThemePickerChanged, UiThemePickerMenu, UiThemePickerOption, UiTitleBar, UiToast,
        UiToolbar, UiTooltip, UiTreeNode, UiTreeNodeToggled, UiView, UiVisibleResponsive, UiWindow,
        WidgetUiAction, WindowBackdropColorScheme, WindowBackdropMaterial, WindowRuntime,
        WindowSize, XilemFontBridge, bubble_ui_pointer_events, button, button_with_child, checkbox,
        collect_bevy_font_assets,
        clear_theme_backdrop_material_override, configure_window_for_backdrop,
        dismiss_overlays_on_click, emit_ui_action,
        ensure_overlay_root, ensure_overlay_root_entity, ensure_template_part,
        expand_builtin_ui_component_templates, find_template_part, gather_ui_roots,
        handle_global_overlay_clicks, handle_overlay_actions, handle_tooltip_hovers,
        handle_widget_actions, inject_bevy_input_into_masonry, mark_style_dirty,
        rebuild_masonry_runtime, register_builtin_projectors, register_builtin_style_type_aliases,
        register_builtin_ui_components, resolve_localized_text, resolve_style,
        resolve_style_for_classes, resolve_style_for_entity_classes, route_masonry_view_messages,
        run_app, run_app_with_window_options, set_theme_backdrop_material, slider,
        spawn_in_overlay_root,
        spawn_popover_in_overlay_root, sync_dropdown_positions, sync_fonts_to_xilem,
        sync_overlay_positions, sync_overlay_stack_lifecycle, synthesize_roots,
        synthesize_roots_with_stats, synthesize_ui, synthesize_world, text_input,
        tick_auto_dismiss, tick_toasts, track_window_size,
    };

    pub use crate::{
        bevy_app, bevy_asset, bevy_ecs, bevy_input, bevy_math, bevy_scene, bevy_tasks, bevy_text,
        bevy_tween, bevy_window, picus_view, rfd, xilem,
    };
}

#[cfg(test)]
mod test_helpers;
