//! WinUI-style hierarchical [`UiNavigationView`].
//!
//! Mirrors `Microsoft.UI.Xaml.Controls.NavigationView`:
//! - Top-level items may contain nested `MenuItems` ([`NavigationViewItem::children`])
//! - Parent items expand/collapse via [`NavigationViewItem::is_expanded`]
//! - Leaf items are selectable and map 1:1 to content children by leaf index
//! - The pane itself can also compact via [`UiNavigationView::is_pane_open`]

use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::*,
};

use crate::{IconGlyph, ProjectionCtx, UiView, components::UiComponentTemplate};

/// A single item in the navigation view sidebar.
///
/// Matches WinUI `NavigationViewItem`: a label/icon row that may host nested
/// [`children`](Self::children) (`MenuItems`). Parents toggle
/// [`is_expanded`](Self::is_expanded); leaves are selectable content destinations.
#[derive(Debug, Clone, Default)]
pub struct NavigationViewItem {
    /// Human-readable label shown in the sidebar.
    pub label: String,
    /// Optional icon glyph and font stack.
    pub icon: Option<IconGlyph>,
    /// Nested menu items (WinUI `NavigationViewItem.MenuItems`).
    pub children: Vec<NavigationViewItem>,
    /// Whether nested children are visible (WinUI `IsExpanded`).
    ///
    /// Ignored for leaf items (`children` empty).
    pub is_expanded: bool,
}

impl NavigationViewItem {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            children: Vec::new(),
            is_expanded: true,
        }
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<IconGlyph>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Attach nested menu items (WinUI `MenuItems`). Defaults to expanded.
    #[must_use]
    pub fn with_children(mut self, children: impl IntoIterator<Item = NavigationViewItem>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    /// Set whether nested children start expanded.
    #[must_use]
    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.is_expanded = expanded;
        self
    }

    /// Expand nested children (WinUI `IsExpanded = true`).
    #[must_use]
    pub fn expanded(self) -> Self {
        self.with_expanded(true)
    }

    /// Collapse nested children (WinUI `IsExpanded = false`).
    #[must_use]
    pub fn collapsed(self) -> Self {
        self.with_expanded(false)
    }

    /// Whether this item is a leaf (no nested menu items).
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Total leaf count under this node (1 if leaf).
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        if self.is_leaf() {
            1
        } else {
            self.children.iter().map(Self::leaf_count).sum()
        }
    }
}

/// ECS template entity for one [`UiNavigationView`] sidebar item.
///
/// Hierarchical items are nested via [`ChildOf`]: top-level items are children
/// of the nav entity; nested menu items are children of their parent item entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiNavigationItem {
    /// Parent navigation view entity.
    pub nav: Entity,
    /// Index among siblings in [`NavigationViewItem::children`] (or root `items`).
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

/// Expanded pane width in logical pixels (labels + icons).
pub const NAV_PANE_EXPANDED_WIDTH: f64 = 280.0;
/// Compact pane width in logical pixels (icons only).
pub const NAV_PANE_COMPACT_WIDTH: f64 = 48.0;

/// Sidebar navigation container with items and a content area.
///
/// The sidebar is rendered as a vertical list of ECS-backed [`UiNavigationItem`]
/// template entities (with optional nested menu items). The content area
/// displays the non-template ECS child at the selected **leaf** index.
///
/// # Hierarchy (WinUI)
///
/// ```text
/// UiNavigationView.items
///   ├── Category (children, is_expanded)
///   │     ├── Leaf page  → content[leaf_index]
///   │     └── Leaf page
///   └── Category
///         └── ...
/// ```
///
/// # Styling classes
///
/// - `"nav.sidebar"` — sidebar panel
/// - `"nav.toggle"` — pane open/close (hamburger) button
/// - `"nav.item"` — each navigation button (base)
/// - `"nav.item.active"` — the active leaf navigation button
/// - `"nav.item.indicator"` — left selection indicator on the active leaf
/// - `"nav.item.chevron"` — expand/collapse chevron on parent items
/// - `"nav.content"` — content area wrapper
#[derive(Component, Debug, Clone)]
pub struct UiNavigationView {
    /// Navigation items displayed in the sidebar (may be hierarchical).
    pub items: Vec<NavigationViewItem>,
    /// Index of the currently selected **leaf** among all leaves in DFS order.
    pub selected: usize,
    /// Whether the navigation pane is expanded (`true`) or compact (`false`).
    pub is_pane_open: bool,
}

impl UiNavigationView {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = NavigationViewItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            selected: 0,
            is_pane_open: true,
        }
    }

    #[must_use]
    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    /// Set whether the navigation pane starts expanded.
    #[must_use]
    pub fn with_pane_open(mut self, open: bool) -> Self {
        self.is_pane_open = open;
        self
    }

    /// Total number of selectable leaf items.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.items.iter().map(NavigationViewItem::leaf_count).sum()
    }
}

impl Default for UiNavigationView {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// Emitted when the selected leaf in a [`UiNavigationView`] changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationSelectionChanged {
    /// The navigation view entity.
    pub nav: Entity,
    /// The newly selected leaf index.
    pub selected: usize,
}

/// Emitted when a [`UiNavigationView`] pane is expanded or collapsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationPaneChanged {
    /// The navigation view entity.
    pub nav: Entity,
    /// Whether the pane is now expanded.
    pub is_pane_open: bool,
}

