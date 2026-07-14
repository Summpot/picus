use std::collections::HashSet;

use bevy_ecs::{entity::Entity, hierarchy::ChildOf, message::MessageReader, prelude::*};
use bevy_input::mouse::{MouseScrollUnit, MouseWheel};
use bevy_math::Vec2;
use bevy_time::Time;
use bevy_window::{PrimaryWindow, Window};
use masonry_core::core::{Widget, WidgetRef};

use crate::{
    AnchoredTo, AutoDismiss, HasTooltip, InteractionState, MasonryRuntime, OverlayAnchorRect,
    OverlayComputedPosition, OverlayConfig, OverlayPlacement, OverlayState, ScrollAxis, UiCheckbox,
    UiCheckboxChanged, UiDataTable, UiDataTableSelectionChanged, UiDataTableSortChanged,
    UiListSelectionMode, UiListView, UiListViewSelectionChanged, UiMultilineTextInput,
    NavigationDisplayMode, NavigationItemRegion, UiMultilineTextInputChanged,
    UiNavigationBackRequested, UiNavigationItemExpandedChanged, UiNavigationItemInvoked,
    UiNavigationPaneChanged, UiNavigationSelectionChanged, UiNavigationView, UiNumericUpDown,
    UiNumericUpDownChanged, UiOverlayRoot, UiPasswordInput, UiPasswordInputChanged, UiRadioGroup,
    UiRadioGroupChanged, UiRating, UiRatingChanged, UiScrollView, UiScrollViewChanged, UiSlider,
    UiSearch, UiSearchChanged, UiSliderChanged, UiSwitch, UiSwitchChanged, UiTabBar, UiTabChanged,
    UiTextInput, UiTextInputChanged, UiTooltip, UiTreeNode, UiTreeNodeToggled,
    events::UiEventQueue,
};

/// Internal action enum for non-overlay widget interactions.
///
/// These actions are emitted by built-in widget projectors and applied by the
/// PreUpdate dispatcher (`apply_widget_ui_action`).
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetUiAction {
    /// Select a specific item in a radio group.
    SelectRadioItem { group: Entity, index: usize },
    /// Switch the active tab in a tab bar.
    SelectTab { bar: Entity, index: usize },
    /// Select a leaf navigation item in a [`UiNavigationView`] by leaf index.
    SelectNavigationItem { nav: Entity, index: usize },
    /// User invoke of a navigation item (WinUI `ItemInvoked` pipeline).
    InvokeNavigationItem {
        nav: Entity,
        item: Entity,
        /// Selectable leaf index when the target is a leaf / Settings.
        leaf_index: Option<usize>,
        is_settings: bool,
        /// When true, parent should expand/collapse or open a flyout.
        is_parent: bool,
        selects_on_invoked: bool,
    },
    /// Expand or collapse a hierarchical navigation menu item (WinUI `IsExpanded`).
    ToggleNavigationItemExpand { nav: Entity, item: Entity },
    /// Toggle compact hierarchy flyout for a parent item.
    ToggleNavigationItemFlyout { nav: Entity, item: Entity },
    /// Expand or collapse a [`UiNavigationView`] pane.
    ToggleNavigationPane { nav: Entity },
    /// Light-dismiss / close the navigation pane (overlay modes).
    CloseNavigationPane { nav: Entity },
    /// Back button pressed.
    NavigationBack { nav: Entity },
    /// Expand or collapse a tree node.
    ToggleTreeNode { node: Entity },
    /// Toggle a checkbox.
    ToggleCheckbox { checkbox: Entity },
    /// Set a checkbox to an explicit checked state.
    SetCheckbox { checkbox: Entity, checked: bool },
    /// Adjust slider value using step increments.
    StepSlider { slider: Entity, delta: f64 },
    /// Set a slider value directly from a native slider interaction.
    SetSliderValue { slider: Entity, value: f64 },
    /// Toggle a switch.
    ToggleSwitch { switch: Entity },
    /// Update text input contents.
    SetTextInput { input: Entity, value: String },
    /// Update search input contents.
    SetSearch { search: Entity, value: String },
    /// Update a password input from the visible masked editor contents.
    SetPasswordInputDisplay {
        input: Entity,
        display_value: String,
    },
    /// Update multiline text input contents.
    SetMultilineTextInput { input: Entity, value: String },
    /// Select an item in a list view.
    SelectListItem { list_view: Entity, index: usize },
    /// Select a row in a data table.
    SelectDataTableRow { table: Entity, row: usize },
    /// Sort a data table by a column.
    SortDataTableColumn { table: Entity, column: usize },
    /// Change a rating value.
    RatingChanged { rating: Entity, value: f64 },
    /// Step a numeric up-down value by `delta` multiples of its step.
    StepNumericUpDown { numeric: Entity, delta: f64 },
    /// Drag an ECS scroll-thumb by a physical pixel delta.
    DragScrollThumb {
        thumb: Entity,
        axis: ScrollAxis,
        delta_pixels: f64,
    },
}

const SCROLLBAR_MIN_THUMB: f64 = 24.0;

fn thumb_length(viewport: f64, content: f64) -> f64 {
    if content <= 0.0 {
        return viewport.max(0.0);
    }
    let ratio = (viewport / content).clamp(0.0, 1.0);
    (viewport * ratio).clamp(SCROLLBAR_MIN_THUMB.min(viewport), viewport)
}

fn scroll_delta_from_thumb_drag(
    scroll_view: UiScrollView,
    axis: ScrollAxis,
    delta_pixels: f64,
) -> f32 {
    let (viewport, content) = match axis {
        ScrollAxis::Horizontal => (
            scroll_view.viewport_size.x as f64,
            scroll_view.content_size.x as f64,
        ),
        ScrollAxis::Vertical => (
            scroll_view.viewport_size.y as f64,
            scroll_view.content_size.y as f64,
        ),
    };

    let max_scroll = (content - viewport).max(0.0);
    if max_scroll <= f64::EPSILON {
        return 0.0;
    }

    let track_len = viewport.max(1.0);
    let thumb_len = thumb_length(viewport, content);
    let travel = (track_len - thumb_len).max(1.0);

    (delta_pixels * (max_scroll / travel)) as f32
}

fn clamp_scroll_offset_strict(scroll_view: &mut UiScrollView) {
    let max_scroll_y = (scroll_view.content_size.y - scroll_view.viewport_size.y).max(0.0);
    let max_scroll_x = (scroll_view.content_size.x - scroll_view.viewport_size.x).max(0.0);

    scroll_view.scroll_offset.x = scroll_view.scroll_offset.x.clamp(0.0, max_scroll_x);
    scroll_view.scroll_offset.y = scroll_view.scroll_offset.y.clamp(0.0, max_scroll_y);
}

