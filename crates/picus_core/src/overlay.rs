use std::collections::{HashMap, HashSet};

use bevy_ecs::{
    bundle::Bundle,
    component::Mutable,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    message::MessageCursor,
    prelude::*,
};
use bevy_input::{
    ButtonInput,
    mouse::{MouseButton, MouseButtonInput},
};
use bevy_math::Vec2;
use bevy_window::{PrimaryWindow, Window};
use masonry_core::core::{Widget, WidgetRef};

use crate::projection::dialog::{
    dialog_surface_gap, dialog_surface_padding, estimate_dialog_surface_height_px,
    estimate_dialog_surface_width_px,
};
use crate::{
    AnchoredTo, AppI18n, AutoDismiss, OverlayAnchorRect, OverlayComputedPosition, OverlayConfig,
    OverlayPlacement, OverlayStack, OverlayState, StopUiPointerPropagation, UiColorPicker,
    UiColorPickerChanged, UiColorPickerPanel, UiComboBox, UiComboBoxChanged, UiContextMenu,
    UiContextMenuItem, UiContextMenuItemSelected, UiContextMenuTrigger, UiDatePicker,
    UiDatePickerChanged, UiDatePickerPanel, UiDialog, UiDropdownItem, UiDropdownMenu, UiEventQueue,
    UiExpander, UiExpanderChanged, UiInteractionEvent, UiMenuBarItem, UiMenuItemPanel,
    UiMenuItemSelected, UiOverlayRoot, UiPointerEvent, UiPointerHitEvent, UiPopover, UiRoot,
    UiThemePicker, UiThemePickerChanged, UiThemePickerMenu, UiTimePicker, UiTimePickerChanged,
    UiTimePickerPanel, UiToast, UiTooltip,
    events::UiEvent,
    runtime::MasonryRuntime,
    set_active_style_variant_by_name,
    styling::{resolve_style, resolve_style_for_classes},
};

const OVERLAY_ANCHOR_GAP: f64 = 4.0;
const DROPDOWN_MAX_VIEWPORT_HEIGHT: f64 = 300.0;
const DROPDOWN_ITEM_HOVER_ENTER_DELAY_SECS: f32 = 0.015;

/// Internal overlay actions emitted by built-in floating UI projectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayUiAction {
    DismissDialog,
    ToggleCombo,
    SelectComboItem { dropdown: Entity, index: usize },
    DismissDropdown,
    ToggleThemePicker,
    SelectThemePickerItem { index: usize },
    DismissThemePicker,
    // Menu bar overlay
    ToggleMenuBarItem,
    DismissMenuBarItem,
    SelectMenuBarItem { index: usize },
    // Color picker overlay
    ToggleColorPicker,
    SelectColorSwatch { r: u8, g: u8, b: u8 },
    DismissColorPicker,
    // Date picker overlay
    ToggleDatePicker,
    NavigateDateMonth { forward: bool },
    SelectDateDay { day: u32 },
    DismissDatePicker,
    // Time picker overlay
    ToggleTimePicker,
    SelectTimeHour { hour: u8 },
    SelectTimeMinute { minute: u8 },
    SelectTimePeriod { is_pm: bool },
    DismissTimePicker,
    // Expander
    ToggleExpander,
    // Context menu
    SelectContextMenuItem { index: usize },
    DismissContextMenu,
    // Toast
    DismissToast,
}

/// Per-frame pointer routing decisions used by the input bridge.
#[derive(Resource, Debug, Default)]
pub struct OverlayPointerRoutingState {
    suppressed_presses: Vec<(Entity, MouseButton)>,
    suppressed_releases: Vec<(Entity, MouseButton)>,
}

impl OverlayPointerRoutingState {
    fn push_unique(entries: &mut Vec<(Entity, MouseButton)>, window: Entity, button: MouseButton) {
        if !entries.iter().any(|(existing_window, existing_button)| {
            *existing_window == window && *existing_button == button
        }) {
            entries.push((window, button));
        }
    }

    /// Returns true if this exact pressed event should be blocked and consumes the block entry.
    pub(crate) fn take_suppressed_press(&mut self, window: Entity, button: MouseButton) -> bool {
        if let Some(index) = self
            .suppressed_presses
            .iter()
            .position(|(w, b)| *w == window && *b == button)
        {
            self.suppressed_presses.swap_remove(index);
            true
        } else {
            false
        }
    }

    /// Returns true if this exact release event should be blocked and consumes the block entry.
    pub(crate) fn take_suppressed_release(&mut self, window: Entity, button: MouseButton) -> bool {
        if let Some(index) = self
            .suppressed_releases
            .iter()
            .position(|(w, b)| *w == window && *b == button)
        {
            self.suppressed_releases.swap_remove(index);
            true
        } else {
            false
        }
    }

    /// Mark the next `Pressed` event for this `(window, button)` pair as consumed.
    pub(crate) fn suppress_press(&mut self, window: Entity, button: MouseButton) {
        Self::push_unique(&mut self.suppressed_presses, window, button);
    }

    /// Suppress the next press for a globally consumed click.
    pub(crate) fn suppress_click(&mut self, window: Entity, button: MouseButton) {
        self.suppress_press(window, button);
        // NOTE: suppressing release can outlive the originating click across frames,
        // which may consume the next valid release and leave trigger buttons in a
        // sticky-pressed state that requires an extra click.
    }
}

/// Message cursor resource used by the world-exclusive click-outside router.
#[derive(Resource, Default)]
pub struct OverlayMouseButtonCursor(pub MessageCursor<MouseButtonInput>);

fn remove_overlay_from_stack(world: &mut World, entity: Entity) {
    let Some(mut stack) = world.get_resource_mut::<OverlayStack>() else {
        return;
    };

    stack.active_overlays.retain(|current| *current != entity);
}

fn push_overlay_to_stack(world: &mut World, entity: Entity) {
    let Some(mut stack) = world.get_resource_mut::<OverlayStack>() else {
        return;
    };

    stack.active_overlays.retain(|current| *current != entity);
    stack.active_overlays.push(entity);
}

/// Keep [`OverlayStack`] synchronized with live overlay entities.
pub fn sync_overlay_stack_lifecycle(world: &mut World) {
    if !world.contains_resource::<OverlayStack>() {
        world.insert_resource(OverlayStack::default());
    }

    let mut live_overlays = {
        let mut query = world.query_filtered::<Entity, With<OverlayState>>();
        query.iter(world).collect::<Vec<_>>()
    };

    live_overlays.sort_by_key(|entity| entity.index());
    let live_set = live_overlays.iter().copied().collect::<HashSet<_>>();

    let mut next_stack = world
        .resource::<OverlayStack>()
        .active_overlays
        .iter()
        .copied()
        .filter(|entity| live_set.contains(entity))
        .collect::<Vec<_>>();
    for entity in live_overlays {
        if !next_stack.contains(&entity) {
            next_stack.push(entity);
        }
    }

    if world.resource::<OverlayStack>().active_overlays != next_stack {
        world.resource_mut::<OverlayStack>().active_overlays = next_stack;
    }
}

fn first_overlay_root(world: &mut World) -> Option<Entity> {
    let mut query = world.query_filtered::<Entity, With<UiOverlayRoot>>();
    query.iter(world).next()
}

/// Ensure an overlay root exists and return its entity id.
pub fn ensure_overlay_root_entity(world: &mut World) -> Entity {
    if let Some(existing) = first_overlay_root(world) {
        return existing;
    }

    world.spawn((UiRoot, UiOverlayRoot)).id()
}

/// Spawn an entity bundle under the global overlay root.
///
/// This is the recommended entrypoint for app-level modal/dropdown/tooltips.
pub fn spawn_in_overlay_root<B: Bundle>(world: &mut World, bundle: B) -> Entity {
    let overlay_root = ensure_overlay_root_entity(world);
    let entity = world.spawn((bundle, ChildOf(overlay_root))).id();

    if world.get::<OverlayState>(entity).is_some()
        || world.get::<UiDialog>(entity).is_some()
        || world.get::<UiDropdownMenu>(entity).is_some()
    {
        push_overlay_to_stack(world, entity);
    }

    entity
}

fn ensure_popover_overlay_components(world: &mut World, entity: Entity, popover: UiPopover) {
    if world.get::<AnchoredTo>(entity).is_none() {
        world.entity_mut(entity).insert(AnchoredTo(popover.anchor));
    }

    ensure_overlay_components(
        world,
        entity,
        OverlayConfig {
            placement: popover.placement,
            anchor: Some(popover.anchor),
            auto_flip: popover.auto_flip_placement,
        },
        OverlayState {
            is_modal: false,
            anchor: Some(popover.anchor),
        },
        Some(OverlayAnchorRect::default()),
    );
}

/// Spawn a generic anchored popover under the global overlay root.
pub fn spawn_popover_in_overlay_root<B: Bundle>(
    world: &mut World,
    bundle: B,
    popover: UiPopover,
) -> Entity {
    spawn_in_overlay_root(
        world,
        (
            bundle,
            popover,
            AnchoredTo(popover.anchor),
            OverlayState {
                is_modal: false,
                anchor: Some(popover.anchor),
            },
            OverlayAnchorRect::default(),
            OverlayConfig {
                placement: popover.placement,
                anchor: Some(popover.anchor),
                auto_flip: popover.auto_flip_placement,
            },
            OverlayComputedPosition::default(),
        ),
    )
}

/// Spawn an overlay at an explicit pixel `(x, y)` position relative to the
/// window's top-left corner, bypassing anchor-based placement.
///
/// This is the manual-positioning entry point for floating panels that are not
/// anchored to an existing widget (e.g. a palette opened at a fixed location).
pub fn spawn_manual_overlay_at<B: Bundle>(world: &mut World, bundle: B, x: f64, y: f64) -> Entity {
    let overlay_root = ensure_overlay_root_entity(world);
    let entity = world
        .spawn((
            bundle,
            ChildOf(overlay_root),
            OverlayState {
                is_modal: false,
                anchor: None,
            },
            OverlayConfig {
                placement: OverlayPlacement::TopStart,
                anchor: None,
                auto_flip: false,
            },
            OverlayComputedPosition {
                x,
                y,
                width: 0.0,
                height: 0.0,
                placement: OverlayPlacement::TopStart,
                is_positioned: true,
            },
        ))
        .id();
    push_overlay_to_stack(world, entity);
    entity
}

fn collect_dropdowns_for_combo(world: &mut World, combo: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &AnchoredTo, &UiDropdownMenu)>();
    query
        .iter(world)
        .filter_map(|(entity, anchored_to, _)| (anchored_to.0 == combo).then_some(entity))
        .collect()
}

fn collect_theme_picker_menus_for_picker(world: &mut World, picker: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &UiThemePickerMenu)>();
    query
        .iter(world)
        .filter_map(|(entity, panel)| (panel.anchor == picker).then_some(entity))
        .collect()
}

fn despawn_entity_tree(world: &mut World, entity: Entity) {
    let children = world
        .get::<Children>(entity)
        .map(|children| children.to_vec())
        .unwrap_or_default();

    for child in children {
        if world.get_entity(child).is_ok() {
            despawn_entity_tree(world, child);
        }
    }

    let _ = world.despawn(entity);
}

fn despawn_overlay_entity(world: &mut World, entity: Entity) {
    despawn_entity_tree(world, entity);
    remove_overlay_from_stack(world, entity);
}

fn dismiss_dialog_overlay(world: &mut World, dialog_entity: Entity) {
    if let Some(mut close_action) = world.get_mut::<crate::UiDialogCloseAction>(dialog_entity)
        && let Some(event) = close_action.take_event()
    {
        world.resource::<UiEventQueue>().push(event);
    }

    despawn_overlay_entity(world, dialog_entity);
}

fn close_anchored_overlay<T: Component<Mutability = Mutable>>(
    world: &mut World,
    overlay_entity: Entity,
    anchor: Option<Entity>,
    reset_owner: impl FnOnce(&mut T),
) {
    despawn_overlay_entity(world, overlay_entity);

    if let Some(anchor) = anchor
        && let Some(mut owner) = world.get_mut::<T>(anchor)
    {
        reset_owner(&mut owner);
    }
}

fn close_dropdown(world: &mut World, dropdown_entity: Entity) {
    let anchor = world
        .get::<AnchoredTo>(dropdown_entity)
        .map(|anchored| anchored.0);

    close_anchored_overlay::<UiComboBox>(world, dropdown_entity, anchor, |combo_box| {
        combo_box.is_open = false;
    });
}

fn spawn_dropdown_items(world: &mut World, dropdown_entity: Entity, combo_entity: Entity) {
    let Some(combo_box) = world.get::<UiComboBox>(combo_entity) else {
        return;
    };

    let selected = combo_box.clamped_selected();
    let item_count = combo_box.options.len();

    for index in 0..item_count {
        let mut classes = vec!["overlay.dropdown.item".to_string()];
        if selected == Some(index) {
            classes.push("overlay.dropdown.item.selected".to_string());
        }

        world.spawn((
            UiDropdownItem {
                dropdown: dropdown_entity,
                index,
            },
            crate::styling::HoverDebounce {
                enter_delay_secs: DROPDOWN_ITEM_HOVER_ENTER_DELAY_SECS,
            },
            crate::StyleClass(classes),
            ChildOf(dropdown_entity),
        ));
    }
}

fn close_theme_picker_menu(world: &mut World, panel_entity: Entity) {
    let anchor = world
        .get::<UiThemePickerMenu>(panel_entity)
        .map(|panel| panel.anchor);

    close_anchored_overlay::<UiThemePicker>(world, panel_entity, anchor, |picker| {
        picker.is_open = false;
    });
}

fn ensure_overlay_components(
    world: &mut World,
    entity: Entity,
    config: OverlayConfig,
    state: OverlayState,
    anchor_rect: Option<OverlayAnchorRect>,
) {
    let needs_config = world.get::<OverlayConfig>(entity).is_none();
    let needs_state = world.get::<OverlayState>(entity).is_none();
    let needs_position = world.get::<OverlayComputedPosition>(entity).is_none();
    let needs_anchor_rect =
        anchor_rect.is_some() && world.get::<OverlayAnchorRect>(entity).is_none();

    if !(needs_config || needs_state || needs_position || needs_anchor_rect) {
        return;
    }

    let mut entity_mut = world.entity_mut(entity);
    if needs_config {
        entity_mut.insert(config);
    }
    if needs_state {
        entity_mut.insert(state);
    }
    if needs_position {
        entity_mut.insert(OverlayComputedPosition::default());
    }
    if needs_anchor_rect && let Some(anchor_rect) = anchor_rect {
        entity_mut.insert(anchor_rect);
    }
}

