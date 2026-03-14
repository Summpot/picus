#![expect(
    unsafe_code,
    reason = "Creating a persistent wgpu surface from Bevy's raw window handles requires raw-window-handle unsafe entry points."
)]

use bevy_window::RawHandleWrapper;
use vello::{
    AaConfig, AaSupport, Error, RenderParams, Renderer, RendererOptions, Scene,
    peniko::Color,
    wgpu::{
        self, CompositeAlphaMode, Device, Instance, MemoryBudgetThresholds, MemoryHints,
        PresentMode, Surface, SurfaceConfiguration, Texture, TextureFormat, TextureUsages,
        TextureView,
    },
};
use wgpu::util::{TextureBlitter, TextureBlitterBuilder};

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
}

/// A Vello surface context attached to an externally owned Bevy window.
pub struct ExternalWindowSurface {
    render_cx: RenderContext,
    surface: RenderSurface<'static>,
    scale_factor: f64,
}

impl ExternalWindowSurface {
    /// Create an attached Vello surface from a Bevy-owned raw-handle wrapper.
    pub fn new_from_bevy_raw_handle(
        raw_handle: RawHandleWrapper,
        metrics: ExistingWindowMetrics,
        present_mode: PresentMode,
    ) -> Result<Self, Error> {
        // SAFETY: The caller provides a `RawHandleWrapper` originating from Bevy's
        // `WindowWrapper`, which internally keeps an owning reference to the window alive.
        // We create a thread-locked handle target only for surface initialization.
        let target = unsafe { raw_handle.get_handle() };
        let mut render_cx = RenderContext::new();
        let surface = pollster::block_on(render_cx.create_surface(
            target,
            metrics.physical_width.max(1),
            metrics.physical_height.max(1),
            present_mode,
        ))?;

        Ok(Self {
            render_cx,
            surface,
            scale_factor: metrics.scale_factor,
        })
    }

    /// Synchronize internal surface size and scale-factor from the attached window.
    pub fn sync_window_metrics(&mut self, metrics: ExistingWindowMetrics) {
        self.scale_factor = metrics.scale_factor;

        if self.surface.config.width != metrics.physical_width
            || self.surface.config.height != metrics.physical_height
        {
            self.render_cx.resize_surface(
                &mut self.surface,
                metrics.physical_width.max(1),
                metrics.physical_height.max(1),
            );
        }
    }

    /// Render a Masonry/Vello scene and present it to the attached window surface.
    pub fn render_scene(
        &mut self,
        renderer: &mut Option<Renderer>,
        scene: Scene,
        logical_width: u32,
        logical_height: u32,
        base_color: Color,
    ) {
        let transformed_scene = if self.scale_factor == 1.0 {
            None
        } else {
            let mut scaled = Scene::new();
            scaled.append(&scene, Some(vello::kurbo::Affine::scale(self.scale_factor)));
            Some(scaled)
        };
        let scene_ref = transformed_scene.as_ref().unwrap_or(&scene);

        let dev_id = self.surface.dev_id;
        let device = &self.render_cx.devices[dev_id].device;
        let queue = &self.render_cx.devices[dev_id].queue;

        let renderer = renderer.get_or_insert_with(|| {
            Renderer::new(
                device,
                RendererOptions {
                    antialiasing_support: AaSupport::area_only(),
                    ..Default::default()
                },
            )
            .expect("failed to create Vello renderer")
        });

        let render_params = RenderParams {
            base_color,
            width: logical_width.max(1),
            height: logical_height.max(1),
            antialiasing_method: AaConfig::Area,
        };

        let surface_texture = match self.surface.surface.get_current_texture() {
            Ok(texture) => texture,
            Err(wgpu::SurfaceError::Outdated) => {
                let current_width = self.surface.config.width.max(1);
                let current_height = self.surface.config.height.max(1);
                self.render_cx
                    .resize_surface(&mut self.surface, current_width, current_height);

                match self.surface.surface.get_current_texture() {
                    Ok(texture) => texture,
                    Err(error) => {
                        tracing::error!(
                            "Couldn't get swap chain texture after configuring. Cause: '{error}'"
                        );
                        return;
                    }
                }
            }
            Err(error) => {
                tracing::error!(
                    "Couldn't get swap chain texture, operation unrecoverable: {error}"
                );
                return;
            }
        };

        if let Err(error) = renderer.render_to_texture(
            device,
            queue,
            scene_ref,
            &self.surface.target_view,
            &render_params,
        ) {
            tracing::error!("failed to render scene to texture: {error}");
            return;
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("External Window Surface Blit"),
        });
        self.surface.blitter.copy(
            device,
            &mut encoder,
            &self.surface.target_view,
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        queue.submit([encoder.finish()]);
        surface_texture.present();

        if let Err(error) = device.poll(wgpu::PollType::wait_indefinitely()) {
            tracing::error!("error while waiting for GPU completion: {error}");
        }
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
    fn new() -> Self {
        let backends = wgpu::Backends::from_env().unwrap_or_default();
        let flags = wgpu::InstanceFlags::from_build_config().with_env();
        let backend_options = wgpu::BackendOptions::from_env_or_default();
        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends,
            flags,
            backend_options,
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
        });

