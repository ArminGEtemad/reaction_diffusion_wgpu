use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::load_ablsolute_path,
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};
use bytemuck::{Pod, Zeroable, bytes_of};
use std::num::NonZeroU64;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiParams {
    pub active_side: u32,
    pub paused: u32,
    pub brush_radius: f32,
    _pad: u32,
}

pub struct UiOverlayNode {
    bgl: BindGroupLayout,
    pipeline: RenderPipeline,
    params_buffer: Buffer,
}

impl UiOverlayNode {
    pub fn new(gpu_res: &GpuResource) -> Self {
        let device = &gpu_res.device;

        let ui_shader_path = load_ablsolute_path("shaders/ui_overlay.wgsl");
        let ui_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("UI Overlay Shader"),
            source: ShaderSource::Wgsl(ui_shader_path.into()),
        });

        let initial_params = UiParams {
            active_side: 0,
            paused: 0,
            brush_radius: 0.0,
            _pad: 0,
        };

        let params_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("UI Params Buffer"),
            contents: bytes_of(&initial_params),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("UI Overlay BGL"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<UiParams>() as u64),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("UI Overlay Pipeline Layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("UI Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: gpu_res.surface_format(),
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            bgl,
            pipeline,
            params_buffer,
        }
    }

    fn reload_pipeline(&mut self, gpu_res: &GpuResource) {
        let ui_shader_path = load_ablsolute_path("shaders/ui_overlay.wgsl");
        let ui_shader = gpu_res.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("UI Overlay Shader (Rebuilding)"),
            source: ShaderSource::Wgsl(ui_shader_path.into()),
        });

        let pipeline_layout = gpu_res
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("UI Overlay Pipeline Layout (Rebuilding)"),
                bind_group_layouts: &[&self.bgl],
                push_constant_ranges: &[],
            });

        self.pipeline = gpu_res
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("UI Overlay Pipeline (Rebuilding)"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &ui_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                fragment: Some(FragmentState {
                    module: &ui_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: PipelineCompilationOptions::default(),
                    targets: &[Some(ColorTargetState {
                        format: gpu_res.surface_format(),
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            });
    }

    fn rebuild_render_pipeline(&mut self, gpu_res: &GpuResource) {
        println!("Rebuilding Render Pipelines (Hot Reload)");
        self.reload_pipeline(gpu_res);
        println!("Render Pipelines Reloaded (Hot Reload)");
    }
}

impl RenderNode for UiOverlayNode {
    fn name(&self) -> &str {
        "UI Overlay Node"
    }

    fn pass_type(&self) -> PassType {
        PassType::Render
    }

    fn prepare(&mut self, _registry: &mut ResourceRegistry, _gpu_res: &GpuResource) {}

    fn execute(
        &mut self,
        _registry: &mut ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame: &PerFrameParameters,
    ) {
        // update uniform
        let params = UiParams {
            active_side: per_frame.ui_active_side,
            paused: per_frame.paused as u32, // turning the boolean to u32
            brush_radius: per_frame.brush_radius,
            _pad: 0,
        };
        gpu_res
            .queue
            .write_buffer(&self.params_buffer, 0, bytes_of(&params));

        let device = &gpu_res.device;

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("UI Overlay BG"),
            layout: &self.bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: self.params_buffer.as_entire_binding(),
            }],
        });

        let mut rpass = frame.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("UI Overlay Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &frame.view,
                resolve_target: None,
                ops: Operations {
                    // don't clear; we want to draw on top of RD
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        self.rebuild_render_pipeline(gpu_res);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
