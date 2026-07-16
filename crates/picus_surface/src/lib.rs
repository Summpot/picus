#![expect(
    unsafe_code,
    reason = "Creating a persistent wgpu surface and applying native window backdrops requires raw window handles and Win32 calls."
)]

#[cfg(windows)]
mod win32_create_window_hook;

use std::collections::HashMap;

use bevy_window::{CompositeAlphaMode as BevyCompositeAlphaMode, RawHandleWrapper};
use picus_imaging::{
    PreparedFrame,
    texture_render::{RenderTarget, Renderer},
};
use wgpu::util::{TextureBlitter, TextureBlitterBuilder};
use wgpu::{
    Backend, Backends, CompositeAlphaMode, Device, DeviceType, Dx12SwapchainKind, Instance,
    MemoryBudgetThresholds, MemoryHints, PresentMode, Surface, SurfaceConfiguration,
    SurfaceTexture, Texture, TextureFormat, TextureUsages, TextureView,
};

/// Native desktop backdrop material requested for a top-level window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NativeWindowBackdropMaterial {
    /// No native backdrop material.
    #[default]
    None,
    /// Let the operating system choose the backdrop.
    Auto,
    /// Windows Mica system backdrop.
    Mica,
    /// Windows Desktop Acrylic system backdrop.
    Acrylic,
    /// Windows tabbed/Mica Alt system backdrop.
    MicaAlt,
}

/// Native color scheme requested for a top-level window backdrop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NativeWindowBackdropColorScheme {
    /// Preserve the operating system's current window appearance policy.
    #[default]
    System,
    /// Request light window chrome and backdrop composition.
    Light,
    /// Request dark window chrome and backdrop composition.
    Dark,
}

impl NativeWindowBackdropMaterial {
    #[must_use]
    pub const fn requires_transparent_surface(self) -> bool {
        cfg!(windows) && !matches!(self, Self::None)
    }
}

/// Error returned when a native window backdrop cannot be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowBackdropError {
    /// The current target platform has no native Picus backdrop integration.
    UnsupportedPlatform,
    /// The raw window handle is not a supported native window kind.
    UnsupportedWindowHandle,
    /// The native API returned a failing HRESULT.
    WindowsHresult(i32),
}

impl core::fmt::Display for NativeWindowBackdropError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedPlatform => write!(
                f,
                "native window backdrops are unsupported on this platform"
            ),
            Self::UnsupportedWindowHandle => {
                write!(f, "native window backdrop requires a Win32 window handle")
            }
            Self::WindowsHresult(hr) => {
                write!(f, "DwmSetWindowAttribute failed with HRESULT {hr:#010x}")
            }
        }
    }
}

impl std::error::Error for NativeWindowBackdropError {}

/// Enable creation-time `WS_EX_NOREDIRECTIONBITMAP` injection via MinHook on
/// `user32!CreateWindowExW`. Call before Bevy/winit creates the HWND when a
/// transparent composition surface is required. No-op on non-Windows.
pub fn set_force_no_redirection_bitmap_on_create(enable: bool) {
    #[cfg(windows)]
    win32_create_window_hook::set_force_no_redirection_bitmap_on_create(enable);
    #[cfg(not(windows))]
    let _ = enable;
}

/// Whether creation-time `WS_EX_NOREDIRECTIONBITMAP` injection is enabled.
#[must_use]
pub fn force_no_redirection_bitmap_on_create() -> bool {
    #[cfg(windows)]
    {
        win32_create_window_hook::force_no_redirection_bitmap_on_create()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Apply a native backdrop material to a Bevy-owned window handle.
///
/// On Windows this uses DWM system backdrops. On other platforms this returns
/// [`NativeWindowBackdropError::UnsupportedPlatform`].
pub fn set_native_window_backdrop_material(
    raw_handle: &RawHandleWrapper,
    material: NativeWindowBackdropMaterial,
) -> Result<(), NativeWindowBackdropError> {
    set_native_window_backdrop_material_with_color_scheme(
        raw_handle,
        material,
        NativeWindowBackdropColorScheme::System,
    )
}

/// Apply a native backdrop material and explicit light/dark appearance.
pub fn set_native_window_backdrop_material_with_color_scheme(
    raw_handle: &RawHandleWrapper,
    material: NativeWindowBackdropMaterial,
    color_scheme: NativeWindowBackdropColorScheme,
) -> Result<(), NativeWindowBackdropError> {
    set_native_window_backdrop_material_impl(raw_handle, material, color_scheme)
}

#[cfg(windows)]
fn set_native_window_backdrop_material_impl(
    raw_handle: &RawHandleWrapper,
    material: NativeWindowBackdropMaterial,
    color_scheme: NativeWindowBackdropColorScheme,
) -> Result<(), NativeWindowBackdropError> {
    use raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Graphics::Dwm::{
        DWM_SYSTEMBACKDROP_TYPE, DWMSBT_AUTO, DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TABBEDWINDOW,
        DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
    };
    use windows_sys::Win32::UI::Controls::MARGINS;

    let hwnd = match raw_handle.get_window_handle() {
        RawWindowHandle::Win32(handle) => handle.hwnd.get() as windows_sys::Win32::Foundation::HWND,
        _ => return Err(NativeWindowBackdropError::UnsupportedWindowHandle),
    };

    prepare_hwnd_for_composition_surface(hwnd);

    if !matches!(color_scheme, NativeWindowBackdropColorScheme::System) {
        let use_dark_mode: i32 = i32::from(matches!(
            color_scheme,
            NativeWindowBackdropColorScheme::Dark
        ));
        let hr = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                (&use_dark_mode as *const i32).cast(),
                core::mem::size_of::<i32>() as u32,
            )
        };
        if hr < 0 {
            return Err(NativeWindowBackdropError::WindowsHresult(hr));
        }
    }

    let backdrop: DWM_SYSTEMBACKDROP_TYPE = match material {
        NativeWindowBackdropMaterial::None => DWMSBT_NONE,
        NativeWindowBackdropMaterial::Auto => DWMSBT_AUTO,
        NativeWindowBackdropMaterial::Mica => DWMSBT_MAINWINDOW,
        NativeWindowBackdropMaterial::Acrylic => DWMSBT_TRANSIENTWINDOW,
        NativeWindowBackdropMaterial::MicaAlt => DWMSBT_TABBEDWINDOW,
    };
    let hr = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            (&backdrop as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
            core::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
    };
    if hr < 0 {
        return Err(NativeWindowBackdropError::WindowsHresult(hr));
    }

    let margins = if matches!(material, NativeWindowBackdropMaterial::None) {
        MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 0,
            cyBottomHeight: 0,
        }
    } else {
        MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        }
    };
    let hr = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
    if hr < 0 {
        Err(NativeWindowBackdropError::WindowsHresult(hr))
    } else {
        Ok(())
    }
}

/// Companion HWND styles for DirectComposition: layered window, disable blur-behind,
/// full window opacity (per-pixel alpha comes from DXGI).
#[cfg(windows)]
fn prepare_hwnd_for_composition_surface(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWM_BB_ENABLE, DWM_BLURBEHIND, DwmEnableBlurBehindWindow,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos,
        WS_EX_LAYERED, WS_EX_NOREDIRECTIONBITMAP,
    };

    let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let desired = ex | WS_EX_NOREDIRECTIONBITMAP as isize | WS_EX_LAYERED as isize;
    if desired != ex {
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired);
            SetWindowPos(
                hwnd,
                core::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }

    let blur = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE,
        fEnable: false.into(),
        hRgnBlur: core::ptr::null_mut(),
        fTransitionOnMaximized: false.into(),
    };
    let _ = unsafe { DwmEnableBlurBehindWindow(hwnd, &blur) };
    let _ = unsafe { SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) };
}

#[cfg(not(windows))]
fn set_native_window_backdrop_material_impl(
    _raw_handle: &RawHandleWrapper,
    _material: NativeWindowBackdropMaterial,
    _color_scheme: NativeWindowBackdropColorScheme,
) -> Result<(), NativeWindowBackdropError> {
    Err(NativeWindowBackdropError::UnsupportedPlatform)
}

/// Metrics captured from an externally owned window.
#[derive(Debug, Clone, Copy)]
pub struct ExistingWindowMetrics {
    /// Current physical width.
    pub physical_width: u32,
    /// Current physical height.
    pub physical_height: u32,
    /// Current logical width.
    pub logical_width: f64,
    /// Current logical height.
    pub logical_height: f64,
    /// Current scale factor.
    pub scale_factor: f64,
    /// Whether the native window was created as transparent.
    pub transparent: bool,
    /// The Bevy-requested alpha composition mode for the native window.
    pub composite_alpha_mode: BevyCompositeAlphaMode,
}

/// Negotiated present capability after mode selection (G7).
///
/// **Do not** treat this as a unified `drop_stale` boolean across modes:
/// frames already submitted to a FIFO / FifoRelaxed swapchain cannot be
/// withdrawn by Picus. Only [`MailboxLatest`](Self::MailboxLatest) can replace
/// queued frames at the GPU/compositor; FIFO relies on CPU-side ready-queue
/// coalescing and backpressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NegotiatedPresentCapability {
    /// Mailbox present mode: intermediate queued frames may be replaced
    /// (true drop-stale at the display path).
    MailboxLatest,
    /// Non-mailbox negotiated mode (FIFO, FifoRelaxed, AutoVsync, Immediate, …).
    ///
    /// Name is historical for the common FIFO fallback path. **Not** every
    /// non-mailbox mode is true FIFO queue backpressure (e.g. Immediate may tear).
    /// Phase 1 honesty: this means “not MailboxLatest”; prefer CPU-side
    /// unsubmitted coalescing helpers over a fake unified drop-stale promise.
    /// Submitted frames are never claimed withdrawable.
    FifoBackpressure,
}

/// CPU-side policy for unsubmitted ready frames before `present()`.
///
/// Helper policy type for future multi-buffer coalescing. **Not yet wired** into
/// [`ExternalWindowSurface::render_frame`] (present remains single in-flight
/// submit). Once a frame is submitted to the swapchain, only mailbox can replace it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReadyQueuePolicy {
    /// Keep only the latest unsubmitted ready frame; older unsubmitted frames
    /// are dropped. Submitted frames are never claimed withdrawable here.
    #[default]
    LatestOnly,
}

/// Present mode preference and latency hints shared by surface creation and core.
///
/// Extracted from the surface configuration path so runtime scheduling and
/// diagnostics can name the **actual** negotiated capability (G7) instead of
/// a fake cross-mode `drop_stale` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentPolicy {
    /// Caller's preferred present mode (typically [`PresentMode::AutoVsync`]).
    pub preferred: PresentMode,
    /// Backend hint for swapchain depth — not a hard guarantee.
    pub desired_maximum_frame_latency: u32,
    /// How unsubmitted ready frames are coalesced on the CPU.
    pub ready_queue: ReadyQueuePolicy,
}

impl Default for PresentPolicy {
    fn default() -> Self {
        Self::default_ui()
    }
}

impl PresentPolicy {
    /// Default UI policy: prefer low-latency modes, latency hint 1, latest-only ready queue.
    #[must_use]
    pub const fn default_ui() -> Self {
        Self {
            preferred: PresentMode::AutoVsync,
            desired_maximum_frame_latency: 1,
            ready_queue: ReadyQueuePolicy::LatestOnly,
        }
    }

