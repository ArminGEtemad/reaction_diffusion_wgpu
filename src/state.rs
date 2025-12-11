use crate::{
    InputState,
    gpu_resources::{FrameContext, GpuResource},
    rd_system::{BrushUniform, ReactionDiffusionSystem},
    shader_watcher::ShaderWatcher,
};
use wgpu::SurfaceError;
use winit::{dpi::PhysicalSize, window::Window};

pub struct State {
    gpu_res: GpuResource,
    rd_system: ReactionDiffusionSystem,
    shader_watcher: ShaderWatcher,
}

impl State {
    pub async fn new(window: &'static Window) -> Result<Self, String> {
        let gpu_res = GpuResource::new(window).await?;
        let rd_system = ReactionDiffusionSystem::new(&gpu_res);
        let shaders_path = format!("{}/shaders", env!("CARGO_MANIFEST_DIR")); // absolute address 
        println!("Watching Shaders at: {}", shaders_path);
        let shader_watcher = ShaderWatcher::new(shaders_path);

        Ok(Self {
            gpu_res,
            rd_system,
            shader_watcher,
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.gpu_res.resize(new_size);
    }

    pub fn render(&mut self, input: &InputState) -> Result<(), SurfaceError> {
        // is anything changed?
        while let Ok(path) = self.shader_watcher.reciever_x.try_recv() {
            println!("Shader has been changed: {:?}", path);
            self.rd_system.rebuild_pipeline(&self.gpu_res);
        }

        let (w_rd, h_rd) = self.rd_system.rd_size();

        // brush input
        let mut brush_uniform = BrushUniform {
            c_x: 0.0,
            c_y: 0.0,
            radius: 5.0, // TODO hardcoded now and needs to be changed in UI live
            _pad: 0.0,
        };

        if input.mouse_down {
            if let Some((mx, my)) = input.mouse_pos {
                let w = self.gpu_res.size.width as f32;
                let h = self.gpu_res.size.height as f32;

                if w > 0.0 && h > 0.0 {
                    let nx = (mx / w).clamp(0.0, 1.0);
                    let ny = (my / h).clamp(0.0, 1.0);

                    // y axis is mirrored because of different (0, 0) point

                    brush_uniform.c_x = nx * w_rd as f32;
                    brush_uniform.c_y = (1.0 - ny) * h_rd as f32;
                }
            }
        }

        // upload the brush uniform
        self.rd_system
            .set_brush_parameters(&self.gpu_res, &brush_uniform);

        let mut frame: FrameContext = self.gpu_res.begin_frame()?;
        self.rd_system
            .compute_and_render_pass(&self.gpu_res, &mut frame);
        self.gpu_res.submit_frame(frame);
        Ok(())
    }
}
