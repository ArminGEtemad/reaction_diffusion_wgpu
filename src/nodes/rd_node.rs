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
use wgpu::*;

pub struct ReactionDiffusionSimulationNode {
    rd_sim: ReactionDiffusionSystem,
    do_reset: Option<StartingPattern>,
}

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
            "rd ping",
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            "rd pong",
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        if let Some(pattern) = self.do_reset.take() {
            if let (Some(ping), Some(pong)) = (
                registry.get_texture("rd ping"),
                registry.get_texture("rd pong"),
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
                registry.get_texture("rd ping"),
                registry.get_texture("rd pong"),
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

        // brush input
        let mut brush_uniform = BrushUniform {
            c_x: 0.0,
            c_y: 0.0,
            radius: if per_frame_parames.mouse_down {
                per_frame_parames.brush_radius
            } else {
                0.0
            },
            mode: per_frame_parames.mode,
        };

        if per_frame_parames.mouse_down {
            if let Some((mx, my)) = per_frame_parames.mouse_pos {
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
        }

        self.rd_sim.set_brush_parameters(gpu_res, &brush_uniform);

        // get ping or pong and clone to shorten borrow
        let (ping_view, pong_view) = {
            let ping = registry
                .get_view("rd ping")
                .expect("rd ping view is not registered")
                .clone();
            let pong = registry
                .get_view("rd pong")
                .expect("rd pong view is not registered")
                .clone();
            (ping, pong)
        };

        let newest_view = {
            self.rd_sim.step_simulation(
                gpu_res,
                frame,
                per_frame_parames.paused,
                &ping_view,
                &pong_view,
            );

            if self.rd_sim.is_ping_source() {
                &ping_view
            } else {
                &pong_view
            }
        };

        registry.set_view("rd output", newest_view);
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
            .get_view("rd output")
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
