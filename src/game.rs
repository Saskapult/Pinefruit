use specs::prelude::*;
use winit::event_loop::*;
use nalgebra::*;
use std::collections::HashMap;
use std::time::{Instant, Duration};
use std::sync::{Arc, Mutex, RwLock};
use std::path::PathBuf;
use rapier3d::prelude::*;
use crate::mesh::*;
use crate::material::*;
// use crate::texture::*;
use crate::ecs::*;
use crate::window::*;




pub struct Game {
	pub world: World,
	
	blocks_manager: Arc<RwLock<crate::world::BlockManager>>,
	
	window_manager: WindowManager,

	tick_dispatcher: Dispatcher<'static, 'static>,
	last_tick: Instant,
	entity_names_map: HashMap<Entity, String>,

	marker_entity: Option<Entity>,
	can_modify_block: bool,
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
		world.register::<ModelComponent>();
		world.register::<MapComponent>();
		world.register::<CameraComponent>();
		world.register::<DynamicPhysicsComponent>();
		world.register::<StaticPhysicsComponent>();

		// Attach resources
		let step_resource = StepResource::new();
		world.insert(step_resource);

		let render_resource = RenderResource::new(&adapter);
		world.insert(render_resource);

		let input_resource = InputResource::new();
		world.insert(input_resource);

		let physics_resource = PhysicsResource::new();
		world.insert(physics_resource);

		// Dispatcher(s?)
		let tick_dispatcher = DispatcherBuilder::new()
			.with(InputSystem, "input_system", &[])
			.with(ParallelMapSystem, "map_system", &["input_system"])
			.with(DynamicPhysicsSystem, "dynamic_physics_system", &["input_system"])
			.with(RenderDataSystem, "render_system", &["input_system", "map_system", "dynamic_physics_system"])
			.build();

		let window_manager = WindowManager::new(
			event_loop_proxy,
			instance,
			adapter,
			event_queue,
		);

