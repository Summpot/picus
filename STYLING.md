# picus Styling System

This document explains the CSS-like, ECS-driven styling pipeline used by `picus`.

It covers:

- style data model (`Selector`, `StyleSetter`, `StyleRule`, `StyleSheet`, inline style components)
- cascade + pseudo state resolution
- computed style cache + incremental invalidation
- projector integration patterns
- smooth transition animations (Phase 4) powered by `bevy_tween`
- practical usage patterns and common pitfalls

> Applies to the current `master` branch state (2026-02-28).

---

## 1. Goals

The styling system is designed to be:

1. **Data-driven**: styles are ECS components/resources, not ad-hoc widget chains.
2. **Composable**: class styles (`StyleClass` + `StyleSheet`) plus inline overrides.
3. **Interactive**: pseudo states (`InteractionState.hovered`, `InteractionState.pressed`) driven from UI interaction events.
4. **Animated**: smooth color transitions between interaction states via tweening.

---

## 2. Core Types

All style primitives live in `crates/picus/src/styling.rs`.

### 2.1 Inline style components

Inline overrides can be represented either as a single consolidated component (`InlineStyle`) or as legacy split components.

- `InlineStyle` (preferred)
  - `layout: LayoutStyle`
  - `colors: ColorStyle`
  - `text: TextStyle`
  - `transition: Option<StyleTransition>`

Legacy split inline overrides (still supported):

- `LayoutStyle`
  - `padding: Option<f64>`
  - `gap: Option<f64>`
  - `corner_radius: Option<f64>`
  - `border_width: Option<f64>`
- `ColorStyle`
  - base: `bg`, `text`, `border`
  - pseudo overrides: `hover_*`, `pressed_*`
- `TextStyle`
  - `size: Option<f32>`
- `StyleTransition`
  - `duration: f32` (seconds)

### 2.2 Class stylesheet model

- `StyleClass(pub Vec<String>)` (component on entities)
- `Selector::{Type, Class, PseudoClass, And, Descendant}`
- `StyleSetter { layout, colors, text, font_family, transition }`
- `StyleValue::{Value(T), Var(String)}` for token-aware rule values
- `StyleRule { selector, setter }`
- `StyleSheet { tokens: HashMap<String, TokenValue>, rules: Vec<StyleRule> }` (resource)

Convenience APIs still exist for class-only rules:

- `StyleSheet::set_class("name", setter)`
- `StyleSheet::with_class("name", setter)`

### 2.3 Pseudo state

Pseudo classes in selectors (`:hover`, `:pressed`) are backed by a stable ECS component:

- `InteractionState { hovered: bool, pressed: bool }`

### 2.4 Cache + invalidation runtime state

- `StyleDirty` (marks entities requiring recomputation)
- `ComputedStyle` (cached resolved style read by projectors)

`ComputedStyle` now includes `font_family: Option<Vec<String>>` so projectors can apply
font-family from stylesheet resolution without re-running cascade logic each frame.

When descendant selectors are present, invalidation propagates from changed ancestors to
their descendants so `A B`-style rules stay correct after ancestor class/pseudo changes.

### 2.5 Transition runtime state

- `TargetColorStyle` (resolved target colors for the current pseudo state)
- `CurrentColorStyle` (animated colors read by projectors)
- `bevy_tween` timer/interpolator components targeting `CurrentColorStyle`

---

## 3. Cascade and Resolution Order

`resolve_style(world, entity)` follows this precedence (low → high):

1. selector-matched rules from `StyleSheet` (`Type`/`Class`/`PseudoClass`/`And`)
  including descendant relations (`Descendant(ancestor, descendant)`)
2. inline component overrides (`InlineStyle`, or legacy `LayoutStyle`/`ColorStyle`/`TextStyle`/`StyleTransition`)
3. compatibility pseudo color overrides (`hover_*`, `pressed_*`) from `ColorStyle`
4. animated override from `CurrentColorStyle` if present

Font family is resolved through the same class/selector cascade (`StyleSetter.font_family`) as a
font stack (`Vec<String>`) and applied as a discrete value (non-interpolated).

In short: class + inline define intent, pseudo state chooses target, animator provides smooth in-between values.

---

## 4. Plugin Wiring

`PicusPlugin` automatically wires the style stack:

- initializes `StyleSheet`
- registers embedded Fluent variant bundle (`src/theme/fluent_theme.ron`) into `RegisteredStyleVariants`
- sets the bundle default variant as `ActiveStyleVariant`
- `PreUpdate`: `collect_bevy_font_assets -> sync_fonts_to_xilem -> sync_ui_interaction_markers`
- `Update`: `sync_active_style_variant -> mark_style_dirty -> sync_style_targets -> animate_style_transitions`
- registers `DefaultTweenPlugins` (from crates.io `bevy_tween`)

So users only need to define styles and apply them from projectors.

Theme switching uses active-variant state only:

- `set_active_style_variant_by_name(world, "dark" | "light" | "high-contrast")`
- optional `set_active_style_variant_to_registered_default(world)`

