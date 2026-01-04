use std::sync::Arc;

use crate::{
    InputState,
    gpu_resources::{FrameContext, GpuResource},
    nodes::{
        brush_node::ReactionDiffusionBrushNode,
        consts::*,
        display_node::ReactionDiffusionDisplayNode,
        rd_node::{ReactionDiffusionSimulationNode, ReactionDiffustionTextureNames},
    },
    rd_system::{StartingPattern, SystemConfig},
    render_graph::{graph::RenderGraph, node::PerFrameParameters},
    shader_watcher::ShaderWatcher,
};
use wgpu::SurfaceError;
use winit::{dpi::PhysicalSize, window::Window};

#[derive(Clone, Copy, Debug)]
pub enum Side {
    AllSides,
    Left,
    Right,
}

pub struct State {
    gpu_res: GpuResource,
    //rd_system: ReactionDiffusionSystem,
    graph: RenderGraph,
    shader_watcher: ShaderWatcher,
    pub number_of_sims: u32,
}

impl State {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let gpu_res = GpuResource::new(window).await?;
        let mut graph = RenderGraph::new();
        let sys_config = SystemConfig {
            width: 1280,
            height: 1280,
        };
        let rd1_sys = ReactionDiffustionTextureNames {
            ping: TEX_RD1_PING,
            pong: TEX_RD1_PONG,
            temp: TEX_RD1_TEMP,
            output: TEX_RD1_OUTPUT,
        };
        let rd2_sys = ReactionDiffustionTextureNames {
            ping: TEX_RD2_PING,
            pong: TEX_RD2_PONG,
            temp: TEX_RD2_TEMP,
            output: TEX_RD2_OUTPUT,
        };
        let brush = ReactionDiffusionBrushNode::new(&gpu_res);
        let rd1_sim = ReactionDiffusionSimulationNode::new(&gpu_res, sys_config.clone(), rd1_sys);
        let rd2_sim = ReactionDiffusionSimulationNode::new(&gpu_res, sys_config, rd2_sys);
        let rd_display = ReactionDiffusionDisplayNode::new(&gpu_res);

        // add nodes
        graph.add_node(brush);
        graph.add_node(rd1_sim);
        graph.add_node(rd2_sim);
        graph.add_node(rd_display);
        // get the number of simulations
        let number_of_sims = graph.simulaton_count();

        // prepare
        graph.prepare(&gpu_res);

        let shaders_path = format!("{}/shaders", env!("CARGO_MANIFEST_DIR")); // absolute address 
        println!("Watching Shaders at: {}", shaders_path);
        let shader_watcher = ShaderWatcher::new(shaders_path);

        Ok(Self {
            gpu_res,
            graph,
            shader_watcher,
            number_of_sims,
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

    pub fn reset(
        &mut self,
        left_pattern: StartingPattern,
        right_pattern: StartingPattern,
        side: Side,
    ) {
        let mut found_any = false;

        self.graph
            .for_each_node_mut::<ReactionDiffusionSimulationNode, _>(|sim_node| {
                let out = sim_node.out_put_texture_name();

                let (matches, pattern) = match side {
                    Side::AllSides => {
                        if out == TEX_RD1_OUTPUT {
                            (true, left_pattern)
                        } else if out == TEX_RD2_OUTPUT {
                            (true, right_pattern)
                        } else {
                            (false, left_pattern) // unknown output, ignore
                        }
                    }
                    Side::Left => (out == TEX_RD1_OUTPUT, left_pattern),
                    Side::Right => (out == TEX_RD2_OUTPUT, right_pattern),
                };

                if matches {
                    sim_node.reset(pattern);
                    found_any = true;
                }
            });

        if !found_any {
            eprint!("ReactionDiffusionSimulationNode not found!");
        }
    }
}
