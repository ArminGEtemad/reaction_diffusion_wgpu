use crate::{
    gpu_resources::{FrameContext, GpuResource},
    render_graph::resource_registry::ResourceRegistry,
};
use std::any::Any;

pub struct PerFrameParameters {
    pub mouse_pos: Option<(f32, f32)>,
    pub mouse_down: bool,
    pub brush_radius: f32,
    pub mode: u32,
    pub paused: bool,
    pub debug_mode: bool,
}

// Compute pass must always come before Render pass
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PassType {
    Compute,
    Render,
}

pub trait RenderNode {
    fn name(&self) -> &str;

    // Give every node its type so the engine knows the oder of execution.
    fn pass_type(&self) -> PassType;

    fn prepare(&mut self, _registry: &mut ResourceRegistry, _gpu_res: &GpuResource) {}

    // execute every frame
    fn execute(
        &mut self,
        registry: &mut ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parameters: &PerFrameParameters,
    );

    // called when shaders are reloded
    fn called_on_hotreload(&mut self, _gpu_res: &GpuResource) {}

    // helper function to get the number of screens
    fn get_number_of_simulations(&self) -> u32 {
        0 // no simulation by default
    }

    // need it for the graph to be able to get access to any node I want in other scripts
    fn as_any(&mut self) -> &mut dyn Any;
}