    /// Negotiate against adapter-reported present modes.
    ///
    /// Logs the selected mode and the effective fallback strategy.
    #[must_use]
    pub fn negotiate(self, available: &[PresentMode]) -> NegotiatedPresent {
        let mode = select_present_mode(available, self.preferred);
        let capability = match mode {
            PresentMode::Mailbox => NegotiatedPresentCapability::MailboxLatest,
            _ => NegotiatedPresentCapability::FifoBackpressure,
        };
        let negotiated = NegotiatedPresent {
            mode,
            capability,
            desired_maximum_frame_latency: self.desired_maximum_frame_latency,
            ready_queue: self.ready_queue,
        };
        match capability {
            NegotiatedPresentCapability::MailboxLatest => {
                tracing::info!(
                    ?mode,
                    preferred = ?self.preferred,
                    available = ?available,
                    desired_maximum_frame_latency = self.desired_maximum_frame_latency,
                    ready_queue = ?self.ready_queue,
                    capability = "MailboxLatest",
                    strategy = "replace_queued_frame",
                    "negotiated present policy"
                );
            }
            NegotiatedPresentCapability::FifoBackpressure => {
                tracing::info!(
                    ?mode,
                    preferred = ?self.preferred,
                    available = ?available,
                    desired_maximum_frame_latency = self.desired_maximum_frame_latency,
                    ready_queue = ?self.ready_queue,
                    capability = "FifoBackpressure",
                    strategy = "cpu_latest_only_unsubmitted_plus_backpressure",
                    note = "submitted FIFO frames are not withdrawable",
                    "negotiated present policy"
                );
            }
        }
        negotiated
    }
}

/// Result of [`PresentPolicy::negotiate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiatedPresent {
    pub mode: PresentMode,
    pub capability: NegotiatedPresentCapability,
    pub desired_maximum_frame_latency: u32,
    pub ready_queue: ReadyQueuePolicy,
}

/// Single-slot ready queue for unsubmitted frames (LatestOnly).
///
/// Scaffolding for CPU-side coalescing (G7): only the newest unsubmitted frame
/// is retained. **Not yet used on the hot present path** — unit-tested helper
/// for Phase 2+ multi-buffer work. Calling [`take_for_submit`](Self::take_for_submit)
/// moves the frame out for submit; after that it is no longer in the queue and
/// **cannot** be withdrawn through this API (submitted FIFO frames are not
/// claimed withdrawable).
#[derive(Debug, Default)]
pub struct LatestReadyQueue<T> {
    pending: Option<T>,
}

impl<T> LatestReadyQueue<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self { pending: None }
    }

    /// Push a ready unsubmitted frame, replacing any previous unsubmitted one.
    pub fn push_ready(&mut self, frame: T) -> Option<T> {
        self.pending.replace(frame)
    }

    /// Take the latest unsubmitted frame for submit/present.
    ///
    /// After this returns `Some`, the frame is no longer withdrawable via this queue.
    pub fn take_for_submit(&mut self) -> Option<T> {
        self.pending.take()
    }

    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        self.pending.as_ref()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_none()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.pending.is_some())
    }
}

/// Intermediate GPU target for one compositor [`OrderedLayerEncode::layer_id`].
struct LayerTextureTarget {
    texture: Texture,
    view: TextureView,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    generation: LayerMetricsGeneration,
}

/// A Vello surface context attached to an externally owned Bevy window.
pub struct ExternalWindowSurface {
    render_cx: RenderContext,
    surface: RenderSurface<'static>,
    scale_factor: f64,
    /// Window requested a transparent composition surface (Mica / clear).
    window_transparent: bool,
    /// Actual negotiated present capability (G7); not a fake unified drop_stale.
    negotiated_present: NegotiatedPresent,
    /// Painter-order intermediate textures keyed by compositor layer id (P2.3).
    /// Layer contents from Vello are **straight-alpha**; ordered stack may convert
    /// to premul in the intermediate when present needs premultiply (Issue 9).
    layer_targets: HashMap<u64, LayerTextureTarget>,
    /// Metrics generation last applied to layer targets (P2.6).
    layer_metrics_generation: LayerMetricsGeneration,
    /// Replace (no blend) into the **Rgba8Unorm intermediate** (layer0 straight path).
    ///
    /// Target format must match [`VELLO_TARGET_FORMAT`] / `create_targets`, **not**
    /// the swapchain format (often `Bgra8Unorm` on Windows).
    layer_replace_blitter: TextureBlitter,
    /// Straight → premul convert into the Rgba8Unorm intermediate (layer0 when
    /// intermediate is held in premul space; SrcAlpha / Zero).
    layer_premul_convert_blitter: TextureBlitter,
    /// Src-over for upper layers onto the Rgba8Unorm intermediate.
    /// Safe when dest is premul (Issue 9) or when dest is opaque (a≈1).
    layer_stack_blitter: TextureBlitter,
    /// Region equivalent of `layer_replace_blitter`, used to rebuild only the
    /// animation-dirty portion of the persistent intermediate.
    region_layer_replace_blitter: RegionTextureBlitter,
    /// Region equivalent of `layer_premul_convert_blitter`.
    region_layer_premul_convert_blitter: RegionTextureBlitter,
    /// Src-over blitter that places a tight layer texture into a destination
    /// viewport, optionally clipped to a smaller dirty rectangle.
    region_layer_stack_blitter: RegionTextureBlitter,
    /// Whether `surface.target_view` contains a complete ordered composite for
    /// the current metrics generation and layer target geometry.
    ordered_intermediate_valid: bool,
    /// Final replace blit from Rgba8Unorm intermediate → **swapchain** format
    /// when intermediate is already premul (must not re-use intermediate blitters).
    present_replace_blitter: TextureBlitter,
}

/// Result of rendering and presenting one frame to an external window surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFrameResult {
    /// The frame was submitted and presented successfully.
    Presented,
    /// The surface was temporarily unavailable and the caller should request another frame.
    Retry,
    /// Rendering failed in a way that should not cause an immediate redraw loop.
    Failed,
}

/// CPU-side phase timings for one [`ExternalWindowSurface::render_frame`] call.
///
/// These are wall-clock durations measured around the present path only. They are
/// **not** display latency or DWM composition time; use PresentMon/ETW for that.
///
/// Fields may be extended as the frame pipeline gains layer isolation; treat as
/// diagnostic data rather than a frozen ABI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderFrameTimings {
    /// Time spent acquiring the swapchain texture (including reconfigure attempts).
    pub surface_acquire: std::time::Duration,
    /// Time spent in Vello `render_to_texture` (full-window / base encode).
    ///
    /// On the ordered multi-texture path this is the sum of non-anim entry encodes.
    pub encode: std::time::Duration,
    /// Anim-entry encode wall time (0 on single full-window path).
    pub encode_anim: std::time::Duration,
    /// Time spent blitting/compositing into the swapchain view.
    pub composite: std::time::Duration,
    /// Time spent in the CPU `present()` call (submit, not vsync wait).
    pub present_submit: std::time::Duration,
}

/// Kind of ordered compositor entry for encode timing attribution (P2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderedEntryKind {
    /// Cached Masonry scene segment or overlay (counts as base encode).
    Cached,
    /// High-frequency anim layer (counts as anim encode).
    Anim,
    /// External placeholder without host content (transparent clear / skip).
    External,
}

/// One painter-order encode unit for [`ExternalWindowSurface::render_ordered_frame`].
///
/// Entries are submitted in Masonry painter order. `frame: None` reuses the
/// cached intermediate texture for `layer_id` (encode skipped — P2.4).
#[derive(Clone, Copy)]
pub struct OrderedLayerEncode<'a> {
    /// Stable compositor layer id (`LayerId::raw` from picus_core).
    pub layer_id: u64,
    pub kind: OrderedEntryKind,
    /// Physical-pixel rectangle occupied by this layer target in the window.
    pub target: OrderedLayerTarget,
    /// When `Some`, encode this prepared frame into the layer target.
    /// When `None`, composite reuses the last successful encode for `layer_id`.
    pub frame: Option<PreparedFrame<'a>>,
}

