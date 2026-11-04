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
        println!(">>> Resumed called! Creating window...");
        let attr = Window::default_attributes()
            .with_title("Silent Hill 2 Test Window")
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
            .with_visible(true);
        match event_loop.create_window(attr) {
            Ok(w) => {
                println!(">>> Window created successfully! id={:?}", w.id());
                self.window = Some(Arc::new(w));
            }
            Err(e) => {
                eprintln!(">>> Error creating window: {:?}", e);
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        println!(">>> Event: {:?}", event);
        if event == WindowEvent::CloseRequested {
            el.exit();
        }
    }
}

fn main() {
    println!(">>> Initializing EventLoop...");
    let event_loop = EventLoop::new().unwrap();
    let mut app = App { window: None };
    event_loop.run_app(&mut app).unwrap();
}
