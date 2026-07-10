#![expect(
    unsafe_code,
    reason = "Creating a persistent wgpu surface and applying native window backdrops requires raw window handles and Win32 calls."
)]

use bevy_window::{CompositeAlphaMode as BevyCompositeAlphaMode, RawHandleWrapper};
use masonry_imaging::{
    PreparedFrame,
    texture_render::{RenderTarget, Renderer},
};
use wgpu::util::{TextureBlitter, TextureBlitterBuilder};
use wgpu::{
    CompositeAlphaMode, Device, Dx12SwapchainKind, Instance, MemoryBudgetThresholds, MemoryHints,
    PresentMode, Surface, SurfaceConfiguration, SurfaceTexture, Texture, TextureFormat,
    TextureUsages, TextureView,
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
            Self::UnsupportedPlatform => write!(f, "native window backdrops are unsupported on this platform"),
            Self::UnsupportedWindowHandle => write!(f, "native window backdrop requires a Win32 window handle"),
            Self::WindowsHresult(hr) => write!(f, "DwmSetWindowAttribute failed with HRESULT {hr:#010x}"),
        }
    }
}

impl std::error::Error for NativeWindowBackdropError {}

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
        DWM_SYSTEMBACKDROP_TYPE, DWMSBT_AUTO, DWMSBT_MAINWINDOW, DWMSBT_NONE,
        DWMSBT_TABBEDWINDOW, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
    };

    let hwnd = match raw_handle.get_window_handle() {
        RawWindowHandle::Win32(handle) => handle.hwnd.get() as windows_sys::Win32::Foundation::HWND,
        _ => return Err(NativeWindowBackdropError::UnsupportedWindowHandle),
    };

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
        Err(NativeWindowBackdropError::WindowsHresult(hr))
    } else {
        Ok(())
    }
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

/// A Vello surface context attached to an externally owned Bevy window.
pub struct ExternalWindowSurface {
    render_cx: RenderContext,
    surface: RenderSurface<'static>,
    scale_factor: f64,
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

impl ExternalWindowSurface {
    /// Create an attached Vello surface from a Bevy-owned raw-handle wrapper.
    pub fn new_from_bevy_raw_handle(
        raw_handle: RawHandleWrapper,
        metrics: ExistingWindowMetrics,
        present_mode: PresentMode,
    ) -> Result<Self, RenderSurfaceError> {
        // SAFETY: The caller provides a `RawHandleWrapper` originating from Bevy's
        // `WindowWrapper`, which internally keeps an owning reference to the window alive.
        // We create a thread-locked handle target only for surface initialization.
        let target = unsafe { raw_handle.get_handle() };
        let mut render_cx = RenderContext::new(metrics.transparent);
        let surface = pollster::block_on(render_cx.create_surface(
            target,
            metrics,
            present_mode,
        ))?;

        Ok(Self {
            render_cx,
            surface,
            scale_factor: metrics.scale_factor,
        })
    }

    /// Synchronize internal surface size and scale-factor from the attached window.
    ///
    /// Returns `true` when the backing surface textures were resized and the
    /// caller should schedule a fresh paint.
    pub fn sync_window_metrics(&mut self, metrics: ExistingWindowMetrics) -> bool {
        self.scale_factor = metrics.scale_factor;
        let mut changed = false;

        if self.surface.config.width != metrics.physical_width
            || self.surface.config.height != metrics.physical_height
        {
            self.render_cx.resize_surface(
                &mut self.surface,
                metrics.physical_width.max(1),
                metrics.physical_height.max(1),
            );
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

    /// Render a prepared Masonry frame and present it to the attached window surface.
    #[must_use]
    pub fn render_frame(
        &mut self,
        renderer: &mut Renderer,
        frame: PreparedFrame<'_>,
    ) -> RenderFrameResult {
        let dev_id = self.surface.dev_id;
        let adapter = &self.render_cx.devices[dev_id].adapter;
        let device = &self.render_cx.devices[dev_id].device;
        let queue = &self.render_cx.devices[dev_id].queue;

        let mut did_reconfigure = false;
        let surface_texture = loop {
            match get_current_surface_texture(&self.surface.surface, device) {
                Ok(texture) if texture.suboptimal => {
                    discard_surface_texture(device, texture);
                    self.render_cx.configure_surface(&self.surface);
                    tracing::debug!("swap chain texture was suboptimal; surface reconfigured");
                    return RenderFrameResult::Retry;
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
                        return RenderFrameResult::Retry;
                    }
                    SurfaceRecoveryAction::Fail => {
                        tracing::error!("couldn't acquire swap chain texture: {error}");
                        return RenderFrameResult::Failed;
                    }
                },
            }
        };

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
            return RenderFrameResult::Failed;
        }

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
            return RenderFrameResult::Retry;
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
            return RenderFrameResult::Retry;
        }

        let ((), present_errors) =
            capture_device_errors(device, || surface_texture.present());
        if !present_errors.is_empty() {
            log_device_errors("presenting the swap chain texture", present_errors);
            self.render_cx.configure_surface(&self.surface);
            return RenderFrameResult::Retry;
        }

