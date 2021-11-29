

use specs::prelude::*;
use specs::{Component, VecStorage};
use nalgebra::*;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use std::sync::{Arc, Mutex};

use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use crate::window::*;
use std::sync::mpsc;
use std::thread;



// Manages windows
// Creates/destroys windows and can read their events
const CLOSE_ON_NO_WINDOWS: bool = true;
struct WindowResource {
	windows: Vec<GameWindow>,
	id_idx: HashMap<WindowId, usize>,
	event_loop_sender: mpsc::SyncSender<EventLoopEvent>,
	event_thread_handle: thread::JoinHandle<i32>,
	instance: wgpu::Instance,
	adapter: wgpu::Adapter,
	event_queue: Arc<Mutex<Vec<EventWhen>>>,
}
impl WindowResource {
	pub fn new(
		event_loop_proxy: EventLoopProxy<EventLoopEvent>,
		instance: wgpu::Instance,
		adapter: wgpu::Adapter,
		event_queue: Arc<Mutex<Vec<EventWhen>>>,
	) -> Self {
		let windows = Vec::new();
		let id_idx = HashMap::new();

		let (event_loop_sender, event_loop_receiver) = mpsc::sync_channel(10);

		// This needs to stay in its own thread because it cannot sync
		let event_thread_handle = thread::spawn(move || {
			loop {
				let event = event_loop_receiver.recv()
					.expect("recv error");
				event_loop_proxy.send_event(event)
					.expect("Could not send window creation request");
			}
		});

		Self {
			windows,
			id_idx,
			event_loop_sender,
			event_thread_handle,
			instance,
			adapter,
			event_queue,
		}
	}


	pub fn request_window(&mut self) {
		// Insert an event to create a new window
		// A RegisterWindow event with the window should be inserted into the queue
		// It should be processed in the next iteration
		self.event_loop_sender.send(EventLoopEvent::CreateWindow)
			.expect("Could not send window creation request");
	}


	pub fn request_window_immediate(&mut self) {
		let (sender, receiver) = mpsc::channel();
		self.event_loop_sender.send(EventLoopEvent::SupplyWindow(sender))
			.expect("Could not send window creation request");
		let window = receiver.recv().unwrap();
		self.register_window(window);
	}


	pub fn register_window(&mut self, window: Window) -> usize {
		let gamewindow = GameWindow::new(&self.instance, &self.adapter, window);

		let idx = self.windows.len();
		self.id_idx.insert(gamewindow.window.id(), idx);
		self.windows.push(gamewindow);
		idx
	}


	pub fn close_window(&mut self, idx: usize) {
		let wid = self.windows[idx].window.id();
		self.id_idx.remove(&wid);
		self.windows.remove(idx);
		// Dropping the value should cause the window to close

		if CLOSE_ON_NO_WINDOWS && self.windows.len() == 0 {
			self.shutdown();
		}
	}


	// Due to this aborting the event loop, the game will also be dropped
	pub fn shutdown(&mut self) {
		// Drop all windows
		for i in 0..self.windows.len() {
			self.close_window(i);
		}
		// Shut down event loop
		self.event_loop_sender.send(EventLoopEvent::Close)
			.expect("Could not send event loop close request");
		// Everything else should stop/drop when self is dropped (here)
	}
	

