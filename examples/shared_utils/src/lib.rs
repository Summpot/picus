use std::sync::OnceLock;

use bevy_xilem::{
    ActiveStyleVariant, BuiltinUiAction, OverlayConfig, OverlayPlacement, OverlayState, StyleClass,
    UiButton, UiEventQueue, bevy_ecs::prelude::*, set_active_style_variant_by_name,
    spawn_in_overlay_root,
};
use tracing_subscriber::{EnvFilter, fmt};

const DEFAULT_LOG_FILTER: &str = "info,wgpu_core=warn,wgpu_hal=warn,wgpu_hal::vulkan=error,bevy_render=warn,bevy_app=warn,masonry::widget=info,xilem_core=info,xilem_masonry=info,xilem_masonry::masonry_root=info,bevy_xilem=debug";

static LOGGING_INITIALIZED: OnceLock<()> = OnceLock::new();
const FLUENT_THEME_VARIANTS: [&str; 3] = ["dark", "light", "high-contrast"];
const FLUENT_THEME_LABELS: [&str; 3] = ["Dark", "Light", "High Contrast"];

#[derive(Resource, Debug, Clone, Copy)]
pub struct FluentThemeToggleRuntime {
    button: Entity,
    active_index: usize,
}

fn theme_index_from_name(name: &str) -> usize {
    FLUENT_THEME_VARIANTS
        .iter()
        .position(|candidate| *candidate == name)
        .unwrap_or(0)
}

fn theme_button_text(index: usize) -> String {
    let label = FLUENT_THEME_LABELS
        .get(index)
        .copied()
        .unwrap_or(FLUENT_THEME_LABELS[0]);
    format!("Fluent: {label}")
}

/// Spawn a reusable floating Fluent theme toggle button.
///
/// The button cycles `dark -> light -> high-contrast -> dark`.
pub fn setup_fluent_theme_toggle(world: &mut World) {
    if let Some(runtime) = world.get_resource::<FluentThemeToggleRuntime>()
        && world.get_entity(runtime.button).is_ok()
    {
        return;
    }

    let active_index = world
        .get_resource::<ActiveStyleVariant>()
        .and_then(|active| active.0.as_deref())
        .map(theme_index_from_name)
        .unwrap_or(0);

    set_active_style_variant_by_name(world, FLUENT_THEME_VARIANTS[active_index]);

    let button = spawn_in_overlay_root(
        world,
        (
            UiButton::new(theme_button_text(active_index)),
            StyleClass(vec!["example.theme-toggle".to_string()]),
            OverlayState {
                is_modal: false,
                anchor: None,
            },
            OverlayConfig {
                placement: OverlayPlacement::TopEnd,
                anchor: None,
                auto_flip: true,
            },
        ),
    );

    world.insert_resource(FluentThemeToggleRuntime {
        button,
        active_index,
    });
}

/// Handle clicks from the shared Fluent theme toggle button.
///
/// This helper preserves unrelated `BuiltinUiAction` events by re-queueing them,
/// so app-level event handlers can continue to consume those actions normally.
pub fn drain_fluent_theme_toggle_events(world: &mut World) {
    let Some(runtime) = world.get_resource::<FluentThemeToggleRuntime>().copied() else {
        return;
    };

    let events = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<BuiltinUiAction>();
    if events.is_empty() {
        return;
    }

    let mut passthrough = Vec::new();
    let mut clicked = false;

    for event in events {
        if event.entity == runtime.button && matches!(event.action, BuiltinUiAction::Clicked) {
            clicked = true;
        } else {
            passthrough.push(event);
        }
    }

    for event in passthrough {
        world
            .resource::<UiEventQueue>()
            .push_typed(event.entity, event.action);
    }

    if !clicked {
        return;
    }

    let next_index = (runtime.active_index + 1) % FLUENT_THEME_VARIANTS.len();
    set_active_style_variant_by_name(world, FLUENT_THEME_VARIANTS[next_index]);

    if world.get_entity(runtime.button).is_ok() {
        world
            .entity_mut(runtime.button)
            .insert(UiButton::new(theme_button_text(next_index)));
    }

    if let Some(mut runtime_mut) = world.get_resource_mut::<FluentThemeToggleRuntime>() {
        runtime_mut.active_index = next_index;
    }
}

/// Initialize process-wide tracing for examples.
///
/// If `RUST_LOG` is set it takes precedence over [`DEFAULT_LOG_FILTER`].
pub fn init_logging() {
    LOGGING_INITIALIZED.get_or_init(|| {
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));

        let _ = fmt().with_env_filter(env_filter).try_init();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_toggle_cycles_variants() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let button = world.spawn_empty().id();
        world.insert_resource(FluentThemeToggleRuntime {
            button,
            active_index: 2,
        });

        world
            .resource::<UiEventQueue>()
            .push_typed(button, BuiltinUiAction::Clicked);

        drain_fluent_theme_toggle_events(&mut world);

        let runtime = world.resource::<FluentThemeToggleRuntime>();
        assert_eq!(runtime.active_index, 0);

        let active = world.resource::<ActiveStyleVariant>();
        assert_eq!(active.0.as_deref(), Some("dark"));
    }

    #[test]
    fn theme_toggle_requeues_unrelated_builtin_events() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let toggle_button = world.spawn_empty().id();
        let other_button = world.spawn_empty().id();
        world.insert_resource(FluentThemeToggleRuntime {
            button: toggle_button,
            active_index: 0,
        });

        world
            .resource::<UiEventQueue>()
            .push_typed(other_button, BuiltinUiAction::Clicked);

        drain_fluent_theme_toggle_events(&mut world);

        let runtime = world.resource::<FluentThemeToggleRuntime>();
        assert_eq!(runtime.active_index, 0);

        let queued = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<BuiltinUiAction>();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].entity, other_button);
    }
}
