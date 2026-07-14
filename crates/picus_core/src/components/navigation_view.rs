//! WinUI-style hierarchical [`UiNavigationView`].
//!
//! Mirrors the left-shell subset of `Microsoft.UI.Xaml.Controls.NavigationView`:
//! - Hierarchical `MenuItems` with expand/collapse and compact flyouts
//! - Footer menu items + optional Settings leaf
//! - Pane display modes (Left / LeftCompact / LeftMinimal / Auto)
//! - Pane title, toggle, back button chrome
//! - Headers and separators
//! - Unified leaf selection across menu → footer → settings
//!
//! Content children of the nav entity map 1:1 to **selectable leaves** in DFS
//! order (menu leaves, then footer leaves, then Settings when visible).

use bevy_ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    prelude::*,
};

use crate::{IconGlyph, ProjectionCtx, UiView, components::UiComponentTemplate};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// App policy for how the navigation pane is displayed (WinUI `PaneDisplayMode`).
///
/// Top mode is intentionally deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationPaneDisplayMode {
    /// Resolve Expanded / Compact / Minimal from window width thresholds.
    Auto,
    /// Always Expanded (inline pane). Default keeps gallery demos stable.
    #[default]
    Left,
    /// Always Compact (icon rail; open pane overlays content).
    LeftCompact,
    /// Always Minimal (no rail when closed; open pane overlays content).
    LeftMinimal,
}

/// Resolved runtime pane mode (WinUI `DisplayMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationDisplayMode {
    Minimal,
    Compact,
    #[default]
    Expanded,
}

/// Visibility policy for the back button (WinUI `IsBackButtonVisible`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationBackButtonVisible {
    #[default]
    Collapsed,
    Visible,
    /// Treated as Visible on desktop Picus builds.
    Auto,
}

/// Kind of a navigation authoring entry (WinUI item base types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationViewItemKind {
    #[default]
    Item,
    Header,
    Separator,
}

/// Which list a template item belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigationItemRegion {
    #[default]
    Menu,
    Footer,
    Settings,
}

// ---------------------------------------------------------------------------
// Authoring item
// ---------------------------------------------------------------------------

/// A single item in the navigation view sidebar.
///
/// Matches WinUI `NavigationViewItem` for [`NavigationViewItemKind::Item`],
/// plus header/separator variants.
#[derive(Debug, Clone, Default)]
pub struct NavigationViewItem {
    /// Human-readable label shown in the sidebar (ignored for separators).
    pub label: String,
    /// Optional icon glyph and font stack.
    pub icon: Option<IconGlyph>,
    /// Nested menu items (WinUI `NavigationViewItem.MenuItems`).
    pub children: Vec<NavigationViewItem>,
    /// Whether nested children are visible when shown inline (WinUI `IsExpanded`).
    ///
    /// Ignored for leaf items and non-Item kinds. Defaults to **false** (WinUI).
    pub is_expanded: bool,
    /// Entry kind.
    pub kind: NavigationViewItemKind,
    /// When false, invoke expands/invokes without moving selection (WinUI
    /// `SelectsOnInvoked`). Default true.
    pub selects_on_invoked: bool,
    /// Optional compact badge text (WinUI `InfoBadge` simplified).
    pub info_badge: Option<String>,
}