	// Processes events
	// Gets the press percentages for all keys pressed during a timestep
	// It is possible for a percentage to be greater than 100%
	// This happends if startt is after the earliest queue value
	// Should probably be modified to return more than that
	pub fn process(
		&mut self, 
		startt: Instant,
		endt: Instant,
	) -> HashMap<VirtualKeyCode, f32> {

		let dt = (endt - startt).as_secs_f32();
		let mut pressmap = HashMap::new();
		let mut kpmap = HashMap::new();
		// Drain items not passed the start of the step
		let events: Vec<EventWhen> = self.event_queue.lock().unwrap().drain_filter(|e| e.ts < endt).collect();
		for event in events {
			let ts = event.ts;
			match event.event {
				Event::UserEvent(event) => {
					match event {
						EventLoopEvent::RegisterWindow(window) => {
							self.register_window(window);
						},
						_ => {},
					}
				},
				Event::WindowEvent {ref event, window_id} => {
					match event {
						WindowEvent::KeyboardInput {input, ..} => {
							// Send changed key data
							let key = input.virtual_keycode.expect("no virtual keycode?!");
							match input.state {
								ElementState::Pressed => {
									// If this button was not already pressed, record the pressing
									if !pressmap.contains_key(&key) {
										pressmap.insert(key, ts);
									}
								},
								ElementState::Released => {
									// Only do something if this key had been pressed in the first place
									if kpmap.contains_key(&key) {
										let mut kp = (ts - pressmap[&key]).as_secs_f32() / dt;
		
										// If this key had been pressed and released, account for that
										if kpmap.contains_key(&key) {
											kp += kpmap[&key];
										}
										// Send the percent of time pressed to input
										kpmap.insert(key, kp);
		
										// Remove key from pressed keys
										kpmap.remove(&key);
									}
									
								},
							}
						},
						WindowEvent::MouseInput {state, button, ..} => {
							// Mouse button presses
						},
						WindowEvent::MouseWheel {delta, phase, ..} => {
						},
						WindowEvent::CursorEntered {..} => {
						},
						WindowEvent::CursorLeft {..} => {
						},
						WindowEvent::CursorMoved {position, ..} => {
						},
						WindowEvent::Resized (newsize) => {
							//self.windows[self.id_idx[&window_id]].resize(newsize);
						},
						WindowEvent::CloseRequested => {
							self.close_window(self.id_idx[&window_id]);
						},
						_ => {},
					}
				}
				_ => {},
			}
		}

		// Process the keys which are still pressed
		for (key, t) in &pressmap {
			let mut kp = (endt - *t).as_secs_f32() / dt;
			// If this key had been pressed and released, account for that
			if kpmap.contains_key(&key) {
				kp += kpmap[&key];
			}
			// Send the percent of time pressed to input
			kpmap.insert(*key, kp);
		}

		kpmap
    }
}



// Holds input data
struct InputResource {
	keys: HashMap<VirtualKeyCode, f32>, // Percentage of step time a key was pressed
	// controlmap: HashMap<VirtualKeyCode, (some kind of enum option?)>
}
impl InputResource {
	pub fn new() -> Self {
		let keys = HashMap::new();
		Self {
			keys
		}
	}
}



// Holds timestep data
struct StepResource {
	last_step: Instant, // Time of last step
	this_step: Instant, // Time of current step
	step_diff: Duration, // this-last
}
impl StepResource {
	pub fn new() -> Self {
		let heh = Instant::now();
		Self {
			last_step: heh,
			this_step: heh, 
			step_diff: heh - heh,
		}
	}
}



use crate::render::Renderer;
struct RenderResource {
	renderer: Renderer,
}
impl RenderResource {
	pub fn new(adapter: &wgpu::Adapter) -> Self {

		let renderer = pollster::block_on(Renderer::new(adapter));

		Self {
			renderer,
		}		
	}
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
struct TransformComponent {
	position: Vector3<f32>,
	rotation: UnitQuaternion<f32>,
	scale: Vector3<f32>,
}
impl TransformComponent {
	pub fn new() -> Self {
		let position = Vector3::new(0.0, 0.0, 0.0);
		// let rotation = UnitQuaternion::look_at_lh(
		// 	&Vector3::new(0.0, 0.0, 1.0),
		// 	&Vector3::new(0.0, 1.0, 0.0),
		// );
		let rotation = UnitQuaternion::identity();
		let scale = Vector3::new(1.0, 1.0, 1.0);
		
		Self {
			position,
			rotation,
			scale,
		}
	}
	pub fn with_position(self, position: Vector3<f32>) -> Self {
		let rotation = self.rotation;
		let scale = self.scale;
		Self {
			position,
			rotation,
			scale,
		}
	}
	pub fn with_rotation(self, rotation: UnitQuaternion<f32>) -> Self {
		let position = self.position;
		let scale = self.scale;
		Self {
			position,
			rotation,
			scale,
		}
	}
	pub fn with_scale(self, scale: Vector3<f32>) -> Self {
		let position = self.position;
		let rotation = self.rotation;
		Self {
			position,
			rotation,
			scale,
		}
	}
	pub fn matrix(&self) -> Matrix4<f32> {
		Matrix4::new_nonuniform_scaling(&self.scale) * self.rotation.to_homogeneous() * Matrix4::new_translation(&self.position)
	}
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
struct MovementComponent {
	speed: f32,	// Units per second
}
impl MovementComponent {
	pub fn new() -> Self {
		let speed = 1.0;
		MovementComponent {
			speed,
		}
	}
	pub fn with_speed(self, speed: f32) -> Self {
		Self {
			speed,
		}
	}
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
struct RenderComponent {
	mesh_id: usize,
	texture_id: usize,
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
struct CameraComponent {
	target: RenderTarget,
	fovy: f32,
	znear: f32,
	zfar: f32,
}
impl CameraComponent {
	pub fn new() -> Self {
		Self {
			target: RenderTarget::Window(0),
			fovy: 45.0,
			znear: 0.1,
			zfar: 100.0,
		}
	}
}



// Handles window events
// Prepares the input resource
struct WindowEventSystem;
impl<'a> System<'a> for WindowEventSystem {
    type SystemData = (
		WriteExpect<'a, WindowResource>,
		WriteExpect<'a, InputResource>,
		ReadExpect<'a, StepResource>
	);

