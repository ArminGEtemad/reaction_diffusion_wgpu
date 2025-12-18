use crate::{
    InputState,
    gpu_resources::{FrameContext, GpuResource},
    nodes::rd_node::{ReactionDiffusionSimulationNode, create_rd_shared_nodes},
    rd_system::StartingPattern,
    render_graph::{graph::RenderGraph, node::PerFrameParameters},
    shader_watcher::ShaderWatcher,
};
use wgpu::SurfaceError;
use winit::{dpi::PhysicalSize, window::Window};

pub struct State {
    gpu_res: GpuResource,
    //rd_system: ReactionDiffusionSystem,
    graph: RenderGraph,
    shader_watcher: ShaderWatcher,
}

impl State {
    pub async fn new(window: &'static Window) -> Result<Self, String> {
        let gpu_res = GpuResource::new(window).await?;
        let mut graph = RenderGraph::new();
        let (rd_sim, rd_display) = create_rd_shared_nodes(&gpu_res);
        graph.add_node(rd_sim);
        graph.add_node(rd_display);
        graph.prepare(&gpu_res);

        let shaders_path = format!("{}/shaders", env!("CARGO_MANIFEST_DIR")); // absolute address 
        println!("Watching Shaders at: {}", shaders_path);
        let shader_watcher = ShaderWatcher::new(shaders_path);

        Ok(Self {
            gpu_res,
            graph,
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
            self.graph.notify_on_hotreload_graph(&self.gpu_res);
        }

        let mut frame: FrameContext = self.gpu_res.begin_frame()?;

        let per_frame_parameters = PerFrameParameters {
            mouse_pos: input.mouse_pos,
            mouse_down: input.mouse_down,
            brush_radius: input.brush_radius,
            mode: input.mode,
            paused: input.paused,
            debug_mode: input.debug_mode,
        };

        self.graph
            .execute(&self.gpu_res, &mut frame, &per_frame_parameters);

        self.gpu_res.submit_frame(frame);
        Ok(())
    }

    pub fn reset(&mut self, pattern: StartingPattern) {
        if let Some(sim_node) = self.graph.get_node_mut::<ReactionDiffusionSimulationNode>() {
            sim_node.reset(&self.gpu_res, pattern);
        } else {
            eprint!("ReactionDiffusionNode not found!");
        }
    }
}