impl NavigationViewItem {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            children: Vec::new(),
            is_expanded: false,
            kind: NavigationViewItemKind::Item,
            selects_on_invoked: true,
            info_badge: None,
        }
    }

    /// Non-interactive section header.
    #[must_use]
    pub fn header(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            kind: NavigationViewItemKind::Header,
            selects_on_invoked: false,
            ..Default::default()
        }
    }

    /// Horizontal separator rule.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            kind: NavigationViewItemKind::Separator,
            selects_on_invoked: false,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<IconGlyph>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Attach nested menu items (WinUI `MenuItems`). Starts collapsed.
    #[must_use]
    pub fn with_children(mut self, children: impl IntoIterator<Item = NavigationViewItem>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.is_expanded = expanded;
        self
    }

    #[must_use]
    pub fn expanded(self) -> Self {
        self.with_expanded(true)
    }

    #[must_use]
    pub fn collapsed(self) -> Self {
        self.with_expanded(false)
    }

    #[must_use]
    pub fn with_selects_on_invoked(mut self, selects: bool) -> Self {
        self.selects_on_invoked = selects;
        self
    }

    #[must_use]
    pub fn with_info_badge(mut self, badge: impl Into<String>) -> Self {
        self.info_badge = Some(badge.into());
        self
    }

    /// Whether this is a selectable destination (Item kind with no children).
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        self.kind == NavigationViewItemKind::Item && self.children.is_empty()
    }

    /// Whether this entry participates in selection as a leaf destination.
    #[must_use]
    pub fn is_selectable_leaf(&self) -> bool {
        self.is_leaf()
    }

    /// Whether this is a parent Item with nested menu items.
    #[must_use]
    pub fn is_parent(&self) -> bool {
        self.kind == NavigationViewItemKind::Item && !self.children.is_empty()
    }

    /// Total selectable leaf count under this node (1 if selectable leaf).
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        match self.kind {
            NavigationViewItemKind::Header | NavigationViewItemKind::Separator => 0,
            NavigationViewItemKind::Item if self.children.is_empty() => 1,
            NavigationViewItemKind::Item => self.children.iter().map(Self::leaf_count).sum(),
        }
    }
}

// ---------------------------------------------------------------------------
// Template entity
// ---------------------------------------------------------------------------

/// ECS template entity for one [`UiNavigationView`] sidebar item.
///
/// Hierarchical items are nested via [`ChildOf`]: top-level menu and footer
/// items are children of the nav entity (distinguished by
/// [`NavigationItemRegion`]); nested menu items are children of their parent
/// item entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiNavigationItem {
    /// Parent navigation view entity.
    pub nav: Entity,
    /// Index among siblings in the authoring list (or 0 for Settings).
    pub index: usize,
    /// Menu, footer, or synthetic Settings region.
    pub region: NavigationItemRegion,
}

impl Default for UiNavigationItem {
    fn default() -> Self {
        Self {
            nav: Entity::PLACEHOLDER,
            index: 0,
            region: NavigationItemRegion::Menu,
        }
    }
}

/// Marker on the synthetic Settings template entity.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiNavigationSettingsItem;

// ---------------------------------------------------------------------------
// Widths
// ---------------------------------------------------------------------------

/// Expanded pane width in logical pixels (WinUI `OpenPaneLength` default).
pub const NAV_PANE_EXPANDED_WIDTH: f64 = 320.0;
/// Compact pane width in logical pixels (WinUI `CompactPaneLength` default).
pub const NAV_PANE_COMPACT_WIDTH: f64 = 48.0;
/// WinUI `CompactModeThresholdWidth` default.
pub const NAV_COMPACT_MODE_THRESHOLD: f64 = 641.0;
/// WinUI `ExpandedModeThresholdWidth` default.
pub const NAV_EXPANDED_MODE_THRESHOLD: f64 = 1008.0;

// ---------------------------------------------------------------------------
// Shell component
// ---------------------------------------------------------------------------

