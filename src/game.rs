use shipyard::*;
use winit::event_loop::*;
use std::time::{Instant, Duration};
use std::sync::Arc;
use crate::ecs::*;
use crate::octree::chunk_to_octree;
use crate::window::*;
use crate::material::*;
use crate::world::{load_blocks_file, Chunk};
use crate::gpu::*;




pub struct Game {
	pub world: World,

	pub device: Arc<wgpu::Device>,
	pub queue: Arc<wgpu::Queue>,
	pub gpu_data: GraphicsData,

	event_loop_proxy: EventLoopProxy<WindowCommand>, 
	
	last_tick: Instant,
	tick_delay: Duration,
	pub last_tick_time: Option<Duration>,
}
impl Game {
	pub fn new(
		device: &Arc<wgpu::Device>,
		queue: &Arc<wgpu::Queue>,
		event_loop_proxy: EventLoopProxy<WindowCommand>, 
	) -> Self {
		let device = device.clone();
		let queue = queue.clone();

		let mut gpu_data = GraphicsData::new(&device, &queue);

		let world = World::new();
		world.add_unique(PhysicsResource::new());
		world.add_unique(TimeResource::new());
		world.add_unique(TextureResource::default());
		world.add_unique(MeshResource::default());
		world.add_unique(MaterialResource::default());
		world.add_unique(BlockResource::default());
		world.add_unique(GraphicsHandleResource {
			device: device.clone(),
			queue: queue.clone(),
		});
		world.add_unique(VoxelRenderingResource::new(
			&device,
			&mut gpu_data.shaders,
		));

		world.add_workload(input_workload);

		Self {
			world,
			device, queue, gpu_data, 
			event_loop_proxy,
			last_tick: Instant::now(),
			tick_delay: Duration::from_secs_f32(1.0 / 120.0),
			last_tick_time: None,

		}
	}

	pub fn setup(&mut self) {
		// self.gpu_data.shaders.register_path("./resources/shaders/acceleration_test.ron");
		
		self.world.run(setup_system);

		self.world.add_unique(MapResource::new([16; 3], 0));

		self.gpu_data.shaders.update_shaders();
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

		self.world.run(map_loading_system);
		self.world.run(map_octree_system);

		self.world.run(map_lookat_system);

		self.world.run(voxel_render_system);

		self.last_tick_time = Some(self.last_tick.elapsed());
	}

	pub fn new_window(&mut self) {
		info!("Requesting new game window");
		self.event_loop_proxy.send_event(WindowCommand::NewWindow).unwrap();
	}
}


fn load_test_octree(
	gpu: UniqueView<GraphicsHandleResource>,
	blocks: UniqueView<BlockResource>,
	mut voxel_data: UniqueViewMut<VoxelRenderingResource>,
) {
	let chunk = Chunk::from_compressed_mapped_rle("./map_saved/0/-3.-2.1.cmrle", [16; 3], &blocks.blocks).unwrap();
	let octree = chunk_to_octree(&chunk).unwrap();

	voxel_data.insert_chunk(&gpu.queue, [0, 0, 2], &octree);
	voxel_data.insert_chunk(&gpu.queue, [1, 0, 2], &octree);

	// println!("{voxel_data:#?}");

	info!("Buffer is now at {:.2}% capacity", voxel_data.buffer.capacity_frac() * 100.0);
}


fn setup_system(
	gpu: UniqueView<GraphicsHandleResource>,
	mut textures: UniqueViewMut<TextureResource>,
	mut materials: UniqueViewMut<MaterialResource>,
	mut blocks: UniqueViewMut<BlockResource>,
	mut voxel_data: UniqueViewMut<VoxelRenderingResource>,
) {
	load_materials_file(
		"resources/materials/kmaterials.ron",
		&mut textures.textures,
		&mut materials.materials,
	).unwrap();

	load_blocks_file(
		"resources/kblocks.ron",
		&mut blocks.blocks,
		&mut textures.textures,
		&mut materials.materials,
	).unwrap();

	voxel_data.update_colours(&gpu.queue, &blocks.blocks, &materials.materials, &textures.textures);
}
