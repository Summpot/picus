//! Internal Picus bridge from ECS projection to retained widget views.
//!
//! This module is not the public Picus component surface. It holds the small adapters that
//! bind retained widget events, hit markers, and portal state back to [`crate::UiEventQueue`]
//! and Bevy entities during projection.
mod button_view;
mod button_with_child_view;
mod component_views;
mod color_spectrum_view;
mod drag_thumb_view;
mod entity_scope_view;
mod opaque_hitbox_view;
mod scroll_portal_view;

pub use button_view::button_view as button;
pub use button_view::{ButtonView, button_view};
pub use button_with_child_view::button_with_child_view as button_with_child;
pub use button_with_child_view::{ButtonWithChildView, button_with_child_view};
pub use component_views::checkbox_view as checkbox;
pub use component_views::slider_view as slider;
pub use component_views::switch_view as switch;
pub use component_views::text_input_view as text_input;
pub use component_views::{CheckboxView, SliderView, SwitchView};
pub(crate) use component_views::{radio_button_view, slider_view, text_input_view};
pub(crate) use color_spectrum_view::color_spectrum_view;
pub(crate) use drag_thumb_view::drag_thumb_view;
pub(crate) use entity_scope_view::entity_scope;
pub(crate) use opaque_hitbox_view::opaque_hitbox_for_entity;
pub(crate) use scroll_portal_view::scroll_portal;