/// Sidebar navigation container with items and a content area.
///
/// # Hierarchy (WinUI left shell)
///
/// ```text
/// UiNavigationView
///   ├── Menu items (hierarchical)
///   ├── Footer host
///   │     └── Footer items (+ Settings leaf)
///   └── Content children [selected leaf index]
/// ```
///
/// # Styling classes
///
/// - `"nav.sidebar"` — sidebar panel
/// - `"nav.toggle"` — pane open/close (hamburger) button
/// - `"nav.back"` — back button
/// - `"nav.pane_title"` — pane title label
/// - `"nav.item"` — navigation button (base)
/// - `"nav.item.active"` — active leaf
/// - `"nav.item.indicator"` — left selection indicator
/// - `"nav.item.chevron"` — expand/collapse chevron
/// - `"nav.item.child_selected"` — parent with selected descendant
/// - `"nav.header"` — section header
/// - `"nav.separator"` — separator rule
/// - `"nav.footer"` — footer region
/// - `"nav.content"` — content area wrapper
/// - `"nav.content.header"` — optional content header
/// - `"nav.flyout"` — compact hierarchy flyout panel
/// - `"nav.overlay_scrim"` — light-dismiss overlay behind open compact/minimal pane
#[derive(Component, Debug, Clone)]
pub struct UiNavigationView {
    /// Top-level menu items (WinUI `MenuItems`).
    pub items: Vec<NavigationViewItem>,
    /// Footer menu items (WinUI `FooterMenuItems`), above Settings when visible.
    pub footer_items: Vec<NavigationViewItem>,
    /// Index of the currently selected **selectable leaf** among menu → footer →
    /// settings in DFS order.
    pub selected: usize,
    /// Whether the navigation pane is open.
    pub is_pane_open: bool,
    /// App pane display policy (WinUI `PaneDisplayMode`).
    pub pane_display_mode: NavigationPaneDisplayMode,
    /// Resolved display mode (WinUI `DisplayMode`). Updated by runtime systems.
    pub display_mode: NavigationDisplayMode,
    /// Open pane width (WinUI `OpenPaneLength`).
    pub open_pane_length: f64,
    /// Compact rail width (WinUI `CompactPaneLength`).
    pub compact_pane_length: f64,
    /// Auto mode: width below this → Minimal.
    pub compact_mode_threshold: f64,
    /// Auto mode: width at/above this → Expanded.
    pub expanded_mode_threshold: f64,
    /// Show framework Settings leaf after footer items.
    pub is_settings_visible: bool,
    /// Settings item label.
    pub settings_label: String,
    /// Show the hamburger / pane toggle control.
    pub is_pane_toggle_button_visible: bool,
    /// Optional title next to the toggle when the pane is open.
    pub pane_title: String,
    /// Back button visibility policy.
    pub is_back_button_visible: NavigationBackButtonVisible,
    /// Whether the back button is enabled (emits [`UiNavigationBackRequested`]).
    pub is_back_enabled: bool,
    /// Optional content-area header text (WinUI `Header` simplified).
    pub header: String,
    /// User force-closed the pane; Auto→Expanded will not reopen until toggled open.
    pub force_closed: bool,
    /// Entity of the open hierarchy flyout overlay, if any.
    pub open_flyout_item: Option<Entity>,
}

