use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::{BrushUniform, ReactionDiffusionSystem, StartingPattern},
    render_graph::{
        node::{PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};

pub struct ReactionDiffusionNode {
    rd_system: ReactionDiffusionSystem,
}

impl ReactionDiffusionNode {
    pub fn new(gpu_res: &GpuResource) -> Self {
        let rd_system = ReactionDiffusionSystem::new(gpu_res);
        Self { rd_system }
    }

    pub fn reset(&mut self, gpu_res: &GpuResource, pattern: StartingPattern) {
        self.rd_system.reset_and_rerun(gpu_res, pattern);
    }
}

impl RenderNode for ReactionDiffusionNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node"
    }

    fn execute(
        &mut self,
        _registry: &ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parames: &PerFrameParameters,
    ) {
        let (w_rd, h_rd) = self.rd_system.rd_size();

        // brush input
        // Brush doesn't live in state anymore but in execute
        // execute will be used in state
        let mut brush_uniform = BrushUniform {
            c_x: 0.0,
            c_y: 0.0,
            radius: if per_frame_parames.mouse_down {
                per_frame_parames.brush_radius
            } else {
                0.0
            },
            mode: per_frame_parames.mode,
        };

        if per_frame_parames.mouse_down {
            if let Some((mx, my)) = per_frame_parames.mouse_pos {
                let w = gpu_res.size.width as f32;
                let h = gpu_res.size.height as f32;

                if w > 0.0 && h > 0.0 {
                    let nx = (mx / w).clamp(0.0, 1.0);
                    let ny = (my / h).clamp(0.0, 1.0);

                    // y axis is mirrored because of different (0, 0) point

                    brush_uniform.c_x = nx * w_rd as f32;
                    brush_uniform.c_y = (1.0 - ny) * h_rd as f32;
                }
            }
        }

        // upload the brush uniform
        self.rd_system.set_brush_parameters(gpu_res, &brush_uniform);

        self.rd_system
            .compute_and_render_pass(gpu_res, frame, per_frame_parames.paused);
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        self.rd_system.rebuild_pipeline(gpu_res);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
