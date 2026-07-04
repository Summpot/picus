use crate::helpers::{card, grid, placeholder};
use bevy_ecs::{hierarchy::ChildOf, prelude::*};
use picus_core::{
    scene::{CommandsSceneExt, bsn, template_value},
    UiButton, UiCheckbox, UiColorPicker, UiComboBox, UiComboOption, UiDatePicker, UiListView,
    UiRadioGroup,
};

/// Checkbox, RadioButton, ColorPicker, DatePicker, ComboBox, and ListBox examples.
///
/// Corresponds to Fluent UI's ChoiceGroup, ColorPicker, DatePicker, and Dropdown components.
pub fn spawn_selection_page(commands: &mut Commands, parent: Entity) -> Entity {
    let g = grid(commands, parent, 3);

    let check = card(commands, g, "CheckBox");
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("CheckBox", false))
        ChildOf(check)
    });
    commands.spawn_scene(bsn! {
        template_value(UiCheckbox::new("Checked", true))
        ChildOf(check)
    });
    placeholder(
        commands,
        check,
        "Three-state CheckBox",
        "UiCheckbox currently stores a bool, so indeterminate state is not represented.",
    );

    let radio = card(commands, g, "RadioButton");
    commands.spawn_scene(bsn! {
        template_value(UiRadioGroup::new(["Apple", "Banana", "Cherry", "Long long option"]).with_selected(1))
        ChildOf(radio)
    });

    let pickers = card(commands, g, "Pickers");
    commands.spawn_scene(bsn! {
        template_value(UiColorPicker::new(0x60, 0xA5, 0xFA))
        ChildOf(pickers)
    });
    commands.spawn_scene(bsn! {
        template_value(UiDatePicker::new(2026, 5, 24))
        ChildOf(pickers)
    });
    commands.spawn_scene(bsn! {
        template_value(
            UiComboBox::new(vec![
                UiComboOption::new("small", "Small"),
                UiComboOption::new("medium", "Medium"),
                UiComboOption::new("large", "Large"),
            ])
            .with_placeholder("Size")
        )
        ChildOf(pickers)
    });

    let list = card(commands, g, "ListBox");
    commands.spawn_scene(bsn! {
        template_value(
            UiListView::new((1..=8).map(|i| format!("Item {i}")))
                .with_selected(2)
                .with_item_padding(7.0)
        )
        ChildOf(list)
    });

    let calendar = card(commands, g, "Calendar");
    commands.spawn_scene(bsn! {
        template_value(UiDatePicker::new(2024, 6, 15))
        ChildOf(calendar)
    });
    placeholder(
        commands,
        calendar,
        "Always-visible calendar",
        "UiDatePicker renders its month grid only as an anchored overlay panel.",
    );

    commands
        .spawn_scene(bsn! {
            template_value(UiButton::new("Success Toast"))
            ChildOf(pickers)
        })
        .id()
}