impl UiNavigationView {
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = NavigationViewItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            footer_items: Vec::new(),
            selected: 0,
            is_pane_open: true,
            pane_display_mode: NavigationPaneDisplayMode::Left,
            display_mode: NavigationDisplayMode::Expanded,
            open_pane_length: NAV_PANE_EXPANDED_WIDTH,
            compact_pane_length: NAV_PANE_COMPACT_WIDTH,
            compact_mode_threshold: NAV_COMPACT_MODE_THRESHOLD,
            expanded_mode_threshold: NAV_EXPANDED_MODE_THRESHOLD,
            is_settings_visible: true,
            settings_label: "Settings".to_string(),
            is_pane_toggle_button_visible: true,
            pane_title: String::new(),
            is_back_button_visible: NavigationBackButtonVisible::Collapsed,
            is_back_enabled: false,
            header: String::new(),
            force_closed: false,
            open_flyout_item: None,
        }
    }

    #[must_use]
    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    #[must_use]
    pub fn with_pane_open(mut self, open: bool) -> Self {
        self.is_pane_open = open;
        self
    }

    #[must_use]
    pub fn with_footer_items(
        mut self,
        items: impl IntoIterator<Item = NavigationViewItem>,
    ) -> Self {
        self.footer_items = items.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_pane_display_mode(mut self, mode: NavigationPaneDisplayMode) -> Self {
        self.pane_display_mode = mode;
        self
    }

    #[must_use]
    pub fn with_open_pane_length(mut self, length: f64) -> Self {
        self.open_pane_length = length.max(0.0);
        self
    }

    #[must_use]
    pub fn with_compact_pane_length(mut self, length: f64) -> Self {
        self.compact_pane_length = length.max(0.0);
        self
    }

    #[must_use]
    pub fn with_settings_visible(mut self, visible: bool) -> Self {
        self.is_settings_visible = visible;
        self
    }

    #[must_use]
    pub fn with_settings_label(mut self, label: impl Into<String>) -> Self {
        self.settings_label = label.into();
        self
    }

    #[must_use]
    pub fn with_pane_title(mut self, title: impl Into<String>) -> Self {
        self.pane_title = title.into();
        self
    }

    #[must_use]
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header = header.into();
        self
    }

    #[must_use]
    pub fn with_back_button(
        mut self,
        visible: NavigationBackButtonVisible,
        enabled: bool,
    ) -> Self {
        self.is_back_button_visible = visible;
        self.is_back_enabled = enabled;
        self
    }

    #[must_use]
    pub fn with_pane_toggle_button_visible(mut self, visible: bool) -> Self {
        self.is_pane_toggle_button_visible = visible;
        self
    }

    /// Selectable leaf count across menu, footer, and optional Settings.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        let menu = self.items.iter().map(NavigationViewItem::leaf_count).sum::<usize>();
        let footer = self
            .footer_items
            .iter()
            .map(NavigationViewItem::leaf_count)
            .sum::<usize>();
        let settings = usize::from(self.is_settings_visible);
        menu + footer + settings
    }

    /// First leaf index of the footer region (or settings/menu end if empty).
    #[must_use]
    pub fn footer_leaf_base(&self) -> usize {
        self.items.iter().map(NavigationViewItem::leaf_count).sum()
    }

    /// Leaf index of the Settings item, if visible.
    #[must_use]
    pub fn settings_leaf_index(&self) -> Option<usize> {
        self.is_settings_visible
            .then(|| self.footer_leaf_base() + self.footer_items.iter().map(NavigationViewItem::leaf_count).sum::<usize>())
    }

    /// Whether the current selection is the Settings leaf.
    #[must_use]
    pub fn is_settings_selected(&self) -> bool {
        self.settings_leaf_index() == Some(self.selected)
    }

    /// Resolve display mode from pane policy and available width.
    #[must_use]
    pub fn resolve_display_mode(&self, width: f64) -> NavigationDisplayMode {
        match self.pane_display_mode {
            NavigationPaneDisplayMode::Left => NavigationDisplayMode::Expanded,
            NavigationPaneDisplayMode::LeftCompact => NavigationDisplayMode::Compact,
            NavigationPaneDisplayMode::LeftMinimal => NavigationDisplayMode::Minimal,
            NavigationPaneDisplayMode::Auto => {
                if width >= self.expanded_mode_threshold {
                    NavigationDisplayMode::Expanded
                } else if width >= self.compact_mode_threshold {
                    NavigationDisplayMode::Compact
                } else {
                    NavigationDisplayMode::Minimal
                }
            }
        }
    }

    /// Whether the back button should be shown.
    #[must_use]
    pub fn back_button_shown(&self) -> bool {
        match self.is_back_button_visible {
            NavigationBackButtonVisible::Collapsed => false,
            NavigationBackButtonVisible::Visible | NavigationBackButtonVisible::Auto => true,
        }
    }

    /// Whether the pane shows labels (open expanded pane, or open overlay).
    #[must_use]
    pub fn pane_shows_labels(&self) -> bool {
        match self.display_mode {
            NavigationDisplayMode::Expanded => self.is_pane_open,
            NavigationDisplayMode::Compact | NavigationDisplayMode::Minimal => self.is_pane_open,
        }
    }

    /// Whether menu items render as an icon-only rail (no labels / no inline children).
    #[must_use]
    pub fn is_rail_mode(&self) -> bool {
        match self.display_mode {
            NavigationDisplayMode::Expanded => !self.is_pane_open,
            NavigationDisplayMode::Compact => !self.is_pane_open,
            NavigationDisplayMode::Minimal => false, // no rail when closed
        }
    }

    /// Whether the open pane is an overlay over content (Compact/Minimal).
    #[must_use]
    pub fn is_overlay_pane(&self) -> bool {
        matches!(
            self.display_mode,
            NavigationDisplayMode::Compact | NavigationDisplayMode::Minimal
        ) && self.is_pane_open
    }

    /// Whether hierarchy should use flyouts instead of inline expand.
    #[must_use]
    pub fn uses_hierarchy_flyout(&self) -> bool {
        // Closed compact rail: parents open flyouts. Open expanded: inline.
        // Open overlay pane: prefer inline expand (like WinUI open pane).
        self.is_rail_mode()
    }
}