    fn run(
		&mut self, 
		(
			mut window_resource, 
			mut input_resource, 
			step_resource
		): Self::SystemData
	) {
		// Put in input resource
		let mut keymap = window_resource.process(step_resource.last_step, step_resource.this_step);
		input_resource.keys = keymap;
    }
}



struct InputSystem;
impl<'a> System<'a> for InputSystem {
    type SystemData = (
		WriteExpect<'a, InputResource>,
		ReadExpect<'a, StepResource>,
		WriteStorage<'a, TransformComponent>,
		ReadStorage<'a, MovementComponent>,
	);

	fn run(
		&mut self, 
		(
			mut input_resource, 
			step_resource,
			mut transform, 
			movement
		): Self::SystemData
	) { 
		let mut displacement = Vector3::new(0.0, 0.0, 0.0);
		for (key, kp) in &input_resource.keys {
			match key {
				VirtualKeyCode::W => {
					displacement.z += kp;
				},
				VirtualKeyCode::S => {
					displacement.z -= kp;
				},
				VirtualKeyCode::D => {
					displacement.x += kp;
				},
				VirtualKeyCode::A => {
					displacement.x -= kp;
				},
				VirtualKeyCode::Space => {
					displacement.y += kp;
				},
				VirtualKeyCode::LShift => {
					displacement.y -= kp;
				},
				_ => {},
			}
		}
		input_resource.keys.clear();

		let secs = step_resource.step_diff.as_secs_f32();

		for (transform_c, movement_c) in (&mut transform, &movement).join() {
			transform_c.position += displacement * movement_c.speed * secs;
			info!("Camera position: {:?}", &transform_c.position);
		}
	}
}



struct RenderSystem;
impl<'a> System<'a> for RenderSystem {
    type SystemData = (
		WriteExpect<'a, RenderResource>,
		ReadExpect<'a, WindowResource>,
		ReadStorage<'a, RenderComponent>,
		ReadStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
	);

