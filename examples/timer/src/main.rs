use std::{
    f64::consts::{FRAC_PI_2, TAU},
    sync::Arc,
    time::Instant,
};
use tokio::time;

use picus::{
    AppPicusExt, PicusPlugin, ProjectionCtx, StyleClass, UiComponentTemplate, UiEventQueue, UiRoot,
    UiThemePicker, UiView, apply_label_style, apply_widget_style,
    bevy_app::{App, PreUpdate, Startup},
    bevy_ecs::prelude::*,
    button, emit_ui_action,
    masonry_core::{
        imaging::{Painter, record::Scene},
        kurbo::{Cap, Circle, Line, Point, Size, Stroke, Vec2},
        layout::Length,
        properties::Padding,
    },
    resolve_style, resolve_style_for_classes, resolve_style_for_entity_classes,
    run_app_with_window_options,
    scene::{CommandsSceneExt, bsn},
    slider,
    xilem::{
        Color,
        core::fork,
        style::Style as _,
        view::{
            CrossAxisAlignment, FlexExt as _, canvas, flex_col, flex_row, label, progress_bar,
            sized_box, task,
        },
        winit::{dpi::LogicalSize, error::EventLoopError},
    },
};
use shared_utils::init_logging;

const DIAL_SIZE: f64 = 188.0;

/// 7GUIs-like Timer.
///
/// - Shows elapsed time
/// - Progress bar (elapsed / duration)
/// - Duration can be adjusted while running
/// - Reset button
#[derive(Resource, Debug, Clone)]
struct TimerState {
    duration_secs: f64,
    elapsed_secs: f64,
    running: bool,
    last_tick: Instant,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            duration_secs: 10.0,
            elapsed_secs: 0.0,
            running: true,
            last_tick: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
enum TimerEvent {
    SetDurationSecs(f64),
    ToggleRunning,
    Reset,
    Tick,
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimerRootView;

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimerTitle;

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimerDialView;

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimerElapsedRow;

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimerProgressRow;

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimerDurationRow;

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimerUiComponentsRow;

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

fn dial_angle(progress: f64) -> f64 {
    (clamp01(progress) * TAU) - FRAC_PI_2
}

fn format_secs(secs: f64) -> String {
    // Keep it readable (one decimal place like many 7GUIs implementations).
    format!("{secs:.1} s")
}

fn apply_timer_event(state: &mut TimerState, event: TimerEvent) {
    match event {
        TimerEvent::SetDurationSecs(new_duration) => {
            state.duration_secs = new_duration.max(0.1);
            state.elapsed_secs = state.elapsed_secs.min(state.duration_secs);
        }
        TimerEvent::ToggleRunning => {
            state.running = !state.running;
            state.last_tick = Instant::now();
        }
        TimerEvent::Reset => {
            state.elapsed_secs = 0.0;
            state.last_tick = Instant::now();
        }
        TimerEvent::Tick => {}
    }
}

fn timer_can_tick(state: &TimerState) -> bool {
    state.running && state.elapsed_secs < state.duration_secs
}

fn tick_timer(state: &mut TimerState) {
    if !timer_can_tick(state) {
        return;
    }

    let now = Instant::now();
    let dt = now.saturating_duration_since(state.last_tick).as_secs_f64();
    state.last_tick = now;
    state.elapsed_secs = (state.elapsed_secs + dt).min(state.duration_secs);
}

fn draw_timer_dial(scene: &mut Scene, size: Size, progress: f64, running: bool) {
    let center = Point::new(size.width / 2.0, size.height / 2.0);
    let radius = (size.width.min(size.height) * 0.5) - 3.0;
    let mut painter = Painter::new(scene);

    let outer = Circle::new(center, radius);
    let inner = Circle::new(center, radius - 12.0);

    painter
        .fill(outer, Color::from_rgb8(0x1B, 0x1B, 0x1D))
        .draw();
    painter
        .fill(inner, Color::from_rgb8(0x28, 0x28, 0x2C))
        .draw();
    painter
        .stroke(outer, &Stroke::new(2.0), Color::from_rgb8(0x70, 0x75, 0x84))
        .draw();

    for tick in 0..60 {
        let major = tick % 5 == 0;
        let angle = ((tick as f64) / 60.0) * TAU - FRAC_PI_2;
        let unit = Vec2::from_angle(angle);
        let outer_pt = center + (radius - 9.0) * unit;
        let inner_pt = center
            + if major {
                (radius - 24.0) * unit
            } else {
                (radius - 17.0) * unit
            };

        painter
            .stroke(
                Line::new(inner_pt, outer_pt),
                &Stroke::new(if major { 2.2 } else { 1.1 }).with_caps(Cap::Round),
                if major {
                    Color::from_rgb8(0xB8, 0xC0, 0xD4)
                } else {
                    Color::from_rgb8(0x6E, 0x75, 0x88)
                },
            )
            .draw();
    }

    let lit_markers = (clamp01(progress) * 60.0).round() as usize;
    for step in 0..lit_markers {
        let angle = ((step as f64) / 60.0) * TAU - FRAC_PI_2;
        let marker_pos = center + (radius - 31.0) * Vec2::from_angle(angle);
        let marker = Circle::new(marker_pos, 1.8);
        painter
            .fill(
                marker,
                if running {
                    Color::from_rgb8(0x79, 0xD7, 0x9C)
                } else {
                    Color::from_rgb8(0xD5, 0xAF, 0x78)
                },
            )
            .draw();
    }

    let hand_angle = dial_angle(progress);
    let hand_end = center + (radius - 35.0) * Vec2::from_angle(hand_angle);
    painter
        .stroke(
            Line::new(center, hand_end),
            &Stroke::new(4.0).with_caps(Cap::Round),
            if running {
                Color::from_rgb8(0x7A, 0xE4, 0xA3)
            } else {
                Color::from_rgb8(0xF0, 0xBF, 0x82)
            },
        )
        .draw();

    painter
        .fill(Circle::new(center, 4.5), Color::from_rgb8(0xF3, 0xF7, 0xFF))
        .draw();
}

impl UiComponentTemplate for TimerRootView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let root_style = resolve_style(ctx.world, ctx.entity);
        let content = apply_widget_style(
            flex_col(
                ctx.children
                    .into_iter()
                    .map(|child| child.into_any_flex())
                    .collect::<Vec<_>>(),
            )
            .cross_axis_alignment(CrossAxisAlignment::Start),
            &root_style,
        );

