use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::ReactionDiffusionSystem,
};
use wgpu::SurfaceError;
use winit::{dpi::PhysicalSize, window::Window};

pub struct State {
    gpu_res: GpuResource,
    rd_system: ReactionDiffusionSystem,
}

impl State {
    pub async fn new(window: &'static Window) -> Result<Self, String> {
        let gpu_res = GpuResource::new(window).await?;
        let rd_system = ReactionDiffusionSystem::new(&gpu_res);
        Ok(Self { gpu_res, rd_system })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.gpu_res.resize(new_size);
    }

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        let mut frame: FrameContext = self.gpu_res.begin_frame()?;
        self.rd_system
            .compute_and_render_pass(&self.gpu_res, &mut frame);
        self.gpu_res.submit_frame(frame);
        Ok(())
    }
}