	fn run(
		&mut self, 
		(
			mut render_resource, 
			window_resource,
			render, 
			camera,
			transform,
		): Self::SystemData,
	) { 
		for (camera_c, transform_c) in (&camera, &transform).join() {
			// Find destination
			let view;
			let width;
			let height;
			match camera_c.target {
				RenderTarget::Window(id) => {
					if id < window_resource.windows.len() {
						info!("Rendering to window idx {}", id);

						// Get window
						let window = &window_resource.windows[id];
						window.surface.configure(&render_resource.renderer.device, &window.surface_config);
						width = window.surface_config.width;
						height = window.surface_config.height;
						let stex = window.surface.get_current_texture().expect("fugg");
						view = stex.texture.create_view(&wgpu::TextureViewDescriptor::default());
					
						// Collect render data
						let mut data = Vec::new();
						for (render_c, transform_c) in (&render, &transform).join() {
							data.push(crate::render::RenderData {
								mesh_id: render_c.mesh_id,
								instance: crate::render::Instance::new(),
							});
						}

						let camera = crate::render::Camera {
							position: transform_c.position,
							rotation: transform_c.rotation,
							fovy: camera_c.fovy,
							znear: camera_c.znear,
							zfar: camera_c.zfar,
						};

						info!("Rendering at {:?}", &camera.position);

						render_resource.renderer.render(&view, width, height, &camera, &data);

						stex.present();
					} else {
						error!("Tried to render to nonexistent window! idx: {}", id);
					}
				},
				RenderTarget::Texture(id) => {
					todo!();
				},
			}

			
			
		}
	}
}



pub struct Game {
	world: World,
	dispatcher: Dispatcher<'static, 'static>,
}
impl Game {
	pub fn new(
		event_loop_proxy: EventLoopProxy<EventLoopEvent>, 
		event_queue: Arc<Mutex<Vec<EventWhen>>>,
	) -> Self {
		let instance = wgpu::Instance::new(wgpu::Backends::all());
		let adapter = pollster::block_on(instance.request_adapter(
			&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::HighPerformance, // Dedicated GPU
				compatible_surface: None, // Some(&surface)
				force_fallback_adapter: false, // Don't use software renderer
			},
		)).unwrap();

		let adapter_info = adapter.get_info();
		info!("Kkraft using device {} ({:?})", adapter_info.name, adapter_info.backend);
		info!("Features: {:?}", adapter.features());
		info!("Limits: {:?}", adapter.limits());

		let mut world = World::new();

		// Register components
		world.register::<TransformComponent>();
		world.register::<MovementComponent>();
		world.register::<RenderComponent>();
		world.register::<CameraComponent>();

		// Attach resources
		let step_resource = StepResource::new();
		world.insert(step_resource);

		let mut render_resource = RenderResource::new(&adapter);
		// Add meshes
		let pentagon_id = render_resource.renderer.add_mesh(
			&"pentagon".to_string(),
			crate::geometry::Mesh::pentagon(&render_resource.renderer.device),
		);
		let quad_id = render_resource.renderer.add_mesh(
			&"quad".to_string(),
			crate::geometry::Mesh::quad(&render_resource.renderer.device),
		);
		// Add textures
		render_resource.renderer.load_texture_disk(
			&"dirt".to_string(), 
			&"resources/blockfaces/dirt.png".to_string(),
		);
		render_resource.renderer.recreate_tbg();
		world.insert(render_resource);

		let window_resource = WindowResource::new(
			event_loop_proxy,
			instance,
			adapter,
			event_queue,
		);
		world.insert(window_resource);
		let input_resource = InputResource::new();
		world.insert(input_resource);


		// Entities
		// Camera
		world.create_entity()
			.with(CameraComponent::new())
			.with(TransformComponent::new().with_position(Vector3::new(0.0, 0.0, -1.0)))
			.with(MovementComponent{speed: 0.4})
			.build();
		// Mesh
		world.create_entity()
			.with(TransformComponent::new())
			.with(RenderComponent {
				mesh_id: quad_id,
				texture_id: 1,
			})
			.build();

		// Called to tick the world
		let dispatcher = DispatcherBuilder::new()
			.with(WindowEventSystem, "window_system", &[])
			.with(InputSystem, "input_system", &["window_system"])
			.with(RenderSystem, "render_system", &["input_system"])
			.build();


		Self {
			world,
			dispatcher,
		}
	}


	pub fn tick(&mut self) {
		info!("Tick!");
		// Prepare step info
		{
			let mut step_resource = self.world.write_resource::<StepResource>();
			step_resource.last_step = step_resource.this_step;
			step_resource.this_step = std::time::Instant::now();
			step_resource.step_diff = step_resource.this_step - step_resource.last_step;
		}

		// Step to it
		self.dispatcher.dispatch(&mut self.world);
		info!("Tock!");
	}


	pub fn new_window(&mut self) {
		info!("Requesting new game window");

		let mut window_resource = self.world.write_resource::<WindowResource>();
		window_resource.request_window();

	}

}



#[derive(Debug)]
enum RenderTarget {
	Window(usize),
	Texture(usize),
}