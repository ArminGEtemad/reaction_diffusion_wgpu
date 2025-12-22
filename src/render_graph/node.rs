use std::any::Any;

use crate::{
    gpu_resources::{FrameContext, GpuResource},
    render_graph::resource_registry::ResourceRegistry,
};

pub struct PerFrameParameters {
    pub mouse_pos: Option<(f32, f32)>,
    pub mouse_down: bool,
    pub brush_radius: f32,
    pub mode: u32,
    pub paused: bool,
    pub debug_mode: bool,
}

pub trait RenderNode {
    fn name(&self) -> &str;

    fn prepare(&mut self, _registry: &mut ResourceRegistry, _gpu_res: &GpuResource) {}

    // execute every frame
    fn execute(
        &mut self,
        registry: &mut ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parames: &PerFrameParameters,
    );

    // called when shaders are reloded
    fn called_on_hotreload(&mut self, _gpu_res: &GpuResource) {}

    // need it for the graph to be able to get access to any node I want in other scripts
    fn as_any(&mut self) -> &mut dyn Any;
}
