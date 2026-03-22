use bevy_ecs::{entity::Entity, prelude::*};

use crate::{
    ProjectionCtx, StyleClass, UiEvent, UiLabel, UiView, components::UiComponentTemplate,
    templates::ensure_template_part,
};

/// Modal dialog entity projected in the overlay layer.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiDialog {
    pub title: String,
    pub body: String,
    pub dismiss_label: String,
    pub title_key: Option<String>,
    pub body_key: Option<String>,
    pub dismiss_key: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

#[derive(Component, Debug)]
pub struct UiDialogCloseAction {
    event: Option<UiEvent>,
}

impl UiDialogCloseAction {
    #[must_use]
    pub fn new<T: Send + Sync + 'static>(target: Entity, action: T) -> Self {
        Self {
            event: Some(UiEvent::typed(target, action)),
        }
    }

    pub(crate) fn take_event(&mut self) -> Option<UiEvent> {
        self.event.take()
    }
}

impl UiDialog {
    #[must_use]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            dismiss_label: "Close".to_string(),
            title_key: None,
            body_key: None,
            dismiss_key: None,
            width: None,
            height: None,
        }
    }

    #[must_use]
    pub fn with_localized_keys(
        mut self,
        title_key: impl Into<String>,
        body_key: impl Into<String>,
        dismiss_key: impl Into<String>,
    ) -> Self {
        self.title_key = Some(title_key.into());
        self.body_key = Some(body_key.into());
        self.dismiss_key = Some(dismiss_key.into());
        self
    }

    #[must_use]
    pub fn with_fixed_width(mut self, width: f64) -> Self {
        self.width = Some(width.max(1.0));
        self
    }

    #[must_use]
    pub fn with_fixed_height(mut self, height: f64) -> Self {
        self.height = Some(height.max(1.0));
        self
    }

    #[must_use]
    pub fn with_fixed_size(mut self, width: f64, height: f64) -> Self {
        self.width = Some(width.max(1.0));
        self.height = Some(height.max(1.0));
        self
    }
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartDialogTitle;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartDialogBody;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartDialogDismiss;

impl UiComponentTemplate for UiDialog {
    fn expand(world: &mut World, entity: Entity) {
        let dialog = world.get::<UiDialog>(entity).cloned();
        let Some(dialog) = dialog else {
            return;
        };

        let title_part = ensure_template_part::<PartDialogTitle, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["overlay.dialog.title".to_string()]),
            )
        });
        let body_part = ensure_template_part::<PartDialogBody, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["overlay.dialog.body".to_string()]),
            )
        });
        let dismiss_part = ensure_template_part::<PartDialogDismiss, _>(world, entity, || {
            (
                UiLabel::new(""),
                StyleClass(vec!["overlay.dialog.dismiss".to_string()]),
            )
        });

        if let Some(mut label) = world.get_mut::<UiLabel>(title_part) {
            label.text = dialog.title;
        }
        if let Some(mut label) = world.get_mut::<UiLabel>(body_part) {
            label.text = dialog.body;
        }
        if let Some(mut label) = world.get_mut::<UiLabel>(dismiss_part) {
            label.text = dialog.dismiss_label;
        }
    }

    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::dialog::project_dialog(component, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_fixed_size_builder_sets_hints() {
        let dialog = UiDialog::new("title", "body").with_fixed_size(920.0, 760.0);

        assert_eq!(dialog.width, Some(920.0));
        assert_eq!(dialog.height, Some(760.0));
    }
}