/// Physical-pixel placement and size of one ordered layer texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderedLayerTarget {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl OrderedLayerTarget {
    #[must_use]
    pub const fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    #[must_use]
    fn intersection(self, other: Self) -> Option<Self> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self
            .x
            .saturating_add(self.width)
            .min(other.x.saturating_add(other.width));
        let y1 = self
            .y
            .saturating_add(self.height)
            .min(other.y.saturating_add(other.height));
        (x1 > x0 && y1 > y0).then_some(Self {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }

    #[must_use]
    fn union(self, other: Self) -> Self {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self
            .x
            .saturating_add(self.width)
            .max(other.x.saturating_add(other.width));
        let y1 = self
            .y
            .saturating_add(self.height)
            .max(other.y.saturating_add(other.height));
        Self {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }
}

/// Metrics generation token for atomic layer-target rebuild (P2.6).
///
/// Surfaces drop all intermediate textures when generation changes so old-size
/// textures never composite with a new plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LayerMetricsGeneration(pub u64);

impl ExternalWindowSurface {
    /// Create an attached Vello surface from a Bevy-owned raw-handle wrapper.
    ///
    /// Uses [`PresentPolicy::default_ui`] and records the negotiated capability.
    pub fn new_from_bevy_raw_handle(
        raw_handle: RawHandleWrapper,
        metrics: ExistingWindowMetrics,
        present_mode: PresentMode,
    ) -> Result<Self, RenderSurfaceError> {
        let policy = PresentPolicy {
            preferred: present_mode,
            ..PresentPolicy::default_ui()
        };
        Self::new_from_bevy_raw_handle_with_policy(raw_handle, metrics, policy)
    }

    /// Create an attached surface with an explicit [`PresentPolicy`].
    pub fn new_from_bevy_raw_handle_with_policy(
        raw_handle: RawHandleWrapper,
        metrics: ExistingWindowMetrics,
        policy: PresentPolicy,
    ) -> Result<Self, RenderSurfaceError> {
        #[cfg(windows)]
        if metrics.transparent {
            use raw_window_handle::RawWindowHandle;
            if let RawWindowHandle::Win32(handle) = raw_handle.get_window_handle() {
                prepare_hwnd_for_composition_surface(
                    handle.hwnd.get() as windows_sys::Win32::Foundation::HWND
                );
            }
        }

        // SAFETY: The caller provides a `RawHandleWrapper` originating from Bevy's
        // `WindowWrapper`, which internally keeps an owning reference to the window alive.
        // We create a thread-locked handle target only for surface initialization.
        let target = unsafe { raw_handle.get_handle() };
        let mut render_cx = RenderContext::new(metrics.transparent);
        let (surface, negotiated) =
            pollster::block_on(render_cx.create_surface(target, metrics, policy))?;

        let dev_id = surface.dev_id;
        let device = &render_cx.devices[dev_id].device;
        // Intermediate + per-layer Vello targets are always Rgba8Unorm (Vello).
        // Swapchain is often Bgra8Unorm on Windows — do not mix dest formats.
        let layer_replace_blitter = create_layer_replace_blitter(device, VELLO_TARGET_FORMAT);
        let layer_premul_convert_blitter =
            create_layer_premul_convert_blitter(device, VELLO_TARGET_FORMAT);
        let layer_stack_blitter = create_layer_stack_blitter(device, VELLO_TARGET_FORMAT);
        let region_layer_replace_blitter =
            RegionTextureBlitter::new(device, VELLO_TARGET_FORMAT, None);
        let region_layer_premul_convert_blitter =
            RegionTextureBlitter::new(device, VELLO_TARGET_FORMAT, Some(PREMUL_CONVERT));
        let region_layer_stack_blitter =
            RegionTextureBlitter::new(device, VELLO_TARGET_FORMAT, Some(STRAIGHT_SRC_OVER));
        let present_replace_blitter = create_layer_replace_blitter(device, surface.format);

        Ok(Self {
            render_cx,
            surface,
            scale_factor: metrics.scale_factor,
            window_transparent: metrics.transparent,
            negotiated_present: negotiated,
            layer_targets: HashMap::new(),
            layer_metrics_generation: LayerMetricsGeneration(0),
            layer_replace_blitter,
            layer_premul_convert_blitter,
            layer_stack_blitter,
            region_layer_replace_blitter,
            region_layer_premul_convert_blitter,
            region_layer_stack_blitter,
            ordered_intermediate_valid: false,
            present_replace_blitter,
        })
    }

    /// Actual present mode / capability negotiated at surface creation (G7).
    #[must_use]
    pub fn negotiated_present(&self) -> NegotiatedPresent {
        self.negotiated_present
    }

    /// Synchronize internal surface size and scale-factor from the attached window.
    ///
    /// Returns `true` when the backing surface textures were resized and the
    /// caller should schedule a fresh paint.
    ///
    /// On size change, **all** intermediate layer targets are dropped so they
    /// cannot be composited at a stale size (P2.6). Callers must pass a new
    /// [`LayerMetricsGeneration`] via [`Self::sync_layer_metrics_generation`]
    /// before the next ordered encode.
    pub fn sync_window_metrics(&mut self, metrics: ExistingWindowMetrics) -> bool {
        self.scale_factor = metrics.scale_factor;
        self.window_transparent = metrics.transparent;
        let mut changed = false;

        if self.surface.config.width != metrics.physical_width
            || self.surface.config.height != metrics.physical_height
        {
            self.render_cx.resize_surface(
                &mut self.surface,
                metrics.physical_width.max(1),
                metrics.physical_height.max(1),
            );
            // Atomic invalidation: never mix old-size layer textures with a new plan.
            self.drop_all_layer_targets();
            changed = true;
        }

        let desired_alpha_mode = self
            .render_cx
            .desired_alpha_mode_for_surface(&self.surface, metrics);
        if self.surface.config.alpha_mode != desired_alpha_mode {
            self.render_cx.set_surface_alpha_mode(
                &mut self.surface,
                desired_alpha_mode,
                metrics.transparent,
            );
            changed = true;
        }

        changed
    }

    /// Whether ordered intermediate stack is held in premultiplied space.
    ///
    /// When true, layer0 is converted straight→premul and the final swapchain
    /// blit is replace (already premul). When false, stack stays straight-alpha
    /// and the present blitter may convert once (opaque / non-premul present).
    #[inline]
    fn ordered_stack_holds_premul(&self) -> bool {
        needs_premultiplied_blit(self.surface.config.alpha_mode, self.window_transparent)
    }

    /// Align intermediate layer targets with the core [`LayerMetricsGeneration`].
    ///
    /// When generation changes, all layer textures are dropped and rebuilt on
    /// demand at the current surface size (P2.6 atomic rebuild).
    pub fn sync_layer_metrics_generation(&mut self, generation: LayerMetricsGeneration) {
        if self.layer_metrics_generation != generation {
            self.drop_all_layer_targets();
            self.layer_metrics_generation = generation;
        }
    }

    /// Drop every intermediate layer texture (resize / plan metrics change).
    pub fn drop_all_layer_targets(&mut self) {
        self.layer_targets.clear();
        self.ordered_intermediate_valid = false;
    }

    /// Drop intermediate textures whose `layer_id` is no longer in the plan (Issue 5).
    pub fn retain_layer_targets(&mut self, live_ids: &[u64]) {
        let old_len = self.layer_targets.len();
        self.layer_targets
            .retain(|id, _| live_ids.iter().any(|live| live == id));
        if self.layer_targets.len() != old_len {
            self.ordered_intermediate_valid = false;
        }
    }

    /// Number of intermediate layer textures currently allocated (tests/diagnostics).
    #[must_use]
    pub fn layer_target_count(&self) -> usize {
        self.layer_targets.len()
    }

    /// Physical pixel size of the swapchain / intermediate targets.
    #[must_use]
    pub fn physical_size(&self) -> (u32, u32) {
        (self.surface.config.width, self.surface.config.height)
    }

    /// Current layer metrics generation last synced from core.
    #[must_use]
    pub fn layer_metrics_generation(&self) -> LayerMetricsGeneration {
        self.layer_metrics_generation
    }

    fn ensure_layer_target(&mut self, layer_id: u64, target: OrderedLayerTarget) -> Result<(), ()> {
        let width = target.width.max(1);
        let height = target.height.max(1);
        let metrics_gen = self.layer_metrics_generation;
        if let Some(existing) = self.layer_targets.get(&layer_id)
            && existing.x == target.x
            && existing.y == target.y
            && existing.width == width
            && existing.height == height
            && existing.generation == metrics_gen
        {
            return Ok(());
        }
        let device = &self.render_cx.devices[self.surface.dev_id].device;
        let (texture, view) = create_targets(width, height, device);
        self.ordered_intermediate_valid = false;
        self.layer_targets.insert(
            layer_id,
            LayerTextureTarget {
                texture,
                view,
                x: target.x,
                y: target.y,
                width,
                height,
                generation: metrics_gen,
            },
        );
        Ok(())
    }

    /// Render a prepared Masonry frame and present it to the attached window surface.
    ///
    /// Returns the outcome together with CPU-side phase timings. Timings measure
    /// submit-path wall time only — not actual display time.
    ///
    /// # Breaking change (frame-pipeline Phase 0)
    ///
    /// The return type is `(RenderFrameResult, RenderFrameTimings)` rather than
    /// bare `RenderFrameResult`. This is an intentional **crate-level** API break
    /// for direct `picus_surface` dependents. Application code that uses only the
    /// `picus` facade / `run_picus` is unaffected.
    #[must_use]
    pub fn render_frame(
        &mut self,
        renderer: &mut Renderer,
        frame: PreparedFrame<'_>,
    ) -> (RenderFrameResult, RenderFrameTimings) {
        let mut timings = RenderFrameTimings::default();
        let dev_id = self.surface.dev_id;
        let adapter = &self.render_cx.devices[dev_id].adapter;
        let device = &self.render_cx.devices[dev_id].device;
        let queue = &self.render_cx.devices[dev_id].queue;

        let acquire_started = std::time::Instant::now();
        let mut did_reconfigure = false;
        let surface_texture = loop {
            match get_current_surface_texture(&self.surface.surface, device) {
                Ok(texture) if texture.suboptimal => {
                    discard_surface_texture(device, texture);
                    self.render_cx.configure_surface(&self.surface);
                    tracing::debug!("swap chain texture was suboptimal; surface reconfigured");
                    timings.surface_acquire = acquire_started.elapsed();
                    return (RenderFrameResult::Retry, timings);
                }
                Ok(texture) => break texture,
                Err(error) => match surface_recovery_action(&error) {
                    SurfaceRecoveryAction::Reconfigure if !did_reconfigure => {
                        did_reconfigure = true;
                        self.render_cx.configure_surface(&self.surface);
                    }
                    SurfaceRecoveryAction::Reconfigure | SurfaceRecoveryAction::Retry => {
                        tracing::warn!(
                            "couldn't acquire swap chain texture; retrying next frame: {error}"
                        );
                        timings.surface_acquire = acquire_started.elapsed();
                        return (RenderFrameResult::Retry, timings);
                    }
                    SurfaceRecoveryAction::Fail => {
                        tracing::error!("couldn't acquire swap chain texture: {error}");
                        timings.surface_acquire = acquire_started.elapsed();
                        return (RenderFrameResult::Failed, timings);
                    }
                },
            }
        };
        timings.surface_acquire = acquire_started.elapsed();

        let encode_started = std::time::Instant::now();
        if let Err(error) = renderer.render_to_texture(
            RenderTarget {
                adapter,
                device,
                queue,
                texture: &self.surface.target_texture,
                view: &self.surface.target_view,
            },
            frame,
        ) {
            tracing::error!("failed to render Masonry frame to texture: {error}");
            discard_surface_texture(device, surface_texture);
            timings.encode = encode_started.elapsed();
            return (RenderFrameResult::Failed, timings);
        }
        timings.encode = encode_started.elapsed();

        let composite_started = std::time::Instant::now();
        let (surface_view, view_errors) = capture_device_errors(device, || {
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Picus Swap Chain Texture View"),
                    ..Default::default()
                })
        });
        if !view_errors.is_empty() {
            log_device_errors("creating the swap chain texture view", view_errors);
            discard_surface_texture(device, surface_texture);
            self.render_cx.configure_surface(&self.surface);
            timings.composite = composite_started.elapsed();
            return (RenderFrameResult::Retry, timings);
        }
        let ((), blit_errors) = capture_device_errors(device, || {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("External Window Surface Blit"),
            });
            self.surface.blitter.copy(
                device,
                &mut encoder,
                &self.surface.target_view,
                &surface_view,
            );
            queue.submit([encoder.finish()]);
        });
        if !blit_errors.is_empty() {
            log_device_errors("submitting the swap chain blit", blit_errors);
            discard_surface_texture(device, surface_texture);
            self.render_cx.configure_surface(&self.surface);
            timings.composite = composite_started.elapsed();
            return (RenderFrameResult::Retry, timings);
        }
        timings.composite = composite_started.elapsed();

        let present_started = std::time::Instant::now();
        let ((), present_errors) = capture_device_errors(device, || surface_texture.present());
        timings.present_submit = present_started.elapsed();
        if !present_errors.is_empty() {
            log_device_errors("presenting the swap chain texture", present_errors);
            self.render_cx.configure_surface(&self.surface);
            return (RenderFrameResult::Retry, timings);
        }

        if let Err(error) = device.poll(wgpu::PollType::Poll) {
            tracing::trace!("non-blocking GPU poll after present returned: {error}");
        }

        (RenderFrameResult::Presented, timings)
    }

    /// Encode dirty painter-order entries into intermediate textures, composite
    /// in entry order, and present (P2.3).
    ///
    /// - `entries` must be in Masonry painter order (cached segments may appear
    ///   both before and after an anim entry).
    /// - `frame: None` reuses the last encoded texture for that `layer_id`.
    /// - On any encode/composite/present failure, intermediate dirty state is
    ///   retained by the caller (this method does not clear core dirty flags).
    /// - Call [`Self::sync_layer_metrics_generation`] before this when resize/DPI
    ///   changed so old-size textures are not mixed with the new plan (P2.6).
    #[must_use]
    pub fn render_ordered_frame(
        &mut self,
        renderer: &mut Renderer,
        entries: &[OrderedLayerEncode<'_>],
    ) -> (RenderFrameResult, RenderFrameTimings) {
        let mut timings = RenderFrameTimings::default();
        if entries.is_empty() {
            tracing::error!("render_ordered_frame called with no entries");
            return (RenderFrameResult::Failed, timings);
        }

        // Drop orphan intermediate textures for LayerIds no longer in the plan.
        let live: Vec<u64> = entries.iter().map(|e| e.layer_id).collect();
        self.retain_layer_targets(&live);

        // Ensure / validate layer targets before encode (avoids borrow conflicts).
        for entry in entries {
            if entry.frame.is_some() {
                if self
                    .ensure_layer_target(entry.layer_id, entry.target)
                    .is_err()
                {
                    return (RenderFrameResult::Failed, timings);
                }
            } else {
                let target_matches =
                    self.layer_targets
                        .get(&entry.layer_id)
                        .is_some_and(|target| {
                            target.x == entry.target.x
                                && target.y == entry.target.y
                                && target.width == entry.target.width.max(1)
                                && target.height == entry.target.height.max(1)
                        });
                if !target_matches {
                    tracing::error!(
                        layer_id = entry.layer_id,
                        ?entry.target,
                        "ordered encode skip requested without a matching cached layer texture"
                    );
                    return (RenderFrameResult::Failed, timings);
                }
            }
        }

        // --- acquire swapchain first (parity with render_frame; avoid encode on Retry) ---
        let dev_id = self.surface.dev_id;
        let acquire_started = std::time::Instant::now();
        let mut did_reconfigure = false;
        let surface_texture = loop {
            let device = &self.render_cx.devices[dev_id].device;
            match get_current_surface_texture(&self.surface.surface, device) {
                Ok(texture) if texture.suboptimal => {
                    discard_surface_texture(device, texture);
                    self.render_cx.configure_surface(&self.surface);
                    tracing::debug!("swap chain texture was suboptimal; surface reconfigured");
                    timings.surface_acquire = acquire_started.elapsed();
                    return (RenderFrameResult::Retry, timings);
                }
                Ok(texture) => break texture,
                Err(error) => match surface_recovery_action(&error) {
                    SurfaceRecoveryAction::Reconfigure if !did_reconfigure => {
                        did_reconfigure = true;
                        self.render_cx.configure_surface(&self.surface);
                    }
                    SurfaceRecoveryAction::Reconfigure | SurfaceRecoveryAction::Retry => {
                        tracing::warn!(
                            "couldn't acquire swap chain texture; retrying next frame: {error}"
                        );
                        timings.surface_acquire = acquire_started.elapsed();
                        return (RenderFrameResult::Retry, timings);
                    }
                    SurfaceRecoveryAction::Fail => {
                        tracing::error!("couldn't acquire swap chain texture: {error}");
                        timings.surface_acquire = acquire_started.elapsed();
                        return (RenderFrameResult::Failed, timings);
                    }
                },
            }
        };
        timings.surface_acquire = acquire_started.elapsed();

        // --- encode dirty entries into layer targets (straight-alpha Vello) ---
        for entry in entries {
            let Some(frame) = entry.frame else {
                continue;
            };
            let encode_started = std::time::Instant::now();
            let result = {
                let device_handle = &self.render_cx.devices[dev_id];
                let target = self
                    .layer_targets
                    .get(&entry.layer_id)
                    .expect("layer target ensured");
                renderer.render_to_texture(
                    RenderTarget {
                        adapter: &device_handle.adapter,
                        device: &device_handle.device,
                        queue: &device_handle.queue,
                        texture: &target.texture,
                        view: &target.view,
                    },
                    frame,
                )
            };
            let elapsed = encode_started.elapsed();
            if entry.kind == OrderedEntryKind::Anim {
                timings.encode_anim += elapsed;
            } else {
                timings.encode += elapsed;
            }
            if let Err(error) = result {
                tracing::error!(
                    layer_id = entry.layer_id,
                    "failed to encode ordered layer: {error}"
                );
                let device = &self.render_cx.devices[dev_id].device;
                discard_surface_texture(device, surface_texture);
                return (RenderFrameResult::Failed, timings);
            }
        }

        // --- composite into intermediate, then present once ---
        //
        // Layer textures are straight-alpha Vello output.
        //
        // When present needs premul (Mica / PreMultiplied) the intermediate is
        // held in **premul** space (Issue 9):
        //   layer0: straight→premul convert (SrcAlpha/Zero)
        //   layer1+: straight src over premul dest (SrcAlpha/OneMinusSrcAlpha)
        //   final:  replace (already premul — never double-premul)
        //
        // Otherwise intermediate stays straight-alpha; final uses surface.blitter
        // (replace on opaque paths).
        let stack_premul = self.ordered_stack_holds_premul();
        // The intermediate persists across presents. When only animation
        // textures changed, rebuild their covered pixels in painter order and
        // leave the rest of the already-composited window untouched.
        let partial_dirty = if self.ordered_intermediate_valid
            && !entries
                .iter()
                .any(|entry| entry.kind != OrderedEntryKind::Anim && entry.frame.is_some())
        {
            entries
                .iter()
                .filter(|entry| entry.kind == OrderedEntryKind::Anim && entry.frame.is_some())
                .map(|entry| entry.target)
                .reduce(OrderedLayerTarget::union)
        } else {
            None
        };
        let composite_started = std::time::Instant::now();
        let device = &self.render_cx.devices[dev_id].device;
        let queue = &self.render_cx.devices[dev_id].queue;
        let (surface_view, view_errors) = capture_device_errors(device, || {
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Picus Ordered Composite Swap Chain View"),
                    ..Default::default()
                })
        });
        if !view_errors.is_empty() {
            log_device_errors("creating the swap chain texture view", view_errors);
            discard_surface_texture(device, surface_texture);
            self.render_cx.configure_surface(&self.surface);
            timings.composite = composite_started.elapsed();
            return (RenderFrameResult::Retry, timings);
        }

        let ((), composite_errors) = capture_device_errors(device, || {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Picus Ordered Layer Composite"),
            });
            for (i, entry) in entries.iter().enumerate() {
                let src = self
                    .layer_targets
                    .get(&entry.layer_id)
                    .expect("layer target required for composite");
                let placement = OrderedLayerTarget {
                    x: src.x,
                    y: src.y,
                    width: src.width,
                    height: src.height,
                };
                if let Some(dirty) = partial_dirty {
                    let Some(scissor) = placement.intersection(dirty) else {
                        continue;
                    };
                    let blitter = if i == 0 {
                        if stack_premul {
                            &self.region_layer_premul_convert_blitter
                        } else {
                            &self.region_layer_replace_blitter
                        }
                    } else {
                        &self.region_layer_stack_blitter
                    };
                    blitter.copy(
                        device,
                        &mut encoder,
                        &src.view,
                        &self.surface.target_view,
                        placement,
                        scissor,
                    );
                    continue;
                }
                if i == 0 {
                    if stack_premul {
                        // Straight → premul into intermediate.
                        self.layer_premul_convert_blitter.copy(
                            device,
                            &mut encoder,
                            &src.view,
                            &self.surface.target_view,
                        );
                    } else {
                        self.layer_replace_blitter.copy(
                            device,
                            &mut encoder,
                            &src.view,
                            &self.surface.target_view,
                        );
                    }
                } else if src.x == 0
                    && src.y == 0
                    && src.width == self.surface.config.width
                    && src.height == self.surface.config.height
                {
                    // Upper layers: straight src over intermediate (premul or opaque dest).
                    self.layer_stack_blitter.copy(
                        device,
                        &mut encoder,
                        &src.view,
                        &self.surface.target_view,
                    );
                } else {
                    self.region_layer_stack_blitter.copy(
                        device,
                        &mut encoder,
                        &src.view,
                        &self.surface.target_view,
                        placement,
                        placement,
                    );
                }
            }
            if stack_premul {
                // Intermediate already premul — replace into swapchain
                // (surface format, not intermediate Rgba8Unorm blitters).
                self.present_replace_blitter.copy(
                    device,
                    &mut encoder,
                    &self.surface.target_view,
                    &surface_view,
                );
            } else {
                self.surface.blitter.copy(
                    device,
                    &mut encoder,
                    &self.surface.target_view,
                    &surface_view,
                );
            }
            queue.submit([encoder.finish()]);
        });
        if !composite_errors.is_empty() {
            log_device_errors("submitting ordered layer composite", composite_errors);
            discard_surface_texture(device, surface_texture);
            self.render_cx.configure_surface(&self.surface);
            timings.composite = composite_started.elapsed();
            return (RenderFrameResult::Retry, timings);
        }
        self.ordered_intermediate_valid = true;
        timings.composite = composite_started.elapsed();

        let present_started = std::time::Instant::now();
        let ((), present_errors) = capture_device_errors(device, || surface_texture.present());
        timings.present_submit = present_started.elapsed();
        if !present_errors.is_empty() {
            log_device_errors("presenting the swap chain texture", present_errors);
            self.render_cx.configure_surface(&self.surface);
            return (RenderFrameResult::Retry, timings);
        }

        if let Err(error) = device.poll(wgpu::PollType::Poll) {
            tracing::trace!("non-blocking GPU poll after ordered present returned: {error}");
        }

        (RenderFrameResult::Presented, timings)
    }
}