/// Ensure a global [`UiOverlayRoot`] exists whenever there is at least one regular [`UiRoot`].
pub fn ensure_overlay_root(world: &mut World) {
    if first_overlay_root(world).is_some() {
        return;
    }

    let has_regular_root = {
        let mut query = world.query_filtered::<Entity, (With<UiRoot>, Without<UiOverlayRoot>)>();
        query.iter(world).next().is_some()
    };

    if !has_regular_root {
        return;
    }

    world.spawn((UiRoot, UiOverlayRoot));
}

/// Ensure built-in overlays have default placement and behavior metadata.
pub fn ensure_overlay_defaults(world: &mut World) {
    let dialogs = {
        let mut query = world.query_filtered::<Entity, With<UiDialog>>();
        query.iter(world).collect::<Vec<_>>()
    };

    for dialog in dialogs {
        ensure_overlay_components(
            world,
            dialog,
            OverlayConfig {
                placement: OverlayPlacement::Center,
                anchor: None,
                auto_flip: false,
            },
            OverlayState {
                is_modal: true,
                anchor: None,
            },
            None,
        );
    }

    let dropdowns = {
        let mut query = world.query::<(Entity, Option<&AnchoredTo>)>();
        query
            .iter(world)
            .filter_map(|(entity, anchored_to)| {
                world
                    .get::<UiDropdownMenu>(entity)
                    .is_some()
                    .then_some((entity, anchored_to.map(|a| a.0)))
            })
            .collect::<Vec<_>>()
    };

    for (dropdown, anchor) in dropdowns {
        if let Some(anchor) = anchor
            && world.get::<UiPopover>(dropdown).is_none()
        {
            world.entity_mut(dropdown).insert(
                UiPopover::new(anchor)
                    .with_placement(OverlayPlacement::BottomStart)
                    .with_auto_flip_placement(true),
            );
        }
    }

    let menu_panels = {
        let mut query = world.query::<(Entity, &UiMenuItemPanel)>();
        query
            .iter(world)
            .map(|(entity, panel)| (entity, panel.anchor))
            .collect::<Vec<_>>()
    };

    for (panel_entity, anchor) in menu_panels {
        if world.get::<UiPopover>(panel_entity).is_none() {
            world.entity_mut(panel_entity).insert(
                UiPopover::new(anchor)
                    .with_placement(OverlayPlacement::BottomStart)
                    .with_auto_flip_placement(true),
            );
        }
    }

    let theme_picker_panels = {
        let mut query = world.query::<(Entity, &UiThemePickerMenu)>();
        query
            .iter(world)
            .map(|(entity, panel)| (entity, panel.anchor))
            .collect::<Vec<_>>()
    };

    for (panel_entity, anchor) in theme_picker_panels {
        if world.get::<UiPopover>(panel_entity).is_none() {
            world.entity_mut(panel_entity).insert(
                UiPopover::new(anchor)
                    .with_placement(OverlayPlacement::BottomEnd)
                    .with_auto_flip_placement(true),
            );
        }
    }

    let color_picker_panels = {
        let mut query = world.query::<(Entity, &UiColorPickerPanel)>();
        query
            .iter(world)
            .map(|(entity, panel)| (entity, panel.anchor))
            .collect::<Vec<_>>()
    };

    for (panel_entity, anchor) in color_picker_panels {
        if world.get::<UiPopover>(panel_entity).is_none() {
            world.entity_mut(panel_entity).insert(
                UiPopover::new(anchor)
                    .with_placement(OverlayPlacement::BottomStart)
                    .with_auto_flip_placement(true),
            );
        }
    }

    let date_picker_panels = {
        let mut query = world.query::<(Entity, &UiDatePickerPanel)>();
        query
            .iter(world)
            .map(|(entity, panel)| (entity, panel.anchor))
            .collect::<Vec<_>>()
    };

    for (panel_entity, anchor) in date_picker_panels {
        if world.get::<UiPopover>(panel_entity).is_none() {
            world.entity_mut(panel_entity).insert(
                UiPopover::new(anchor)
                    .with_placement(OverlayPlacement::BottomStart)
                    .with_auto_flip_placement(true),
            );
        }
    }

    let time_picker_panels = {
        let mut query = world.query::<(Entity, &UiTimePickerPanel)>();
        query
            .iter(world)
            .map(|(entity, panel)| (entity, panel.anchor))
            .collect::<Vec<_>>()
    };

    for (panel_entity, anchor) in time_picker_panels {
        if world.get::<UiPopover>(panel_entity).is_none() {
            world.entity_mut(panel_entity).insert(
                UiPopover::new(anchor)
                    .with_placement(OverlayPlacement::BottomStart)
                    .with_auto_flip_placement(true),
            );
        }
    }

    let tooltips = {
        let mut query = world.query::<(Entity, &UiTooltip)>();
        query
            .iter(world)
            .map(|(entity, tooltip)| (entity, tooltip.anchor))
            .collect::<Vec<_>>()
    };

    for (tooltip_entity, anchor) in tooltips {
        if world.get::<UiPopover>(tooltip_entity).is_none() {
            world.entity_mut(tooltip_entity).insert(
                UiPopover::new(anchor)
                    .with_placement(OverlayPlacement::Top)
                    .with_auto_flip_placement(true),
            );
        }
    }

    let popovers = {
        let mut query = world.query::<(Entity, &UiPopover)>();
        query
            .iter(world)
            .map(|(entity, popover)| (entity, *popover))
            .collect::<Vec<_>>()
    };

    for (entity, popover) in popovers {
        ensure_popover_overlay_components(world, entity, popover);
    }

    let toasts = {
        let mut query = world.query::<(Entity, &UiToast)>();
        query
            .iter(world)
            .map(|(entity, toast)| {
                (
                    entity,
                    toast.duration_secs,
                    toast.placement,
                    toast.auto_flip_placement,
                )
            })
            .collect::<Vec<_>>()
    };

    for (toast_entity, duration_secs, placement, auto_flip) in toasts {
        ensure_overlay_components(
            world,
            toast_entity,
            OverlayConfig {
                placement,
                anchor: None,
                auto_flip,
            },
            OverlayState {
                is_modal: false,
                anchor: None,
            },
            None,
        );

        if duration_secs > 0.0 {
            if world.get::<AutoDismiss>(toast_entity).is_none() {
                world
                    .entity_mut(toast_entity)
                    .insert(AutoDismiss::from_seconds(duration_secs));
            }
        } else if world.get::<AutoDismiss>(toast_entity).is_some() {
            world.entity_mut(toast_entity).remove::<AutoDismiss>();
        }
    }

    sync_overlay_stack_lifecycle(world);
}

/// Move built-in overlay entities under [`UiOverlayRoot`], creating one if needed.
///
/// This keeps modal/dropdown ownership internal to the library and avoids app-level
/// overlay root plumbing for common cases.
pub fn reparent_overlay_entities(world: &mut World) {
    let overlay_entities = {
        let mut query = world.query_filtered::<Entity, (
            Or<(
                With<UiDialog>,
                With<UiDropdownMenu>,
                With<UiMenuItemPanel>,
                With<UiThemePickerMenu>,
                With<UiColorPickerPanel>,
                With<UiDatePickerPanel>,
                With<UiTimePickerPanel>,
                With<UiContextMenu>,
                With<UiToast>,
                With<UiPopover>,
                With<UiTooltip>,
            )>,
            Without<UiOverlayRoot>,
        )>();
        query.iter(world).collect::<Vec<_>>()
    };

    if overlay_entities.is_empty() {
        return;
    }

    let overlay_root = ensure_overlay_root_entity(world);

    for entity in overlay_entities {
        let already_parented = world
            .get::<ChildOf>(entity)
            .is_some_and(|child_of| child_of.parent() == overlay_root);
        if already_parented {
            if world.get::<OverlayState>(entity).is_some() {
                push_overlay_to_stack(world, entity);
            }
            continue;
        }

        if world.get_entity(entity).is_ok() {
            world.entity_mut(entity).insert(ChildOf(overlay_root));

            if world.get::<OverlayState>(entity).is_some() {
                push_overlay_to_stack(world, entity);
            }
        }
    }
}

fn collect_menu_panels_for_item(world: &mut World, anchor: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &UiMenuItemPanel)>();
    query
        .iter(world)
        .filter_map(|(entity, panel)| (panel.anchor == anchor).then_some(entity))
        .collect()
}

fn collect_color_picker_panels_for_picker(world: &mut World, anchor: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &UiColorPickerPanel)>();
    query
        .iter(world)
        .filter_map(|(entity, panel)| (panel.anchor == anchor).then_some(entity))
        .collect()
}

fn collect_date_picker_panels_for_picker(world: &mut World, anchor: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &UiDatePickerPanel)>();
    query
        .iter(world)
        .filter_map(|(entity, panel)| (panel.anchor == anchor).then_some(entity))
        .collect()
}

fn collect_time_picker_panels_for_picker(world: &mut World, anchor: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &UiTimePickerPanel)>();
    query
        .iter(world)
        .filter_map(|(entity, panel)| (panel.anchor == anchor).then_some(entity))
        .collect()
}

fn close_menu_panel(world: &mut World, panel_entity: Entity) {
    let anchor = world.get::<UiMenuItemPanel>(panel_entity).map(|p| p.anchor);
    close_anchored_overlay::<UiMenuBarItem>(world, panel_entity, anchor, |item| {
        item.is_open = false;
    });
}

fn close_color_picker_panel(world: &mut World, panel_entity: Entity) {
    let anchor = world
        .get::<UiColorPickerPanel>(panel_entity)
        .map(|p| p.anchor);
    close_anchored_overlay::<UiColorPicker>(world, panel_entity, anchor, |picker| {
        picker.is_open = false;
    });
}

fn close_date_picker_panel(world: &mut World, panel_entity: Entity) {
    let anchor = world
        .get::<UiDatePickerPanel>(panel_entity)
        .map(|p| p.anchor);
    close_anchored_overlay::<UiDatePicker>(world, panel_entity, anchor, |picker| {
        picker.is_open = false;
    });
}

fn close_time_picker_panel(world: &mut World, panel_entity: Entity) {
    let anchor = world
        .get::<UiTimePickerPanel>(panel_entity)
        .map(|p| p.anchor);
    close_anchored_overlay::<UiTimePicker>(world, panel_entity, anchor, |picker| {
        picker.is_open = false;
    });
}

fn close_context_menu(world: &mut World, menu_entity: Entity) {
    despawn_overlay_entity(world, menu_entity);
}

fn close_overlay_entity(world: &mut World, overlay_entity: Entity) {
    if world.get::<UiDialog>(overlay_entity).is_some() {
        dismiss_dialog_overlay(world, overlay_entity);
    } else if world.get::<UiDropdownMenu>(overlay_entity).is_some() {
        close_dropdown(world, overlay_entity);
    } else if world.get::<UiThemePickerMenu>(overlay_entity).is_some() {
        close_theme_picker_menu(world, overlay_entity);
    } else if world.get::<UiMenuItemPanel>(overlay_entity).is_some() {
        close_menu_panel(world, overlay_entity);
    } else if world.get::<UiColorPickerPanel>(overlay_entity).is_some() {
        close_color_picker_panel(world, overlay_entity);
    } else if world.get::<UiDatePickerPanel>(overlay_entity).is_some() {
        close_date_picker_panel(world, overlay_entity);
    } else if world.get::<UiTimePickerPanel>(overlay_entity).is_some() {
        close_time_picker_panel(world, overlay_entity);
    } else if world.get::<UiContextMenu>(overlay_entity).is_some() {
        close_context_menu(world, overlay_entity);
    } else {
        despawn_overlay_entity(world, overlay_entity);
    }
}

