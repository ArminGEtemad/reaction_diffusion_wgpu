use std::num::NonZeroU64;

use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::{
        BrushUniform, ReactionDiffusionSystem, StartingPattern, SystemConfig, load_ablsolute_path,
        write_pattern_to_starting_space,
    },
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};
use bytemuck::bytes_of;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

// texture names for referencing
const TEX_RD_PING: &str = "rd ping";
const TEX_RD_PONG: &str = "rd pong";
const TEX_RD_TEMP: &str = "rd temp"; // needed for RK2 predictor, corrector
const TEX_RD_OUTPUT: &str = "rd output";

const WG_X: u32 = 16;
const WG_Y: u32 = 16;

// the brush node owns the bgls and the pipelines since it is not
// necessary for the RD system (it is just a feature)
pub struct ReactionDiffusionBrushNode {
    brush_buffer: Buffer,
    brush_bgl: BindGroupLayout,
    brush_pipeline: ComputePipeline,
}

// the simulation node is a wrapper for the system
// because I change the system from one project to another
pub struct ReactionDiffusionSimulationNode {
    rd_sim: ReactionDiffusionSystem,
    do_reset: Option<StartingPattern>,
}

// the render node owns the bgl and the pipelines.
pub struct ReactionDiffusionDisplayNode {
    render_bgl: Option<BindGroupLayout>,
    render_pipeline: Option<RenderPipeline>,
    sampler: Option<Sampler>,
}

pub fn create_rd_shared_nodes(
    gpu_res: &GpuResource,
) -> (
    ReactionDiffusionSimulationNode,
    ReactionDiffusionDisplayNode,
) {
    let sys_config = SystemConfig {
        width: 1280,
        height: 1280,
    };
    let sim = ReactionDiffusionSimulationNode {
        rd_sim: ReactionDiffusionSystem::new(gpu_res, sys_config),
        do_reset: Some(StartingPattern::Circle),
    };
    let display = ReactionDiffusionDisplayNode {
        render_bgl: None,
        render_pipeline: None,
        sampler: None,
    };

    (sim, display)
}

impl ReactionDiffusionBrushNode {
    pub fn new(gpu_res: &GpuResource) -> Self {
        let device = &gpu_res.device;

        let brush_shader_path = load_ablsolute_path("shaders/brush_compute.wgsl");
        let brush_shader = gpu_res.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Brush Shader"),
            source: ShaderSource::Wgsl(brush_shader_path.into()),
        });

        let brush_uniform = BrushUniform {
            c_x: 0.0,
            c_y: 0.0,
            radius: 0.0,
            mode: 0,
        };

        let brush_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Brush Uniform Buffer"),
            contents: bytes_of(&brush_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let brush_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Brush Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    // uniform buffer binding 0
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
                    // storage binding 1
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

        let brush_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Brush Pipeline Layout"),
            bind_group_layouts: &[&brush_bgl],
            push_constant_ranges: &[],
        });

        let brush_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Brush Pipeline"),
            layout: Some(&brush_pipeline_layout),
            module: &brush_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            brush_buffer,
            brush_bgl,
            brush_pipeline,
        }
    }
}

impl RenderNode for ReactionDiffusionBrushNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (Brush)"
    }

    fn pass_type(&self) -> PassType {
        PassType::Compute
    }

    fn prepare(&mut self, registry: &mut ResourceRegistry, gpu_res: &GpuResource) {
        // TODO hard coded now. should be read from the system config
        let (width, height) = (1280_u32, 1280_u32);

        registry.storage_texture_creator(
            TEX_RD_PING,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            TEX_RD_PONG,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );
    }

    fn execute(
        &mut self,
        registry: &mut ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parameters: &PerFrameParameters,
    ) {
        if !per_frame_parameters.mouse_down || per_frame_parameters.paused {
            return;
        }

        let device = &gpu_res.device;

        // TODO hardcoded now and should be read from the system config
        let (width, height) = (1280_u32, 1280_u32);

        let mut brush_uniform = BrushUniform {
            c_x: 0.0,
            c_y: 0.0,
            radius: per_frame_parameters.brush_radius,
            mode: per_frame_parameters.mode,
        };

        if let Some((mx, my)) = per_frame_parameters.mouse_pos {
            let w = gpu_res.size.width as f32;
            let h = gpu_res.size.height as f32;

            if w > 0.0 && h > 0.0 {
                let nx = (mx / w).clamp(0.0, 1.0);
                let ny = (my / h).clamp(0.0, 1.0);

                // y axis is mirrored because of different (0, 0) point

                brush_uniform.c_x = nx * width as f32;
                brush_uniform.c_y = (1.0 - ny) * height as f32;
            }
        }

        gpu_res
            .queue
            .write_buffer(&self.brush_buffer, 0, bytes_of(&brush_uniform));

        let target_view = registry
            .get_view(TEX_RD_OUTPUT)
            .expect("target view for brush is missing!");

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
                    resource: BindingResource::TextureView(&target_view),
                },
            ],
        });

        let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Brush Compute Pass"),
            timestamp_writes: None,
        });

        let workgroup_x = (width + WG_X - 1) / WG_X;
        let workgroup_y = (height + WG_Y - 1) / WG_Y;

        cpass.set_pipeline(&self.brush_pipeline);
        cpass.set_bind_group(0, &brush_bg, &[]);
        cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ReactionDiffusionSimulationNode {
    pub fn reset(&mut self, pattern: StartingPattern) {
        self.do_reset = Some(pattern)
    }
}

