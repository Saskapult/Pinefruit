use shipyard::*;
use winit::event_loop::*;
use std::time::{Instant, Duration};
use std::sync::{Arc, RwLock};
use crate::ecs::*;
use crate::window::*;
use crate::mesh::*;
use crate::texture::*;
use crate::material::*;
use crate::world::BlockManager;
use crate::gpu::*;




pub struct Game {
	pub world: World,
	
	pub textures: Arc<RwLock<TextureManager>>,
	pub meshes: Arc<RwLock<MeshManager>>,
	pub materials: Arc<RwLock<MaterialManager>>,
	pub blocks: Arc<RwLock<BlockManager>>,

	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,
	pub gpu_data: GpuData,
	event_loop_proxy: EventLoopProxy<WindowCommand>, 
	
	last_tick: Instant,
	tick_delay: Duration,
}
impl Game {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
		event_loop_proxy: EventLoopProxy<WindowCommand>, 
	) -> Self {
		let device = device.clone();
		let queue = queue.clone();

		let textures = Arc::new(RwLock::new(TextureManager::new()));
		let meshes = Arc::new(RwLock::new(MeshManager::new()));
		let materials = Arc::new(RwLock::new(MaterialManager::new()));
		let blocks = Arc::new(RwLock::new(BlockManager::new()));		

		let gpu_data = GpuData::new(
			&device,
			&queue,
			&textures,
			&meshes,
			&materials,
		);

		let world = World::new();
		world.add_unique(PhysicsResource::new());
		world.add_unique(TimeResource::new());

		world.add_workload(input_workload);

		Self {
			world,
			textures, meshes, materials, blocks, 
			device, queue, gpu_data, 
			event_loop_proxy,
			last_tick: Instant::now(),
			tick_delay: Duration::from_secs_f32(1.0 / 60.0),
		}
	}

	pub fn setup(&mut self) {
		// Load testing shaders
		self.gpu_data.shaders.register_path("./resources/shaders/acceleration_test.ron");
		self.gpu_data.shaders.register_path("./resources/shaders/blit.ron");

		// Material loading
		{
			let mut matm = self.materials.write().unwrap();
			let mut texm = self.textures.write().unwrap();

			// Load some materials
			load_materials_file(
				"resources/materials/kmaterials.ron",
				&mut texm,
				&mut matm,
			).unwrap();
		}

		// Block loading
		{
			let mut bm = self.blocks.write().unwrap();
			let mut mm = self.materials.write().unwrap();
			let mut tm = self.textures.write().unwrap();

			crate::world::blocks::load_blocks_file(
				"resources/kblocks.ron",
				&mut bm,
				&mut tm,
				&mut mm,
			).unwrap();
		}

		
		// // Map
		// self.world.create_entity()
		// 	.with(TransformComponent::new())
		// 	.with(MapComponent::new(&self.blocks_manager))
		// 	.build();
		
	}

	pub fn should_tick(&self) -> bool {
		self.last_tick.elapsed() >= self.tick_delay
	}

	pub fn tick(&mut self) {
		self.last_tick = Instant::now();

		{
			let mut time_resource = self.world.borrow::<UniqueViewMut<TimeResource>>().unwrap();
			(*time_resource).next_tick();
		}

		self.world.run_workload(input_workload).unwrap();

		self.world.run(control_movement_system);
	}

	pub fn new_window(&mut self) {
		info!("Requesting new game window");
		self.event_loop_proxy.send_event(WindowCommand::NewWindow).unwrap();
	}
}
