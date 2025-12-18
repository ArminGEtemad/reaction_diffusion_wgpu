use crate::{
    gpu_resources::{FrameContext, GpuResource},
    render_graph::{
        node::{PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};

pub struct RenderGraph {
    pub registry: ResourceRegistry,
    pub nodes: Vec<Box<dyn RenderNode>>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            registry: ResourceRegistry::new(),
            nodes: Vec::new(),
        }
    }

    pub fn add_node<N: RenderNode + 'static>(&mut self, node: N) {
        self.nodes.push(Box::new(node));
    }

    pub fn prepare(&mut self, gpu_res: &GpuResource) {
        for node in self.nodes.iter_mut() {
            node.prepare(&mut self.registry, gpu_res);
        }
    }

    pub fn execute(
        &mut self,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parameters: &PerFrameParameters,
    ) {
        for node in self.nodes.iter_mut() {
            if per_frame_parameters.debug_mode {
                println!("Executing {}", node.name());
            }
            node.execute(&mut self.registry, gpu_res, frame, per_frame_parameters);
        }
    }

    pub fn notify_on_hotreload_graph(&mut self, gpu_res: &GpuResource) {
        for node in self.nodes.iter_mut() {
            node.called_on_hotreload(gpu_res);
        }
    }

    // getting a mutabe refrence to a node in other scripts
    pub fn get_node_mut<N: 'static>(&mut self) -> Option<&mut N> {
        for node in self.nodes.iter_mut() {
            if let Some(n) = node.as_any().downcast_mut::<N>() {
                return Some(n);
            }
        }
        None
    }
}