There are no public style-theme `install_*` APIs anymore.

---

## 5. Defining Styles

A common pattern is one startup system per screen/example:

- `setup_*_styles` for style declarations
- `setup_*_world` for ECS structure

Example shape:

- `style_sheet.set_class("todo.root", setter)`
- `style_sheet.set_class("todo.add-button", setter)`
- `style_sheet.add_rule(StyleRule::new(Selector::and(...), setter))`

Naming suggestions:

- namespace by feature: `todo.*`, `calc.*`, `chess.*`
- split container/UI-component/text classes:
  - `*.root`
  - `*.button`
  - `*.button.label`

---

## 6. Applying Styles in Projectors

Key helper functions:

- `resolve_style(world, entity)`
- `resolve_style_for_classes(world, ["class.a", "class.b"])`
- `resolve_style_for_entity_classes(world, entity, ["class.a", "class.b"])`
- `apply_widget_style(view, &style)`
- `apply_label_style(label(...), &style)`
- `apply_text_input_style(text_input(...), &style)`

Recommended projector pattern:

1. resolve root/entity style with `resolve_style`
2. resolve shared class styles with `resolve_style_for_classes`
3. compose the widget tree using style helpers

This keeps structure and style concerns separated.

---

## 7. Interaction and Pseudo States

Interaction events are emitted by ECS-backed UI components (notably the custom ECS button widget path):

- `PointerEntered`
- `PointerLeft`
- `PointerPressed`
- `PointerReleased`

`sync_ui_interaction_markers` consumes these events and updates `InteractionState`.
This is intentionally done by mutating a stable component in-place (rather than inserting/removing marker components)
to avoid frequent archetype changes.

That state then affects color target selection during style resolution.

---

## 8. Phase 4: Smooth Tween-Based Transitions

Phase 4 replaces manual per-frame color lerp logic with a tweening animator pipeline.

### 8.1 `bevy_tween` integration

`picus` now uses crates.io `bevy_tween` (`0.12`) for transitions:

- `DefaultTweenPlugins`
- `EaseKind`
- `TimeRunner` + `TimeSpan`
- `ComponentTween<T>`
- `Interpolator`

### 8.2 Custom interpolator: `ColorStyleLens`

`ColorStyleLens` implements:

- `Interpolator<Item = CurrentColorStyle>`

It linearly interpolates each RGBA channel for:

- background color
- text color
- border color

while easing is applied by tween sampling (`QuadraticInOut` by default for interaction transitions).

For full computed-style tweening, `ComputedStyleLens` is also available. It deliberately treats
`font_family` as **non-interpolable** and only switches on tween completion.

### 8.3 State-change behavior

When target style changes (for example, due to hover/press changes):

- `mark_style_dirty` marks entities with changed style dependencies.
- `sync_style_targets` recomputes dirty entities, updates `ComputedStyle`, and computes a new `TargetColorStyle`.
- If a transition is configured and target changed:
  - insert/update `TimeRunner` + `TimeSpan` + `ComponentTween<ColorStyleLens>` targeting `CurrentColorStyle`
  - tween starts from current animated value and ends at new target value
- `DefaultTweenPlugins` sample easing and apply component tweens each frame in the configured tween system sets.
- Projectors read `ComputedStyle` (+ animated `CurrentColorStyle`) via `resolve_style`.

Result: no color snap; smooth CSS-like interpolation.

### 8.4 Duration recommendations

For UI interaction micro-animations:

- common range: `0.10`–`0.18` seconds
- default used in examples: around `0.15` seconds

---

## 9. Practical Example Checklist

To make a UI component animate on interaction:

1. define base and hover/pressed colors in `ColorStyle`
2. set `transition: Some(StyleTransition { duration: ... })`
3. ensure UI component path emits interaction events (updating `InteractionState`) so entities become `StyleDirty`
4. apply style with projector helpers

---

## 10. Common Pitfalls

- **Class-only resolution is static**
  - `resolve_style_for_classes(...)` does not bind pseudo state by itself.
  - Use `resolve_style_for_entity_classes(...)` when pseudo-state-dependent classes are needed.
- **Interaction event source matters**
  - if a UI component path does not emit `UiInteractionEvent`, pseudo-state-based transitions will not trigger.
- **Wrapper styling vs. inner widget styling**
  - some UI components may have internal defaults (such as borders) that require styling the interactive path itself.
- **Keep design/docs in sync**
  - if style behavior changes, update both implementation and `DESIGN.md`/docs in one change.

---

## 11. Reference Files

- Styling core: `crates/picus/src/styling.rs`
- Plugin wiring: `crates/picus/src/plugin.rs`
- ECS button interaction source: `crates/picus/src/widgets/ecs_button_widget.rs`
- ECS button view path: `crates/picus/src/views/ecs_button_view.rs`
- Architecture doc: `DESIGN.md`

---

If you plan to extend this system (for example `:disabled`, inherited style contexts, or layout tweening), extend `StyleRule` first, then wire `resolve + sync + animation`, and finally update docs/examples together.