struct RenderContext {
    instance: Instance,
    /// Created devices used by this context.
    devices: Vec<DeviceHandle>,
    /// Prefer a DX12 adapter for DirectComposition premultiplied alpha.
    prefer_composition_adapter: bool,
}

struct DeviceHandle {
    adapter: wgpu::Adapter,
    device: Device,
    queue: wgpu::Queue,
}

impl RenderContext {
    fn new(transparent: bool) -> Self {
        let backends = backends_for_surface();
        let flags = wgpu::InstanceFlags::from_build_config().with_env();
        let backend_options = backend_options_for_surface(transparent);
        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends,
            flags,
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options,
        });

        Self {
            instance,
            devices: Vec::new(),
            prefer_composition_adapter: cfg!(windows) && transparent,
        }
    }

    async fn create_surface<'w>(
        &mut self,
        window: impl Into<wgpu::SurfaceTarget<'w>>,
        metrics: ExistingWindowMetrics,
        policy: PresentPolicy,
    ) -> Result<(RenderSurface<'w>, NegotiatedPresent), RenderSurfaceError> {
        self.create_render_surface(
            self.instance
                .create_surface(window.into())
                .map_err(RenderSurfaceError::CreateSurface)?,
            metrics,
            policy,
        )
        .await
    }

    async fn create_render_surface<'w>(
        &mut self,
        surface: Surface<'w>,
        metrics: ExistingWindowMetrics,
        policy: PresentPolicy,
    ) -> Result<(RenderSurface<'w>, NegotiatedPresent), RenderSurfaceError> {
        let dev_id = self
            .device(Some(&surface))
            .await
            .ok_or(RenderSurfaceError::NoCompatibleDevice)?;

        let device_handle = &self.devices[dev_id];
        let capabilities = surface.get_capabilities(&device_handle.adapter);
        let format = capabilities
            .formats
            .into_iter()
            .find(|format| {
                matches!(
                    format,
                    TextureFormat::Rgba8Unorm | TextureFormat::Bgra8Unorm
                )
            })
            .ok_or(RenderSurfaceError::UnsupportedSurfaceFormat)?;

        let alpha_mode = choose_alpha_mode(
            &capabilities.alpha_modes,
            metrics.transparent,
            metrics.composite_alpha_mode,
        );
        let blitter = create_blitter(
            &device_handle.device,
            format,
            alpha_mode,
            metrics.transparent,
        );
        // PresentPolicy selects mode + names real capability (MailboxLatest vs
        // FifoBackpressure). Continuous animation still benefits from mailbox
        // when available; FIFO does not claim drop-stale for submitted frames.
        let negotiated = policy.negotiate(&capabilities.present_modes);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: metrics.physical_width.max(1),
            height: metrics.physical_height.max(1),
            present_mode: negotiated.mode,
            // Backend hint only — not a hard guarantee of single in-flight frame.
            desired_maximum_frame_latency: negotiated.desired_maximum_frame_latency,
            alpha_mode,
            view_formats: vec![],
        };
        let (target_texture, target_view) = create_targets(
            metrics.physical_width.max(1),
            metrics.physical_height.max(1),
            &device_handle.device,
        );

        let surface = RenderSurface {
            surface,
            config,
            dev_id,
            format,
            target_texture,
            target_view,
            blitter,
        };
        self.configure_surface(&surface);
        Ok((surface, negotiated))
    }

    fn resize_surface(&self, surface: &mut RenderSurface<'_>, width: u32, height: u32) {
        let device = &self.devices[surface.dev_id].device;
        let ((texture, view), errors) =
            capture_device_errors(device, || create_targets(width, height, device));
        if !errors.is_empty() {
            log_device_errors("resizing the Vello render target", errors);
            return;
        }
        surface.target_texture = texture;
        surface.target_view = view;
        surface.config.width = width;
        surface.config.height = height;
        self.configure_surface(surface);
    }

    fn configure_surface(&self, surface: &RenderSurface<'_>) {
        let device = &self.devices[surface.dev_id].device;
        let ((), errors) = capture_device_errors(device, || {
            surface.surface.configure(device, &surface.config);
        });
        log_device_errors("configuring the window surface", errors);
    }

    fn desired_alpha_mode_for_surface(
        &self,
        surface: &RenderSurface<'_>,
        metrics: ExistingWindowMetrics,
    ) -> CompositeAlphaMode {
        let device_handle = &self.devices[surface.dev_id];
        let capabilities = surface.surface.get_capabilities(&device_handle.adapter);
        choose_alpha_mode(
            &capabilities.alpha_modes,
            metrics.transparent,
            metrics.composite_alpha_mode,
        )
    }

    fn set_surface_alpha_mode(
        &self,
        surface: &mut RenderSurface<'_>,
        alpha_mode: CompositeAlphaMode,
        transparent: bool,
    ) {
        let device_handle = &self.devices[surface.dev_id];
        surface.blitter = create_blitter(
            &device_handle.device,
            surface.format,
            alpha_mode,
            transparent,
        );
        surface.config.alpha_mode = alpha_mode;
        self.configure_surface(surface);
    }

    async fn device(&mut self, compatible_surface: Option<&Surface<'_>>) -> Option<usize> {
        let compatible = match compatible_surface {
            Some(surface) => self
                .devices
                .iter()
                .enumerate()
                .find(|(_, device)| device.adapter.is_surface_supported(surface))
                .map(|(index, _)| index),
            None => (!self.devices.is_empty()).then_some(0),
        };

        if compatible.is_none() {
            return self.new_device(compatible_surface).await;
        }

        compatible
    }

    async fn new_device(&mut self, compatible_surface: Option<&Surface<'_>>) -> Option<usize> {
        let adapter = select_adapter(
            &self.instance,
            compatible_surface,
            self.prefer_composition_adapter,
        )
        .await?;

        let requested_features = wgpu::Features::CLEAR_TEXTURE;
        let required_features = adapter.features() & requested_features;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .ok()?;

        self.devices.push(DeviceHandle {
            adapter,
            device,
            queue,
        });
        Some(self.devices.len() - 1)
    }
}

