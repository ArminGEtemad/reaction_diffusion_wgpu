use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

use crate::state::State;

mod gpu_resources;
mod rd_system;
mod shader_watcher;
mod state;

fn main() {
    let event_loop_m = EventLoop::new().expect("Failed to create Event Loop!");
    event_loop_m.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    let _ = event_loop_m.run_app(&mut app);
}

#[derive(Default)]
struct InputState {
    mouse_pos: Option<(f32, f32)>,
    mouse_down: bool,
    brush_radius: f32,
}

struct App {
    window: Option<&'static Window>,
    state: Option<State>,
    input: InputState,
}

// making the Application
impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            state: None,
            input: InputState::default(),
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
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(w) = self.window {
            w.request_redraw();
        }
    }
}