		Self {
			world,
			blocks_manager,
			window_manager,
			tick_dispatcher,
			last_tick: Instant::now(),
			entity_names_map: HashMap::new(),
			marker_entity: None,
			can_modify_block: true,
		}
	}

	pub fn setup(&mut self) {
		// Material loading
		{
			let rr = self.world.write_resource::<RenderResource>();

			let mut matm = rr.materials_manager.write().unwrap();
			let mut texm = rr.textures_manager.write().unwrap();

			// Load some materials
			load_materials_file(
				"resources/materials/kmaterials.ron",
				&mut texm,
				&mut matm,
			).unwrap();
		}

		// Block loading
		{
			let mut bm = self.blocks_manager.write().unwrap();

			let rr = self.world.write_resource::<RenderResource>();
			let mut mm = rr.materials_manager.write().unwrap();
			let mut tm = rr.textures_manager.write().unwrap();

			crate::world::blocks::load_blocks_file(
				"resources/kblocks.ron",
				&mut bm,
				&mut tm,
				&mut mm,
			).unwrap();
		}

		// Teapot loading
		let teapot_mesh_idx = {
			let rr = self.world.write_resource::<RenderResource>();

			let mut meshm = rr.meshes_manager.write().unwrap();

			let (obj_models, _) = tobj::load_obj(
				"resources/not_for_git/bunny.obj", 
				&tobj::LoadOptions {
					triangulate: true,
					single_index: true,
					..Default::default()
				},
			).unwrap();
			let test_mesh = Mesh::from_obj_model(obj_models[0].clone()).unwrap();
			meshm.insert(test_mesh.clone())
		};

		// {
		// 	let mut pr = self.world.write_resource::<PhysicsResource>();
		// 	pr.add_ground()
		// }

		// Static and dynamic teapots
		{
			let collider_shape = {
				let rr = self.world.write_resource::<RenderResource>();
				let mm = rr.meshes_manager.read().unwrap();
				mm.index(teapot_mesh_idx).make_trimesh().unwrap()
			};

			// let tc = TransformComponent::new().with_position([0.0, 3.0, 0.0].into());
			// let spc = {
			// 	let mut pr = self.world.write_resource::<PhysicsResource>();
			// 	let mut spc = StaticPhysicsComponent::new(
			// 		&mut pr,
			// 	).with_transform(
			// 		&mut pr,
			// 		&tc,
			// 	);
			// 	spc.add_collider(
			// 		&mut pr, 
			// 		ColliderBuilder::new(collider_shape.clone()).density(100.0).build(),
			// 	);
			// 	spc
			// };
			// self.world.create_entity()
			// 	.with(tc)
			// 	.with(ModelComponent::new(teapot_mesh_idx, 0))
			// 	.with(spc)
			// 	.build();

			// let tc = TransformComponent::new().with_position([5.0, 10.0, 0.0].into());
			// let dpc = {
			// 	let mut pr = self.world.write_resource::<PhysicsResource>();
			// 	let mut dpc = DynamicPhysicsComponent::new(
			// 		&mut pr,
			// 	).with_transform(
			// 		&mut pr,
			// 		&tc,
			// 	);
			// 	dpc.add_collider(
			// 		&mut pr, 
			// 		ColliderBuilder::new(collider_shape.clone()).density(100.0).build(),
			// 	);
			// 	dpc
			// };
			// self.world.create_entity()
			// 	.with(tc)
			// 	.with(ModelComponent::new(teapot_mesh_idx, 0))
			// 	.with(dpc)
			// 	.build();
		}

		{
			// Camera
			self.world.create_entity()
				.with(CameraComponent::new())
				.with(
					TransformComponent::new()
					.with_position(Vector3::new(0.5, 5.5, 0.5))
				)
				.with(MovementComponent{speed: 4.0})
				.build();
			// Map
			let spc = StaticPhysicsComponent::new(
				&mut self.world.write_resource::<PhysicsResource>(),
			);
			self.world.create_entity()
				.with(TransformComponent::new())
				.with(MapComponent::new(&self.blocks_manager))
				.with(spc)
				.build();
		}
		

		// Place testing faces
		//self.make_testing_faces();
	}

	pub fn tick(&mut self) {
		// Run window update
		{
			let mut input_resource = self.world.write_resource::<InputResource>();
			self.window_manager.update(&mut input_resource);
		}

		// If should re-render then tick and re-render
		let now = Instant::now();
		if now - self.last_tick >= Duration::from_millis(20) { // 16.7 to 33.3
			self.last_tick = now;

			let tick_st = Instant::now();

			// Ray casting and marker entity pseudo-system
			// Needs to run before everything else because it requeires direct access to the pressed keys
			{
				// Set up/retrieve marker entity
				let marker_entity = match self.marker_entity {
					Some(e) => e,
					None => {
						self.marker_entity = Some(
							self.world.create_entity()
							.with(TransformComponent::new())
							.with(ModelComponent::new(0, 0))
							.build()
						);
						self.marker_entity.unwrap()
					},
				};

				let mut new_marker_pos = None;

				let cameras =  self.world.read_component::<CameraComponent>();
				let mut transforms =  self.world.write_component::<TransformComponent>();
				let mut maps =  self.world.write_component::<MapComponent>();

				for (_camera, transform) in (&cameras, &transforms).join() {
					
					for map in (&mut maps).join() {
						let map_raypositions = map.map.voxel_ray(
							&transform.position,
							&(transform.rotation * vector![0.0, 0.0, 1.0]),
							0.0,
							25.0,
						);

						let first_block_index = map_raypositions.iter().position(|&pos| {
							if let Some(v) = map.map.get_voxel_world(pos) {
								match v {
									crate::world::Voxel::Block(_) => true,
									_ => false,
								}
							} else {
								false
							}
						});
						let back_block_index = match first_block_index {
							Some(idx) => {
								if idx > 0 {
									Some(idx-1)
								} else {
									None
								}
							},
							None => None,
						};

						let first_block_pos = match first_block_index {
							Some(idx) => Some(map_raypositions[idx]),
							None => None,
						};
						let back_block_pos = match back_block_index {
							Some(idx) => Some(map_raypositions[idx]),
							None => None,
						};

						let g = self.world.write_resource::<InputResource>();

						// Make sure the line has no missing segments (in case I did it wrong)
						let distances = map_raypositions.as_slice().chunks_exact(2).map(|v| {
							let v1 = v[0];
							let v2 = v[1];
							let dist = v1.iter().zip(v2.iter())
								.map(|(p1, p2)| p1 - p2)
								.map(|g| g.pow(2) as f32)
								.sum::<f32>();
							dist.powf(0.5)
						}).collect::<Vec<f32>>();
						assert!(distances.iter().all(|&v| v == 1.0));
						
						if self.can_modify_block {	
							// Casted block placement
							if g.mouse_keys.contains_key(&winit::event::MouseButton::Right) {
								self.can_modify_block = false;

								if let Some(pos) = back_block_pos {
									map.set_voxel(pos, crate::world::Voxel::Block(0))
								}
							}

							// Casted block removal
							if g.mouse_keys.contains_key(&winit::event::MouseButton::Left) {
								self.can_modify_block = false;

								if let Some(pos) = first_block_pos {
									map.set_voxel(pos, crate::world::Voxel::Empty)
								}
							}
						} else if !(g.mouse_keys.contains_key(&winit::event::MouseButton::Left) || g.mouse_keys.contains_key(&winit::event::MouseButton::Right)) {
							self.can_modify_block = true;
						}

						// Positional block placement
						if g.board_keys.contains_key(&winit::event::VirtualKeyCode::H) {
							// The voxel the camera is in
							let pos = map.map.point_world_voxel(&transform.position);
							// Set it to dirt
							map.set_voxel(pos, crate::world::Voxel::Block(0));
						}

						// Update marker position
						if let Some(pos) = back_block_pos {
							new_marker_pos = Some(Vector3::new(
								pos[0] as f32 + 0.5, 
								pos[1] as f32 + 0.5, 
								pos[2] as f32 + 0.5,
							));
						} else {
							new_marker_pos = Some(Vector3::new(
								0.5, 
								0.5, 
								0.5,
							));
						}

					}
				}

				if let Some(pos) = new_marker_pos {
					let marker_transform = transforms.entry(marker_entity).unwrap().or_insert(TransformComponent::new());
					marker_transform.position = pos;
				}
			}

			self.tick_dispatcher.dispatch(&mut self.world);
			
			self.render_cameras();

			let dur = Instant::now() - tick_st;
			self.world.write_resource::<StepResource>().step_durations.record(dur);
			let tps = 1.0 / dur.as_secs_f32();
			info!("Tock! (duration {}ms, theoretical frequency: {:.2}tps)", dur.as_millis(), tps);
		}
	}

	fn render_cameras(&mut self) {

		let mut windows_to_redraw = Vec::new();
		
		{
			let cameras = self.world.read_component::<CameraComponent>();
			let transforms = self.world.read_component::<TransformComponent>();
			for (camera, camera_transform) in (&cameras, &transforms).join() {
				match camera.target {
					// Find destination
					RenderTarget::Window(id) => {
						if id >= self.window_manager.windows.len() {
							error!("Tried to render to nonexistent window! idx: {}", id);
							continue
						}
						// This is bad because it clones the camera's model queue
						windows_to_redraw.push((id, camera.clone(), camera_transform.clone()));
					},
					RenderTarget::Texture(_) => {
						todo!("Todo: Implement rendering to a texture");
					},
				}
			}
		}

		for (window_id, camera, camera_transform) in windows_to_redraw {
			let window = self.window_manager.windows.get_mut(window_id).unwrap();
			window.draw(
				&mut self.world, 
				&camera, 
				&camera_transform,
			);
		}
		
	}

	pub fn new_window(&mut self) {
		info!("Requesting new game window");

		self.window_manager.request_window();
	}
}




