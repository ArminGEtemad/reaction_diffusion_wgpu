use wgpu::*;
use winit::{dpi::PhysicalSize, window::Window};

pub struct GpuResource {
    // includes communication steps with GPU
    // making the window, openning connection to GPU
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
}

pub struct FrameContext {
    pub surface_texture: SurfaceTexture,
    pub view: TextureView,
    pub encoder: CommandEncoder,
}

impl GpuResource {
    pub async fn new(window: &'static Window) -> Result<Self, String> {
        let size = window.inner_size();

        // making the instance (calling it with _m at the end so it is not)
        // confused with instance in wgpu. Keeping _m for the same reason everywhere
        let instance_m = Instance::new(&InstanceDescriptor::default());
        let surface_m = instance_m.create_surface(window).unwrap();

        let adapter_m = instance_m
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface_m),
                ..Default::default()
            })
            .await
            .expect("No GPU found!");

        let (device_m, queue_m) = adapter_m
            .request_device(&wgt::DeviceDescriptor {
                label: Some("Device"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
                memory_hints: MemoryHints::default(),
                trace: Trace::Off,
            })
            .await
            .expect("Failed to create device!");

        let surface_m_capab = surface_m.get_capabilities(&adapter_m); // needed for format
        // needed for configuration later
        let surfacr_m_format = surface_m_capab
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    TextureFormat::Bgra8UnormSrgb | TextureFormat::Rgba8UnormSrgb
                )
            })
            .unwrap_or(surface_m_capab.formats[0]);

        let config_m = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surfacr_m_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: PresentMode::Fifo, // first in first out
            // frame delay between when a frame is recorded and when it is displayed.
            // default is 2
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_m_capab.alpha_modes[0],
            view_formats: vec![],
        };

        surface_m.configure(&device_m, &config_m);

        Ok(Self {
            surface: surface_m,
            device: device_m,
            queue: queue_m,
            config: config_m,
            size,
        })
    }

    // resizing the window must be communicated with GPU
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn begin_frame(&self) -> Result<FrameContext, SurfaceError> {
        let surface_texture = self.surface.get_current_texture()?;
        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Encoder"),
            });

        Ok(FrameContext {
            surface_texture,
            view,
            encoder,
        })
    }

    pub fn submit_frame(&self, frame: FrameContext) {
        self.queue.submit([frame.encoder.finish()]);
        frame.surface_texture.present();
    }
}
