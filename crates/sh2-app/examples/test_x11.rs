use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        println!(">>> EventLoop resumed() called!");
        let attr = Window::default_attributes()
            .with_title("Silent Hill 2 Test Window (Direct)")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_visible(true);
        let w = event_loop.create_window(attr).unwrap();
        println!(">>> Window created! ID: {:?}", w.id());
        self.window = Some(Arc::new(w));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        println!(">>> Window event: {:?}", event);
        if event == WindowEvent::CloseRequested {
            el.exit();
        }
    }
}

fn main() {
    println!(">>> Launching test window...");
    let event_loop = EventLoop::new().unwrap();
    let mut app = App { window: None };
    event_loop.run_app(&mut app).unwrap();
}
