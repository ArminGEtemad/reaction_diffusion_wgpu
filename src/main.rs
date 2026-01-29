use crate::{
    rd_system::StartingPattern,
    state::{Side, State},
};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

mod gpu_resources;
mod nodes;
mod rd_system;
mod render_graph;
mod shader_watcher;
mod state;

fn main() {
    let event_loop_m = EventLoop::new().expect("Failed to create Event Loop!");
    event_loop_m.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    let _ = event_loop_m.run_app(&mut app);
}

struct InputState {
    mouse_pos: Option<(f32, f32)>,
    mouse_down: bool,
    brush_radius: f32,
    mode: u32,
    paused: bool,
    debug_mode: bool,
}

struct App {
    window: Option<Arc<Window>>,
    state: Option<State>,
    input: InputState,
    current_starting_pattern_left: StartingPattern,
    current_starting_pattern_right: StartingPattern,
    sim_side: Side,
}

// making the Application
impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            state: None,
            input: InputState::default(),
            current_starting_pattern_left: StartingPattern::Circle,
            current_starting_pattern_right: StartingPattern::Square,
            sim_side: Side::AllSides,
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            mouse_pos: None,
            mouse_down: false,
            brush_radius: 5.0,
            mode: 0,
            paused: false,
            debug_mode: false,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let base_wh = 970.0_f64;

        let attributes = Window::default_attributes()
            .with_title("Reaction-Diffusion in WGPU")
            .with_inner_size(LogicalSize::new(base_wh, base_wh));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("Failed to create window!"),
        );

        // create GPU state
        let state = pollster::block_on(State::new(window.clone())).expect("wgpu init failed!");

        // get the number of simulations
        let n = state.number_of_sims as f64;
        let new_size = LogicalSize::new(base_wh * n, base_wh);
        if let Some(physical_size) = window.as_ref().request_inner_size(new_size) {
            println!("Requested resize applied: {:?}", physical_size);
        } else {
            println!("Resize request deferred or ignored by platform");
        }

        self.window = Some(window);
        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Closing Window Requested!");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(st) = &mut self.state {
                    st.resize(size);
                    println!("Resizing: {:?}", size);
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(st) = &mut self.state {
                    let _ = st.render(&self.input);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_pos = Some((position.x as f32, position.y as f32));
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    self.input.mouse_down = state == ElementState::Pressed;
                }
                println!("Mouse Input: {:?}, {:?}", button, state);
                println!("Mouse Position: {:?}", self.input.mouse_pos);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 1.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.05,
                };

                self.input.brush_radius = (self.input.brush_radius + scroll_d).clamp(1.0, 20.0);
                println!("Brush radius: {:?}", self.input.brush_radius);
            }

            // TODO right now the code assume split screen. I have to get rid of that
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        self.sim_side = Side::Left;
                        println!("Reset side: Left");
                    }

                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        self.sim_side = Side::Right;
                        println!("Reset side: Right");
                    }

                    // Optionally: ArrowUp to go back to "Both"
                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                        self.sim_side = Side::AllSides;
                        println!("Reset Side: Both");
                    }
                    PhysicalKey::Code(KeyCode::Digit1) => {
                        self.input.mode = 0; // add V
                        println!("Add V Mode: {}", self.input.mode);
                    }

                    PhysicalKey::Code(KeyCode::Digit2) => {
                        self.input.mode = 1; // add U
                        println!("Add U Mode: {}", self.input.mode);
                    }

                    PhysicalKey::Code(KeyCode::Digit0) => {
                        self.input.mode = 3; // erase
                        println!("erase {}", self.input.mode);
                    }
                    // TODO I am assuming both sides exist. Add error handling later
                    PhysicalKey::Code(KeyCode::KeyA) => {
                        match self.sim_side {
                            Side::Left => {
                                self.current_starting_pattern_left = StartingPattern::Circle;
                            }
                            Side::Right => {
                                self.current_starting_pattern_right = StartingPattern::Circle;
                            }
                            Side::AllSides => {
                                self.current_starting_pattern_left = StartingPattern::Circle;
                                self.current_starting_pattern_right = StartingPattern::Circle;
                            }
                        }
                        println!(
                            "Patterns: left={:?}, right={:?}",
                            self.current_starting_pattern_left, self.current_starting_pattern_right
                        );
                    }

                    PhysicalKey::Code(KeyCode::KeyB) => {
                        match self.sim_side {
                            Side::Left => {
                                self.current_starting_pattern_left = StartingPattern::Square;
                            }
                            Side::Right => {
                                self.current_starting_pattern_right = StartingPattern::Square;
                            }
                            Side::AllSides => {
                                self.current_starting_pattern_left = StartingPattern::Square;
                                self.current_starting_pattern_right = StartingPattern::Square;
                            }
                        }
                        println!(
                            "Patterns: left={:?}, right={:?}",
                            self.current_starting_pattern_left, self.current_starting_pattern_right
                        );
                    }

                    PhysicalKey::Code(KeyCode::KeyC) => {
                        match self.sim_side {
                            Side::Left => {
                                self.current_starting_pattern_left = StartingPattern::CleanSheet;
                            }
                            Side::Right => {
                                self.current_starting_pattern_right = StartingPattern::CleanSheet;
                            }
                            Side::AllSides => {
                                self.current_starting_pattern_left = StartingPattern::CleanSheet;
                                self.current_starting_pattern_right = StartingPattern::CleanSheet;
                            }
                        }
                        println!(
                            "Patterns: left={:?}, right={:?}",
                            self.current_starting_pattern_left, self.current_starting_pattern_right
                        );
                    }

                    PhysicalKey::Code(KeyCode::KeyP) => {
                        self.input.paused = !self.input.paused;
                        println!("Paused: {}", self.input.paused);
                    }

                    PhysicalKey::Code(KeyCode::KeyR) => {
                        if let Some(st) = &mut self.state {
                            // needs to be updated because of Side enum
                            st.reset(
                                self.current_starting_pattern_left,
                                self.current_starting_pattern_right,
                                self.sim_side,
                            );
                            println!(
                                "Simulation {:?} restarted with the starting pattern: left={:?}, right={:?}",
                                self.sim_side,
                                self.current_starting_pattern_left,
                                self.current_starting_pattern_right
                            );
                        }
                    }

                    PhysicalKey::Code(KeyCode::KeyD) => {
                        self.input.debug_mode = !self.input.debug_mode;
                        println!("Debug mode: {}", self.input.debug_mode);
                    }

                    _ => {}
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
