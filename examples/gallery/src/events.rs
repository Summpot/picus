//! Gallery event handling for interactive showcase controls.

use bevy_ecs::prelude::*;
use picus::{
    AppI18n, BuiltinUiAction, OverlayPlacement, ToastKind, UiCheckboxChanged,
    UiColorPickerChanged, UiComboBoxChanged, UiDataTableSelectionChanged,
    UiDataTableSortChanged, UiDatePickerChanged, UiDialog, UiEventQueue,
    UiListViewSelectionChanged, UiMenuItemSelected, UiMultilineTextInputChanged,
    UiNavigationSelectionChanged, UiNavigationView, UiNumericUpDownChanged,
    UiPasswordInputChanged, UiRadioGroupChanged, UiScrollViewChanged, UiSliderChanged,
    UiSwitchChanged, UiTabChanged, UiTextInputChanged, UiThemePickerChanged, UiToast,
    UiTreeNodeToggled, WindowBackdropMaterial, set_theme_backdrop_material,
    spawn_in_overlay_root,
};

use crate::state::{GalleryBackdropPicker, GalleryButtonAction, GalleryRuntime};

/// Drain UI actions and execute the gallery interactions that have visible effects.
pub fn drain_gallery_events(world: &mut World) {
    let Some(rt) = world.get_resource::<GalleryRuntime>().cloned() else {
        return;
    };

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiNavigationSelectionChanged>()
    {
        set_gallery_page(world, &rt, event.action.selected);
    }

    let builtin_events = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<BuiltinUiAction>();
    for event in builtin_events {
        if !matches!(event.action, BuiltinUiAction::Clicked) {
            continue;
        }

        if event.entity == rt.open_dialog_btn {
            spawn_dialog(
                world,
                "Button Dialog",
                "Demonstrates Picus UiDialog for message boxes.",
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
        } else if let Some(action) = world.get::<GalleryButtonAction>(event.entity).cloned() {
            match action {
                GalleryButtonAction::Toast {
                    message,
                    kind,
                    duration,
                } => spawn_toast(world, &message, kind, duration),
                GalleryButtonAction::Dialog { title, body } => {
                    spawn_dialog(world, &title, &body);
                }
                GalleryButtonAction::Info { message } => {
                    spawn_toast(world, &message, ToastKind::Info, 2.0);
                }
            }
        } else if world
            .get::<crate::pages::overlay::ManualOverlayMarker>(event.entity)
            .is_some()
        {
            picus::spawn_manual_overlay_at(
                world,
                picus::UiDialog::new(
                    "Manual overlay",
                    "This popover was positioned at a fixed (x, y) pixel coordinate via spawn_manual_overlay_at.",
                )
                .with_fixed_width(360.0),
                120.0,
                80.0,
            );
        }
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiRadioGroupChanged>()
    {
        if world.get::<GalleryBackdropPicker>(event.action.group).is_some() {
            let material = match event.action.selected {
                0 => WindowBackdropMaterial::None,
                2 => WindowBackdropMaterial::Acrylic,
                _ => WindowBackdropMaterial::Mica,
            };
            set_theme_backdrop_material(world, material);
        }
    }

    for event in world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<UiComboBoxChanged>()
    {
        if event.entity == rt.locale_combo
            && let Ok(locale) = event
                .action
                .value
                .parse::<unic_langid::LanguageIdentifier>()
        {
            world.resource_mut::<AppI18n>().set_active_locale(locale);
        }
    }

    discard_logged_actions(world);
}

fn discard_logged_actions(world: &mut World) {
    macro_rules! discard {
        ($($action:ty),+ $(,)?) => {
            $(let _ = world.resource_mut::<UiEventQueue>().drain_actions::<$action>();)+
        };
    }

    discard!(
        UiThemePickerChanged,
        UiCheckboxChanged,
        UiSwitchChanged,
        UiSliderChanged,
        UiNumericUpDownChanged,
        UiTextInputChanged,
        UiPasswordInputChanged,
        UiMultilineTextInputChanged,
        UiColorPickerChanged,
        UiDatePickerChanged,
        UiListViewSelectionChanged,
        UiDataTableSelectionChanged,
        UiDataTableSortChanged,
        UiTreeNodeToggled,
        UiMenuItemSelected,
        UiTabChanged,
        UiScrollViewChanged,
    );
}

fn set_gallery_page(world: &mut World, rt: &GalleryRuntime, page: usize) {
    if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(rt.nav_view) {
        nav_view.selected = page.min(nav_view.items.len().saturating_sub(1));
    }
}

fn spawn_dialog(world: &mut World, title: &str, body: &str) {
    spawn_in_overlay_root(world, (UiDialog::new(title, body).with_fixed_width(460.0),));
}

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
}
