use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::*,
};

use crate::{IconGlyph, ProjectionCtx, UiView, components::UiComponentTemplate};

/// A single item in the navigation view sidebar.
///
/// Each item has a display label and an optional icon glyph source.
#[derive(Debug, Clone, Default)]
pub struct NavigationViewItem {
    /// Human-readable label shown in the sidebar.
    pub label: String,
    /// Optional icon glyph and font stack.
    pub icon: Option<IconGlyph>,
}

impl NavigationViewItem {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
        }
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<IconGlyph>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// ECS template entity for one [`UiNavigationView`] sidebar item.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiNavigationItem {
    /// Parent navigation view entity.
    pub nav: Entity,
    /// Index into [`UiNavigationView::items`].
    pub index: usize,
}

impl Default for UiNavigationItem {
    fn default() -> Self {
        Self {
            nav: Entity::PLACEHOLDER,
            index: 0,
        }
    }
}

/// Sidebar navigation container with items and a content area.
///
/// The sidebar is rendered as a vertical list of ECS-backed [`UiNavigationItem`]
/// template entities (with optional icon glyphs). The content area displays
/// the non-template ECS child at [`selected`] index — analogous to a
/// [`UiTabBar`](crate::UiTabBar) with hidden headers but with a separate
/// navigation panel.
///
/// # Styling classes
///
/// The projector resolves these class names from the style system:
/// - `"nav.sidebar"` — sidebar panel
/// - `"nav.item"` — each navigation button (base)
/// - `"nav.item.active"` — the active navigation button
/// - `"nav.content"` — content area wrapper
#[derive(Component, Debug, Clone)]
pub struct UiNavigationView {
    /// Navigation items displayed in the sidebar.
    pub items: Vec<NavigationViewItem>,
    /// Index of the currently selected item.
    pub selected: usize,
}

impl UiNavigationView {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = NavigationViewItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            selected: 0,
        }
    }

    #[must_use]
    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }
}

impl Default for UiNavigationView {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// Emitted when the selected item in a [`UiNavigationView`] changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationSelectionChanged {
    /// The navigation view entity.
    pub nav: Entity,
    /// The newly selected index.
    pub selected: usize,
}

impl UiComponentTemplate for UiNavigationView {
    fn expand(world: &mut World, entity: Entity) {
        sync_navigation_view_item_entities(world, entity);
    }

    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_navigation_view(component, ctx)
    }
}

impl UiComponentTemplate for UiNavigationItem {
    fn project(component: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        crate::projection::widgets::project_navigation_item(component, ctx)
    }
}

pub(crate) fn sync_navigation_view_item_templates(world: &mut World) {
    let nav_entities = {
        let mut query =
            world.query_filtered::<Entity, (With<UiNavigationView>, Changed<UiNavigationView>)>();
        query.iter(world).collect::<Vec<_>>()
    };

    for nav in nav_entities {
        sync_navigation_view_item_entities(world, nav);
    }
}

fn sync_navigation_view_item_entities(world: &mut World, nav: Entity) {
    let Some(item_count) = world
        .get::<UiNavigationView>(nav)
        .map(|view| view.items.len())
    else {
        return;
    };

    let existing = world
        .get::<Children>(nav)
        .map(|children| {
            children
                .iter()
                .filter_map(|child| {
                    world
                        .get::<UiNavigationItem>(child)
                        .filter(|item| item.nav == nav)
                        .map(|item| (child, item.index))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut by_index: Vec<Option<Entity>> = vec![None; item_count];
    let mut stale = Vec::new();

    for (entity, index) in existing {
        if index < item_count && by_index[index].is_none() {
            by_index[index] = Some(entity);
        } else {
            stale.push(entity);
        }
    }

    for entity in stale {
        let _ = world.despawn(entity);
    }

    for (index, item) in by_index.iter().enumerate().take(item_count) {
        if item.is_none() {
            world.spawn((UiNavigationItem { nav, index }, ChildOf(nav)));
        }
    }
}