/// Consume built-in overlay actions and mutate ECS overlay state.
pub fn handle_overlay_actions(world: &mut World) {
    let actions = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<OverlayUiAction>();

    for event in actions {
        if world.get_entity(event.entity).is_err() {
            continue;
        }

        match event.action {
            OverlayUiAction::DismissDialog => {
                if world.get::<UiDialog>(event.entity).is_some() {
                    dismiss_dialog_overlay(world, event.entity);
                }
            }
            OverlayUiAction::ToggleCombo => {
                let Some(combo) = world.get::<UiComboBox>(event.entity).cloned() else {
                    continue;
                };

                let existing_dropdowns = collect_dropdowns_for_combo(world, event.entity);
                for dropdown in existing_dropdowns {
                    if world.get_entity(dropdown).is_ok() {
                        close_dropdown(world, dropdown);
                    }
                }

                if combo.is_open {
                    if let Some(mut combo_box) = world.get_mut::<UiComboBox>(event.entity) {
                        combo_box.is_open = false;
                    }
                    continue;
                }

                let placement = combo.dropdown_placement;
                let auto_flip = combo.auto_flip_placement;

                let dropdown = spawn_popover_in_overlay_root(
                    world,
                    UiDropdownMenu,
                    UiPopover::new(event.entity)
                        .with_placement(placement)
                        .with_auto_flip_placement(auto_flip),
                );

                spawn_dropdown_items(world, dropdown, event.entity);

                if let Some(mut combo_box) = world.get_mut::<UiComboBox>(event.entity) {
                    combo_box.is_open = true;
                }
            }
            action @ OverlayUiAction::SelectComboItem { dropdown, index } => {
                tracing::info!("ComboBox Item Clicked: {:?}", action);

                if world.get_entity(dropdown).is_err() {
                    continue;
                }

                let Some(anchor) = world.get::<AnchoredTo>(dropdown).map(|anchored| anchored.0)
                else {
                    continue;
                };

                let mut changed_event = None;
                if let Some(mut combo_box) = world.get_mut::<UiComboBox>(anchor)
                    && !combo_box.options.is_empty()
                {
                    let selected = index.min(combo_box.options.len() - 1);
                    combo_box.selected = selected;
                    changed_event = Some(UiComboBoxChanged {
                        combo: anchor,
                        selected,
                        value: combo_box.options[selected].value.clone(),
                    });
                }

                if let Some(changed_event) = changed_event {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(anchor, changed_event);
                }

                close_dropdown(world, dropdown);
            }
            OverlayUiAction::DismissDropdown => {
                if world.get_entity(event.entity).is_ok()
                    && world.get::<UiDropdownMenu>(event.entity).is_some()
                {
                    close_dropdown(world, event.entity);
                }
            }

            OverlayUiAction::ToggleThemePicker => {
                let Some(picker) = world.get::<UiThemePicker>(event.entity).cloned() else {
                    continue;
                };

                let existing_panels = collect_theme_picker_menus_for_picker(world, event.entity);
                for panel in existing_panels {
                    if world.get_entity(panel).is_ok() {
                        close_theme_picker_menu(world, panel);
                    }
                }

                if picker.is_open {
                    if let Some(mut theme_picker) = world.get_mut::<UiThemePicker>(event.entity) {
                        theme_picker.is_open = false;
                    }
                    continue;
                }

                if picker.options.is_empty() {
                    continue;
                }

                spawn_popover_in_overlay_root(
                    world,
                    UiThemePickerMenu {
                        anchor: event.entity,
                    },
                    UiPopover::new(event.entity)
                        .with_placement(picker.dropdown_placement)
                        .with_auto_flip_placement(picker.auto_flip_placement),
                );

                if let Some(mut theme_picker) = world.get_mut::<UiThemePicker>(event.entity) {
                    theme_picker.is_open = true;
                }
            }

            OverlayUiAction::SelectThemePickerItem { index } => {
                let Some(anchor) = world
                    .get::<UiThemePickerMenu>(event.entity)
                    .map(|panel| panel.anchor)
                else {
                    continue;
                };

                let mut changed_event = None;
                let mut selected_variant = None;
                if let Some(mut picker) = world.get_mut::<UiThemePicker>(anchor)
                    && !picker.options.is_empty()
                {
                    let selected = index.min(picker.options.len() - 1);
                    picker.selected = selected;
                    selected_variant = Some(picker.options[selected].variant.clone());
                    changed_event = Some(UiThemePickerChanged {
                        picker: anchor,
                        selected,
                        variant: picker.options[selected].variant.clone(),
                    });
                }

                if let Some(variant) = selected_variant {
                    set_active_style_variant_by_name(world, variant.as_str());
                }

                if world.get_entity(event.entity).is_ok() {
                    close_theme_picker_menu(world, event.entity);
                }

                if let Some(ev) = changed_event {
                    world.resource::<UiEventQueue>().push_typed(anchor, ev);
                }
            }

            OverlayUiAction::DismissThemePicker => {
                if world.get_entity(event.entity).is_ok()
                    && world.get::<UiThemePickerMenu>(event.entity).is_some()
                {
                    close_theme_picker_menu(world, event.entity);
                }
            }

            OverlayUiAction::ToggleMenuBarItem => {
                let Some(bar_item) = world.get::<UiMenuBarItem>(event.entity).cloned() else {
                    continue;
                };

                let existing_panels = collect_menu_panels_for_item(world, event.entity);
                for panel in existing_panels {
                    if world.get_entity(panel).is_ok() {
                        close_menu_panel(world, panel);
                    }
                }

                if bar_item.is_open {
                    if let Some(mut item) = world.get_mut::<UiMenuBarItem>(event.entity) {
                        item.is_open = false;
                    }
                    continue;
                }

                spawn_popover_in_overlay_root(
                    world,
                    UiMenuItemPanel {
                        anchor: event.entity,
                    },
                    UiPopover::new(event.entity)
                        .with_placement(OverlayPlacement::BottomStart)
                        .with_auto_flip_placement(true),
                );

                if let Some(mut item) = world.get_mut::<UiMenuBarItem>(event.entity) {
                    item.is_open = true;
                }
            }

            OverlayUiAction::SelectMenuBarItem { index } => {
                let Some(anchor) = world.get::<UiMenuItemPanel>(event.entity).map(|p| p.anchor)
                else {
                    continue;
                };

                let mut selected_event = None;
                if let Some(bar_item) = world.get::<UiMenuBarItem>(anchor)
                    && index < bar_item.items.len()
                {
                    let value = bar_item.items[index].value.clone();
                    selected_event = Some(UiMenuItemSelected {
                        bar_item: anchor,
                        value,
                    });
                }

                if world.get_entity(event.entity).is_ok() {
                    close_menu_panel(world, event.entity);
                }

                if let Some(ev) = selected_event {
                    world.resource::<UiEventQueue>().push_typed(anchor, ev);
                }
            }

            OverlayUiAction::DismissMenuBarItem => {
                if world.get_entity(event.entity).is_ok()
                    && world.get::<UiMenuItemPanel>(event.entity).is_some()
                {
                    close_menu_panel(world, event.entity);
                }
            }

            OverlayUiAction::ToggleColorPicker => {
                let Some(color_picker) = world.get::<UiColorPicker>(event.entity).copied() else {
                    continue;
                };

                let existing_panels = collect_color_picker_panels_for_picker(world, event.entity);
                for panel in existing_panels {
                    if world.get_entity(panel).is_ok() {
                        close_color_picker_panel(world, panel);
                    }
                }

                if color_picker.is_open {
                    if let Some(mut picker) = world.get_mut::<UiColorPicker>(event.entity) {
                        picker.is_open = false;
                    }
                    continue;
                }

                spawn_popover_in_overlay_root(
                    world,
                    UiColorPickerPanel {
                        anchor: event.entity,
                    },
                    UiPopover::new(event.entity)
                        .with_placement(OverlayPlacement::BottomStart)
                        .with_auto_flip_placement(true),
                );

                if let Some(mut picker) = world.get_mut::<UiColorPicker>(event.entity) {
                    picker.is_open = true;
                }
            }

            OverlayUiAction::SelectColorSwatch { r, g, b } => {
                let Some(anchor) = world
                    .get::<UiColorPickerPanel>(event.entity)
                    .map(|p| p.anchor)
                else {
                    continue;
                };

                let mut changed_event = None;
                if let Some(mut picker) = world.get_mut::<UiColorPicker>(anchor) {
                    picker.r = r;
                    picker.g = g;
                    picker.b = b;
                    changed_event = Some(UiColorPickerChanged {
                        picker: anchor,
                        r,
                        g,
                        b,
                    });
                }

                if world.get_entity(event.entity).is_ok() {
                    close_color_picker_panel(world, event.entity);
                }

                if let Some(ev) = changed_event {
                    world.resource::<UiEventQueue>().push_typed(anchor, ev);
                }
            }

            OverlayUiAction::DismissColorPicker => {
                if world.get_entity(event.entity).is_ok()
                    && world.get::<UiColorPickerPanel>(event.entity).is_some()
                {
                    close_color_picker_panel(world, event.entity);
                }
            }

            OverlayUiAction::ToggleDatePicker => {
                let Some(date_picker) = world.get::<UiDatePicker>(event.entity).copied() else {
                    continue;
                };

                let existing_panels = collect_date_picker_panels_for_picker(world, event.entity);
                for panel in existing_panels {
                    if world.get_entity(panel).is_ok() {
                        close_date_picker_panel(world, panel);
                    }
                }

                if date_picker.is_open {
                    if let Some(mut picker) = world.get_mut::<UiDatePicker>(event.entity) {
                        picker.is_open = false;
                    }
                    continue;
                }

                spawn_popover_in_overlay_root(
                    world,
                    UiDatePickerPanel {
                        anchor: event.entity,
                        view_year: date_picker.year,
                        view_month: date_picker.month,
                    },
                    UiPopover::new(event.entity)
                        .with_placement(OverlayPlacement::BottomStart)
                        .with_auto_flip_placement(true),
                );

                if let Some(mut picker) = world.get_mut::<UiDatePicker>(event.entity) {
                    picker.is_open = true;
                }
            }

            OverlayUiAction::NavigateDateMonth { forward } => {
                if let Some(mut panel) = world.get_mut::<UiDatePickerPanel>(event.entity) {
                    if forward {
                        if panel.view_month >= 12 {
                            panel.view_month = 1;
                            panel.view_year += 1;
                        } else {
                            panel.view_month += 1;
                        }
                    } else if panel.view_month <= 1 {
                        panel.view_month = 12;
                        panel.view_year -= 1;
                    } else {
                        panel.view_month -= 1;
                    }
                }
            }

            OverlayUiAction::SelectDateDay { day } => {
                let panel_data = world.get::<UiDatePickerPanel>(event.entity).copied();
                let Some(panel) = panel_data else {
                    continue;
                };

                let anchor = panel.anchor;
                let view_year = panel.view_year;
                let view_month = panel.view_month;

                let mut changed_event = None;
                if let Some(mut date_picker) = world.get_mut::<UiDatePicker>(anchor) {
                    date_picker.year = view_year;
                    date_picker.month = view_month;
                    date_picker.day = day;
                    changed_event = Some(UiDatePickerChanged {
                        picker: anchor,
                        year: view_year,
                        month: view_month,
                        day,
                    });
                }

                if world.get_entity(event.entity).is_ok() {
                    close_date_picker_panel(world, event.entity);
                }

                if let Some(ev) = changed_event {
                    world.resource::<UiEventQueue>().push_typed(anchor, ev);
                }
            }

            OverlayUiAction::DismissDatePicker => {
                if world.get_entity(event.entity).is_ok()
                    && world.get::<UiDatePickerPanel>(event.entity).is_some()
                {
                    close_date_picker_panel(world, event.entity);
                }
            }

            // --- Time picker actions ---
            OverlayUiAction::ToggleTimePicker => {
                let Some(time_picker) = world.get::<UiTimePicker>(event.entity).copied() else {
                    continue;
                };

                let existing = collect_time_picker_panels_for_picker(world, event.entity);
                for panel in existing {
                    if world.get_entity(panel).is_ok() {
                        close_time_picker_panel(world, panel);
                    }
                }

                if time_picker.is_open {
                    if let Some(mut picker) = world.get_mut::<UiTimePicker>(event.entity) {
                        picker.is_open = false;
                    }
                    continue;
                }

                spawn_popover_in_overlay_root(
                    world,
                    UiTimePickerPanel {
                        anchor: event.entity,
                        use_24h: time_picker.use_24h,
                    },
                    UiPopover::new(event.entity)
                        .with_placement(OverlayPlacement::BottomStart)
                        .with_auto_flip_placement(true),
                );

                if let Some(mut picker) = world.get_mut::<UiTimePicker>(event.entity) {
                    picker.is_open = true;
                }
            }

            OverlayUiAction::SelectTimeHour { hour } => {
                let Some(anchor) = world
                    .get::<UiTimePickerPanel>(event.entity)
                    .map(|p| p.anchor)
                else {
                    continue;
                };
                if let Some(mut picker) = world.get_mut::<UiTimePicker>(anchor) {
                    picker.hour = hour;
                }
            }

            OverlayUiAction::SelectTimeMinute { minute } => {
                let Some(anchor) = world
                    .get::<UiTimePickerPanel>(event.entity)
                    .map(|p| p.anchor)
                else {
                    continue;
                };
                if let Some(mut picker) = world.get_mut::<UiTimePicker>(anchor) {
                    picker.minute = minute;
                }
            }

            OverlayUiAction::SelectTimePeriod { is_pm } => {
                let Some(anchor) = world
                    .get::<UiTimePickerPanel>(event.entity)
                    .map(|p| p.anchor)
                else {
                    continue;
                };
                if let Some(mut picker) = world.get_mut::<UiTimePicker>(anchor) {
                    let (h12, _) = picker.hour_12();
                    let new_hour = if is_pm {
                        if h12 == 12 { 12 } else { h12 + 12 }
                    } else {
                        if h12 == 12 { 0 } else { h12 }
                    };
                    picker.hour = new_hour.min(23);
                }
            }

            OverlayUiAction::DismissTimePicker => {
                let Some(panel) = world.get::<UiTimePickerPanel>(event.entity).copied() else {
                    continue;
                };
                let anchor = panel.anchor;
                let Some(picker) = world.get::<UiTimePicker>(anchor).copied() else {
                    close_time_picker_panel(world, event.entity);
                    continue;
                };
                let changed = UiTimePickerChanged {
                    picker: anchor,
                    hour: picker.hour,
                    minute: picker.minute,
                    second: picker.second,
                };
                if world.get_entity(event.entity).is_ok() {
                    close_time_picker_panel(world, event.entity);
                }
                world.resource::<UiEventQueue>().push_typed(anchor, changed);
            }

            // --- Expander actions ---
            OverlayUiAction::ToggleExpander => {
                if let Some(mut expander) = world.get_mut::<UiExpander>(event.entity) {
                    expander.is_expanded = !expander.is_expanded;
                    let changed = UiExpanderChanged {
                        expander: event.entity,
                        is_expanded: expander.is_expanded,
                    };
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(event.entity, changed);
                }
            }

            // --- Context menu actions ---
            OverlayUiAction::SelectContextMenuItem { index } => {
                let Some(ctx_menu) = world.get::<UiContextMenu>(event.entity).cloned() else {
                    continue;
                };
                let trigger = ctx_menu.trigger;
                if index < ctx_menu.items.len() {
                    let label = ctx_menu.items[index].label.clone();
                    let selected = UiContextMenuItemSelected {
                        trigger,
                        index,
                        label,
                    };
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(trigger, selected);
                }
                if world.get_entity(event.entity).is_ok() {
                    close_context_menu(world, event.entity);
                }
            }

            OverlayUiAction::DismissContextMenu => {
                if world.get_entity(event.entity).is_ok()
                    && world.get::<UiContextMenu>(event.entity).is_some()
                {
                    close_context_menu(world, event.entity);
                }
            }

            OverlayUiAction::DismissToast => {
                if world.get_entity(event.entity).is_ok() {
                    despawn_entity_tree(world, event.entity);
                }
            }
        }
    }

    sync_overlay_stack_lifecycle(world);
}

#[derive(Debug, Clone, Copy)]
struct EntityHitBox {
    entity: Entity,
    rect: OverlayAnchorRect,
}

fn parse_entity_from_button_view(widget: WidgetRef<'_, dyn Widget>) -> Option<Entity> {
    if widget.short_type_name() != "ActionButtonWidget"
        && widget.short_type_name() != "ActionButtonWithChildWidget"
    {
        return None;
    }

    let debug = widget.get_debug_text()?;
    let bits = debug.strip_prefix("entity=")?.parse::<u64>().ok()?;
    Entity::try_from_bits(bits)
}

fn parse_entity_bits_from_debug(debug: &str) -> Option<u64> {
    if let Some(bits) = debug.strip_prefix("opaque_hitbox_entity=") {
        return bits.parse::<u64>().ok();
    }
    if let Some(bits) = debug.strip_prefix("entity_scope=") {
        return bits.parse::<u64>().ok();
    }
    if let Some(bits) = debug.strip_prefix("entity=") {
        return bits.parse::<u64>().ok();
    }
    None
}

