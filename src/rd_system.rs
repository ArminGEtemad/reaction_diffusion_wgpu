use bytemuck::{Pod, Zeroable};
use std::{fs, num::NonZeroU64, path::PathBuf, time::Instant};

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::gpu_resources::{FrameContext, GpuResource};

pub struct SystemConfig {
    pub width: u32,
    pub height: u32,
}

const WG_X: u32 = 16;
const WG_Y: u32 = 16;

// helper function to have a dynamical shader address
// so the source is not "hard coded" in the compile time
pub fn load_ablsolute_path(relative_path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path); // making absolute path
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read shader {:?}\nError: {}", path, e))
}

// helper function to make the initial blob or make a clean sheet
pub fn write_pattern_to_starting_space(
    gpu_res: &GpuResource,
    texture_1: &Texture,
    texture_2: &Texture,
    pattern: StartingPattern,
    width: u32,
    height: u32,
) {
    let mut data = vec![0.0; (width * height * 4) as usize]; // all pixels with all RGBA elements 0.0

    match pattern {
        StartingPattern::Circle => {
            // loop over all the pixels
            for y in 0..height {
                for x in 0..width {
                    let pixel_idx = ((y * width + x) * 4) as usize;

                    // element U everywhere
                    // element V only in blob
                    let u = 1.0_f32;
                    let mut v = 0.0_f32;

                    // blob in the center for element V
                    let center_x = width as i32 / 2;
                    let center_y = height as i32 / 2;

                    let dist_x = x as i32 - center_x;
                    let dist_y = y as i32 - center_y;

                    // TODO the size of the circle can be chosen by the user
                    if dist_x.abs() * dist_x.abs() + dist_y.abs() * dist_y.abs() < 100 {
                        // TODO check out standard initializations
                        v = 1.0; // add element V to the area
                    }

                    // write the data to the channels
                    data[pixel_idx + 0] = u;
                    data[pixel_idx + 1] = v;
                    data[pixel_idx + 2] = 0.0;
                    data[pixel_idx + 3] = 1.0;
                }
            }
        }

        StartingPattern::Square => {
            // loop over all the pixels
            for y in 0..height {
                for x in 0..width {
                    let pixel_idx = ((y * width + x) * 4) as usize;

                    // element U everywhere
                    // element V only in blob
                    let u = 1.0_f32;
                    let mut v = 0.0_f32;

                    // blob in the center for element V
                    let center_x = width as i32 / 2;
                    let center_y = height as i32 / 2;

                    let dist_x = x as i32 - center_x;
                    let dist_y = y as i32 - center_y;

                    if dist_x.abs() < 10 && dist_y.abs() < 10 {
                        // TODO check out standard initializations
                        v = 1.0; // add element V to the area
                    }

                    // write the data to the channels
                    data[pixel_idx + 0] = u;
                    data[pixel_idx + 1] = v;
                    data[pixel_idx + 2] = 0.0;
                    data[pixel_idx + 3] = 1.0;
                }
            }
        }
        StartingPattern::CleanSheet => {}
    }

    let data_bytes: &[u8] = bytemuck::cast_slice(&data);
    let layout = TexelCopyBufferLayout {
        offset: 0,
        // RGBA32Float = 4 channel * 4 byte per pixel
        bytes_per_row: Some(4 * 4 * width),
        rows_per_image: Some(height),
    };

    let extent = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    // queue source 1
    gpu_res.queue.write_texture(
        TexelCopyTextureInfo {
            texture: texture_1,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        data_bytes,
        layout,
        extent,
    );
    // queue source 2
    gpu_res.queue.write_texture(
        TexelCopyTextureInfo {
            texture: texture_2,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        data_bytes,
        layout,
        extent,
    );
}

// time
// this lives in group 0 binding 0 (RD shader)
#[repr(C)] // format expected by the gpu
#[derive(Clone, Copy, Pod, Zeroable)]
struct TimeUniform {
    // 16 byte alignment needed
    dt: f32,        // 4 byte
    _pad: [f32; 3], // 12 byte
}

// Brush
// this lives in group 0 binding 0 (brush shader)
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BrushUniform {
    pub c_x: f32,    // 4 byte
    pub c_y: f32,    // 4 byte
    pub radius: f32, // 4 byte
    pub mode: u32,   // 4 byte
}

// different starting patterns
#[derive(Clone, Copy, Debug)]
pub enum StartingPattern {
    Circle,
    Square,
    CleanSheet,
}

// Communication between the system and GPU
pub struct ReactionDiffusionSystem {
    // config
    pub sys_config: SystemConfig,

    // uniform
    pub time_buffer: Buffer,
    pub _start_instant: Instant,
    pub _last_time: f32,

    // brush
    pub brush_buffer: Buffer,
    pub brush_bgl: BindGroupLayout,
    pub brush_pipeline: ComputePipeline,

    // compute
    pub compute_bgl: BindGroupLayout,
    pub compute_pipeline: ComputePipeline,

    // ping or pong :)
    pub use_ping_as_source: bool,
}

impl ReactionDiffusionSystem {
    pub fn new(gpu_res: &GpuResource, sys_config: SystemConfig) -> Self {
        // importing resources
        let device_m = &gpu_res.device;

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

        // initializing brush
        // TODO where do I initialize what could be important
        let brush_uniform = BrushUniform {
            c_x: 0.0,
            c_y: 0.0,
            radius: 0.0,
            mode: 0,
        };

        let brush_buffer = device_m.create_buffer_init(&BufferInitDescriptor {
            label: Some("Brush Uniform Buffer"),
            contents: bytemuck::bytes_of(&brush_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // shader modules

        // a run time shader loader instead of compile time which makes the program ready for hot reload
        let brush_shader_path = load_ablsolute_path("shaders/brush_compute.wgsl");
        let compute_shader_path = load_ablsolute_path("shaders/rd_compute.wgsl");

        let brush_shader = device_m.create_shader_module(ShaderModuleDescriptor {
            label: Some("Brush Shader Module"),
            source: ShaderSource::Wgsl(brush_shader_path.into()),
        });
        let compute_shader = device_m.create_shader_module(ShaderModuleDescriptor {
            label: Some("Compute Shader Module"),
            source: ShaderSource::Wgsl(compute_shader_path.into()),
        });

        // brush compute
        let brush_bgl = device_m.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Brush Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    // brush uniform
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            std::mem::size_of::<BrushUniform>() as u64
                        ),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    // texture
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadWrite,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let brush_pipeline_layout = device_m.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Brush Pipeline Layout"),
            bind_group_layouts: &[&brush_bgl],
            push_constant_ranges: &[],
        });

        let brush_pipeline = device_m.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Brush Pipeline"),
            layout: Some(&brush_pipeline_layout),
            module: &brush_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        // RD compute
        let compute_bgl =
            device_m.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        // time uniform buffer binding 0
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(
                                std::mem::size_of::<TimeUniform>() as u64
                            ),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        // source (sampled)
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: false },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        // dst (storage)
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::WriteOnly,
                            format: TextureFormat::Rgba32Float,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
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

        let self_package = Self {
            sys_config,

            time_buffer,
            _start_instant: start_instant,
            _last_time: last_time,

            brush_buffer,
            brush_bgl,

            brush_pipeline,

            compute_bgl,
            compute_pipeline,

            use_ping_as_source: true,
        };

        // return
        self_package
    }

    pub fn set_brush_parameters(&self, gpu_res: &GpuResource, brush_uniform: &BrushUniform) {
        gpu_res
            .queue
            .write_buffer(&self.brush_buffer, 0, bytemuck::bytes_of(brush_uniform));
    }

    pub fn step_simulation(
        &mut self,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        paused: bool,
        ping_view: &TextureView,
        pong_view: &TextureView,
    ) {
        let device = &gpu_res.device;
        // return instead of if statement actually
        if paused {
            return;
        }
        // get the size for dispatch
        let (width, height) = self.rd_size();

        // find out the source and destination ping or pong
        let (brush_target, compute_source, compute_destination) = if self.use_ping_as_source {
            (ping_view, ping_view, pong_view)
        } else {
            (pong_view, pong_view, ping_view)
        };

        // Brush injection pass
        {
            // TODO maybe cache it? it looks a bit work for CPU to make a BG every time
            let brush_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Brush Bind Group"),
                layout: &self.brush_bgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.brush_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&brush_target),
                    },
                ],
            });

            let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Brush Compute Pass"),
                timestamp_writes: None,
            });

            cpass.set_pipeline(&self.brush_pipeline);
            cpass.set_bind_group(0, &brush_bg, &[]);

            let workgroup_x = (width + WG_X - 1) / WG_X;
            let workgroup_y = (height + WG_Y - 1) / WG_Y;
            cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        }

        // time
        let dt = 0.7;
        let time_uniform = TimeUniform { dt, _pad: [0.0; 3] };

        gpu_res
            .queue
            .write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&time_uniform));

        // compute pass scope
        {
            // TODO I think I should cache it
            let compute_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Compute Bind Group"),
                layout: &self.compute_bgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.time_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&compute_source),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(&compute_destination),
                    },
                ],
            });
            let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, &compute_bg, &[]);

            let workgroup_x = (width + WG_X - 1) / WG_X;
            let workgroup_y = (height + WG_Y - 1) / WG_Y;
            cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        }

        self.use_ping_as_source = !self.use_ping_as_source;
    }

    // reload and rebuild pipelines if shaders are changed
    // TODO This makes this script too long. Should I refactor it or make a script for it?
    fn reload_compute_pipeline(&mut self, gpu_res: &GpuResource) {
        let compute_shader_path = load_ablsolute_path("shaders/rd_compute.wgsl");
        let compute_shader = gpu_res.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Compute Shader (Rebuilding)"),
            source: ShaderSource::Wgsl(compute_shader_path.into()),
        });

        // "new layout" it is the same same but different (after changes in the shader)
        let compute_pipeline_layout =
            gpu_res
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Compute Pipeline Layout (Rebuilding"),
                    bind_group_layouts: &[&self.compute_bgl],
                    push_constant_ranges: &[],
                });

        self.compute_pipeline =
            gpu_res
                .device
                .create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some("Compute Pipeline (Rebuilding)"),
                    layout: Some(&compute_pipeline_layout),
                    module: &compute_shader,
                    entry_point: Some("main"),
                    compilation_options: PipelineCompilationOptions::default(),
                    cache: None,
                });
    }

    // rebuild
    pub fn rebuild_pipeline(&mut self, gpu_res: &GpuResource) {
        println!("Rebuilding Compute Pipelines (Hot Reload)");
        self.reload_compute_pipeline(gpu_res);
        println!("Compute Pipelines Reloaded (Hot Reload)");
    }

    // reset function
    pub fn reset_time(&mut self) {
        self._start_instant = Instant::now();
        self._last_time = 0.0;
    }

    // a helper function for WIDTH and WEIGHT
    pub fn rd_size(&self) -> (u32, u32) {
        (self.sys_config.width, self.sys_config.height)
    }

    pub fn is_ping_source(&self) -> bool {
        self.use_ping_as_source
    }
}
