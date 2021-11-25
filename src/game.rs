
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


// Resources

// A resource is available to all systems
// Use WriteExpect because Default is hard to do

struct WindowResource {
	windows: Vec<GameWindow>,
	id_idx: HashMap<WindowId, usize>,
	process_queue: Vec<EventWhen>,
}
impl WindowResource {
	pub fn new() -> Self {
		let windows = Vec::new();
		let id_idx = HashMap::new();
		let process_queue = Vec::new();
		Self {
			windows,
			id_idx,
			process_queue,
		}
	}

	pub fn register_window(&mut self, window: GameWindow) {
		let idx = self.windows.len();
		self.id_idx.insert(window.window.id(), idx);
		self.windows.push(window);
	}
}


struct InputResource {
	keys: HashMap<VirtualKeyCode, f32>, // Percentage of step time a key was pressed
	// control mapping hashmap?
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


struct SimulationResource {
	last_step: Instant, // Time of last step
	this_step: Instant, // Time of current step
	step_diff: Duration, // this-last
}
impl SimulationResource {
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
	pub fn new(instance: &wgpu::Instance, adapter: &wgpu::Adapter) -> Self {

		let mut renderer = pollster::block_on(Renderer::new(adapter));

		// Make pipeline and prereqs
		renderer.create_tarray_bgl(&"tarray_bgl".to_string(), 1024);
		renderer.create_camera_bgl(&"camera_bgl".to_string());
		renderer.create_pipeline_layout(
			&"tarray_pipeline_layout".to_string(), 
			&vec![&"tarray_bgl".to_string(), &"camera_bgl".to_string()],
		);
		renderer.create_pipeline(
			&"tarray_pipeline".to_string(), 
			&"../shaders/texture_array_shader.wgsl".to_string(), 
			&"tarray_pipeline_layout".to_string(), 
			wgpu::TextureFormat::Rgba8UnormSrgb,
			Some(crate::texture::Texture::DEPTH_FORMAT),
			&[crate::geometry::Vertex::desc()],
		);

		// Add textures
		renderer.load_texture_disk(&"dirt".to_string(), &"resources/blockfaces/dirt.png".to_string());
		renderer.create_tarray_bg(
			&"tarray_bg".to_string(), 
			&(0..renderer.textures.data.len()).collect(),
			&"tarray_bgl".to_string(),
		);

		//let crp = crate::render::make_crp(&renderer.device, &renderer.texture_manager.bind_group_layout, &renderer.camera_bind_group_layout);

		Self {
			renderer,
		}		
	}
}



// Components

#[derive(Component, Debug)]
#[storage(VecStorage)]
struct TransformComponent {
	position: Vector3<f32>,
	rotation: Vector3<f32>,
	scale: Vector3<f32>,
}
impl TransformComponent {
	pub fn new() -> Self {
		let v = Vector3::new(0.0, 0.0, 0.0);
		Self {
			position: v,
			rotation: v,
			scale: v,
		}
	}
}

#[derive(Component, Debug)]
#[storage(VecStorage)]
struct MovementComponent {
	speed: f32,
}


// Systems

// Accounts for percentage of time a key was pressed for since the last time step
// I think it's neat and will make movements more precise

// Still collects input from after start of next timestep, breaks thing
struct WindowEventSystem;
impl<'a> System<'a> for WindowEventSystem {
    type SystemData = (
		WriteExpect<'a, WindowResource>,
		WriteExpect<'a, InputResource>,
		ReadExpect<'a, SimulationResource>
	);

	// Distribute events to window queues?
    fn run(&mut self, (mut window_resource, mut input_resource, simulation_resource): Self::SystemData) {

		// Pressed keys for this timestep
		let mut keymap = HashMap::new();
		
		// Duration of this timestep
		let dt = simulation_resource.step_diff.as_secs_f32();

		for event in &window_resource.process_queue {
			match &event.event {
				WindowEvent::Resized(physical_size) => {
					//renderer_resource.size = physical_size;
				},
				WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
					//renderer_resource.size = *new_inner_size;
				},
				WindowEvent::KeyboardInput {input, ..} => {
					// Send changed key data
					let key = input.virtual_keycode.expect("no virtual keycode?!");
					match input.state {
						ElementState::Pressed => {
							trace!("key pressed: {:?}", &key);
							// If this button was not already pressed, record the pressing
							if !keymap.contains_key(&key) {
								keymap.insert(key, event.ts);
							}
						},
						ElementState::Released => {
							trace!("key released: {:?}", &key);
							// Only do something if this key had been pressed in the first place
							if keymap.contains_key(&key) {
								let mut kp = (event.ts - keymap[&key]).as_secs_f32() / dt;

								// If this key had been pressed and released, account for that
								if input_resource.keys.contains_key(&key) {
									kp += input_resource.keys[&key];
								}
								// Send the percent of time pressed to input
								input_resource.keys.insert(key, kp);

								// Remove key from pressed keys
								keymap.remove(&key);
							}
							
						},
					}
				},
				_ => {},
			}
		}
		
