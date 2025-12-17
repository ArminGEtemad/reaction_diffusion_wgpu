use std::{cell::RefCell, rc::Rc};

use crate::{
    gpu_resources::{FrameContext, GpuResource},
    rd_system::{BrushUniform, ReactionDiffusionSystem, StartingPattern},
    render_graph::{
        node::{PerFrameParameters, RenderNode},
        resource_registry::ResourceRegistry,
    },
};

// RC allows for different owners of the same data
// RefCell allows to mutate data even if they are immutable
type ReactionDiffusionShared = Rc<RefCell<ReactionDiffusionSystem>>;

pub struct ReactionDiffusionSimulationNode {
    rd_shared: ReactionDiffusionShared,
}

pub struct ReactionDiffusionDisplayNode {
    rd_shared: ReactionDiffusionShared,
}

pub fn create_rd_shared_nodes(
    gpu_res: &GpuResource,
) -> (
    ReactionDiffusionSimulationNode,
    ReactionDiffusionDisplayNode,
) {
    let shared = Rc::new(RefCell::new(ReactionDiffusionSystem::new(gpu_res)));
    let sim = ReactionDiffusionSimulationNode {
        rd_shared: Rc::clone(&shared),
    };
    let display = ReactionDiffusionDisplayNode { rd_shared: shared };

    (sim, display)
}

impl ReactionDiffusionSimulationNode {
    pub fn reset(&mut self, gpu_res: &GpuResource, pattern: StartingPattern) {
        self.rd_shared
            .borrow_mut()
            .reset_and_rerun(gpu_res, pattern);
    }
}

// Compute Only (+ mouse injection)
impl RenderNode for ReactionDiffusionSimulationNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (Simulation)"
    }

    fn execute(
        &mut self,
        _registry: &ResourceRegistry,
        gpu_res: &GpuResource,
        frame: &mut FrameContext,
        per_frame_parames: &PerFrameParameters,
    ) {
        let (w_rd, h_rd) = {
            // new scope
            let rd_shared = self.rd_shared.borrow();
            rd_shared.rd_size()
        };

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
        {
            let rd_shared = self.rd_shared.borrow();
            rd_shared.set_brush_parameters(gpu_res, &brush_uniform);
        }

        // do the compute
        {
            let mut rd_shared = self.rd_shared.borrow_mut();
            rd_shared.step_simulation(gpu_res, frame, per_frame_parames.paused);
        }
    }

    fn called_on_hotreload(&mut self, gpu_res: &GpuResource) {
        // TODO the pipeline rebuild does the whole pipeline rebuild compute and render
        // would that be a problem if I don't separate them?
        self.rd_shared.borrow_mut().rebuild_pipeline(gpu_res);
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// Render
impl RenderNode for ReactionDiffusionDisplayNode {
    fn name(&self) -> &str {
        "Reaction Diffusion Node (Display)"
    }

    fn execute(
        &mut self,
        _registry: &ResourceRegistry,
        _gpu_res: &GpuResource,
        frame: &mut FrameContext,
        _per_frame_parames: &PerFrameParameters,
    ) {
        // it just renders actually
        let rd_shared = self.rd_shared.borrow();

        let view = frame.view.clone();
        rd_shared.step_render(frame, &view);
    }

    fn called_on_hotreload(&mut self, _gpu_res: &GpuResource) {
        // already done in compute one
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