fn clamped_scroll_offset(offset: Vec2, viewport_size: Vec2, content_size: Vec2) -> Vec2 {
    let max_scroll_y = (content_size.y - viewport_size.y).max(0.0);
    let max_scroll_x = (content_size.x - viewport_size.x).max(0.0);

    Vec2::new(
        offset.x.clamp(0.0, max_scroll_x),
        offset.y.clamp(0.0, max_scroll_y),
    )
}

fn quantize_slider_value(slider: &UiSlider, value: f64) -> f64 {
    let step = slider.step.max(f64::EPSILON);
    let steps = ((value - slider.min) / step).round();
    (slider.min + steps * step).clamp(slider.min, slider.max)
}

fn reconcile_password_value(previous: &str, display_value: &str, mask: char) -> String {
    let mut previous_chars = previous.chars();
    let mut resolved = String::new();

    for display_char in display_value.chars() {
        match previous_chars.next() {
            Some(previous_char) if display_char == mask => resolved.push(previous_char),
            Some(_) | None => resolved.push(display_char),
        }
    }

    resolved
}

fn find_ancestor_scroll_view(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if world.get::<UiScrollView>(entity).is_some() {
            return Some(entity);
        }

        let parent = world
            .get::<ChildOf>(entity)
            .map(|child_of| child_of.parent())?;
        entity = parent;
    }
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

fn collect_scroll_view_targets_from_hit_path(
    window_runtime: &crate::runtime::WindowRuntime,
    hit_path: &[masonry_core::core::WidgetId],
    parents: &Query<&ChildOf>,
    scroll_markers: &Query<(), With<UiScrollView>>,
) -> Vec<Entity> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();

    for widget_id in hit_path.iter().rev().copied() {
        let Some(entity_bits) = window_runtime
            .render_root
            .get_widget(widget_id)
            .and_then(|widget| widget.get_debug_text())
            .and_then(|debug| parse_entity_bits_from_debug(&debug))
        else {
            continue;
        };

        let Some(mut entity) = Entity::try_from_bits(entity_bits) else {
            continue;
        };

        loop {
            if scroll_markers.get(entity).is_ok() {
                if seen.insert(entity) {
                    ordered.push(entity);
                }
                break;
            }

            let Ok(parent) = parents.get(entity) else {
                break;
            };
            entity = parent.parent();
        }
    }

    ordered
}

fn portal_geometry_from_subtree(widget: WidgetRef<'_, dyn Widget>) -> Option<(Vec2, Vec2)> {
    if widget.ctx().is_stashed() {
        return None;
    }

    if widget.short_type_name() == "Portal" {
        let viewport_size = widget.ctx().border_box().size();
        let viewport = Vec2::new(viewport_size.width as f32, viewport_size.height as f32);

        let content = widget
            .children()
            .into_iter()
            .find(|child| !child.ctx().is_stashed() && child.short_type_name() != "ScrollBar")
            .map(|child| {
                let size = child.ctx().border_box().size();
                Vec2::new(size.width as f32, size.height as f32)
            })
            .unwrap_or(viewport);

        return Some((viewport.max(Vec2::ZERO), content.max(Vec2::ZERO)));
    }

    for child in widget.children() {
        if let Some(geometry) = portal_geometry_from_subtree(child) {
            return Some(geometry);
        }
    }

    None
}

/// Synchronize [`UiScrollView`] geometry from Masonry layout results.
///
/// This keeps ECS `content_size`/`viewport_size` aligned with the retained portal,
/// then strictly clamps `scroll_offset` so it cannot drift past real bounds.
pub fn sync_scroll_view_layout_geometry(
    runtime: Option<NonSend<MasonryRuntime>>,
    mut scroll_views: Query<(Entity, &mut UiScrollView)>,
    ui_events: Res<UiEventQueue>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let Some(window_runtime) = runtime.primary() else {
        return;
    };

    for (entity, mut scroll_view) in &mut scroll_views {
        let widget_id = window_runtime
            .find_widget_id_for_entity_bits(entity.to_bits(), false)
            .or_else(|| window_runtime.find_widget_id_for_entity_bits(entity.to_bits(), true));

        let Some(widget_id) = widget_id else {
            continue;
        };

        let Some(root_widget) = window_runtime.render_root.get_widget(widget_id) else {
            continue;
        };

        let Some((viewport_size, content_size)) = portal_geometry_from_subtree(root_widget) else {
            continue;
        };

        let before = scroll_view.scroll_offset;
        let next_offset =
            clamped_scroll_offset(scroll_view.scroll_offset, viewport_size, content_size);

        if scroll_view.viewport_size != viewport_size
            || scroll_view.content_size != content_size
            || scroll_view.scroll_offset != next_offset
        {
            scroll_view.viewport_size = viewport_size;
            scroll_view.content_size = content_size;
            scroll_view.scroll_offset = next_offset;
        }

        if scroll_view.scroll_offset != before {
            ui_events.push_typed(
                entity,
                UiScrollViewChanged {
                    scroll_view: entity,
                    scroll_offset: scroll_view.scroll_offset,
                },
            );
        }
    }
}

