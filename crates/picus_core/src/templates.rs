use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::*,
};

use crate::{
    components::expand_all_ui_component_templates,
    ecs::{UiCheckbox, UiComboBox, UiDialog, UiScrollView, UiSlider, UiSwitch, UiTextInput},
};

/// Find the first child template part entity for `parent` tagged with marker `P`.
#[must_use]
pub fn find_template_part<P: Component>(world: &World, parent: Entity) -> Option<Entity> {
    let children = world.get::<Children>(parent)?;
    children
        .iter()
        .find(|child| world.get::<P>(*child).is_some())
}

/// Spawn a new template part under `parent`.
#[must_use]
pub fn spawn_template_part<B: Bundle>(world: &mut World, parent: Entity, bundle: B) -> Entity {
    world.spawn((bundle, ChildOf(parent))).id()
}

/// Ensure a child template part tagged with marker `P` exists.
#[must_use]
pub fn ensure_template_part<P, B>(
    world: &mut World,
    parent: Entity,
    make_bundle: impl FnOnce() -> B,
) -> Entity
where
    P: Component + Default,
    B: Bundle,
{
    if let Some(existing) = find_template_part::<P>(world, parent) {
        return existing;
    }

    spawn_template_part(world, parent, (P::default(), make_bundle()))
}

/// Compatibility helper: expand built-in logical UI components into ECS child template parts.
///
/// New code should prefer trait-driven registration (`register_ui_component::<T>()`),
/// which installs `Added<T>` expansion systems automatically.
pub fn expand_builtin_ui_component_templates(world: &mut World) {
    expand_all_ui_component_templates::<UiCheckbox>(world);
    expand_all_ui_component_templates::<UiSlider>(world);
    expand_all_ui_component_templates::<UiSwitch>(world);
    expand_all_ui_component_templates::<UiTextInput>(world);
    expand_all_ui_component_templates::<UiDialog>(world);
    expand_all_ui_component_templates::<UiComboBox>(world);
    expand_all_ui_component_templates::<UiScrollView>(world);
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_ext::AppPicusExt;
    use crate::{PicusPlugin, UiEventQueue, UiRoot};
    use bevy_app::App;
    use std::sync::Arc;

    #[test]
    fn template_expansion_and_widget_actions_update_checkbox_state() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let checkbox = world
            .spawn((crate::UiCheckbox::new("Receive updates", false),))
            .id();

        crate::expand_builtin_ui_component_templates(&mut world);

        let indicator = crate::find_template_part::<crate::PartCheckboxIndicator>(&world, checkbox)
            .expect("checkbox indicator part should be expanded");
        let label = crate::find_template_part::<crate::PartCheckboxLabel>(&world, checkbox)
            .expect("checkbox label part should be expanded");

        assert_eq!(
            world
                .get::<crate::UiLabel>(indicator)
                .expect("indicator label should exist")
                .text,
            "☐"
        );
        assert_eq!(
            world
                .get::<crate::UiLabel>(label)
                .expect("label part should have text")
                .text,
            "Receive updates"
        );

        world
            .resource::<UiEventQueue>()
            .push_typed(checkbox, crate::WidgetUiAction::ToggleCheckbox { checkbox });
        crate::handle_widget_actions(&mut world);
        crate::expand_builtin_ui_component_templates(&mut world);

        assert!(
            world
                .get::<crate::UiCheckbox>(checkbox)
                .expect("checkbox should exist")
                .checked
        );
        assert_eq!(
            world
                .resource_mut::<UiEventQueue>()
                .drain_actions::<crate::UiCheckboxChanged>()
                .len(),
            1
        );
        assert_eq!(
            world
                .get::<crate::UiLabel>(indicator)
                .expect("indicator label should exist")
                .text,
            "☑"
        );
    }

    #[test]
    fn third_party_ui_component_can_register_via_trait_api() {
        #[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
        struct UiKnob;

        #[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
        struct PartKnobIndicator;

        impl crate::UiComponentTemplate for UiKnob {
            fn expand(world: &mut World, entity: Entity) {
                let _ = crate::ensure_template_part::<PartKnobIndicator, _>(world, entity, || {
                    (
                        crate::UiLabel::new("○"),
                        crate::StyleClass(vec!["template.knob.indicator".to_string()]),
                    )
                });
            }

            fn project(_: &Self, _ctx: crate::ProjectionCtx<'_>) -> crate::UiView {
                Arc::new(crate::xilem::view::label("knob"))
            }
        }

        let mut app = App::new();
        app.add_plugins(PicusPlugin)
            .register_ui_component::<UiKnob>();

        let knob = app.world_mut().spawn((UiRoot, UiKnob)).id();
        app.update();

        assert!(
            app.world()
                .resource::<crate::StyleTypeRegistry>()
                .resolve("UiKnob")
                .is_some()
        );

        assert!(crate::find_template_part::<PartKnobIndicator>(app.world(), knob).is_some());
    }
}