        let tick_entity = ctx.entity;
        let heartbeat = task(
            |proxy, _: &mut ()| async move {
                let mut interval = time::interval(std::time::Duration::from_millis(50));
                loop {
                    interval.tick().await;
                    let Ok(()) = proxy.message(()) else {
                        break;
                    };
                }
            },
            move |_: &mut (), ()| {
                emit_ui_action(tick_entity, TimerEvent::Tick);
            },
        );

        Arc::new(fork(content, Some(heartbeat)))
    }
}

impl UiComponentTemplate for TimerTitle {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let title_style = resolve_style_for_classes(ctx.world, ["timer.title"]);
        Arc::new(apply_label_style(label("Timer"), &title_style))
    }
}

impl UiComponentTemplate for TimerDialView {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let dial_shell_style = resolve_style_for_classes(ctx.world, ["timer.dial-shell"]);
        let state = ctx.world.resource::<TimerState>().clone();
        let progress = if state.duration_secs > 0.0 {
            Some(clamp01(state.elapsed_secs / state.duration_secs))
        } else {
            Some(1.0)
        };
        let progress_value = progress.unwrap_or(1.0);

        Arc::new(
            sized_box(
                canvas(
                    move |_: &mut (), _ctx: &mut _, scene: &mut Scene, size: Size| {
                        draw_timer_dial(scene, size, progress_value, state.running);
                    },
                )
                .alt_text("Timer dial")
                .padding(Padding::all(Length::px(dial_shell_style.layout.padding)))
                .corner_radius(Length::px(dial_shell_style.layout.corner_radius))
                .border(
                    dial_shell_style.colors.border.unwrap_or(Color::TRANSPARENT),
                    Length::px(dial_shell_style.layout.border_width),
                )
                .background_color(dial_shell_style.colors.bg.unwrap_or(Color::TRANSPARENT)),
            )
            .fixed_width(Length::px(DIAL_SIZE))
            .fixed_height(Length::px(DIAL_SIZE)),
        )
    }
}

impl UiComponentTemplate for TimerElapsedRow {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let row_style = resolve_style_for_classes(ctx.world, ["timer.row"]);
        let body_text_style = resolve_style_for_classes(ctx.world, ["timer.body-text"]);
        let state = ctx.world.resource::<TimerState>();

        Arc::new(apply_widget_style(
            flex_row((
                apply_label_style(label("Elapsed Time:"), &body_text_style),
                apply_label_style(label(format_secs(state.elapsed_secs)), &body_text_style),
            )),
            &row_style,
        ))
    }
}

impl UiComponentTemplate for TimerProgressRow {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let state = ctx.world.resource::<TimerState>();
        let progress = if state.duration_secs > 0.0 {
            Some(clamp01(state.elapsed_secs / state.duration_secs))
        } else {
            Some(1.0)
        };
        Arc::new(progress_bar(progress))
    }
}

impl UiComponentTemplate for TimerDurationRow {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let row_style = resolve_style_for_classes(ctx.world, ["timer.row"]);
        let body_text_style = resolve_style_for_classes(ctx.world, ["timer.body-text"]);
        let state = ctx.world.resource::<TimerState>();
        let duration_value = state.duration_secs;