        if let Err(error) = device.poll(wgpu::PollType::Poll) {
            tracing::trace!("non-blocking GPU poll after present returned: {error}");
        }

        RenderFrameResult::Presented
    }
}

struct RenderContext {
    instance: Instance,
    /// Created devices used by this context.
    devices: Vec<DeviceHandle>,
}

struct DeviceHandle {
    adapter: wgpu::Adapter,
    device: Device,
    queue: wgpu::Queue,
}

impl RenderContext {
    fn new(transparent: bool) -> Self {
        let backends = wgpu::Backends::from_env().unwrap_or_default();
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
        }
    }

    async fn create_surface<'w>(
        &mut self,
        window: impl Into<wgpu::SurfaceTarget<'w>>,
        metrics: ExistingWindowMetrics,
        present_mode: PresentMode,
    ) -> Result<RenderSurface<'w>, RenderSurfaceError> {
        self.create_render_surface(
            self.instance
                .create_surface(window.into())
                .map_err(RenderSurfaceError::CreateSurface)?,
            metrics,
            present_mode,
        )
        .await
    }

    async fn create_render_surface<'w>(
        &mut self,
        surface: Surface<'w>,
        metrics: ExistingWindowMetrics,
        present_mode: PresentMode,
    ) -> Result<RenderSurface<'w>, RenderSurfaceError> {
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

        let adapter_name = device_handle.adapter.get_info().name;
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
            &adapter_name,
        );

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: metrics.physical_width.max(1),
            height: metrics.physical_height.max(1),
            present_mode,
            desired_maximum_frame_latency: 1,
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
        Ok(surface)
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
        let adapter_name = device_handle.adapter.get_info().name;
        surface.blitter = create_blitter(
            &device_handle.device,
            surface.format,
            alpha_mode,
            transparent,
            &adapter_name,
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
        let adapter =
            wgpu::util::initialize_adapter_from_env_or_default(&self.instance, compatible_surface)
                .await
                .ok()?;

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
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        format: TextureFormat::Rgba8Unorm,
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
    adapter_name: &str,
) -> TextureBlitter {
    const PREMUL_BLEND_STATE: wgpu::BlendState = wgpu::BlendState {
        alpha: wgpu::BlendComponent::REPLACE,
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        },
    };

    let needs_premultiplied_blit = needs_premultiplied_blit(
        alpha_mode,
        transparent,
        cfg!(windows),
        adapter_name,
    );
    if needs_premultiplied_blit {
        if cfg!(windows) && adapter_name.contains("AMD") {
            tracing::info!("using premultiplied blitting for Windows AMD compatibility");
        }
        TextureBlitterBuilder::new(device, format)
            .blend_state(PREMUL_BLEND_STATE)
            .build()
    } else {
        TextureBlitter::new(device, format)
    }
}

fn needs_premultiplied_blit(
    alpha_mode: CompositeAlphaMode,
    transparent: bool,
    windows: bool,
    adapter_name: &str,
) -> bool {
    matches!(alpha_mode, CompositeAlphaMode::PreMultiplied)
        || (windows && transparent)
        || (windows
            && adapter_name.contains("AMD")
            && matches!(
                alpha_mode,
                CompositeAlphaMode::Auto | CompositeAlphaMode::Opaque
            ))
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
        wgpu::SurfaceError::Outdated
        | wgpu::SurfaceError::Lost
        | wgpu::SurfaceError::Other => SurfaceRecoveryAction::Reconfigure,
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

    #[cfg(windows)]
    #[test]
    fn transparent_windows_use_direct_composition_visuals() {
        assert_eq!(
            backend_options_for_surface(true).dx12.presentation_system,
            Dx12SwapchainKind::DxgiFromVisual
        );
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
    fn windows_transparent_surfaces_always_premultiply_the_final_blit() {
        assert!(needs_premultiplied_blit(
            CompositeAlphaMode::Auto,
            true,
            true,
            "Generic GPU",
        ));
        assert!(!needs_premultiplied_blit(
            CompositeAlphaMode::Auto,
            true,
            false,
            "Generic GPU",
        ));
    }

    #[test]
    fn explicit_premultiplied_alpha_always_premultiplies_the_final_blit() {
        assert!(needs_premultiplied_blit(
            CompositeAlphaMode::PreMultiplied,
            false,
            false,
            "Generic GPU",
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
        assert_eq!(native_backdrop_ordinal(NativeWindowBackdropMaterial::Auto), 0);
        assert_eq!(native_backdrop_ordinal(NativeWindowBackdropMaterial::None), 1);
        assert_eq!(native_backdrop_ordinal(NativeWindowBackdropMaterial::Mica), 2);
        assert_eq!(
            native_backdrop_ordinal(NativeWindowBackdropMaterial::Acrylic),
            3
        );
        assert_eq!(
            native_backdrop_ordinal(NativeWindowBackdropMaterial::MicaAlt),
            4
        );
    }
}
