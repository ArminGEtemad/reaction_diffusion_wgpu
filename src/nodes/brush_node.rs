use crate::{
    gpu_resources::{FrameContext, GpuResource},
    nodes::consts::*,
    rd_system::{BrushUniform, load_ablsolute_path},
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};
use bytemuck::bytes_of;
use std::num::NonZeroU64;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

// the brush node owns the bgls and the pipelines since it is not
// necessary for the RD system (it is just a feature)
pub struct ReactionDiffusionBrushNode {
    brush_buffer: Buffer,
    brush_bgl: BindGroupLayout,
    brush_pipeline: ComputePipeline,
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
