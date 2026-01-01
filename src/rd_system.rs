use bytemuck::{Pod, Zeroable};
use std::{fs, num::NonZeroU64, path::PathBuf, time::Instant};

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::{
    gpu_resources::{FrameContext, GpuResource},
    nodes::consts::{WG_X, WG_Y},
};

#[derive(Clone, Debug)]
pub struct SystemConfig {
    pub width: u32,
    pub height: u32,
}

// For better mathematical stability
// we can do N small simulation steps per frame
pub struct SimulationParameters {
    pub dt_per_step: f32,
    pub substeps_per_frame: u32,
}

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
        StartingPattern::CleanSheet => {
            for y in 0..height {
                for x in 0..width {
                    let pixel_idx = ((y * width + x) * 4) as usize;

                    // a clean sheet only out of element U
                    data[pixel_idx + 0] = 1.0;
                    data[pixel_idx + 1] = 0.0;
                    data[pixel_idx + 2] = 0.0;
                    data[pixel_idx + 3] = 1.0;
                }
            }
        }
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

    // parameters
    pub sim_parameters: SimulationParameters,

    // uniform
    pub time_buffer: Buffer,
    pub _start_instant: Instant,
    pub _last_time: f32,

    // compute for predictor and corrector
    pub compute_bgl: BindGroupLayout,
    pub compute_pipeline_stage_1: ComputePipeline,
    pub compute_pipeline_stage_2: ComputePipeline,

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

        let sim_parameters = SimulationParameters {
            dt_per_step: 0.5,
            substeps_per_frame: 5,
        };

        let time_buffer = device_m.create_buffer_init(&BufferInitDescriptor {
            label: Some("Time Uniform Buffer"),
            contents: bytemuck::bytes_of(&time_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let start_instant = Instant::now();
        let last_time: f32 = 0.0;

        // shader modules
        // a run time shader loader instead of compile time which makes the program ready for hot reload
        let compute_shader_path = load_ablsolute_path("shaders/rd_compute.wgsl");

        let compute_shader = device_m.create_shader_module(ShaderModuleDescriptor {
            label: Some("Compute Shader Module"),
            source: ShaderSource::Wgsl(compute_shader_path.into()),
        });

        // RD compute
        let compute_bgl =
            device_m.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        // uniform time buffer binding 0
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
                        // sampled source texture n binding 1
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
                        // storage texture declared as read and write so it can be used by RK2 second stage
                        // binding 2
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadWrite,
                            format: TextureFormat::Rgba32Float,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        // storage texture write only for the final result
                        binding: 3,
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

        let compute_pipeline_layout_stage_1 =
            device_m.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout Stage 1"),
                bind_group_layouts: &[&compute_bgl],
                push_constant_ranges: &[],
            });

        let compute_pipeline_stage_1 =
            device_m.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Compute Pipeline Stage 1"),
                layout: Some(&compute_pipeline_layout_stage_1),
                module: &compute_shader,
                entry_point: Some("main_predictor"),
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            });

        let compute_pipeline_layout_stage_2 =
            device_m.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout Stage 2"),
                bind_group_layouts: &[&compute_bgl],
                push_constant_ranges: &[],
            });

        let compute_pipeline_stage_2 =
            device_m.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Compute Pipeline Stage 2"),
                layout: Some(&compute_pipeline_layout_stage_2),
                module: &compute_shader,
                entry_point: Some("main_corrector"),
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            });

        let self_package = Self {
            sys_config,

            sim_parameters,

            time_buffer,
            _start_instant: start_instant,
            _last_time: last_time,

            compute_bgl,
            compute_pipeline_stage_1,
            compute_pipeline_stage_2,

            use_ping_as_source: true,
        };

        // return
        self_package
    }

    fn update_time_uniform(&self, gpu_res: &GpuResource) {
        // time
        let dt = self.sim_parameters.dt_per_step;
        let time_uniform = TimeUniform { dt, _pad: [0.0; 3] };

        gpu_res
            .queue
            .write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&time_uniform));
    }

    fn single_step_sim(
        &mut self,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        ping_view: &TextureView,
        pong_view: &TextureView,
        temp_view: &TextureView,
    ) {
        let device = &gpu_res.device;

        // get the size for dispatch
        let (width, height) = self.rd_size();

        // find out the source and destination ping or pong
        let (compute_source, compute_destination) = if self.use_ping_as_source {
            (ping_view, pong_view)
        } else {
            (pong_view, ping_view)
        };
        let workgroup_x = (width + WG_X - 1) / WG_X;
        let workgroup_y = (height + WG_Y - 1) / WG_Y;

        // scope calculating the predictor
        {
            // TODO I think I should cache it
            let compute_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Compute Bind Group First Stage"),
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
                        resource: BindingResource::TextureView(&temp_view),
                    },
                    BindGroupEntry {
                        binding: 3, // not used by the predictor only corrector
                        resource: BindingResource::TextureView(&compute_destination),
                    },
                ],
            });

            let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass Stage 1"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.compute_pipeline_stage_1);
            cpass.set_bind_group(0, &compute_bg, &[]);

            cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        }

        // scope calculating the corrector
        {
            // TODO I think I should cache it
            let compute_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Compute Bind Group Second Stage"),
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
                        resource: BindingResource::TextureView(&temp_view),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(&compute_destination),
                    },
                ],
            });
            let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass Stage 2"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.compute_pipeline_stage_2);
            cpass.set_bind_group(0, &compute_bg, &[]);

            cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        }

        self.use_ping_as_source = !self.use_ping_as_source;
    }

    // step simulation wraps the single step. Because we should be able to control how many steps
    // we want to step in every frame in the future patches
    pub fn step_simulation(
        &mut self,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        paused: bool,
        ping_view: &TextureView,
        pong_view: &TextureView,
        temp_view: &TextureView,
    ) {
        if paused {
            return;
        }

        self.update_time_uniform(gpu_res);

        let substeps = self.sim_parameters.substeps_per_frame.max(1);

        for _ in 0..substeps {
            self.single_step_sim(gpu_res, frame, ping_view, pong_view, temp_view);
        }
    }

    // reload and rebuild pipelines if shaders are changed
    fn reload_compute_pipeline(&mut self, gpu_res: &GpuResource) {
        let compute_shader_path = load_ablsolute_path("shaders/rd_compute.wgsl");
        let compute_shader = gpu_res.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Compute Shader (Rebuilding)"),
            source: ShaderSource::Wgsl(compute_shader_path.into()),
        });

        // "new layout" it is the same same but different (after changes in the shader)
        let compute_pipeline_layout_stage_1 =
            gpu_res
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Compute Pipeline Layout Stage 1 (Rebuilding"),
                    bind_group_layouts: &[&self.compute_bgl],
                    push_constant_ranges: &[],
                });

        let compute_pipeline_layout_stage_2 =
            gpu_res
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Compute Pipeline Layout Stage 2 (Rebuilding"),
                    bind_group_layouts: &[&self.compute_bgl],
                    push_constant_ranges: &[],
                });

        self.compute_pipeline_stage_1 =
            gpu_res
                .device
                .create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some("Compute Pipeline Stage 1 (Rebuilding)"),
                    layout: Some(&compute_pipeline_layout_stage_1),
                    module: &compute_shader,
                    entry_point: Some("main_predictor"),
                    compilation_options: PipelineCompilationOptions::default(),
                    cache: None,
                });

        self.compute_pipeline_stage_2 =
            gpu_res
                .device
                .create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some("Compute Pipeline Stage 2(Rebuilding)"),
                    layout: Some(&compute_pipeline_layout_stage_2),
                    module: &compute_shader,
                    entry_point: Some("main_corrector"),
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