impl Default for UiNavigationView {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when the selected leaf in a [`UiNavigationView`] changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationSelectionChanged {
    pub nav: Entity,
    pub selected: usize,
    pub is_settings_selected: bool,
}

/// Emitted when a navigation item is invoked by the user (WinUI `ItemInvoked`).
///
/// Fired even when [`NavigationViewItem::selects_on_invoked`] is false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationItemInvoked {
    pub nav: Entity,
    /// Selectable leaf index when the target is a leaf / Settings; `None` for parents.
    pub selected: Option<usize>,
    pub is_settings_invoked: bool,
    /// Template item entity that was invoked, when available.
    pub item: Option<Entity>,
}

/// Emitted when a [`UiNavigationView`] pane is expanded or collapsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationPaneChanged {
    pub nav: Entity,
    pub is_pane_open: bool,
}

/// Emitted when a hierarchical navigation item expands or collapses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationItemExpandedChanged {
    pub nav: Entity,
    pub item: Entity,
    pub is_expanded: bool,
}

/// Emitted when the resolved [`NavigationDisplayMode`] changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationDisplayModeChanged {
    pub nav: Entity,
    pub display_mode: NavigationDisplayMode,
}

/// Emitted when the back button is pressed (WinUI `BackRequested`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNavigationBackRequested {
    pub nav: Entity,
}

// ---------------------------------------------------------------------------
// Template trait impls
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Template sync
// ---------------------------------------------------------------------------

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

/// Sync menu, footer, and settings template trees for one navigation view.
fn sync_navigation_view_item_entities(world: &mut World, nav: Entity) {
    let Some((items, footer_items, settings_visible)) = world.get::<UiNavigationView>(nav).map(|v| {
        (
            v.items.clone(),
            v.footer_items.clone(),
            v.is_settings_visible,
        )
    }) else {
        return;
    };

    // Menu + footer items are direct children of nav, distinguished by region.
    sync_children_for_parent(world, nav, nav, &items, NavigationItemRegion::Menu);
    sync_children_for_parent(
        world,
        nav,
        nav,
        &footer_items,
        NavigationItemRegion::Footer,
    );
    sync_settings_item(world, nav, settings_visible);
}

fn sync_settings_item(world: &mut World, nav: Entity, visible: bool) {
    let existing = world.get::<Children>(nav).and_then(|children| {
        children
            .iter()
            .find(|child| world.get::<UiNavigationSettingsItem>(*child).is_some())
    });

    match (visible, existing) {
        (true, Some(_)) => {}
        (true, None) => {
            world.spawn((
                UiNavigationItem {
                    nav,
                    index: 0,
                    region: NavigationItemRegion::Settings,
                },
                UiNavigationSettingsItem,
                ChildOf(nav),
            ));
        }
        (false, Some(entity)) => {
            let _ = world.despawn(entity);
        }
        (false, None) => {}
    }
}

