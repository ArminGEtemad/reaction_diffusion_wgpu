use std::sync::Arc;

use crate::{
    InputState,
    gpu_resources::{FrameContext, GpuResource},
    nodes::{
        brush_node::ReactionDiffusionBrushNode,
        display_node::ReactionDiffusionDisplayNode,
        rd_node::{ReactionDiffusionSimulationNode, ReactionDiffustionTextureNames},
    },
    rd_system::{StartingPattern, SystemConfig, SystemParamsUniform},
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

// number of active slots for split screen
pub struct SimSlotConfig {
    pub slot_idx: u32,
    pub enabled: bool,
}

pub struct State {
    gpu_res: GpuResource,
    //rd_system: ReactionDiffusionSystem,
    graph: RenderGraph,
    shader_watcher: ShaderWatcher,
    pub number_of_sims: u32,
}

// helper function to generate texture names
fn make_texture_names(slot_idx: u32) -> ReactionDiffustionTextureNames {
    let prefix = format!("rd{}_", slot_idx);

    ReactionDiffustionTextureNames {
        ping: format!("{prefix}ping"),
        pong: format!("{prefix}pong"),
        temp: format!("{prefix}temp"),
        output: format!("{prefix}output"),
    }
}

impl State {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let gpu_res = GpuResource::new(window).await?;
        let mut graph = RenderGraph::new();

        let sys_config = SystemConfig {
            width: 1280,
            height: 1280,
        };

        // the enabled can be changed to false if we want only one screen
        let slots = vec![
            SimSlotConfig {
                slot_idx: 0,
                enabled: true,
            },
            SimSlotConfig {
                slot_idx: 1,
                enabled: true,
            },
        ];

        let mut slot_names: Vec<String> = Vec::new();

        // add brush and display node.
        let brush = ReactionDiffusionBrushNode::new(&gpu_res);
        let rd_display = ReactionDiffusionDisplayNode::new(&gpu_res);
        graph.add_node(brush);
        graph.add_node(rd_display);

        for slot in &slots {
            if !slot.enabled {
                continue;
            }
            // get the texture names
            let texture_names = make_texture_names(slot.slot_idx);

            slot_names.push(texture_names.output.clone());

            let rd_sim = ReactionDiffusionSimulationNode::new(
                &gpu_res,
                sys_config.clone(),
                texture_names,
                slot.slot_idx,
            );

            graph.add_node(rd_sim);
        }

        // get the number of simulations
        // TODO now that I have defined slots I don't need the simulation counts any more
        let number_of_sims = graph.simulaton_count();

        // prepare
        graph.prepare(&gpu_res);

        graph.for_each_node_mut::<ReactionDiffusionSimulationNode, _>(|sim_node| {
            match sim_node.slot_idx() {
                0 => {
                    // left side
                    let params: SystemParamsUniform = SystemParamsUniform {
                        du_rate: 0.16,
                        dv_rate: 0.08,
                        feed: 0.04,
                        kill: 0.06,
                    };
                    sim_node.set_params(&gpu_res, params);
                }
                1 => {
                    // right side
                    let params = SystemParamsUniform {
                        du_rate: 0.16,
                        dv_rate: 0.08,
                        feed: 0.025,
                        kill: 0.055,
                    };
                    sim_node.set_params(&gpu_res, params);
                }
                _ => {}
            }
        });

        // lookup the struct added to the graph
        // mutate the fields on it using the precomputed texture names
        let output_1_name = slot_names.get(0).cloned();
        let output_2_name = slot_names.get(1).cloned();

        if let Some(ref main_name) = output_1_name {
            if let Some(brush_node) = graph.get_node_mut::<ReactionDiffusionBrushNode>() {
                brush_node.set_targets(main_name.clone(), output_2_name.clone());
            }
            if let Some(display_node) = graph.get_node_mut::<ReactionDiffusionDisplayNode>() {
                display_node.set_targets(main_name.clone(), output_2_name.clone());
            }
        }

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
                let slot = sim_node.slot_idx();

                let initial_pattern_for_slot = match slot {
                    0 => left_pattern,
                    1 => right_pattern,
                    _ => return,
                };

                let matches_side = match side {
                    Side::AllSides => true,
                    Side::Left => slot == 0,
                    Side::Right => slot == 1,
                };

                if matches_side {
                    sim_node.reset(initial_pattern_for_slot);
                    found_any = true;
                }
            });

        if !found_any {
            eprint!("ReactionDiffusionSimulationNode not found!");
        }
    }
}
