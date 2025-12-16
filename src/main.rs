use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{rd_system::StartingPattern, state::State};

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
    window: Option<&'static Window>,
    state: Option<State>,
    input: InputState,
    current_starting_pattern: StartingPattern,
}

// making the Application
impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            state: None,
            input: InputState::default(),
            current_starting_pattern: StartingPattern::Circle,
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
        let attributes = Window::default_attributes()
            .with_title("Reaction-Diffusion in WGPU")
            .with_inner_size(LogicalSize::new(970.0_f64, 970.0_f64));
        let window = event_loop
            .create_window(attributes)
            .expect("Failed to create window!");

        // I cheated here to get the window stay open by leaking it
        // TODO: is this the correct way to handle it?
        let window: &'static Window = Box::leak(Box::new(window));

        // create GPU state
        let state = pollster::block_on(State::new(window)).expect("wgpu init failed!");
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

            // TODO test it on the laptop later
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 1.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.05,
                };

                self.input.brush_radius = (self.input.brush_radius + scroll_d).clamp(1.0, 20.0);
                println!("Brush radius: {:?}", self.input.brush_radius);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
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

                    PhysicalKey::Code(KeyCode::KeyA) => {
                        self.current_starting_pattern = StartingPattern::Circle;
                        println!(
                            "The starting pattern has been changed to: {:?}",
                            self.current_starting_pattern
                        );
                    }

                    PhysicalKey::Code(KeyCode::KeyB) => {
                        self.current_starting_pattern = StartingPattern::Square;
                        println!(
                            "The starting pattern has been changed to: {:?}",
                            self.current_starting_pattern
                        );
                    }

                    PhysicalKey::Code(KeyCode::KeyC) => {
                        self.current_starting_pattern = StartingPattern::CleanSheet;
                        println!(
                            "The starting pattern has been changed to: {:?}",
                            self.current_starting_pattern
                        );
                    }

                    PhysicalKey::Code(KeyCode::KeyP) => {
                        self.input.paused = !self.input.paused;
                        println!("Paused: {}", self.input.paused);
                    }

                    PhysicalKey::Code(KeyCode::KeyR) => {
                        if let Some(st) = &mut self.state {
                            st.reset(self.current_starting_pattern);
                            println!(
                                "Simulation restarted with the starting pattern: {:?}",
                                self.current_starting_pattern
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
        if let Some(w) = self.window {
            w.request_redraw();
        }
    }
}