// Compute Only (+ mouse injection)
impl RenderNode for ReactionDiffusionSimulationNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (Simulation)"
    }

    fn pass_type(&self) -> PassType {
        PassType::Compute
    }

    fn prepare(&mut self, registry: &mut ResourceRegistry, gpu_res: &GpuResource) {
        let (width, height) = self.rd_sim.rd_size();

        registry.storage_texture_creator(
            TEX_RD_PING,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            TEX_RD_PONG,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            TEX_RD_TEMP,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        if let Some(pattern) = self.do_reset.take() {
            if let (Some(ping), Some(pong)) = (
                registry.get_texture(TEX_RD_PING),
                registry.get_texture(TEX_RD_PONG),
            ) {
                write_pattern_to_starting_space(
                    gpu_res,
                    &ping.texture,
                    &pong.texture,
                    pattern,
                    width,
                    height,
                );
                self.rd_sim.reset_time();
            }
        }
    }

    fn execute(
        &mut self,
        registry: &mut ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parames: &PerFrameParameters,
    ) {
        let (width, height) = self.rd_sim.rd_size();

        if let Some(pattern) = self.do_reset.take() {
            if let (Some(ping), Some(pong)) = (
                registry.get_texture(TEX_RD_PING),
                registry.get_texture(TEX_RD_PONG),
            ) {
                write_pattern_to_starting_space(
                    gpu_res,
                    &ping.texture,
                    &pong.texture,
                    pattern,
                    width,
                    height,
                );
                self.rd_sim.reset_time();
            } else {
                eprintln!("do_reset requested but ping/pong are missing");
            }
        }

        // get ping/pong/temp and clone to shorten borrow
        let (ping_view, pong_view, temp_view) = {
            let ping = registry
                .get_view(TEX_RD_PING)
                .expect("rd ping view is not registered!")
                .clone();
            let pong = registry
                .get_view(TEX_RD_PONG)
                .expect("rd pong view is not registered!")
                .clone();
            let temp_view = registry
                .get_view(TEX_RD_TEMP)
                .expect("rd temp view is not registered!")
                .clone();

            (ping, pong, temp_view)
        };

        self.rd_sim.step_simulation(
            gpu_res,
            frame,
            per_frame_parames.paused,
            &ping_view,
            &pong_view,
            &temp_view,
        );

        // decide newest view (same as before)
        let newest_view = if self.rd_sim.is_ping_source() {
            &ping_view
        } else {
            &pong_view
        };

        registry.set_view(TEX_RD_OUTPUT, newest_view);
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        // TODO bring the rebuild pipeline here too
        self.rd_sim.rebuild_pipeline(gpu_res);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// Render
impl RenderNode for ReactionDiffusionDisplayNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (Display)"
    }

    fn pass_type(&self) -> PassType {
        PassType::Render
    }

    fn prepare(&mut self, _registry: &mut ResourceRegistry, gpu_res: &GpuResource) {
        let device = &gpu_res.device;

        let render_shader_path = load_ablsolute_path("shaders/rd_display.wgsl");
        let render_shader = gpu_res.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Render Shader (Rebuilding)"),
            source: ShaderSource::Wgsl(render_shader_path.into()),
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Sampler Descriptor"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let render_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    // rd texture
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    // rd sampler
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Rendering Pipeline Layout"),
            bind_group_layouts: &[&render_bgl],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Rendering Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &render_shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: gpu_res.surface_format(),
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        self.render_bgl = Some(render_bgl);
        self.render_pipeline = Some(render_pipeline);
        self.sampler = Some(sampler);
    }

    // runs every frame
    fn execute(
        &mut self,
        registry: &mut ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        _per_frame_parames: &PerFrameParameters,
    ) {
        let device = &gpu_res.device;

        let render_pipeline = self
            .render_pipeline
            .as_ref()
            .expect("RD Display pipeline not ready");

        let render_bgl = self.render_bgl.as_ref().expect("RD Display BGL not ready");

        let sampler = self.sampler.as_ref().expect("RD Display sampler not ready");

        let rd_view = registry
            .get_view(TEX_RD_OUTPUT)
            .expect("RD output view is not registered!");

        // TODO Do I need to make the bind group every frame? can I cache it?
        let render_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Rendering from BG from  source 1"),
            layout: &render_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&rd_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        let mut rpass = frame.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render Display Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &frame.view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(render_pipeline);
        rpass.set_bind_group(0, &render_bg, &[]);
        rpass.draw(0..3, 0..1);
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        let render_shader_path = load_ablsolute_path("shaders/rd_display.wgsl");
        let render_shader = gpu_res.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Display Shader (Rebuilding)"),
            source: ShaderSource::Wgsl(render_shader_path.into()),
        });

        let render_bgl = self.render_bgl.as_ref().unwrap();
        let render_pipeline_layout =
            gpu_res
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Display Pipeline Layout (Rebuilding)"),
                    bind_group_layouts: &[&render_bgl],
                    push_constant_ranges: &[],
                });

        let render_pipeline = gpu_res
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Display Pipeline (Rebuilding)"),
                layout: Some(&render_pipeline_layout),
                vertex: VertexState {
                    module: &render_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                fragment: Some(FragmentState {
                    module: &render_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: PipelineCompilationOptions::default(),
                    targets: &[Some(ColorTargetState {
                        format: gpu_res.surface_format(),
                        blend: Some(BlendState::REPLACE),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            });

        self.render_pipeline = Some(render_pipeline);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
