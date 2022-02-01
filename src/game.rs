
use specs::prelude::*;
use specs::{Component, VecStorage};
use nalgebra::*;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use std::sync::{Arc, Mutex, RwLock};

use winit::{
	event::*,
	event_loop::*,
	window::*,
};
use crate::window::*;
use crate::world::Voxel;
use std::sync::mpsc;
use std::thread;
use std::path::PathBuf;
use crate::render::Renderer;



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

	// Request a window (to be processed in the next iteration)
	pub fn request_window(&mut self) {
		// Insert an event to create a new window
		// A RegisterWindow event with the window should be inserted into the queue
		// It should be processed in the next iteration
		self.event_loop_sender.send(EventLoopEvent::CreateWindow)
			.expect("Could not send window creation request");
	}

	// Get a window fastly (in this iteration)
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
		// Everything else *should* stop/drop when self is dropped (here)
	}
}



// Holds input data
struct InputResource {
	// The press percentages for all keys pressed during a timestep
	// It is possible for a percentage to be greater than 100%
	// This happends if startt is after the earliest queue value
	board_keys: HashMap<VirtualKeyCode, f32>,
	board_presscache: Vec<VirtualKeyCode>,
	mouse_keys: HashMap<MouseButton, f32>,
	mouse_presscache: Vec<MouseButton>,
	mx: f64,
	my: f64,
	mdx: f64,
	mdy: f64,
	// controlmap: HashMap<VirtualKeyCode, (some kind of enum option?)>
}
impl InputResource {
	pub fn new() -> Self {
		let board_keys = HashMap::new();
		let board_presscache = Vec::new();
		let mouse_keys = HashMap::new();
		let mouse_presscache = Vec::new();
		let mx = 0.0;
		let my = 0.0;
		let mdx = 0.0;
		let mdy = 0.0;
		Self {
			board_keys,
			board_presscache,
			mouse_keys,
			mouse_presscache,
			mx, 
			my, 
			mdx, 
			mdy
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



// Todo: add a render queue which other systems write to and is used by the render system
struct RenderResource {
	pub renderer: Renderer,
	materials_manager: Arc<RwLock<crate::render::MaterialManager>>,
	textures_manager: Arc<RwLock<crate::render::TextureManager>>,
	meshes_manager: Arc<RwLock<crate::render::MeshManager>>,
}
impl RenderResource {
	pub fn new(adapter: &wgpu::Adapter) -> Self {

		let textures_manager = Arc::new(RwLock::new(crate::render::TextureManager::new()));

		let materials_manager = Arc::new(RwLock::new(crate::render::MaterialManager::new()));

		
		let meshes_manager = Arc::new(RwLock::new(crate::render::MeshManager::new()));

		let renderer = pollster::block_on(
			crate::render::Renderer::new(
				adapter,
				&textures_manager,
				&meshes_manager,
				&materials_manager,
			)
		);

		Self {
			renderer,
			materials_manager,
			textures_manager,
			meshes_manager,
		}		
	}

	/// Loads materials AND their textures
	pub fn load_materials(&mut self) {
		let mut mm = self.materials_manager.write().unwrap();
		// let mats = crate::render::material::read_materials_file(&PathBuf::from("resources/kmaterials.ron"));
		// for mat in mats {
		// 	mm.insert(mat);
		// }
		todo!()
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
		Self {
			position: Vector3::from_element(0.0),
			rotation: UnitQuaternion::identity(),
			scale: Vector3::from_element(1.0),
		}
	}
	pub fn with_position(self, position: Vector3<f32>) -> Self {
		Self {
			position,
			rotation: self.rotation,
			scale: self.scale,
		}
	}
	pub fn with_rotation(self, rotation: UnitQuaternion<f32>) -> Self {
		Self {
			position: self.position,
			rotation,
			scale: self.scale,
		}
	}
	pub fn with_scale(self, scale: Vector3<f32>) -> Self {
		Self {
			position: self.position,
			rotation: self.rotation,
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
		MovementComponent {
			speed: 1.0,
		}
	}
	pub fn with_speed(self, speed: f32) -> Self {
		Self {
			speed,
		}
	}
}



// An entry in the mesh storage for a map component
#[derive(Debug)]
enum ChunkModelEntry {
	Empty,
	Unloaded,
	UnModeled,
	Complete(Vec<(usize, usize)>),
}



#[derive(Component, Debug)]
#[storage(VecStorage)]
struct MapComponent {
	map: crate::world::Map,
	// A field for storing generated mesh index collections (or a lack thereof)
	chunk_models: HashMap<[i32; 3], ChunkModelEntry>,
}
impl MapComponent {
	pub fn new(blockmanager: &Arc<RwLock<crate::world::BlockManager>>) -> Self {
		let mut map = crate::world::Map::new([4; 3], blockmanager);
		map.generate();
		let chunk_models = HashMap::new();
		Self {
			map,
			chunk_models,
		}		
	}

	// Regenerates chunk models if needed
	fn set_voxel(&mut self, pos: [i32; 3], voxel: Voxel) {
		self.map.set_voxel_world(pos, voxel);
		let (c, v) = self.map.world_chunk_voxel(pos);
		let [cdx, cdy, cdz] = self.map.chunk_dimensions;
		if v[0] as u32 >= cdx {
			let cx = [c[0]+1, c[1], c[2]];
			if self.chunk_models.contains_key(&cx) {
				self.chunk_models.insert(cx, ChunkModelEntry::UnModeled);
			}
		}
		if v[1] as u32 >= cdy {
			let cy = [c[0], c[1]+1, c[2]];
			if self.chunk_models.contains_key(&cy) {
				self.chunk_models.insert(cy, ChunkModelEntry::UnModeled);
			}
		}
		if v[2] as u32 >= cdx {
			let cz = [c[0], c[1], c[2]+1];
			if self.chunk_models.contains_key(&cz) {
				self.chunk_models.insert(cz, ChunkModelEntry::UnModeled);
			}
		}
		if self.chunk_models.contains_key(&c) {
			self.chunk_models.insert(c, ChunkModelEntry::UnModeled);
		}
	}
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
// Feeds the input resource
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
		let startt = step_resource.last_step;
		let endt = step_resource.this_step;
		let dt = (endt - startt).as_secs_f32();

		// Keyboard buttons
		let mut board_pressmap = HashMap::new();
		for key in &input_resource.board_presscache {
			board_pressmap.insert(*key, startt);
		}
		let mut kpmap = HashMap::new();
		
		// Mouse buttons
		let mut mouse_pressmap = HashMap::new();
		// Unlike key presses, mouse button presses are not constantly resubmitted
		for button in &input_resource.mouse_presscache {
			mouse_pressmap.insert(*button, startt);
		}
		let mut mpmap = HashMap::new();
		
		// Mouse position
		let mut mx = input_resource.mx;
		let mut my = input_resource.my;
		
		// Mouse movement
		let mut mdx = 0.0;
		let mut mdy = 0.0;
		
		// Drain items not passed the start of the step
		let events: Vec<EventWhen> = window_resource.event_queue.lock().unwrap().drain_filter(|e| e.ts < endt).collect();
		for event in events {
			let ts = event.ts;
			match event.event {
				Event::UserEvent(event) => {
					match event {
						EventLoopEvent::RegisterWindow(window) => {
							window_resource.register_window(window);
						},
						_ => {},
					}
				},
				Event::WindowEvent {ref event, window_id} => {
					match event {
						WindowEvent::KeyboardInput {input, ..} => {
							// Send changed key data
							if let Some(key) = input.virtual_keycode {
								match input.state {
									ElementState::Pressed => {
										// If this button was not already pressed, record the pressing
										if !board_pressmap.contains_key(&key) {
											board_pressmap.insert(key, ts);
										}
									},
									ElementState::Released => {
										// Only do something if this key had been pressed in the first place
										if board_pressmap.contains_key(&key) {
											let mut kp = (ts - board_pressmap[&key]).as_secs_f32() / dt;
			
											// If this key had been pressed and released, account for that
											if kpmap.contains_key(&key) {
												kp += kpmap[&key];
											}
											// Send the percent of time pressed to input
											kpmap.insert(key, kp);
			
											// Remove key from pressed keys
											board_pressmap.remove(&key);
										}
									},
								}
							}
							else {
								warn!("Key input with no virtual key code");
							}
						},
						WindowEvent::MouseInput {state, button, ..} => {
							let button = *button;
							info!("mb {:?}", &button);
							// Mouse button presses
							match state {
								ElementState::Pressed => {
									if !mouse_pressmap.contains_key(&button) {
										mouse_pressmap.insert(button, ts);
									}
								},
								ElementState::Released => {
									if mouse_pressmap.contains_key(&button) {
										let mut mp = (ts - mouse_pressmap[&button]).as_secs_f32() / dt;
										if mpmap.contains_key(&button) {
											mp += mpmap[&button];
										}
										mpmap.insert(button, mp);
										mouse_pressmap.remove(&button);
									}
								},
							}
						},
						WindowEvent::MouseWheel {delta, phase, ..} => {
						},
						WindowEvent::CursorEntered {..} => {
						},
						WindowEvent::CursorLeft {..} => {
						},
						WindowEvent::CursorMoved {position, ..} => {
							// Don't use this for camera control!
							// This can be used for ui stuff though
							mx = position.x;
							my = position.y;
						},
						WindowEvent::Resized (newsize) => {
							//self.windows[self.id_idx[&window_id]].resize(newsize);
						},
						WindowEvent::CloseRequested => {
							let idx = window_resource.id_idx[&window_id];
							window_resource.close_window(idx);
						},
						_ => {},
					}
				},
				Event::DeviceEvent {event, ..} => {
					match event {
						DeviceEvent::MouseMotion {delta} => {
							if mouse_pressmap.contains_key(&MouseButton::Left) {
								mdx += delta.0;
								mdy += delta.1;
							}
						},
						_ => {},
					}
				}
				_ => {},
			}
		}

		// Process the keys which are still pressed
		for (key, t) in &board_pressmap {
			let mut kp = (endt - *t).as_secs_f32() / dt;
			// If this key had been pressed and released, account for that
			if kpmap.contains_key(&key) {
				kp += kpmap[&key];
			}
			// Send the percent of time pressed to input
			kpmap.insert(*key, kp);
		}
		let board_stillpressed = board_pressmap.keys().map(|x| x.clone()).collect();
		// Again for mouse keys
		for (button, t) in &mouse_pressmap {
			let mut mp = (endt - *t).as_secs_f32() / dt;
			if mpmap.contains_key(&button) {
				mp += mpmap[&button];
			}
			mpmap.insert(*button, mp);
		}
		let mouse_stillpressed = mouse_pressmap.keys().map(|x| x.clone()).collect();

		// Update input resource
		input_resource.board_keys = kpmap;
		input_resource.board_presscache = board_stillpressed;
		input_resource.mouse_keys = mpmap;
		input_resource.mouse_presscache = mouse_stillpressed;
		input_resource.mx = mx;
		input_resource.my = my;
		//info!("mdx {}, mdy {}", mdx, mdy);
		input_resource.mdx = mdx;
		input_resource.mdy = mdy;
    }
}



// Reads input resource queue and decides what to do with it
struct InputSystem;
impl<'a> System<'a> for InputSystem {
    type SystemData = (
		ReadExpect<'a, InputResource>,
		ReadExpect<'a, StepResource>,
		WriteStorage<'a, TransformComponent>,
		ReadStorage<'a, MovementComponent>,
	);

	fn run(
		&mut self, 
		(
			input_resource, 
			step_resource,
			mut transform, 
			movement
		): Self::SystemData
	) { 
		let secs = step_resource.step_diff.as_secs_f32();

		let rx = input_resource.mdx as f32 * secs * 0.01;
		let ry = input_resource.mdy as f32 * secs * 0.01;

		let mut displacement = Vector3::from_element(0.0);
		for (key, kp) in &input_resource.board_keys {
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

		for (transform_c, movement_c) in (&mut transform, &movement).join() {

			let quat_ry = UnitQuaternion::from_euler_angles(ry, 0.0, 0.0);
			let quat_rx = UnitQuaternion::from_euler_angles(0.0, rx, 0.0);
			transform_c.rotation = quat_rx * transform_c.rotation * quat_ry;

			transform_c.position += transform_c.rotation * displacement * movement_c.speed * secs;
		}
	}
}



// The map system is responsible for loading and meshing chunks of maps near the cameras 
struct MapSystem;
impl MapSystem {
	fn model_chunk(
		renderr: &mut RenderResource,
		map: &crate::world::Map, 
		chunk_position: [i32; 3],
	) -> ChunkModelEntry {
		//info!("Evaluating chunk {:?} for modeling", chunk_position);
		if map.is_chunk_loaded(chunk_position) {
			info!("Modeling chunk {:?}", chunk_position);
			// Model it and register the segments
			let mesh_mats = {
				let mut mm = renderr.meshes_manager.write().unwrap();
				map.mesh_chunk(chunk_position).drain(..).map(|(material_idx, mesh)| {
					let mesh_idx = mm.insert(mesh);
					(mesh_idx, material_idx)
				}).collect::<Vec<_>>()
			};
			if mesh_mats.len() > 0 {
				//info!("Chunk {:?} modeled", chunk_position);
				ChunkModelEntry::Complete(mesh_mats)
			} else {
				info!("Chunk {:?} was empty", chunk_position);
				ChunkModelEntry::Empty
			}
		} else {
			//info!("Chunk {:?} was not available", chunk_position);
			ChunkModelEntry::Unloaded
		}
	}
}
impl<'a> System<'a> for MapSystem {
    type SystemData = (
		WriteExpect<'a, RenderResource>,
		WriteStorage<'a, MapComponent>,
		ReadStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
	);
	fn run(
		&mut self, 
		(
			mut render_resource,
			mut map,
			camera,
			transform,
		): Self::SystemData,
	) { 
		for map_c in (&mut map).join() {
			
			// Find all chunks which should be displayed
			let mut chunks_to_show = Vec::new();
			for (_, transform_c) in (&camera, &transform).join() {
				let camera_chunk = map_c.map.point_chunk(transform_c.position);
				let mut cposs = map_c.map.chunks_sphere(camera_chunk, 4);
				chunks_to_show.append(&mut cposs);
			}

			info!("Need to show {} chunks!", chunks_to_show.len());

			for chunk_position in chunks_to_show {
				if map_c.chunk_models.contains_key(&chunk_position) {
					match map_c.chunk_models[&chunk_position] {
						ChunkModelEntry::UnModeled => {
							// Model it
							let res = MapSystem::model_chunk(&mut render_resource, &map_c.map, chunk_position);
							map_c.chunk_models.insert(chunk_position, res);
						}
						_ => {},
					}
				} else { 
					let res = MapSystem::model_chunk(&mut render_resource, &map_c.map, chunk_position);
					map_c.chunk_models.insert(chunk_position, res);
				}
			}
		}
	}
}



struct RenderSystem;
impl<'a> System<'a> for RenderSystem {
    type SystemData = (
		WriteExpect<'a, RenderResource>,
		ReadExpect<'a, WindowResource>,
		// ReadStorage<'a, MeshComponent>,
		ReadStorage<'a, MapComponent>,
		ReadStorage<'a, CameraComponent>,
		ReadStorage<'a, TransformComponent>,
	);

	fn run(
		&mut self, 
		(
			mut render_resource, 
			window_resource,
			map,
			camera,
			transform,
		): Self::SystemData,
	) { 
		for (camera_c, transform_c) in (&camera, &transform).join() {
			
			match camera_c.target {
				// Find destination
				RenderTarget::Window(id) => {
					if id < window_resource.windows.len() {
						info!("Rendering to window idx {}", id);

						// Get window
						let window = &window_resource.windows[id];
						window.surface.configure(&render_resource.renderer.device, &window.surface_config);
						let width = window.surface_config.width;
						let height = window.surface_config.height;
						let stex = window.surface.get_current_texture().expect("fugg");
					
						// Collect render data
						let mut render_data = Vec::new();
						// Map chunks
						for (map_c, transform_c) in (&map, &transform).join() {
							// Renders ALL meshed chunks
							for (cp, entry) in &map_c.chunk_models {
								match entry {
									ChunkModelEntry::Complete(mesh_mats) => {
										use crate::render::*;
										let position = transform_c.position + map_c.map.chunk_point(*cp);
										let instance = Instance::new().with_position(position);
										for (mesh_idx, material_idx) in mesh_mats.iter().cloned() {
											let model_instance = ModelInstance {
												material_idx,
												mesh_idx,
												instance,
											};
											render_data.push(model_instance);
										}
									},
									_ => {},
								}
							}
						}
						render_resource.renderer.set_data(render_data);

						let camera = crate::render::Camera {
							position: transform_c.position,
							rotation: transform_c.rotation,
							fovy: camera_c.fovy,
							znear: camera_c.znear,
							zfar: camera_c.zfar,
						};
						
						render_resource.renderer.render(&stex.texture, width, height, &camera, Instant::now());

						stex.present();

						std::thread::sleep(std::time::Duration::from_millis(10000));
						panic!()
					} else {
						error!("Tried to render to nonexistent window! idx: {}", id);
					}
				},
				RenderTarget::Texture(_) => {
					todo!();
				},
			}
		}
	}
}



pub struct Game {
	world: World,
	blocks_manager: Arc<RwLock<crate::world::BlockManager>>,
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

		let blocks_manager = Arc::new(RwLock::new(crate::world::BlockManager::new()));

		let mut world = World::new();

		// Register components
		world.register::<TransformComponent>();
		world.register::<MovementComponent>();
		world.register::<MapComponent>();
		world.register::<CameraComponent>();

		// Attach resources
		let step_resource = StepResource::new();
		world.insert(step_resource);

		let render_resource = RenderResource::new(&adapter);
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
			.with(
				TransformComponent::new()
				.with_position(Vector3::new(0.0, 5.0, -5.0))
			)
			.with(MovementComponent{speed: 2.0})
			.build();
		// Map
		world.create_entity()
			.with(TransformComponent::new())
			.with(MapComponent::new(&blocks_manager))
			.build();

		// Called to tick the stuff
		let dispatcher = DispatcherBuilder::new()
			.with(WindowEventSystem, "window_system", &[])
			.with(InputSystem, "input_system", &["window_system"])
			.with(MapSystem, "map_system", &["input_system"])
			.with(RenderSystem, "render_system", &["input_system", "map_system"])
			.build();

		Self {
			world,
			blocks_manager,
			dispatcher,
		}
	}

	pub fn setup(&mut self) {
		// Load default materials
		let rr = self.world.write_resource::<RenderResource>();

		let mut mm = rr.materials_manager.write().unwrap();
		crate::render::Material::from_specification(&PathBuf::from("resources/materials/kmaterials.ron")).drain(..).for_each(|m| {mm.insert(m);});

		let mut tm = rr.textures_manager.write().unwrap();
		tm.insert(crate::render::Texture::new(&"dirt".to_string(), &PathBuf::from("resources/blockfaces/dirt.png")));

		// Make default block
		self.blocks_manager.write().unwrap().insert(crate::world::Block {
			name: "debug".to_string(),
			material_idx: 0,
		});
	}

	pub fn tick(&mut self) {
		info!("Tick!");
		let st = Instant::now();
		// Prepare step info
		{
			let mut step_resource = self.world.write_resource::<StepResource>();
			step_resource.last_step = step_resource.this_step;
			step_resource.this_step = std::time::Instant::now();
			step_resource.step_diff = step_resource.this_step - step_resource.last_step;
		}
		// Step to it
		self.dispatcher.dispatch(&mut self.world);

		let en = Instant::now();
		let dur = en - st;
		let tps = 1.0 / dur.as_secs_f32();
		info!("Tock! (duration {}ms, freq: {:.2}tps)", dur.as_millis(), tps);
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