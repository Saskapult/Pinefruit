
use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use wgpu;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use egui;



pub struct GameWindow {
	pub window: Window,
	pub surface: wgpu::Surface,
	pub surface_config: wgpu::SurfaceConfiguration,
	pub platform: egui_winit_platform::Platform,
	pub previous_frame_time: Option<f32>,
}
impl GameWindow {
	pub fn new(instance: &wgpu::Instance, adapter: &wgpu::Adapter, window: Window) -> Self {
		let surface = unsafe { instance.create_surface(&window) };
		let size = window.inner_size();
		let surface_config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
			format: surface.get_preferred_format(&adapter).unwrap(),
			width: size.width,
			height: size.height,
			present_mode: wgpu::PresentMode::Fifo,
		};
		info!("Created new game window with format {:?}", &surface_config.format);

		let platform = egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
			physical_width: size.width as u32,
			physical_height: size.height as u32,
			scale_factor: window.scale_factor(),
			font_definitions: egui::FontDefinitions::default(),
			style: Default::default(),
		});

		Self {
			window,
			surface,
			surface_config,
			platform,
			previous_frame_time: None,
		}
	}

	// To be called by the game when there is a resize event in the queue
	pub fn resize(&mut self, device: &wgpu::Device, new_size: winit::dpi::PhysicalSize<u32>) {
		if new_size.width > 0 && new_size.height > 0 {
			self.surface_config.width = new_size.width;
			self.surface_config.height = new_size.height;
			self.surface.configure(&device, &self.surface_config);		
		}
	}
}


#[derive(Debug)]
pub struct EventWhen {
	pub event: Event<'static, EventLoopEvent>,
	pub when: std::time::Instant,
}


// A custom event which can be injected into the event loop
#[derive(Debug)]
pub enum EventLoopEvent {
	Close,
	CreateWindow,
	RegisterWindow(Window),
	SupplyWindow(mpsc::Sender<Window>),
}


pub fn run_event_loop(
	event_loop: EventLoop<EventLoopEvent>, 
	event_queue: Arc<Mutex<Vec<EventWhen>>>,
) {
	event_loop.run(move |event, event_loop, control_flow| {
		match event {
			Event::UserEvent(event) => {
				match event {
					EventLoopEvent::Close => *control_flow = ControlFlow::Exit,
					EventLoopEvent::CreateWindow => {
						let window = WindowBuilder::new()
							.with_title("window title")
							.build(event_loop)
							.unwrap();
						let ew = EventWhen {
							event: Event::UserEvent(EventLoopEvent::RegisterWindow(window)),
							when: std::time::Instant::now(),
						};
						event_queue.lock().unwrap().push(ew);
					}
					EventLoopEvent::SupplyWindow(sender) => {
						let window = WindowBuilder::new().build(event_loop).unwrap();
						sender.send(window).expect("error sending window");
					}
					_ => {},
				}
			},
			_ => {
				// Not a memory leak because 'static implies that it *can* live forever, not that it does live forever
				if let Some(event) = event.to_static() {
					let ew = EventWhen {
						event,
						when: std::time::Instant::now(),
					};
					event_queue.lock().unwrap().push(ew);
				}
			},
		}
	});
	
}


pub fn new_event_loop() -> EventLoop<EventLoopEvent> {
	EventLoop::<EventLoopEvent>::with_user_event()
}


pub fn new_queue() -> Arc<Mutex<Vec<EventWhen>>> {
	Arc::new(Mutex::new(Vec::<EventWhen>::new()))
}





