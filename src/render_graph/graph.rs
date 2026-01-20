use crate::{
    gpu_resources::{FrameContext, GpuResource},
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};

struct NodeEntry {
    pass_type: PassType,
    node: Box<dyn RenderNode>,
}

pub struct RenderGraph {
    registry: ResourceRegistry,
    nodes: Vec<NodeEntry>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            registry: ResourceRegistry::new(),
            nodes: Vec::new(),
        }
    }

    pub fn add_node<N: RenderNode + 'static>(&mut self, node: N) {
        let pass_type = node.pass_type();
        let entry = NodeEntry {
            pass_type,
            node: Box::new(node),
        };
        self.nodes.push(entry);

        self.nodes.sort_by_key(|e| e.pass_type);
    }

    pub fn prepare(&mut self, gpu_res: &GpuResource) {
        for entry in self.nodes.iter_mut() {
            entry.node.prepare(&mut self.registry, gpu_res);
        }
    }

    pub fn execute(
        &mut self,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parameters: &PerFrameParameters,
    ) {
        for entry in self.nodes.iter_mut() {
            if per_frame_parameters.debug_mode {
                println!("Executing node: {}", entry.node.name());
            }

            entry
                .node
                .execute(&mut self.registry, gpu_res, frame, per_frame_parameters);
        }
    }

    pub fn notify_on_hotreload_graph(&mut self, gpu_res: &GpuResource) {
        for entry in self.nodes.iter_mut() {
            entry.node.called_on_hotreload(gpu_res);
        }
    }

    // getting a mutabe refrence to a node in other scripts
    pub fn get_node_mut<N: 'static>(&mut self) -> Option<&mut N> {
        for entry in self.nodes.iter_mut() {
            if let Some(node) = entry.node.as_any().downcast_mut::<N>() {
                return Some(node);
            }
        }
        None
    }

    // apply a function to every node.
    pub fn for_each_node_mut<N: 'static, F: FnMut(&mut N)>(&mut self, mut f: F) {
        for entry in self.nodes.iter_mut() {
            if let Some(node) = entry.node.as_any().downcast_mut::<N>() {
                f(node);
            }
        }
    }

    // count the number of simulation
    pub fn simulaton_count(&self) -> u32 {
        self.nodes
            .iter()
            .map(|sim| sim.node.get_number_of_simulations())
            .sum()
    }
}