/// Ensure `parent_entity` has one [`UiNavigationItem`] child per entry in `items`.
fn sync_children_for_parent(
    world: &mut World,
    nav: Entity,
    parent_entity: Entity,
    items: &[NavigationViewItem],
    region: NavigationItemRegion,
) {
    let item_count = items.len();

    let existing = world
        .get::<Children>(parent_entity)
        .map(|children| {
            children
                .iter()
                .filter_map(|child| {
                    // Settings is managed separately.
                    if world.get::<UiNavigationSettingsItem>(child).is_some() {
                        return None;
                    }
                    world
                        .get::<UiNavigationItem>(child)
                        .filter(|item| item.nav == nav && item.region == region)
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

    for (index, slot) in by_index.iter_mut().enumerate().take(item_count) {
        let entity = if let Some(existing) = *slot {
            if let Some(mut item) = world.get_mut::<UiNavigationItem>(existing) {
                item.region = region;
                item.index = index;
            }
            existing
        } else {
            let id = world
                .spawn((
                    UiNavigationItem {
                        nav,
                        index,
                        region,
                    },
                    ChildOf(parent_entity),
                ))
                .id();
            *slot = Some(id);
            id
        };
        sync_children_for_parent(world, nav, entity, &items[index].children, region);
    }
}

// ---------------------------------------------------------------------------
// Tree path helpers
// ---------------------------------------------------------------------------

/// Walk [`ChildOf`] / [`UiNavigationItem`] to recover the path from the region root.
pub(crate) fn navigation_item_path(world: &World, mut entity: Entity) -> Option<Vec<usize>> {
    let mut path = Vec::new();
    let region = world.get::<UiNavigationItem>(entity)?.region;
    loop {
        let item = world.get::<UiNavigationItem>(entity)?;
        if item.region != region {
            break;
        }
        path.push(item.index);
        let parent = world.get::<ChildOf>(entity)?.parent();
        if world
            .get::<UiNavigationItem>(parent)
            .is_some_and(|p| p.region == region)
        {
            entity = parent;
        } else {
            break;
        }
    }
    path.reverse();
    Some(path)
}

/// Immutable lookup of a nav item by path within a list.
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

/// Resolve authoring item for a template entity.
pub(crate) fn navigation_item_for_entity<'a>(
    nav: &'a UiNavigationView,
    world: &World,
    entity: Entity,
) -> Option<&'a NavigationViewItem> {
    let item = world.get::<UiNavigationItem>(entity)?;
    match item.region {
        NavigationItemRegion::Settings => None,
        NavigationItemRegion::Menu => {
            let path = navigation_item_path(world, entity)?;
            navigation_item_at(&nav.items, &path)
        }
        NavigationItemRegion::Footer => {
            let path = navigation_item_path(world, entity)?;
            navigation_item_at(&nav.footer_items, &path)
        }
    }
}

/// First leaf index for a path within a list, offset by `base`.
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
    if !target.is_selectable_leaf() {
        return None;
    }
    first_leaf_index_for_path(items, path)
}

/// Global leaf index for a template entity (menu + footer + settings).
pub(crate) fn global_leaf_index_for_entity(
    nav: &UiNavigationView,
    world: &World,
    entity: Entity,
) -> Option<usize> {
    let item = world.get::<UiNavigationItem>(entity)?;
    match item.region {
        NavigationItemRegion::Settings => nav.settings_leaf_index(),
        NavigationItemRegion::Menu => {
            let path = navigation_item_path(world, entity)?;
            leaf_index_for_path(&nav.items, &path)
        }
        NavigationItemRegion::Footer => {
            let path = navigation_item_path(world, entity)?;
            let local = leaf_index_for_path(&nav.footer_items, &path)?;
            Some(nav.footer_leaf_base() + local)
        }
    }
}

/// Global first leaf under a parent entity path (for child-selected checks).
pub(crate) fn global_first_leaf_for_entity(
    nav: &UiNavigationView,
    world: &World,
    entity: Entity,
) -> Option<usize> {
    let item = world.get::<UiNavigationItem>(entity)?;
    match item.region {
        NavigationItemRegion::Settings => nav.settings_leaf_index(),
        NavigationItemRegion::Menu => {
            let path = navigation_item_path(world, entity)?;
            first_leaf_index_for_path(&nav.items, &path)
        }
        NavigationItemRegion::Footer => {
            let path = navigation_item_path(world, entity)?;
            let local = first_leaf_index_for_path(&nav.footer_items, &path)?;
            Some(nav.footer_leaf_base() + local)
        }
    }
}

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
    match item.kind {
        NavigationViewItemKind::Header | NavigationViewItemKind::Separator => false,
        NavigationViewItemKind::Item if item.children.is_empty() => {
            let is_selected = *next_leaf == selected_leaf;
            *next_leaf += 1;
            is_selected
        }
        NavigationViewItemKind::Item => {
            let mut found = false;
            for child in &item.children {
                if subtree_contains_leaf(child, selected_leaf, next_leaf) {
                    found = true;
                }
            }
            found
        }
    }
}

// ---------------------------------------------------------------------------
// Display mode system
// ---------------------------------------------------------------------------

