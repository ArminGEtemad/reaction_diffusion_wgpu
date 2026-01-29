use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::{
        ReactionDiffusionSystem, StartingPattern, SystemConfig, SystemParamsUniform,
        write_pattern_to_starting_space,
    },
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};
use wgpu::*;

pub struct ReactionDiffustionTextureNames {
    pub ping: String,
    pub pong: String,
    pub temp: String,
    pub output: String,
}

// the simulation node is a wrapper for the system
// because I change the system from one project to another
pub struct ReactionDiffusionSimulationNode {
    slot_idx: u32,
    rd_sim: ReactionDiffusionSystem,
    sys_config: SystemConfig,
    texture_names: ReactionDiffustionTextureNames,
    do_reset: Option<StartingPattern>,
}

impl ReactionDiffusionSimulationNode {
    pub fn new(
        gpu_res: &GpuResource,
        texture_names: ReactionDiffustionTextureNames,
        slot_idx: u32,
    ) -> Self {
        let rd_sim = ReactionDiffusionSystem::new(gpu_res);
        let sys_config = SystemConfig::new();

        Self {
            slot_idx,
            rd_sim,
            sys_config,
            texture_names,
            do_reset: Some(StartingPattern::CleanSheet), // Staring with clean sheet makes more sense
        }
    }

    pub fn reset(&mut self, pattern: StartingPattern) {
        self.do_reset = Some(pattern)
    }

    pub fn slot_idx(&self) -> u32 {
        self.slot_idx
    }

    pub fn set_params(&mut self, gpu_res: &GpuResource, params: SystemParamsUniform) {
        self.rd_sim.set_rd_sys_parameters(gpu_res, params);
    }
}

// Compute Only (+ mouse injection)
impl RenderNode for ReactionDiffusionSimulationNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (Simulation)"
    }

    fn pass_type(&self) -> PassType {
        PassType::Compute
    }

    fn prepare(&mut self, registry: &mut ResourceRegistry, gpu_res: &GpuResource) {
        let width = self.sys_config.width;
        let height = self.sys_config.height;

        registry.storage_texture_creator(
            self.texture_names.ping.as_str(),
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            self.texture_names.pong.as_str(),
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            self.texture_names.temp.as_str(),
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        if let Some(pattern) = self.do_reset.take() {
            if let (Some(ping), Some(pong)) = (
                registry.get_texture(self.texture_names.ping.as_str()),
                registry.get_texture(self.texture_names.pong.as_str()),
            ) {
                write_pattern_to_starting_space(
                    gpu_res,
                    &ping.texture,
                    &pong.texture,
                    pattern,
                    width,
                    height,
                );
                self.rd_sim.reset_time();
            }
        }
    }

    fn execute(
        &mut self,
        registry: &mut ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parames: &PerFrameParameters,
    ) {
        let width = self.sys_config.width;
        let height = self.sys_config.height;

        if let Some(pattern) = self.do_reset.take() {
            if let (Some(ping), Some(pong)) = (
                registry.get_texture(self.texture_names.ping.as_str()),
                registry.get_texture(self.texture_names.pong.as_str()),
            ) {
                write_pattern_to_starting_space(
                    gpu_res,
                    &ping.texture,
                    &pong.texture,
                    pattern,
                    width,
                    height,
                );
                self.rd_sim.reset_time();
            } else {
                eprintln!("do_reset requested but ping/pong are missing");
            }
        }

        // get ping/pong/temp and clone to shorten borrow
        let (ping_view, pong_view, temp_view) = {
            let ping = registry
                .get_view(self.texture_names.ping.as_str())
                .expect("rd ping view is not registered!")
                .clone();
            let pong = registry
                .get_view(self.texture_names.pong.as_str())
                .expect("rd pong view is not registered!")
                .clone();
            let temp_view = registry
                .get_view(self.texture_names.temp.as_str())
                .expect("rd temp view is not registered!")
                .clone();

            (ping, pong, temp_view)
        };

        self.rd_sim.step_simulation(
            gpu_res,
            frame,
            per_frame_parames.paused,
            &ping_view,
            &pong_view,
            &temp_view,
        );

        // decide newest view (same as before)
        let newest_view = if self.rd_sim.is_ping_source() {
            &ping_view
        } else {
            &pong_view
        };

        registry.set_view(self.texture_names.output.as_str(), newest_view);
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        // TODO bring the rebuild pipeline here too
        self.rd_sim.rebuild_pipeline(gpu_res);
    }

    fn get_number_of_simulations(&self) -> u32 {
        // every simulation is 1 simulation of course
        // in brush for example will be still 0 simulation
        1
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
