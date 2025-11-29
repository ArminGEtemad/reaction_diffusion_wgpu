use std::{num::NonZeroU64, time::Instant};

use bytemuck::{Pod, Zeroable};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    wgt::CommandEncoderDescriptor,
    *,
};
use winit::{dpi::PhysicalSize, window::Window};

// time
#[repr(C)] // format expected by the gpu
#[derive(Clone, Copy, Pod, Zeroable)]
struct TimeUniform {
    // 16 byte alignment needed
    dt: f32,        // 4 byte
    _pad: [f32; 3], // 12 byte
}
// connection to gpu
pub struct State {
    // includes communication steps with GPU
    // making the window, openning connection to GPU
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub size: PhysicalSize<u32>,

    // uniform
    pub time_buffer: Buffer,
    pub start_instant: Instant,
    pub last_time: f32,

    // compute
    pub compute_bgl: BindGroupLayout,
    pub compute_bg: BindGroup,
    pub compute_pipeline: ComputePipeline,
}

impl State {
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

        // time uniform buffer
        let time_uniform = TimeUniform {
            dt: 0.0,
            _pad: [0.0; 3],
        };

        let time_buffer = device_m.create_buffer_init(&BufferInitDescriptor {
            label: Some("Time Uniform Buffer"),
            contents: bytemuck::bytes_of(&time_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let start_instant = Instant::now();
        let last_time: f32 = 0.0;

        // shader modules
        let compute_shader = device_m.create_shader_module(ShaderModuleDescriptor {
            label: Some("Compute Shder Module"),
            source: ShaderSource::Wgsl(include_str!("../shaders/rd_compute.wgsl").into()),
        });

        // compute
        let compute_bgl = device_m.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Compute Bing Group Layout"),
            entries: &[BindGroupLayoutEntry {
                // time uniform buffer binding 0
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<TimeUniform>() as u64),
                },
                count: None,
            }],
        });

        let compute_bg = device_m.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &compute_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: time_buffer.as_entire_binding(),
            }],
        });

        let compute_pipeline_layout = device_m.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[&compute_bgl],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device_m.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(Self {
            surface: surface_m,
            device: device_m,
            queue: queue_m,
            config: config_m,
            size,

            time_buffer,
            start_instant,
            last_time,

            compute_bgl,
            compute_bg,
            compute_pipeline,
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

    pub fn compute_and_render(&mut self) -> Result<(), SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Encoder"),
            });

        // update dt
        let now = self.start_instant.elapsed().as_secs_f32();
        let dt = (now - self.last_time).max(0.0);
        self.last_time = now;

        let time_uniform = TimeUniform { dt, _pad: [0.0; 3] };

        self.queue
            .write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&time_uniform));

        // compute pass scope
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, &self.compute_bg, &[]);

            let workgroup = (256 + 255) / 256; // TODO (WG + (N - 1) / N) Need to be changed later accordigly
            cpass.dispatch_workgroups(workgroup, 1, 1);
        }

        // render pass scope
        {
            let _rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLUE),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        self.queue.submit([encoder.finish()]);
        frame.present();
        Ok(())
    }
}