/// Resolve and apply adaptive display mode from window width.
pub(crate) fn update_navigation_view_display_mode(world: &mut World) {
    use crate::events::UiEventQueue;
    use bevy_window::{PrimaryWindow, Window};

    let width = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|w| w.resolution.width() as f64)
        .or_else(|| {
            world
                .query::<&Window>()
                .iter(world)
                .next()
                .map(|w| w.resolution.width() as f64)
        })
        .unwrap_or(1280.0);

    let updates = {
        let mut query = world.query::<(Entity, &UiNavigationView)>();
        query
            .iter(world)
            .filter_map(|(entity, nav)| {
                let resolved = nav.resolve_display_mode(width);
                if resolved != nav.display_mode {
                    Some((entity, resolved, nav.display_mode, nav.force_closed, nav.is_pane_open))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    for (entity, resolved, previous, force_closed, is_pane_open) in updates {
        let mut pane_open = is_pane_open;
        let force = force_closed;

        // WinUI-like: enter Compact/Minimal closes pane; enter Expanded opens unless force_closed.
        match resolved {
            NavigationDisplayMode::Expanded => {
                if !force {
                    pane_open = true;
                }
            }
            NavigationDisplayMode::Compact | NavigationDisplayMode::Minimal => {
                pane_open = false;
            }
        }

        // Leaving Expanded into compact modes: force close.
        if matches!(
            previous,
            NavigationDisplayMode::Expanded
        ) && matches!(
            resolved,
            NavigationDisplayMode::Compact | NavigationDisplayMode::Minimal
        ) {
            pane_open = false;
        }

        if let Some(mut nav) = world.get_mut::<UiNavigationView>(entity) {
            nav.display_mode = resolved;
            nav.is_pane_open = pane_open;
            nav.force_closed = force;
        }

        if let Some(queue) = world.get_resource::<UiEventQueue>() {
            queue.push_typed(
                entity,
                UiNavigationDisplayModeChanged {
                    nav: entity,
                    display_mode: resolved,
                },
            );
            if pane_open != is_pane_open {
                queue.push_typed(
                    entity,
                    UiNavigationPaneChanged {
                        nav: entity,
                        is_pane_open: pane_open,
                    },
                );
            }
        }
        let _ = previous;
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_count_skips_headers_and_separators() {
        let items = vec![
            NavigationViewItem::header("Section"),
            NavigationViewItem::new("A"),
            NavigationViewItem::separator(),
            NavigationViewItem::new("B").with_children([
                NavigationViewItem::new("B1"),
                NavigationViewItem::new("B2"),
            ]),
        ];
        let nav = UiNavigationView::new(items).with_settings_visible(false);
        assert_eq!(nav.leaf_count(), 3);
    }

    #[test]
    fn leaf_count_includes_footer_and_settings() {
        let nav = UiNavigationView::new([NavigationViewItem::new("Home")])
            .with_footer_items([NavigationViewItem::new("Account")])
            .with_settings_visible(true);
        assert_eq!(nav.leaf_count(), 3);
        assert_eq!(nav.footer_leaf_base(), 1);
        assert_eq!(nav.settings_leaf_index(), Some(2));
    }

    #[test]
    fn resolve_display_mode_auto_thresholds() {
        let nav = UiNavigationView::new([]).with_pane_display_mode(NavigationPaneDisplayMode::Auto);
        assert_eq!(nav.resolve_display_mode(1200.0), NavigationDisplayMode::Expanded);
        assert_eq!(nav.resolve_display_mode(800.0), NavigationDisplayMode::Compact);
        assert_eq!(nav.resolve_display_mode(400.0), NavigationDisplayMode::Minimal);
    }

    #[test]
    fn resolve_display_mode_fixed_left() {
        let nav = UiNavigationView::new([]);
        assert_eq!(nav.resolve_display_mode(100.0), NavigationDisplayMode::Expanded);
    }

    #[test]
    fn leaf_index_for_nested_path() {
        let items = vec![NavigationViewItem::new("Cat").with_children([
            NavigationViewItem::new("One"),
            NavigationViewItem::new("Two"),
        ])];
        assert_eq!(leaf_index_for_path(&items, &[0, 1]), Some(1));
        assert_eq!(leaf_index_for_path(&items, &[0]), None);
    }

    #[test]
    fn parent_default_is_collapsed() {
        let parent = NavigationViewItem::new("Cat")
            .with_children([NavigationViewItem::new("Child")]);
        assert!(!parent.is_expanded);
    }
}