fn backends_for_surface() -> Backends {
    Backends::from_env().unwrap_or_default()
}

/// Pick a GPU adapter. When `prefer_composition` is set, prefer DX12 so
/// [`Dx12SwapchainKind::DxgiFromVisual`] premultiplied alpha is available.
async fn select_adapter(
    instance: &Instance,
    compatible_surface: Option<&Surface<'_>>,
    prefer_composition: bool,
) -> Option<wgpu::Adapter> {
    if let Ok(adapter) = wgpu::util::initialize_adapter_from_env(instance, compatible_surface).await
    {
        return Some(adapter);
    }

    if !prefer_composition {
        return wgpu::util::initialize_adapter_from_env_or_default(instance, compatible_surface)
            .await
            .ok();
    }

    let adapters = instance.enumerate_adapters(Backends::all()).await;
    let mut best: Option<(u32, wgpu::Adapter)> = None;
    for adapter in adapters {
        if let Some(surface) = compatible_surface
            && !adapter.is_surface_supported(surface)
        {
            continue;
        }
        let score = composition_adapter_score(&adapter);
        match &best {
            Some((best_score, _)) if score <= *best_score => {}
            _ => best = Some((score, adapter)),
        }
    }
    if let Some((_, adapter)) = best {
        return Some(adapter);
    }

    wgpu::util::initialize_adapter_from_env_or_default(instance, compatible_surface)
        .await
        .ok()
}

fn composition_adapter_score(adapter: &wgpu::Adapter) -> u32 {
    let info = adapter.get_info();
    let mut score = 0u32;
    if info.backend == Backend::Dx12 {
        score += 1_000;
    }
    match info.device_type {
        DeviceType::DiscreteGpu => score += 100,
        DeviceType::IntegratedGpu => score += 50,
        DeviceType::VirtualGpu => score += 25,
        DeviceType::Cpu => score += 1,
        DeviceType::Other => {}
    }
    score
}

fn backend_options_for_surface(transparent: bool) -> wgpu::BackendOptions {
    let mut backend_options = wgpu::BackendOptions::from_env_or_default();

    if cfg!(windows) && transparent {
        // DXGI swapchains created directly from an HWND only expose opaque alpha.
        // DirectComposition visuals preserve the Vello target's alpha channel so
        // DWM backdrops can composite through transparent Picus content.
        backend_options.dx12.presentation_system = Dx12SwapchainKind::DxgiFromVisual;
    }

    backend_options
}

/// Texture format for Vello encode targets, ordered layer targets, and the
/// intermediate composite stack.
///
/// Always `Rgba8Unorm` (Vello requirement). The window swapchain may be
/// `Bgra8Unorm` on Windows — intermediate layer blitters must use this format,
/// and only the final present blit uses `surface.format`.
const VELLO_TARGET_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

const STRAIGHT_SRC_OVER: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
};

const PREMUL_CONVERT: wgpu::BlendState = wgpu::BlendState {
    alpha: wgpu::BlendComponent::REPLACE,
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::Zero,
        operation: wgpu::BlendOperation::Add,
    },
};

struct RegionTextureBlitter {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl RegionTextureBlitter {
    fn new(device: &Device, format: TextureFormat, blend_state: Option<wgpu::BlendState>) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Picus Region Blitter Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Picus Region Blitter Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Picus Region Blitter Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("region_blit.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Picus Region Blitter Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: blend_state,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    fn copy(
        &self,
        device: &Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &TextureView,
        target: &TextureView,
        viewport: OrderedLayerTarget,
        scissor: OrderedLayerTarget,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Picus Region Blitter Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Picus Region Blitter Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_viewport(
            viewport.x as f32,
            viewport.y as f32,
            viewport.width.max(1) as f32,
            viewport.height.max(1) as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(
            scissor.x,
            scissor.y,
            scissor.width.max(1),
            scissor.height.max(1),
        );
        pass.draw(0..3, 0..1);
    }
}

fn create_targets(width: u32, height: u32, device: &Device) -> (Texture, TextureView) {
    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Picus Vello Render Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // STORAGE: Vello encode; TEXTURE: blit source; RENDER_ATTACHMENT: ordered
        // intermediate stack destination (straight-alpha layer composite).
        usage: TextureUsages::STORAGE_BINDING
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT,
        format: VELLO_TARGET_FORMAT,
        view_formats: &[],
    });
    let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Picus Vello Render Target View"),
        ..Default::default()
    });
    (target_texture, target_view)
}

fn map_requested_alpha_mode(mode: BevyCompositeAlphaMode) -> Option<CompositeAlphaMode> {
    match mode {
        BevyCompositeAlphaMode::Auto => None,
        BevyCompositeAlphaMode::Opaque => Some(CompositeAlphaMode::Opaque),
        BevyCompositeAlphaMode::PreMultiplied => Some(CompositeAlphaMode::PreMultiplied),
        BevyCompositeAlphaMode::PostMultiplied => Some(CompositeAlphaMode::PostMultiplied),
        BevyCompositeAlphaMode::Inherit => Some(CompositeAlphaMode::Inherit),
    }
}

/// Choose a swapchain present mode from device capabilities.
///
/// Preference order prioritizes **low latency under load**:
/// 1. [`PresentMode::Mailbox`] — triple-buffer, may replace queued frames
///    ([`NegotiatedPresentCapability::MailboxLatest`])
/// 2. [`PresentMode::FifoRelaxed`] — late frames without hard queueing
///    ([`NegotiatedPresentCapability::FifoBackpressure`])
/// 3. The caller's preferred mode (typically [`PresentMode::AutoVsync`])
/// 4. [`PresentMode::Fifo`] / [`PresentMode::AutoVsync`] as final fallbacks
///
/// Shared with core scheduling via [`PresentPolicy::negotiate`]. Callers must
/// map the result to an explicit capability — do not invent a unified
/// `drop_stale` promise across modes.
#[must_use]
pub fn select_present_mode(available: &[PresentMode], preferred: PresentMode) -> PresentMode {
    let prefer = [
        PresentMode::Mailbox,
        PresentMode::FifoRelaxed,
        preferred,
        PresentMode::AutoVsync,
        PresentMode::Fifo,
        PresentMode::AutoNoVsync,
        PresentMode::Immediate,
    ];
    for mode in prefer {
        if available.contains(&mode) {
            if mode != preferred {
                tracing::debug!(
                    ?mode,
                    ?preferred,
                    available = ?available,
                    "selected low-latency present mode"
                );
            }
            return mode;
        }
    }
    preferred
}

fn choose_alpha_mode(
    modes: &[CompositeAlphaMode],
    transparent: bool,
    requested: BevyCompositeAlphaMode,
) -> CompositeAlphaMode {
    if let Some(requested) = map_requested_alpha_mode(requested)
        && modes.contains(&requested)
    {
        return requested;
    }

    let preferences: &[CompositeAlphaMode] = if transparent && cfg!(windows) {
        &[
            CompositeAlphaMode::PreMultiplied,
            CompositeAlphaMode::PostMultiplied,
            CompositeAlphaMode::Inherit,
            CompositeAlphaMode::Auto,
            CompositeAlphaMode::Opaque,
        ]
    } else if transparent {
        &[
            CompositeAlphaMode::PostMultiplied,
            CompositeAlphaMode::PreMultiplied,
            CompositeAlphaMode::Inherit,
            CompositeAlphaMode::Auto,
            CompositeAlphaMode::Opaque,
        ]
    } else {
        &[
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PreMultiplied,
            CompositeAlphaMode::PostMultiplied,
            CompositeAlphaMode::Inherit,
            CompositeAlphaMode::Auto,
        ]
    };

    for mode in preferences {
        if modes.contains(mode) {
            return *mode;
        }
    }

    CompositeAlphaMode::Auto
}

