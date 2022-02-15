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
	world: World,
	blocks_manager: Arc<RwLock<crate::world::BlockManager>>,
	window_dispatcher: Dispatcher<'static, 'static>,
	tick_dispatcher: Dispatcher<'static, 'static>,
	last_tick: Instant,
	entity_names_map: HashMap<Entity, String>,
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

		let window_resource = WindowResource::new(
			event_loop_proxy,
			instance,
			adapter,
			event_queue,
		);
		world.insert(window_resource);

		let input_resource = InputResource::new();
		world.insert(input_resource);

		let physics_resource = PhysicsResource::new();
		world.insert(physics_resource);

		// Entities
		// Camera
		world.create_entity()
			.with(CameraComponent::new())
			.with(
				TransformComponent::new()
				.with_position(Vector3::new(0.0, 5.0, -5.0))
			)
			.with(MovementComponent{speed: 3.0})
			.build();
		// Map
		world.create_entity()
			.with(TransformComponent::new())
			.with(MapComponent::new(&blocks_manager))
			.build();

		// Dispatchers
		let window_dispatcher = DispatcherBuilder::new()
			.with(WindowEventSystem, "window_system", &[])
			.build();
		let tick_dispatcher = DispatcherBuilder::new()
			.with(InputSystem, "input_system", &[])
			.with(MapSystem, "map_system", &["input_system"])
			.with(PhysicsInitializationSystem, "physics_init", &["input_system"])
			.with(DynamicPhysicsSystem, "dynamic_physics_system", &["input_system", "physics_init"])
			.with(RenderSystem, "render_system", &["input_system", "map_system", "dynamic_physics_system"])
			.build();

		Self {
			world,
			blocks_manager,
			window_dispatcher,
			tick_dispatcher,
			last_tick: Instant::now(),
			entity_names_map: HashMap::new(),
		}
	}

	fn make_testing_faces(&mut self) {
		use crate::world::*;
		let rr = self.world.write_resource::<RenderResource>();

		let xp_idx = {
			let xp = Mesh::new(&"xp_quad".to_string())
				.with_positions(XP_QUAD_VERTICES.iter().map(|v| [v[0], v[1], v[2]]).collect::<Vec<_>>())
				.with_normals((0..4).map(|_| [1.0, 0.0, 0.0]).collect::<Vec<_>>())
				.with_uvs(QUAD_UVS.iter().cloned().collect::<Vec<_>>())
				.with_indices(QUAD_INDICES.iter().cloned().collect::<Vec<_>>());
			rr.meshes_manager.write().unwrap().insert(xp)
		};
		let yp_idx = {
			let xp = Mesh::new(&"yp_quad".to_string())
				.with_positions(YP_QUAD_VERTICES.iter().map(|v| [v[0], v[1], v[2]]).collect::<Vec<_>>())
				.with_normals((0..4).map(|_| [1.0, 0.0, 0.0]).collect::<Vec<_>>())
				.with_uvs(QUAD_UVS.iter().cloned().collect::<Vec<_>>())
				.with_indices(REVERSE_QUAD_INDICES.iter().cloned().collect::<Vec<_>>());
			rr.meshes_manager.write().unwrap().insert(xp)
		};
		let zp_idx = {
			let xp = Mesh::new(&"zp_quad".to_string())
				.with_positions(ZP_QUAD_VERTICES.iter().map(|v| [v[0], v[1], v[2]]).collect::<Vec<_>>())
				.with_normals((0..4).map(|_| [1.0, 0.0, 0.0]).collect::<Vec<_>>())
				.with_uvs(QUAD_UVS.iter().cloned().collect::<Vec<_>>())
				.with_indices(QUAD_INDICES.iter().cloned().collect::<Vec<_>>());
			rr.meshes_manager.write().unwrap().insert(xp)
		};

		let xn_idx = {
			let xp = Mesh::new(&"xn_quad".to_string())
				.with_positions(XN_QUAD_VERTICES.iter().map(|v| [v[0], v[1], v[2]]).collect::<Vec<_>>())
				.with_normals((0..4).map(|_| [-1.0, 0.0, 0.0]).collect::<Vec<_>>())
				.with_uvs(QUAD_UVS.iter().cloned().collect::<Vec<_>>())
				.with_indices(REVERSE_QUAD_INDICES.iter().cloned().collect::<Vec<_>>());
			rr.meshes_manager.write().unwrap().insert(xp)
		};
		let yn_idx = {
			let xp = Mesh::new(&"yn_quad".to_string())
				.with_positions(YN_QUAD_VERTICES.iter().map(|v| [v[0], v[1], v[2]]).collect::<Vec<_>>())
				.with_normals((0..4).map(|_| [0.0, -1.0, 0.0]).collect::<Vec<_>>())
				.with_uvs(QUAD_UVS.iter().cloned().collect::<Vec<_>>())
				.with_indices(QUAD_INDICES.iter().cloned().collect::<Vec<_>>());
			rr.meshes_manager.write().unwrap().insert(xp)
		};
		let zn_idx = {
			let xp = Mesh::new(&"zn_quad".to_string())
				.with_positions(ZN_QUAD_VERTICES.iter().map(|v| [v[0], v[1], v[2]]).collect::<Vec<_>>())
				.with_normals((0..4).map(|_| [0.0, 0.0, -1.0]).collect::<Vec<_>>())
				.with_uvs(QUAD_UVS.iter().cloned().collect::<Vec<_>>())
				.with_indices(REVERSE_QUAD_INDICES.iter().cloned().collect::<Vec<_>>());
			rr.meshes_manager.write().unwrap().insert(xp)
		};

		drop(rr);

		self.world.create_entity()
			.with(TransformComponent::new().with_position([0.1, 0.0, 0.0].into()))
			.with(ModelComponent::new(xp_idx, 0))
			.build();
		self.world.create_entity()
			.with(TransformComponent::new().with_position([0.0, 0.1, 0.0].into()))
			.with(ModelComponent::new(yp_idx, 0))
			.build();
		self.world.create_entity()
			.with(TransformComponent::new().with_position([0.0, 0.0, 0.1].into()))
			.with(ModelComponent::new(zp_idx, 0))
			.build();

		self.world.create_entity()
			.with(TransformComponent::new().with_position([-0.1, 0.0, 0.0].into()))
			.with(ModelComponent::new(xn_idx, 0))
			.build();
		self.world.create_entity()
			.with(TransformComponent::new().with_position([0.0, -0.1, 0.0].into()))
			.with(ModelComponent::new(yn_idx, 0))
			.build();
		self.world.create_entity()
			.with(TransformComponent::new().with_position([0.0, 0.0, -0.1].into()))
			.with(ModelComponent::new(zn_idx, 0))
			.build();
	}

	pub fn setup(&mut self) {
		// Asset loading
		{
			let rr = self.world.write_resource::<RenderResource>();

			let mut matm = rr.materials_manager.write().unwrap();
			let mut texm = rr.textures_manager.write().unwrap();

			// Load some materials
			load_materials_file(
				PathBuf::from("resources/materials/kmaterials.ron"),
				&mut texm,
				&mut matm,
			).unwrap();
		}

		// Block loading
		{
			let mut bm = self.blocks_manager.write().unwrap();

			// Add some blocks
			bm.insert(crate::world::Block {
				name: "dirt".to_string(),
				material_idx: 0,
			});
			bm.insert(crate::world::Block {
				name: "stone".to_string(),
				material_idx: 1,
			});
			bm.insert(crate::world::Block {
				name: "cobblestone".to_string(),
				material_idx: 2,
			});
		}

		// Teapot loading
		let teapot_mesh_idx = {
			let rr = self.world.write_resource::<RenderResource>();

			let mut meshm = rr.meshes_manager.write().unwrap();

			let (obj_models, _) = tobj::load_obj(
				"resources/not_for_git/teapot.obj", 
				&tobj::LoadOptions {
					triangulate: true,
					single_index: true,
					..Default::default()
				},
			).unwrap();
			let test_mesh = Mesh::from_obj_model(obj_models[0].clone()).unwrap();
			meshm.insert(test_mesh.clone())
		};

		{
			let mut pr = self.world.write_resource::<PhysicsResource>();
			pr.add_ground()
		}

		// Static and dynamic teapots
		{
			self.world.create_entity()
				.with(TransformComponent::new().with_position([0.0, 3.0, 0.0].into()))
				.with(ModelComponent::new(teapot_mesh_idx, 0))
				.with(StaticPhysicsComponent::new())
				.build();
			
			self.world.create_entity()
				.with(TransformComponent::new().with_position([1.0, 10.0, 0.0].into()))
				.with(ModelComponent::new(teapot_mesh_idx, 0))
				.with(DynamicPhysicsComponent::new())
				.build();
		}
		

		// Place testing faces
		//self.make_testing_faces();
	}

	pub fn tick(&mut self) {
		self.window_dispatcher.dispatch(&mut self.world);

		let now = Instant::now();
		if now - self.last_tick >= Duration::from_millis(20) { // 16.7 to 33.3
			self.last_tick = now;

			info!("Tick!");
			let st = Instant::now();
			
			{ // Prepare step info
				let mut step_resource = self.world.write_resource::<StepResource>();
				step_resource.last_step = step_resource.this_step;
				step_resource.this_step = std::time::Instant::now();
				step_resource.step_diff = step_resource.this_step - step_resource.last_step;
			}

			self.tick_dispatcher.dispatch(&mut self.world);

			let en = Instant::now();
			let dur = en - st;
			let tps = 1.0 / dur.as_secs_f32();
			info!("Tock! (duration {}ms, theoretical frequency: {:.2}tps)", dur.as_millis(), tps);
		}
	}

	pub fn new_window(&mut self) {
		info!("Requesting new game window");

		let mut window_resource = self.world.write_resource::<WindowResource>();
		window_resource.request_window();
	}
}



