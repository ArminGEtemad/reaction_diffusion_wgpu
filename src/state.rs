use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::ReactionDiffusionSystem,
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

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        // is anything changed?
        while let Ok(path) = self.shader_watcher.reciever_x.try_recv() {
            println!("Shader has been changed: {:?}", path);
            self.rd_system.rebuild_pipeline(&self.gpu_res);
        }

        let mut frame: FrameContext = self.gpu_res.begin_frame()?;
        self.rd_system
            .compute_and_render_pass(&self.gpu_res, &mut frame);
        self.gpu_res.submit_frame(frame);
        Ok(())
    }
}