fn collect_entity_hit_boxes(widget: WidgetRef<'_, dyn Widget>, out: &mut Vec<EntityHitBox>) {
    for child in widget.children() {
        collect_entity_hit_boxes(child, out);
    }

    let Some(entity) = parse_entity_from_button_view(widget) else {
        return;
    };

    let ctx = widget.ctx();
    let origin = ctx.to_window(masonry_core::kurbo::Point::ZERO);
    let size = ctx.border_box().size();
    out.push(EntityHitBox {
        entity,
        rect: OverlayAnchorRect {
            left: origin.x,
            top: origin.y,
            width: size.width,
            height: size.height,
        },
    });
}

fn translate_text(world: &World, key: Option<&str>, fallback: &str) -> String {
    match key {
        Some(key) => world.get_resource::<AppI18n>().map_or_else(
            || {
                if fallback.is_empty() {
                    key.to_string()
                } else {
                    fallback.to_string()
                }
            },
            |i18n| i18n.translate(key),
        ),
        None => fallback.to_string(),
    }
}

fn estimate_text_width_px(text: &str, font_size: f32) -> f64 {
    let units = text
        .chars()
        .map(|ch| {
            if ch.is_ascii_whitespace() {
                0.34
            } else if ch.is_ascii() {
                0.56
            } else {
                1.0
            }
        })
        .sum::<f64>();

    (units * font_size as f64).max(font_size as f64 * 2.0)
}

fn estimate_dropdown_surface_width_px<'a>(
    anchor_width: f64,
    labels: impl IntoIterator<Item = &'a str>,
    font_size: f32,
    horizontal_padding: f64,
) -> f64 {
    let widest_label = labels
        .into_iter()
        .map(|label| estimate_text_width_px(label, font_size))
        .fold(0.0, f64::max);

    (widest_label + horizontal_padding + 24.0).max(anchor_width.max(1.0))
}

fn estimate_dropdown_viewport_height_px(
    item_count: usize,
    item_font_size: f32,
    item_padding: f64,
    item_gap: f64,
) -> f64 {
    let per_item = (item_font_size as f64 + item_padding * 2.0 + 8.0).max(28.0);
    let gap_total = item_gap * item_count.saturating_sub(1) as f64;
    let content_height = per_item * item_count as f64 + gap_total;
    content_height.clamp(per_item, DROPDOWN_MAX_VIEWPORT_HEIGHT)
}

fn overlay_size_for_entity(
    world: &World,
    entity: Entity,
    anchor_rects: &HashMap<Entity, OverlayAnchorRect>,
) -> (f64, f64) {
    if let Some(dialog) = world.get::<UiDialog>(entity) {
        let dialog_style = resolve_style_for_classes(world, ["overlay.dialog.surface"]);
        let title_style = resolve_style_for_classes(world, ["overlay.dialog.title"]);
        let body_style = resolve_style_for_classes(world, ["overlay.dialog.body"]);

        let title = translate_text(world, dialog.title_key.as_deref(), &dialog.title);
        let body = translate_text(world, dialog.body_key.as_deref(), &dialog.body);
        let _dismiss_label =
            translate_text(world, dialog.dismiss_key.as_deref(), &dialog.dismiss_label);

        let estimated_width = estimate_dialog_surface_width_px(
            &title,
            &body,
            title_style.text.size,
            body_style.text.size,
            dialog_surface_padding(dialog_style.layout.padding),
        );

        let width = dialog.width.unwrap_or(estimated_width);

        let estimated_height = estimate_dialog_surface_height_px(
            &title,
            &body,
            width,
            title_style.text.size,
            body_style.text.size,
            dialog_surface_gap(dialog_style.layout.gap),
            dialog_surface_padding(dialog_style.layout.padding),
            dialog_surface_padding(dialog_style.layout.padding),
        );

        let height = dialog.height.unwrap_or(estimated_height);

        return (width, height);
    }

    if world.get::<UiDropdownMenu>(entity).is_some() {
        let Some(anchor) = world.get::<AnchoredTo>(entity).map(|a| a.0) else {
            return (220.0, 120.0);
        };

        let Some(combo_box) = world.get::<UiComboBox>(anchor) else {
            return (220.0, 120.0);
        };

        let item_style = resolve_style_for_classes(world, ["overlay.dropdown.item"]);
        let menu_style = resolve_style_for_classes(world, ["overlay.dropdown.menu"]);

        let translated_options = combo_box
            .options
            .iter()
            .map(|option| translate_text(world, option.label_key.as_deref(), &option.label))
            .collect::<Vec<_>>();

        let anchor_width = anchor_rects
            .get(&anchor)
            .map(|rect| rect.width)
            .unwrap_or(160.0);

        let width = estimate_dropdown_surface_width_px(
            anchor_width,
            translated_options.iter().map(String::as_str),
            item_style.text.size,
            item_style.layout.padding * 2.0 + menu_style.layout.padding * 2.0,
        );

        let height = estimate_dropdown_viewport_height_px(
            translated_options.len(),
            item_style.text.size,
            item_style.layout.padding,
            menu_style.layout.gap,
        );

        return (width, height);
    }

    if let Some(panel) = world.get::<UiThemePickerMenu>(entity) {
        let anchor = panel.anchor;
        if let Some(picker) = world.get::<UiThemePicker>(anchor) {
            let item_style = resolve_style_for_classes(world, ["overlay.dropdown.item"]);
            let menu_style = resolve_style_for_classes(world, ["overlay.dropdown.menu"]);

            let translated_options = picker
                .options
                .iter()
                .map(|option| translate_text(world, option.label_key.as_deref(), &option.label))
                .collect::<Vec<_>>();

            let anchor_width = anchor_rects
                .get(&anchor)
                .map(|rect| rect.width)
                .unwrap_or(40.0);

            let width = estimate_dropdown_surface_width_px(
                anchor_width,
                translated_options.iter().map(String::as_str),
                item_style.text.size,
                item_style.layout.padding * 2.0 + menu_style.layout.padding * 2.0 + 18.0,
            );

            let height = estimate_dropdown_viewport_height_px(
                translated_options.len().max(1),
                item_style.text.size,
                item_style.layout.padding,
                menu_style.layout.gap,
            );

            return (width, height);
        }

        return (180.0, 48.0);
    }

    if let Some(panel) = world.get::<UiMenuItemPanel>(entity) {
        let anchor = panel.anchor;
        if let Some(bar_item) = world.get::<UiMenuBarItem>(anchor) {
            let item_style = resolve_style_for_classes(world, ["overlay.dropdown.item"]);
            let menu_style = resolve_style_for_classes(world, ["overlay.dropdown.menu"]);
            let anchor_width = anchor_rects.get(&anchor).map(|r| r.width).unwrap_or(120.0);
            let labels: Vec<&str> = bar_item.items.iter().map(|i| i.label.as_str()).collect();
            let width = estimate_dropdown_surface_width_px(
                anchor_width,
                labels,
                item_style.text.size,
                item_style.layout.padding * 2.0 + menu_style.layout.padding * 2.0,
            );
            let height = estimate_dropdown_viewport_height_px(
                bar_item.items.len(),
                item_style.text.size,
                item_style.layout.padding,
                menu_style.layout.gap,
            );
            return (width, height);
        }
        return (180.0, 120.0);
    }

    if world.get::<UiColorPickerPanel>(entity).is_some() {
        return (260.0, 200.0);
    }

    if world.get::<UiDatePickerPanel>(entity).is_some() {
        return (280.0, 300.0);
    }

    if world.get::<UiTimePickerPanel>(entity).is_some() {
        return (220.0, 300.0);
    }

    if let Some(ctx_menu) = world.get::<UiContextMenu>(entity) {
        let item_style = resolve_style_for_classes(world, ["overlay.context_menu.item"]);
        let labels: Vec<&str> = ctx_menu.items.iter().map(|i| i.label.as_str()).collect();
        let width = estimate_dropdown_surface_width_px(
            160.0,
            labels,
            item_style.text.size,
            item_style.layout.padding * 2.0 + 16.0,
        );
        let height = estimate_dropdown_viewport_height_px(
            ctx_menu.items.len(),
            item_style.text.size,
            item_style.layout.padding,
            4.0,
        );
        return (width.max(160.0), height.max(48.0));
    }

    if let Some(toast) = world.get::<UiToast>(entity) {
        let style = resolve_style(world, entity);

        let text_width = estimate_text_width_px(&toast.message, style.text.size);
        let min_width = toast.min_width.max(120.0);
        let max_width = toast.max_width.max(min_width);
        let width = (text_width + style.layout.padding * 2.0 + 52.0).clamp(min_width, max_width);
        let line_height = (style.text.size as f64 * 1.35).max(20.0);
        let height = (line_height + style.layout.padding * 2.0).max(44.0);
        return (width, height);
    }

    if let Some(tooltip) = world.get::<UiTooltip>(entity) {
        let style = resolve_style_for_classes(world, ["overlay.tooltip"]);

        let text_width = estimate_text_width_px(&tooltip.text, style.text.size);
        let width = (text_width + style.layout.padding * 2.0).clamp(96.0, 360.0);
        let line_height = (style.text.size as f64 * 1.35).max(18.0);
        let height = (line_height + style.layout.padding * 2.0).max(28.0);
        return (width, height);
    }

    if let Some(popover) = world.get::<UiPopover>(entity) {
        return popover.size_hint();
    }

    (240.0, 120.0)
}

fn overlay_origin_for_placement(
    placement: OverlayPlacement,
    anchor_rect: OverlayAnchorRect,
    overlay_width: f64,
    overlay_height: f64,
    gap: f64,
) -> (f64, f64) {
    let start_x = anchor_rect.left;
    let centered_x = anchor_rect.left + (anchor_rect.width - overlay_width) * 0.5;
    let end_x = anchor_rect.left + anchor_rect.width - overlay_width;

    let top_y = anchor_rect.top - overlay_height - gap;
    let centered_y = anchor_rect.top + (anchor_rect.height - overlay_height) * 0.5;
    let bottom_y = anchor_rect.top + anchor_rect.height + gap;

    match placement {
        OverlayPlacement::Center => (centered_x, centered_y),
        OverlayPlacement::Top => (centered_x, top_y),
        OverlayPlacement::Bottom => (centered_x, bottom_y),
        OverlayPlacement::Left => (anchor_rect.left - overlay_width - gap, centered_y),
        OverlayPlacement::Right => (anchor_rect.left + anchor_rect.width + gap, centered_y),
        OverlayPlacement::TopStart => (start_x, top_y),
        OverlayPlacement::TopEnd => (end_x, top_y),
        OverlayPlacement::BottomStart => (start_x, bottom_y),
        OverlayPlacement::BottomEnd => (end_x, bottom_y),
        OverlayPlacement::LeftStart => (anchor_rect.left - overlay_width - gap, anchor_rect.top),
        OverlayPlacement::RightStart => {
            (anchor_rect.left + anchor_rect.width + gap, anchor_rect.top)
        }
    }
}

fn flip_placement(placement: OverlayPlacement) -> Option<OverlayPlacement> {
    Some(match placement {
        OverlayPlacement::Top => OverlayPlacement::Bottom,
        OverlayPlacement::Bottom => OverlayPlacement::Top,
        OverlayPlacement::TopStart => OverlayPlacement::BottomStart,
        OverlayPlacement::TopEnd => OverlayPlacement::BottomEnd,
        OverlayPlacement::BottomStart => OverlayPlacement::TopStart,
        OverlayPlacement::BottomEnd => OverlayPlacement::TopEnd,
        OverlayPlacement::Left => OverlayPlacement::Right,
        OverlayPlacement::Right => OverlayPlacement::Left,
        OverlayPlacement::LeftStart => OverlayPlacement::RightStart,
        OverlayPlacement::RightStart => OverlayPlacement::LeftStart,
        OverlayPlacement::Center => return None,
    })
}

fn visible_area(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_width: f64,
    viewport_height: f64,
) -> f64 {
    let left = x.max(0.0);
    let top = y.max(0.0);
    let right = (x + width).min(viewport_width);
    let bottom = (y + height).min(viewport_height);

    let visible_width = (right - left).max(0.0);
    let visible_height = (bottom - top).max(0.0);
    visible_width * visible_height
}

fn overflows_bottom(y: f64, height: f64, viewport_height: f64) -> bool {
    y + height > viewport_height
}

fn clamp_overlay_origin(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_width: f64,
    viewport_height: f64,
) -> (f64, f64) {
    let max_x = (viewport_width - width).max(0.0);
    let max_y = (viewport_height - height).max(0.0);
    (x.clamp(0.0, max_x), y.clamp(0.0, max_y))
}

