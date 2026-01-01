use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::{
        ReactionDiffusionSystem, StartingPattern, SystemConfig, write_pattern_to_starting_space,
    },
    render_graph::{
        node::{PassType, PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};
use wgpu::*;

pub struct ReactionDiffustionTextureNames {
    pub ping: &'static str,
    pub pong: &'static str,
    pub temp: &'static str,
    pub output: &'static str,
}

// the simulation node is a wrapper for the system
// because I change the system from one project to another
pub struct ReactionDiffusionSimulationNode {
    rd_sim: ReactionDiffusionSystem,
    texture_names: ReactionDiffustionTextureNames,
    do_reset: Option<StartingPattern>,
}

impl ReactionDiffusionSimulationNode {
    pub fn new(
        gpu_res: &GpuResource,
        sys_config: SystemConfig,
        texture_names: ReactionDiffustionTextureNames,
    ) -> Self {
        let rd_sim = ReactionDiffusionSystem::new(gpu_res, sys_config);

        Self {
            rd_sim,
            texture_names,
            do_reset: Some(StartingPattern::Circle),
        }
    }

    pub fn reset(&mut self, pattern: StartingPattern) {
        self.do_reset = Some(pattern)
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
        let (width, height) = self.rd_sim.rd_size();

        registry.storage_texture_creator(
            self.texture_names.ping,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            self.texture_names.pong,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        registry.storage_texture_creator(
            self.texture_names.temp,
            gpu_res,
            width,
            height,
            TextureFormat::Rgba32Float,
        );

        if let Some(pattern) = self.do_reset.take() {
            if let (Some(ping), Some(pong)) = (
                registry.get_texture(self.texture_names.ping),
                registry.get_texture(self.texture_names.pong),
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
        let (width, height) = self.rd_sim.rd_size();

        if let Some(pattern) = self.do_reset.take() {
            if let (Some(ping), Some(pong)) = (
                registry.get_texture(self.texture_names.ping),
                registry.get_texture(self.texture_names.pong),
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
                .get_view(self.texture_names.ping)
                .expect("rd ping view is not registered!")
                .clone();
            let pong = registry
                .get_view(self.texture_names.pong)
                .expect("rd pong view is not registered!")
                .clone();
            let temp_view = registry
                .get_view(self.texture_names.temp)
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

        registry.set_view(self.texture_names.output, newest_view);
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        // TODO bring the rebuild pipeline here too
        self.rd_sim.rebuild_pipeline(gpu_res);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