        Arc::new(apply_widget_style(
            flex_row((
                apply_label_style(
                    label(format!("Duration: {duration_value:.0} s")),
                    &body_text_style,
                ),
                slider(
                    ctx.entity,
                    1.0,
                    60.0,
                    duration_value,
                    TimerEvent::SetDurationSecs,
                )
                .step(1.0)
                .flex(1.0),
            )),
            &row_style,
        ))
    }
}

impl UiComponentTemplate for TimerUiComponentsRow {
    fn project(_: &Self, ctx: ProjectionCtx<'_>) -> UiView {
        let row_style = resolve_style_for_classes(ctx.world, ["timer.row"]);
        let pause_button_style =
            resolve_style_for_entity_classes(ctx.world, ctx.entity, ["timer.pause-button"]);
        let reset_button_style =
            resolve_style_for_entity_classes(ctx.world, ctx.entity, ["timer.reset-button"]);
        let state = ctx.world.resource::<TimerState>();
        let pause_label = if state.running { "Pause" } else { "Resume" };

        Arc::new(apply_widget_style(
            flex_row((
                apply_widget_style(
                    button(ctx.entity, TimerEvent::ToggleRunning, pause_label),
                    &pause_button_style,
                ),
                apply_widget_style(
                    button(ctx.entity, TimerEvent::Reset, "Reset"),
                    &reset_button_style,
                ),
            )),
            &row_style,
        ))
    }
}

fn setup_timer_world(mut commands: Commands) {
    commands.spawn_scene(bsn! {
        UiRoot
        TimerRootView
        StyleClass(vec!["timer.root".to_string()])
        Children [
            UiThemePicker,
            TimerTitle,
            TimerDialView,
            TimerElapsedRow,
            TimerProgressRow,
            TimerDurationRow,
            TimerUiComponentsRow,
        ]
    });
}

fn drain_timer_events_and_tick(world: &mut World) {
    let events = world
        .resource_mut::<UiEventQueue>()
        .drain_actions::<TimerEvent>();

    let has_mutating_event = events
        .iter()
        .any(|event| !matches!(event.action, TimerEvent::Tick));
    let has_tick_event = events
        .iter()
        .any(|event| matches!(event.action, TimerEvent::Tick));
    let should_tick = has_tick_event
        && world
            .get_resource::<TimerState>()
            .is_some_and(timer_can_tick);
    if !has_mutating_event && !should_tick {
        return;
    }

    let mut state = world.resource_mut::<TimerState>();
    for event in events {
        apply_timer_event(&mut state, event.action);
    }
    tick_timer(&mut state);
}

fn build_bevy_timer_app() -> App {
    init_logging();

    let mut app = App::new();
    app.add_plugins(PicusPlugin)
        .load_style_sheet_ron(include_str!("../assets/themes/timer.ron"))
        .insert_resource(TimerState::default())
        .register_projection_resource::<TimerState>()
        .register_ui_component::<TimerRootView>()
        .register_ui_component::<TimerTitle>()
        .register_ui_component::<TimerDialView>()
        .register_ui_component::<TimerElapsedRow>()
        .register_ui_component::<TimerProgressRow>()
        .register_ui_component::<TimerDurationRow>()
        .register_ui_component::<TimerUiComponentsRow>()
        .add_systems(Startup, setup_timer_world);

    app.add_systems(PreUpdate, drain_timer_events_and_tick);

    app
}

fn main() -> Result<(), EventLoopError> {
    run_app_with_window_options(build_bevy_timer_app(), "Timer", |options| {
        options.with_initial_inner_size(LogicalSize::new(520.0, 480.0))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn embedded_timer_theme_ron_parses() {
        let sheet = picus::parse_stylesheet_ron(include_str!("../assets/themes/timer.ron"))
            .expect("embedded timer stylesheet should parse");
        assert_eq!(sheet.default_variant.as_deref(), Some("dark"));
    }

    #[test]
    fn dial_angle_maps_progress_clockwise_from_top() {
        assert!((dial_angle(0.0) + FRAC_PI_2).abs() < 1e-9);
        assert!(dial_angle(0.25).abs() < 1e-9);
        assert!((dial_angle(0.5) - FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn toggle_running_event_flips_state() {
        let mut state = TimerState::default();
        assert!(state.running);

        apply_timer_event(&mut state, TimerEvent::ToggleRunning);
        assert!(!state.running);

        apply_timer_event(&mut state, TimerEvent::ToggleRunning);
        assert!(state.running);
    }

    #[test]
    fn paused_timer_does_not_accumulate_elapsed_time() {
        let mut state = TimerState {
            duration_secs: 10.0,
            elapsed_secs: 3.0,
            running: false,
            last_tick: Instant::now() - Duration::from_secs(2),
        };

        tick_timer(&mut state);

        assert!((state.elapsed_secs - 3.0).abs() < 1e-9);
    }
}