/// Universal placement + collision-detection system for overlay entities.
///
/// Runs after layout/input updates and computes final window-space coordinates that
/// projectors apply to overlay surfaces.
pub fn sync_overlay_positions(world: &mut World) {
    let overlays = {
        let mut query = world.query::<(Entity, &OverlayState, Option<&OverlayConfig>)>();
        query
            .iter(world)
            .map(|(entity, state, config)| (entity, *state, config.copied()))
            .collect::<Vec<_>>()
    };

    if overlays.is_empty() {
        return;
    }

    let (viewport_width, viewport_height, _viewport_scale_factor) = {
        let mut primary_window_query = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let primary_window = primary_window_query.iter(world).next();

        let window = if let Some(window) = primary_window {
            window
        } else {
            let mut window_query = world.query::<&Window>();
            let Some(window) = window_query.iter(world).next() else {
                tracing::error!("sync_overlay_positions could not find any Window entity");
                return;
            };
            window
        };

        let window_width = window.width() as f64;
        let window_height = window.height() as f64;
        let window_scale_factor = window.scale_factor() as f64;

        (window_width, window_height, window_scale_factor)
    };

    let hit_boxes = {
        let Some(runtime) = world.get_non_send::<MasonryRuntime>() else {
            return;
        };
        let Some(window_runtime) = runtime.primary() else {
            return;
        };
        let root = window_runtime.render_root.get_layer_root(0);
        let mut boxes = Vec::new();
        collect_entity_hit_boxes(root, &mut boxes);
        boxes
    };

    let mut anchor_rects = HashMap::new();
    for hit in hit_boxes {
        anchor_rects.insert(hit.entity, hit.rect);
    }

    let mut stale_overlays = Vec::new();

    for (entity, state, config) in overlays {
        if world.get_entity(entity).is_err() {
            continue;
        }

        let preferred_placement =
            config
                .map(|cfg| cfg.placement)
                .unwrap_or(if state.anchor.is_some() {
                    OverlayPlacement::BottomStart
                } else {
                    OverlayPlacement::Center
                });
        let auto_flip = config
            .map(|cfg| cfg.auto_flip)
            .unwrap_or(state.anchor.is_some());
        let anchor_entity = state.anchor.or(config.and_then(|cfg| cfg.anchor));

        let (width, height) = overlay_size_for_entity(world, entity, &anchor_rects);

        let (anchor_rect, anchor_gap) = if let Some(anchor) = anchor_entity {
            let Some(anchor_rect) = anchor_rects.get(&anchor).copied() else {
                tracing::warn!(
                    "Anchor entity {:?} geometry resolution failed (missing GlobalTransform/Node/hit-box)",
                    anchor
                );
                stale_overlays.push(entity);
                continue;
            };
            tracing::trace!(
                "Anchor entity {:?} global bounds: {:?}",
                anchor,
                anchor_rect
            );
            (anchor_rect, OVERLAY_ANCHOR_GAP)
        } else {
            (
                OverlayAnchorRect {
                    left: 0.0,
                    top: 0.0,
                    width: viewport_width,
                    height: viewport_height,
                },
                0.0,
            )
        };

        let mut chosen_placement = preferred_placement;
        let mut _did_flip = false;
        let (mut x, mut y) = overlay_origin_for_placement(
            preferred_placement,
            anchor_rect,
            width,
            height,
            anchor_gap,
        );

        if auto_flip
            && overflows_bottom(y, height, viewport_height)
            && let Some(flipped) = flip_placement(preferred_placement)
        {
            let (fx, fy) =
                overlay_origin_for_placement(flipped, anchor_rect, width, height, anchor_gap);

            let preferred_visible =
                visible_area(x, y, width, height, viewport_width, viewport_height);
            let flipped_visible =
                visible_area(fx, fy, width, height, viewport_width, viewport_height);

            if flipped_visible > preferred_visible {
                x = fx;
                y = fy;
                chosen_placement = flipped;
                _did_flip = true;
            }
        }

        let (x, y) = clamp_overlay_origin(
            x,
            y,
            width,
            height,
            viewport_width.max(1.0),
            viewport_height.max(1.0),
        );

        let next_computed = OverlayComputedPosition {
            x,
            y,
            width,
            height,
            placement: chosen_placement,
            is_positioned: true,
        };

        if let Some(mut computed) = world.get_mut::<OverlayComputedPosition>(entity) {
            if *computed != next_computed {
                *computed = next_computed;
            }
        } else {
            world.entity_mut(entity).insert(next_computed);
        }

        tracing::trace!(
            "Applied overlay position to projection state for {:?}: x={}, y={}, w={}, h={}",
            entity,
            x,
            y,
            width,
            height
        );

        if let Some(anchor) = anchor_entity
            && let Some(anchor_rect) = anchor_rects.get(&anchor).copied()
        {
            if let Some(mut cached_anchor) = world.get_mut::<OverlayAnchorRect>(entity) {
                if *cached_anchor != anchor_rect {
                    *cached_anchor = anchor_rect;
                }
            } else {
                world.entity_mut(entity).insert(anchor_rect);
            }
        }
    }

    for stale in stale_overlays {
        if world.get_entity(stale).is_ok() {
            if world.get::<UiDropdownMenu>(stale).is_some() {
                close_dropdown(world, stale);
            } else if world.get::<UiThemePickerMenu>(stale).is_some() {
                close_theme_picker_menu(world, stale);
            } else if world.get::<UiTimePickerPanel>(stale).is_some() {
                close_time_picker_panel(world, stale);
            } else {
                despawn_entity_tree(world, stale);
                remove_overlay_from_stack(world, stale);
            }
        }
    }

    sync_overlay_stack_lifecycle(world);
}

/// Backward-compatible alias kept for existing callsites.
pub fn sync_dropdown_positions(world: &mut World) {
    sync_overlay_positions(world);
}

fn primary_window_physical_cursor(world: &mut World) -> Option<(Entity, Vec2)> {
    let mut primary_window_query = world.query_filtered::<(Entity, &Window), With<PrimaryWindow>>();
    if let Some((window_entity, window)) = primary_window_query.iter(world).next()
        && let Some(cursor) = window.physical_cursor_position()
    {
        return Some((window_entity, cursor));
    }

    let mut window_query = world.query::<(Entity, &Window)>();
    let (window_entity, window) = window_query.iter(world).next()?;
    let cursor = window.physical_cursor_position()?;
    Some((window_entity, cursor))
}

/// Spawn a context menu overlay at the current cursor position.
///
/// Returns the overlay entity if a context menu was spawned, `None` otherwise.
fn spawn_context_menu_at_cursor(
    world: &mut World,
    trigger: Entity,
    items: Vec<UiContextMenuItem>,
) -> Option<Entity> {
    let cursor_pos = {
        let mut primary_window_query = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let window = primary_window_query.iter(world).next()?;
        let logical_pos = window.cursor_position()?;
        (logical_pos.x as f64, logical_pos.y as f64)
    };

    let overlay_root = ensure_overlay_root_entity(world);
    let entity = world
        .spawn((
            UiContextMenu { items, trigger },
            ChildOf(overlay_root),
            OverlayState {
                is_modal: false,
                anchor: None,
            },
            OverlayConfig {
                placement: OverlayPlacement::BottomStart,
                anchor: None,
                auto_flip: false,
            },
            OverlayComputedPosition {
                x: cursor_pos.0,
                y: cursor_pos.1,
                width: 0.0,
                height: 0.0,
                placement: OverlayPlacement::BottomStart,
                is_positioned: true,
            },
        ))
        .id();

    push_overlay_to_stack(world, entity);
    Some(entity)
}