/// Apply one built-in [`WidgetUiAction`] (dispatcher handler + tests).
///
/// After mutating component state this re-emits high-level Changed payloads
/// onto the internal queue so the same-frame dispatcher can convert them to
/// application [`crate::UiAction`] messages when registered.
pub fn apply_widget_ui_action(world: &mut World, _source: Entity, action: &WidgetUiAction) {
    match action.clone() {
            WidgetUiAction::SelectRadioItem { group, index } => {
                if world.get_entity(group).is_err() {
                    return;
                }

                let changed = if let Some(mut radio_group) = world.get_mut::<UiRadioGroup>(group) {
                    radio_group.selected = index;
                    Some(UiRadioGroupChanged {
                        group,
                        selected: index,
                    })
                } else {
                    None
                };

                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(group, ev);
                }
            }

            WidgetUiAction::SelectTab { bar, index } => {
                if world.get_entity(bar).is_err() {
                    return;
                }

                let changed = if let Some(mut tab_bar) = world.get_mut::<UiTabBar>(bar) {
                    tab_bar.active = index;
                    Some(UiTabChanged { bar, active: index })
                } else {
                    None
                };

                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(bar, ev);
                }
            }

            WidgetUiAction::SelectNavigationItem { nav, index } => {
                if world.get_entity(nav).is_err() {
                    return;
                }

                let changed = if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                    let max = nav_view.leaf_count().saturating_sub(1);
                    let selected = index.min(max);
                    if nav_view.selected != selected {
                        nav_view.selected = selected;
                        let is_settings_selected = nav_view.is_settings_selected();
                        Some(UiNavigationSelectionChanged {
                            nav,
                            selected,
                            is_settings_selected,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(nav, ev);
                }
            }

            WidgetUiAction::InvokeNavigationItem {
                nav,
                item,
                leaf_index,
                is_settings,
                is_parent,
                selects_on_invoked,
            } => {
                if world.get_entity(nav).is_err() {
                    return;
                }

                world.resource::<UiEventQueue>().push_typed(
                    nav,
                    UiNavigationItemInvoked {
                        nav,
                        selected: leaf_index,
                        is_settings_invoked: is_settings,
                        item: Some(item),
                    },
                );

                // Selection
                if selects_on_invoked {
                    if let Some(index) = leaf_index {
                        let changed =
                            if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                                let max = nav_view.leaf_count().saturating_sub(1);
                                let selected = index.min(max);
                                if nav_view.selected != selected {
                                    nav_view.selected = selected;
                                    let is_settings_selected = nav_view.is_settings_selected();
                                    Some(UiNavigationSelectionChanged {
                                        nav,
                                        selected,
                                        is_settings_selected,
                                    })
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                        if let Some(ev) = changed {
                            world.resource::<UiEventQueue>().push_typed(nav, ev);
                        }
                    }
                }

                // Parent expand / flyout
                if is_parent && world.get_entity(item).is_ok() {
                    let uses_flyout = world
                        .get::<UiNavigationView>(nav)
                        .is_some_and(|n| n.uses_hierarchy_flyout());
                    if uses_flyout {
                        if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                            if nav_view.open_flyout_item == Some(item) {
                                nav_view.open_flyout_item = None;
                            } else {
                                nav_view.open_flyout_item = Some(item);
                            }
                        }
                    } else {
                        // Inline expand/collapse
                        let path =
                            crate::components::navigation_view::navigation_item_path(world, item);
                        let region = world
                            .get::<crate::UiNavigationItem>(item)
                            .map(|i| i.region);
                        if let (Some(path), Some(region)) = (path, region) {
                            let changed = if let Some(mut nav_view) =
                                world.get_mut::<UiNavigationView>(nav)
                            {
                                let list = match region {
                                    NavigationItemRegion::Menu => Some(&mut nav_view.items),
                                    NavigationItemRegion::Footer => {
                                        Some(&mut nav_view.footer_items)
                                    }
                                    NavigationItemRegion::Settings => None,
                                };
                                if let Some(list) = list {
                                    if let Some(node) =
                                        crate::components::navigation_view::navigation_item_at_mut(
                                            list, &path,
                                        )
                                    {
                                        if node.is_parent() {
                                            node.is_expanded = !node.is_expanded;
                                            Some(UiNavigationItemExpandedChanged {
                                                nav,
                                                item,
                                                is_expanded: node.is_expanded,
                                            })
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            if let Some(ev) = changed {
                                world.resource::<UiEventQueue>().push_typed(nav, ev);
                            }
                        }
                    }
                }

                // Close overlay pane after leaf navigate (WinUI).
                if leaf_index.is_some() {
                    if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                        if nav_view.display_mode != NavigationDisplayMode::Expanded
                            && nav_view.is_pane_open
                        {
                            nav_view.is_pane_open = false;
                            nav_view.force_closed = true;
                            nav_view.open_flyout_item = None;
                            let is_pane_open = nav_view.is_pane_open;
                            world.resource::<UiEventQueue>().push_typed(
                                nav,
                                UiNavigationPaneChanged { nav, is_pane_open },
                            );
                        } else {
                            // Always dismiss flyout after leaf select.
                            nav_view.open_flyout_item = None;
                        }
                    }
                }
            }

            WidgetUiAction::ToggleNavigationItemExpand { nav, item } => {
                if world.get_entity(nav).is_err() || world.get_entity(item).is_err() {
                    return;
                }

                let path = crate::components::navigation_view::navigation_item_path(world, item);
                let region = world
                    .get::<crate::UiNavigationItem>(item)
                    .map(|i| i.region);
                let Some(path) = path else {
                    return;
                };
                let Some(region) = region else {
                    return;
                };

                let changed = if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                    let list = match region {
                        NavigationItemRegion::Menu => &mut nav_view.items,
                        NavigationItemRegion::Footer => &mut nav_view.footer_items,
                        NavigationItemRegion::Settings => {
                            return;
                        }
                    };
                    if let Some(node) =
                        crate::components::navigation_view::navigation_item_at_mut(list, &path)
                    {
                        if node.is_parent() {
                            node.is_expanded = !node.is_expanded;
                            Some(UiNavigationItemExpandedChanged {
                                nav,
                                item,
                                is_expanded: node.is_expanded,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(nav, ev);
                }
            }

            WidgetUiAction::ToggleNavigationItemFlyout { nav, item } => {
                if world.get_entity(nav).is_err() || world.get_entity(item).is_err() {
                    return;
                }
                if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                    if nav_view.open_flyout_item == Some(item) {
                        nav_view.open_flyout_item = None;
                    } else {
                        nav_view.open_flyout_item = Some(item);
                    }
                }
            }

            WidgetUiAction::ToggleNavigationPane { nav } => {
                if world.get_entity(nav).is_err() {
                    return;
                }

                let changed = if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                    nav_view.is_pane_open = !nav_view.is_pane_open;
                    nav_view.force_closed = !nav_view.is_pane_open;
                    nav_view.open_flyout_item = None;
                    Some(UiNavigationPaneChanged {
                        nav,
                        is_pane_open: nav_view.is_pane_open,
                    })
                } else {
                    None
                };

                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(nav, ev);
                }
            }

            WidgetUiAction::CloseNavigationPane { nav } => {
                if world.get_entity(nav).is_err() {
                    return;
                }
                let changed = if let Some(mut nav_view) = world.get_mut::<UiNavigationView>(nav) {
                    if nav_view.is_pane_open {
                        nav_view.is_pane_open = false;
                        nav_view.force_closed = true;
                        nav_view.open_flyout_item = None;
                        Some(UiNavigationPaneChanged {
                            nav,
                            is_pane_open: false,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(ev) = changed {
                    world.resource::<UiEventQueue>().push_typed(nav, ev);
                }
            }

            WidgetUiAction::NavigationBack { nav } => {
                if world.get_entity(nav).is_err() {
                    return;
                }
                let enabled = world
                    .get::<UiNavigationView>(nav)
                    .is_some_and(|n| n.is_back_enabled && n.back_button_shown());
                if enabled {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(nav, UiNavigationBackRequested { nav });
                }
            }

            WidgetUiAction::ToggleTreeNode { node } => {
                if world.get_entity(node).is_err() {
                    return;
                }

                let toggled = world
                    .get::<UiTreeNode>(node)
                    .map(|tree_node| !tree_node.is_expanded);

                if let Some(is_expanded) = toggled {
                    if let Some(mut tree_node) = world.get_mut::<UiTreeNode>(node) {
                        tree_node.is_expanded = is_expanded;
                    }
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(node, UiTreeNodeToggled { node, is_expanded });
                }
            }

            WidgetUiAction::ToggleCheckbox { checkbox } => {
                if world.get_entity(checkbox).is_err() {
                    return;
                }

                let changed =
                    if let Some(mut checkbox_state) = world.get_mut::<UiCheckbox>(checkbox) {
                        // Tri-state toggle: indeterminate → checked → unchecked → checked.
                        // If currently indeterminate, clear indeterminate and set checked.
                        if checkbox_state.indeterminate {
                            checkbox_state.indeterminate = false;
                            checkbox_state.checked = true;
                            Some((true, false))
                        } else {
                            checkbox_state.checked = !checkbox_state.checked;
                            Some((checkbox_state.checked, false))
                        }
                    } else {
                        None
                    };

                if let Some((checked, indeterminate)) = changed {
                    world.resource::<UiEventQueue>().push_typed(
                        checkbox,
                        UiCheckboxChanged {
                            checkbox,
                            checked,
                            indeterminate,
                        },
                    );
                }
            }

            WidgetUiAction::SetCheckbox { checkbox, checked } => {
                if world.get_entity(checkbox).is_err() {
                    return;
                }

                let changed =
                    if let Some(mut checkbox_state) = world.get_mut::<UiCheckbox>(checkbox) {
                        if checkbox_state.checked == checked && !checkbox_state.indeterminate {
                            None
                        } else {
                            checkbox_state.checked = checked;
                            checkbox_state.indeterminate = false;
                            Some(checked)
                        }
                    } else {
                        None
                    };

                if let Some(checked) = changed {
                    world.resource::<UiEventQueue>().push_typed(
                        checkbox,
                        UiCheckboxChanged {
                            checkbox,
                            checked,
                            indeterminate: false,
                        },
                    );
                }
            }

            WidgetUiAction::StepSlider { slider, delta } => {
                if world.get_entity(slider).is_err() {
                    return;
                }

                if let Some(mut slider_state) = world.get_mut::<UiSlider>(slider) {
                    let step = slider_state.step.max(f64::EPSILON);
                    let next = quantize_slider_value(
                        &slider_state,
                        (slider_state.value + delta * step)
                            .clamp(slider_state.min, slider_state.max),
                    );
                    if (next - slider_state.value).abs() > f64::EPSILON {
                        slider_state.value = next;
                        world.resource::<UiEventQueue>().push_typed(
                            slider,
                            UiSliderChanged {
                                slider,
                                value: next,
                            },
                        );
                    }
                }
            }

            WidgetUiAction::RatingChanged { rating, value } => {
                if world.get_entity(rating).is_err() {
                    return;
                }

                if let Some(mut rating_state) = world.get_mut::<UiRating>(rating) {
                    rating_state.value = value.clamp(0.0, f64::from(rating_state.max));
                }

                world
                    .resource::<UiEventQueue>()
                    .push_typed(rating, UiRatingChanged { rating, value });
            }

            WidgetUiAction::StepNumericUpDown { numeric, delta } => {
                if world.get_entity(numeric).is_err() {
                    return;
                }

                if let Some(mut numeric_state) = world.get_mut::<UiNumericUpDown>(numeric)
                    && !numeric_state.disabled
                {
                    let step = numeric_state.step.max(f64::EPSILON);
                    let next = (numeric_state.value + delta * step)
                        .clamp(numeric_state.min, numeric_state.max);
                    if (next - numeric_state.value).abs() > f64::EPSILON {
                        numeric_state.value = next;
                        world.resource::<UiEventQueue>().push_typed(
                            numeric,
                            UiNumericUpDownChanged {
                                numeric,
                                value: next,
                            },
                        );
                    }
                }
            }

            WidgetUiAction::SetSliderValue { slider, value } => {
                if world.get_entity(slider).is_err() {
                    return;
                }

                if let Some(mut slider_state) = world.get_mut::<UiSlider>(slider) {
                    let next = quantize_slider_value(&slider_state, value);
                    if (next - slider_state.value).abs() > f64::EPSILON {
                        slider_state.value = next;
                        world.resource::<UiEventQueue>().push_typed(
                            slider,
                            UiSliderChanged {
                                slider,
                                value: next,
                            },
                        );
                    }
                }
            }

            WidgetUiAction::ToggleSwitch { switch } => {
                if world.get_entity(switch).is_err() {
                    return;
                }

                let changed = if let Some(mut switch_state) = world.get_mut::<UiSwitch>(switch) {
                    switch_state.on = !switch_state.on;
                    Some(switch_state.on)
                } else {
                    None
                };

                if let Some(on) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(switch, UiSwitchChanged { switch, on });
                }
            }

            WidgetUiAction::SetTextInput { input, value } => {
                if world.get_entity(input).is_err() {
                    return;
                }

                let changed = if let Some(mut text_input) = world.get_mut::<UiTextInput>(input) {
                    if text_input.value == value {
                        None
                    } else {
                        text_input.value = value.clone();
                        Some(value)
                    }
                } else {
                    None
                };

                if let Some(value) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(input, UiTextInputChanged { input, value });
                }
            }

            WidgetUiAction::SetSearch { search, value } => {
                if world.get_entity(search).is_err() {
                    return;
                }

                let changed = if let Some(mut search_state) = world.get_mut::<UiSearch>(search) {
                    if search_state.value == value {
                        None
                    } else {
                        search_state.value = value.clone();
                        Some(value)
                    }
                } else {
                    None
                };

                if let Some(value) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(search, UiSearchChanged { search, value });
                }
            }

            WidgetUiAction::SetPasswordInputDisplay {
                input,
                display_value,
            } => {
                if world.get_entity(input).is_err() {
                    return;
                }

                let changed =
                    if let Some(mut password_input) = world.get_mut::<UiPasswordInput>(input) {
                        if password_input.read_only {
                            return;
                        }
                        let next = reconcile_password_value(
                            &password_input.value,
                            &display_value,
                            password_input.mask,
                        );
                        let next = password_input.clamped_value(next);
                        if password_input.value == next {
                            None
                        } else {
                            password_input.value = next.clone();
                            Some(next)
                        }
                    } else {
                        None
                    };

                if let Some(value) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(input, UiPasswordInputChanged { input, value });
                }
            }

            WidgetUiAction::SetMultilineTextInput { input, value } => {
                if world.get_entity(input).is_err() {
                    return;
                }

                let changed =
                    if let Some(mut text_input) = world.get_mut::<UiMultilineTextInput>(input) {
                        if text_input.read_only {
                            return;
                        }
                        let next = text_input.clamped_value(value);
                        if text_input.value == next {
                            None
                        } else {
                            text_input.value = next.clone();
                            Some(next)
                        }
                    } else {
                        None
                    };

                if let Some(value) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(input, UiMultilineTextInputChanged { input, value });
                }
            }

            WidgetUiAction::SelectListItem { list_view, index } => {
                if world.get_entity(list_view).is_err() {
                    return;
                }

                let changed = if let Some(mut list) = world.get_mut::<UiListView>(list_view) {
                    if index >= list.items.len() {
                        return;
                    }
                    match list.selection_mode {
                        UiListSelectionMode::None => None,
                        UiListSelectionMode::Single => {
                            if list.selected == Some(index) {
                                None
                            } else {
                                list.selected = Some(index);
                                list.selected_indices = vec![index];
                                Some((list.selected, list.clamped_selected_indices()))
                            }
                        }
                        UiListSelectionMode::Multiple => {
                            if let Some(position) =
                                list.selected_indices.iter().position(|item| *item == index)
                            {
                                list.selected_indices.remove(position);
                            } else {
                                list.selected_indices.push(index);
                            }
                            list.selected_indices.sort_unstable();
                            list.selected_indices.dedup();
                            list.selected = list.selected_indices.first().copied();
                            Some((list.selected, list.clamped_selected_indices()))
                        }
                    }
                } else {
                    None
                };

                if let Some((selected, selected_indices)) = changed {
                    world.resource::<UiEventQueue>().push_typed(
                        list_view,
                        UiListViewSelectionChanged {
                            list_view,
                            selected,
                            selected_indices,
                        },
                    );
                }
            }

            WidgetUiAction::SelectDataTableRow { table, row } => {
                if world.get_entity(table).is_err() {
                    return;
                }

                let changed = if let Some(mut data_table) = world.get_mut::<UiDataTable>(table) {
                    if row >= data_table.rows.len() {
                        return;
                    }
                    match data_table.selection_mode {
                        UiListSelectionMode::None => None,
                        UiListSelectionMode::Single => {
                            if data_table.selected_row == Some(row) {
                                None
                            } else {
                                data_table.selected_row = Some(row);
                                data_table.selected_rows = vec![row];
                                Some((data_table.selected_row, data_table.clamped_selected_rows()))
                            }
                        }
                        UiListSelectionMode::Multiple => {
                            if let Some(position) = data_table
                                .selected_rows
                                .iter()
                                .position(|item| *item == row)
                            {
                                data_table.selected_rows.remove(position);
                            } else {
                                data_table.selected_rows.push(row);
                            }
                            data_table.selected_rows.sort_unstable();
                            data_table.selected_rows.dedup();
                            data_table.selected_row = data_table.selected_rows.first().copied();
                            Some((data_table.selected_row, data_table.clamped_selected_rows()))
                        }
                    }
                } else {
                    None
                };

                if let Some((selected_row, selected_rows)) = changed {
                    world.resource::<UiEventQueue>().push_typed(
                        table,
                        UiDataTableSelectionChanged {
                            table,
                            selected_row,
                            selected_rows,
                        },
                    );
                }
            }

            WidgetUiAction::SortDataTableColumn { table, column } => {
                if world.get_entity(table).is_err() {
                    return;
                }

                let changed = if let Some(mut data_table) = world.get_mut::<UiDataTable>(table) {
                    data_table.toggle_sort_column(column)
                } else {
                    None
                };

                if let Some(sort) = changed {
                    world
                        .resource::<UiEventQueue>()
                        .push_typed(table, UiDataTableSortChanged { table, sort });
                }
            }

            WidgetUiAction::DragScrollThumb {
                thumb,
                axis,
                delta_pixels,
            } => {
                if world.get_entity(thumb).is_err() {
                    return;
                }

                let Some(scroll_entity) = find_ancestor_scroll_view(world, thumb) else {
                    return;
                };

                let changed =
                    if let Some(mut scroll_view) = world.get_mut::<UiScrollView>(scroll_entity) {
                        let before = scroll_view.scroll_offset;
                        let delta = scroll_delta_from_thumb_drag(*scroll_view, axis, delta_pixels);

                        match axis {
                            ScrollAxis::Horizontal => {
                                scroll_view.scroll_offset.x += delta;
                            }
                            ScrollAxis::Vertical => {
                                scroll_view.scroll_offset.y += delta;
                            }
                        }

                        clamp_scroll_offset_strict(&mut scroll_view);
                        let after = scroll_view.scroll_offset;
                        (after != before).then_some(after)
                    } else {
                        None
                    };

                if let Some(scroll_offset) = changed {
                    world.resource::<UiEventQueue>().push_typed(
                        scroll_entity,
                        UiScrollViewChanged {
                            scroll_view: scroll_entity,
                            scroll_offset,
                        },
                    );
                }
            }
        }

}

/// Drain remaining typed WidgetUiAction entries (test/compat helper).
///
/// Production scheduling uses the dispatcher registry; prefer
/// crate::events::dispatch_ui_actions.
pub fn handle_widget_actions(world: &mut World) {
    let actions = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<WidgetUiAction>();

    for event in actions {
        apply_widget_ui_action(world, event.entity, &event.action);
    }
}

/// Route mouse-wheel input to the nearest hit-tested [`UiScrollView`] entity.
///
/// This keeps ECS `scroll_offset` synchronized with pointer-wheel interactions
/// while the portal primitive handles clipping/composition.
pub fn handle_scroll_view_wheel(
    runtime: Option<NonSend<MasonryRuntime>>,
    mut wheel_events: MessageReader<MouseWheel>,
    primary_window_query: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut scroll_views: Query<&mut UiScrollView>,
    scroll_markers: Query<(), With<UiScrollView>>,
    parents: Query<&ChildOf>,
    ui_events: Res<UiEventQueue>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let Some(window_runtime) = runtime.primary() else {
        return;
    };

    let Some((primary_window_entity, primary_window)) = primary_window_query.iter().next() else {
        return;
    };

    let Some(cursor_pos) = primary_window.physical_cursor_position() else {
        return;
    };

    for wheel in wheel_events.read() {
        if wheel.window != primary_window_entity {
            continue;
        }

        let hit_path =
            window_runtime.get_hit_path((cursor_pos.x as f64, cursor_pos.y as f64).into());

        let scroll_targets = collect_scroll_view_targets_from_hit_path(
            window_runtime,
            &hit_path,
            &parents,
            &scroll_markers,
        );

        if scroll_targets.is_empty() {
            continue;
        }

        let factor = if wheel.unit == MouseScrollUnit::Line {
            MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
        } else {
            1.0
        };

        for scroll_entity in scroll_targets {
            let Ok(mut scroll_view) = scroll_views.get_mut(scroll_entity) else {
                continue;
            };

            let before = scroll_view.scroll_offset;

            if scroll_view.show_horizontal_scrollbar {
                scroll_view.scroll_offset.x -= wheel.x * factor;
            }
            if scroll_view.show_vertical_scrollbar {
                scroll_view.scroll_offset.y -= wheel.y * factor;
            }

            clamp_scroll_offset_strict(&mut scroll_view);

            let after = scroll_view.scroll_offset;
            if after != before {
                ui_events.push_typed(
                    scroll_entity,
                    UiScrollViewChanged {
                        scroll_view: scroll_entity,
                        scroll_offset: after,
                    },
                );
                break;
            }
        }
    }
}

/// Advance all [`AutoDismiss`] timers and despawn finished entities.
pub fn tick_auto_dismiss(
    mut commands: Commands,
    mut auto_dismiss_entities: Query<(Entity, &mut AutoDismiss)>,
    time: Res<Time>,
) {
    let delta = time.delta();

    for (entity, mut auto_dismiss) in &mut auto_dismiss_entities {
        auto_dismiss.timer.tick(delta);
        if auto_dismiss.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Backward-compatible alias retained for existing call sites.
pub fn tick_toasts(
    commands: Commands,
    auto_dismiss_entities: Query<(Entity, &mut AutoDismiss)>,
    time: Res<Time>,
) {
    tick_auto_dismiss(commands, auto_dismiss_entities, time);
}

/// Spawn or despawn tooltip overlay entities in response to hover state changes.
///
/// When an entity that carries [`HasTooltip`] becomes hovered (`InteractionState.hovered = true`) a
/// [`UiTooltip`] overlay is spawned under [`UiOverlayRoot`] anchored to that
/// entity. When the entity is no longer hovered, all tooltip overlays
/// anchored to it are despawned.
pub fn handle_tooltip_hovers(
    mut commands: Commands,
    overlay_root: Query<Entity, With<UiOverlayRoot>>,
    tooltip_sources: Query<(Entity, &HasTooltip, Option<&InteractionState>)>,
    existing_tooltips: Query<(Entity, &UiTooltip)>,
) {
    let Ok(root) = overlay_root.single() else {
        return;
    };

    let mut hovered_sources = HashSet::new();
    for (entity, _has_tooltip, state) in &tooltip_sources {
        if state.is_some_and(|state| state.hovered) {
            hovered_sources.insert(entity);
        }
    }

    // Spawn missing tooltips for hovered sources.
    for (entity, has_tooltip, state) in &tooltip_sources {
        if !state.is_some_and(|state| state.hovered) {
            continue;
        }

        let already_spawned = existing_tooltips
            .iter()
            .any(|(_, tooltip)| tooltip.anchor == entity);
        if already_spawned {
            continue;
        }

        commands.spawn((
            UiTooltip {
                text: has_tooltip.text.clone(),
                anchor: entity,
            },
            AnchoredTo(entity),
            OverlayAnchorRect::default(),
            OverlayConfig {
                placement: OverlayPlacement::Top,
                anchor: Some(entity),
                auto_flip: true,
            },
            OverlayState {
                is_modal: false,
                anchor: Some(entity),
            },
            OverlayComputedPosition::default(),
            ChildOf(root),
        ));
    }

    // Despawn tooltips whose source is no longer hovered (or no longer exists / has tooltip).
    for (tooltip_entity, tooltip) in &existing_tooltips {
        if !hovered_sources.contains(&tooltip.anchor) {
            commands.entity(tooltip_entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn direct_slider_action_updates_slider_state() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let slider = world
            .spawn((crate::UiSlider::new(0.0, 100.0, 10.0).with_step(5.0),))
            .id();

        world.resource::<UiEventQueue>().push_typed(
            slider,
            crate::WidgetUiAction::SetSliderValue {
                slider,
                value: 42.0,
            },
        );

        crate::handle_widget_actions(&mut world);

        let slider_state = world
            .get::<crate::UiSlider>(slider)
            .expect("slider should exist");
        assert_eq!(slider_state.value, 40.0);

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiSliderChanged>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].action.value, 40.0);
    }

    #[test]
    fn direct_checkbox_action_sets_checkbox_state() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let checkbox = world.spawn((crate::UiCheckbox::new("demo", false),)).id();

        world.resource::<UiEventQueue>().push_typed(
            checkbox,
            crate::WidgetUiAction::SetCheckbox {
                checkbox,
                checked: true,
            },
        );

        crate::handle_widget_actions(&mut world);

        let checkbox_state = world
            .get::<crate::UiCheckbox>(checkbox)
            .expect("checkbox should exist");
        assert!(checkbox_state.checked);

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiCheckboxChanged>();
        assert_eq!(changed.len(), 1);
        assert!(changed[0].action.checked);
    }

    #[test]
    fn indeterminate_checkbox_toggle_transitions_to_checked() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let checkbox = world
            .spawn((crate::UiCheckbox::new("tri-state", false).indeterminate(true),))
            .id();

        world
            .resource::<UiEventQueue>()
            .push_typed(checkbox, crate::WidgetUiAction::ToggleCheckbox { checkbox });
        crate::handle_widget_actions(&mut world);

        let state = world
            .get::<crate::UiCheckbox>(checkbox)
            .expect("checkbox should exist");
        assert!(!state.indeterminate, "indeterminate should clear on toggle");
        assert!(state.checked, "indeterminate toggle should land on checked");

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiCheckboxChanged>();
        assert_eq!(changed.len(), 1);
        assert!(changed[0].action.checked);
        assert!(!changed[0].action.indeterminate);
    }

    #[test]
    fn direct_text_input_actions_update_new_input_state() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let password = world
            .spawn((crate::UiPasswordInput::new("pw").with_mask('*'),))
            .id();
        let multiline = world
            .spawn((crate::UiMultilineTextInput::new("before"),))
            .id();

        world.resource::<UiEventQueue>().push_typed(
            password,
            crate::WidgetUiAction::SetPasswordInputDisplay {
                input: password,
                display_value: "**d".to_string(),
            },
        );
        world.resource::<UiEventQueue>().push_typed(
            multiline,
            crate::WidgetUiAction::SetMultilineTextInput {
                input: multiline,
                value: "a\nb".to_string(),
            },
        );

        crate::handle_widget_actions(&mut world);

        let password_state = world
            .get::<crate::UiPasswordInput>(password)
            .expect("password input should exist");
        assert_eq!(password_state.value, "pwd");
        let multiline_state = world
            .get::<crate::UiMultilineTextInput>(multiline)
            .expect("multiline input should exist");
        assert_eq!(multiline_state.value, "a\nb");

        let password_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiPasswordInputChanged>();
        assert_eq!(password_changed.len(), 1);
        assert_eq!(password_changed[0].action.value, "pwd");

        let multiline_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiMultilineTextInputChanged>();
        assert_eq!(multiline_changed.len(), 1);
        assert_eq!(multiline_changed[0].action.value, "a\nb");
    }

    #[test]
    fn direct_search_action_updates_search_state() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let search = world.spawn((crate::UiSearch::new("Find"),)).id();

        world.resource::<UiEventQueue>().push_typed(
            search,
            crate::WidgetUiAction::SetSearch {
                search,
                value: "button".to_string(),
            },
        );

        crate::handle_widget_actions(&mut world);

        let search_state = world
            .get::<crate::UiSearch>(search)
            .expect("search should exist");
        assert_eq!(search_state.value, "button");

        let changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiSearchChanged>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].action.value, "button");
    }

    #[test]
    fn new_input_options_enforce_read_only_and_max_length() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let password = world
            .spawn((crate::UiPasswordInput::new("pw")
                .with_mask('*')
                .with_max_length(3),))
            .id();
        let read_only = world
            .spawn((crate::UiPasswordInput::new("stay").read_only(true),))
            .id();
        let multiline = world
            .spawn((crate::UiMultilineTextInput::new("before").with_max_length(4),))
            .id();

        world.resource::<UiEventQueue>().push_typed(
            password,
            crate::WidgetUiAction::SetPasswordInputDisplay {
                input: password,
                display_value: "**def".to_string(),
            },
        );
        world.resource::<UiEventQueue>().push_typed(
            read_only,
            crate::WidgetUiAction::SetPasswordInputDisplay {
                input: read_only,
                display_value: "changed".to_string(),
            },
        );
        world.resource::<UiEventQueue>().push_typed(
            multiline,
            crate::WidgetUiAction::SetMultilineTextInput {
                input: multiline,
                value: "abcdef".to_string(),
            },
        );

        crate::handle_widget_actions(&mut world);

        assert_eq!(
            world
                .get::<crate::UiPasswordInput>(password)
                .expect("password input should exist")
                .value,
            "pwd"
        );
        assert_eq!(
            world
                .get::<crate::UiPasswordInput>(read_only)
                .expect("read-only password input should exist")
                .value,
            "stay"
        );
        assert_eq!(
            world
                .get::<crate::UiMultilineTextInput>(multiline)
                .expect("multiline input should exist")
                .value,
            "abcd"
        );

        let password_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiPasswordInputChanged>();
        assert_eq!(password_changed.len(), 1);

        let multiline_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiMultilineTextInputChanged>();
        assert_eq!(multiline_changed.len(), 1);
        assert_eq!(multiline_changed[0].action.value, "abcd");
    }

    #[test]
    fn direct_selection_actions_update_list_and_data_table_state() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let list = world
            .spawn((crate::UiListView::new(["one", "two", "three"]),))
            .id();
        let table = world
            .spawn((crate::UiDataTable::from_labels(["Name"]).with_cells("1", ["Ada"]),))
            .id();

        world.resource::<UiEventQueue>().push_typed(
            list,
            crate::WidgetUiAction::SelectListItem {
                list_view: list,
                index: 2,
            },
        );
        world.resource::<UiEventQueue>().push_typed(
            table,
            crate::WidgetUiAction::SelectDataTableRow { table, row: 0 },
        );

        crate::handle_widget_actions(&mut world);

        assert_eq!(
            world
                .get::<crate::UiListView>(list)
                .expect("list view should exist")
                .selected,
            Some(2)
        );
        assert_eq!(
            world
                .get::<crate::UiDataTable>(table)
                .expect("data table should exist")
                .selected_row,
            Some(0)
        );

        let list_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiListViewSelectionChanged>();
        assert_eq!(list_changed.len(), 1);
        assert_eq!(list_changed[0].action.selected, Some(2));
        assert_eq!(list_changed[0].action.selected_indices, vec![2]);

        let table_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiDataTableSelectionChanged>();
        assert_eq!(table_changed.len(), 1);
        assert_eq!(table_changed[0].action.selected_row, Some(0));
        assert_eq!(table_changed[0].action.selected_rows, vec![0]);
    }

    #[test]
    fn new_selection_options_support_multiple_and_data_table_sorting() {
        let mut world = World::new();
        world.insert_resource(UiEventQueue::default());

        let list = world
            .spawn((crate::UiListView::new(["one", "two", "three"])
                .with_selection_mode(crate::UiListSelectionMode::Multiple)
                .with_item_height(24.0)
                .with_item_padding(3.0),))
            .id();
        let table = world
            .spawn((crate::UiDataTable::new([
                crate::UiDataColumn::new("name", "Name").width(120.0),
                crate::UiDataColumn::new("role", "Role"),
            ])
            .with_selection_mode(crate::UiListSelectionMode::Multiple)
            .striped(true)
            .with_cells("2", ["Grace", "Admiral"])
            .with_cells("1", ["Ada", "Engineer"]),))
            .id();

        for index in [0, 2, 0] {
            world.resource::<UiEventQueue>().push_typed(
                list,
                crate::WidgetUiAction::SelectListItem {
                    list_view: list,
                    index,
                },
            );
        }
        world.resource::<UiEventQueue>().push_typed(
            table,
            crate::WidgetUiAction::SelectDataTableRow { table, row: 1 },
        );
        world.resource::<UiEventQueue>().push_typed(
            table,
            crate::WidgetUiAction::SortDataTableColumn { table, column: 0 },
        );

        crate::handle_widget_actions(&mut world);

        let list_state = world
            .get::<crate::UiListView>(list)
            .expect("list view should exist");
        assert_eq!(list_state.clamped_selected_indices(), vec![2]);
        assert_eq!(list_state.selected, Some(2));

        let table_state = world
            .get::<crate::UiDataTable>(table)
            .expect("data table should exist");
        assert_eq!(table_state.clamped_selected_rows(), vec![1]);
        assert_eq!(
            table_state.sort,
            Some(crate::UiDataTableSort::new(
                0,
                crate::UiSortDirection::Ascending
            ))
        );
        assert_eq!(table_state.sorted_row_indices(), vec![1, 0]);

        let list_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiListViewSelectionChanged>();
        assert_eq!(list_changed.len(), 3);
        assert_eq!(
            list_changed.last().unwrap().action.selected_indices,
            vec![2]
        );

        let sort_changed = world
            .resource_mut::<UiEventQueue>()
            .drain_actions::<crate::UiDataTableSortChanged>();
        assert_eq!(sort_changed.len(), 1);
        assert_eq!(
            sort_changed[0].action.sort,
            crate::UiDataTableSort::new(0, crate::UiSortDirection::Ascending)
        );
    }

    #[test]
    fn new_grid_canvas_and_image_options_are_data_complete() {
        let tracks = crate::UiGrid::parse_tracks("Auto, *, 2*, 120px, 48")
            .expect("grid track spec should parse");
        assert_eq!(
            tracks,
            vec![
                crate::UiGridLength::Auto,
                crate::UiGridLength::Star(1.0),
                crate::UiGridLength::Star(2.0),
                crate::UiGridLength::Px(120.0),
                crate::UiGridLength::Px(48.0),
            ]
        );

        let grid = crate::UiGrid::new(1, 1)
            .try_with_columns_spec("Auto 2* 80")
            .expect("column spec should parse")
            .with_auto_flow(crate::UiGridAutoFlow::Column)
            .auto_indexing(false)
            .show_grid_lines(true)
            .share_star_size(true);
        assert_eq!(grid.effective_columns(), 3);
        assert_eq!(grid.auto_flow, crate::UiGridAutoFlow::Column);
        assert!(!grid.auto_indexing);
        assert!(grid.show_grid_lines);
        assert!(grid.share_star_size);

        let cell = crate::UiGridCell::row(1).with_column(2).with_span(3, 2);
        assert!(cell.has_row);
        assert!(cell.has_column);
        assert_eq!(cell.row_span, 2);
        assert_eq!(cell.column_span, 3);

        let canvas = crate::UiCanvas::new()
            .with_command(crate::UiCanvasCommand::FillCanvas {
                color: crate::xilem::Color::from_rgb8(0, 0, 0),
            })
            .with_command(crate::UiCanvasCommand::FillPath {
                commands: vec![
                    crate::UiCanvasPathCommand::MoveTo { x: 0.0, y: 0.0 },
                    crate::UiCanvasPathCommand::LineTo { x: 8.0, y: 0.0 },
                    crate::UiCanvasPathCommand::LineTo { x: 8.0, y: 8.0 },
                    crate::UiCanvasPathCommand::ClosePath,
                ],
                color: crate::xilem::Color::from_rgb8(255, 0, 0),
            });
        assert_eq!(canvas.commands.len(), 2);
        assert_eq!(
            crate::UiCanvasPosition::new(12.0, 24.0).offset((0.0, 0.0)),
            (12.0, 24.0)
        );

        // Right/bottom anchoring resolves against the canvas size.
        let right_bottom = crate::UiCanvasPosition::default()
            .with_right(10.0)
            .with_bottom(20.0);
        assert_eq!(
            right_bottom.offset((300.0, 200.0)),
            (290.0, 180.0),
            "right/bottom should offset from the far edges of the canvas"
        );

        // Gradient commands carry their stops through the canvas component.
        let gradient_canvas =
            crate::UiCanvas::new().with_command(crate::UiCanvasCommand::FillLinearGradientRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                start_x: 0.0,
                start_y: 0.0,
                end_x: 100.0,
                end_y: 0.0,
                stops: vec![
                    crate::UiGradientStop::new(0.0, crate::xilem::Color::from_rgb8(0, 0, 0)),
                    crate::UiGradientStop::new(1.0, crate::xilem::Color::from_rgb8(255, 255, 255)),
                ],
            });
        assert_eq!(gradient_canvas.commands.len(), 1);

        let image = crate::UiImage::from_bgra8(2, 1, vec![0, 0, 255, 255, 0, 255, 0, 128])
            .quality(masonry_core::peniko::ImageQuality::High)
            .alpha(0.5)
            .view_box(crate::UiImageViewBox::pixels(0.0, 0.0, 1.0, 1.0))
            .alignment(
                crate::UiImageAlignmentX::Right,
                crate::UiImageAlignmentY::Bottom,
            );
        assert_eq!(image.source_size(), Some((2, 1)));
        assert_eq!(image.peek_rgba8(0, 0), Some([255, 0, 0, 255]));
        assert_eq!(image.peek_rgba8(1, 0), Some([0, 255, 0, 128]));
        assert_eq!(
            image
                .peek_color(1, 0)
                .expect("pixel should exist")
                .to_rgba8()
                .to_u8_array(),
            [0, 255, 0, 128]
        );
    }

    #[test]
    fn data_row_accepts_image_cell_templates() {
        let row = crate::UiDataRow::new("1", ["text", "more text"])
            .with_cell_image(0, crate::UiImage::empty().with_alt_text("icon"));
        assert!(matches!(row.cells[0], crate::UiDataCell::Image(_)));
        assert!(matches!(row.cells[1], crate::UiDataCell::Text(_)));
        assert_eq!(
            row.cells[0].text(),
            "icon",
            "image cell text falls back to alt_text"
        );
        assert_eq!(row.cells[1].text(), "more text");
    }
}