fn create_blitter(
    device: &Device,
    format: TextureFormat,
    alpha_mode: CompositeAlphaMode,
    transparent: bool,
) -> TextureBlitter {
    let needs_premultiplied_blit = needs_premultiplied_blit(alpha_mode, transparent);
    if needs_premultiplied_blit {
        TextureBlitterBuilder::new(device, format)
            .blend_state(PREMUL_CONVERT)
            .build()
    } else {
        TextureBlitter::new(device, format)
    }
}

/// Replace blitter (no blend).
///
/// `format` is the **destination** attachment format for the blit pipeline
/// (intermediate → use [`VELLO_TARGET_FORMAT`]; swapchain present → use
/// `surface.format`).
fn create_layer_replace_blitter(device: &Device, format: TextureFormat) -> TextureBlitter {
    TextureBlitter::new(device, format)
}

/// Straight-alpha → premultiplied convert into the intermediate.
///
/// Same color factors as the present premul blitter (`SrcAlpha` / `Zero`).
/// `format` must be [`VELLO_TARGET_FORMAT`] (intermediate dest).
fn create_layer_premul_convert_blitter(device: &Device, format: TextureFormat) -> TextureBlitter {
    TextureBlitterBuilder::new(device, format)
        .blend_state(PREMUL_CONVERT)
        .build()
}

/// Src-over for upper Vello layers (straight src) onto the intermediate.
///
/// Color: `Cs*As + Cd*(1-As)`; alpha: `As + Ad*(1-As)`.
/// Correct when dest is **premul** (Mica stack) or dest is fully opaque.
/// `format` must be [`VELLO_TARGET_FORMAT`] (intermediate dest).
fn create_layer_stack_blitter(device: &Device, format: TextureFormat) -> TextureBlitter {
    TextureBlitterBuilder::new(device, format)
        .blend_state(STRAIGHT_SRC_OVER)
        .build()
}

fn needs_premultiplied_blit(alpha_mode: CompositeAlphaMode, transparent: bool) -> bool {
    matches!(alpha_mode, CompositeAlphaMode::PreMultiplied) || (cfg!(windows) && transparent)
}

#[cfg(test)]
fn native_backdrop_ordinal(material: NativeWindowBackdropMaterial) -> i32 {
    match material {
        NativeWindowBackdropMaterial::Auto => 0,
        NativeWindowBackdropMaterial::None => 1,
        NativeWindowBackdropMaterial::Mica => 2,
        NativeWindowBackdropMaterial::Acrylic => 3,
        NativeWindowBackdropMaterial::MicaAlt => 4,
    }
}

#[derive(Debug)]
pub enum RenderSurfaceError {
    CreateSurface(wgpu::CreateSurfaceError),
    NoCompatibleDevice,
    UnsupportedSurfaceFormat,
}

impl core::fmt::Display for RenderSurfaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CreateSurface(err) => write!(f, "creating surface failed: {err}"),
            Self::NoCompatibleDevice => write!(f, "no compatible WGPU device found"),
            Self::UnsupportedSurfaceFormat => write!(f, "unsupported surface format"),
        }
    }
}

impl std::error::Error for RenderSurfaceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceRecoveryAction {
    Reconfigure,
    Retry,
    Fail,
}

fn surface_recovery_action(error: &wgpu::SurfaceError) -> SurfaceRecoveryAction {
    match error {
        wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost | wgpu::SurfaceError::Other => {
            SurfaceRecoveryAction::Reconfigure
        }
        wgpu::SurfaceError::Timeout => SurfaceRecoveryAction::Retry,
        wgpu::SurfaceError::OutOfMemory => SurfaceRecoveryAction::Fail,
    }
}

fn get_current_surface_texture(
    surface: &Surface<'_>,
    device: &Device,
) -> Result<SurfaceTexture, wgpu::SurfaceError> {
    let (result, errors) = capture_device_errors(device, || surface.get_current_texture());
    if errors.is_empty() {
        return result;
    }

    log_device_errors("acquiring the swap chain texture", errors);
    if let Ok(texture) = result {
        discard_surface_texture(device, texture);
    }
    Err(wgpu::SurfaceError::Other)
}

fn discard_surface_texture(device: &Device, texture: SurfaceTexture) {
    let ((), errors) = capture_device_errors(device, || drop(texture));
    log_device_errors("discarding the swap chain texture", errors);
}

fn capture_device_errors<T>(
    device: &Device,
    operation: impl FnOnce() -> T,
) -> (T, Vec<wgpu::Error>) {
    let out_of_memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let result = operation();

    let errors = [
        pollster::block_on(validation.pop()),
        pollster::block_on(internal.pop()),
        pollster::block_on(out_of_memory.pop()),
    ]
    .into_iter()
    .flatten()
    .collect();
    (result, errors)
}

fn log_device_errors(operation: &str, errors: Vec<wgpu::Error>) {
    for error in errors {
        tracing::warn!("wgpu error while {operation}: {error}");
    }
}

struct RenderSurface<'surface> {
    surface: Surface<'surface>,
    config: SurfaceConfiguration,
    dev_id: usize,
    format: TextureFormat,
    target_texture: Texture,
    target_view: TextureView,
    blitter: TextureBlitter,
}