/// Centralized native Bevy click interception for layered overlay dismissal + blocking.
pub fn handle_global_overlay_clicks(world: &mut World) {
    let left_just_pressed = {
        let Some(mouse_input) = world.get_resource::<ButtonInput<MouseButton>>() else {
            return;
        };
        mouse_input.just_pressed(MouseButton::Left)
    };

    if !left_just_pressed {
        return;
    }

    sync_overlay_stack_lifecycle(world);

    let top_overlay_entity = {
        let stack = world.resource::<OverlayStack>();
        let Some(top_overlay) = stack.active_overlays.last().copied() else {
            return;
        };
        top_overlay
    };

    if world.get_entity(top_overlay_entity).is_err() {
        sync_overlay_stack_lifecycle(world);
        return;
    }

    let Some((window_entity, cursor_pos)) = primary_window_physical_cursor(world) else {
        return;
    };

    let (top_overlay_widget_ids, preferred_overlay_widget_id) = {
        let Some(runtime) = world.get_non_send::<MasonryRuntime>() else {
            return;
        };
        let Some(window_runtime) = runtime.primary() else {
            return;
        };

        let all = window_runtime.find_widget_ids_for_entity_bits(top_overlay_entity.to_bits());
        let preferred = window_runtime
            .find_widget_id_for_entity_bits(top_overlay_entity.to_bits(), true)
            .or_else(|| {
                window_runtime.find_widget_id_for_entity_bits(top_overlay_entity.to_bits(), false)
            });

        (all, preferred)
    };

    let anchor_entity = world
        .get::<OverlayState>(top_overlay_entity)
        .and_then(|state| state.anchor);

    let (anchor_widget_ids, anchor_widget_id) = anchor_entity
        .and_then(|anchor| {
            world.get_non_send::<MasonryRuntime>().and_then(|runtime| {
                let window_runtime = runtime.primary()?;
                let all = window_runtime.find_widget_ids_for_entity_bits(anchor.to_bits());
                let preferred = window_runtime
                    .find_widget_id_for_entity_bits(anchor.to_bits(), true)
                    .or_else(|| {
                        window_runtime.find_widget_id_for_entity_bits(anchor.to_bits(), false)
                    });
                Some((all, preferred))
            })
        })
        .unwrap_or_default();

    let (hit_path, top_hit_widget_id, top_hit_entity) = {
        let Some(mut runtime) = world.get_non_send_mut::<MasonryRuntime>() else {
            return;
        };
        let Some(window_runtime) = runtime.primary_mut() else {
            return;
        };

        let pointer = (cursor_pos.x as f64, cursor_pos.y as f64).into();
        let _ = window_runtime.render_root.redraw();
        let hit_path = window_runtime.get_hit_path(pointer);
        let (top_hit_widget_id, top_hit_entity) = window_runtime
            .render_root
            .get_layer_root(0)
            .find_widget_under_pointer(pointer)
            .map(|widget| {
                let entity = widget
                    .get_debug_text()
                    .and_then(|debug| parse_entity_bits_from_debug(&debug))
                    .and_then(Entity::try_from_bits);
                (Some(widget.id()), entity)
            })
            .unwrap_or((None, None));

        (hit_path, top_hit_widget_id, top_hit_entity)
    };

    let hit_entities = world
        .get_non_send::<MasonryRuntime>()
        .and_then(|runtime| runtime.primary())
        .map(|window_runtime| {
            hit_path
                .iter()
                .filter_map(|widget_id| {
                    window_runtime
                        .render_root
                        .get_widget(*widget_id)
                        .and_then(|widget| widget.get_debug_text())
                        .and_then(|debug| parse_entity_bits_from_debug(&debug))
                        .and_then(Entity::try_from_bits)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let clicked_inside_overlay_by_hit_path = if let Some(preferred_widget_id) =
        preferred_overlay_widget_id
    {
        top_hit_widget_id == Some(preferred_widget_id) || hit_path.contains(&preferred_widget_id)
    } else {
        top_hit_widget_id.is_some_and(|widget_id| top_overlay_widget_ids.contains(&widget_id))
            || hit_path
                .iter()
                .any(|widget_id| top_overlay_widget_ids.contains(widget_id))
    };

    let positioned_overlay_rect = world
        .get::<OverlayComputedPosition>(top_overlay_entity)
        .copied()
        .filter(|position| position.is_positioned);

    let cursor_inside_overlay_rect = positioned_overlay_rect.is_some_and(|position| {
        position.is_positioned
            && cursor_pos.x as f64 >= position.x
            && cursor_pos.x as f64 <= position.x + position.width
            && cursor_pos.y as f64 >= position.y
            && cursor_pos.y as f64 <= position.y + position.height
    });

    let clicked_anchor_by_widget = top_hit_widget_id
        .is_some_and(|widget_id| anchor_widget_ids.contains(&widget_id))
        || anchor_widget_id.is_some_and(|widget_id| hit_path.contains(&widget_id));

    let clicked_anchor_by_top_hit =
        anchor_entity.is_some_and(|anchor| top_hit_entity == Some(anchor));

    let clicked_anchor_by_rect = anchor_widget_id
        .and_then(|widget_id| {
            world
                .get_non_send::<MasonryRuntime>()
                .and_then(|runtime| runtime.primary())
                .and_then(|window_runtime| window_runtime.get_widget_bounding_box(widget_id))
        })
        .is_some_and(|bounds| {
            let cursor_x = cursor_pos.x as f64;
            let cursor_y = cursor_pos.y as f64;

            cursor_x >= bounds.x0
                && cursor_x <= bounds.x1
                && cursor_y >= bounds.y0
                && cursor_y <= bounds.y1
        });

    let clicked_anchor =
        (clicked_anchor_by_top_hit || clicked_anchor_by_widget || clicked_anchor_by_rect)
            && !clicked_inside_overlay_by_hit_path;

    if clicked_anchor {
        close_overlay_entity(world, top_overlay_entity);

        if let Some(mut routing) = world.get_resource_mut::<OverlayPointerRoutingState>() {
            routing.suppress_click(window_entity, MouseButton::Left);
            tracing::debug!(
                "Closed overlay {:?} by clicking anchor and consumed pointer",
                top_overlay_entity
            );
        }

        sync_overlay_stack_lifecycle(world);
        return;
    }

    let clicked_other_entity = hit_entities
        .iter()
        .any(|entity| *entity != top_overlay_entity);

    let clicked_inside_overlay_by_rect = !clicked_other_entity && cursor_inside_overlay_rect;

    let clicked_inside_overlay =
        clicked_inside_overlay_by_hit_path || clicked_inside_overlay_by_rect;

    if clicked_inside_overlay {
        return;
    }

    // Diagnostic: log the mismatch so we can see what widget was hit vs. what was expected.
    {
        let (_scale_factor, computed_pos, masonry_sf, overlay_subtree) = {
            let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
            let sf = q
                .iter(world)
                .next()
                .map(|w| w.scale_factor() as f64)
                .unwrap_or(1.0);
            let cp = world
                .get::<OverlayComputedPosition>(top_overlay_entity)
                .copied();
            let (masonry_sf, subtree) = world
                .get_non_send::<MasonryRuntime>()
                .and_then(|runtime| runtime.primary())
                .map(|r| {
                    let subtree = preferred_overlay_widget_id
                        .map(|widget_id| r.get_overlay_subtree_info(widget_id))
                        .unwrap_or_default();
                    (r.masonry_scale_factors(), subtree)
                })
                .unwrap_or(((f64::NAN, f64::NAN), vec![]));
            (sf, cp, masonry_sf, subtree)
        };
        let (bevy_window_sf, masonry_global_sf) = masonry_sf;
        let masonry_logical_x = cursor_pos.x as f64 / masonry_global_sf.max(f64::EPSILON);
        let masonry_logical_y = cursor_pos.y as f64 / masonry_global_sf.max(f64::EPSILON);
        tracing::debug!(
            overlay_entity = ?top_overlay_entity,
            expected_widget_id = ?preferred_overlay_widget_id,
            overlay_widget_ids = ?top_overlay_widget_ids,
            hit_path = ?hit_path,
            hit_entities = ?hit_entities,
            physical_cursor_x = cursor_pos.x,
            physical_cursor_y = cursor_pos.y,
            masonry_logical_cursor_x = masonry_logical_x,
            masonry_logical_cursor_y = masonry_logical_y,
            bevy_window_scale_factor = bevy_window_sf,
            masonry_global_scale_factor = masonry_global_sf,
            dialog_computed_x = computed_pos.map(|p| p.x),
            dialog_computed_y = computed_pos.map(|p| p.y),
            dialog_computed_w = computed_pos.map(|p| p.width),
            dialog_computed_h = computed_pos.map(|p| p.height),
            dialog_is_positioned = computed_pos.map(|p| p.is_positioned),
            // (widget_id, bounding_box(x0,y0,x1,y1), is_stashed) for ESW(overlay) and its children
            overlay_subtree_bboxes = ?overlay_subtree
                .iter()
                .map(|(id, bb, stashed)| format!("{id:?} bbox=({:.1},{:.1},{:.1},{:.1}) stashed={stashed}", bb.x0, bb.y0, bb.x1, bb.y1))
                .collect::<Vec<_>>(),
            "overlay_hit_test: overlay dismissed as outside-click (widget ID not found in hit path)"
        );
    }

    close_overlay_entity(world, top_overlay_entity);

    tracing::debug!(
        "Closed overlay {:?} from outside click and allowed pointer propagation",
        top_overlay_entity
    );

    sync_overlay_stack_lifecycle(world);
}

/// Backward-compatible alias kept for existing callsites.
pub fn dismiss_overlays_on_click(world: &mut World) {
    handle_global_overlay_clicks(world);
}

/// Bubble pointer hits up the ECS parent hierarchy, emitting [`UiPointerEvent`] entries.
pub fn bubble_ui_pointer_events(world: &mut World) {
    let hits = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiPointerHitEvent>();

    if hits.is_empty() {
        return;
    }

    for hit in hits {
        if world.get_entity(hit.action.target).is_err() {
            continue;
        }

        let mut current = Some(hit.action.target);

        while let Some(current_entity) = current {
            let consumed = world
                .get::<StopUiPointerPropagation>(current_entity)
                .is_some();

            world.resource::<UiEventQueue>().push(UiEvent::typed(
                current_entity,
                UiPointerEvent {
                    target: hit.action.target,
                    current_target: current_entity,
                    position: hit.action.position,
                    button: hit.action.button,
                    phase: hit.action.phase,
                    consumed,
                },
            ));

            if consumed {
                break;
            }

            current = world
                .get::<ChildOf>(current_entity)
                .map(|child_of| child_of.parent());
        }
    }
}

/// Detect right-click on entities carrying [`UiContextMenuTrigger`] and spawn
/// a context menu overlay at the cursor position.
pub fn handle_context_menu_right_clicks(world: &mut World) {
    let right_just_pressed = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|input| input.just_pressed(MouseButton::Right));

    if !right_just_pressed {
        return;
    }

    // If a context menu is already open, close it first
    {
        let stack = world.resource::<OverlayStack>();
        let existing: Vec<Entity> = stack
            .active_overlays
            .iter()
            .filter(|e| world.get::<UiContextMenu>(**e).is_some())
            .copied()
            .collect();
        let _ = stack;
        for e in existing {
            close_context_menu(world, e);
        }
    }

    // Hit test to find entity under cursor
    let (cursor_x, cursor_y) = {
        let mut window_query = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let Some(window) = window_query.iter(world).next() else {
            return;
        };
        let Some(phys) = window.physical_cursor_position() else {
            return;
        };
        (phys.x as f64, phys.y as f64)
    };

    // Collect widget ids under cursor for entity matching
    let hit_widget_ids: Vec<u64> = {
        let Some(mut runtime) = world.get_non_send_mut::<MasonryRuntime>() else {
            return;
        };
        let Some(window_runtime) = runtime.primary_mut() else {
            return;
        };
        let _ = window_runtime.render_root.redraw();
        let pointer = (cursor_x, cursor_y).into();
        let hit_path = window_runtime.get_hit_path(pointer);
        hit_path
            .iter()
            .filter_map(|id| {
                let debug = window_runtime
                    .render_root
                    .get_widget(*id)?
                    .get_debug_text()?;
                debug
                    .strip_prefix("entity=")
                    .or_else(|| debug.strip_prefix("opaque_hitbox_entity="))
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .collect()
    };

    // Now find the trigger entity from the collected bits
    let hit_entity = hit_widget_ids.iter().find_map(|bits| {
        let entity = Entity::try_from_bits(*bits)?;
        world
            .get::<UiContextMenuTrigger>(entity)
            .is_some()
            .then_some(entity)
    });

    let Some(trigger_entity) = hit_entity else {
        return;
    };

    let items = world
        .get::<UiContextMenuTrigger>(trigger_entity)
        .map(|t| t.items.clone())
        .unwrap_or_default();

    if items.is_empty() {
        return;
    }

    spawn_context_menu_at_cursor(world, trigger_entity, items);
}

/// Keep pseudo-state interaction queue alive when raw pointer events are consumed.
///
/// If we suppress a pointer click before it reaches Masonry, we still clear stale pressed
/// marker transitions to avoid sticky visual states.
pub fn clear_stale_pressed_interactions(world: &mut World) {
    let events = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiInteractionEvent>();

    for event in events {
        world
            .resource::<UiEventQueue>()
            .push_typed(event.entity, event.action);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OVERLAY_ANCHOR_GAP, OverlayAnchorRect, OverlayPlacement, overlay_origin_for_placement,
        overlay_size_for_entity,
    };
    use crate::test_helpers::*;
    use crate::{
        AppPicusExt, PicusPlugin, UiDialog, UiEventQueue, UiRoot, ensure_overlay_defaults,
        ensure_overlay_root, ensure_overlay_root_entity, handle_overlay_actions,
        reparent_overlay_entities, spawn_in_overlay_root,
    };
    use bevy_app::App;
    use bevy_ecs::{hierarchy::ChildOf, prelude::*};
    use bevy_input::{ButtonInput, mouse::MouseButton};
    use bevy_math::{Rect, Vec2};
    use bevy_window::{PrimaryWindow, Window};
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn top_placement_is_horizontally_centered_on_anchor() {
        let anchor = OverlayAnchorRect {
            left: 140.0,
            top: 320.0,
            width: 120.0,
            height: 40.0,
        };

        let (x, y) = overlay_origin_for_placement(
            OverlayPlacement::Top,
            anchor,
            200.0,
            56.0,
            OVERLAY_ANCHOR_GAP,
        );

        assert_eq!(x, 100.0);
        assert_eq!(y, 260.0);
    }

    #[test]
    fn top_start_placement_keeps_anchor_left_edge() {
        let anchor = OverlayAnchorRect {
            left: 96.0,
            top: 200.0,
            width: 180.0,
            height: 32.0,
        };

        let (x, y) = overlay_origin_for_placement(
            OverlayPlacement::TopStart,
            anchor,
            160.0,
            44.0,
            OVERLAY_ANCHOR_GAP,
        );

        assert_eq!(x, 96.0);
        assert_eq!(y, 152.0);
    }

    #[test]
    fn dialog_overlay_size_prefers_fixed_hints() {
        let mut world = World::new();
        let dialog = world
            .spawn((UiDialog::new("title", "body").with_fixed_size(920.0, 760.0),))
            .id();

        let (width, height) = overlay_size_for_entity(&world, dialog, &HashMap::new());

        assert_eq!((width, height), (920.0, 760.0));
    }

    #[test]
    fn ensure_overlay_root_spawns_once() {
        let mut world = World::new();
        world.spawn((UiRoot,));

        ensure_overlay_root(&mut world);
        ensure_overlay_root(&mut world);

        let mut overlay_query = world.query_filtered::<Entity, With<crate::UiOverlayRoot>>();
        let overlays = overlay_query.iter(&world).collect::<Vec<_>>();

        assert_eq!(overlays.len(), 1);
        assert!(world.get::<UiRoot>(overlays[0]).is_some());
    }

    #[test]
    fn overlay_actions_toggle_and_select_combo_box() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let overlay_root = world.spawn((UiRoot, crate::UiOverlayRoot)).id();
        let mut combo_box = crate::UiComboBox::new(vec![
            crate::UiComboOption::new("one", "One"),
            crate::UiComboOption::new("two", "Two"),
        ]);
        combo_box.selected = 0;
        let combo = world.spawn((combo_box,)).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(combo, crate::OverlayUiAction::ToggleCombo);

        handle_overlay_actions(&mut world);

        let mut dropdown_query =
            world.query::<(Entity, &crate::AnchoredTo, &crate::UiDropdownMenu)>();
        let dropdowns = dropdown_query
            .iter(&world)
            .filter_map(|(entity, anchored_to, _)| (anchored_to.0 == combo).then_some(entity))
            .collect::<Vec<_>>();

        assert_eq!(dropdowns.len(), 1);
        let dropdown = dropdowns[0];
        let mut item_query = world.query::<(Entity, &crate::UiDropdownItem, &crate::StyleClass)>();
        let items = item_query
            .iter(&world)
            .filter(|(_, item, _)| item.dropdown == dropdown)
            .map(|(entity, item, classes)| (entity, *item, classes.clone()))
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|(_, item, classes)| {
            item.index == 0
                && classes
                    .0
                    .iter()
                    .any(|class_name| class_name == "overlay.dropdown.item.selected")
        }));

        let second_item = items
            .iter()
            .find_map(|(entity, item, _)| (item.index == 1).then_some(*entity))
            .expect("second dropdown item should exist");
        assert!(
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(dropdown)
                .is_some()
        );
        assert_eq!(
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(dropdown)
                .expect("dropdown should be parented")
                .parent(),
            overlay_root
        );
        assert!(
            world
                .get::<crate::UiComboBox>(combo)
                .expect("combo should exist")
                .is_open
        );

        world.resource::<UiEventQueue>().push_typed(
            second_item,
            crate::OverlayUiAction::SelectComboItem { dropdown, index: 1 },
        );

        handle_overlay_actions(&mut world);

        let combo_after = world
            .get::<crate::UiComboBox>(combo)
            .expect("combo should exist");
        assert_eq!(combo_after.selected, 1);
        assert!(!combo_after.is_open);
        assert!(world.get_entity(dropdown).is_err());
        assert!(world.get_entity(second_item).is_err());
    }

    #[test]
    fn overlay_actions_toggle_and_select_theme_picker() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let overlay_root = world.spawn((UiRoot, crate::UiOverlayRoot)).id();
        let picker = world.spawn((crate::UiThemePicker::fluent(),)).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(picker, crate::OverlayUiAction::ToggleThemePicker);

        handle_overlay_actions(&mut world);

        let mut panel_query = world.query::<(Entity, &crate::UiThemePickerMenu)>();
        let panels = panel_query
            .iter(&world)
            .filter_map(|(entity, panel)| (panel.anchor == picker).then_some(entity))
            .collect::<Vec<_>>();

        assert_eq!(panels.len(), 1);
        let panel = panels[0];
        assert_eq!(
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(panel)
                .expect("theme picker panel should be parented")
                .parent(),
            overlay_root
        );
        assert!(
            world
                .get::<crate::UiThemePicker>(picker)
                .expect("theme picker should exist")
                .is_open
        );

        world.resource::<UiEventQueue>().push_typed(
            panel,
            crate::OverlayUiAction::SelectThemePickerItem { index: 1 },
        );

        handle_overlay_actions(&mut world);

        let picker_after = world
            .get::<crate::UiThemePicker>(picker)
            .expect("theme picker should exist");
        assert_eq!(picker_after.selected, 1);
        assert!(!picker_after.is_open);
        assert!(world.get_entity(panel).is_err());

        let active_variant = world.resource::<crate::ActiveStyleVariant>();
        assert_eq!(active_variant.0.as_deref(), Some("light"));

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiThemePickerChanged>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].entity, picker);
        assert_eq!(changed[0].action.selected, 1);
        assert_eq!(changed[0].action.variant, "light");
    }

    #[test]
    fn overlay_actions_toggle_and_select_color_picker() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let overlay_root = world.spawn((UiRoot, crate::UiOverlayRoot)).id();
        let picker = world.spawn((crate::UiColorPicker::new(12, 34, 56),)).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(picker, crate::OverlayUiAction::ToggleColorPicker);

        handle_overlay_actions(&mut world);

        let mut panel_query = world.query::<(Entity, &crate::UiColorPickerPanel)>();
        let panels = panel_query
            .iter(&world)
            .filter_map(|(entity, panel)| (panel.anchor == picker).then_some(entity))
            .collect::<Vec<_>>();

        assert_eq!(panels.len(), 1);
        let panel = panels[0];
        assert_eq!(
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(panel)
                .expect("color picker panel should be parented")
                .parent(),
            overlay_root
        );
        assert!(
            world
                .get::<crate::UiColorPicker>(picker)
                .expect("color picker should exist")
                .is_open
        );

        world.resource::<UiEventQueue>().push_typed(
            panel,
            crate::OverlayUiAction::SelectColorSwatch {
                r: 200,
                g: 100,
                b: 50,
            },
        );

        handle_overlay_actions(&mut world);

        let picker_after = world
            .get::<crate::UiColorPicker>(picker)
            .expect("color picker should exist");
        assert_eq!(
            (picker_after.r, picker_after.g, picker_after.b),
            (200, 100, 50)
        );
        assert!(!picker_after.is_open);
        assert!(world.get_entity(panel).is_err());

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiColorPickerChanged>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].entity, picker);
        assert_eq!(
            (
                changed[0].action.r,
                changed[0].action.g,
                changed[0].action.b
            ),
            (200, 100, 50)
        );
    }

    #[test]
    fn overlay_actions_toggle_and_select_date_picker() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let overlay_root = world.spawn((UiRoot, crate::UiOverlayRoot)).id();
        let picker = world.spawn((crate::UiDatePicker::new(2026, 3, 17),)).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(picker, crate::OverlayUiAction::ToggleDatePicker);

        handle_overlay_actions(&mut world);

        let mut panel_query = world.query::<(Entity, &crate::UiDatePickerPanel)>();
        let panels = panel_query
            .iter(&world)
            .filter_map(|(entity, panel)| (panel.anchor == picker).then_some(entity))
            .collect::<Vec<_>>();

        assert_eq!(panels.len(), 1);
        let panel = panels[0];
        assert_eq!(
            world
                .get::<bevy_ecs::hierarchy::ChildOf>(panel)
                .expect("date picker panel should be parented")
                .parent(),
            overlay_root
        );
        let panel_state = world
            .get::<crate::UiDatePickerPanel>(panel)
            .expect("date picker panel should exist");
        assert_eq!(panel_state.view_year, 2026);
        assert_eq!(panel_state.view_month, 3);
        assert!(
            world
                .get::<crate::UiDatePicker>(picker)
                .expect("date picker should exist")
                .is_open
        );

        world
            .resource::<UiEventQueue>()
            .push_typed(panel, crate::OverlayUiAction::SelectDateDay { day: 29 });

        handle_overlay_actions(&mut world);

        let picker_after = world
            .get::<crate::UiDatePicker>(picker)
            .expect("date picker should exist");
        assert_eq!(picker_after.year, 2026);
        assert_eq!(picker_after.month, 3);
        assert_eq!(picker_after.day, 29);
        assert!(!picker_after.is_open);
        assert!(world.get_entity(panel).is_err());

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiDatePickerChanged>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].entity, picker);
        assert_eq!(changed[0].action.year, 2026);
        assert_eq!(changed[0].action.month, 3);
        assert_eq!(changed[0].action.day, 29);
    }

    #[test]
    fn spawn_in_overlay_root_parents_entity_under_overlay_root() {
        let mut world = World::new();
        world.spawn((UiRoot,));

        let dialog = spawn_in_overlay_root(&mut world, (crate::UiDialog::new("title", "body"),));

        let overlay_root = ensure_overlay_root_entity(&mut world);
        let parent = world
            .get::<bevy_ecs::hierarchy::ChildOf>(dialog)
            .expect("dialog should be parented")
            .parent();

        assert_eq!(parent, overlay_root);
        assert!(world.get::<crate::UiOverlayRoot>(overlay_root).is_some());
    }

    #[test]
    fn reparent_overlay_entities_moves_dialog_to_overlay_root() {
        let mut world = World::new();
        let app_root = world.spawn((UiRoot,)).id();
        let dialog = world
            .spawn((crate::UiDialog::new("title", "body"), ChildOf(app_root)))
            .id();

        reparent_overlay_entities(&mut world);

        let mut overlays = world.query_filtered::<Entity, With<crate::UiOverlayRoot>>();
        let overlay_root = overlays
            .iter(&world)
            .next()
            .expect("overlay root should exist");

        let parent = world
            .get::<bevy_ecs::hierarchy::ChildOf>(dialog)
            .expect("dialog should be parented")
            .parent();
        assert_eq!(parent, overlay_root);
    }

    #[test]
    fn reparent_overlay_entities_moves_toast_and_tooltip_to_overlay_root_and_tracks_stack() {
        let mut world = World::new();
        world.insert_resource(crate::OverlayStack::default());

        let app_root = world.spawn((UiRoot,)).id();
        let anchor = world.spawn((ChildOf(app_root),)).id();

        let toast = world
            .spawn((
                crate::UiToast::new("Saved"),
                crate::OverlayState {
                    is_modal: false,
                    anchor: None,
                },
                ChildOf(app_root),
            ))
            .id();

        let tooltip = world
            .spawn((
                crate::UiTooltip {
                    text: "Helpful tip".to_string(),
                    anchor,
                },
                crate::OverlayState {
                    is_modal: false,
                    anchor: Some(anchor),
                },
                ChildOf(app_root),
            ))
            .id();

        reparent_overlay_entities(&mut world);

        let mut overlays = world.query_filtered::<Entity, With<crate::UiOverlayRoot>>();
        let overlay_root = overlays
            .iter(&world)
            .next()
            .expect("overlay root should exist");

        let toast_parent = world
            .get::<bevy_ecs::hierarchy::ChildOf>(toast)
            .expect("toast should be parented")
            .parent();
        let tooltip_parent = world
            .get::<bevy_ecs::hierarchy::ChildOf>(tooltip)
            .expect("tooltip should be parented")
            .parent();

        assert_eq!(toast_parent, overlay_root);
        assert_eq!(tooltip_parent, overlay_root);

        let stack = world.resource::<crate::OverlayStack>();
        assert!(stack.active_overlays.contains(&toast));
        assert!(stack.active_overlays.contains(&tooltip));
    }

    #[test]
    fn ensure_overlay_defaults_assigns_built_in_overlay_metadata() {
        let mut world = World::new();
        let combo = world
            .spawn((crate::UiComboBox::new(vec![crate::UiComboOption::new(
                "v", "V",
            )]),))
            .id();
        let dialog = world.spawn((crate::UiDialog::new("t", "b"),)).id();
        let dropdown = world
            .spawn((crate::UiDropdownMenu, crate::AnchoredTo(combo)))
            .id();
        let menu_item = world
            .spawn((crate::UiMenuBarItem::new(
                "File",
                [crate::UiMenuItem::new("Open", "file.open")],
            ),))
            .id();
        let menu_panel = world
            .spawn((crate::UiMenuItemPanel { anchor: menu_item },))
            .id();
        let theme_picker = world.spawn((crate::UiThemePicker::fluent(),)).id();
        let theme_panel = world
            .spawn((crate::UiThemePickerMenu {
                anchor: theme_picker,
            },))
            .id();
        let color_picker = world.spawn((crate::UiColorPicker::new(12, 34, 56),)).id();
        let color_panel = world
            .spawn((crate::UiColorPickerPanel {
                anchor: color_picker,
            },))
            .id();
        let date_picker = world.spawn((crate::UiDatePicker::new(2026, 3, 17),)).id();
        let date_panel = world
            .spawn((crate::UiDatePickerPanel {
                anchor: date_picker,
                view_year: 2026,
                view_month: 3,
            },))
            .id();
        let tooltip_anchor = world.spawn_empty().id();
        let tooltip = world
            .spawn((crate::UiTooltip {
                text: "Helpful tip".to_string(),
                anchor: tooltip_anchor,
            },))
            .id();
        let toast = world
            .spawn((crate::UiToast::new("Saved").with_duration(1.25),))
            .id();
        let custom_toast = world
            .spawn((crate::UiToast::new("Pinned top")
                .with_placement(crate::OverlayPlacement::TopEnd)
                .with_auto_flip_placement(true)
                .with_duration(0.0),))
            .id();
        let persistent_toast = world
            .spawn((
                crate::UiToast::new("Pinned").with_duration(0.0),
                crate::AutoDismiss::from_seconds(2.0),
            ))
            .id();

        ensure_overlay_defaults(&mut world);

        assert_overlay_defaults_for_entity(
            &world,
            dialog,
            "dialog",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::Center,
                anchor: None,
                auto_flip: false,
            },
            crate::OverlayState {
                is_modal: true,
                anchor: None,
            },
            false,
        );
        assert_overlay_defaults_for_entity(
            &world,
            dropdown,
            "dropdown",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::BottomStart,
                anchor: Some(combo),
                auto_flip: true,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: Some(combo),
            },
            true,
        );
        assert_overlay_defaults_for_entity(
            &world,
            menu_panel,
            "menu panel",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::BottomStart,
                anchor: Some(menu_item),
                auto_flip: true,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: Some(menu_item),
            },
            true,
        );
        assert_overlay_defaults_for_entity(
            &world,
            theme_panel,
            "theme picker panel",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::BottomEnd,
                anchor: Some(theme_picker),
                auto_flip: true,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: Some(theme_picker),
            },
            true,
        );
        assert_overlay_defaults_for_entity(
            &world,
            color_panel,
            "color picker panel",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::BottomStart,
                anchor: Some(color_picker),
                auto_flip: true,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: Some(color_picker),
            },
            true,
        );
        assert_overlay_defaults_for_entity(
            &world,
            date_panel,
            "date picker panel",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::BottomStart,
                anchor: Some(date_picker),
                auto_flip: true,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: Some(date_picker),
            },
            true,
        );
        assert_overlay_defaults_for_entity(
            &world,
            tooltip,
            "tooltip",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::Top,
                anchor: Some(tooltip_anchor),
                auto_flip: true,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: Some(tooltip_anchor),
            },
            true,
        );
        assert_overlay_defaults_for_entity(
            &world,
            toast,
            "toast",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::BottomEnd,
                anchor: None,
                auto_flip: false,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: None,
            },
            false,
        );
        assert_overlay_defaults_for_entity(
            &world,
            custom_toast,
            "custom toast",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::TopEnd,
                anchor: None,
                auto_flip: true,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: None,
            },
            false,
        );
        assert_overlay_defaults_for_entity(
            &world,
            persistent_toast,
            "persistent toast",
            crate::OverlayConfig {
                placement: crate::OverlayPlacement::BottomEnd,
                anchor: None,
                auto_flip: false,
            },
            crate::OverlayState {
                is_modal: false,
                anchor: None,
            },
            false,
        );

        let dismiss = world
            .get::<crate::AutoDismiss>(toast)
            .expect("toast should receive auto-dismiss timer");
        assert_eq!(dismiss.timer.duration(), Duration::from_secs_f32(1.25));

        assert!(world.get::<crate::AutoDismiss>(custom_toast).is_none());

        assert!(world.get::<crate::AutoDismiss>(persistent_toast).is_none());
    }

    #[test]
    fn sync_overlay_positions_uses_dynamic_primary_window_size() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(1024.0, 768.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let dialog = app
            .world_mut()
            .spawn((crate::UiDialog::new("title", "body"),))
            .id();

        app.update();

        let initial = *app
            .world()
            .get::<crate::OverlayComputedPosition>(dialog)
            .expect("dialog should have computed position");
        assert!(initial.is_positioned);

        {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
            let mut primary_window = query
                .single_mut(world)
                .expect("primary window should exist");
            primary_window.resolution.set(1600.0, 900.0);
        }

        app.update();

        let resized = *app
            .world()
            .get::<crate::OverlayComputedPosition>(dialog)
            .expect("dialog should still have computed position");

        assert!(resized.x > initial.x);
        assert_eq!(initial.width, resized.width);
        assert_eq!(initial.height, resized.height);
        assert!(resized.is_positioned);
        assert!(resized.x + resized.width <= 1600.0 + f64::EPSILON);
        assert!(resized.y + resized.height <= 900.0 + f64::EPSILON);
    }

    #[test]
    fn sync_overlay_positions_works_without_primary_window_marker() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        app.world_mut().spawn((window,));

        let dialog = app
            .world_mut()
            .spawn((crate::UiDialog::new("title", "body"),))
            .id();

        app.update();

        let computed = *app
            .world()
            .get::<crate::OverlayComputedPosition>(dialog)
            .expect("dialog should have computed position without PrimaryWindow marker");

        assert!(computed.width > 1.0);
        assert!(computed.height > 1.0);
        assert!(computed.x > 0.0);
        assert!(computed.y > 0.0);
        assert!(computed.is_positioned);
    }

    #[test]
    fn dialog_body_click_does_not_dismiss_overlay() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        window.set_cursor_position(Some(Vec2::new(0.0, 0.0)));
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let dialog = spawn_in_overlay_root(app.world_mut(), (crate::UiDialog::new("t", "b"),));

        app.update();
        app.update();

        let computed = app
            .world()
            .get::<crate::OverlayComputedPosition>(dialog)
            .expect("dialog should have computed position");

        let click_position = Vec2::new(
            (computed.x + computed.width * 0.5) as f32,
            (computed.y + 24.0) as f32,
        );

        send_primary_click(&mut app, window_entity, click_position);

        assert!(app.world().get_entity(dialog).is_ok());
    }

    #[test]
    fn dialog_padding_click_is_in_overlay_hit_path_and_does_not_dismiss() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        window.set_cursor_position(Some(Vec2::new(0.0, 0.0)));
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let dialog = spawn_in_overlay_root(app.world_mut(), (crate::UiDialog::new("t", "b"),));

        app.update();

        let opaque_debug = format!("opaque_hitbox_entity={}", dialog.to_bits());
        let opaque_widget_id = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            find_widget_id_by_debug_text(root, &opaque_debug)
                .expect("dialog should project an entity-tagged OpaqueHitboxWidget")
        };

        // Deliberately target a stable inset point inside the opaque panel surface.
        let click_position = widget_inset_point_for_widget_id(&app, opaque_widget_id, 14.0);
        let hit_path = hit_path_for_position(&mut app, window_entity, click_position);
        assert!(hit_path.contains(&opaque_widget_id));

        run_global_overlay_click(&mut app, window_entity, click_position);

        assert!(app.world().get_entity(dialog).is_ok());
    }

    #[test]
    fn dialog_dismiss_button_targets_dialog_entity() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        window.set_cursor_position(Some(Vec2::new(0.0, 0.0)));
        app.world_mut().spawn((window, PrimaryWindow));

        let dialog = spawn_in_overlay_root(app.world_mut(), (crate::UiDialog::new("t", "b"),));

        app.update();

        let computed = app
            .world()
            .get::<crate::OverlayComputedPosition>(dialog)
            .expect("dialog should have computed position");
        let content_rect = Rect::from_corners(
            Vec2::new(computed.x as f32, computed.y as f32),
            Vec2::new(
                (computed.x + computed.width) as f32,
                (computed.y + computed.height) as f32,
            ),
        );

        let button_rect = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            let mut button_rects = Vec::new();
            collect_widget_bounds_by_short_name(
                root,
                "ActionButtonWithChildWidget",
                &mut button_rects,
            );

            button_rects
                .into_iter()
                .filter(|rect| {
                    let width = rect.max.x - rect.min.x;
                    let height = rect.max.y - rect.min.y;
                    width < (content_rect.max.x - content_rect.min.x)
                        && height < (content_rect.max.y - content_rect.min.y)
                })
                .min_by(|a, b| {
                    let area_a = (a.max.x - a.min.x) * (a.max.y - a.min.y);
                    let area_b = (b.max.x - b.min.x) * (b.max.y - b.min.y);
                    area_a.total_cmp(&area_b)
                })
                .expect("dialog should project a dedicated dismiss button")
        };

        let click_position = Vec2::new(
            (button_rect.min.x + button_rect.max.x) * 0.5,
            (button_rect.min.y + button_rect.max.y) * 0.5,
        );

        let (hit_widget, hit_debug_text) = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            root.find_widget_under_pointer(
                (click_position.x as f64, click_position.y as f64).into(),
            )
            .map(|widget| {
                (
                    widget.short_type_name().to_string(),
                    widget.get_debug_text().unwrap_or_default(),
                )
            })
            .unwrap_or_default()
        };

        assert_eq!(hit_widget.as_str(), "ActionButtonWithChildWidget");
        assert_eq!(hit_debug_text, format!("entity={}", dialog.to_bits()));

        let content_width = content_rect.max.x - content_rect.min.x;
        let content_height = content_rect.max.y - content_rect.min.y;
        let button_top = button_rect.min.y;
        let button_right = button_rect.max.x;

        assert!(
            button_right > content_width * 0.82,
            "dismiss button should align against the right side of the dialog header"
        );
        assert!(
            button_top < content_height * 0.22,
            "dismiss button should sit in the top portion of the dialog header"
        );
    }

    #[test]
    fn dialog_projects_single_dismiss_button_without_fullscreen_backdrop_button() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let dialog = spawn_in_overlay_root(app.world_mut(), (crate::UiDialog::new("t", "b"),));

        app.update();

        let computed = app
            .world()
            .get::<crate::OverlayComputedPosition>(dialog)
            .expect("dialog should have computed position");
        let content_rect = Rect::from_corners(
            Vec2::new(computed.x as f32, computed.y as f32),
            Vec2::new(
                (computed.x + computed.width) as f32,
                (computed.y + computed.height) as f32,
            ),
        );

        let button_rects = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            let mut button_rects = Vec::new();
            collect_widget_bounds_by_short_name(
                root,
                "ActionButtonWithChildWidget",
                &mut button_rects,
            );
            button_rects
        };

        assert_eq!(
            button_rects.len(),
            1,
            "dialog projector should only emit the dismiss button, not a structural backdrop button"
        );

        let only_button = button_rects[0];
        let button_area = (only_button.max.x - only_button.min.x).max(0.0)
            * (only_button.max.y - only_button.min.y).max(0.0);
        let content_area = (content_rect.max.x - content_rect.min.x).max(0.0)
            * (content_rect.max.y - content_rect.min.y).max(0.0);

        assert!(button_area < content_area * 0.8);
    }

    #[test]
    fn overlay_action_dismiss_dialog_despawns_dialog() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let dialog = world.spawn((crate::UiDialog::new("title", "body"),)).id();

        world
            .resource::<UiEventQueue>()
            .push_typed(dialog, crate::OverlayUiAction::DismissDialog);

        handle_overlay_actions(&mut world);

        assert!(world.get_entity(dialog).is_err());
    }

    #[test]
    fn overlay_action_dismiss_dialog_emits_optional_close_hook_before_despawn() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let target = world.spawn_empty().id();
        let dialog = world
            .spawn((
                crate::UiDialog::new("title", "body"),
                crate::UiDialogCloseAction::new(target, DialogCloseTestAction::Closed),
            ))
            .id();

        world
            .resource::<UiEventQueue>()
            .push_typed(dialog, crate::OverlayUiAction::DismissDialog);

        handle_overlay_actions(&mut world);

        assert!(world.get_entity(dialog).is_err());

        let events = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<DialogCloseTestAction>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity, target);
        assert_eq!(events[0].action, DialogCloseTestAction::Closed);
    }

    #[test]
    fn handle_global_overlay_clicks_closes_when_clicking_anchor_and_suppresses_pointer() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);
        crate::set_active_style_variant_by_name(app.world_mut(), "dark");

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let combo = app
            .world_mut()
            .spawn((
                crate::UiComboBox::new(vec![
                    crate::UiComboOption::new("one", "One"),
                    crate::UiComboOption::new("two", "Two"),
                ]),
                ChildOf(root),
            ))
            .id();

        {
            let mut combo_state = app
                .world_mut()
                .get_mut::<crate::UiComboBox>(combo)
                .expect("combo should exist");
            combo_state.selected = usize::MAX;
        }

        app.update();

        let dropdown = open_combo_dropdown(&mut app, combo);
        app.update();
        let anchor_center = widget_center_for_entity(&app, combo);

        run_global_overlay_click(&mut app, window_entity, anchor_center);

        assert!(app.world().get_entity(dropdown).is_err());

        let mut routing = app
            .world_mut()
            .resource_mut::<crate::OverlayPointerRoutingState>();
        assert!(routing.take_suppressed_press(window_entity, MouseButton::Left));
        assert!(!routing.take_suppressed_release(window_entity, MouseButton::Left));
    }

    #[test]
    fn handle_global_overlay_clicks_closes_menu_panel_anchor_and_resets_open_state() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(900.0, 680.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let menu_bar = app
            .world_mut()
            .spawn((crate::UiMenuBar, ChildOf(root)))
            .id();
        let menu_item = app
            .world_mut()
            .spawn((
                crate::UiMenuBarItem::new(
                    "File",
                    [
                        crate::UiMenuItem::new("Open", "file.open"),
                        crate::UiMenuItem::new("Save", "file.save"),
                    ],
                ),
                ChildOf(menu_bar),
            ))
            .id();

        app.update();

        app.world()
            .resource::<UiEventQueue>()
            .push_typed(menu_item, crate::OverlayUiAction::ToggleMenuBarItem);
        app.update();

        let panel = {
            let mut query = app.world_mut().query::<(Entity, &crate::UiMenuItemPanel)>();
            query
                .iter(app.world())
                .find_map(|(entity, panel)| (panel.anchor == menu_item).then_some(entity))
                .expect("menu toggle should spawn menu panel")
        };

        assert!(
            app.world()
                .get::<crate::UiMenuBarItem>(menu_item)
                .expect("menu item should exist")
                .is_open
        );

        let anchor_center = widget_center_for_entity(&app, menu_item);
        run_global_overlay_click(&mut app, window_entity, anchor_center);

        assert!(app.world().get_entity(panel).is_err());
        assert!(
            !app.world()
                .get::<crate::UiMenuBarItem>(menu_item)
                .expect("menu item should remain")
                .is_open
        );
    }

    #[test]
    fn handle_global_overlay_clicks_closes_theme_picker_anchor_and_resets_open_state() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(900.0, 680.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let picker = app
            .world_mut()
            .spawn((crate::UiThemePicker::fluent(), ChildOf(root)))
            .id();

        app.update();

        app.world()
            .resource::<UiEventQueue>()
            .push_typed(picker, crate::OverlayUiAction::ToggleThemePicker);
        app.update();

        let panel = {
            let mut query = app
                .world_mut()
                .query::<(Entity, &crate::UiThemePickerMenu)>();
            query
                .iter(app.world())
                .find_map(|(entity, panel)| (panel.anchor == picker).then_some(entity))
                .expect("theme picker toggle should spawn menu panel")
        };

        assert!(
            app.world()
                .get::<crate::UiThemePicker>(picker)
                .expect("theme picker should exist")
                .is_open
        );

        let anchor_center = widget_center_for_entity(&app, picker);
        run_global_overlay_click(&mut app, window_entity, anchor_center);

        assert!(app.world().get_entity(panel).is_err());
        assert!(
            !app.world()
                .get::<crate::UiThemePicker>(picker)
                .expect("theme picker should remain")
                .is_open
        );
    }

    #[test]
    fn overlay_pointer_routing_suppress_click_only_suppresses_press() {
        let mut routing = crate::OverlayPointerRoutingState::default();
        let window = Entity::from_raw_u32(7).expect("test entity index should be valid");

        routing.suppress_click(window, MouseButton::Left);

        assert!(routing.take_suppressed_press(window, MouseButton::Left));
        assert!(!routing.take_suppressed_release(window, MouseButton::Left));
    }

    #[test]
    fn handle_global_overlay_clicks_keeps_overlay_open_when_clicking_inside_overlay() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let combo = app
            .world_mut()
            .spawn((
                crate::UiComboBox::new(vec![
                    crate::UiComboOption::new("one", "One"),
                    crate::UiComboOption::new("two", "Two"),
                ]),
                ChildOf(root),
            ))
            .id();

        app.update();

        let dropdown = open_combo_dropdown(&mut app, combo);
        let dropdown_center = widget_center_for_entity(&app, dropdown);

        run_global_overlay_click(&mut app, window_entity, dropdown_center);

        assert!(app.world().get_entity(dropdown).is_ok());

        let mut routing = app
            .world_mut()
            .resource_mut::<crate::OverlayPointerRoutingState>();
        assert!(!routing.take_suppressed_press(window_entity, MouseButton::Left));
        assert!(!routing.take_suppressed_release(window_entity, MouseButton::Left));
    }

    #[test]
    fn dropdown_padding_click_is_in_overlay_hit_path_and_does_not_dismiss() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let combo = app
            .world_mut()
            .spawn((
                crate::UiComboBox::new(vec![
                    crate::UiComboOption::new("one", "One"),
                    crate::UiComboOption::new("two", "Two"),
                ]),
                ChildOf(root),
            ))
            .id();

        app.update();

        let dropdown = open_combo_dropdown(&mut app, combo);

        let opaque_debug = format!("opaque_hitbox_entity={}", dropdown.to_bits());
        let opaque_widget_id = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            find_widget_id_by_debug_text(root, &opaque_debug)
                .expect("dropdown should project an entity-tagged OpaqueHitboxWidget")
        };

        // Deliberately target menu padding, not option label text.
        let click_position = widget_inset_point_for_widget_id(&app, opaque_widget_id, 6.0);
        let hit_path = hit_path_for_position(&mut app, window_entity, click_position);
        assert!(hit_path.contains(&opaque_widget_id));

        run_global_overlay_click(&mut app, window_entity, click_position);

        assert!(app.world().get_entity(dropdown).is_ok());
    }

    #[test]
    fn dropdown_item_text_region_hits_button_entity_instead_of_child_subwidget() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        app.world_mut().spawn((window, PrimaryWindow));

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let combo = app
            .world_mut()
            .spawn((
                crate::UiComboBox::new(vec![
                    crate::UiComboOption::new("one", "One"),
                    crate::UiComboOption::new("two", "Longer option label"),
                ]),
                ChildOf(root),
            ))
            .id();

        app.update();

        let dropdown = open_combo_dropdown(&mut app, combo);
        app.update();

        let item_entity = {
            let mut query = app.world_mut().query::<(Entity, &crate::UiDropdownItem)>();
            query
                .iter(app.world())
                .find_map(|(entity, item)| {
                    (item.dropdown == dropdown && item.index == 1).then_some(entity)
                })
                .expect("second dropdown item should exist")
        };

        let hit_position = {
            let debug = format!("entity={}", item_entity.to_bits());
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            let widget_id = find_widget_id_by_debug_text(root, &debug)
                .expect("dropdown item button should expose an entity-tagged widget");
            widget_center_for_widget_id(&app, widget_id)
        };
        let (hit_widget, hit_debug_text) = {
            let runtime = app.world().non_send::<crate::MasonryRuntime>();
            let root = runtime
                .primary()
                .expect("primary window runtime should exist")
                .render_root
                .get_layer_root(0);
            root.find_widget_under_pointer((hit_position.x as f64, hit_position.y as f64).into())
                .map(|widget| {
                    (
                        widget.short_type_name().to_string(),
                        widget.get_debug_text().unwrap_or_default(),
                    )
                })
                .unwrap_or_default()
        };

        assert_eq!(hit_widget.as_str(), "ActionButtonWithChildWidget");
        assert_eq!(hit_debug_text, format!("entity={}", item_entity.to_bits()));
    }

    #[test]
    fn handle_global_overlay_clicks_closes_overlay_on_outside_click_without_suppression() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let combo = app
            .world_mut()
            .spawn((
                crate::UiComboBox::new(vec![
                    crate::UiComboOption::new("one", "One"),
                    crate::UiComboOption::new("two", "Two"),
                ]),
                ChildOf(root),
            ))
            .id();

        app.update();

        let dropdown = open_combo_dropdown(&mut app, combo);

        run_global_overlay_click(&mut app, window_entity, Vec2::new(790.0, 590.0));

        assert!(app.world().get_entity(dropdown).is_err());

        let mut routing = app
            .world_mut()
            .resource_mut::<crate::OverlayPointerRoutingState>();
        assert!(!routing.take_suppressed_press(window_entity, MouseButton::Left));
        assert!(!routing.take_suppressed_release(window_entity, MouseButton::Left));
    }

    #[test]
    fn handle_global_overlay_clicks_outside_dialog_emits_same_optional_close_hook() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let target = app.world_mut().spawn_empty().id();
        let dialog = spawn_in_overlay_root(
            app.world_mut(),
            (
                crate::UiDialog::new("title", "body"),
                crate::UiDialogCloseAction::new(target, DialogCloseTestAction::Closed),
            ),
        );

        app.update();
        app.update();

        run_global_overlay_click(&mut app, window_entity, Vec2::new(790.0, 590.0));

        assert!(app.world().get_entity(dialog).is_err());

        let events = app
            .world_mut()
            .resource_mut::<UiEventQueue>()
            .drain_actions::<DialogCloseTestAction>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity, target);
        assert_eq!(events[0].action, DialogCloseTestAction::Closed);
    }

    #[test]
    fn handle_global_overlay_clicks_outside_dialog_without_hook_keeps_existing_behavior() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let dialog =
            spawn_in_overlay_root(app.world_mut(), (crate::UiDialog::new("title", "body"),));

        app.update();
        app.update();

        run_global_overlay_click(&mut app, window_entity, Vec2::new(790.0, 590.0));

        assert!(app.world().get_entity(dialog).is_err());
        assert!(
            app.world_mut()
                .resource_mut::<UiEventQueue>()
                .drain_actions::<DialogCloseTestAction>()
                .is_empty()
        );
    }

    #[test]
    fn handle_global_overlay_clicks_works_without_primary_window_marker() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window,)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let combo = app
            .world_mut()
            .spawn((
                crate::UiComboBox::new(vec![
                    crate::UiComboOption::new("one", "One"),
                    crate::UiComboOption::new("two", "Two"),
                ]),
                ChildOf(root),
            ))
            .id();

        app.update();

        let dropdown = open_combo_dropdown(&mut app, combo);

        run_global_overlay_click(&mut app, window_entity, Vec2::new(790.0, 590.0));

        assert!(app.world().get_entity(dropdown).is_err());
    }

    #[test]
    fn toast_in_overlay_root_is_isolated_from_dropdown_overlay_stack_dismissal() {
        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .register_projector::<ToastProbe>(project_toast_probe);

        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let root = app.world_mut().spawn((UiRoot, crate::UiFlexColumn)).id();
        let combo = app
            .world_mut()
            .spawn((
                crate::UiComboBox::new(vec![
                    crate::UiComboOption::new("one", "One"),
                    crate::UiComboOption::new("two", "Two"),
                ]),
                ChildOf(root),
            ))
            .id();

        app.update();

        let dropdown = open_combo_dropdown(&mut app, combo);
        let toast = spawn_in_overlay_root(app.world_mut(), (ToastProbe,));

        app.update();

        assert!(app.world().get::<crate::OverlayState>(toast).is_none());
        {
            let stack = app.world().resource::<crate::OverlayStack>();
            assert_eq!(stack.active_overlays, vec![dropdown]);
        }

        let toast_center = widget_center_for_entity(&app, toast);
        run_global_overlay_click(&mut app, window_entity, toast_center);

        assert!(app.world().get_entity(dropdown).is_err());
        assert!(app.world().get_entity(toast).is_ok());
        assert!(
            app.world()
                .resource::<crate::OverlayStack>()
                .active_overlays
                .is_empty()
        );

        let mut routing = app
            .world_mut()
            .resource_mut::<crate::OverlayPointerRoutingState>();
        assert!(!routing.take_suppressed_press(window_entity, MouseButton::Left));
        assert!(!routing.take_suppressed_release(window_entity, MouseButton::Left));
    }

    #[test]
    fn handle_global_overlay_clicks_logs_when_window_missing() {
        init_test_tracing();

        let mut world = World::new();
        world.insert_resource(ButtonInput::<MouseButton>::default());

        {
            let mut input = world.resource_mut::<ButtonInput<MouseButton>>();
            input.press(MouseButton::Left);
        }

        let dialog = world
            .spawn((
                crate::UiDialog::new("title", "body"),
                crate::OverlayState {
                    is_modal: true,
                    anchor: None,
                },
            ))
            .id();

        crate::handle_global_overlay_clicks(&mut world);

        assert!(world.get_entity(dialog).is_ok());
    }
}
