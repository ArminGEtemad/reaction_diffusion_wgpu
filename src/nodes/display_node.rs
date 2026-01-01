use std::num::NonZeroU64;

use crate::{
    gpu_resources::{FrameContext, GpuResource},
    nodes::consts::*,
    rd_system::load_ablsolute_path,
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};
use bytemuck::{Pod, Zeroable, bytes_of};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct DisplayParameters {
    // not boolean because I want up to 4 rd sim at the same time at some point
    pub split_screen: u32, // 0: no, 1: yes
    _pad: [u32; 3],
}

// the render node owns the bgl and the pipelines.
pub struct ReactionDiffusionDisplayNode {
    render_bgl: Option<BindGroupLayout>,
    render_pipeline: Option<RenderPipeline>,
    sampler: Option<Sampler>,
    display_buffer: Option<Buffer>,
}

impl ReactionDiffusionDisplayNode {
    pub fn new(gpu_res: &GpuResource) -> Self {
        let device = &gpu_res.device;

        let display_parameters = DisplayParameters {
            split_screen: 0,
            _pad: [0; 3],
        };

        let display_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Display Parameter Buffer"),
            contents: bytes_of(&display_parameters),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        Self {
            render_bgl: None,
            render_pipeline: None,
            sampler: None,
            display_buffer: Some(display_buffer),
        }
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
                    // rd1 texture
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
                    // rd2 texture
                    binding: 1,
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
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    // Uniform for display buffer
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            std::mem::size_of::<DisplayParameters>() as u64
                        ),
                    },
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
        let queue = &gpu_res.queue;

        let render_pipeline = self
            .render_pipeline
            .as_ref()
            .expect("RD Display pipeline not ready");

        let render_bgl = self.render_bgl.as_ref().expect("RD Display BGL not ready");

        let sampler = self.sampler.as_ref().expect("RD Display sampler not ready");

        let display_buffer = self
            .display_buffer
            .as_ref()
            .expect("Display Buffer not ready!");

        let rd1_view = registry
            .get_view(TEX_RD1_OUTPUT)
            .expect("RD output view is not registered!");

        // split screen is optional
        let rd2_view_opt = registry.get_view(TEX_RD2_OUTPUT);

        let split_screen_flag = if rd2_view_opt.is_some() { 1_u32 } else { 0_u32 };
        let rd2_view = rd2_view_opt.unwrap();

        let display_parameters = DisplayParameters {
            split_screen: split_screen_flag,
            _pad: [0; 3],
        };

        queue.write_buffer(display_buffer, 0, bytes_of(&display_parameters));

        // TODO Do I need to make the bind group every frame? can I cache it?
        let render_bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Rendering from BG from  source 1"),
            layout: &render_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&rd1_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&rd2_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: display_buffer.as_entire_binding(),
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
