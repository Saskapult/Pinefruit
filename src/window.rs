
use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use wgpu;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;


pub struct GameWindow {
	pub window: Window,
	pub surface: wgpu::Surface,
	pub surface_config: wgpu::SurfaceConfiguration,
}
impl GameWindow {
	pub fn new(instance: &wgpu::Instance, adapter: &wgpu::Adapter, event_loop: &EventLoop<()>) -> Self {
		let window = new_window(&event_loop);
		Self::from_window(instance, adapter, window)
	}

	pub fn from_window(instance: &wgpu::Instance, adapter: &wgpu::Adapter, window: Window) -> Self {
		let surface = unsafe { instance.create_surface(&window) };
		let size = window.inner_size();
		let surface_config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: surface.get_preferred_format(&adapter).unwrap(),
			width: size.width,
			height: size.height,
			present_mode: wgpu::PresentMode::Fifo,
		};
		
		Self {
			window,
			surface,
			surface_config,
		}
	}

	// To be called by the game when there is a resize event in the queue
	pub fn resize(&mut self, device: &wgpu::Device, new_size: winit::dpi::PhysicalSize<u32>) {
		if new_size.width > 0 && new_size.height > 0 {
			self.surface_config.width = new_size.width;
			self.surface_config.height = new_size.height;
			self.surface.configure(&device, &self.surface_config);
			// Depth texture?			
		}
	}
}








#[derive(Debug)]
pub struct EventWhen {
	pub window_id: WindowId,
	pub event: WindowEvent<'static>,
	pub ts: std::time::Instant,
}






pub fn new_event_loop() -> EventLoop<EventLoopEvent> {
	EventLoop::<EventLoopEvent>::with_user_event()
}

pub fn new_window(event_loop: &EventLoop<()>) -> Window {
	WindowBuilder::new()
		.build(event_loop)
		.unwrap()
}

pub fn new_queue() -> Arc<Mutex<Vec<EventWhen>>> {
	Arc::new(Mutex::new(Vec::<EventWhen>::new()))
}

#[derive(Debug)]
pub enum EventLoopEvent {
	Close,
	NewWindow(mpsc::Sender<Window>),
}

// Could use custom event to send cf::exit from within the program
pub fn run_event_loop(
	event_loop: EventLoop<EventLoopEvent>, 
	event_queue: Arc<Mutex<Vec<EventWhen>>>,
) {
	event_loop.run(move |event, event_loop, control_flow| {
		match event {
			Event::UserEvent(event) => {
				match event {
					EventLoopEvent::Close => *control_flow = ControlFlow::Exit,
					EventLoopEvent::NewWindow(sender) => {
						let window = WindowBuilder::new().build(event_loop).unwrap();
						sender.send(window).expect("Failed to send window");
					}
					_ => {},
				}
				
			}
			Event::WindowEvent {
				event,
				window_id,
			} => match event {
				WindowEvent::CloseRequested  => *control_flow = ControlFlow::Exit,
				_ => {
					if let Some(event) = event.to_static() {
						let g = EventWhen {
							window_id,
							event,
							ts: std::time::Instant::now(),
						};
						event_queue.lock().unwrap().push(g);
					}
				},
			}
			_ => {},
		}
	});
	
}






