use crate::{
    gpu_resources::{FrameContext, GpuResource},
    nodes::consts::*,
    rd_system::{SystemConfig, load_ablsolute_path},
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};
use bytemuck::{Pod, Zeroable, bytes_of};
use std::num::NonZeroU64;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

// the brush node owns the bgls and the pipelines since it is not
// necessary for the RD system (it is just a feature)

// Brush
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BrushUniform {
    pub c_x: f32,    // 4 byte
    pub c_y: f32,    // 4 byte
    pub radius: f32, // 4 byte
    pub mode: u32,   // 4 byte
}
pub struct ReactionDiffusionBrushNode {
    sys_config: SystemConfig,

    brush_buffer: Buffer,
    brush_bgl: BindGroupLayout,
    brush_pipeline: ComputePipeline,

    output_1: Option<String>, // lefgt
    output_2: Option<String>, // right
}

impl ReactionDiffusionBrushNode {
    pub fn new(gpu_res: &GpuResource) -> Self {
        let device = &gpu_res.device;

        let sys_config = SystemConfig::new();

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
            sys_config,
            brush_buffer,
            brush_bgl,
            brush_pipeline,
            output_1: None,
            output_2: None,
        }
    }

    pub fn set_targets(&mut self, output_1: String, output_2: Option<String>) {
        self.output_1 = Some(output_1);
        self.output_2 = output_2;
    }
}

impl RenderNode for ReactionDiffusionBrushNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (Brush)"
    }

    fn pass_type(&self) -> PassType {
        PassType::Compute
    }

    fn prepare(&mut self, _registry: &mut ResourceRegistry, _gpu_res: &GpuResource) {}

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

        // get names
        let output_1_name = match &self.output_1 {
            Some(name) => name.as_str(),
            None => {
                eprintln!("Mandatory brush target not found!");
                return;
            }
        };

        let output_2_name_opt = self.output_2.as_deref();

        let width = self.sys_config.width;
        let height = self.sys_config.height;

        let mut brush_uniform = BrushUniform {
            c_x: 0.0,
            c_y: 0.0,
            radius: per_frame_parameters.brush_radius,
            mode: per_frame_parameters.mode,
        };

        // default one screen
        let mut target_texture_name: &str = output_1_name;

        if let Some((mx, my)) = per_frame_parameters.mouse_pos {
            let w = gpu_res.size.width as f32;
            let h = gpu_res.size.height as f32;

            if w > 0.0 && h > 0.0 {
                let nx_screen = (mx / w).clamp(0.0, 1.0);
                let ny = (my / h).clamp(0.0, 1.0);

                if nx_screen >= 0.9 {
                    return;
                }

                let nx = (nx_screen / 0.9).clamp(0.0, 1.0);

                // is rd2 available
                let rd2_view_opt = output_2_name_opt.and_then(|name| registry.get_view(name));

                if rd2_view_opt.is_some() && nx >= 0.5 {
                    // right half
                    target_texture_name = output_2_name_opt.unwrap();
                    let mapped_nx = (nx - 0.5) * 2.0;
                    brush_uniform.c_x = mapped_nx * width as f32;
                } else {
                    // left half
                    target_texture_name = output_1_name;

                    let mapped_nx = if rd2_view_opt.is_some() {
                        // rd1 is half left
                        nx * 2.0
                    } else {
                        // no split screen
                        nx
                    };
                    brush_uniform.c_x = mapped_nx * width as f32;
                }

                // y axis is mirrored because of different (0, 0) point
                brush_uniform.c_y = (1.0 - ny) * height as f32;
            }
        }

        gpu_res
            .queue
            .write_buffer(&self.brush_buffer, 0, bytes_of(&brush_uniform));

        let target_view = registry
            .get_view(target_texture_name)
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