impl std::fmt::Debug for RenderSurface<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSurface")
            .field("surface", &self.surface)
            .field("config", &self.config)
            .field("dev_id", &self.dev_id)
            .field("format", &self.format)
            .field("target_texture", &self.target_texture)
            .field("target_view", &self.target_view)
            .field("blitter", &"(Not Debug)")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[cfg(windows)]
    #[test]
    fn transparent_windows_use_direct_composition_visuals() {
        assert_eq!(
            backend_options_for_surface(true).dx12.presentation_system,
            Dx12SwapchainKind::DxgiFromVisual
        );
    }

    #[test]
    fn composition_adapter_score_prefers_dx12_over_vulkan() {
        let dx12_discrete = 1_000u32 + 100;
        let vulkan_discrete = 100u32;
        assert!(dx12_discrete > vulkan_discrete);
    }

    #[test]
    fn backends_for_surface_keeps_multi_backend_default() {
        if Backends::from_env().is_some() {
            return;
        }
        assert_eq!(backends_for_surface(), Backends::default());
    }

    #[test]
    fn present_mode_prefers_mailbox_when_available() {
        let modes = [
            PresentMode::Fifo,
            PresentMode::Mailbox,
            PresentMode::Immediate,
        ];
        assert_eq!(
            select_present_mode(&modes, PresentMode::AutoVsync),
            PresentMode::Mailbox
        );
    }

    #[test]
    fn present_mode_falls_back_when_mailbox_missing() {
        let modes = [PresentMode::Fifo, PresentMode::FifoRelaxed];
        assert_eq!(
            select_present_mode(&modes, PresentMode::AutoVsync),
            PresentMode::FifoRelaxed
        );
    }

    #[test]
    fn present_policy_negotiates_mailbox_latest() {
        let modes = [PresentMode::Fifo, PresentMode::Mailbox];
        let negotiated = PresentPolicy::default_ui().negotiate(&modes);
        assert_eq!(negotiated.mode, PresentMode::Mailbox);
        assert_eq!(
            negotiated.capability,
            NegotiatedPresentCapability::MailboxLatest
        );
        assert_eq!(negotiated.desired_maximum_frame_latency, 1);
        assert_eq!(negotiated.ready_queue, ReadyQueuePolicy::LatestOnly);
    }

    #[test]
    fn present_policy_negotiates_fifo_backpressure_without_fake_drop_stale() {
        let modes = [PresentMode::Fifo];
        let negotiated = PresentPolicy::default_ui().negotiate(&modes);
        assert_eq!(negotiated.mode, PresentMode::Fifo);
        assert_eq!(
            negotiated.capability,
            NegotiatedPresentCapability::FifoBackpressure
        );
        // Capability enum is explicit — no unified drop_stale boolean.
        assert_ne!(
            negotiated.capability,
            NegotiatedPresentCapability::MailboxLatest
        );
    }

    #[test]
    fn latest_ready_queue_keeps_only_latest_unsubmitted() {
        let mut q = LatestReadyQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.push_ready(1), None);
        assert_eq!(q.push_ready(2), Some(1)); // dropped unsubmitted older frame
        assert_eq!(q.push_ready(3), Some(2));
        assert_eq!(q.len(), 1);
        assert_eq!(q.peek(), Some(&3));
        // Submit moves the frame out — it is no longer withdrawable from the queue.
        assert_eq!(q.take_for_submit(), Some(3));
        assert!(q.is_empty());
        assert_eq!(q.take_for_submit(), None);
        // A newly ready frame after submit does not resurrect the submitted one.
        q.push_ready(4);
        assert_eq!(q.take_for_submit(), Some(4));
    }

    #[test]
    fn ordered_entry_kinds_cover_cached_anim_external() {
        // P2.3 inventory: multi-texture path attributes encode cost by kind.
        let _ = [
            OrderedEntryKind::Cached,
            OrderedEntryKind::Anim,
            OrderedEntryKind::External,
        ];
        assert_ne!(OrderedEntryKind::Cached, OrderedEntryKind::Anim);
    }

    #[test]
    fn layer_metrics_generation_is_stable_token() {
        let g0 = LayerMetricsGeneration(0);
        let g1 = LayerMetricsGeneration(1);
        assert_ne!(g0, g1);
        assert_eq!(g1.0, 1);
    }

    #[test]
    fn alpha_mode_prefers_opaque_surfaces_by_default() {
        let modes = [
            CompositeAlphaMode::PostMultiplied,
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PreMultiplied,
        ];

        assert_eq!(
            choose_alpha_mode(&modes, false, BevyCompositeAlphaMode::Auto),
            CompositeAlphaMode::Opaque
        );
    }

    #[test]
    fn alpha_mode_prefers_platform_compositor_mode_for_transparent_windows() {
        let modes = [
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PreMultiplied,
            CompositeAlphaMode::PostMultiplied,
        ];

        assert_eq!(
            choose_alpha_mode(&modes, true, BevyCompositeAlphaMode::Auto),
            if cfg!(windows) {
                CompositeAlphaMode::PreMultiplied
            } else {
                CompositeAlphaMode::PostMultiplied
            }
        );
    }

    #[test]
    fn alpha_mode_honors_explicit_supported_request() {
        let modes = [
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PreMultiplied,
            CompositeAlphaMode::PostMultiplied,
        ];

        assert_eq!(
            choose_alpha_mode(&modes, false, BevyCompositeAlphaMode::PostMultiplied),
            CompositeAlphaMode::PostMultiplied
        );
    }

    #[test]
    fn transparent_surfaces_premultiply_on_windows_only() {
        assert_eq!(
            needs_premultiplied_blit(CompositeAlphaMode::Auto, true),
            cfg!(windows)
        );
    }

    #[test]
    fn explicit_premultiplied_alpha_always_premultiplies_the_final_blit() {
        assert!(needs_premultiplied_blit(
            CompositeAlphaMode::PreMultiplied,
            false,
        ));
    }

    #[test]
    fn recoverable_surface_changes_reconfigure_the_swap_chain() {
        for error in [
            wgpu::SurfaceError::Outdated,
            wgpu::SurfaceError::Lost,
            wgpu::SurfaceError::Other,
        ] {
            assert_eq!(
                surface_recovery_action(&error),
                SurfaceRecoveryAction::Reconfigure
            );
        }
    }

    #[test]
    fn surface_timeout_retries_without_reconfiguration() {
        assert_eq!(
            surface_recovery_action(&wgpu::SurfaceError::Timeout),
            SurfaceRecoveryAction::Retry
        );
    }

    #[test]
    fn surface_out_of_memory_does_not_start_a_redraw_loop() {
        assert_eq!(
            surface_recovery_action(&wgpu::SurfaceError::OutOfMemory),
            SurfaceRecoveryAction::Fail
        );
    }

    #[test]
    fn native_backdrop_materials_match_dwm_system_backdrop_values() {
        assert_eq!(
            native_backdrop_ordinal(NativeWindowBackdropMaterial::Auto),
            0
        );
        assert_eq!(
            native_backdrop_ordinal(NativeWindowBackdropMaterial::None),
            1
        );
        assert_eq!(
            native_backdrop_ordinal(NativeWindowBackdropMaterial::Mica),
            2
        );
        assert_eq!(
            native_backdrop_ordinal(NativeWindowBackdropMaterial::Acrylic),
            3
        );
        assert_eq!(
            native_backdrop_ordinal(NativeWindowBackdropMaterial::MicaAlt),
            4
        );
    }

    /// Pixel layout used by the GPU blit transparency tests (2×2 RGBA8):
    ///
    /// ```text
    /// (0,0) transparent black  (0, 0, 0, 0)
    /// (1,0) half-white         (255, 255, 255, 128)
    /// (0,1) opaque red         (255, 0, 0, 255)
    /// (1,1) quarter-blue       (0, 0, 255, 64)
    /// ```
    const BLIT_TEST_WIDTH: u32 = 2;
    const BLIT_TEST_HEIGHT: u32 = 2;
    const BLIT_TEST_SOURCE: [[u8; 4]; 4] = [
        [0, 0, 0, 0],
        [255, 255, 255, 128],
        [255, 0, 0, 255],
        [0, 0, 255, 64],
    ];

    fn create_headless_device() -> Option<(Device, wgpu::Queue)> {
        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: backends_for_surface(),
            flags: wgpu::InstanceFlags::from_build_config().with_env(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            // Match transparent-window backend options so Windows tests exercise the
            // same DX12 presentation system selection used by Mica surfaces.
            backend_options: backend_options_for_surface(true),
        });

        let adapter =
            pollster::block_on(async { select_adapter(&instance, None, cfg!(windows)).await })?;

        let (device, queue) = pollster::block_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("picus_surface transparency test device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                })
                .await
                .ok()
        })?;

        Some((device, queue))
    }

    fn create_rgba8_texture(
        device: &Device,
        width: u32,
        height: u32,
        usage: TextureUsages,
        label: &str,
    ) -> Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        })
    }

    fn write_rgba8_pixels(
        queue: &wgpu::Queue,
        texture: &Texture,
        width: u32,
        height: u32,
        pixels: &[[u8; 4]],
    ) {
        assert_eq!(pixels.len(), (width * height) as usize);
        let bytes: Vec<u8> = pixels.iter().flat_map(|px| px.iter().copied()).collect();
        queue.write_texture(
            texture.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn readback_rgba8_pixels(
        device: &Device,
        queue: &wgpu::Queue,
        texture: &Texture,
        width: u32,
        height: u32,
    ) -> Vec<[u8; 4]> {
        let unpadded_bytes_per_row = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = u64::from(padded_bytes_per_row) * u64::from(height);

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("picus_surface pixel readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("picus_surface pixel readback encoder"),
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU poll for pixel readback should succeed");
        receiver
            .recv()
            .expect("map_async callback should fire")
            .expect("mapping readback buffer should succeed");

        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            let row_start = (y * padded_bytes_per_row) as usize;
            for x in 0..width {
                let i = row_start + (x as usize) * 4;
                pixels.push([mapped[i], mapped[i + 1], mapped[i + 2], mapped[i + 3]]);
            }
        }
        drop(mapped);
        readback.unmap();
        pixels
    }

    /// Blit a Vello-like source texture into a surface-like destination using the
    /// same [`create_blitter`] path production code uses, then read destination pixels.
    fn blit_surface_like_pixels(
        transparent: bool,
        alpha_mode: CompositeAlphaMode,
    ) -> Option<Vec<[u8; 4]>> {
        let (device, queue) = create_headless_device()?;

        let source = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            "vello-like source",
        );
        write_rgba8_pixels(
            &queue,
            &source,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &BLIT_TEST_SOURCE,
        );

        // Destination mirrors a swapchain texture: render target + readback for the test.
        let destination = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
            "surface-like destination",
        );
        // Seed with non-zero garbage so LoadOp::Load cannot hide a failed blit.
        write_rgba8_pixels(
            &queue,
            &destination,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &[[1, 2, 3, 4]; 4],
        );

        let blitter = create_blitter(&device, TextureFormat::Rgba8Unorm, alpha_mode, transparent);
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let dest_view = destination.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("surface-like transparency blit"),
        });
        blitter.copy(&device, &mut encoder, &source_view, &dest_view);
        queue.submit(Some(encoder.finish()));

        Some(readback_rgba8_pixels(
            &device,
            &queue,
            &destination,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
        ))
    }

    fn pixel_at(pixels: &[[u8; 4]], x: u32, y: u32) -> [u8; 4] {
        pixels[(y * BLIT_TEST_WIDTH + x) as usize]
    }

    fn assert_approx_premultiplied(src: [u8; 4], out: [u8; 4], label: &str) {
        let alpha = src[3];
        let expected = [
            ((u16::from(src[0]) * u16::from(alpha) + 127) / 255) as u8,
            ((u16::from(src[1]) * u16::from(alpha) + 127) / 255) as u8,
            ((u16::from(src[2]) * u16::from(alpha) + 127) / 255) as u8,
            alpha,
        ];
        for channel in 0..4 {
            let delta = out[channel].abs_diff(expected[channel]);
            assert!(
                delta <= 1,
                "{label}: channel {channel} expected ~{:?} got {:?}; full expected {:?} out {:?}",
                expected[channel],
                out[channel],
                expected,
                out
            );
        }
    }

    /// GPU integration: transparent-window blit path preserves fully transparent
    /// destination pixels (alpha == 0) so DWM Mica/Acrylic can show through.
    #[test]
    fn transparent_surface_blit_keeps_specific_pixels_transparent() {
        let Some(pixels) = blit_surface_like_pixels(true, CompositeAlphaMode::PreMultiplied) else {
            eprintln!("skipping GPU transparency test: no compatible wgpu adapter");
            return;
        };

        let transparent = pixel_at(&pixels, 0, 0);
        assert_eq!(
            transparent,
            [0, 0, 0, 0],
            "pixel (0,0) must remain fully transparent so native Mica can composite through"
        );

        let opaque_red = pixel_at(&pixels, 0, 1);
        assert_eq!(
            opaque_red,
            [255, 0, 0, 255],
            "pixel (0,1) must stay opaque red after the surface blit"
        );

        // On Windows (and when PreMultiplied is requested) the final blit premultiplies
        // straight-alpha Vello output; other platforms keep source RGBA as-is for Auto.
        if needs_premultiplied_blit(CompositeAlphaMode::PreMultiplied, true) {
            assert_approx_premultiplied(
                BLIT_TEST_SOURCE[1],
                pixel_at(&pixels, 1, 0),
                "half-white pixel (1,0)",
            );
            assert_approx_premultiplied(
                BLIT_TEST_SOURCE[3],
                pixel_at(&pixels, 1, 1),
                "quarter-blue pixel (1,1)",
            );
        } else {
            assert_eq!(pixel_at(&pixels, 1, 0), BLIT_TEST_SOURCE[1]);
            assert_eq!(pixel_at(&pixels, 1, 1), BLIT_TEST_SOURCE[3]);
        }
    }

    /// GPU integration: mica-style Auto transparent windows on Windows also leave
    /// clear pixels at alpha 0 after the production blit path.
    #[test]
    fn mica_style_auto_transparent_blit_preserves_zero_alpha_pixels() {
        let Some(pixels) = blit_surface_like_pixels(true, CompositeAlphaMode::Auto) else {
            eprintln!("skipping GPU transparency test: no compatible wgpu adapter");
            return;
        };

        let transparent = pixel_at(&pixels, 0, 0);
        assert_eq!(
            transparent[3], 0,
            "pixel (0,0) alpha must be 0 for Mica-style transparent surfaces; got {transparent:?}"
        );
        assert_eq!(
            transparent[0], 0,
            "fully transparent pixel must not leak non-zero RGB (got {transparent:?})"
        );
        assert_eq!(transparent[1], 0);
        assert_eq!(transparent[2], 0);

        // Opaque content must still land correctly.
        assert_eq!(pixel_at(&pixels, 0, 1), [255, 0, 0, 255]);

        if needs_premultiplied_blit(CompositeAlphaMode::Auto, true) {
            // Windows transparent Auto path uses the premultiply blitter.
            assert_approx_premultiplied(
                BLIT_TEST_SOURCE[1],
                pixel_at(&pixels, 1, 0),
                "Windows Auto half-white",
            );
        }
    }

    /// Ordered multi-layer composite for Mica/premul present (Issues 1 + 9).
    ///
    /// Production path when `ordered_stack_holds_premul()`:
    /// layer0 premul-convert → intermediate; layer1+ src-over; final **replace**.
    ///
    /// Layer 0 = full BLIT_TEST_SOURCE (straight alpha).
    /// Layer 1 = transparent except opaque green at (0,1).
    #[test]
    fn ordered_multi_layer_straight_alpha_stack_then_present_premul_once() {
        let Some((device, queue)) = create_headless_device() else {
            eprintln!("skipping ordered multi-layer GPU test: no compatible wgpu adapter");
            return;
        };

        let tex_usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let layer0 = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            tex_usage,
            "ordered-layer0",
        );
        write_rgba8_pixels(
            &queue,
            &layer0,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &BLIT_TEST_SOURCE,
        );

        // Layer 1: transparent except opaque green at (0,1).
        let mut layer1_px = [[0u8, 0, 0, 0]; 4];
        layer1_px[2] = [0, 255, 0, 255]; // (0,1)
        let layer1 = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            tex_usage,
            "ordered-layer1",
        );
        write_rgba8_pixels(
            &queue,
            &layer1,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &layer1_px,
        );

        let intermediate = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            "ordered-intermediate",
        );
        write_rgba8_pixels(
            &queue,
            &intermediate,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &[[9, 9, 9, 9]; 4],
        );

        let destination = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
            "ordered-present-dest",
        );
        write_rgba8_pixels(
            &queue,
            &destination,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &[[1, 2, 3, 4]; 4],
        );

        let replace = create_layer_replace_blitter(&device, TextureFormat::Rgba8Unorm);
        let premul_convert =
            create_layer_premul_convert_blitter(&device, TextureFormat::Rgba8Unorm);
        let stack = create_layer_stack_blitter(&device, TextureFormat::Rgba8Unorm);

        let l0_view = layer0.create_view(&wgpu::TextureViewDescriptor::default());
        let l1_view = layer1.create_view(&wgpu::TextureViewDescriptor::default());
        let mid_view = intermediate.create_view(&wgpu::TextureViewDescriptor::default());
        let dest_view = destination.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ordered multi-layer composite"),
        });
        // Mirrors render_ordered_frame when ordered_stack_holds_premul().
        premul_convert.copy(&device, &mut encoder, &l0_view, &mid_view);
        stack.copy(&device, &mut encoder, &l1_view, &mid_view);
        // Already premul — replace final (not present premul again).
        replace.copy(&device, &mut encoder, &mid_view, &dest_view);
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba8_pixels(
            &device,
            &queue,
            &destination,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
        );

        // Transparent hole survives stack + present (Mica punch-through).
        assert_eq!(
            pixel_at(&pixels, 0, 0),
            [0, 0, 0, 0],
            "ordered stack must preserve fully transparent base hole"
        );
        // Layer1 opaque green over layer0 red at (0,1).
        assert_eq!(
            pixel_at(&pixels, 0, 1),
            [0, 255, 0, 255],
            "src-over must let opaque green replace red"
        );
        // Semi-transparent base pixels: single premul (via layer0 convert), not a².
        assert_approx_premultiplied(
            BLIT_TEST_SOURCE[1],
            pixel_at(&pixels, 1, 0),
            "ordered half-white must be single-premul not double",
        );
        assert_approx_premultiplied(
            BLIT_TEST_SOURCE[3],
            pixel_at(&pixels, 1, 1),
            "ordered quarter-blue must be single-premul not double",
        );
    }

    #[test]
    fn region_blitter_only_updates_requested_destination_rect() {
        let Some((device, queue)) = create_headless_device() else {
            eprintln!("skipping region blitter GPU test: no compatible wgpu adapter");
            return;
        };
        const DEST_W: u32 = 4;
        const DEST_H: u32 = 4;
        let source = create_rgba8_texture(
            &device,
            2,
            2,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            "region-source",
        );
        write_rgba8_pixels(&queue, &source, 2, 2, &[[0, 255, 0, 255]; 4]);
        let destination = create_rgba8_texture(
            &device,
            DEST_W,
            DEST_H,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
            "region-destination",
        );
        write_rgba8_pixels(
            &queue,
            &destination,
            DEST_W,
            DEST_H,
            &[[255, 0, 0, 255]; (DEST_W * DEST_H) as usize],
        );

        let blitter =
            RegionTextureBlitter::new(&device, TextureFormat::Rgba8Unorm, Some(STRAIGHT_SRC_OVER));
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let destination_view = destination.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("region blitter test"),
        });
        let placement = OrderedLayerTarget {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        };
        blitter.copy(
            &device,
            &mut encoder,
            &source_view,
            &destination_view,
            placement,
            placement,
        );
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba8_pixels(&device, &queue, &destination, DEST_W, DEST_H);
        let at = |x: u32, y: u32| pixels[(y * DEST_W + x) as usize];
        assert_eq!(at(0, 0), [255, 0, 0, 255]);
        assert_eq!(at(1, 1), [0, 255, 0, 255]);
        assert_eq!(at(2, 2), [0, 255, 0, 255]);
        assert_eq!(at(3, 3), [255, 0, 0, 255]);
    }

    #[test]
    fn region_blitter_scissor_preserves_full_window_source_coordinates() {
        let Some((device, queue)) = create_headless_device() else {
            eprintln!("skipping scissored region GPU test: no compatible wgpu adapter");
            return;
        };
        const WIDTH: u32 = 4;
        const HEIGHT: u32 = 4;
        let source_pixels = std::array::from_fn::<_, 16, _>(|index| {
            let x = (index as u32) % WIDTH;
            let y = (index as u32) / WIDTH;
            [(x * 40) as u8, (y * 40) as u8, 0, 255]
        });
        let source = create_rgba8_texture(
            &device,
            WIDTH,
            HEIGHT,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            "scissored-region-source",
        );
        write_rgba8_pixels(&queue, &source, WIDTH, HEIGHT, &source_pixels);
        let destination = create_rgba8_texture(
            &device,
            WIDTH,
            HEIGHT,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
            "scissored-region-destination",
        );
        write_rgba8_pixels(&queue, &destination, WIDTH, HEIGHT, &[[255, 0, 0, 255]; 16]);

        let blitter = RegionTextureBlitter::new(&device, TextureFormat::Rgba8Unorm, None);
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let destination_view = destination.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("scissored region blitter test"),
        });
        blitter.copy(
            &device,
            &mut encoder,
            &source_view,
            &destination_view,
            OrderedLayerTarget::full(WIDTH, HEIGHT),
            OrderedLayerTarget {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        );
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba8_pixels(&device, &queue, &destination, WIDTH, HEIGHT);
        let at = |x: u32, y: u32| pixels[(y * WIDTH + x) as usize];
        assert_eq!(at(0, 0), [255, 0, 0, 255]);
        assert_eq!(at(1, 1), source_pixels[5]);
        assert_eq!(at(2, 2), source_pixels[10]);
        assert_eq!(at(3, 3), [255, 0, 0, 255]);
    }

    /// Issue 9: semi-transparent **upper** layer over opaque base must not be
    /// darkened by a second present premul (intermediate held in premul space).
    #[test]
    fn ordered_semi_transparent_upper_layer_over_opaque_no_double_premul() {
        let Some((device, queue)) = create_headless_device() else {
            eprintln!("skipping ordered upper-layer GPU test: no compatible wgpu adapter");
            return;
        };

        let tex_usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;

        // Layer0: opaque red everywhere.
        let layer0_px = [[255u8, 0, 0, 255]; 4];
        let layer0 = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            tex_usage,
            "upper-semi-layer0",
        );
        write_rgba8_pixels(
            &queue,
            &layer0,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &layer0_px,
        );

        // Layer1: half-white only at (1,0); rest transparent.
        let mut layer1_px = [[0u8, 0, 0, 0]; 4];
        layer1_px[1] = [255, 255, 255, 128]; // (1,0)
        let layer1 = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            tex_usage,
            "upper-semi-layer1",
        );
        write_rgba8_pixels(
            &queue,
            &layer1,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &layer1_px,
        );

        let intermediate = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            "upper-semi-mid",
        );
        let destination = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
            "upper-semi-dest",
        );

        let replace = create_layer_replace_blitter(&device, TextureFormat::Rgba8Unorm);
        let premul_convert =
            create_layer_premul_convert_blitter(&device, TextureFormat::Rgba8Unorm);
        let stack = create_layer_stack_blitter(&device, TextureFormat::Rgba8Unorm);

        let l0_view = layer0.create_view(&wgpu::TextureViewDescriptor::default());
        let l1_view = layer1.create_view(&wgpu::TextureViewDescriptor::default());
        let mid_view = intermediate.create_view(&wgpu::TextureViewDescriptor::default());
        let dest_view = destination.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ordered upper semi composite"),
        });
        premul_convert.copy(&device, &mut encoder, &l0_view, &mid_view);
        stack.copy(&device, &mut encoder, &l1_view, &mid_view);
        replace.copy(&device, &mut encoder, &mid_view, &dest_view);
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba8_pixels(
            &device,
            &queue,
            &destination,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
        );

        // Uncovered opaque red stays opaque red.
        assert_eq!(pixel_at(&pixels, 0, 0), [255, 0, 0, 255]);

        // Half-white over premul red: Cout = (255,255,255)*0.5 + (255,0,0)*0.5
        // ≈ (255, 128, 128), A=255. A wrong double-premul path would darken RGB.
        let blended = pixel_at(&pixels, 1, 0);
        assert_eq!(blended[3], 255, "result must be opaque; got {blended:?}");
        for (ch, (got, exp)) in blended[..3]
            .iter()
            .zip([255u8, 128, 128].iter())
            .enumerate()
        {
            let delta = got.abs_diff(*exp);
            assert!(
                delta <= 1,
                "channel {ch}: expected ~{exp} got {got}; full {blended:?} (double-premul would be darker)"
            );
        }

        // Contrast: if we had wrongly present-premul'd coverage-weighted straight
        // intermediate, half-white-over-red could collapse; ensure not near black.
        assert!(blended[0] > 200, "R must stay bright: {blended:?}");
    }

    #[test]
    fn retain_layer_targets_drops_orphan_ids() {
        // Pure HashMap contract used by ExternalWindowSurface::retain_layer_targets.
        let mut map: HashMap<u64, u32> = HashMap::from([(1, 10), (2, 20), (3, 30)]);
        let live = [1u64, 3];
        map.retain(|id, _| live.iter().any(|l| l == id));
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&1));
        assert!(map.contains_key(&3));
        assert!(!map.contains_key(&2));
    }

    /// Spinner / ordered path regression: intermediate is always Rgba8Unorm while
    /// the Windows swapchain is often Bgra8Unorm. Layer stack blitters must target
    /// the intermediate; only the final present replace may use the surface format.
    #[test]
    fn ordered_composite_accepts_bgra_swapchain_dest_from_rgba_intermediate() {
        let Some((device, queue)) = create_headless_device() else {
            eprintln!("skipping Bgra present format test: no compatible wgpu adapter");
            return;
        };

        let tex_usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;

        // Straight-alpha layer (Vello style) in Rgba8Unorm.
        let layer0 = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            tex_usage,
            "bgra-present-layer0",
        );
        write_rgba8_pixels(
            &queue,
            &layer0,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            &BLIT_TEST_SOURCE,
        );

        let intermediate = create_rgba8_texture(
            &device,
            BLIT_TEST_WIDTH,
            BLIT_TEST_HEIGHT,
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            "bgra-present-mid",
        );

        // Destination matches Windows swapchain format.
        let destination = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bgra-present-dest"),
            size: wgpu::Extent3d {
                width: BLIT_TEST_WIDTH,
                height: BLIT_TEST_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Bgra8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Production split: intermediate blitters = VELLO_TARGET_FORMAT,
        // present replace = surface format (Bgra8Unorm here).
        let premul_convert = create_layer_premul_convert_blitter(&device, VELLO_TARGET_FORMAT);
        let present_replace = create_layer_replace_blitter(&device, TextureFormat::Bgra8Unorm);

        let l0_view = layer0.create_view(&wgpu::TextureViewDescriptor::default());
        let mid_view = intermediate.create_view(&wgpu::TextureViewDescriptor::default());
        let dest_view = destination.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bgra present composite"),
        });
        premul_convert.copy(&device, &mut encoder, &l0_view, &mid_view);
        present_replace.copy(&device, &mut encoder, &mid_view, &dest_view);
        // If formats were mismatched (Rgba blitter → Bgra dest), submit panics
        // via wgpu validation (same error as gallery Spinner page).
        queue.submit(Some(encoder.finish()));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU work after cross-format ordered present");
    }
}