		// Process the keys which are still pressed
		for (key, t) in &keymap {
			let mut kp = (simulation_resource.this_step - *t).as_secs_f32() / dt;
			// If this key had been pressed and released, account for that
			if input_resource.keys.contains_key(&key) {
				kp += input_resource.keys[&key];
			}
			// Send the percent of time pressed to input
			input_resource.keys.insert(*key, kp);
		}

    }
}

struct InputSystem;
impl<'a> System<'a> for InputSystem {
    type SystemData = (
		WriteExpect<'a, InputResource>,
		WriteStorage<'a, TransformComponent>,
		ReadStorage<'a, MovementComponent>,
	);

	fn run(&mut self, (mut input_resource, mut transform, movement): Self::SystemData) { 

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

		for (transform_c, movement_c) in (&mut transform, &movement).join() {
			transform_c.position += displacement * movement_c.speed;
			//println!("Position: {:?}", &transform_c.position);
		}
	}
}



pub struct Game {
	instance: wgpu::Instance,
	adapter: wgpu::Adapter,

	world: World,
	simulation_dispatcher: Dispatcher<'static, 'static>,
	event_loop_proxy: EventLoopProxy<EventLoopEvent>,
	window_event_queue: Arc<Mutex<Vec<EventWhen>>>,
}
impl Game {
	pub fn new(
		event_loop_proxy: EventLoopProxy<EventLoopEvent>, 
		window_event_queue: Arc<Mutex<Vec<EventWhen>>>,
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


		// Attach resources

		let simulation_resource = SimulationResource::new();
		world.insert(simulation_resource);

		let window_resource = WindowResource::new();
		world.insert(window_resource);

		let input_resource = InputResource::new();
		world.insert(input_resource);

		let render_resource = RenderResource::new(&instance, &adapter);
		world.insert(render_resource);

		
		// Enitities

		world.create_entity()
			.with(TransformComponent::new())
			.with(MovementComponent{speed: 0.4})
			.build();

		// Called to tick the world
		let simulation_dispatcher = DispatcherBuilder::new()
			.with(WindowEventSystem, "window_system", &[])
			.with(InputSystem, "input_system", &["window_system"])
			.build();


		Self {
			instance,
			adapter,
			world,
			simulation_dispatcher,
			event_loop_proxy,
			window_event_queue,
		}
	}

	pub fn simulation_tick(&mut self) {
		
		// Fetch data from queue (in new scope because of mutex paranoia)
		// Must be retreived before this_step is decided because events might be recorded after this_step and cause an error
		{
			let mut window_resource = self.world.write_resource::<WindowResource>();
			window_resource.process_queue.clear();
			window_resource.process_queue.append(&mut self.window_event_queue.lock().unwrap());
		}

		// Prepare simulation step info
		{
			let mut simulation_resource = self.world.write_resource::<SimulationResource>();
			simulation_resource.last_step = simulation_resource.this_step;
			simulation_resource.this_step = std::time::Instant::now();
			simulation_resource.step_diff = simulation_resource.this_step - simulation_resource.last_step;
		}
		self.simulation_dispatcher.dispatch(&mut self.world);

	}

	pub fn new_window(&mut self) {
		info!("Creating new game window");

		let (sender, receiver) = mpsc::channel();

		self.event_loop_proxy.send_event(EventLoopEvent::NewWindow(sender))
			.expect("Could not send window creation event");

		let window = receiver.recv().unwrap();
		let gamewindow = GameWindow::new(&self.instance, &self.adapter, window);

		self.render(&gamewindow);

		info!("Created new game window with format {:?}", &gamewindow.surface_config.format);

		let mut window_resource = self.world.write_resource::<WindowResource>();
		window_resource.register_window(gamewindow);

		
		
	}

	pub fn render(&mut self, window: &GameWindow) {
		let tf = self.world.read_storage::<TransformComponent>();
		let mo = self.world.read_storage::<MovementComponent>();

		let mut render_resource = self.world.write_resource::<RenderResource>();
	
		window.surface.configure(&render_resource.renderer.device, &window.surface_config);
		let stex = window.surface.get_current_texture().expect("fugg");
		let view = stex.texture.create_view(&wgpu::TextureViewDescriptor::default());

		let width = window.surface_config.width;
		let height = window.surface_config.height;

		let camera = crate::render::camera::Camera::new(
			Vector3::new(0.0, 0.0, 0.0),
			UnitQuaternion::look_at_lh(
				&Vector3::new(0.0, 0.0, 1.0),
				&Vector3::new(0.0, 1.0, 0.0),
			),
			45.0,
			0.1,
			100.0,
		);

		render_resource.renderer.render(&view, width, height, &camera, &vec![]);

		stex.present();
		info!("prensented");
	}
}

