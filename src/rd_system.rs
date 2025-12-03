use bytemuck::{Pod, Zeroable};
use std::{num::NonZeroU64, time::Instant};

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use crate::gpu_resources::{FrameContext, GpuResource};

// Pixels
const HEIGHT: u32 = 1280;
const WIDTH: u32 = 720;

const WG_X: u32 = 8;
const WG_Y: u32 = 8;

// time
// this lives in group 0 binding 0
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

    // texture source lives in group 0 binding 1
    // using two textures one reads while other writes
    // then the roles change
    pub texture_source_1: Texture,
    pub texture_source_2: Texture,
    pub texture_view_1: TextureView,
    pub texture_view_2: TextureView,
    pub sampler: Sampler,

    // compute
    pub _compute_bgl: BindGroupLayout,
    pub compute_bg_1_to_2: BindGroup,
    pub compute_bg_2_to_1: BindGroup,
    pub compute_pipeline: ComputePipeline,

    // ping or pong :)
    pub use_1_as_source: bool,
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

        // create textures
        let texture_desc = TextureDescriptor {
            label: Some("Texture Descriptor"),
            size: Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture_source_1 = device_m.create_texture(&TextureDescriptor {
            label: Some("Texture Descriptor 1"),
            ..texture_desc.clone()
        });

        let texture_source_2 = device_m.create_texture(&TextureDescriptor {
            label: Some("Texture Descriptor 2"),
            ..texture_desc // passing ownership since we don't need it anymore
        });

        let texture_view_1 = texture_source_1.create_view(&TextureViewDescriptor::default());
        let texture_view_2 = texture_source_2.create_view(&TextureViewDescriptor::default());

        let sampler = device_m.create_sampler(&SamplerDescriptor {
            label: Some("Sampler Descriptor"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // initialize a blob in the middle
        // TODO make a separate file for blob
        let mut data = vec![0.0_f32; (WIDTH * HEIGHT * 4) as usize]; // each pixel has 4 values RGBA

        // loop over all the pixels
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let pixel_idx = ((y * WIDTH + x) * 4) as usize;

                // element U everywhere
                // element V only in blob
                let u = 1.0_f32;
                let mut v = 0.0_f32;

                // blob in the center for element V
                let center_x = WIDTH as i32 / 2;
                let center_y = HEIGHT as i32 / 2;

                let dist_x = x as i32 - center_x;
                let dist_y = y as i32 - center_y;

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

        let data_bytes: &[u8] = bytemuck::cast_slice(&data);

        let layout = TexelCopyBufferLayout {
            offset: 0,
            // RGBA32Float = 4 channel * 4 byte per pixel
            bytes_per_row: Some(4 * 4 * WIDTH),
            rows_per_image: Some(HEIGHT),
        };

        let extent = Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        };

        // queue source 1
        gpu_res.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture_source_1,
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
                texture: &texture_source_2,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            data_bytes,
            layout,
            extent,
        );

        // shader modules
        let compute_shader = device_m.create_shader_module(ShaderModuleDescriptor {
            label: Some("Compute Shder Module"),
            source: ShaderSource::Wgsl(include_str!("../shaders/rd_compute.wgsl").into()),
        });

        // compute
        let compute_bgl =
            device_m.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Compute Bing Group Layout"),
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

        // write to 2
        let compute_bg_1_to_2 = device_m.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &compute_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: time_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&texture_view_1),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&texture_view_2),
                },
            ],
        });

        // write to 1
        let compute_bg_2_to_1 = device_m.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &compute_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: time_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&texture_view_2),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&texture_view_1),
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

        Self {
            time_buffer,
            start_instant,
            last_time,

            texture_source_1,
            texture_source_2,
            texture_view_1,
            texture_view_2,
            sampler,

            _compute_bgl: compute_bgl,
            compute_bg_1_to_2,
            compute_bg_2_to_1,
            compute_pipeline,

            use_1_as_source: true,
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

        // ping or pong?
        let compute_bg = if self.use_1_as_source {
            &self.compute_bg_1_to_2
        } else {
            &self.compute_bg_2_to_1
        };

        // compute pass scope
        {
            let mut cpass = frame.encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, compute_bg, &[]);

            let workgroup_x = (WIDTH + WG_X - 1) / WG_X;
            let workgroup_y = (HEIGHT + WG_Y - 1) / WG_Y;
            cpass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
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
