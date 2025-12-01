use bytemuck::{Pod, Zeroable};
use std::{num::NonZeroU64, time::Instant};

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::gpu_resources::{FrameContext, GpuResource};

// time
#[repr(C)] // format expected by the gpu
#[derive(Clone, Copy, Pod, Zeroable)]
struct TimeUniform {
    // 16 byte alignment needed
    dt: f32,        // 4 byte
    _pad: [f32; 3], // 12 byte
}
// Communication between the system and GPU
pub struct ReactionDiffusionSystem {
    // uniform
    pub time_buffer: Buffer,
    pub start_instant: Instant,
    pub last_time: f32,

    // compute
    pub _compute_bgl: BindGroupLayout,
    pub compute_bg: BindGroup,
    pub compute_pipeline: ComputePipeline,
}

impl ReactionDiffusionSystem {
    pub fn new(gpu_res: &GpuResource) -> Self {
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

        Self {
            time_buffer,
            start_instant,
            last_time,

            _compute_bgl: compute_bgl,
            compute_bg,
            compute_pipeline,
        }
    }

    // resposible for updating time and render pass / compute pass
    pub fn compute_and_render_pass(&mut self, gpu_res: &GpuResource, frame: &mut FrameContext) {
        // update dt
        let now = self.start_instant.elapsed().as_secs_f32();
        let dt = (now - self.last_time).max(0.0);
        self.last_time = now;

        let time_uniform = TimeUniform { dt, _pad: [0.0; 3] };

        gpu_res
            .queue
            .write_buffer(&self.time_buffer, 0, bytemuck::bytes_of(&time_uniform));

        // compute pass scope
        {
            let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
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
            let _rpass = frame.encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &frame.view,
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
    }
}