/// Emitted when a hierarchical navigation item expands or collapses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationItemExpandedChanged {
    /// The navigation view entity.
    pub nav: Entity,
    /// The item template entity that was toggled.
    pub item: Entity,
    /// Whether the item is now expanded.
    pub is_expanded: bool,
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

/// Sync the full hierarchical template tree for one navigation view.
fn sync_navigation_view_item_entities(world: &mut World, nav: Entity) {
    let Some(items) = world
        .get::<UiNavigationView>(nav)
        .map(|view| view.items.clone())
    else {
        return;
    };

    sync_children_for_parent(world, nav, nav, &items);
}

/// Ensure `parent_entity` has one [`UiNavigationItem`] child per entry in `items`,
/// and recurse into nested menu items.
fn sync_children_for_parent(
    world: &mut World,
    nav: Entity,
    parent_entity: Entity,
    items: &[NavigationViewItem],
) {
    let item_count = items.len();

    let existing = world
        .get::<Children>(parent_entity)
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
        // Despawn recursive — nested UiNavigationItem children go with the parent.
        let _ = world.despawn(entity);
    }

    for (index, slot) in by_index.iter_mut().enumerate().take(item_count) {
        let entity = if let Some(existing) = *slot {
            existing
        } else {
            let id = world
                .spawn((UiNavigationItem { nav, index }, ChildOf(parent_entity)))
                .id();
            *slot = Some(id);
            id
        };
        sync_children_for_parent(world, nav, entity, &items[index].children);
    }
}

// ---------------------------------------------------------------------------
// Tree path helpers (shared by projection + actions)
// ---------------------------------------------------------------------------

/// Walk [`ChildOf`] / [`UiNavigationItem`] to recover the path from the nav root.
pub(crate) fn navigation_item_path(world: &World, mut entity: Entity) -> Option<Vec<usize>> {
    let mut path = Vec::new();
    loop {
        let item = world.get::<UiNavigationItem>(entity)?;
        path.push(item.index);
        let parent = world.get::<ChildOf>(entity)?.parent();
        if world.get::<UiNavigationItem>(parent).is_some() {
            entity = parent;
        } else {
            // parent is the nav (or something else) — done
            break;
        }
    }
    path.reverse();
    Some(path)
}

/// Immutable lookup of a nav item by path.
pub(crate) fn navigation_item_at<'a>(
    items: &'a [NavigationViewItem],
    path: &[usize],
) -> Option<&'a NavigationViewItem> {
    let mut current = items;
    let mut last = None;
    for &index in path {
        let item = current.get(index)?;
        last = Some(item);
        current = &item.children;
    }
    last
}

/// Mutable lookup of a nav item by path.
pub(crate) fn navigation_item_at_mut<'a>(
    items: &'a mut [NavigationViewItem],
    path: &[usize],
) -> Option<&'a mut NavigationViewItem> {
    match path {
        [] => None,
        [index] => items.get_mut(*index),
        [index, rest @ ..] => {
            let item = items.get_mut(*index)?;
            navigation_item_at_mut(&mut item.children, rest)
        }
    }
}

/// Count leaves visited in DFS **before** `path` (not including the path node subtree).
///
/// Returns `None` if `path` is invalid. The returned count is also the first leaf
/// index under the path node (or the leaf's own index when the path is a leaf).
pub(crate) fn first_leaf_index_for_path(
    items: &[NavigationViewItem],
    path: &[usize],
) -> Option<usize> {
    let mut count = 0usize;
    if !count_leaves_before(items, path, 0, &mut count) {
        return None;
    }
    Some(count)
}

/// DFS leaf index for a path, or `None` if the path points at a non-leaf.
pub(crate) fn leaf_index_for_path(items: &[NavigationViewItem], path: &[usize]) -> Option<usize> {
    let target = navigation_item_at(items, path)?;
    if !target.is_leaf() {
        return None;
    }
    first_leaf_index_for_path(items, path)
}

/// Count leaves visited in DFS before `path`. Returns false if path is invalid.
fn count_leaves_before(
    items: &[NavigationViewItem],
    path: &[usize],
    depth: usize,
    count: &mut usize,
) -> bool {
    if path.is_empty() || depth >= path.len() {
        return !path.is_empty();
    }
    let target_index = path[depth];
    if target_index >= items.len() {
        return false;
    }
    for (i, item) in items.iter().enumerate() {
        if i < target_index {
            *count += item.leaf_count();
        } else if i == target_index {
            if depth + 1 == path.len() {
                // Path ends on this node — `count` is the first leaf under it.
                return true;
            }
            return count_leaves_before(&item.children, path, depth + 1, count);
        }
    }
    false
}

/// Whether any leaf under this node is the selected leaf index.
///
/// `next_leaf` must start at the first leaf index under `item`.
pub(crate) fn subtree_contains_leaf(
    item: &NavigationViewItem,
    selected_leaf: usize,
    next_leaf: &mut usize,
) -> bool {
    if item.is_leaf() {
        let is_selected = *next_leaf == selected_leaf;
        *next_leaf += 1;
        return is_selected;
    }
    let mut found = false;
    for child in &item.children {
        if subtree_contains_leaf(child, selected_leaf, next_leaf) {
            found = true;
        }
    }
    found
}