        Self {
            instance,
            devices: Vec::new(),
        }
    }

    async fn create_surface<'w>(
        &mut self,
        window: impl Into<wgpu::SurfaceTarget<'w>>,
        width: u32,
        height: u32,
        present_mode: PresentMode,
    ) -> Result<RenderSurface<'w>, Error> {
        self.create_render_surface(
            self.instance.create_surface(window.into())?,
            width,
            height,
            present_mode,
        )
        .await
    }

    async fn create_render_surface<'w>(
        &mut self,
        surface: Surface<'w>,
        width: u32,
        height: u32,
        present_mode: PresentMode,
    ) -> Result<RenderSurface<'w>, Error> {
        let dev_id = self
            .device(Some(&surface))
            .await
            .ok_or(Error::NoCompatibleDevice)?;

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
            .ok_or(Error::UnsupportedSurfaceFormat)?;

        const PREMUL_BLEND_STATE: wgpu::BlendState = wgpu::BlendState {
            alpha: wgpu::BlendComponent::REPLACE,
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let (alpha_mode, blitter) = if capabilities
            .alpha_modes
            .contains(&CompositeAlphaMode::PostMultiplied)
        {
            (
                CompositeAlphaMode::PostMultiplied,
                TextureBlitter::new(&device_handle.device, format),
            )
        } else if capabilities
            .alpha_modes
            .contains(&CompositeAlphaMode::PreMultiplied)
        {
            (
                CompositeAlphaMode::PreMultiplied,
                TextureBlitterBuilder::new(&device_handle.device, format)
                    .blend_state(PREMUL_BLEND_STATE)
                    .build(),
            )
        } else {
            let texture_blitter =
                if cfg!(windows) && device_handle.adapter.get_info().name.contains("AMD") {
                    tracing::info!(
                        "on Windows with AMD GPUs use premultiplied blitting even on opaque surface"
                    );
                    TextureBlitterBuilder::new(&device_handle.device, format)
                        .blend_state(PREMUL_BLEND_STATE)
                        .build()
                } else {
                    TextureBlitter::new(&device_handle.device, format)
                };
            (CompositeAlphaMode::Auto, texture_blitter)
        };

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        let (target_texture, target_view) = create_targets(width, height, &device_handle.device);

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
        let (texture, view) = create_targets(width, height, &self.devices[surface.dev_id].device);
        surface.target_texture = texture;
        surface.target_view = view;
        surface.config.width = width;
        surface.config.height = height;
        self.configure_surface(surface);
    }

    fn configure_surface(&self, surface: &RenderSurface<'_>) {
        let device = &self.devices[surface.dev_id].device;
        surface.surface.configure(device, &surface.config);
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

fn create_targets(width: u32, height: u32, device: &Device) -> (Texture, TextureView) {
    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
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
    let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (target_texture, target_view)
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
