//! Gallery event handling — processes typed UI actions from Picus components.
//!
//! In Fluent UI terms, this corresponds to the event handler pattern where
//! component interactions bubble up to a central dispatcher that routes
//! them to the appropriate state management logic.

use bevy_ecs::prelude::*;
use picus_core::{
    BuiltinUiAction, OverlayPlacement, ToastKind, UiCheckboxChanged, UiColorPickerChanged,
    UiComboBoxChanged, UiDataTableSelectionChanged, UiDataTableSortChanged, UiDatePickerChanged,
    UiDialog, UiEventQueue, UiListViewSelectionChanged, UiMenuItemSelected,
    UiMultilineTextInputChanged, UiPasswordInputChanged, UiRadioGroupChanged, UiScrollViewChanged,
    UiSliderChanged, UiSwitchChanged, UiTabBar, UiTabChanged, UiTextInputChanged,
    UiThemePickerChanged, UiToast, UiTreeNodeToggled, spawn_in_overlay_root,
};

use crate::state::{GalleryPage, GalleryRuntime, GalleryState};

/// Main event handler system: drains all UI action queues and updates gallery state.
///
/// Dispatches navigation clicks, dialog/toast triggers, and per-component
/// event logging to the status bar.
pub fn drain_gallery_events(world: &mut World) {
    let Some(rt) = world.get_resource::<GalleryRuntime>().cloned() else {
        return;
    };

    let builtin_events = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<BuiltinUiAction>();
    for event in builtin_events {
        if !matches!(event.action, BuiltinUiAction::Clicked) {
            continue;
        }

        if let Some(index) = rt
            .nav_buttons
            .iter()
            .position(|button| *button == event.entity)
        {
            set_gallery_page(world, &rt, index);
            update_status(
                world,
                format!(
                    "Navigation: switched to {}",
                    GalleryPage::ALL[index].label()
                ),
            );
        } else if event.entity == rt.open_dialog_btn {
            spawn_dialog(
                world,
                "Button Dialog",
                "This replaces MewUI's basic MessageBox with Picus UiDialog.",
            );
        } else if event.entity == rt.persistent_toast_btn {
            spawn_toast(
                world,
                "Persistent info toast. Close it manually.",
                ToastKind::Info,
                0.0,
            );
        } else if event.entity == rt.success_toast_btn {
            spawn_toast(
                world,
                "Selection page success toast.",
                ToastKind::Success,
                2.4,
            );
        } else if event.entity == rt.warning_toast_btn {
            spawn_toast(
                world,
                "Window/Menu placeholder warning.",
                ToastKind::Warning,
                3.2,
            );
        } else if event.entity == rt.error_toast_btn {
            spawn_toast(world, "MessageBox error toast.", ToastKind::Error, 3.2);
        } else if event.entity == rt.prompt_dialog_btn {
            spawn_dialog(
                world,
                "Prompt Placeholder",
                "Picus UiDialog does not yet expose an input slot, so the prompt sample is represented here.",
            );
        } else if event.entity == rt.native_message_btn {
            spawn_dialog(
                world,
                "Native Hook Placeholder",
                "Platform-native message hooks are not part of the public Picus runtime API.",
            );
        } else if event.entity == rt.popover_dialog_btn {
            spawn_dialog(
                world,
                "Popover Note",
                "Anchored overlays are implemented by combo boxes, menus, color pickers, date pickers, and tooltips.",
            );
        } else if event.entity == rt.burst_placeholder_btn {
            spawn_toast(
                world,
                "Confetti placeholder: animated retained canvas is not public yet.",
                ToastKind::Warning,
                3.5,
            );
        }
    }

    // --- Per-component event logging ---

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiThemePickerChanged>()
    {
        update_status(
            world,
            format!(
                "Theme picker {:?}: selected {} ({})",
                event.action.picker, event.action.selected, event.action.variant
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiCheckboxChanged>()
    {
        update_status(
            world,
            format!(
                "CheckBox {:?}: {}",
                event.action.checkbox,
                if event.action.checked {
                    "checked"
                } else {
                    "unchecked"
                }
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiSwitchChanged>()
    {
        update_status(
            world,
            format!(
                "Switch {:?}: {}",
                event.action.switch,
                if event.action.on { "on" } else { "off" }
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiSliderChanged>()
    {
        update_status(
            world,
            format!(
                "Slider {:?}: value {:.2}",
                event.action.slider, event.action.value
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiTextInputChanged>()
    {
        update_status(
            world,
            format!("TextInput {:?}: {}", event.action.input, event.action.value),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiPasswordInputChanged>()
    {
        update_status(
            world,
            format!(
                "PasswordInput {:?}: {} chars",
                event.action.input,
                event.action.value.chars().count()
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiMultilineTextInputChanged>()
    {
        update_status(
            world,
            format!(
                "MultilineTextInput {:?}: {} chars",
                event.action.input,
                event.action.value.chars().count()
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiRadioGroupChanged>()
    {
        update_status(
            world,
            format!(
                "RadioGroup {:?}: index {}",
                event.action.group, event.action.selected
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiComboBoxChanged>()
    {
        update_status(
            world,
            format!(
                "ComboBox {:?}: {} ({})",
                event.action.combo, event.action.selected, event.action.value
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiColorPickerChanged>()
    {
        update_status(
            world,
            format!(
                "ColorPicker {:?}: #{:02X}{:02X}{:02X}",
                event.action.picker, event.action.r, event.action.g, event.action.b
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiDatePickerChanged>()
    {
        update_status(
            world,
            format!(
                "DatePicker {:?}: {:04}-{:02}-{:02}",
                event.action.picker, event.action.year, event.action.month, event.action.day
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiListViewSelectionChanged>()
    {
        update_status(
            world,
            format!(
                "ListView {:?}: selected {:?} rows {:?}",
                event.action.list_view, event.action.selected, event.action.selected_indices
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiDataTableSelectionChanged>()
    {
        update_status(
            world,
            format!(
                "DataTable {:?}: selected {:?}",
                event.action.table, event.action.selected_rows
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiDataTableSortChanged>()
    {
        update_status(
            world,
            format!(
                "DataTable {:?}: sorted column {} {:?}",
                event.action.table, event.action.sort.column, event.action.sort.direction
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiTreeNodeToggled>()
    {
        update_status(
            world,
            format!(
                "TreeNode {:?}: {}",
                event.action.node,
                if event.action.is_expanded {
                    "expanded"
                } else {
                    "collapsed"
                }
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiMenuItemSelected>()
    {
        update_status(
            world,
            format!(
                "Menu item {:?}: {}",
                event.action.bar_item, event.action.value
            ),
        );
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiTabChanged>()
    {
        if event.action.bar != rt.pages_tab_bar {
            update_status(
                world,
                format!(
                    "TabBar {:?}: index {}",
                    event.action.bar, event.action.active
                ),
            );
        }
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiScrollViewChanged>()
    {
        update_status(
            world,
            format!(
                "ScrollView {:?}: offset ({:.1}, {:.1})",
                event.action.scroll_view,
                event.action.scroll_offset.x,
                event.action.scroll_offset.y
            ),
        );
    }
}

/// Switch the active gallery page programmatically (by sidebar navigation).
fn set_gallery_page(world: &mut World, rt: &GalleryRuntime, page: usize) {
    if let Some(mut tab_bar) = world.get_mut::<UiTabBar>(rt.pages_tab_bar) {
        tab_bar.active = page.min(tab_bar.tabs.len().saturating_sub(1));
    }

    for (index, button) in rt.nav_buttons.iter().copied().enumerate() {
        let class_list = if index == page {
            crate::helpers::classes(&["gallery.sidebar_button", "gallery.sidebar_button.active"])
        } else {
            crate::helpers::class("gallery.sidebar_button")
        };
        if world.get_entity(button).is_ok() {
            world.entity_mut(button).insert(class_list);
        }
    }
}

/// Spawn a modal dialog overlay.
fn spawn_dialog(world: &mut World, title: &str, body: &str) {
    spawn_in_overlay_root(world, (UiDialog::new(title, body).with_fixed_width(460.0),));
    update_status(world, format!("Dialog opened: {title}"));
}

/// Spawn a toast notification overlay.
fn spawn_toast(world: &mut World, message: &str, kind: ToastKind, duration: f32) {
    spawn_in_overlay_root(
        world,
        (UiToast::new(message)
            .with_kind(kind)
            .with_duration(duration)
            .with_min_width(320.0)
            .with_max_width(480.0)
            .with_placement(OverlayPlacement::BottomEnd),),
    );
    update_status(world, format!("Toast: {message}"));
}

/// Update the gallery status text.
fn update_status(world: &mut World, text: String) {
    if let Some(mut state) = world.get_resource_mut::<GalleryState>() {
        state.last_event = text;
    }
}
