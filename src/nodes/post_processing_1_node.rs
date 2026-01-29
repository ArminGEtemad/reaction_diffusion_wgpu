use crate::{
    gpu_resources::GpuResource,
    nodes::consts::{WG_X, WG_Y},
    rd_system::{SystemConfig, load_ablsolute_path},
    render_graph::node::{PassType, RenderNode},
};
use wgpu::*;

pub struct PostProcessingNode {
    sys_config: SystemConfig,
    post_bgl: BindGroupLayout,
    post_pipeline: ComputePipeline,

    input_1: Option<String>,
    input_2: Option<String>,

    output_1: Option<String>,
    output_2: Option<String>,
}

impl PostProcessingNode {
    pub fn new(gpu_res: &GpuResource) -> Self {
        let device = &gpu_res.device;

        let sys_config = SystemConfig::new();

        let shader_path = load_ablsolute_path("shaders/post_processing_compute_1.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("RD PostProcessing"),
            source: ShaderSource::Wgsl(shader_path.into()),
        });

        let post_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("RD PostProcessing BGL"),
            entries: &[
                BindGroupLayoutEntry {
                    // input texture
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    // storage
                    binding: 1,
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

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("RD PostProcessing Pipeline Layout"),
            bind_group_layouts: &[&post_bgl],
            push_constant_ranges: &[],
        });

        let post_conpute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("RD PostProcessing Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            sys_config,
            post_bgl,
            post_pipeline: post_conpute_pipeline,
            input_1: None,
            input_2: None,
            output_1: None,
            output_2: None,
        }
    }

    pub fn set_targets(
        &mut self,
        input_1: String,
        input_2: Option<String>,
        output_1: String,
        output_2: Option<String>,
    ) {
        self.input_1 = Some(input_1);
        self.input_2 = input_2;
        self.output_1 = Some(output_1);
        self.output_2 = output_2;
    }
}

impl RenderNode for PostProcessingNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (PostProcessing)"
    }

    fn pass_type(&self) -> crate::render_graph::node::PassType {
        PassType::Compute
    }

    fn prepare(
        &mut self,
        _registry: &mut crate::render_graph::resource_registry::ResourceRegistry,
        _gpu_res: &GpuResource,
    ) {
    }

    fn execute(
        &mut self,
        registry: &mut crate::render_graph::resource_registry::ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut crate::gpu_resources::FrameContext,
        _per_frame_parameters: &crate::render_graph::node::PerFrameParameters,
    ) {
        let device = &gpu_res.device;

        let width = self.sys_config.width;
        let height = self.sys_config.height;

        let workgroup_x = (width + WG_X - 1) / WG_X;
        let workgroup_y = (height + WG_Y - 1) / WG_Y;

        // LEFT
        if let (Some(input_name), Some(output_name)) = (&self.input_1, &self.output_1) {
            registry.storage_texture_creator(
                output_name,
                gpu_res,
                width,
                height,
                TextureFormat::Rgba32Float,
            );

            if let (Some(input_view), Some(output_view)) = (
                registry.get_view(input_name),
                registry.get_view(output_name),
            ) {
                let bg = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("RD PostProcessing BG (left)"),
                    layout: &self.post_bgl,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(input_view),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(output_view),
                        },
                    ],
                });

                let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("RD PostProcessing Compute Pass (left)"),
                    timestamp_writes: None,
                });

                cpass.set_pipeline(&self.post_pipeline);
                cpass.set_bind_group(0, &bg, &[]);
                cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
            } else {
                eprintln!("[PostProcessing] Missing view for '{}'", input_name);
            }
        }

        // RIGHT
        if let (Some(input_name), Some(output_name)) = (&self.input_2, &self.output_2) {
            registry.storage_texture_creator(
                output_name,
                gpu_res,
                width,
                height,
                TextureFormat::Rgba32Float,
            );

            if let (Some(input_view), Some(output_view)) = (
                registry.get_view(input_name),
                registry.get_view(output_name),
            ) {
                let bg = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("RD PostProcessing BG (right)"),
                    layout: &self.post_bgl,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(input_view),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(output_view),
                        },
                    ],
                });

                let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("RD PostProcessing Compute Pass (right)"),
                    timestamp_writes: None,
                });

                cpass.set_pipeline(&self.post_pipeline);
                cpass.set_bind_group(0, &bg, &[]);
                cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
            } else {
                eprintln!("[PostProcessing] Missing view for '{}'", input_name);
            }
        }
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        let device = &gpu_res.device;

        let shader_path = load_ablsolute_path("shaders/post_processing_compute_1.wgsl");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("RD PostProcessing Shader (Rebuild)"),
            source: ShaderSource::Wgsl(shader_path.into()),
        });

        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("RD PostProcessing Pipeline Layout (Rebuild)"),
            bind_group_layouts: &[&self.post_bgl],
            push_constant_ranges: &[],
        });

        self.post_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("RD PostProcessing Pipeline (Rebuild)"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
